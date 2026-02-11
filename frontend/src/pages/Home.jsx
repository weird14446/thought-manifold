import { useState, useEffect, useCallback } from 'react';
import { Link } from 'react-router-dom';
import { PostCard } from '../components';
import { postsAPI } from '../api';

const categories = [
    { key: 'all', label: '전체' },
    { key: 'essay', label: '에세이' },
    { key: 'paper', label: '논문' },
    { key: 'report', label: '리포트' },
    { key: 'note', label: '노트' },
];

function Home() {
    const [posts, setPosts] = useState([]);
    const [totalPosts, setTotalPosts] = useState(0);
    const [selectedCategory, setSelectedCategory] = useState('all');
    const [searchQuery, setSearchQuery] = useState('');
    const [debouncedSearch, setDebouncedSearch] = useState('');
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState(null);

    // Debounce search input
    useEffect(() => {
        const timer = setTimeout(() => {
            setDebouncedSearch(searchQuery);
        }, 400);
        return () => clearTimeout(timer);
    }, [searchQuery]);

    // Fetch posts from API
    const fetchPosts = useCallback(async () => {
        setLoading(true);
        setError(null);

        try {
            const category = selectedCategory === 'all' ? null : selectedCategory;
            const search = debouncedSearch || null;
            const data = await postsAPI.getPosts(1, 12, category, search);
            setPosts(data.posts || []);
            setTotalPosts(data.total || 0);
        } catch (err) {
            console.error('Failed to fetch posts:', err);
            setError('글을 불러오는데 실패했습니다. 서버 연결을 확인해주세요.');
            setPosts([]);
        } finally {
            setLoading(false);
        }
    }, [selectedCategory, debouncedSearch]);

    useEffect(() => {
        fetchPosts();
    }, [fetchPosts]);

    // Loading skeleton
    const renderSkeleton = () => (
        <div className="posts-grid">
            {[...Array(6)].map((_, i) => (
                <div key={i} className="post-card" style={{ pointerEvents: 'none' }}>
                    <div className="post-card-header">
                        <div className="skeleton" style={{ width: 44, height: 44, borderRadius: '50%' }} />
                        <div style={{ flex: 1, display: 'flex', flexDirection: 'column', gap: 6 }}>
                            <div className="skeleton" style={{ width: '60%', height: 14 }} />
                            <div className="skeleton" style={{ width: '40%', height: 12 }} />
                        </div>
                    </div>
                    <div className="post-card-body">
                        <div className="skeleton" style={{ width: '90%', height: 18, marginBottom: 8 }} />
                        <div className="skeleton" style={{ width: '100%', height: 14, marginBottom: 4 }} />
                        <div className="skeleton" style={{ width: '80%', height: 14 }} />
                    </div>
                </div>
            ))}
        </div>
    );

    return (
        <main>
            {/* Hero Section */}
            <section className="hero">
                <div className="container hero-content">
                    <span className="hero-badge">
                        <span className="hero-badge-icon">✨</span>
                        지식을 나누고 함께 성장하는 공간
                    </span>
                    <h1>생각이 모이는 곳, Thought Manifold</h1>
                    <p className="hero-description">
                        학습한 내용을 에세이, 논문, 리포트로 정리하고 공유하세요.
                        다양한 관점에서 영감을 얻고, 함께 배움을 확장해 나갈 수 있습니다.
                    </p>
                    <div className="hero-actions">
                        <Link to="/upload" className="btn btn-primary">
                            ✍️ 글 작성하기
                        </Link>
                        <Link to="/explore" className="btn btn-secondary">
                            🔍 탐색하기
                        </Link>
                    </div>

                    <div className="hero-stats">
                        <div className="stat">
                            <div className="stat-value">{totalPosts.toLocaleString()}</div>
                            <div className="stat-label">공유된 글</div>
                        </div>
                    </div>
                </div>
            </section>

            {/* Search */}
            <div className="container">
                <div className="search-bar">
                    <span className="search-icon">🔍</span>
                    <input
                        type="text"
                        className="search-input"
                        placeholder="관심 있는 주제를 검색해보세요..."
                        value={searchQuery}
                        onChange={(e) => setSearchQuery(e.target.value)}
                    />
                </div>
            </div>

            {/* Posts Section */}
            <section className="posts-section">
                <div className="container">
                    <div className="section-header">
                        <h2 className="section-title">최신 글</h2>
                        <div className="category-tabs">
                            {categories.map(cat => (
                                <button
                                    key={cat.key}
                                    className={`category-tab ${selectedCategory === cat.key ? 'active' : ''}`}
                                    onClick={() => setSelectedCategory(cat.key)}
                                >
                                    {cat.label}
                                </button>
                            ))}
                        </div>
                    </div>

                    {error ? (
                        <div className="empty-state">
                            <div className="empty-state-icon">⚠️</div>
                            <h3>연결 오류</h3>
                            <p>{error}</p>
                            <button className="btn btn-primary" onClick={fetchPosts}>
                                🔄 다시 시도
                            </button>
                        </div>
                    ) : loading ? (
                        renderSkeleton()
                    ) : posts.length > 0 ? (
                        <div className="posts-grid">
                            {posts.map(post => (
                                <PostCard key={post.id} post={post} />
                            ))}
                        </div>
                    ) : (
                        <div className="empty-state">
                            <div className="empty-state-icon">📭</div>
                            <h3>글이 없습니다</h3>
                            <p>아직 이 카테고리에 작성된 글이 없습니다. 첫 번째 글을 작성해보세요!</p>
                            <Link to="/upload" className="btn btn-primary">
                                ✍️ 첫 글 작성하기
                            </Link>
                        </div>
                    )}
                </div>
            </section>
        </main>
    );
}

export default Home;

