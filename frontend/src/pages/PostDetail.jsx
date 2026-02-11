import { useState, useEffect } from 'react';
import { useParams, useNavigate, Link } from 'react-router-dom';
import { postsAPI, commentsAPI, adminAPI } from '../api';
import { useAuth } from '../context/AuthContext';

const categoryLabels = {
    essay: '에세이',
    paper: '논문',
    report: '리포트',
    note: '노트',
    other: '기타',
};

const categoryEmojis = {
    essay: '✍️',
    paper: '📄',
    report: '📊',
    note: '📝',
    other: '📁',
};

const POST_DETAIL_CACHE_TTL_MS = 2000;
const postDetailRequestCache = new Map();

function getPostDeduped(postId) {
    const now = Date.now();
    const cached = postDetailRequestCache.get(postId);

    if (cached?.data && now - cached.timestamp < POST_DETAIL_CACHE_TTL_MS) {
        return Promise.resolve(cached.data);
    }

    if (cached?.promise) {
        return cached.promise;
    }

    const promise = postsAPI.getPost(postId)
        .then((data) => {
            postDetailRequestCache.set(postId, {
                data,
                timestamp: Date.now(),
                promise: null,
            });
            return data;
        })
        .catch((error) => {
            postDetailRequestCache.delete(postId);
            throw error;
        });

    postDetailRequestCache.set(postId, {
        data: cached?.data || null,
        timestamp: cached?.timestamp || 0,
        promise,
    });

    return promise;
}

