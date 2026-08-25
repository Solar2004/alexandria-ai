/**
 * providers/provider-factory.ts — elige un proveedor IA disponible.
 * Reconstruida (2026-08-25).
 *
 * Orden de preferencia:
 *   1. Anthropic (@anthropic-ai/sdk) — si hay ANTHROPIC_API_KEY. Nota: en este
 *      sistema esa clave apunta a la cadena local (headroom→gateway→routatic),
 *      así que la clasificación de skills viaja por la misma ruta que CC y con
 *      failover incluido.
 *   2. OpenAI (openai) — si hay OPENAI_API_KEY.
 *   3. Google Gemini (@google/genai) — si hay GEMINI_API_KEY / GOOGLE_API_KEY.
 *
 * Sin clave utilizable devuelve null (los hooks degradan a keyword matching).
 */

import type { AIProvider } from './ai-provider.js';

export interface CreateProviderOptions {
    /** Avisar por stderr cuando no haya proveedor (ruido controlado). */
    warnIfUnavailable?: boolean;
}

export async function createProvider(
    opts: CreateProviderOptions = {},
): Promise<AIProvider | null> {
    const candidates: Array<() => Promise<AIProvider | null>> = [
        tryAnthropic,
        tryOpenAI,
        tryGemini,
    ];
    for (const make of candidates) {
        try {
            const provider = await make();
            if (provider) return provider;
        } catch {
            // siguiente candidato
        }
    }
    if (opts.warnIfUnavailable) {
        console.error('[provider-factory] sin proveedor IA disponible; usando fallback');
    }
    return null;
}

async function tryAnthropic(): Promise<AIProvider | null> {
    if (!process.env.ANTHROPIC_API_KEY) return null;
    const mod = await import('@anthropic-ai/sdk');
    const Anthropic = mod.default;
    // Enrutado:
    //  - ANTHROPIC_BASE_URL presente (sesiones CC) -> el SDK la usa solo.
    //  - Sin BASE_URL y clave que NO es de Anthropic real (no empieza por
    //    sk-ant-) -> asumimos cadena local y apuntamos al gateway :3460.
    //  - Clave sk-ant-... sin BASE_URL -> api.anthropic.com (comportamiento
    //    por defecto del SDK).
    const opts: ConstructorParameters<typeof Anthropic>[0] = {};
    if (!process.env.ANTHROPIC_BASE_URL && !process.env.ANTHROPIC_API_KEY.startsWith('sk-ant-')) {
        opts.baseURL = process.env.SKILLS_AI_BASE_URL || 'http://127.0.0.1:3460';
    }
    const client = new Anthropic(opts);
    const model = process.env.SKILLS_AI_MODEL || 'claude-haiku-4-5';
    const call = async (prompt: string): Promise<string> => {
        const resp = await client.messages.create({
            model,
            max_tokens: 300,
            messages: [{ role: 'user', content: prompt }],
        });
        const block = resp.content.find(b => b.type === 'text');
        return block && 'text' in block ? block.text : '';
    };
    return {
        name: 'anthropic',
        classifyPrompt: call,
        analyzeEdit: call,
    };
}

async function tryOpenAI(): Promise<AIProvider | null> {
    if (!process.env.OPENAI_API_KEY) return null;
    const mod = await import('openai');
    const OpenAI = mod.default;
    const client = new OpenAI();
    const model = process.env.SKILLS_AI_MODEL || 'gpt-4o-mini';
    const call = async (prompt: string): Promise<string> => {
        const resp = await client.chat.completions.create({
            model,
            max_tokens: 300,
            messages: [{ role: 'user', content: prompt }],
        });
        return resp.choices[0]?.message?.content ?? '';
    };
    return {
        name: 'openai',
        classifyPrompt: call,
        analyzeEdit: call,
    };
}

async function tryGemini(): Promise<AIProvider | null> {
    const key = process.env.GEMINI_API_KEY || process.env.GOOGLE_API_KEY;
    if (!key) return null;
    const mod = await import('@google/genai');
    const GoogleGenAI = mod.GoogleGenAI;
    const client = new GoogleGenAI({ apiKey: key });
    const model = process.env.SKILLS_AI_MODEL_GEMINI || 'gemini-2.0-flash';
    const call = async (prompt: string): Promise<string> => {
        const resp = await client.models.generateContent({
            model,
            contents: prompt,
        });
        return resp.text ?? '';
    };
    return {
        name: 'gemini',
        classifyPrompt: call,
        analyzeEdit: call,
    };
}
