import { Link } from 'react-router-dom';
import { getPostExcerptMarkdown } from '../utils/markdown';
import MarkdownRenderer from './MarkdownRenderer';

const categoryLabels = {
    essay: '에세이',
    paper: '논문',
    report: '리포트',
    note: '노트',
    other: '기타',
};

function PostCard({ post }) {
    const authorName = post.author?.display_name || post.author?.username || '익명';
    const formattedDate = new Date(post.created_at).toLocaleDateString('ko-KR', {
        year: 'numeric',
        month: 'short',
        day: 'numeric',
    });
    const excerptMarkdown = getPostExcerptMarkdown(post);

    return (
        <Link to={`/posts/${post.id}`} className="post-card">
            <div className="post-card-header">
                <span className="post-category">
                    {categoryLabels[post.category] || post.category}
                </span>
                <span className="post-date">{formattedDate}</span>
            </div>

            <div className="post-card-body">
                <h3 className="post-title">{post.title}</h3>
                <p className="post-card-byline">by {authorName}</p>
                <div className="post-excerpt">
                    <MarkdownRenderer
                        content={excerptMarkdown}
                        className="markdown-card-excerpt"
                        enableInteractiveEmbeds={false}
                    />
                </div>
                {post.tags && post.tags.length > 0 && (
                    <div className="post-card-tags">
                        {post.tags.map(tag => (
                            <Link
                                key={tag}
                                to={`/?tag=${tag}`}
                                className="post-tag"
                                onClick={(e) => e.stopPropagation()}
                            >
                                #{tag}
                            </Link>
                        ))}
                    </div>
                )}
            </div>

            <div className="post-card-footer">
                <div className="post-stats">
                    <span className="post-stat">
                        <span className="post-stat-icon">👁️</span>
                        {post.view_count}
                    </span>
                    <span className="post-stat">
                        <span className="post-stat-icon">❤️</span>
                        {post.like_count}
                    </span>
                </div>
                <span className="post-card-read-more">Read Article</span>
                {post.file_name && (
                    <span className="post-file-badge">
                        📎 {post.file_name}
                    </span>
                )}
            </div>
        </Link>
    );
}

export default PostCard;
