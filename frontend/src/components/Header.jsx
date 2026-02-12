import { useState, useEffect } from 'react';
import { Link, useLocation } from 'react-router-dom';
import { useAuth } from '../context/AuthContext';

function Header() {
    const { user, logout } = useAuth();
    const location = useLocation();

    const [theme, setTheme] = useState(() => {
        const saved = localStorage.getItem('theme');
        return saved || (window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light');
    });

    const [menuOpen, setMenuOpen] = useState(false);

    useEffect(() => {
        document.documentElement.setAttribute('data-theme', theme);
        localStorage.setItem('theme', theme);
    }, [theme]);

    const toggleTheme = () => {
        setTheme(prev => prev === 'light' ? 'dark' : 'light');
    };

    const getInitials = (name) => {
        return name?.charAt(0)?.toUpperCase() || '?';
    };

    return (
        <header className="header">
            <div className="container header-inner">
                <Link to="/" className="logo">
                    <span className="logo-icon">💭</span>
                    Thought Manifold
                </Link>

                <nav className="nav">
                    <ul className="nav-links">
                        <li><Link to="/" className={`nav-link ${location.pathname === '/' ? 'active' : ''}`}>홈</Link></li>
                        <li><Link to="/explore" className={`nav-link ${location.pathname === '/explore' ? 'active' : ''}`}>탐색</Link></li>
                        <li><Link to="/about" className={`nav-link ${location.pathname === '/about' ? 'active' : ''}`}>소개</Link></li>
                        <li><Link to="/guidelines" className={`nav-link ${location.pathname === '/guidelines' ? 'active' : ''}`}>가이드라인</Link></li>
                        {user && (
                            <li><Link to="/reviews" className={`nav-link ${location.pathname === '/reviews' ? 'active' : ''}`}>심사센터</Link></li>
                        )}
                    </ul>

                    <div className="nav-actions">
                        <button
                            className="btn btn-ghost theme-toggle"
                            onClick={toggleTheme}
                            aria-label="테마 전환"
                        >
                            {theme === 'light' ? '🌙' : '☀️'}
                        </button>

                        {user ? (
                            <>
                                <Link to="/upload" className="btn btn-primary">
                                    ✍️ 글쓰기
                                </Link>
                                <div className="user-menu-wrapper">
                                    <button
                                        className="user-avatar-btn"
                                        onClick={() => setMenuOpen(prev => !prev)}
                                        aria-label="사용자 메뉴"
                                    >
                                        <span className="user-avatar">{getInitials(user.username)}</span>
                                    </button>
                                    {menuOpen && (
                                        <div className="user-dropdown" onClick={() => setMenuOpen(false)}>
                                            <div className="user-dropdown-header">
                                                <span className="user-dropdown-name">{user.username}</span>
                                                <span className="user-dropdown-email">{user.email}</span>
                                            </div>
                                            <div className="user-dropdown-divider" />
                                            <Link to="/profile" className="user-dropdown-item">
                                                👤 프로필
                                            </Link>
                                            <Link to="/reviews" className="user-dropdown-item">
                                                🧪 심사센터
                                            </Link>
                                            {user.is_admin && (
                                                <Link to="/admin" className="user-dropdown-item">
                                                    🔐 관리자
                                                </Link>
                                            )}
                                            <button className="user-dropdown-item" onClick={logout}>
                                                🚪 로그아웃
                                            </button>
                                        </div>
                                    )}
                                </div>
                            </>
                        ) : (
                            <Link to="/login" className="btn btn-primary">
                                🔐 로그인
                            </Link>
                        )}
                    </div>
                </nav>
            </div>
        </header>
    );
}

export default Header;
