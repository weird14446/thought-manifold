import { Link } from 'react-router-dom';

const categoryLabels = {
    essay: '에세이',
    paper: '논문',
    report: '리포트',
    note: '노트',
    other: '기타',
};

function PostCard({ post }) {
    const authorInitial = post.author?.display_name?.[0] || post.author?.username?.[0] || '?';
    const authorName = post.author?.display_name || post.author?.username || '익명';
    const formattedDate = new Date(post.created_at).toLocaleDateString('ko-KR', {
        year: 'numeric',
        month: 'short',
        day: 'numeric',
    });

    return (
        <Link to={`/posts/${post.id}`} className="post-card">
            <div className="post-card-header">
                <div className="post-author-avatar">
                    {authorInitial.toUpperCase()}
                </div>
                <div className="post-author-info">
                    <div className="post-author-name">{authorName}</div>
                    <div className="post-date">{formattedDate}</div>
                </div>
                <span className="post-category">
                    {categoryLabels[post.category] || post.category}
                </span>
            </div>

            <div className="post-card-body">
                <h3 className="post-title">{post.title}</h3>
                <p className="post-excerpt">
                    {post.summary || post.content.slice(0, 150) + '...'}
                </p>
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
