/**
 * PostgreSQL Memory Backend for claude-flow MCP
 * Uses ruvector extension for native HNSW vector search.
 * Drop-in replacement for sql.js functions when memory.backend === 'postgres'.
 */
import { readFileSync, existsSync } from 'fs';
import { join, resolve } from 'path';
import { generateEmbedding } from './memory-initializer.js';

let pool = null;
let _isPostgres = null;

function getConnectionConfig() {
    const configPath = resolve(join(process.cwd(), '.claude-flow', 'config.json'));
    if (existsSync(configPath)) {
        try {
            const config = JSON.parse(readFileSync(configPath, 'utf-8'));
            const v = config.values || config;
            if (v['ruvector.host']) {
                return {
                    host: v['ruvector.host'],
                    port: parseInt(v['ruvector.port']) || 5432,
                    database: v['ruvector.database'],
                    user: v['ruvector.user'],
                    password: v['ruvector.password'],
                    options: `--search_path=${v['ruvector.schema'] || 'claude_flow'},public`,
                };
            }
        } catch { /* fall through to env vars */ }
    }
    return {
        host: process.env.PGHOST || 'localhost',
        port: parseInt(process.env.PGPORT || '5432'),
        database: process.env.PGDATABASE || 'claude_flow',
        user: process.env.PGUSER || 'claude',
        password: process.env.PGPASSWORD || '',
        options: '--search_path=claude_flow,public',
    };
}

async function getPool() {
    if (pool) return pool;
    const pg = await import('pg');
    const Pool = pg.default?.Pool || pg.Pool;
    pool = new Pool({ ...getConnectionConfig(), max: 5 });
    return pool;
}

function fmtVec(arr) {
    return '[' + arr.join(',') + ']';
}

// ── public: detect backend ──────────────────────────────────────────

export function isPostgresBackend() {
    if (_isPostgres !== null) return _isPostgres;
    const configPath = resolve(join(process.cwd(), '.claude-flow', 'config.json'));
    if (!existsSync(configPath)) { _isPostgres = false; return false; }
    try {
        const config = JSON.parse(readFileSync(configPath, 'utf-8'));
        const v = config.values || config;
        _isPostgres = v['memory.backend'] === 'postgres';
    } catch { _isPostgres = false; }
    return _isPostgres;
}

// ── store ───────────────────────────────────────────────────────────

export async function pgStoreEntry(options) {
    const { key, value, namespace = 'default', generateEmbeddingFlag = true, tags = [], ttl, upsert = false } = options;
    const p = await getPool();
    try {
        let embStr = null, embDim = null, embModel = null;
        if (generateEmbeddingFlag && value.length > 0) {
            const e = await generateEmbedding(value);
            embStr = fmtVec(e.embedding);
            embDim = e.dimensions;
            embModel = e.model;
        }
        const meta = tags.length > 0 ? JSON.stringify({ tags }) : '{}';
        const ttlTs = ttl ? new Date(Date.now() + ttl * 1000).toISOString() : null;

        let id;
        if (upsert) {
            const r = await p.query(
                `SELECT upsert_memory($1, $2, $3::ruvector(384), $4, $5::jsonb, $6::timestamptz) AS id`,
                [key, value, embStr, namespace, meta, ttlTs]
            );
            id = r.rows[0]?.id;
        } else {
            const r = await p.query(
                `INSERT INTO memory_entries (key, value, embedding, namespace, metadata, ttl)
                 VALUES ($1, $2, $3::ruvector(384), $4, $5::jsonb, $6::timestamptz)
                 RETURNING id`,
                [key, value, embStr, namespace, meta, ttlTs]
            );
            id = r.rows[0]?.id;
        }
        return {
            success: true,
            id: id || '',
            embedding: embStr ? { dimensions: embDim, model: embModel } : undefined,
        };
    } catch (error) {
        return { success: false, id: '', error: error instanceof Error ? error.message : String(error) };
    }
}

// ── search ──────────────────────────────────────────────────────────

export async function pgSearchEntries(options) {
    const { query, namespace = 'default', limit = 10, threshold = 0.3 } = options;
    const p = await getPool();
    const t0 = Date.now();
    try {
        const e = await generateEmbedding(query);
        const embStr = fmtVec(e.embedding);
        const nsFilter = namespace === 'all' ? null : namespace;

        const r = await p.query(
            `SELECT * FROM search_memory($1::ruvector(384), $2, $3, $4)`,
            [embStr, nsFilter, limit, threshold]
        );
        return {
            success: true,
            results: r.rows.map(row => ({
                id: String(row.id).substring(0, 12),
                key: row.key,
                content: (row.value || '').substring(0, 60) + ((row.value || '').length > 60 ? '...' : ''),
                score: row.similarity,
                namespace: row.namespace,
            })),
            searchTime: Date.now() - t0,
        };
    } catch (error) {
        return { success: false, results: [], searchTime: Date.now() - t0, error: error instanceof Error ? error.message : String(error) };
    }
}

