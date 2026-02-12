import { Link } from 'react-router-dom';

function AboutPage() {
    return (
        <main className="about-page">
            {/* Hero */}
            <section className="about-hero">
                <div className="container about-hero-content">
                    <span className="hero-badge">
                        <span className="hero-badge-icon">💭</span>
                        About&nbsp;Thought&nbsp;Manifold
                    </span>
                    <h1>생각이 만나고,<br />지식이 확장되는 공간</h1>
                    <p className="about-hero-lead">
                        Thought Manifold는 학습한 내용을 체계적으로 정리하고,
                        다양한 관점에서 영감을 주고받으며 함께 성장하는
                        지식 공유 커뮤니티입니다.
                    </p>
                </div>
            </section>

            {/* 미션 */}
            <section className="about-section">
                <div className="container">
                    <div className="about-mission-card">
                        <h2>우리의 미션</h2>
                        <p>
                            좋은 지식은 나눌수록 커집니다.
                            Thought Manifold는 개인이 학습하고 탐구한 내용을 에세이, 논문, 리포트, 노트 형태로 작성·공유할 수 있는 플랫폼을 제공합니다.
                            인용과 태그를 통해 지식 간의 연결 고리를 만들고,
                            학술적 엄밀함과 자유로운 탐구를 함께 격려합니다.
                        </p>
                    </div>
                </div>
            </section>

            {/* 핵심 기능 */}
            <section className="about-section about-features-section">
                <div className="container">
                    <h2 className="about-section-title">핵심 기능</h2>
                    <div className="about-features-grid">
                        <div className="about-feature-card">
                            <div className="about-feature-icon">✍️</div>
                            <h3>다양한 카테고리</h3>
                            <p>에세이, 논문, 리포트, 노트 등 목적에 맞는 카테고리를 선택해 글을 작성할 수 있습니다. 파일 첨부도 지원합니다.</p>
                        </div>
                        <div className="about-feature-card">
                            <div className="about-feature-icon">🏷️</div>
                            <h3>태그 시스템</h3>
                            <p>키워드 태그로 글을 분류하세요. 태그를 클릭하면 관련 글을 빠르게 탐색할 수 있습니다.</p>
                        </div>
                        <div className="about-feature-card">
                            <div className="about-feature-icon">📎</div>
                            <h3>인용 관리</h3>
                            <p>논문 카테고리에서 다른 글을 인용하면, 인용 관계가 자동으로 기록됩니다. 자동 인용 탐지도 제공합니다.</p>
                        </div>
                        <div className="about-feature-card">
                            <div className="about-feature-icon">🤖</div>
                            <h3>AI 논문 심사</h3>
                            <p>논문 작성 시 편집자 1차 심사와 동료심사를 AI 심사로 대체하여, 빠른 피드백과 재심사 워크플로우를 제공합니다.</p>
                        </div>
                        <div className="about-feature-card">
                            <div className="about-feature-icon">📊</div>
                            <h3>저자 메트릭</h3>
                            <p>조회수, 좋아요, 인용 횟수를 기반으로 g-index 등 저자 활동 지표를 확인할 수 있습니다.</p>
                        </div>
                        <div className="about-feature-card">
                            <div className="about-feature-icon">💬</div>
                            <h3>댓글 & 토론</h3>
                            <p>모든 글에 댓글을 달아 생각을 공유하고, 건설적인 피드백을 주고받을 수 있습니다.</p>
                        </div>
                        <div className="about-feature-card">
                            <div className="about-feature-icon">🌓</div>
                            <h3>다크 / 라이트 테마</h3>
                            <p>시스템 설정에 맞춰 자동 전환되거나 직접 선택할 수 있는 테마를 제공합니다.</p>
                        </div>
                    </div>
                </div>
            </section>

            {/* 기술 스택 */}
            <section className="about-section">
                <div className="container">
                    <h2 className="about-section-title">기술 스택</h2>
                    <div className="about-tech-grid">
                        <div className="about-tech-card">
                            <span className="about-tech-emoji">⚛️</span>
                            <div>
                                <h4>React + Vite</h4>
                                <p>빠른 HMR과 모듈 번들링으로 최적화된 프론트엔드</p>
                            </div>
                        </div>
                        <div className="about-tech-card">
                            <span className="about-tech-emoji">🦀</span>
                            <div>
                                <h4>Rust Axum</h4>
                                <p>메모리 안전성과 고성능을 보장하는 백엔드 프레임워크</p>
                            </div>
                        </div>
                        <div className="about-tech-card">
                            <span className="about-tech-emoji">🐬</span>
                            <div>
                                <h4>MySQL</h4>
                                <p>정규화된 스키마와 외래 키로 데이터 무결성 확보</p>
                            </div>
                        </div>
                        <div className="about-tech-card">
                            <span className="about-tech-emoji">🔐</span>
                            <div>
                                <h4>JWT + OAuth 2.0</h4>
                                <p>자체 회원가입과 Google 소셜 로그인 동시 지원</p>
                            </div>
                        </div>
                    </div>
                </div>
            </section>

            {/* 커뮤니티 가치 */}
            <section className="about-section about-values-section">
                <div className="container">
                    <h2 className="about-section-title">커뮤니티 가치</h2>
                    <div className="about-values-grid">
                        <div className="about-value-item">
                            <span className="about-value-number">01</span>
                            <h3>지적 호기심</h3>
                            <p>배움에 끝은 없습니다. 새로운 주제를 탐구하고 정리하는 과정을 즐기세요.</p>
                        </div>
                        <div className="about-value-item">
                            <span className="about-value-number">02</span>
                            <h3>열린 공유</h3>
                            <p>완벽하지 않아도 괜찮습니다. 학습 과정 자체가 다른 누군가에게 영감이 됩니다.</p>
                        </div>
                        <div className="about-value-item">
                            <span className="about-value-number">03</span>
                            <h3>건설적 대화</h3>
                            <p>비판과 비난은 다릅니다. 서로의 관점을 존중하며 함께 더 나은 이해를 만들어갑니다.</p>
                        </div>
                        <div className="about-value-item">
                            <span className="about-value-number">04</span>
                            <h3>참고와 인용</h3>
                            <p>다른 사람의 지식 위에 쌓아 올릴 때, 출처를 밝히는 것은 예의이자 학문적 기본입니다.</p>
                        </div>
                    </div>
                </div>
            </section>

            {/* CTA */}
            <section className="about-section about-cta-section">
                <div className="container about-cta-content">
                    <h2>함께 시작해볼까요?</h2>
                    <p>지금 가입하고, 학습한 내용을 정리하세요. 생각을 나누면 지식은 두 배가 됩니다.</p>
                    <div className="about-cta-actions">
                        <Link to="/login" className="btn btn-primary btn-lg">
                            🚀 시작하기
                        </Link>
                        <Link to="/explore" className="btn btn-secondary btn-lg">
                            🔍 글 둘러보기
                        </Link>
                    </div>
                </div>
            </section>
        </main>
    );
}

export default AboutPage;
