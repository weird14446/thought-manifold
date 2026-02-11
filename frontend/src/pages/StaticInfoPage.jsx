import { Link } from 'react-router-dom';

function StaticInfoPage({ title, description }) {
    return (
        <main>
            <section className="posts-section">
                <div className="container">
                    <div className="empty-state">
                        <div className="empty-state-icon">📌</div>
                        <h2>{title}</h2>
                        <p>{description}</p>
                        <p>해당 페이지는 현재 준비 중입니다.</p>
                        <Link to="/" className="btn btn-primary">
                            홈으로 이동
                        </Link>
                    </div>
                </div>
            </section>
        </main>
    );
}

export default StaticInfoPage;
