/**
 * lib/session-state.ts — estado por sesión de los hooks de skills.
 *
 * Reconstruida (2026-08-25): el fichero original nunca llegó al repo y tres
 * hooks registrados (skill-activation-tracker / -prompt, skill-verification-guard)
 * importaban de aquí y morían con ERR_MODULE_NOT_FOUND en silencio (exit 0).
 *
 * Almacenamiento: un JSON por sesión bajo
 *   $CLAUDE_PROJECT_DIR/.claude/hooks/state/session-<id>.json
 * Escritura atómica (tmp+rename) para que dos hooks del mismo evento nunca
 * dejen el fichero a medias.
 */

import { existsSync, mkdirSync, readFileSync, renameSync, writeFileSync } from 'fs';
import { dirname, join } from 'path';

export interface SessionState {
    /** skills que la sesión ya usó (Skill tool o guardrail superado). */
    skills_used: string[];
    /** skills OBLIGATORIAS pendientes de activar antes de editar. */
    mandatory_pending: string[];
    /** sugerencias IA pendientes de revisar en PreToolUse. */
    pretooluse_pending: string[];
    /** histórico de sugerencias hechas por clasificación IA. */
    ai_suggested_skills: string[];
    /** ficheros ya analizados por la IA (para no repetir coste). */
    files_analyzed_by_ai: string[];
    turnCount: number;
    lastUpdateTime: number;
    lastUpdateTurn: number;
}

export function emptySessionState(): SessionState {
    return {
        skills_used: [],
        mandatory_pending: [],
        pretooluse_pending: [],
        ai_suggested_skills: [],
        files_analyzed_by_ai: [],
        turnCount: 0,
        lastUpdateTime: 0,
        lastUpdateTurn: 0,
    };
}

export function stateDir(): string {
    const projectDir = process.env.CLAUDE_PROJECT_DIR || '.';
    return join(projectDir, '.claude', 'hooks', 'state');
}

export function sessionStatePath(sessionId: string): string {
    // saneo: session_id viene de CC (uuid), pero no confiamos en nadie
    const safe = sessionId.replace(/[^a-zA-Z0-9_-]/g, '_').slice(0, 80);
    return join(stateDir(), `session-${safe}.json`);
}

export function loadSessionState(sessionId: string): SessionState {
    const defaults = emptySessionState();
    try {
        const path = sessionStatePath(sessionId);
        if (!existsSync(path)) return defaults;
        const raw = JSON.parse(readFileSync(path, 'utf-8'));
        // mezcla defensiva: un JSON viejo/parcial no rompe los campos nuevos
        return { ...defaults, ...raw };
    } catch {
        return defaults;
    }
}

export function updateSessionState(
    sessionId: string,
    mutate: (state: SessionState) => void,
): SessionState {
    const state = loadSessionState(sessionId);
    mutate(state);
    const path = sessionStatePath(sessionId);
    try {
        mkdirSync(dirname(path), { recursive: true });
        const tmp = `${path}.${process.pid}.tmp`;
        writeFileSync(tmp, JSON.stringify(state, null, 2));
        renameSync(tmp, path);
    } catch (err) {
        if (process.env.DEBUG_SKILLS === '1') {
            console.error('[session-state] no pude persistir:', err);
        }
    }
    return state;
}
