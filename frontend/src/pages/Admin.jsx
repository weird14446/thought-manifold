import { useState, useEffect } from 'react';
import { useNavigate, Link } from 'react-router-dom';
import { adminAPI, postsAPI } from '../api';
import { useAuth } from '../context/AuthContext';

function Admin() {
    const { user } = useAuth();
    const navigate = useNavigate();

    const [stats, setStats] = useState(null);
    const [users, setUsers] = useState([]);
    const [managedPosts, setManagedPosts] = useState([]);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState(null);
    const [contentLoading, setContentLoading] = useState(true);
    const [contentError, setContentError] = useState(null);
    const [reviewItems, setReviewItems] = useState([]);
    const [reviewsLoading, setReviewsLoading] = useState(true);
    const [reviewsError, setReviewsError] = useState(null);
    const [reviewStatusFilter, setReviewStatusFilter] = useState('');
    const [activeTab, setActiveTab] = useState('overview'); // overview, users, content, reviews

    useEffect(() => {
        if (!user || !user.is_admin) {
            navigate('/');
            return;
        }

        fetchData();
        fetchManagedPosts();
        fetchAiReviews();
    }, [user, navigate]);

    const fetchData = async () => {
        try {
            setLoading(true);
            const [statsData, usersData] = await Promise.all([
                adminAPI.getStats(),
                adminAPI.getUsers(),
            ]);
            setStats(statsData);
            setUsers(usersData);
            setError(null);
        } catch (err) {
            console.error('Failed to fetch admin data:', err);
            setError('데이터를 불러오는데 실패했습니다.');
        } finally {
            setLoading(false);
        }
    };

    const fetchManagedPosts = async () => {
        try {
            setContentLoading(true);
            const data = await postsAPI.getPosts(1, 100, null, null, null);
            setManagedPosts(data.posts || []);
            setContentError(null);
        } catch (err) {
            console.error('Failed to fetch posts for admin:', err);
            setContentError('게시글 목록을 불러오는데 실패했습니다.');
        } finally {
            setContentLoading(false);
        }
    };

    const fetchAiReviews = async (statusFilter = reviewStatusFilter) => {
        try {
            setReviewsLoading(true);
            const params = { page: 1, per_page: 50 };
            if (statusFilter) {
                params.status = statusFilter;
            }
            const data = await adminAPI.getAiReviews(params);
            setReviewItems(data.reviews || []);
            setReviewsError(null);
        } catch (err) {
            console.error('Failed to fetch AI reviews for admin:', err);
            setReviewsError('AI 심사 목록을 불러오는데 실패했습니다.');
        } finally {
            setReviewsLoading(false);
        }
    };

    const handleRoleToggle = async (userId, currentStatus) => {
        if (!window.confirm(`이 사용자의 관리자 권한을 ${currentStatus ? '해제' : '부여'}하시겠습니까?`)) return;
        try {
            await adminAPI.updateUserRole(userId, !currentStatus);
            fetchData();
        } catch (err) {
            alert('권한 변경 실패: ' + (err.response?.data?.detail || err.message));
        }
    };

    const handleDeleteUser = async (userId) => {
        if (!window.confirm('정말로 이 사용자를 삭제하시겠습니까? 이 작업은 되돌릴 수 없으며, 모든 게시물과 댓글이 삭제됩니다.')) return;
        try {
            await adminAPI.deleteUser(userId);
            fetchData();
            fetchManagedPosts();
        } catch (err) {
            alert('사용자 삭제 실패: ' + (err.response?.data?.detail || err.message));
        }
    };

    const handleDeletePost = async (postId) => {
        if (!window.confirm('해당 게시글을 삭제하시겠습니까? 이 작업은 되돌릴 수 없습니다.')) return;
        try {
            await adminAPI.deletePost(postId);
            fetchData();
            fetchManagedPosts();
        } catch (err) {
            alert('게시글 삭제 실패: ' + (err.response?.data?.detail || err.message));
        }
    };

    if (loading && !stats) return <div className="container" style={{ padding: '2rem' }}>Loading...</div>;

    if (error) return (
        <div className="container" style={{ padding: '2rem', color: 'red' }}>
            <h3>Error: {error}</h3>
            <button onClick={fetchData} className="btn btn-primary">Retry</button>
        </div>
    );

    return (
        <main className="admin-page">
            <div className="container">
                <div className="admin-header">
                    <h1>🔐 관리자 대시보드</h1>
                    <div className="admin-tabs">
                        <button
                            className={`admin-tab ${activeTab === 'overview' ? 'active' : ''}`}
                            onClick={() => setActiveTab('overview')}
                        >
                            개요
                        </button>
                        <button
                            className={`admin-tab ${activeTab === 'users' ? 'active' : ''}`}
                            onClick={() => setActiveTab('users')}
                        >
                            사용자 관리
                        </button>
                        <button
                            className={`admin-tab ${activeTab === 'content' ? 'active' : ''}`}
                            onClick={() => setActiveTab('content')}
                        >
                            콘텐츠 관리
                        </button>
                        <button
                            className={`admin-tab ${activeTab === 'reviews' ? 'active' : ''}`}
                            onClick={() => setActiveTab('reviews')}
                        >
                            AI 심사
                        </button>
                    </div>
                </div>

                {activeTab === 'overview' && stats && (
                    <div className="admin-dashboard-grid">
                        <div className="admin-stat-card">
                            <h3>총 사용자</h3>
                            <div className="stat-value">{stats.total_users}</div>
                        </div>
                        <div className="admin-stat-card">
                            <h3>총 게시물</h3>
                            <div className="stat-value">{stats.total_posts}</div>
                        </div>
                        <div className="admin-stat-card">
                            <h3>총 댓글</h3>
                            <div className="stat-value">{stats.total_comments}</div>
                        </div>
                        <div className="admin-stat-card">
                            <h3>총 조회수</h3>
                            <div className="stat-value">{stats.total_views}</div>
                        </div>
                        <div className="admin-stat-card">
                            <h3>총 좋아요</h3>
                            <div className="stat-value">{stats.total_likes}</div>
                        </div>
                        <div className="admin-stat-card">
                            <h3>Impact Factor ({stats.journal_metrics?.year || '-'})</h3>
                            <div className="stat-value">
                                {typeof stats.journal_metrics?.impact_factor === 'number'
                                    ? stats.journal_metrics.impact_factor.toFixed(3)
                                    : '-'}
                            </div>
                        </div>
                        <div className="admin-stat-card">
                            <h3>AI 심사 총건수</h3>
                            <div className="stat-value">{stats.ai_review_metrics?.total_reviews ?? 0}</div>
                        </div>
                        <div className="admin-stat-card">
                            <h3>AI 심사 대기/완료/실패</h3>
                            <div className="stat-value" style={{ fontSize: '1.2rem' }}>
                                {stats.ai_review_metrics?.pending_reviews ?? 0}
                                {' / '}
                                {stats.ai_review_metrics?.completed_reviews ?? 0}
                                {' / '}
                                {stats.ai_review_metrics?.failed_reviews ?? 0}
                            </div>
                        </div>
                    </div>
                )}

                {activeTab === 'users' && (
                    <div className="admin-table-container">
                        <table className="admin-table">
                            <thead>
                                <tr>
                                    <th>ID</th>
                                    <th>사용자</th>
                                    <th>가입일</th>
                                    <th>활동 (글/댓글)</th>
                                    <th>권한</th>
                                    <th>관리</th>
                                </tr>
                            </thead>
                            <tbody>
                                {users.map(u => (
                                    <tr key={u.id}>
                                        <td>{u.id}</td>
                                        <td>
                                            <div className="admin-user-cell">
                                                <div>
                                                    <div className="admin-user-name">
                                                        <Link to={`/profile/${u.id}`}>{u.display_name || u.username}</Link>
                                                    </div>
                                                    <div className="admin-user-email">{u.email}</div>
                                                </div>
                                            </div>
                                        </td>
                                        <td>{new Date(u.created_at).toLocaleDateString()}</td>
                                        <td>{u.post_count} / {u.comment_count}</td>
                                        <td>
                                            <span className={`role-badge ${u.is_admin ? 'admin' : 'user'}`}>
                                                {u.is_admin ? '관리자' : '일반'}
                                            </span>
                                        </td>
                                        <td>
                                            <div className="admin-actions">
                                                <button
                                                    className="btn btn-sm btn-ghost"
                                                    onClick={() => handleRoleToggle(u.id, u.is_admin)}
                                                    disabled={u.id === user.id}
                                                    title={u.is_admin ? '일반 사용자로 변경' : '관리자로 승격'}
                                                >
                                                    {u.is_admin ? '⬇️' : '⬆️'}
                                                </button>
                                                <button
                                                    className="btn btn-sm btn-ghost text-red"
                                                    onClick={() => handleDeleteUser(u.id)}
                                                    disabled={u.id === user.id}
                                                    title="사용자 삭제"
                                                >
                                                    ❌
                                                </button>
                                            </div>
                                        </td>
                                    </tr>
                                ))}
                            </tbody>
                        </table>
                    </div>
                )}

                {activeTab === 'content' && (
                    <div className="admin-table-container">
                        {contentError ? (
                            <div className="empty-state">
                                <div className="empty-state-icon">⚠️</div>
                                <h3>콘텐츠 로드 실패</h3>
                                <p>{contentError}</p>
                                <button className="btn btn-primary" onClick={fetchManagedPosts}>다시 시도</button>
                            </div>
                        ) : contentLoading ? (
                            <div className="container" style={{ padding: '2rem' }}>Loading content...</div>
                        ) : managedPosts.length === 0 ? (
                            <div className="empty-state">
                                <div className="empty-state-icon">📭</div>
                                <h3>게시글이 없습니다</h3>
                                <p>관리할 게시글이 아직 없습니다.</p>
                            </div>
                        ) : (
                            <table className="admin-table">
                                <thead>
                                    <tr>
                                        <th>ID</th>
                                        <th>제목</th>
                                        <th>작성자</th>
                                        <th>카테고리</th>
                                        <th>논문 상태</th>
                                        <th>작성일</th>
                                        <th>지표</th>
                                        <th>관리</th>
                                    </tr>
                                </thead>
                                <tbody>
                                    {managedPosts.map(post => (
                                        <tr key={post.id}>
                                            <td>{post.id}</td>
                                            <td>
                                                <Link to={`/posts/${post.id}`}>{post.title}</Link>
                                            </td>
                                            <td>
                                                <Link to={`/profile/${post.author_id}`}>
                                                    {post.author?.display_name || post.author?.username || '알 수 없음'}
                                                </Link>
                                            </td>
                                            <td>{post.category}</td>
                                            <td>{post.category === 'paper' ? (post.paper_status || '-') : '-'}</td>
                                            <td>{new Date(post.created_at).toLocaleDateString()}</td>
                                            <td>👁️ {post.view_count} / ❤️ {post.like_count}</td>
                                            <td>
                                                <button
                                                    className="btn btn-sm btn-ghost text-red"
                                                    onClick={() => handleDeletePost(post.id)}
                                                    title="게시글 삭제"
                                                >
                                                    🗑️
                                                </button>
                                            </td>
                                        </tr>
                                    ))}
                                </tbody>
                            </table>
                        )}
                    </div>
                )}

                {activeTab === 'reviews' && (
                    <div className="admin-table-container">
                        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '1rem', gap: '0.75rem', flexWrap: 'wrap' }}>
                            <div>
                                <label htmlFor="review-status-filter" style={{ marginRight: '0.5rem', fontSize: '0.9rem' }}>상태 필터</label>
                                <select
                                    id="review-status-filter"
                                    value={reviewStatusFilter}
                                    onChange={(e) => {
                                        const next = e.target.value;
                                        setReviewStatusFilter(next);
                                        fetchAiReviews(next);
                                    }}
                                    className="form-input"
                                    style={{ minWidth: '180px', height: '2.25rem' }}
                                >
                                    <option value="">전체</option>
                                    <option value="pending">pending</option>
                                    <option value="completed">completed</option>
                                    <option value="failed">failed</option>
                                </select>
                            </div>
                            <button className="btn btn-secondary btn-sm" onClick={() => fetchAiReviews()}>
                                새로고침
                            </button>
                        </div>

                        {reviewsError ? (
                            <div className="empty-state">
                                <div className="empty-state-icon">⚠️</div>
                                <h3>AI 심사 목록 로드 실패</h3>
                                <p>{reviewsError}</p>
                                <button className="btn btn-primary" onClick={() => fetchAiReviews()}>다시 시도</button>
                            </div>
                        ) : reviewsLoading ? (
                            <div className="container" style={{ padding: '2rem' }}>Loading reviews...</div>
                        ) : reviewItems.length === 0 ? (
                            <div className="empty-state">
                                <div className="empty-state-icon">📭</div>
                                <h3>AI 심사 이력이 없습니다</h3>
                                <p>선택한 조건에 맞는 심사 결과가 없습니다.</p>
                            </div>
                        ) : (
                            <table className="admin-table">
                                <thead>
                                    <tr>
                                        <th>Review ID</th>
                                        <th>Post ID</th>
                                        <th>상태</th>
                                        <th>판정</th>
                                        <th>모델</th>
                                        <th>총점</th>
                                        <th>트리거</th>
                                        <th>생성시각</th>
                                        <th>오류</th>
                                    </tr>
                                </thead>
                                <tbody>
                                    {reviewItems.map((item) => (
                                        <tr key={item.id}>
                                            <td>{item.id}</td>
                                            <td><Link to={`/posts/${item.post_id}`}>{item.post_id}</Link></td>
                                            <td>{item.status}</td>
                                            <td>{item.decision || '-'}</td>
                                            <td>{item.model || '-'}</td>
                                            <td>{item.scores?.overall_score ?? '-'}</td>
                                            <td>{item.trigger}</td>
                                            <td>{new Date(item.created_at).toLocaleString()}</td>
                                            <td style={{ maxWidth: '240px' }}>{item.error_message || '-'}</td>
                                        </tr>
                                    ))}
                                </tbody>
                            </table>
                        )}
                    </div>
                )}
            </div>
        </main>
    );
}

export default Admin;
