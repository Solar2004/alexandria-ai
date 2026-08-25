/**
 * lib/metrics.ts — métricas JSONL de los hooks de skills.
 *
 * Reconstruida (2026-08-25) junto con session-state.ts. Cada evento se añade
 * a .claude/hooks/state/metrics.jsonl con timestamp; nunca lanza (las métricas
 * no pueden tumbar un hook).
 */

import { appendFileSync, mkdirSync } from 'fs';
import { join } from 'path';
import { stateDir } from './session-state.js';

export interface SkillMetric {
    event: 'suggested' | 'activated' | 'blocked' | 'cleared';
    session?: string;
    skill?: string;
    skills?: string[];
    level?: string;
    kind?: string;
    source?: string;
    file?: string;
    ts?: string;
}

export function recordMetric(metric: Omit<SkillMetric, 'ts'>): void {
    try {
        const dir = stateDir();
        mkdirSync(dir, { recursive: true });
        const entry: SkillMetric = { ...metric, ts: new Date().toISOString() };
        appendFileSync(join(dir, 'metrics.jsonl'), JSON.stringify(entry) + '\n');
    } catch (err) {
        if (process.env.DEBUG_SKILLS === '1') {
            console.error('[metrics] no pude registrar:', err);
        }
    }
}
