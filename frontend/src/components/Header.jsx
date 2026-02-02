import { useState, useEffect } from 'react';
import { Link } from 'react-router-dom';

function Header() {
    const [theme, setTheme] = useState(() => {
        const saved = localStorage.getItem('theme');
        return saved || (window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light');
    });

    useEffect(() => {
        document.documentElement.setAttribute('data-theme', theme);
        localStorage.setItem('theme', theme);
    }, [theme]);

    const toggleTheme = () => {
        setTheme(prev => prev === 'light' ? 'dark' : 'light');
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
                        <li><Link to="/" className="nav-link active">홈</Link></li>
                        <li><Link to="/explore" className="nav-link">탐색</Link></li>
                        <li><Link to="/about" className="nav-link">소개</Link></li>
                    </ul>

                    <div className="nav-actions">
                        <button
                            className="btn btn-ghost theme-toggle"
                            onClick={toggleTheme}
                            aria-label="테마 전환"
                        >
                            {theme === 'light' ? '🌙' : '☀️'}
                        </button>
                        <Link to="/upload" className="btn btn-primary">
                            ✍️ 글쓰기
                        </Link>
                    </div>
                </nav>
            </div>
        </header>
    );
}

export default Header;
