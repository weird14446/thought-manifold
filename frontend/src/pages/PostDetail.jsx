import { useEffect, useMemo, useState } from 'react';
import { useParams, useNavigate, Link, useLocation } from 'react-router-dom';
import { postsAPI, commentsAPI, adminAPI, reviewsAPI, reviewCommentsAPI } from '../api';
import { useAuth } from '../context/AuthContext';
import { MarkdownEditorPreview, MarkdownRenderer } from '../components';

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

const reviewStatusLabels = {
    pending: '심사 대기중',
    completed: '심사 완료',
    failed: '심사 실패',
};

const reviewDecisionLabels = {
    accept: 'Accept',
    minor_revision: 'Minor Revision',
    major_revision: 'Major Revision',
    reject: 'Reject',
};

const paperStatusLabels = {
    draft: 'Draft',
    submitted: 'Submitted',
    revision: 'Revision',
    accepted: 'Accepted',
    published: 'Published',
    rejected: 'Rejected',
};

const POST_DETAIL_CACHE_TTL_MS = 2000;
const postDetailRequestCache = new Map();
const MAX_COMMENT_INDENT_LEVEL = 8;

function toTimestamp(value) {
    const time = new Date(value).getTime();
    return Number.isNaN(time) ? 0 : time;
}

function buildCommentTree(flatComments) {
    const nodeMap = new Map();
    const roots = [];

    flatComments.forEach((comment) => {
        nodeMap.set(comment.id, { ...comment, children: [] });
    });

    nodeMap.forEach((node) => {
        const parentId = node.parent_comment_id;
        if (parentId !== null && parentId !== undefined && nodeMap.has(parentId)) {
            nodeMap.get(parentId).children.push(node);
        } else {
            roots.push(node);
        }
    });

    const sortThread = (nodes) => {
        nodes.sort((a, b) => toTimestamp(a.created_at) - toTimestamp(b.created_at));
        nodes.forEach((child) => sortThread(child.children));
    };

    roots.forEach((root) => sortThread(root.children));
    roots.sort((a, b) => toTimestamp(b.created_at) - toTimestamp(a.created_at));

    return roots;
}

function getPostDeduped(postId, source = null) {
    const cacheKey = source ? `${postId}:${source}` : `${postId}`;
    const now = Date.now();
    const cached = postDetailRequestCache.get(cacheKey);

    if (cached?.data && now - cached.timestamp < POST_DETAIL_CACHE_TTL_MS) {
        return Promise.resolve(cached.data);
    }

    if (cached?.promise) {
        return cached.promise;
    }

    const params = {};
    if (source) params.source = source;

    const promise = postsAPI.getPost(postId, params)
        .then((data) => {
            postDetailRequestCache.set(cacheKey, {
                data,
                timestamp: Date.now(),
                promise: null,
            });
            return data;
        })
        .catch((error) => {
            postDetailRequestCache.delete(cacheKey);
            throw error;
        });

    postDetailRequestCache.set(cacheKey, {
        data: cached?.data || null,
        timestamp: cached?.timestamp || 0,
        promise,
    });

    return promise;
}

async function copyTextToClipboard(text) {
    if (navigator?.clipboard?.writeText) {
        await navigator.clipboard.writeText(text);
        return;
    }

    const textarea = document.createElement('textarea');
    textarea.value = text;
    textarea.setAttribute('readonly', '');
    textarea.style.position = 'absolute';
    textarea.style.left = '-9999px';
    document.body.appendChild(textarea);
    textarea.select();
    document.execCommand('copy');
    document.body.removeChild(textarea);
}

