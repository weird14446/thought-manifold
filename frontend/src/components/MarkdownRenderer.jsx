import DesmosGraphBlock from './DesmosGraphBlock';
import ReactMarkdown from 'react-markdown';
import rehypeKatex from 'rehype-katex';
import remarkBreaks from 'remark-breaks';
import remarkGfm from 'remark-gfm';
import remarkMath from 'remark-math';

function MarkdownRenderer({ content, className = '', enableInteractiveEmbeds = true }) {
    const markdownText = typeof content === 'string' ? content : '';
    const mergedClassName = ['markdown-body', className].filter(Boolean).join(' ');
    const components = {
        code({ inline, className: codeClassName, children, ...props }) {
            const text = String(children ?? '').replace(/\n$/, '');
            const matched = /language-([a-z0-9_-]+)/i.exec(codeClassName || '');
            const language = matched?.[1]?.toLowerCase();

            if (!inline && enableInteractiveEmbeds && language === 'desmos') {
                return <DesmosGraphBlock source={text} />;
            }

            return (
                <code className={codeClassName} {...props}>
                    {children}
                </code>
            );
        },
    };

    return (
        <ReactMarkdown
            className={mergedClassName}
            skipHtml
            components={components}
            remarkPlugins={[remarkGfm, remarkMath, remarkBreaks]}
            rehypePlugins={[[rehypeKatex, { throwOnError: false, strict: 'ignore' }]]}
        >
            {markdownText}
        </ReactMarkdown>
    );
}

export default MarkdownRenderer;
