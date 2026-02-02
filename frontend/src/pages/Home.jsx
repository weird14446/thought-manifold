import { useState, useEffect } from 'react';
import { Link } from 'react-router-dom';
import { PostCard } from '../components';

// Sample data for demonstration
const samplePosts = [
    {
        id: 1,
        title: '딥러닝 기초: 신경망의 이해와 역전파 알고리즘',
        content: '인공신경망(Artificial Neural Network)은 인간 뇌의 신경세포 구조를 모방하여 만든 학습 알고리즘입니다. 이 글에서는 퍼셉트론부터 다층 신경망까지의 발전 과정과 역전파 알고리즘의 원리를 상세히 설명합니다.',
        summary: '신경망의 기본 개념부터 역전파 알고리즘까지 딥러닝 입문자를 위한 완벽 가이드',
        category: 'essay',
        view_count: 1234,
        like_count: 89,
        created_at: '2026-01-28T10:00:00Z',
        author: { id: 1, username: 'airesearcher', display_name: 'AI 연구자' }
    },
    {
        id: 2,
        title: '현대 사회에서의 지식 공유와 오픈 소스 문화',
        content: '오픈 소스 소프트웨어 운동은 단순한 개발 방법론을 넘어 하나의 문화적 현상이 되었습니다. 이 논문에서는 오픈 소스 문화가 현대 지식 공유에 미친 영향을 분석합니다.',
        summary: '오픈 소스 문화가 현대 지식 생태계에 미친 영향에 대한 분석',
        category: 'paper',
        view_count: 892,
        like_count: 67,
        created_at: '2026-01-25T14:30:00Z',
        author: { id: 2, username: 'techphilosopher', display_name: '기술철학자' },
        file_name: 'opensource_culture.pdf'
    },
    {
        id: 3,
        title: '2026년 기술 트렌드 분석 리포트',
        content: '양자 컴퓨팅, AI, 그린 테크놀로지 등 2026년을 이끌어갈 핵심 기술 트렌드를 분석했습니다. 각 기술의 현재 발전 상황과 향후 전망을 담았습니다.',
        summary: '2026년 핵심 기술 동향과 전망에 대한 종합 분석 리포트',
        category: 'report',
        view_count: 2341,
        like_count: 156,
        created_at: '2026-01-20T09:15:00Z',
        author: { id: 3, username: 'techanalyst', display_name: '테크 애널리스트' },
        file_name: 'tech_trends_2026.pdf'
    },
    {
        id: 4,
        title: 'React Hooks 완벽 가이드: useState부터 useReducer까지',
        content: 'React 16.8에서 도입된 Hooks는 함수형 컴포넌트에서도 상태 관리와 생명주기 기능을 사용할 수 있게 해줍니다. 이 노트에서는 모든 기본 Hook과 커스텀 Hook 작성법을 다룹니다.',
        summary: 'React Hooks의 모든 것을 정리한 개발 노트',
        category: 'note',
        view_count: 1876,
        like_count: 134,
        created_at: '2026-01-18T16:45:00Z',
        author: { id: 4, username: 'frontenddev', display_name: '프론트엔드 개발자' }
    },
    {
        id: 5,
        title: '효과적인 학습 방법론: 능동적 회상과 간격 반복',
        content: '인지 과학 연구를 바탕으로 가장 효과적인 학습 방법을 분석합니다. 능동적 회상(Active Recall)과 간격 반복(Spaced Repetition)을 활용한 학습 전략을 제시합니다.',
        summary: '과학적으로 검증된 효과적인 학습 전략 가이드',
        category: 'essay',
        view_count: 3201,
        like_count: 245,
        created_at: '2026-01-15T11:20:00Z',
        author: { id: 5, username: 'learningscientist', display_name: '학습과학 연구자' }
    },
    {
        id: 6,
        title: 'FastAPI와 React로 풀스택 앱 만들기',
        content: 'Python FastAPI 백엔드와 React 프론트엔드를 연동하여 풀스택 웹 애플리케이션을 구축하는 방법을 단계별로 설명합니다. JWT 인증부터 배포까지 모든 과정을 다룹니다.',
        summary: 'FastAPI + React 풀스택 개발 튜토리얼',
        category: 'note',
        view_count: 1567,
        like_count: 112,
        created_at: '2026-01-12T08:30:00Z',
        author: { id: 6, username: 'fullstackdev', display_name: '풀스택 엔지니어' }
    }
];

const categories = [
    { key: 'all', label: '전체' },
    { key: 'essay', label: '에세이' },
    { key: 'paper', label: '논문' },
    { key: 'report', label: '리포트' },
    { key: 'note', label: '노트' },
];

function Home() {
    const [posts, setPosts] = useState(samplePosts);
    const [selectedCategory, setSelectedCategory] = useState('all');
    const [searchQuery, setSearchQuery] = useState('');

    const filteredPosts = posts.filter(post => {
        const matchesCategory = selectedCategory === 'all' || post.category === selectedCategory;
        const matchesSearch = !searchQuery ||
            post.title.toLowerCase().includes(searchQuery.toLowerCase()) ||
            post.content.toLowerCase().includes(searchQuery.toLowerCase());
        return matchesCategory && matchesSearch;
    });

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
                            <div className="stat-value">1,234</div>
                            <div className="stat-label">공유된 글</div>
                        </div>
                        <div className="stat">
                            <div className="stat-value">567</div>
                            <div className="stat-label">활동 멤버</div>
                        </div>
                        <div className="stat">
                            <div className="stat-value">89K</div>
                            <div className="stat-label">조회수</div>
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

                    {filteredPosts.length > 0 ? (
                        <div className="posts-grid">
                            {filteredPosts.map(post => (
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