function PostDetail() {
    const { id } = useParams();
    const location = useLocation();
    const navigate = useNavigate();
    const { user } = useAuth();
    const source = new URLSearchParams(location.search).get('source');
    const reviewCenterSource = source === 'review_center' ? 'review_center' : null;

    const [post, setPost] = useState(null);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState(null);
    const [liking, setLiking] = useState(false);
    const [userLiked, setUserLiked] = useState(false);
    const [deleting, setDeleting] = useState(false);
    const [showDeleteConfirm, setShowDeleteConfirm] = useState(false);
    const [review, setReview] = useState(null);
    const [reviewLoading, setReviewLoading] = useState(false);
    const [reviewError, setReviewError] = useState(null);
    const [reviewRerunning, setReviewRerunning] = useState(false);
    const [publishing, setPublishing] = useState(false);
    const [versions, setVersions] = useState([]);
    const [versionsLoading, setVersionsLoading] = useState(false);
    const [selectedReviewVersionId, setSelectedReviewVersionId] = useState(null);

    // Comments state
    const [comments, setComments] = useState([]);
    const [commentText, setCommentText] = useState('');
    const [commentSubmitting, setCommentSubmitting] = useState(false);
    const [commentError, setCommentError] = useState(null);
    const [replyParentId, setReplyParentId] = useState(null);
    const [replyText, setReplyText] = useState('');
    const [replySubmitting, setReplySubmitting] = useState(false);
    const [replyError, setReplyError] = useState(null);

    // Review comments state
    const [reviewComments, setReviewComments] = useState([]);
    const [reviewCommentsLoading, setReviewCommentsLoading] = useState(false);
    const [reviewCommentsError, setReviewCommentsError] = useState(null);
    const [reviewCommentText, setReviewCommentText] = useState('');
    const [reviewCommentSubmitting, setReviewCommentSubmitting] = useState(false);
    const [reviewCommentError, setReviewCommentError] = useState(null);
    const [reviewReplyParentId, setReviewReplyParentId] = useState(null);
    const [reviewReplyText, setReviewReplyText] = useState('');
    const [reviewReplySubmitting, setReviewReplySubmitting] = useState(false);
    const [reviewReplyError, setReviewReplyError] = useState(null);
    const [copiedBibtexDoi, setCopiedBibtexDoi] = useState(null);

    useEffect(() => {
        let cancelled = false;

        const fetchPost = async () => {
            try {
                if (!cancelled) {
                    setLoading(true);
                }
                const data = await getPostDeduped(id, reviewCenterSource);
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
                setComments(Array.isArray(data) ? data : []);
            } catch (err) {
                if (cancelled) return;
                console.error('Failed to fetch comments:', err);
            }
        };
        fetchPost();
        fetchComments();
        setReplyParentId(null);
        setReplyText('');
        setReplyError(null);

        return () => {
            cancelled = true;
        };
    }, [id, reviewCenterSource]);

    const commentTree = useMemo(() => buildCommentTree(comments), [comments]);
    const reviewCommentTree = useMemo(() => buildCommentTree(reviewComments), [reviewComments]);

    const refreshComments = async () => {
        const data = await commentsAPI.list(id);
        setComments(Array.isArray(data) ? data : []);
    };

    const refreshReviewComments = async (versionId = selectedReviewVersionId) => {
        if (!post || !user || post.category !== 'paper') return;
        const data = await reviewCommentsAPI.list(post.id, versionId);
        setReviewComments(Array.isArray(data?.comments) ? data.comments : []);
    };

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
            await commentsAPI.create(id, commentText.trim(), null);
            await refreshComments();
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
            if (replyParentId === commentId) {
                setReplyParentId(null);
                setReplyText('');
                setReplyError(null);
            }
            await refreshComments();
        } catch (err) {
            console.error('Failed to delete comment:', err);
        }
    };

    const handleReplyToggle = (commentId) => {
        if (!user) {
            navigate('/login');
            return;
        }

        if (replyParentId === commentId) {
            setReplyParentId(null);
            setReplyText('');
            setReplyError(null);
            return;
        }

        setReplyParentId(commentId);
        setReplyText('');
        setReplyError(null);
    };

    const handleReplySubmit = async (e, parentCommentId) => {
        e.preventDefault();
        if (!replyText.trim() || replySubmitting) return;

        setReplySubmitting(true);
        setReplyError(null);

        try {
            await commentsAPI.create(id, replyText.trim(), parentCommentId);
            setReplyParentId(null);
            setReplyText('');
            await refreshComments();
        } catch (err) {
            console.error('Failed to create reply:', err);
            if (err.response?.status === 401) {
                setReplyError('로그인이 필요합니다.');
            } else {
                setReplyError('답글 작성에 실패했습니다.');
            }
        } finally {
            setReplySubmitting(false);
        }
    };

    const isAuthor = user && post && user.id === post.author_id;
    const canDeletePost = user && post && (user.id === post.author_id || user.is_admin);
    const isPdf = post?.file_name?.toLowerCase().endsWith('.pdf');
    const canViewAiReview = !!(
        post &&
        post.category === 'paper' &&
        user &&
        (user.id === post.author_id || user.is_admin)
    );
    const canAccessReviewComments = !!(
        post &&
        post.category === 'paper' &&
        user &&
        (post.current_revision || 0) > 0 &&
        (post.is_published || canViewAiReview)
    );

    useEffect(() => {
        let cancelled = false;

        const fetchLatestReview = async () => {
            if (!post || post.category !== 'paper' || !canViewAiReview) {
                setReview(null);
                setReviewError(null);
                return;
            }

            setReviewLoading(true);
            setReviewError(null);

            try {
                const data = await reviewsAPI.getLatest(post.id);
                if (!cancelled) {
                    setReview(data);
                }
            } catch (err) {
                if (cancelled) return;
                if ([401, 403, 404].includes(err.response?.status)) {
                    if (err.response?.status === 404) {
                        setReview(null);
                    } else {
                        setReview(null);
                        setReviewError(null);
                    }
                } else {
                    setReviewError(err.response?.data?.detail || 'AI 심사 결과를 불러오지 못했습니다.');
                }
            } finally {
                if (!cancelled) {
                    setReviewLoading(false);
                }
            }
        };

        fetchLatestReview();
        return () => {
            cancelled = true;
        };
    }, [post?.id, post?.category, post?.author_id, canViewAiReview]);

    useEffect(() => {
        if (!post || !canViewAiReview || review?.status !== 'pending') return;

        let cancelled = false;
        const timer = setInterval(async () => {
            try {
                const data = await reviewsAPI.getLatest(post.id);
                if (cancelled) return;
                setReview(data);
                if (data.status !== 'pending') {
                    clearInterval(timer);
                }
            } catch {
                // Keep silent polling behavior for transient errors.
            }
        }, 4000);

        return () => {
            cancelled = true;
            clearInterval(timer);
        };
    }, [post?.id, canViewAiReview, review?.status]);

    const handleRerunReview = async () => {
        if (!post || !canViewAiReview || reviewRerunning) return;
        setReviewRerunning(true);
        setReviewError(null);

        try {
            await reviewsAPI.rerun(post.id);
            const latest = await reviewsAPI.getLatest(post.id);
            setReview(latest);
        } catch (err) {
            setReviewError(err.response?.data?.detail || '재심사 요청에 실패했습니다.');
        } finally {
            setReviewRerunning(false);
        }
    };

    const handlePublishPaper = async () => {
        if (!post || publishing) return;
        setPublishing(true);
        try {
            await postsAPI.publishPost(post.id);
            postDetailRequestCache.clear();
            const params = reviewCenterSource ? { source: reviewCenterSource } : {};
            const refreshed = await postsAPI.getPost(post.id, params);
            setPost(refreshed);
        } catch (err) {
            setReviewError(err.response?.data?.detail || '논문 게재 처리에 실패했습니다.');
        } finally {
            setPublishing(false);
        }
    };

    useEffect(() => {
        let cancelled = false;

        const fetchVersions = async () => {
            if (!post || post.category !== 'paper' || !canViewAiReview) {
                setVersions([]);
                setSelectedReviewVersionId(null);
                return;
            }

            setVersionsLoading(true);
            try {
                const data = await postsAPI.getVersions(post.id, 50, 0);
                if (cancelled) return;
                const fetched = Array.isArray(data?.versions) ? data.versions : [];
                setVersions(fetched);
                if (fetched.length === 0) {
                    setSelectedReviewVersionId(null);
                } else if (!selectedReviewVersionId || !fetched.some((v) => v.id === selectedReviewVersionId)) {
                    setSelectedReviewVersionId(fetched[0].id);
                }
            } catch (err) {
                if (!cancelled) {
                    setVersions([]);
                    setSelectedReviewVersionId(null);
                }
            } finally {
                if (!cancelled) {
                    setVersionsLoading(false);
                }
            }
        };

        fetchVersions();
        return () => {
            cancelled = true;
        };
    }, [post?.id, post?.category, canViewAiReview]);

    useEffect(() => {
        let cancelled = false;

        const fetchReviewComments = async () => {
            if (!canAccessReviewComments || !post) {
                setReviewComments([]);
                setReviewCommentsError(null);
                return;
            }

            setReviewCommentsLoading(true);
            setReviewCommentsError(null);
            try {
                const versionId = canViewAiReview ? selectedReviewVersionId : null;
                const data = await reviewCommentsAPI.list(post.id, versionId);
                if (cancelled) return;
                setReviewComments(Array.isArray(data?.comments) ? data.comments : []);
            } catch (err) {
                if (cancelled) return;
                setReviewComments([]);
                setReviewCommentsError(err.response?.data?.detail || '심사 코멘트를 불러오지 못했습니다.');
            } finally {
                if (!cancelled) {
                    setReviewCommentsLoading(false);
                }
            }
        };

        fetchReviewComments();
        return () => {
            cancelled = true;
        };
    }, [post?.id, canAccessReviewComments, canViewAiReview, selectedReviewVersionId]);

    const handleReviewCommentSubmit = async (e) => {
        e.preventDefault();
        if (!post || !reviewCommentText.trim() || reviewCommentSubmitting) return;

        setReviewCommentSubmitting(true);
        setReviewCommentError(null);
        try {
            await reviewCommentsAPI.create(
                post.id,
                reviewCommentText.trim(),
                null,
                canViewAiReview ? selectedReviewVersionId : null,
            );
            setReviewCommentText('');
            await refreshReviewComments(canViewAiReview ? selectedReviewVersionId : null);
        } catch (err) {
            setReviewCommentError(err.response?.data?.detail || '심사 코멘트 작성에 실패했습니다.');
        } finally {
            setReviewCommentSubmitting(false);
        }
    };

    const handleReviewReplyToggle = (commentId) => {
        if (!user) {
            navigate('/login');
            return;
        }

        if (reviewReplyParentId === commentId) {
            setReviewReplyParentId(null);
            setReviewReplyText('');
            setReviewReplyError(null);
            return;
        }

        setReviewReplyParentId(commentId);
        setReviewReplyText('');
        setReviewReplyError(null);
    };

    const handleReviewReplySubmit = async (e, parentCommentId) => {
        e.preventDefault();
        if (!post || !reviewReplyText.trim() || reviewReplySubmitting) return;

        setReviewReplySubmitting(true);
        setReviewReplyError(null);
        try {
            await reviewCommentsAPI.create(
                post.id,
                reviewReplyText.trim(),
                parentCommentId,
                canViewAiReview ? selectedReviewVersionId : null,
            );
            setReviewReplyParentId(null);
            setReviewReplyText('');
            await refreshReviewComments(canViewAiReview ? selectedReviewVersionId : null);
        } catch (err) {
            setReviewReplyError(err.response?.data?.detail || '답글 작성에 실패했습니다.');
        } finally {
            setReviewReplySubmitting(false);
        }
    };

    const handleDeleteReviewComment = async (comment) => {
        if (!post || !user) return;

        try {
            await reviewCommentsAPI.delete(post.id, comment.id);
            if (reviewReplyParentId === comment.id) {
                setReviewReplyParentId(null);
                setReviewReplyText('');
                setReviewReplyError(null);
            }
            await refreshReviewComments(canViewAiReview ? selectedReviewVersionId : null);
        } catch (err) {
            setReviewCommentsError(err.response?.data?.detail || '심사 코멘트 삭제에 실패했습니다.');
        }
    };

    const handleCopyBibtex = async (doi, bibtex) => {
        if (!bibtex) return;
        try {
            await copyTextToClipboard(bibtex);
            setCopiedBibtexDoi(doi);
            window.setTimeout(() => {
                setCopiedBibtexDoi((current) => (current === doi ? null : current));
            }, 1500);
        } catch (copyError) {
            console.error('Failed to copy BibTeX', copyError);
            setCopiedBibtexDoi(null);
        }
    };

    const formattedDate = post ? new Date(post.created_at).toLocaleDateString('ko-KR', {
        year: 'numeric',
        month: 'long',
        day: 'numeric',
        hour: '2-digit',
        minute: '2-digit',
    }) : '';

    const authorInitial = post?.author?.display_name?.[0] || post?.author?.username?.[0] || '?';
    const authorName = post?.author?.display_name || post?.author?.username || '익명';

    const renderCommentNode = (comment, depth = 0) => {
        const visualDepth = Math.min(depth, MAX_COMMENT_INDENT_LEVEL);
        const commentAuthorInitial = comment.author?.display_name?.[0] || comment.author?.username?.[0] || '?';
        const commentAuthorName = comment.author?.display_name || comment.author?.username || '익명';
        const commentDate = new Date(comment.created_at).toLocaleDateString('ko-KR', {
            month: 'short',
            day: 'numeric',
            hour: '2-digit',
            minute: '2-digit',
        });
        const canDeleteComment = user && !comment.is_deleted && (user.id === comment.author_id || user.is_admin);
        const isReplyFormOpen = replyParentId === comment.id;

        return (
            <div
                key={comment.id}
                className="comment-node"
                style={{ '--comment-depth': visualDepth }}
            >
                <div className={`comment-item ${comment.is_deleted ? 'is-deleted' : ''}`}>
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
                            <div className="comment-actions">
                                <button
                                    type="button"
                                    className="comment-reply-btn"
                                    onClick={() => handleReplyToggle(comment.id)}
                                >
                                    {isReplyFormOpen ? '닫기' : '답글'}
                                </button>
                                {canDeleteComment && (
                                    <button
                                        type="button"
                                        className="comment-delete-btn"
                                        onClick={() => handleDeleteComment(comment.id, comment.author_id)}
                                        title="삭제"
                                    >
                                        ✕
                                    </button>
                                )}
                            </div>
                        </div>
                        <div className="comment-content">
                            {comment.is_deleted ? (
                                <p className="comment-deleted-placeholder">삭제된 댓글입니다.</p>
                            ) : (
                                <MarkdownRenderer content={comment.content} className="markdown-comment" />
                            )}
                        </div>
                    </div>
                </div>

                {isReplyFormOpen && user && (
                    <form
                        className="comment-reply-form"
                        onSubmit={(event) => handleReplySubmit(event, comment.id)}
                    >
                        {replyError && (
                            <div className="form-error" style={{ marginBottom: '0.75rem' }}>
                                <span className="form-error-icon">⚠️</span>
                                {replyError}
                            </div>
                        )}
                        <div className="comment-form-row">
                            <div className="comment-avatar">
                                {user.display_name?.[0]?.toUpperCase() || user.username?.[0]?.toUpperCase() || '?'}
                            </div>
                            <MarkdownEditorPreview
                                compact
                                value={replyText}
                                onChange={setReplyText}
                                placeholder="답글을 작성하세요..."
                                rows={4}
                                previewClassName="markdown-comment markdown-preview"
                                emptyText="답글 미리보기가 여기에 표시됩니다."
                            />
                        </div>
                        <div className="comment-form-actions">
                            <button
                                type="submit"
                                className="btn btn-primary btn-sm"
                                disabled={replySubmitting || !replyText.trim()}
                            >
                                {replySubmitting ? '등록 중...' : '답글 등록'}
                            </button>
                        </div>
                    </form>
                )}

                {comment.children?.length > 0 && (
                    <div className="comment-children">
                        {comment.children.map((child) => renderCommentNode(child, depth + 1))}
                    </div>
                )}
            </div>
        );
    };

    const renderReviewCommentNode = (comment, depth = 0) => {
        const visualDepth = Math.min(depth, MAX_COMMENT_INDENT_LEVEL);
        const commentAuthorInitial = comment.author?.display_name?.[0] || comment.author?.username?.[0] || '?';
        const commentAuthorName = comment.author?.display_name || comment.author?.username || '익명';
        const commentDate = new Date(comment.created_at).toLocaleDateString('ko-KR', {
            month: 'short',
            day: 'numeric',
            hour: '2-digit',
            minute: '2-digit',
        });
        const canDeleteReviewComment = user && !comment.is_deleted && (
            user.is_admin ||
            user.id === comment.author_id ||
            (!post.is_published && user.id === post.author_id)
        );
        const isReplyFormOpen = reviewReplyParentId === comment.id;

        return (
            <div
                key={comment.id}
                className="comment-node"
                style={{ '--comment-depth': visualDepth }}
            >
                <div className={`comment-item ${comment.is_deleted ? 'is-deleted' : ''}`}>
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
                            <div className="comment-actions">
                                <button
                                    type="button"
                                    className="comment-reply-btn"
                                    onClick={() => handleReviewReplyToggle(comment.id)}
                                >
                                    {isReplyFormOpen ? '닫기' : '답글'}
                                </button>
                                {canDeleteReviewComment && (
                                    <button
                                        type="button"
                                        className="comment-delete-btn"
                                        onClick={() => handleDeleteReviewComment(comment)}
                                        title="삭제"
                                    >
                                        ✕
                                    </button>
                                )}
                            </div>
                        </div>
                        <div className="comment-content">
                            {comment.is_deleted ? (
                                <p className="comment-deleted-placeholder">삭제된 댓글입니다.</p>
                            ) : (
                                <MarkdownRenderer content={comment.content} className="markdown-comment" />
                            )}
                        </div>
                    </div>
                </div>

                {isReplyFormOpen && user && (
                    <form
                        className="comment-reply-form"
                        onSubmit={(event) => handleReviewReplySubmit(event, comment.id)}
                    >
                        {reviewReplyError && (
                            <div className="form-error" style={{ marginBottom: '0.75rem' }}>
                                <span className="form-error-icon">⚠️</span>
                                {reviewReplyError}
                            </div>
                        )}
                        <div className="comment-form-row">
                            <div className="comment-avatar">
                                {user.display_name?.[0]?.toUpperCase() || user.username?.[0]?.toUpperCase() || '?'}
                            </div>
                            <MarkdownEditorPreview
                                compact
                                value={reviewReplyText}
                                onChange={setReviewReplyText}
                                placeholder="심사 답글을 작성하세요..."
                                rows={4}
                                previewClassName="markdown-comment markdown-preview"
                                emptyText="답글 미리보기가 여기에 표시됩니다."
                            />
                        </div>
                        <div className="comment-form-actions">
                            <button
                                type="submit"
                                className="btn btn-primary btn-sm"
                                disabled={reviewReplySubmitting || !reviewReplyText.trim()}
                            >
                                {reviewReplySubmitting ? '등록 중...' : '답글 등록'}
                            </button>
                        </div>
                    </form>
                )}

                {comment.children?.length > 0 && (
                    <div className="comment-children">
                        {comment.children.map((child) => renderReviewCommentNode(child, depth + 1))}
                    </div>
                )}
            </div>
        );
    };

    if (loading) {
        return (
            <main className="post-detail-page page-article-layout">
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
            <main className="post-detail-page page-article-layout">
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
        <main className="post-detail-page page-article-layout">
            <div className="container">
                <div className="post-detail-wrapper">
                    {/* Back Navigation */}
                    <Link to="/" className="post-detail-back">
                        ← 목록으로
                    </Link>

                    {/* Article Header */}
                    <article className="post-detail-article page-article">
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
                                    {post.category === 'paper' && (
                                        <span className="post-detail-stat">🗂️ Rev v{post.current_revision ?? 0}</span>
                                    )}
                                    {post.category === 'paper' && (
                                        <span className="post-detail-stat">🧭 {paperStatusLabels[post.paper_status] || post.paper_status}</span>
                                    )}
                                </div>
                            </div>

                            {post.github_url && (
                                <a
                                    className="post-detail-github-link"
                                    href={post.github_url}
                                    target="_blank"
                                    rel="noopener noreferrer"
                                >
                                    🔗 GitHub 링크 열기
                                </a>
                            )}

                            {Array.isArray(post.doi_metadata) && post.doi_metadata.length > 0 && (
                                <div className="post-detail-doi-metadata">
                                    <h3 className="post-detail-doi-title">🔎 DOI 메타데이터</h3>
                                    <ul className="post-detail-doi-list">
                                        {post.doi_metadata.map((item) => (
                                            <li key={item.doi} className="post-detail-doi-item">
                                                <a
                                                    href={item.source_url || `https://doi.org/${item.doi}`}
                                                    target="_blank"
                                                    rel="noopener noreferrer"
                                                >
                                                    DOI: {item.doi}
                                                </a>
                                                {item.title && <p className="post-detail-doi-text">{item.title}</p>}
                                                <p className="post-detail-doi-text">
                                                    {[item.journal, item.publisher, item.published_at]
                                                        .filter(Boolean)
                                                        .join(' · ') || '메타데이터 수집 대기중'}
                                                </p>
                                                {item.bibtex && (
                                                    <div className="post-detail-bibtex">
                                                        <div className="post-detail-bibtex-header">
                                                            <span>BibTeX</span>
                                                            <button
                                                                type="button"
                                                                className="post-detail-bibtex-copy"
                                                                onClick={() => handleCopyBibtex(item.doi, item.bibtex)}
                                                            >
                                                                {copiedBibtexDoi === item.doi ? '복사됨' : '복사'}
                                                            </button>
                                                        </div>
                                                        <pre className="post-detail-bibtex-pre">
                                                            <code>{item.bibtex}</code>
                                                        </pre>
                                                    </div>
                                                )}
                                            </li>
                                        ))}
                                    </ul>
                                </div>
                            )}
                        </header>

                        {post.category === 'paper' && canViewAiReview && (
                            <section className="post-ai-review-card">
                                <div className="post-ai-review-header">
                                    <div>
                                        <h2>🤖 AI 논문 심사</h2>
                                        <p>편집자 1차 심사 및 동료심사 대체 결과</p>
                                    </div>
                                    <div className="post-ai-review-header-actions">
                                        <span className={`ai-review-status-badge ${review?.status || 'empty'}`}>
                                            {reviewStatusLabels[review?.status] || '심사 이력 없음'}
                                        </span>
                                        <button
                                            type="button"
                                            className="btn btn-secondary btn-sm"
                                            onClick={handleRerunReview}
                                            disabled={reviewRerunning}
                                        >
                                            {reviewRerunning ? '재심사 요청 중...' : '재심사 실행'}
                                        </button>
                                        {post.paper_status === 'accepted' && !post.is_published && (
                                            <button
                                                type="button"
                                                className="btn btn-primary btn-sm"
                                                onClick={handlePublishPaper}
                                                disabled={publishing}
                                            >
                                                {publishing ? '게재 처리 중...' : '게재하기'}
                                            </button>
                                        )}
                                    </div>
                                </div>

                                {reviewLoading ? (
                                    <div className="post-ai-review-loading">심사 결과를 불러오는 중...</div>
                                ) : reviewError ? (
                                    <div className="post-ai-review-error">{reviewError}</div>
                                ) : !review ? (
                                    <div className="post-ai-review-empty">등록된 AI 심사 이력이 없습니다.</div>
                                ) : (
                                    <div className="post-ai-review-content">
                                        <div className="post-ai-review-decision">
                                            <span className="label">최종 판정</span>
                                            <span className="value">
                                                {reviewDecisionLabels[review.decision] || '-'}
                                            </span>
                                        </div>
                                        <div className="post-ai-review-scores">
                                            <div className="score-item">
                                                <span>총점</span>
                                                <strong>{review.scores?.overall_score ?? '-'}</strong>
                                            </div>
                                            <div className="score-item">
                                                <span>참신성</span>
                                                <strong>{review.scores?.novelty_score ?? '-'}</strong>
                                            </div>
                                            <div className="score-item">
                                                <span>방법론</span>
                                                <strong>{review.scores?.methodology_score ?? '-'}</strong>
                                            </div>
                                            <div className="score-item">
                                                <span>명확성</span>
                                                <strong>{review.scores?.clarity_score ?? '-'}</strong>
                                            </div>
                                            <div className="score-item">
                                                <span>인용 정합성</span>
                                                <strong>{review.scores?.citation_integrity_score ?? '-'}</strong>
                                            </div>
                                        </div>

                                        <div className="post-ai-review-block">
                                            <h3>편집자 대체 요약</h3>
                                            <p>{review.editorial?.summary || '-'}</p>
                                        </div>

                                        <div className="post-ai-review-block">
                                            <h3>동료심사 대체 종합</h3>
                                            <p>{review.peer?.summary || '-'}</p>
                                        </div>

                                        <div className="post-ai-review-lists">
                                            <div className="review-list-item">
                                                <h4>주요 이슈</h4>
                                                {review.peer?.major_issues?.length ? (
                                                    <ul>{review.peer.major_issues.map((item) => <li key={item}>{item}</li>)}</ul>
                                                ) : <p>-</p>}
                                            </div>
                                            <div className="review-list-item">
                                                <h4>경미 이슈</h4>
                                                {review.peer?.minor_issues?.length ? (
                                                    <ul>{review.peer.minor_issues.map((item) => <li key={item}>{item}</li>)}</ul>
                                                ) : <p>-</p>}
                                            </div>
                                            <div className="review-list-item">
                                                <h4>필수 수정사항</h4>
                                                {review.peer?.required_revisions?.length ? (
                                                    <ul>{review.peer.required_revisions.map((item) => <li key={item}>{item}</li>)}</ul>
                                                ) : <p>-</p>}
                                            </div>
                                            <div className="review-list-item">
                                                <h4>강점</h4>
                                                {review.peer?.strengths?.length ? (
                                                    <ul>{review.peer.strengths.map((item) => <li key={item}>{item}</li>)}</ul>
                                                ) : <p>-</p>}
                                            </div>
                                        </div>

                                        {review.status === 'failed' && review.error_message && (
                                            <div className="post-ai-review-error">
                                                {review.error_message}
                                            </div>
                                        )}
                                    </div>
                                )}
                            </section>
                        )}

                        {!post.is_published && canViewAiReview && (
                            <div className="post-unpublished-notice">
                                현재 이 논문은 공개 전 상태({paperStatusLabels[post.paper_status] || post.paper_status})입니다. <Link to="/reviews">AI 심사 센터</Link>에서 진행 상태를 확인하세요.
                            </div>
                        )}

                        {post.category === 'paper' && (canViewAiReview || post.is_published) && (
                            <section className="paper-workflow-panel">
                                <div className="paper-workflow-header">
                                    <h2>📑 Revision & 심사 코멘트</h2>
                                    {canViewAiReview && selectedReviewVersionId && (
                                        <span className="paper-workflow-version-badge">
                                            현재 선택: v{versions.find((v) => v.id === selectedReviewVersionId)?.version_number ?? '-'}
                                        </span>
                                    )}
                                </div>

                                {canViewAiReview && (
                                    <div className="paper-version-list">
                                        {versionsLoading ? (
                                            <div className="paper-version-empty">버전 이력을 불러오는 중...</div>
                                        ) : versions.length === 0 ? (
                                            <div className="paper-version-empty">등록된 제출 버전이 없습니다.</div>
                                        ) : (
                                            versions.map((version) => (
                                                <button
                                                    key={version.id}
                                                    type="button"
                                                    className={`paper-version-item ${selectedReviewVersionId === version.id ? 'active' : ''}`}
                                                    onClick={() => setSelectedReviewVersionId(version.id)}
                                                >
                                                    <strong>Revision v{version.version_number}</strong>
                                                    <span>{new Date(version.submitted_at).toLocaleString('ko-KR')}</span>
                                                </button>
                                            ))
                                        )}
                                    </div>
                                )}

                                {!user && post.is_published ? (
                                    <div className="comment-login-prompt">
                                        <Link to="/login">로그인</Link>하면 심사 코멘트를 확인하고 답글을 남길 수 있습니다.
                                    </div>
                                ) : canAccessReviewComments ? (
                                    <div className="paper-review-comments">
                                        <form className="comment-form" onSubmit={handleReviewCommentSubmit}>
                                            {reviewCommentError && (
                                                <div className="form-error" style={{ marginBottom: '0.75rem' }}>
                                                    <span className="form-error-icon">⚠️</span>
                                                    {reviewCommentError}
                                                </div>
                                            )}
                                            <div className="comment-form-row">
                                                <div className="comment-avatar">
                                                    {user.display_name?.[0]?.toUpperCase() || user.username?.[0]?.toUpperCase() || '?'}
                                                </div>
                                                <MarkdownEditorPreview
                                                    compact
                                                    value={reviewCommentText}
                                                    onChange={setReviewCommentText}
                                                    placeholder="심사 코멘트를 작성하세요..."
                                                    rows={4}
                                                    previewClassName="markdown-comment markdown-preview"
                                                    emptyText="심사 코멘트 미리보기가 여기에 표시됩니다."
                                                />
                                            </div>
                                            <div className="comment-form-actions">
                                                <button
                                                    type="submit"
                                                    className="btn btn-primary btn-sm"
                                                    disabled={reviewCommentSubmitting || !reviewCommentText.trim()}
                                                >
                                                    {reviewCommentSubmitting ? '등록 중...' : '심사 코멘트 등록'}
                                                </button>
                                            </div>
                                        </form>

                                        <div className="comment-list">
                                            {reviewCommentsLoading ? (
                                                <div className="comment-empty">심사 코멘트를 불러오는 중...</div>
                                            ) : reviewCommentsError ? (
                                                <div className="comment-empty">{reviewCommentsError}</div>
                                            ) : reviewCommentTree.length === 0 ? (
                                                <div className="comment-empty">등록된 심사 코멘트가 없습니다.</div>
                                            ) : (
                                                reviewCommentTree.map((comment) => renderReviewCommentNode(comment))
                                            )}
                                        </div>
                                    </div>
                                ) : null}
                            </section>
                        )}

                        {/* Content */}
                        <div className="post-detail-content">
                            <MarkdownRenderer content={post.content} className="markdown-post" />
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
                                        <MarkdownEditorPreview
                                            compact
                                            value={commentText}
                                            onChange={setCommentText}
                                            placeholder="댓글을 작성하세요..."
                                            rows={4}
                                            previewClassName="markdown-comment markdown-preview"
                                            emptyText="댓글 미리보기가 여기에 표시됩니다."
                                        />
                                    </div>
                                    <span className="form-hint">수식은 `$...$`(inline), `$$...$$`(block) 문법을 사용할 수 있습니다.</span>
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
                                {commentTree.length === 0 ? (
                                    <div className="comment-empty">
                                        아직 댓글이 없습니다. 첫 댓글을 남겨보세요! 🙌
                                    </div>
                                ) : (
                                    commentTree.map((comment) => renderCommentNode(comment))
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
