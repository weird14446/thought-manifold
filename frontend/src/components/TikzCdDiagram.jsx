import { useId, useMemo } from 'react';
import katex from 'katex';

const NODE_WIDTH = 152;
const NODE_HEIGHT = 52;
const GRID_GAP_X = 176;
const GRID_GAP_Y = 96;
const PADDING_X = 96;
const PADDING_Y = 64;
const ARROW_EDGE_PADDING = 26;
const LABEL_OFFSET = 16;

function stripOuterBraces(raw) {
    if (typeof raw !== 'string') return '';
    const trimmed = raw.trim();
    if (trimmed.startsWith('{') && trimmed.endsWith('}')) {
        return trimmed.slice(1, -1).trim();
    }
    return trimmed;
}

function normalizeLatex(raw) {
    if (typeof raw !== 'string') return '';
    return stripOuterBraces(raw)
        .replace(/\\,/g, ' ')
        .trim();
}

function renderLatex(raw) {
    const latex = normalizeLatex(raw);
    if (!latex) return '';
    try {
        return katex.renderToString(latex, {
            throwOnError: false,
            strict: 'ignore',
            displayMode: false,
        });
    } catch {
        return latex
            .replace(/&/g, '&amp;')
            .replace(/</g, '&lt;')
            .replace(/>/g, '&gt;');
    }
}

function parseNodeRows(source) {
    const lines = source
        .replace(/\r\n/g, '\n')
        .split('\n')
        .map((line) => line.trim())
        .filter(Boolean)
        .filter((line) => !line.startsWith('\\arrow'));

    const matrixText = lines.join('\n');
    const rows = matrixText
        .split(/\\\\/g)
        .map((row) => row.trim())
        .filter(Boolean)
        .map((row) => row.split('&').map((cell) => normalizeLatex(cell)));

    return rows;
}

function parseArrow(sourceLine) {
    const bodyMatch = sourceLine.trim().match(/^\\arrow\s*\[(.*)\]\s*$/);
    if (!bodyMatch) {
        return null;
    }

    const options = bodyMatch[1];
    const fromMatch = options.match(/from\s*=\s*(\d+)\s*-\s*(\d+)/i);
    const toMatch = options.match(/to\s*=\s*(\d+)\s*-\s*(\d+)/i);
    if (!fromMatch || !toMatch) {
        return null;
    }

    const from = {
        row: Number.parseInt(fromMatch[1], 10) - 1,
        col: Number.parseInt(fromMatch[2], 10) - 1,
    };
    const to = {
        row: Number.parseInt(toMatch[1], 10) - 1,
        col: Number.parseInt(toMatch[2], 10) - 1,
    };

    const labelMatch = options.match(/"([^"]*)"\s*('?)/);
    const label = labelMatch ? normalizeLatex(labelMatch[1]) : '';
    const flipped = Boolean(labelMatch?.[2]);
    const dashed = /\bdashed\b/i.test(options);

    return { from, to, label, flipped, dashed };
}

function parseTikzCd(source) {
    const rows = parseNodeRows(source);
    const arrows = source
        .replace(/\r\n/g, '\n')
        .split('\n')
        .map((line) => line.trim())
        .filter((line) => line.startsWith('\\arrow'))
        .map(parseArrow)
        .filter(Boolean);

    return {
        rows,
        arrows,
    };
}

function toCanvasCoordinates(row, col) {
    return {
        x: PADDING_X + (col * GRID_GAP_X),
        y: PADDING_Y + (row * GRID_GAP_Y),
    };
}

