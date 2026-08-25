/**
 * lib/gemini-client.ts — utilidades de sesión inteligente con LLM.
 * Reconstruida (2026-08-25). A pesar del nombre histórico, usa la fábrica de
 * proveedores (anthropic/openai/gemini según claves), no solo Gemini.
 *
 * Ambas funciones son OPCIONALES para los hooks: si fallan devuelven [] / null
 * y el flujo sigue con keyword matching.
 */

import type { AIProvider } from '../providers/ai-provider.js';
import { createProvider } from '../providers/provider-factory.js';
import type { VectorSearchResult } from './vector-store.js';

export interface RelevanceAssessment {
    relevant: boolean;
    score: number; // 0..1
    keyFiles: string[];
    keyDecisions: string[];
    condensedContext: string;
}

let cached: AIProvider | null | undefined;

async function provider(): Promise<AIProvider | null> {
    if (cached === undefined) {
        try {
            cached = await createProvider();
        } catch {
            cached = null;
        }
    }
    return cached;
}

/** Genera 3-5 términos de búsqueda alternativos para el prompt dado. */
export async function generateSearchTerms(prompt: string): Promise<string[]> {
    const p = await provider();
    if (!p) return [];
    try {
        const text = await p.classifyPrompt(
            `Dado este prompt de desarrollo, genera entre 3 y 5 términos de búsqueda cortos ` +
            `(1-3 palabras, en inglés o español) para encontrar sesiones de trabajo previas ` +
            `relacionadas. Responde SOLO JSON: {"terms": ["...", "..."]}\n\nPrompt: ${prompt.slice(0, 500)}`,
        );
        const m = text.match(/\{[\s\S]*\}/);
        if (!m) return [];
        const obj = JSON.parse(m[0]) as { terms?: unknown };
        if (!Array.isArray(obj.terms)) return [];
        return obj.terms.filter((t): t is string => typeof t === 'string').slice(0, 5);
    } catch {
        return [];
    }
}

/** ¿Son estos resultados realmente relevantes para el prompt? Veredicto LLM. */
export async function assessRelevance(
    prompt: string,
    results: VectorSearchResult[],
): Promise<RelevanceAssessment | null> {
    const p = await provider();
    if (!p || results.length === 0) return null;
    try {
        const listing = results
            .map((r, i) => `${i + 1}. [${r.sourceType}] ${r.chunkText.slice(0, 150).replace(/\n/g, ' ')}`)
            .join('\n');
        const text = await p.classifyPrompt(
            `Prompt actual del desarrollador:\n${prompt.slice(0, 400)}\n\n` +
            `Resultados de búsquedas anteriores:\n${listing}\n\n` +
            `¿Hay contexto útil para el prompt actual? Responde SOLO JSON con esta forma exacta:\n` +
            `{"relevant": true|false, "score": 0.0-1.0, "keyFiles": ["ruta.ts"], ` +
            `"keyDecisions": ["decisión breve"], "condensedContext": "resumen de 1-3 frases"}`,
        );
        const m = text.match(/\{[\s\S]*\}/);
        if (!m) return null;
        const obj = JSON.parse(m[0]) as Record<string, unknown>;
        const strArr = (v: unknown): string[] =>
            Array.isArray(v) ? v.filter((x): x is string => typeof x === 'string') : [];
        return {
            relevant: obj.relevant === true,
            score: typeof obj.score === 'number' ? Math.min(1, Math.max(0, obj.score)) : 0.5,
            keyFiles: strArr(obj.keyFiles),
            keyDecisions: strArr(obj.keyDecisions),
            condensedContext: typeof obj.condensedContext === 'string' ? obj.condensedContext : '',
        };
    } catch {
        return null;
    }
}
