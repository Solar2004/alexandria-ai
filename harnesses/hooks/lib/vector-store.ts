/**
 * lib/vector-store.ts — almacenamiento vectorial de sesiones/devdocs.
 * Reconstruida (2026-08-25) sobre better-sqlite3 (ya era dependencia).
 *
 * Esquema: una fila por chunk indexado; el embedding se guarda como JSON.
 * La búsqueda es similitud coseno en memoria — con los volúmenes de un
 * proyecto (<10k chunks) es instantáneo y no necesita extensión sqlite-vec.
 */

import Database from 'better-sqlite3';
import { mkdirSync } from 'fs';
import { dirname } from 'path';

export interface VectorSearchResult {
    sessionId: string;
    sourceType: 'session' | 'devdoc';
    chunkType: string;
    chunkText: string;
    score: number;
}

export interface SearchOptions {
    limit?: number;
    minScore?: number;
    sourceType?: 'session' | 'devdoc';
}

export class VectorStore {
    private db: Database.Database;

    constructor(dbPath: string) {
        mkdirSync(dirname(dbPath), { recursive: true });
        this.db = new Database(dbPath);
        this.db.pragma('journal_mode = WAL');
        this.db.exec(`
            CREATE TABLE IF NOT EXISTS vectors (
                id TEXT PRIMARY KEY,
                source_type TEXT NOT NULL,
                chunk_type TEXT NOT NULL,
                content TEXT NOT NULL,
                embedding TEXT NOT NULL,
                updated_at REAL NOT NULL
            )
        `);
    }

    /** Inserta o actualiza un chunk con su embedding. */
    upsertEmbedding(
        id: string,
        sourceType: 'session' | 'devdoc',
        chunkType: string,
        content: string,
        embedding: number[],
    ): void {
        this.db
            .prepare(`
                INSERT INTO vectors (id, source_type, chunk_type, content, embedding, updated_at)
                VALUES (?, ?, ?, ?, ?, ?)
                ON CONFLICT(id) DO UPDATE SET
                    source_type = excluded.source_type,
                    chunk_type = excluded.chunk_type,
                    content = excluded.content,
                    embedding = excluded.embedding,
                    updated_at = excluded.updated_at
            `)
            .run(id, sourceType, chunkType, content, JSON.stringify(embedding), Date.now());
    }

    /** Búsqueda por similitud coseno. Nunca lanza por filas corruptas. */
    search(embedding: number[], opts: SearchOptions = {}): VectorSearchResult[] {
        const limit = opts.limit ?? 5;
        const minScore = opts.minScore ?? 0.0;
        const rows = (opts.sourceType
            ? this.db.prepare('SELECT * FROM vectors WHERE source_type = ?').all(opts.sourceType)
            : this.db.prepare('SELECT * FROM vectors').all()) as Array<{
            id: string;
            source_type: string;
            chunk_type: string;
            content: string;
            embedding: string;
        }>;

        const results: VectorSearchResult[] = [];
        for (const row of rows) {
            try {
                const other = JSON.parse(row.embedding) as number[];
                const score = cosine(embedding, other);
                if (score >= minScore) {
                    results.push({
                        sessionId: row.id,
                        sourceType: row.source_type as 'session' | 'devdoc',
                        chunkType: row.chunk_type,
                        chunkText: row.content,
                        score,
                    });
                }
            } catch {
                continue;
            }
        }
        results.sort((a, b) => b.score - a.score);
        return results.slice(0, limit);
    }

    close(): void {
        try {
            this.db.close();
        } catch {
            // ya cerrada
        }
    }
}

function cosine(a: number[], b: number[]): number {
    const n = Math.min(a.length, b.length);
    let dot = 0;
    let na = 0;
    let nb = 0;
    for (let i = 0; i < n; i++) {
        const ai = a[i] ?? 0;
        const bi = b[i] ?? 0;
        dot += ai * bi;
        na += ai * ai;
        nb += bi * bi;
    }
    if (na === 0 || nb === 0) return 0;
    return dot / (Math.sqrt(na) * Math.sqrt(nb));
}
