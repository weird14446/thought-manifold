import { useState, useEffect } from 'react';
import { useParams, Link, useNavigate } from 'react-router-dom';
import { usersAPI } from '../api';
import { useAuth } from '../context/AuthContext';
import { MarkdownRenderer } from '../components';
import { getPostExcerptMarkdown } from '../utils/markdown';

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

function Profile() {
    const { id } = useParams();
    const { user: currentUser } = useAuth();
    const navigate = useNavigate();

    const [profileUser, setProfileUser] = useState(null);
    const [posts, setPosts] = useState([]);
    const [userMetrics, setUserMetrics] = useState(null);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState(null);

    // Edit state
    const [editing, setEditing] = useState(false);
    const [editDisplayName, setEditDisplayName] = useState('');
    const [editBio, setEditBio] = useState('');
    const [saving, setSaving] = useState(false);
    const [saveError, setSaveError] = useState(null);

    const isOwnProfile = !id || (currentUser && profileUser && currentUser.id === profileUser.id);
    const targetUserId = id || currentUser?.id;

    useEffect(() => {
        if (!targetUserId) {
            if (!id) {
                navigate('/login');
            }
            return;
        }

        const fetchData = async () => {
            try {
                setLoading(true);
                const [userData, userPosts, metricsData] = await Promise.all([
                    usersAPI.getUser(targetUserId),
                    usersAPI.getUserPosts(targetUserId),
                    usersAPI.getUserMetrics(targetUserId).catch(() => null),
                ]);
                setProfileUser(userData);
                setPosts(userPosts);
                setUserMetrics(metricsData);
                setEditDisplayName(userData.display_name || userData.username || '');
                setEditBio(userData.bio || '');
            } catch (err) {
                console.error('Failed to fetch profile:', err);
                if (err.response?.status === 404) {
                    setError('사용자를 찾을 수 없습니다.');
                } else {
                    setError('프로필을 불러오는데 실패했습니다.');
                }
            } finally {
                setLoading(false);
            }
        };
        fetchData();
    }, [targetUserId, id, navigate]);

    const handleSaveProfile = async (e) => {
        e.preventDefault();
        if (saving) return;
        setSaving(true);
        setSaveError(null);
        try {
            const updated = await usersAPI.updateProfile({
                display_name: editDisplayName.trim() || undefined,
                bio: editBio.trim() || '',
            });
            setProfileUser(updated);
            setEditing(false);
        } catch (err) {
            console.error('Failed to update profile:', err);
            setSaveError('프로필 수정에 실패했습니다.');
        } finally {
            setSaving(false);
        }
    };

    const totalLikes = posts.reduce((sum, p) => sum + (p.like_count || 0), 0);
    const totalViews = posts.reduce((sum, p) => sum + (p.view_count || 0), 0);

    const initial = profileUser?.display_name?.[0] || profileUser?.username?.[0] || '?';
    const displayName = profileUser?.display_name || profileUser?.username || '익명';
    const joinDate = profileUser ? new Date(profileUser.created_at).toLocaleDateString('ko-KR', {
        year: 'numeric',
        month: 'long',
        day: 'numeric',
    }) : '';

    if (loading) {
        return (
            <main className="profile-page">
                <div className="container">
                    <div className="profile-skeleton">
                        <div className="skeleton-line" style={{ width: '120px', height: '120px', borderRadius: '50%', margin: '0 auto var(--space-lg)' }} />
                        <div className="skeleton-line skeleton-title" />
                        <div className="skeleton-line skeleton-meta" />
                    </div>
                </div>
            </main>
        );
    }

    if (error) {
        return (
            <main className="profile-page">
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

    if (!profileUser) return null;

    return (
        <main className="profile-page">
            <div className="container">
                <div className="profile-wrapper">
                    {/* Profile Card */}
                    <div className="profile-card">
                        <div className="profile-card-header">
                            <div className="profile-avatar-large">
                                {profileUser.avatar_url ? (
                                    <img src={profileUser.avatar_url} alt={displayName} />
                                ) : (
                                    initial.toUpperCase()
                                )}
                            </div>

                            {!editing ? (
                                <div className="profile-info">
                                    <h1 className="profile-display-name">{displayName}</h1>
                                    <span className="profile-username">@{profileUser.username}</span>
                                    {profileUser.bio && (
                                        <p className="profile-bio">{profileUser.bio}</p>
                                    )}
                                    <div className="profile-meta">
                                        <span className="profile-meta-item">📧 {profileUser.email}</span>
                                        <span className="profile-meta-item">📅 {joinDate} 가입</span>
                                    </div>
                                    {isOwnProfile && (
                                        <button
                                            className="btn btn-ghost profile-edit-btn"
                                            onClick={() => setEditing(true)}
                                        >
                                            ✏️ 프로필 수정
                                        </button>
                                    )}
                                </div>
                            ) : (
                                <form className="profile-edit-form" onSubmit={handleSaveProfile}>
                                    {saveError && (
                                        <div className="form-error">
                                            <span className="form-error-icon">⚠️</span>
                                            {saveError}
                                        </div>
                                    )}
                                    <div className="form-group">
                                        <label className="form-label">표시 이름</label>
                                        <input
                                            type="text"
                                            className="form-input"
                                            value={editDisplayName}
                                            onChange={(e) => setEditDisplayName(e.target.value)}
                                            placeholder="표시 이름"
                                        />
                                    </div>
                                    <div className="form-group">
                                        <label className="form-label">소개</label>
                                        <textarea
                                            className="form-input"
                                            value={editBio}
                                            onChange={(e) => setEditBio(e.target.value)}
                                            placeholder="자기소개를 작성하세요..."
                                            rows={3}
                                        />
                                    </div>
                                    <div className="profile-edit-actions">
                                        <button
                                            type="submit"
                                            className="btn btn-primary"
                                            disabled={saving}
                                        >
                                            {saving ? '저장 중...' : '저장'}
                                        </button>
                                        <button
                                            type="button"
                                            className="btn btn-ghost"
                                            onClick={() => {
                                                setEditing(false);
                                                setEditDisplayName(profileUser.display_name || profileUser.username || '');
                                                setEditBio(profileUser.bio || '');
                                                setSaveError(null);
                                            }}
                                        >
                                            취소
                                        </button>
                                    </div>
                                </form>
                            )}
                        </div>

                        {/* Stats */}
                        <div className="profile-stats">
                            <div className="profile-stat">
                                <span className="profile-stat-value">{posts.length}</span>
                                <span className="profile-stat-label">작성글</span>
                            </div>
                            <div className="profile-stat">
                                <span className="profile-stat-value">{totalLikes}</span>
                                <span className="profile-stat-label">받은 좋아요</span>
                            </div>
                            <div className="profile-stat">
                                <span className="profile-stat-value">{totalViews}</span>
                                <span className="profile-stat-label">총 조회수</span>
                            </div>
                            <div className="profile-stat">
                                <span className="profile-stat-value">{userMetrics?.g_index ?? 0}</span>
                                <span className="profile-stat-label">g-index</span>
                            </div>
                        </div>
                    </div>

                    {/* User Posts */}
                    <section className="profile-posts-section">
                        <h2 className="profile-section-title">
                            📝 {isOwnProfile ? '내 글' : `${displayName}의 글`}
                            {posts.length > 0 && <span className="comments-count">{posts.length}</span>}
                        </h2>

                        {posts.length === 0 ? (
                            <div className="profile-empty">
                                {isOwnProfile ? (
                                    <>
                                        <p>아직 작성한 글이 없습니다.</p>
                                        <Link to="/upload" className="btn btn-primary">✍️ 첫 글 작성하기</Link>
                                    </>
                                ) : (
                                    <p>아직 작성한 글이 없습니다.</p>
                                )}
                            </div>
                        ) : (
                            <div className="profile-posts-grid">
                                {posts.map(post => (
                                    <Link to={`/posts/${post.id}`} key={post.id} className="profile-post-card">
                                        <div className="profile-post-category">
                                            {categoryEmojis[post.category] || '📁'} {categoryLabels[post.category] || post.category}
                                        </div>
                                        <h3 className="profile-post-title">{post.title}</h3>
                                        <div className="profile-post-summary">
                                            <MarkdownRenderer
                                                content={getPostExcerptMarkdown(post)}
                                                className="markdown-profile-excerpt"
                                            />
                                        </div>
                                        <div className="profile-post-meta">
                                            <span>❤️ {post.like_count}</span>
                                            <span>👁️ {post.view_count}</span>
                                            <span>{new Date(post.created_at).toLocaleDateString('ko-KR', { month: 'short', day: 'numeric' })}</span>
                                        </div>
                                    </Link>
                                ))}
                            </div>
                        )}
                    </section>
                </div>
            </div>
        </main>
    );
}

export default Profile;