function buildArrowGeometry(arrow) {
    const fromPoint = toCanvasCoordinates(arrow.from.row, arrow.from.col);
    const toPoint = toCanvasCoordinates(arrow.to.row, arrow.to.col);

    const dx = toPoint.x - fromPoint.x;
    const dy = toPoint.y - fromPoint.y;
    const len = Math.hypot(dx, dy);
    if (len === 0) {
        return null;
    }

    const ux = dx / len;
    const uy = dy / len;

    const startX = fromPoint.x + (ux * ARROW_EDGE_PADDING);
    const startY = fromPoint.y + (uy * ARROW_EDGE_PADDING);
    const endX = toPoint.x - (ux * ARROW_EDGE_PADDING);
    const endY = toPoint.y - (uy * ARROW_EDGE_PADDING);

    const midX = (startX + endX) / 2;
    const midY = (startY + endY) / 2;
    const perpX = -uy;
    const perpY = ux;
    const side = arrow.flipped ? -1 : 1;

    return {
        startX,
        startY,
        endX,
        endY,
        labelX: midX + (perpX * LABEL_OFFSET * side),
        labelY: midY + (perpY * LABEL_OFFSET * side),
    };
}

function TikzCdDiagram({ source }) {
    const markerId = useId().replace(/:/g, '_');
    const parsed = useMemo(() => parseTikzCd(source || ''), [source]);
    const rowCount = parsed.rows.length;
    const columnCount = parsed.rows.reduce((max, row) => Math.max(max, row.length), 0);

    if (!rowCount || !columnCount) {
        return (
            <div className="tikzcd-diagram tikzcd-diagram-error" data-tikzcd-block>
                <p className="tikzcd-error-text">tikzcd 다이어그램을 해석할 수 없습니다.</p>
                <pre className="tikzcd-fallback"><code>{source}</code></pre>
            </div>
        );
    }

    const width = (PADDING_X * 2) + ((columnCount - 1) * GRID_GAP_X);
    const height = (PADDING_Y * 2) + ((rowCount - 1) * GRID_GAP_Y);

    return (
        <div className="tikzcd-diagram" data-tikzcd-block>
            <div className="tikzcd-header">TikZ-CD Diagram</div>
            <div className="tikzcd-stage" style={{ width, height }}>
                <svg className="tikzcd-svg" width={width} height={height} viewBox={`0 0 ${width} ${height}`}>
                    <defs>
                        <marker
                            id={markerId}
                            viewBox="0 0 10 8"
                            refX="9"
                            refY="4"
                            markerWidth="8"
                            markerHeight="8"
                            orient="auto-start-reverse"
                        >
                            <path d="M0,0 L10,4 L0,8 z" fill="currentColor" />
                        </marker>
                    </defs>
                    {parsed.arrows.map((arrow, index) => {
                        const geometry = buildArrowGeometry(arrow);
                        if (!geometry) return null;

                        return (
                            <g key={`arrow-${index}`} className="tikzcd-arrow-group">
                                <line
                                    x1={geometry.startX}
                                    y1={geometry.startY}
                                    x2={geometry.endX}
                                    y2={geometry.endY}
                                    className={`tikzcd-arrow ${arrow.dashed ? 'dashed' : ''}`}
                                    markerEnd={`url(#${markerId})`}
                                />
                            </g>
                        );
                    })}
                </svg>

                <div className="tikzcd-layer">
                    {parsed.rows.map((row, rowIndex) =>
                        row.map((cell, colIndex) => {
                            const coords = toCanvasCoordinates(rowIndex, colIndex);
                            const rendered = renderLatex(cell);
                            if (!rendered) return null;

                            return (
                                <div
                                    key={`node-${rowIndex}-${colIndex}`}
                                    className="tikzcd-node"
                                    style={{
                                        width: NODE_WIDTH,
                                        minHeight: NODE_HEIGHT,
                                        left: coords.x - (NODE_WIDTH / 2),
                                        top: coords.y - (NODE_HEIGHT / 2),
                                    }}
                                    dangerouslySetInnerHTML={{ __html: rendered }}
                                />
                            );
                        }),
                    )}

                    {parsed.arrows.map((arrow, index) => {
                        if (!arrow.label) return null;
                        const geometry = buildArrowGeometry(arrow);
                        if (!geometry) return null;

                        return (
                            <div
                                key={`label-${index}`}
                                className="tikzcd-label"
                                style={{
                                    left: geometry.labelX,
                                    top: geometry.labelY,
                                }}
                                dangerouslySetInnerHTML={{ __html: renderLatex(arrow.label) }}
                            />
                        );
                    })}
                </div>
            </div>
        </div>
    );
}

export default TikzCdDiagram;
