import { useState } from 'react';
import { useNavigate, Link } from 'react-router-dom';
import { useAuth } from '../context/AuthContext';

function Login() {
    const navigate = useNavigate();
    const { login, register } = useAuth();

    const [isRegister, setIsRegister] = useState(false);
    const [username, setUsername] = useState('');
    const [email, setEmail] = useState('');
    const [password, setPassword] = useState('');
    const [confirmPassword, setConfirmPassword] = useState('');
    const [error, setError] = useState(null);
    const [submitting, setSubmitting] = useState(false);

    const handleSubmit = async (e) => {
        e.preventDefault();
        setError(null);

        if (!username.trim() || !password.trim()) {
            setError('아이디와 비밀번호를 입력해주세요.');
            return;
        }

        if (isRegister) {
            if (!email.trim()) {
                setError('이메일을 입력해주세요.');
                return;
            }
            if (password.length < 6) {
                setError('비밀번호는 6자 이상이어야 합니다.');
                return;
            }
            if (password !== confirmPassword) {
                setError('비밀번호가 일치하지 않습니다.');
                return;
            }
        }

        setSubmitting(true);

        try {
            if (isRegister) {
                await register({ username: username.trim(), email: email.trim(), password });
            } else {
                await login(username.trim(), password);
            }
            navigate('/');
        } catch (err) {
            console.error('Auth error:', err);
            if (err.response?.status === 401) {
                setError('아이디 또는 비밀번호가 올바르지 않습니다.');
            } else if (err.response?.status === 409 || err.response?.data?.detail?.includes?.('exists')) {
                setError('이미 존재하는 아이디입니다.');
            } else {
                setError(err.response?.data?.detail || (isRegister ? '회원가입에 실패했습니다.' : '로그인에 실패했습니다.'));
            }
        } finally {
            setSubmitting(false);
        }
    };

    const handleGoogleLogin = () => {
        window.location.href = '/api/auth/google';
    };

    const toggleMode = () => {
        setIsRegister(prev => !prev);
        setError(null);
    };

    return (
        <main className="login-page">
            <div className="container">
                <div className="login-card">
                    <div className="login-header">
                        <span className="login-logo">💭</span>
                        <h1>{isRegister ? '회원가입' : '로그인'}</h1>
                        <p>{isRegister ? '새 계정을 만들어 시작하세요' : 'Thought Manifold에 오신 것을 환영합니다'}</p>
                    </div>

                    {/* Google Login Button */}
                    <button
                        type="button"
                        className="google-login-btn"
                        onClick={handleGoogleLogin}
                    >
                        <svg className="google-icon" viewBox="0 0 24 24" width="20" height="20">
                            <path d="M22.56 12.25c0-.78-.07-1.53-.2-2.25H12v4.26h5.92a5.06 5.06 0 0 1-2.2 3.32v2.77h3.57c2.08-1.92 3.28-4.74 3.28-8.1z" fill="#4285F4" />
                            <path d="M12 23c2.97 0 5.46-.98 7.28-2.66l-3.57-2.77c-.98.66-2.23 1.06-3.71 1.06-2.86 0-5.29-1.93-6.16-4.53H2.18v2.84C3.99 20.53 7.7 23 12 23z" fill="#34A853" />
                            <path d="M5.84 14.09c-.22-.66-.35-1.36-.35-2.09s.13-1.43.35-2.09V7.07H2.18C1.43 8.55 1 10.22 1 12s.43 3.45 1.18 4.93l2.85-2.22.81-.62z" fill="#FBBC05" />
                            <path d="M12 5.38c1.62 0 3.06.56 4.21 1.64l3.15-3.15C17.45 2.09 14.97 1 12 1 7.7 1 3.99 3.47 2.18 7.07l3.66 2.84c.87-2.6 3.3-4.53 6.16-4.53z" fill="#EA4335" />
                        </svg>
                        Google로 {isRegister ? '가입' : '로그인'}
                    </button>

                    <div className="login-divider">
                        <span>또는</span>
                    </div>

                    <form className="login-form" onSubmit={handleSubmit}>
                        {error && (
                            <div className="form-error">
                                <span className="form-error-icon">⚠️</span>
                                {error}
                            </div>
                        )}

                        <div className="form-group">
                            <label className="form-label" htmlFor="username">아이디</label>
                            <input
                                id="username"
                                type="text"
                                className="form-input"
                                placeholder="아이디를 입력하세요"
                                value={username}
                                onChange={(e) => setUsername(e.target.value)}
                                autoComplete="username"
                                autoFocus
                            />
                        </div>

                        {isRegister && (
                            <div className="form-group">
                                <label className="form-label" htmlFor="email">이메일</label>
                                <input
                                    id="email"
                                    type="email"
                                    className="form-input"
                                    placeholder="이메일을 입력하세요"
                                    value={email}
                                    onChange={(e) => setEmail(e.target.value)}
                                    autoComplete="email"
                                />
                            </div>
                        )}

                        <div className="form-group">
                            <label className="form-label" htmlFor="password">비밀번호</label>
                            <input
                                id="password"
                                type="password"
                                className="form-input"
                                placeholder="비밀번호를 입력하세요"
                                value={password}
                                onChange={(e) => setPassword(e.target.value)}
                                autoComplete={isRegister ? 'new-password' : 'current-password'}
                            />
                        </div>

                        {isRegister && (
                            <div className="form-group">
                                <label className="form-label" htmlFor="confirm-password">비밀번호 확인</label>
                                <input
                                    id="confirm-password"
                                    type="password"
                                    className="form-input"
                                    placeholder="비밀번호를 다시 입력하세요"
                                    value={confirmPassword}
                                    onChange={(e) => setConfirmPassword(e.target.value)}
                                    autoComplete="new-password"
                                />
                            </div>
                        )}

                        <button
                            type="submit"
                            className="btn btn-primary login-submit"
                            disabled={submitting}
                        >
                            {submitting ? (
                                <>
                                    <span className="spinner" />
                                    {isRegister ? '가입 중...' : '로그인 중...'}
                                </>
                            ) : (
                                isRegister ? '🚀 회원가입' : '🔐 로그인'
                            )}
                        </button>
                    </form>

                    <div className="login-footer">
                        <p>
                            {isRegister ? '이미 계정이 있으신가요?' : '아직 계정이 없으신가요?'}
                            <button type="button" className="login-toggle" onClick={toggleMode}>
                                {isRegister ? '로그인' : '회원가입'}
                            </button>
                        </p>
                    </div>
                </div>
            </div>
        </main>
    );
}

export default Login;
