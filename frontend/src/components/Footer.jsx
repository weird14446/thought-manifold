import { Link } from 'react-router-dom';

function Footer() {
    return (
        <footer className="footer">
            <div className="container">
                <div className="footer-content">
                    <div className="footer-brand">
                        <Link to="/" className="logo">
                            <span className="logo-icon">💭</span>
                            Thought Manifold
                        </Link>
                        <p>
                            학습한 지식을 나누고, 함께 성장하는 커뮤니티.
                            에세이, 논문, 리포트를 공유하고 영감을 얻으세요.
                        </p>
                    </div>

                    <div className="footer-section">
                        <h4>탐색</h4>
                        <ul className="footer-links">
                            <li><Link to="/explore?category=essay">에세이</Link></li>
                            <li><Link to="/explore?category=paper">논문</Link></li>
                            <li><Link to="/explore?category=report">리포트</Link></li>
                            <li><Link to="/explore?category=note">노트</Link></li>
                        </ul>
                    </div>

                    <div className="footer-section">
                        <h4>커뮤니티</h4>
                        <ul className="footer-links">
                            <li><Link to="/about">소개</Link></li>
                            <li><Link to="/guidelines">가이드라인</Link></li>
                            <li><Link to="/faq">자주 묻는 질문</Link></li>
                            <li><Link to="/contact">문의하기</Link></li>
                        </ul>
                    </div>

                    <div className="footer-section">
                        <h4>법적 정보</h4>
                        <ul className="footer-links">
                            <li><Link to="/terms">이용약관</Link></li>
                            <li><Link to="/privacy">개인정보처리방침</Link></li>
                            <li><Link to="/copyright">저작권 정책</Link></li>
                        </ul>
                    </div>
                </div>

                <div className="footer-bottom">
                    <p>&copy; {new Date().getFullYear()} Thought Manifold. All rights reserved.</p>
                    <div className="footer-social">
                        <a href="https://github.com" target="_blank" rel="noopener noreferrer" aria-label="GitHub">
                            🐙
                        </a>
                        <a href="https://twitter.com" target="_blank" rel="noopener noreferrer" aria-label="Twitter">
                            🐦
                        </a>
                        <a href="https://discord.com" target="_blank" rel="noopener noreferrer" aria-label="Discord">
                            💬
                        </a>
                    </div>
                </div>
            </div>
        </footer>
    );
}

export default Footer;