function PostDetail() {
    const { id } = useParams();
    const navigate = useNavigate();
    const { user } = useAuth();

    const [post, setPost] = useState(null);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState(null);
    const [liking, setLiking] = useState(false);
    const [userLiked, setUserLiked] = useState(false);
    const [deleting, setDeleting] = useState(false);
    const [showDeleteConfirm, setShowDeleteConfirm] = useState(false);

    // Comments state
    const [comments, setComments] = useState([]);
    const [commentText, setCommentText] = useState('');
    const [commentSubmitting, setCommentSubmitting] = useState(false);
    const [commentError, setCommentError] = useState(null);

    useEffect(() => {
        let cancelled = false;

        const fetchPost = async () => {
            try {
                if (!cancelled) {
                    setLoading(true);
                }
                const data = await getPostDeduped(id);
                if (cancelled) return;
                setPost(data);
                setUserLiked(data.user_liked ?? false);
            } catch (err) {
                if (cancelled) return;
                console.error('Failed to fetch post:', err);
                if (err.response?.status === 404) {
                    setError('글을 찾을 수 없습니다.');
                } else {
                    setError('글을 불러오는데 실패했습니다.');
                }
            } finally {
                if (!cancelled) {
                    setLoading(false);
                }
            }
        };
        const fetchComments = async () => {
            try {
                const data = await commentsAPI.list(id);
                if (cancelled) return;
                setComments(data);
            } catch (err) {
                if (cancelled) return;
                console.error('Failed to fetch comments:', err);
            }
        };
        fetchPost();
        fetchComments();

        return () => {
            cancelled = true;
        };
    }, [id]);

    const handleLike = async () => {
        if (!user) {
            navigate('/login');
            return;
        }
        if (liking) return;
        setLiking(true);
        try {
            const result = await postsAPI.likePost(id);
            setPost(prev => ({ ...prev, like_count: result.like_count }));
            setUserLiked(result.user_liked);
        } catch (err) {
            console.error('Failed to like post:', err);
        } finally {
            setLiking(false);
        }
    };

    const handleDelete = async () => {
        if (deleting) return;
        if (!user) return;

        setDeleting(true);
        try {
            if (user.id === post.author_id) {
                await postsAPI.deletePost(id);
            } else if (user.is_admin) {
                await adminAPI.deletePost(id);
            } else {
                throw new Error('Not authorized to delete this post');
            }
            navigate('/');
        } catch (err) {
            console.error('Failed to delete post:', err);
            setDeleting(false);
            setShowDeleteConfirm(false);
        }
    };

    const handleCommentSubmit = async (e) => {
        e.preventDefault();
        if (!commentText.trim() || commentSubmitting) return;
        setCommentSubmitting(true);
        setCommentError(null);
        try {
            const newComment = await commentsAPI.create(id, commentText.trim());
            setComments(prev => [...prev, newComment]);
            setCommentText('');
        } catch (err) {
            console.error('Failed to create comment:', err);
            if (err.response?.status === 401) {
                setCommentError('로그인이 필요합니다.');
            } else {
                setCommentError('댓글 작성에 실패했습니다.');
            }
        } finally {
            setCommentSubmitting(false);
        }
    };

    const handleDeleteComment = async (commentId, commentAuthorId) => {
        if (!user) return;

        try {
            if (user.id === commentAuthorId) {
                await commentsAPI.delete(id, commentId);
            } else if (user.is_admin) {
                await adminAPI.deleteComment(commentId);
            } else {
                throw new Error('Not authorized to delete this comment');
            }
            setComments(prev => prev.filter(c => c.id !== commentId));
        } catch (err) {
            console.error('Failed to delete comment:', err);
        }
    };

    const isAuthor = user && post && user.id === post.author_id;
    const canDeletePost = user && post && (user.id === post.author_id || user.is_admin);
    const isPdf = post?.file_name?.toLowerCase().endsWith('.pdf');

    const formattedDate = post ? new Date(post.created_at).toLocaleDateString('ko-KR', {
        year: 'numeric',
        month: 'long',
        day: 'numeric',
        hour: '2-digit',
        minute: '2-digit',
    }) : '';

    const authorInitial = post?.author?.display_name?.[0] || post?.author?.username?.[0] || '?';
    const authorName = post?.author?.display_name || post?.author?.username || '익명';

    if (loading) {
        return (
            <main className="post-detail-page">
                <div className="container">
                    <div className="post-detail-skeleton">
                        <div className="skeleton-line skeleton-title" />
                        <div className="skeleton-line skeleton-meta" />
                        <div className="skeleton-line skeleton-content-1" />
                        <div className="skeleton-line skeleton-content-2" />
                        <div className="skeleton-line skeleton-content-3" />
                    </div>
                </div>
            </main>
        );
    }

    if (error) {
        return (
            <main className="post-detail-page">
                <div className="container">
                    <div className="post-detail-error">
                        <span className="post-detail-error-icon">😥</span>
                        <h2>{error}</h2>
                        <Link to="/" className="btn btn-primary">홈으로 돌아가기</Link>
                    </div>
                </div>
            </main>
        );
    }

    if (!post) return null;

    return (
        <main className="post-detail-page">
            <div className="container">
                <div className="post-detail-wrapper">
                    {/* Back Navigation */}
                    <Link to="/" className="post-detail-back">
                        ← 목록으로
                    </Link>

                    {/* Article Header */}
                    <article className="post-detail-article">
                        <header className="post-detail-header">
                            <span className="post-detail-category">
                                {categoryEmojis[post.category] || '📁'} {categoryLabels[post.category] || post.category}
                            </span>
                            <h1 className="post-detail-title">{post.title}</h1>

                            {post.summary && (
                                <p className="post-detail-summary">{post.summary}</p>
                            )}

                            <div className="post-detail-meta">
                                <div className="post-detail-author">
                                    <div className="post-detail-avatar">
                                        {post.author?.avatar_url ? (
                                            <img src={post.author.avatar_url} alt={authorName} />
                                        ) : (
                                            authorInitial.toUpperCase()
                                        )}
                                    </div>
                                    <div className="post-detail-author-info">
                                        <span className="post-detail-author-name">{authorName}</span>
                                        <span className="post-detail-date">{formattedDate}</span>
                                    </div>
                                </div>
                                <div className="post-detail-stats">
                                    <span className="post-detail-stat">🆔 {post.id}</span>
                                    <span className="post-detail-stat">👁️ {post.view_count}</span>
                                    <span className="post-detail-stat">❤️ {post.like_count}</span>
                                    {post.metrics?.citation_count !== undefined && (
                                        <span className="post-detail-stat">📚 {post.metrics.citation_count}</span>
                                    )}
                                </div>
                            </div>
                        </header>

                        {/* Content */}
                        <div className="post-detail-content">
                            {post.content.split('\n').map((paragraph, i) => (
                                paragraph.trim() ? <p key={i}>{paragraph}</p> : <br key={i} />
                            ))}
                        </div>

                        {/* Tags */}
                        {post.tags && post.tags.length > 0 && (
                            <div className="post-detail-tags">
                                {post.tags.map(tag => (
                                    <Link key={tag} to={`/?tag=${tag}`} className="post-tag-large">#{tag}</Link>
                                ))}
                            </div>
                        )}

                        {/* PDF Preview */}
                        {isPdf && (
                            <div className="pdf-preview-section">
                                <div className="pdf-preview-header">
                                    <span>📄 PDF 미리보기</span>
                                    <a
                                        href={`/${post.file_path}`}
                                        download={post.file_name}
                                        className="pdf-download-link"
                                    >
                                        ⬇️ 다운로드
                                    </a>
                                </div>
                                <div className="pdf-preview-container">
                                    <iframe
                                        src={`/${post.file_path}`}
                                        title={post.file_name}
                                        className="pdf-preview-iframe"
                                    />
                                </div>
                            </div>
                        )}

                        {/* File Attachment (non-PDF) */}
                        {post.file_name && !isPdf && (
                            <div className="post-detail-attachment">
                                <div className="attachment-label">📎 첨부파일</div>
                                <a
                                    href={`/${post.file_path}`}
                                    download={post.file_name}
                                    className="attachment-file"
                                >
                                    <span className="attachment-icon">📄</span>
                                    <span className="attachment-name">{post.file_name}</span>
                                    <span className="attachment-download">다운로드 ↓</span>
                                </a>
                            </div>
                        )}

                        {/* Actions */}
                        <div className="post-detail-actions">
                            <button
                                className={`post-action-btn like-btn ${liking ? 'liking' : ''} ${userLiked ? 'liked' : ''}`}
                                onClick={handleLike}
                                disabled={liking}
                            >
                                <span className="like-icon">{userLiked ? '❤️' : '🤍'}</span>
                                <span>좋아요 {post.like_count > 0 && post.like_count}</span>
                            </button>

                            {canDeletePost && (
                                <div className="post-author-actions">
                                    {isAuthor && (
                                        <Link
                                            to={`/posts/${post.id}/edit`}
                                            className="post-action-btn edit-btn"
                                        >
                                            ✏️ 수정
                                        </Link>
                                    )}
                                    {showDeleteConfirm ? (
                                        <div className="delete-confirm">
                                            <span>정말 삭제하시겠습니까?</span>
                                            <button
                                                className="btn-delete-confirm"
                                                onClick={handleDelete}
                                                disabled={deleting}
                                            >
                                                {deleting ? '삭제 중...' : '삭제'}
                                            </button>
                                            <button
                                                className="btn-delete-cancel"
                                                onClick={() => setShowDeleteConfirm(false)}
                                            >
                                                취소
                                            </button>
                                        </div>
                                    ) : (
                                        <button
                                            className="post-action-btn delete-btn"
                                            onClick={() => setShowDeleteConfirm(true)}
                                        >
                                            🗑️ {isAuthor ? '삭제' : '관리자 삭제'}
                                        </button>
                                    )}
                                </div>
                            )}
                        </div>

                        {/* Comments Section */}
                        <section className="comments-section">
                            <h2 className="comments-title">💬 댓글 {comments.length > 0 && <span className="comments-count">{comments.length}</span>}</h2>

                            {/* Comment Form */}
                            {user ? (
                                <form className="comment-form" onSubmit={handleCommentSubmit}>
                                    {commentError && (
                                        <div className="form-error" style={{ marginBottom: '0.75rem' }}>
                                            <span className="form-error-icon">⚠️</span>
                                            {commentError}
                                        </div>
                                    )}
                                    <div className="comment-form-row">
                                        <div className="comment-avatar">
                                            {user.display_name?.[0]?.toUpperCase() || user.username?.[0]?.toUpperCase() || '?'}
                                        </div>
                                        <textarea
                                            className="comment-input"
                                            placeholder="댓글을 작성하세요..."
                                            value={commentText}
                                            onChange={(e) => setCommentText(e.target.value)}
                                            rows={3}
                                        />
                                    </div>
                                    <div className="comment-form-actions">
                                        <button
                                            type="submit"
                                            className="btn btn-primary btn-sm"
                                            disabled={commentSubmitting || !commentText.trim()}
                                        >
                                            {commentSubmitting ? '등록 중...' : '댓글 등록'}
                                        </button>
                                    </div>
                                </form>
                            ) : (
                                <div className="comment-login-prompt">
                                    <Link to="/login">로그인</Link>하고 댓글을 남겨보세요.
                                </div>
                            )}

                            {/* Comment List */}
                            <div className="comment-list">
                                {comments.length === 0 ? (
                                    <div className="comment-empty">
                                        아직 댓글이 없습니다. 첫 댓글을 남겨보세요! 🙌
                                    </div>
                                ) : (
                                    comments.map(comment => {
                                        const commentAuthorInitial = comment.author?.display_name?.[0] || comment.author?.username?.[0] || '?';
                                        const commentAuthorName = comment.author?.display_name || comment.author?.username || '익명';
                                        const commentDate = new Date(comment.created_at).toLocaleDateString('ko-KR', {
                                            month: 'short',
                                            day: 'numeric',
                                            hour: '2-digit',
                                            minute: '2-digit',
                                        });
                                        const canDeleteComment = user && (user.id === comment.author_id || user.is_admin);

                                        return (
                                            <div key={comment.id} className="comment-item">
                                                <div className="comment-avatar">
                                                    {comment.author?.avatar_url ? (
                                                        <img src={comment.author.avatar_url} alt={commentAuthorName} />
                                                    ) : (
                                                        commentAuthorInitial.toUpperCase()
                                                    )}
                                                </div>
                                                <div className="comment-body">
                                                    <div className="comment-header">
                                                        <span className="comment-author">{commentAuthorName}</span>
                                                        <span className="comment-date">{commentDate}</span>
                                                        {canDeleteComment && (
                                                            <button
                                                                className="comment-delete-btn"
                                                                onClick={() => handleDeleteComment(comment.id, comment.author_id)}
                                                                title="삭제"
                                                            >
                                                                ✕
                                                            </button>
                                                        )}
                                                    </div>
                                                    <p className="comment-content">{comment.content}</p>
                                                </div>
                                            </div>
                                        );
                                    })
                                )}
                            </div>
                        </section>
                    </article>
                </div>
            </div>
        </main>
    );
}

export default PostDetail;
