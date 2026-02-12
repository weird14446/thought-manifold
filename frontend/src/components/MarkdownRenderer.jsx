import ReactMarkdown from 'react-markdown';
import rehypeKatex from 'rehype-katex';
import remarkBreaks from 'remark-breaks';
import remarkGfm from 'remark-gfm';
import remarkMath from 'remark-math';

function MarkdownRenderer({ content, className = '' }) {
    const markdownText = typeof content === 'string' ? content : '';
    const mergedClassName = ['markdown-body', className].filter(Boolean).join(' ');

    return (
        <ReactMarkdown
            className={mergedClassName}
            skipHtml
            remarkPlugins={[remarkGfm, remarkMath, remarkBreaks]}
            rehypePlugins={[[rehypeKatex, { throwOnError: false, strict: 'ignore' }]]}
        >
            {markdownText}
        </ReactMarkdown>
    );
}

export default MarkdownRenderer;
