import { useEffect, useState } from 'react';
import { Link, useNavigate } from 'react-router-dom';
import { reviewsAPI } from '../api';
import { useAuth } from '../context/AuthContext';

const reviewStatusLabels = {
    pending: '심사 대기중',
    completed: '심사 완료',
    failed: '심사 실패',
};

const reviewDecisionLabels = {
    accept: 'Accept',
    minor_revision: 'Minor Revision',
    major_revision: 'Major Revision',
    reject: 'Reject',
};

const paperStatusLabels = {
    draft: 'Draft',
    submitted: 'Submitted',
    revision: 'Revision',
    accepted: 'Accepted',
    published: 'Published',
    rejected: 'Rejected',
};

function ReviewCenter() {
    const navigate = useNavigate();
    const { user, loading: authLoading } = useAuth();

    const [items, setItems] = useState([]);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState(null);
    const [page, setPage] = useState(1);
    const [perPage] = useState(20);
    const [total, setTotal] = useState(0);
    const [reloadNonce, setReloadNonce] = useState(0);

    useEffect(() => {
        if (!authLoading && !user) {
            navigate('/login');
        }
    }, [authLoading, user, navigate]);

    useEffect(() => {
        if (!user) return;

        let cancelled = false;
        const fetchMine = async () => {
            setLoading(true);
            setError(null);
            try {
                const data = await reviewsAPI.getMine(page, perPage);
                if (cancelled) return;
                setItems(data.items || []);
                setTotal(data.total || 0);
            } catch (err) {
                if (cancelled) return;
                setError(err.response?.data?.detail || '심사 현황을 불러오지 못했습니다.');
                setItems([]);
                setTotal(0);
            } finally {
                if (!cancelled) {
                    setLoading(false);
                }
            }
        };

        fetchMine();
        return () => {
            cancelled = true;
        };
    }, [user, page, perPage, reloadNonce]);

    if (authLoading || (!user && !error)) {
        return (
            <main className="review-center-page">
                <div className="container">
                    <div className="review-center-loading">로딩 중...</div>
                </div>
            </main>
        );
    }

    const totalPages = Math.max(1, Math.ceil(total / perPage));

    return (
        <main className="review-center-page">
            <div className="container">
                <section className="review-center-header">
                    <div>
                        <h1>AI 심사 센터</h1>
                        <p>논문별 심사 진행 상태와 최신 판정을 확인할 수 있습니다.</p>
                    </div>
                    <button
                        type="button"
                        className="btn btn-secondary"
                        onClick={() => setReloadNonce((prev) => prev + 1)}
                    >
                        새로고침
                    </button>
                </section>

                {error ? (
                    <div className="review-center-error">
                        <p>{error}</p>
                        <button
                            type="button"
                            className="btn btn-primary"
                            onClick={() => setReloadNonce((prev) => prev + 1)}
                        >
                            다시 시도
                        </button>
                    </div>
                ) : loading ? (
                    <div className="review-center-loading">심사 현황을 불러오는 중...</div>
                ) : items.length === 0 ? (
                    <div className="review-center-empty">
                        아직 등록된 논문 심사 대상이 없습니다.
                    </div>
                ) : (
                    <div className="review-center-list">
                        {items.map((item) => {
                            const review = item.latest_review;
                            const isPublished = !!item.is_published;
                            const detailLink = isPublished
                                ? `/posts/${item.post_id}`
                                : `/posts/${item.post_id}?source=review_center`;

                            return (
                                <article key={item.post_id} className="review-center-card">
                                    <div className="review-center-card-head">
                                        <div>
                                            <h2>
                                                <Link to={detailLink}>{item.title}</Link>
                                            </h2>
                                            <p className="review-center-meta">
                                                ID {item.post_id} · {item.category} · Revision v{item.current_revision || 0} · 상태 {paperStatusLabels[item.paper_status] || item.paper_status}
                                            </p>
                                        </div>
                                        <span className={`review-publish-badge ${isPublished ? 'published' : 'unpublished'}`}>
                                            {isPublished ? '게시됨' : '미게시'}
                                        </span>
                                    </div>

                                    {!review ? (
                                        <div className="review-center-no-review">아직 AI 심사 이력이 없습니다.</div>
                                    ) : (
                                        <div className="review-center-review">
                                            <span className={`ai-review-status-badge ${review.status}`}>
                                                {reviewStatusLabels[review.status] || review.status}
                                            </span>
                                            <span className="review-center-value">
                                                판정: {reviewDecisionLabels[review.decision] || '-'}
                                            </span>
                                            <span className="review-center-value">
                                                총점: {review.overall_score ?? '-'}
                                            </span>
                                            {review.error_message && (
                                                <p className="review-center-inline-error">{review.error_message}</p>
                                            )}
                                        </div>
                                    )}

                                    <div className="review-center-actions">
                                        <Link className="btn btn-secondary btn-sm" to={detailLink}>
                                            상세 보기
                                        </Link>
                                        {!isPublished && (
                                            <span className="review-center-only-hint">심사센터 전용 접근</span>
                                        )}
                                    </div>
                                </article>
                            );
                        })}
                    </div>
                )}

                {total > perPage && (
                    <div className="review-center-pagination">
                        <button
                            type="button"
                            className="btn btn-secondary btn-sm"
                            onClick={() => setPage((prev) => Math.max(1, prev - 1))}
                            disabled={page <= 1}
                        >
                            이전
                        </button>
                        <span>{page} / {totalPages}</span>
                        <button
                            type="button"
                            className="btn btn-secondary btn-sm"
                            onClick={() => setPage((prev) => Math.min(totalPages, prev + 1))}
                            disabled={page >= totalPages}
                        >
                            다음
                        </button>
                    </div>
                )}
            </div>
        </main>
    );
}

export default ReviewCenter;
