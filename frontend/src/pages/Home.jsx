import { useState, useEffect, useCallback } from 'react';
import { Link, useSearchParams } from 'react-router-dom';
import { MarkdownRenderer } from '../components';
import { postsAPI } from '../api';
import { getPostExcerptMarkdown } from '../utils/markdown';

const categories = [
    { key: 'all', label: '전체' },
    { key: 'essay', label: '에세이' },
    { key: 'paper', label: '논문' },
    { key: 'report', label: '리포트' },
    { key: 'note', label: '노트' },
];

const paperStatuses = [
    { key: '', label: '전체 상태' },
    { key: 'draft', label: 'Draft' },
    { key: 'submitted', label: 'Submitted' },
    { key: 'revision', label: 'Revision' },
    { key: 'accepted', label: 'Accepted' },
    { key: 'published', label: 'Published' },
    { key: 'rejected', label: 'Rejected' },
];

const aiDecisions = [
    { key: '', label: '전체 AI 판정' },
    { key: 'accept', label: 'Accept' },
    { key: 'minor_revision', label: 'Minor Revision' },
    { key: 'major_revision', label: 'Major Revision' },
    { key: 'reject', label: 'Reject' },
];

function Home() {
    const [searchParams, setSearchParams] = useSearchParams();
    const [posts, setPosts] = useState([]);
    const [totalPosts, setTotalPosts] = useState(0);
    const [selectedCategory, setSelectedCategory] = useState('all');
    const [searchQuery, setSearchQuery] = useState('');
    const [debouncedSearch, setDebouncedSearch] = useState('');
    const [advancedTagFilter, setAdvancedTagFilter] = useState('');
    const [authorFilter, setAuthorFilter] = useState('');
    const [yearFilter, setYearFilter] = useState('');
    const [statusFilter, setStatusFilter] = useState('');
    const [aiDecisionFilter, setAiDecisionFilter] = useState('');
    const [minCitationFilter, setMinCitationFilter] = useState('');
    const [minGIndexFilter, setMinGIndexFilter] = useState('');
    const [showAdvancedFilters, setShowAdvancedFilters] = useState(false);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState(null);

    const tagFilter = searchParams.get('tag');
    const effectiveTagFilter = advancedTagFilter.trim() || tagFilter || null;

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
            const parsedYear = yearFilter ? Number(yearFilter) : null;
            const parsedMinCitation = minCitationFilter ? Number(minCitationFilter) : null;
            const parsedMinGIndex = minGIndexFilter ? Number(minGIndexFilter) : null;
            const data = await postsAPI.getPosts({
                page: 1,
                per_page: 12,
                category,
                search,
                tag: effectiveTagFilter,
                author: authorFilter.trim() || null,
                year: Number.isFinite(parsedYear) ? parsedYear : null,
                paper_status: statusFilter || null,
                ai_decision: aiDecisionFilter || null,
                min_citation_count: Number.isFinite(parsedMinCitation) ? parsedMinCitation : null,
                min_author_g_index: Number.isFinite(parsedMinGIndex) ? parsedMinGIndex : null,
            });
            setPosts(data.posts || []);
            setTotalPosts(data.total || 0);
        } catch (err) {
            console.error('Failed to fetch posts:', err);
            setError('글을 불러오는데 실패했습니다. 서버 연결을 확인해주세요.');
            setPosts([]);
        } finally {
            setLoading(false);
        }
    }, [
        selectedCategory,
        debouncedSearch,
        effectiveTagFilter,
        authorFilter,
        yearFilter,
        statusFilter,
        aiDecisionFilter,
        minCitationFilter,
        minGIndexFilter,
    ]);

    useEffect(() => {
        fetchPosts();
    }, [fetchPosts]);

    const clearTagFilter = () => {
        setAdvancedTagFilter('');
        setSearchParams({});
    };

    const clearAdvancedFilters = () => {
        setAdvancedTagFilter('');
        setAuthorFilter('');
        setYearFilter('');
        setStatusFilter('');
        setAiDecisionFilter('');
        setMinCitationFilter('');
        setMinGIndexFilter('');
    };

    const featuredPost = posts[0] || null;
    const highlightedPosts = posts.slice(1, 5);
    const archivePosts = posts.slice(5);
    const issueDateLabel = new Date().toLocaleDateString('ko-KR', {
        year: 'numeric',
        month: 'long',
        day: 'numeric',
    });

    // Loading skeleton
    const renderSkeleton = () => (
        <>
            <div className="journal-board-grid">
                <div className="journal-board-main">
                    <div className="skeleton" style={{ width: '30%', height: 14, marginBottom: 12 }} />
                    <div className="skeleton" style={{ width: '70%', height: 28, marginBottom: 12 }} />
                    <div className="skeleton" style={{ width: '100%', height: 14, marginBottom: 6 }} />
                    <div className="skeleton" style={{ width: '92%', height: 14, marginBottom: 6 }} />
                    <div className="skeleton" style={{ width: '74%', height: 14 }} />
                </div>
                <aside className="journal-board-side">
                    {[...Array(3)].map((_, idx) => (
                        <div key={idx} className="journal-brief-item" style={{ pointerEvents: 'none' }}>
                            <div className="skeleton" style={{ width: '35%', height: 12, marginBottom: 8 }} />
                            <div className="skeleton" style={{ width: '100%', height: 14, marginBottom: 6 }} />
                            <div className="skeleton" style={{ width: '80%', height: 14 }} />
                        </div>
                    ))}
                </aside>
            </div>
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
        </>
    );

    return (
        <main className="home-page">
            <section className="journal-hero">
                <div className="container">
                    <div className="journal-hero-head">
                        <p className="journal-kicker">COMMUNITY RESEARCH PERIODICAL</p>
                        <h1>Thought Manifold Journal</h1>
                        <p className="journal-subtitle">
                            논문, 에세이, 리포트를 저널 구조로 읽고 토론하는 커뮤니티 아카이브.
                            최신 연구 노트와 심사 흐름을 하나의 발행면에서 확인하세요.
                        </p>
                    </div>
                    <div className="journal-issue-line">
                        <span>{issueDateLabel}</span>
                        <span>Current Issue · Open Review</span>
                        <span>{totalPosts.toLocaleString()} Articles Indexed</span>
                    </div>
                    <div className="hero-actions journal-hero-actions">
                        <Link to="/upload" className="btn btn-primary">
                            원고 제출
                        </Link>
                        <Link to="/guidelines" className="btn btn-secondary">
                            투고 가이드
                        </Link>
                        <Link to="/explore" className="btn btn-secondary">
                            아카이브 탐색
                        </Link>
                    </div>
                </div>
            </section>

            <section className="journal-board">
                <div className="container">
                    {loading ? (
                        <div className="journal-board-panel">{renderSkeleton()}</div>
                    ) : featuredPost ? (
                        <div className="journal-board-panel">
                            <div className="journal-board-grid">
                                <article className="journal-board-main">
                                    <p className="journal-section-label">Lead Article</p>
                                    <h2>
                                        <Link to={`/posts/${featuredPost.id}`}>{featuredPost.title}</Link>
                                    </h2>
                                    <p className="journal-board-summary">
                                        {featuredPost.summary || '요약이 없는 게시글입니다. 상세 페이지에서 본문을 확인하세요.'}
                                    </p>
                                    <div className="journal-board-main-meta">
                                        <span>{featuredPost.author?.display_name || featuredPost.author?.username || '익명'}</span>
                                        <span>{new Date(featuredPost.created_at).toLocaleDateString('ko-KR')}</span>
                                        <span>조회 {featuredPost.view_count}</span>
                                        <span>좋아요 {featuredPost.like_count}</span>
                                    </div>
                                </article>
                                <aside className="journal-board-side">
                                    <h3>Editor&apos;s Brief</h3>
                                    {highlightedPosts.length === 0 ? (
                                        <p className="journal-brief-empty">추가 발행 글이 없습니다.</p>
                                    ) : (
                                        highlightedPosts.map((post) => (
                                            <article key={post.id} className="journal-brief-item">
                                                <p className="journal-brief-category">{post.category.toUpperCase()}</p>
                                                <h4>
                                                    <Link to={`/posts/${post.id}`}>{post.title}</Link>
                                                </h4>
                                                <p>{new Date(post.created_at).toLocaleDateString('ko-KR')}</p>
                                            </article>
                                        ))
                                    )}
                                </aside>
                            </div>
                        </div>
                    ) : null}
                </div>
            </section>

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
                <div className="advanced-search-toolbar">
                    <button
                        type="button"
                        className="btn btn-secondary btn-sm"
                        onClick={() => setShowAdvancedFilters(prev => !prev)}
                    >
                        {showAdvancedFilters ? '고급 필터 닫기' : '고급 필터 열기'}
                    </button>
                    <button
                        type="button"
                        className="btn btn-secondary btn-sm"
                        onClick={clearAdvancedFilters}
                    >
                        필터 초기화
                    </button>
                </div>
                {showAdvancedFilters && (
                    <div className="advanced-search-panel">
                        <div className="advanced-search-grid">
                            <label className="advanced-search-field">
                                <span>태그</span>
                                <input
                                    type="text"
                                    value={advancedTagFilter}
                                    onChange={(e) => setAdvancedTagFilter(e.target.value)}
                                    placeholder="예: react"
                                />
                            </label>

                            <label className="advanced-search-field">
                                <span>저자</span>
                                <input
                                    type="text"
                                    value={authorFilter}
                                    onChange={(e) => setAuthorFilter(e.target.value)}
                                    placeholder="이름 또는 아이디"
                                />
                            </label>

                            <label className="advanced-search-field">
                                <span>연도</span>
                                <input
                                    type="number"
                                    min="1900"
                                    max="2100"
                                    value={yearFilter}
                                    onChange={(e) => setYearFilter(e.target.value)}
                                    placeholder="예: 2026"
                                />
                            </label>

                            <label className="advanced-search-field">
                                <span>논문 상태</span>
                                <select
                                    value={statusFilter}
                                    onChange={(e) => setStatusFilter(e.target.value)}
                                >
                                    {paperStatuses.map((option) => (
                                        <option key={option.key || 'all'} value={option.key}>
                                            {option.label}
                                        </option>
                                    ))}
                                </select>
                            </label>

                            <label className="advanced-search-field">
                                <span>AI 판정</span>
                                <select
                                    value={aiDecisionFilter}
                                    onChange={(e) => setAiDecisionFilter(e.target.value)}
                                >
                                    {aiDecisions.map((option) => (
                                        <option key={option.key || 'all'} value={option.key}>
                                            {option.label}
                                        </option>
                                    ))}
                                </select>
                            </label>

                            <label className="advanced-search-field">
                                <span>최소 피인용수</span>
                                <input
                                    type="number"
                                    min="0"
                                    value={minCitationFilter}
                                    onChange={(e) => setMinCitationFilter(e.target.value)}
                                    placeholder="0"
                                />
                            </label>

                            <label className="advanced-search-field">
                                <span>최소 저자 g-index</span>
                                <input
                                    type="number"
                                    min="0"
                                    value={minGIndexFilter}
                                    onChange={(e) => setMinGIndexFilter(e.target.value)}
                                    placeholder="0"
                                />
                            </label>
                        </div>
                    </div>
                )}
            </div>

            <section className="posts-section">
                <div className="container">
                    <div className="section-header">
                        <div className="header-left">
                            <h2 className="section-title">
                                {effectiveTagFilter ? `#${effectiveTagFilter} 태그 검색 결과` : 'Archive Articles'}
                            </h2>
                            {(effectiveTagFilter || tagFilter) && (
                                <button onClick={clearTagFilter} className="clear-filter-btn">
                                    필터 해제 ✕
                                </button>
                            )}
                        </div>
                        <div className="category-tabs">
                            {categories.map(cat => (
                                <button
                                    key={cat.key}
                                    className={`category-tab ${selectedCategory === cat.key ? 'active' : ''}`}
                                    onClick={() => {
                                        setSelectedCategory(cat.key);
                                        if (tagFilter) clearTagFilter();
                                    }}
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
                        <div className="journal-board-panel">{renderSkeleton()}</div>
                    ) : archivePosts.length > 0 ? (
                        <div className="journal-archive-list">
                            {archivePosts.map((post) => {
                                const authorName = post.author?.display_name || post.author?.username || '익명';
                                const publishedDate = new Date(post.created_at).toLocaleDateString('ko-KR');
                                const excerptMarkdown = getPostExcerptMarkdown(post);

                                return (
                                    <article key={post.id} className="journal-article-row">
                                        <div className="journal-article-meta">
                                            <span className="journal-article-category">{post.category.toUpperCase()}</span>
                                            <span>{publishedDate}</span>
                                            <span>{authorName}</span>
                                            {post.category === 'paper' && post.paper_status && (
                                                <span className="journal-article-status">{post.paper_status}</span>
                                            )}
                                        </div>

                                        <h3 className="journal-article-title">
                                            <Link to={`/posts/${post.id}`}>{post.title}</Link>
                                        </h3>

                                        <div className="journal-article-excerpt">
                                            <MarkdownRenderer
                                                content={excerptMarkdown}
                                                className="markdown-page-excerpt"
                                                enableInteractiveEmbeds={false}
                                            />
                                        </div>

                                        <div className="journal-article-footer">
                                            <div className="journal-article-stats">
                                                <span>👁️ {post.view_count}</span>
                                                <span>❤️ {post.like_count}</span>
                                            </div>
                                            {post.tags?.length > 0 && (
                                                <div className="journal-article-tags">
                                                    {post.tags.map((tag) => (
                                                        <Link key={tag} to={`/?tag=${tag}`} className="post-tag">
                                                            #{tag}
                                                        </Link>
                                                    ))}
                                                </div>
                                            )}
                                            <Link to={`/posts/${post.id}`} className="journal-article-read">
                                                Read Full Article
                                            </Link>
                                        </div>
                                    </article>
                                );
                            })}
                        </div>
                    ) : posts.length > 0 ? (
                        <div className="empty-state journal-empty-state">
                            <h3>현재 이슈의 리드 글만 등록되어 있습니다.</h3>
                            <p>추가 원고가 발행되면 아카이브 섹션에 함께 노출됩니다.</p>
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
