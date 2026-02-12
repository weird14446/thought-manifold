import { useState, useRef } from 'react';
import { useNavigate } from 'react-router-dom';
import { postsAPI } from '../api';

const categories = [
    { key: 'essay', label: '에세이', icon: '📝', desc: '자유로운 형식의 글' },
    { key: 'paper', label: '논문', icon: '📄', desc: '학술적 연구 결과' },
    { key: 'report', label: '리포트', icon: '📊', desc: '분석 및 보고서' },
    { key: 'note', label: '노트', icon: '📒', desc: '학습 노트 및 정리' },
];

function Upload() {
    const navigate = useNavigate();
    const fileInputRef = useRef(null);

    const [title, setTitle] = useState('');
    const [content, setContent] = useState('');
    const [summary, setSummary] = useState('');
    const [category, setCategory] = useState('essay');
    const [tags, setTags] = useState('');
    const [citations, setCitations] = useState('');
    const [file, setFile] = useState(null);
    const [dragActive, setDragActive] = useState(false);
    const [submitting, setSubmitting] = useState(false);
    const [error, setError] = useState(null);

    const handleFileChange = (e) => {
        const selectedFile = e.target.files?.[0];
        if (selectedFile) {
            setFile(selectedFile);
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
        }
    };

    const removeFile = () => {
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
            await postsAPI.createPost({
                title: title.trim(),
                content: content.trim(),
                summary: summary.trim() || undefined,
                category,
                tags: tags.trim() || undefined,
                citations: category === 'paper' ? (citations.trim() || undefined) : undefined,
                file: file || undefined,
            });
            navigate(category === 'paper' ? '/reviews' : '/');
        } catch (err) {
            console.error('Failed to create post:', err);
            if (err.response?.status === 401) {
                setError('로그인이 필요합니다. 먼저 로그인해주세요.');
            } else {
                setError(err.response?.data?.detail || '글 작성에 실패했습니다. 다시 시도해주세요.');
            }
        } finally {
            setSubmitting(false);
        }
    };

    return (
        <main className="upload-page">
            <div className="container">
                <div className="upload-header">
                    <h1>✍️ 새 글 작성</h1>
                    <p>학습한 내용을 정리하고 커뮤니티와 공유하세요.</p>
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
                            placeholder="태그를 입력하세요 (쉼표로 구분, 예: 공부, 리액트, 일상)"
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
                                placeholder="쉼표로 구분된 게시글 ID (예: 12,34,56)"
                                value={citations}
                                onChange={(e) => setCitations(e.target.value)}
                            />
                            <span className="form-hint">논문 카테고리에서만 인용 문헌을 입력할 수 있습니다.</span>
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
                                        onClick={(e) => { e.stopPropagation(); removeFile(); }}
                                    >
                                        ✕
                                    </button>
                                </div>
                            ) : (
                                <div className="file-dropzone-content">
                                    <span className="file-dropzone-icon">📁</span>
                                    <p>파일을 드래그하여 놓거나 클릭하여 업로드하세요</p>
                                    <span className="file-dropzone-hint">
                                        PDF, Word, PPT, 이미지 등 (최대 10MB)
                                    </span>
                                </div>
                            )}
                        </div>
                    </div>

                    {/* Actions */}
                    <div className="form-actions">
                        <button
                            type="button"
                            className="btn btn-secondary"
                            onClick={() => navigate('/')}
                            disabled={submitting}
                        >
                            취소
                        </button>
                        <button
                            type="submit"
                            className="btn btn-primary"
                            disabled={submitting || !title.trim() || !content.trim()}
                        >
                            {submitting ? (
                                <>
                                    <span className="spinner" />
                                    업로드 중...
                                </>
                            ) : (
                                '📤 글 발행하기'
                            )}
                        </button>
                    </div>
                </form>
            </div >
        </main >
    );
}

export default Upload;
