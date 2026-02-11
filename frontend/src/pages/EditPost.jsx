import { useState, useEffect, useRef } from 'react';
import { useParams, useNavigate, Link } from 'react-router-dom';
import { postsAPI } from '../api';
import { useAuth } from '../context/AuthContext';

const categories = [
    { key: 'essay', label: '에세이', icon: '📝', desc: '자유로운 형식의 글' },
    { key: 'paper', label: '논문', icon: '📄', desc: '학술적 연구 결과' },
    { key: 'report', label: '리포트', icon: '📊', desc: '분석 및 보고서' },
    { key: 'note', label: '노트', icon: '📒', desc: '학습 노트 및 정리' },
];

function EditPost() {
    const { id } = useParams();
    const navigate = useNavigate();
    const { user } = useAuth();
    const fileInputRef = useRef(null);

    const [loading, setLoading] = useState(true);
    const [title, setTitle] = useState('');
    const [content, setContent] = useState('');
    const [summary, setSummary] = useState('');
    const [category, setCategory] = useState('essay');
    const [tags, setTags] = useState('');
    const [citations, setCitations] = useState('');
    const [citationsTouched, setCitationsTouched] = useState(false);
    const [file, setFile] = useState(null);
    const [existingFile, setExistingFile] = useState(null);
    const [removeFile, setRemoveFile] = useState(false);
    const [dragActive, setDragActive] = useState(false);
    const [submitting, setSubmitting] = useState(false);
    const [error, setError] = useState(null);

    useEffect(() => {
        const fetchPost = async () => {
            try {
                const data = await postsAPI.getPost(id);
                // Check authorization
                if (user && data.author_id !== user.id) {
                    navigate(`/posts/${id}`);
                    return;
                }
                setTitle(data.title);
                setContent(data.content);
                setSummary(data.summary || '');
                setCategory(data.category);
                if (data.tags) {
                    setTags(data.tags.join(', '));
                }
                setCitations('');
                setCitationsTouched(false);
                if (data.file_name) {
                    setExistingFile({ name: data.file_name, path: data.file_path });
                }
            } catch (err) {
                console.error('Failed to fetch post:', err);
                setError('글을 불러오는데 실패했습니다.');
            } finally {
                setLoading(false);
            }
        };

        if (user) {
            fetchPost();
        } else {
            navigate('/login');
        }
    }, [id, user, navigate]);

    const handleFileChange = (e) => {
        const selectedFile = e.target.files?.[0];
        if (selectedFile) {
            setFile(selectedFile);
            setRemoveFile(false);
        }
    };

    const handleDrag = (e) => {
        e.preventDefault();
        e.stopPropagation();
        if (e.type === 'dragenter' || e.type === 'dragover') {
            setDragActive(true);
        } else if (e.type === 'dragleave') {
            setDragActive(false);
        }
    };

    const handleDrop = (e) => {
        e.preventDefault();
        e.stopPropagation();
        setDragActive(false);
        if (e.dataTransfer.files?.[0]) {
            setFile(e.dataTransfer.files[0]);
            setRemoveFile(false);
        }
    };

    const handleRemoveFile = () => {
        setFile(null);
        setRemoveFile(true);
        setExistingFile(null);
        if (fileInputRef.current) {
            fileInputRef.current.value = '';
        }
    };

    const handleRemoveNewFile = () => {
        setFile(null);
        if (fileInputRef.current) {
            fileInputRef.current.value = '';
        }
    };

    const formatFileSize = (bytes) => {
        if (bytes < 1024) return bytes + ' B';
        if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + ' KB';
        return (bytes / (1024 * 1024)).toFixed(1) + ' MB';
    };

    const handleSubmit = async (e) => {
        e.preventDefault();
        setError(null);

        if (!title.trim()) {
            setError('제목을 입력해주세요.');
            return;
        }
        if (!content.trim()) {
            setError('내용을 입력해주세요.');
            return;
        }

        setSubmitting(true);

        try {
            const citationsPayload =
                category === 'paper'
                    ? (citationsTouched ? citations.trim() : undefined)
                    : '';

            await postsAPI.updatePost(id, {
                title: title.trim(),
                content: content.trim(),
                summary: summary.trim() || '',
                category,
                tags: tags.trim() || undefined,
                citations: citationsPayload,
                file: file || undefined,
                removeFile: removeFile,
            });
            navigate(`/posts/${id}`);
        } catch (err) {
            console.error('Failed to update post:', err);
            if (err.response?.status === 401) {
                setError('로그인이 필요합니다.');
            } else if (err.response?.status === 403) {
                setError('이 글을 수정할 권한이 없습니다.');
            } else {
                setError(err.response?.data?.detail || '글 수정에 실패했습니다.');
            }
        } finally {
            setSubmitting(false);
        }
    };

    if (loading) {
        return (
            <main className="upload-page">
                <div className="container">
                    <div className="post-detail-skeleton">
                        <div className="skeleton-line skeleton-title" />
                        <div className="skeleton-line skeleton-meta" />
                        <div className="skeleton-line skeleton-content-1" />
                    </div>
                </div>
            </main>
        );
    }

    return (
        <main className="upload-page">
            <div className="container">
                <div className="upload-header">
                    <h1>✏️ 글 수정</h1>
                    <p>내용을 수정하고 저장하세요.</p>
                </div>

                <form className="upload-form" onSubmit={handleSubmit}>
                    {error && (
                        <div className="form-error">
                            <span className="form-error-icon">⚠️</span>
                            {error}
                        </div>
                    )}

                    {/* Category Selection */}
                    <div className="form-group">
                        <label className="form-label">카테고리</label>
                        <div className="category-selector">
                            {categories.map(cat => (
                                <button
                                    key={cat.key}
                                    type="button"
                                    className={`category-option ${category === cat.key ? 'active' : ''}`}
                                    onClick={() => setCategory(cat.key)}
                                >
                                    <span className="category-option-icon">{cat.icon}</span>
                                    <span className="category-option-label">{cat.label}</span>
                                    <span className="category-option-desc">{cat.desc}</span>
                                </button>
                            ))}
                        </div>
                    </div>

                    {/* Title */}
                    <div className="form-group">
                        <label className="form-label" htmlFor="title">
                            제목 <span className="required">*</span>
                        </label>
                        <input
                            id="title"
                            type="text"
                            className="form-input"
                            placeholder="글의 제목을 입력하세요"
                            value={title}
                            onChange={(e) => setTitle(e.target.value)}
                            maxLength={200}
                        />
                        <span className="form-hint">{title.length}/200</span>
                    </div>



                    {/* Summary */}
                    <div className="form-group">
                        <label className="form-label" htmlFor="summary">
                            요약 <span className="optional">(선택)</span>
                        </label>
                        <input
                            id="summary"
                            type="text"
                            className="form-input"
                            placeholder="글을 한 줄로 요약해주세요"
                            value={summary}
                            onChange={(e) => setSummary(e.target.value)}
                            maxLength={300}
                        />
                    </div>

                    {/* Tags */}
                    <div className="form-group">
                        <label className="form-label" htmlFor="tags">
                            태그 <span className="optional">(선택)</span>
                        </label>
                        <input
                            id="tags"
                            type="text"
                            className="form-input"
                            placeholder="태그를 입력하세요 (쉼표로 구분)"
                            value={tags}
                            onChange={(e) => setTags(e.target.value)}
                        />
                    </div>

                    {category === 'paper' && (
                        <div className="form-group">
                            <label className="form-label" htmlFor="citations">
                                인용 문헌 ID <span className="optional">(선택)</span>
                            </label>
                            <input
                                id="citations"
                                type="text"
                                className="form-input"
                                placeholder="쉼표로 구분된 게시글 ID (입력 시 전체 교체, 예: 12,34,56)"
                                value={citations}
                                onChange={(e) => {
                                    setCitations(e.target.value);
                                    setCitationsTouched(true);
                                }}
                            />
                            <span className="form-hint">비워두면 기존 인용 관계를 유지합니다.</span>
                            <span className="form-hint">본문의 `/posts/{'{'}ID{'}'}` 링크 또는 `cite:ID` 표기도 자동 인용으로 추출됩니다.</span>
                        </div>
                    )}

                    {/* Content */}
                    <div className="form-group">
                        <label className="form-label" htmlFor="content">
                            내용 <span className="required">*</span>
                        </label>
                        <textarea
                            id="content"
                            className="form-textarea"
                            placeholder="학습한 내용을 자유롭게 작성하세요..."
                            value={content}
                            onChange={(e) => setContent(e.target.value)}
                            rows={16}
                        />
                    </div>

                    {/* File Upload */}
                    <div className="form-group">
                        <label className="form-label">
                            파일 첨부 <span className="optional">(선택)</span>
                        </label>

                        {/* Show existing file */}
                        {existingFile && !file && (
                            <div className="existing-file-info">
                                <div className="file-preview">
                                    <div className="file-preview-info">
                                        <span className="file-preview-icon">📎</span>
                                        <div>
                                            <div className="file-preview-name">{existingFile.name}</div>
                                            <div className="file-preview-size">기존 첨부파일</div>
                                        </div>
                                    </div>
                                    <button
                                        type="button"
                                        className="file-remove-btn"
                                        onClick={handleRemoveFile}
                                    >
                                        ✕
                                    </button>
                                </div>
                            </div>
                        )}

                        <div
                            className={`file-dropzone ${dragActive ? 'drag-active' : ''} ${file ? 'has-file' : ''}`}
                            onDragEnter={handleDrag}
                            onDragLeave={handleDrag}
                            onDragOver={handleDrag}
                            onDrop={handleDrop}
                            onClick={() => !file && fileInputRef.current?.click()}
                        >
                            <input
                                ref={fileInputRef}
                                type="file"
                                className="file-input-hidden"
                                onChange={handleFileChange}
                                accept=".pdf,.doc,.docx,.txt,.md,.pptx,.xlsx,.zip,.png,.jpg,.jpeg,.gif"
                            />
                            {file ? (
                                <div className="file-preview">
                                    <div className="file-preview-info">
                                        <span className="file-preview-icon">📎</span>
                                        <div>
                                            <div className="file-preview-name">{file.name}</div>
                                            <div className="file-preview-size">{formatFileSize(file.size)}</div>
                                        </div>
                                    </div>
                                    <button
                                        type="button"
                                        className="file-remove-btn"
                                        onClick={(e) => { e.stopPropagation(); handleRemoveNewFile(); }}
                                    >
                                        ✕
                                    </button>
                                </div>
                            ) : (
                                <div className="file-dropzone-content">
                                    <span className="file-dropzone-icon">📁</span>
                                    <p>{existingFile ? '새 파일로 교체하려면 클릭하세요' : '파일을 드래그하여 놓거나 클릭하여 업로드하세요'}</p>
                                    <span className="file-dropzone-hint">
                                        PDF, Word, PPT, 이미지 등 (최대 10MB)
                                    </span>
                                </div>
                            )}
                        </div>
                    </div>

                    {/* Actions */}
                    <div className="form-actions">
                        <Link
                            to={`/posts/${id}`}
                            className="btn btn-secondary"
                        >
                            취소
                        </Link>
                        <button
                            type="submit"
                            className="btn btn-primary"
                            disabled={submitting || !title.trim() || !content.trim()}
                        >
                            {submitting ? (
                                <>
                                    <span className="spinner" />
                                    저장 중...
                                </>
                            ) : (
                                '💾 수정 완료'
                            )}
                        </button>
                    </div>
                </form>
            </div >
        </main >
    );
}

export default EditPost;
