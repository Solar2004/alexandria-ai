/**
 * lib/embeddings.ts — proveedor de embeddings para la inteligencia de sesión.
 * Reconstruida (2026-08-25).
 *
 * Estrategia en dos niveles:
 *   1. Si hay proveedor IA disponible (misma fábrica que clasificación),
 *      intenta su .embed().
 *   2. Si no, embedding local determinista por hashing de tokens (dim 256).
 *      Es léxico, no semántico — suficiente para "sesiones parecidas" y no
 *      requiere red ni claves. El sistema NUNCA se queda sin embeddings.
 */

import type { AIProvider } from '../providers/ai-provider.js';
import { createProvider } from '../providers/provider-factory.js';

export interface EmbeddingProvider {
    readonly name: string;
    readonly dim: number;
    embed(text: string): Promise<number[]>;
}

const HASH_DIM = 256;

export function createEmbeddingProvider(): EmbeddingProvider {
    // Nota: deliberadamente síncrono en la fábrica (el contrato de los hooks
    // llama createEmbeddingProvider() sin await); el coste de red va en embed().
    let aiProvider: AIProvider | null = null;
    let tried = false;

    async function ensureAI(): Promise<AIProvider | null> {
        if (!tried) {
            tried = true;
            try {
                aiProvider = await createProvider();
            } catch {
                aiProvider = null;
            }
        }
        return aiProvider;
    }

    return {
        name: 'hash-fallback',
        dim: HASH_DIM,
        async embed(text: string): Promise<number[]> {
            const ai = await ensureAI();
            if (ai?.embed) {
                try {
                    const v = await ai.embed(text);
                    if (Array.isArray(v) && v.length > 0) return v;
                } catch {
                    // caída al hash local
                }
            }
            return hashEmbed(text);
        },
    };
}

/** Bag-of-words con hashing (feature hashing). Determinista y sin dependencias. */
export function hashEmbed(text: string, dim: number = HASH_DIM): number[] {
    const vec = new Array(dim).fill(0);
    const tokens = text.toLowerCase().match(/[\p{L}\p{N}_]{2,}/gu) ?? [];
    for (const tok of tokens) {
        const h = fnv1a(tok);
        const idx = h % dim;
        const sign = (h >>> 31) & 1 ? -1 : 1;
        vec[idx] += sign;
    }
    // normalización L2: solo importa la dirección para coseno
    const norm = Math.sqrt(vec.reduce((s, v) => s + v * v, 0));
    if (norm > 0) {
        for (let i = 0; i < dim; i++) vec[i] /= norm;
    }
    return vec;
}

function fnv1a(s: string): number {
    let h = 0x811c9dc5;
    for (let i = 0; i < s.length; i++) {
        h ^= s.charCodeAt(i);
        h = Math.imul(h, 0x01000193) >>> 0;
    }
    return h;
}
