import { useState } from 'react';
import MarkdownRenderer from './MarkdownRenderer';

function MarkdownEditorPreview({
    value,
    onChange,
    placeholder,
    rows = 12,
    inputId,
    compact = false,
    previewClassName = '',
    emptyText = '미리보기가 여기에 표시됩니다.',
}) {
    const [mobileTab, setMobileTab] = useState('edit');
    const textValue = typeof value === 'string' ? value : '';
    const textareaClassName = [
        compact ? 'comment-input' : 'form-textarea',
        'editor-preview-textarea',
    ]
        .filter(Boolean)
        .join(' ');

    return (
        <div className={`editor-preview ${compact ? 'compact' : ''}`}>
            <div className="editor-preview-tabs" role="tablist" aria-label="편집 모드">
                <button
                    type="button"
                    className={`editor-preview-tab ${mobileTab === 'edit' ? 'active' : ''}`}
                    onClick={() => setMobileTab('edit')}
                >
                    편집
                </button>
                <button
                    type="button"
                    className={`editor-preview-tab ${mobileTab === 'preview' ? 'active' : ''}`}
                    onClick={() => setMobileTab('preview')}
                >
                    미리보기
                </button>
            </div>

            <div className={`editor-preview-layout ${compact ? 'compact' : ''}`}>
                <div className={`editor-pane ${mobileTab === 'edit' ? 'is-active' : ''}`}>
                    <textarea
                        id={inputId}
                        className={textareaClassName}
                        placeholder={placeholder}
                        value={textValue}
                        onChange={(event) => onChange(event.target.value)}
                        rows={rows}
                    />
                </div>

                <div className={`preview-pane ${mobileTab === 'preview' ? 'is-active' : ''}`}>
                    <div className="editor-preview-title">미리보기</div>
                    {textValue.trim() ? (
                        <MarkdownRenderer
                            content={textValue}
                            className={previewClassName}
                        />
                    ) : (
                        <p className="editor-preview-empty">{emptyText}</p>
                    )}
                </div>
            </div>
        </div>
    );
}

export default MarkdownEditorPreview;
