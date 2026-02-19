import DesmosGraphBlock from './DesmosGraphBlock';
import TikzCdDiagram from './TikzCdDiagram';
import ReactMarkdown from 'react-markdown';
import rehypeKatex from 'rehype-katex';
import remarkBreaks from 'remark-breaks';
import remarkGfm from 'remark-gfm';
import remarkMath from 'remark-math';

function transformTikzCdBlocks(markdown) {
    if (!markdown) return '';

    const wrapAsCodeFence = (_whole, body) => `\n\`\`\`tikzcd\n${body.trim()}\n\`\`\`\n`;
    const squareBracketPattern = /\\\[\s*\\begin\{tikzcd\}([\s\S]*?)\\end\{tikzcd\}\s*\\\]/g;
    const dollarPattern = /\$\$\s*\\begin\{tikzcd\}([\s\S]*?)\\end\{tikzcd\}\s*\$\$/g;
    const barePattern = /\\begin\{tikzcd\}([\s\S]*?)\\end\{tikzcd\}/g;

    return markdown
        .replace(squareBracketPattern, wrapAsCodeFence)
        .replace(dollarPattern, wrapAsCodeFence)
        .replace(barePattern, wrapAsCodeFence);
}

function MarkdownRenderer({ content, className = '', enableInteractiveEmbeds = true }) {
    const markdownText = transformTikzCdBlocks(typeof content === 'string' ? content : '');
    const mergedClassName = ['markdown-body', className].filter(Boolean).join(' ');
    const components = {
        code({ inline, className: codeClassName, children, ...props }) {
            const text = String(children ?? '').replace(/\n$/, '');
            const matched = /language-([a-z0-9_-]+)/i.exec(codeClassName || '');
            const language = matched?.[1]?.toLowerCase();

            if (!inline && enableInteractiveEmbeds && language === 'desmos') {
                return <DesmosGraphBlock source={text} />;
            }
            if (!inline && language === 'tikzcd') {
                return <TikzCdDiagram source={text} />;
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