// ── get ─────────────────────────────────────────────────────────────

export async function pgGetEntry(options) {
    const { key, namespace = 'default' } = options;
    const p = await getPool();
    try {
        const r = await p.query(
            `SELECT id, key, namespace, value, embedding IS NOT NULL AS has_embedding,
                    metadata, created_at, updated_at
             FROM memory_entries WHERE key = $1 AND namespace = $2 LIMIT 1`,
            [key, namespace]
        );
        if (r.rows.length === 0) return { success: true, found: false };
        const row = r.rows[0];
        return {
            success: true,
            found: true,
            entry: {
                id: String(row.id),
                key: row.key,
                namespace: row.namespace,
                content: row.value,
                accessCount: 0,
                createdAt: row.created_at,
                updatedAt: row.updated_at,
                hasEmbedding: row.has_embedding,
                tags: row.metadata?.tags || [],
            },
        };
    } catch (error) {
        return { success: false, found: false, error: error instanceof Error ? error.message : String(error) };
    }
}

// ── delete ──────────────────────────────────────────────────────────

export async function pgDeleteEntry(options) {
    const { key, namespace = 'default' } = options;
    const p = await getPool();
    try {
        const r = await p.query(
            `DELETE FROM memory_entries WHERE key = $1 AND namespace = $2 RETURNING id`,
            [key, namespace]
        );
        const cnt = await p.query(`SELECT COUNT(*)::int AS cnt FROM memory_entries`);
        return {
            success: true,
            deleted: r.rowCount > 0,
            key,
            namespace,
            remainingEntries: cnt.rows[0]?.cnt || 0,
        };
    } catch (error) {
        return { success: false, deleted: false, key, namespace, remainingEntries: 0, error: error instanceof Error ? error.message : String(error) };
    }
}

// ── list ────────────────────────────────────────────────────────────

export async function pgListEntries(options) {
    const { namespace, limit = 20, offset = 0 } = options;
    const p = await getPool();
    try {
        const cParams = namespace ? [namespace] : [];
        const cSql = namespace
            ? `SELECT COUNT(*)::int AS cnt FROM memory_entries WHERE namespace = $1`
            : `SELECT COUNT(*)::int AS cnt FROM memory_entries`;
        const total = (await p.query(cSql, cParams)).rows[0]?.cnt || 0;

        const lParams = namespace ? [namespace, limit, offset] : [limit, offset];
        const lSql = namespace
            ? `SELECT id, key, namespace, value, embedding IS NOT NULL AS has_embedding, metadata, created_at, updated_at
               FROM memory_entries WHERE namespace = $1 ORDER BY updated_at DESC LIMIT $2 OFFSET $3`
            : `SELECT id, key, namespace, value, embedding IS NOT NULL AS has_embedding, metadata, created_at, updated_at
               FROM memory_entries ORDER BY updated_at DESC LIMIT $1 OFFSET $2`;
        const r = await p.query(lSql, lParams);

        return {
            success: true,
            entries: r.rows.map(row => ({
                id: String(row.id).substring(0, 20),
                key: row.key,
                namespace: row.namespace,
                size: (row.value || '').length,
                accessCount: 0,
                createdAt: row.created_at,
                updatedAt: row.updated_at,
                hasEmbedding: row.has_embedding,
            })),
            total,
        };
    } catch (error) {
        return { success: false, entries: [], total: 0, error: error instanceof Error ? error.message : String(error) };
    }
}

// ── check initialization ────────────────────────────────────────────

export async function pgCheckInitialization() {
    try {
        const p = await getPool();
        const r = await p.query(
            `SELECT EXISTS(
                SELECT 1 FROM information_schema.tables
                WHERE table_schema = 'claude_flow' AND table_name = 'memory_entries'
            ) AS initialized`
        );
        return {
            initialized: r.rows[0]?.initialized || false,
            version: '3.0.0',
            backend: 'ruvector-postgres',
            features: { vectorEmbeddings: true, patternLearning: true, temporalDecay: true },
            tables: ['memory_entries', 'embeddings', 'patterns', 'trajectories'],
        };
    } catch (error) {
        return { initialized: false, error: error instanceof Error ? error.message : String(error) };
    }
}
