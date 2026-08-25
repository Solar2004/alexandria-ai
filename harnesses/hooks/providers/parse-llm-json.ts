/**
 * providers/parse-llm-json.ts — extrae el primer objeto JSON de una respuesta
 * LLM. Reconstruida (2026-08-25).
 *
 * Los LLM envuelven JSON en fences, lo preceden con "Aquí tienes..." o lo
 * pisan con texto después. Esto tolera todo eso sin depender de json5.
 */

export function parseLLMJson(text: string): Record<string, unknown> | null {
    if (!text || typeof text !== 'string') return null;

    // 1) ¿JSON puro?
    const direct = tryParse(text.trim());
    if (direct) return direct;

    // 2) Bloque con fence ```json ... ``` (o ``` sin lenguaje)
    const fence = text.match(/```(?:json)?\s*([\s\S]*?)```/i);
    if (fence?.[1]) {
        const parsed = tryParse(fence[1].trim());
        if (parsed) return parsed;
    }

    // 3) Primer {...} balanceado en el texto libre
    const start = text.indexOf('{');
    if (start === -1) return null;
    let depth = 0;
    let inString = false;
    let escaped = false;
    for (let i = start; i < text.length; i++) {
        const ch = text[i];
        if (escaped) { escaped = false; continue; }
        if (ch === '\\') { escaped = true; continue; }
        if (ch === '"') { inString = !inString; continue; }
        if (inString) continue;
        if (ch === '{') depth++;
        else if (ch === '}') {
            depth--;
            if (depth === 0) {
                const parsed = tryParse(text.slice(start, i + 1));
                if (parsed) return parsed;
                break; // primer bloque roto: no insistir más
            }
        }
    }
    return null;
}

function tryParse(raw: string): Record<string, unknown> | null {
    try {
        const obj = JSON.parse(raw);
        return obj && typeof obj === 'object' && !Array.isArray(obj)
            ? (obj as Record<string, unknown>)
            : null;
    } catch {
        return null;
    }
}
