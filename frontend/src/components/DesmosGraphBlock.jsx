import { useEffect, useMemo, useRef, useState } from 'react';

const DESMOS_SCRIPT_ID = 'desmos-calculator-script';
const DEFAULT_DESMOS_API_KEY = 'desmos';

let desmosScriptPromise = null;

function parseNumeric(value) {
    if (value === null || value === undefined || value === '') return null;
    const parsed = Number(value);
    return Number.isFinite(parsed) ? parsed : null;
}

function normalizeExpression(entry, index) {
    if (typeof entry === 'string') {
        return {
            id: `expr-${index + 1}`,
            latex: entry,
        };
    }

    if (entry && typeof entry === 'object' && typeof entry.latex === 'string') {
        return {
            id: entry.id || `expr-${index + 1}`,
            latex: entry.latex,
            color: entry.color,
            hidden: typeof entry.hidden === 'boolean' ? entry.hidden : undefined,
        };
    }

    return null;
}

function normalizeDesmosPayload(rawSource) {
    const trimmedSource = typeof rawSource === 'string' ? rawSource.trim() : '';
    if (!trimmedSource) {
        return {
            expressions: [],
            bounds: null,
            settings: {},
        };
    }

    let parsedJson = null;
    try {
        parsedJson = JSON.parse(trimmedSource);
    } catch {
        parsedJson = null;
    }

    if (parsedJson) {
        const rawExpressions = Array.isArray(parsedJson)
            ? parsedJson
            : Array.isArray(parsedJson.expressions)
                ? parsedJson.expressions
                : typeof parsedJson.latex === 'string'
                    ? [{ latex: parsedJson.latex }]
                    : [];

        const expressions = rawExpressions
            .map((entry, index) => normalizeExpression(entry, index))
            .filter(Boolean);

        const sourceBounds = parsedJson.bounds || parsedJson.viewport || parsedJson;
        const left = parseNumeric(sourceBounds.left ?? sourceBounds.xMin);
        const right = parseNumeric(sourceBounds.right ?? sourceBounds.xMax);
        const bottom = parseNumeric(sourceBounds.bottom ?? sourceBounds.yMin);
        const top = parseNumeric(sourceBounds.top ?? sourceBounds.yMax);
        const bounds =
            left !== null && right !== null && bottom !== null && top !== null
                ? { left, right, bottom, top }
                : null;

        const settings = {};
        ['showGrid', 'showXAxis', 'showYAxis', 'degreeMode', 'projectorMode'].forEach((key) => {
            if (typeof parsedJson[key] === 'boolean') {
                settings[key] = parsedJson[key];
            }
        });

        return { expressions, bounds, settings };
    }

    const expressions = trimmedSource
        .split(/\r?\n/)
        .map((line) => line.trim())
        .filter(Boolean)
        .map((latex, index) => ({
            id: `expr-${index + 1}`,
            latex,
        }));

    return { expressions, bounds: null, settings: {} };
}

function loadDesmosLibrary() {
    if (typeof window === 'undefined') {
        return Promise.reject(new Error('Window object is unavailable'));
    }

    if (window.Desmos) {
        return Promise.resolve(window.Desmos);
    }

    if (desmosScriptPromise) {
        return desmosScriptPromise;
    }

    const apiKey = import.meta.env.VITE_DESMOS_API_KEY || DEFAULT_DESMOS_API_KEY;
    const scriptSrc = `https://www.desmos.com/api/v1.11/calculator.js?apiKey=${encodeURIComponent(apiKey)}`;

    desmosScriptPromise = new Promise((resolve, reject) => {
        const existing = document.getElementById(DESMOS_SCRIPT_ID);
        if (existing) {
            existing.addEventListener('load', () => {
                if (window.Desmos) resolve(window.Desmos);
            });
            existing.addEventListener('error', () => {
                reject(new Error('Failed to load Desmos script'));
            });
            return;
        }

        const script = document.createElement('script');
        script.id = DESMOS_SCRIPT_ID;
        script.async = true;
        script.src = scriptSrc;
        script.onload = () => {
            if (window.Desmos) {
                resolve(window.Desmos);
                return;
            }
            reject(new Error('Desmos library not found after script load'));
        };
        script.onerror = () => {
            reject(new Error('Failed to load Desmos script'));
        };
        document.head.appendChild(script);
    });

    return desmosScriptPromise;
}

function DesmosGraphBlock({ source }) {
    const graphRef = useRef(null);
    const calculatorRef = useRef(null);
    const [error, setError] = useState(null);
    const payload = useMemo(() => normalizeDesmosPayload(source), [source]);

    useEffect(() => {
        let cancelled = false;

        const mountGraph = async () => {
            if (!graphRef.current) return;

            if (!payload.expressions.length) {
                setError('표현식이 없습니다. desmos 코드 블록에 수식을 입력해주세요.');
                return;
            }

            try {
                const Desmos = await loadDesmosLibrary();
                if (cancelled || !graphRef.current) return;

                if (!calculatorRef.current) {
                    calculatorRef.current = Desmos.GraphingCalculator(graphRef.current, {
                        keypad: false,
                        expressionsCollapsed: true,
                        settingsMenu: true,
                        zoomButtons: true,
                    });
                }

                const calculator = calculatorRef.current;
                calculator.setBlank();
                calculator.updateSettings(payload.settings);

                payload.expressions.forEach((expression, index) => {
                    calculator.setExpression({
                        id: expression.id || `expr-${index + 1}`,
                        latex: expression.latex,
                        color: expression.color,
                        hidden: expression.hidden,
                    });
                });

                if (payload.bounds) {
                    calculator.setMathBounds(payload.bounds);
                }

                setError(null);
            } catch (mountError) {
                if (cancelled) return;
                console.error('Failed to render Desmos graph:', mountError);
                setError('Desmos 그래프를 불러오지 못했습니다.');
            }
        };

        mountGraph();

        return () => {
            cancelled = true;
        };
    }, [payload]);

    useEffect(() => {
        return () => {
            if (calculatorRef.current) {
                calculatorRef.current.destroy();
                calculatorRef.current = null;
            }
        };
    }, []);

    return (
        <div className="desmos-graph-block" data-desmos-block>
            <div className="desmos-graph-header">Desmos Graph</div>
            <div ref={graphRef} className="desmos-graph-canvas" />
            {error && <p className="desmos-graph-error">{error}</p>}
        </div>
    );
}

export default DesmosGraphBlock;
