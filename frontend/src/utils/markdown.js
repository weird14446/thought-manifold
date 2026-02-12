export function extractFirstNonEmptyParagraph(markdown) {
    if (!markdown || typeof markdown !== 'string') {
        return '';
    }

    const normalized = markdown.replace(/\r\n/g, '\n').trim();
    if (!normalized) {
        return '';
    }

    const paragraphs = normalized
        .split(/\n\s*\n/)
        .map((section) => section.trim())
        .filter(Boolean);

    return paragraphs[0] || '';
}

function truncateText(text, maxChars = 220) {
    if (!text) {
        return '';
    }

    const normalized = text.replace(/\s+/g, ' ').trim();
    if (normalized.length <= maxChars) {
        return normalized;
    }

    const sliced = normalized.slice(0, maxChars + 1);
    const lastSpaceIndex = sliced.lastIndexOf(' ');
    const safeCutoff = Math.floor(maxChars * 0.6);
    const cutoff = lastSpaceIndex > safeCutoff ? lastSpaceIndex : maxChars;

    return `${normalized.slice(0, cutoff).trimEnd()}...`;
}

export function getPostExcerptMarkdown(post) {
    const summary = post?.summary?.trim();
    if (summary) {
        return truncateText(summary);
    }

    const firstParagraph = extractFirstNonEmptyParagraph(post?.content || '');
    return truncateText(firstParagraph);
}
