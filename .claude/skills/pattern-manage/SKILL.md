---
name: "pattern-manage"
description: "AgentDB pattern lifecycle management: list, get, delete, deprecate, update, stats, search, duplicates. Use when cleaning up stale patterns, removing deprecated entries, finding duplicates, or auditing pattern health. Workaround for missing MCP delete/update tools (GH Issue #42)."
---

# Pattern Manage - AgentDB Pattern Lifecycle

## What This Skill Does

Provides CRUD operations on AgentDB's `reasoning_patterns` table that are missing from the MCP tool surface. AgentDB exposes `agentdb_pattern_store` (create) and `agentdb_pattern_search` (read) via MCP, but has **no delete, update, or deprecate** tools despite the controller code existing (GH Issue #42).

This skill wraps direct SQLite operations via agentdb's bundled sql.js.

---

## Quick Reference

All commands use the helper script at `scripts/pattern-manage.js`:

```bash
# List all patterns
node .claude/skills/pattern-manage/scripts/pattern-manage.js list

# Filter by category
node .claude/skills/pattern-manage/scripts/pattern-manage.js list --type adr:

# Get full details for a pattern
node .claude/skills/pattern-manage/scripts/pattern-manage.js get 17

# Delete patterns (cascades to embeddings)
node .claude/skills/pattern-manage/scripts/pattern-manage.js delete 17 18 19

# Soft-delete: set success_rate=0 so pattern ranks last in searches
node .claude/skills/pattern-manage/scripts/pattern-manage.js deprecate 29 32

# Update specific fields
node .claude/skills/pattern-manage/scripts/pattern-manage.js update 5 --success_rate 0.8
node .claude/skills/pattern-manage/scripts/pattern-manage.js update 5 --tags '["updated","v2"]'
node .claude/skills/pattern-manage/scripts/pattern-manage.js update 5 --task_type "procedure:add-stream-v2"

# Category breakdown + health checks
node .claude/skills/pattern-manage/scripts/pattern-manage.js stats

# Text search (not semantic — searches task_type and approach text)
node .claude/skills/pattern-manage/scripts/pattern-manage.js search "domain adapter"

# Find patterns sharing the same task_type
node .claude/skills/pattern-manage/scripts/pattern-manage.js duplicates
```

---

## Commands

### list

List all patterns with ID, success rate, uses, type, and preview.

```bash
node .claude/skills/pattern-manage/scripts/pattern-manage.js list
node .claude/skills/pattern-manage/scripts/pattern-manage.js list --type adr:ops-007
node .claude/skills/pattern-manage/scripts/pattern-manage.js list --min-rate 0.9
node .claude/skills/pattern-manage/scripts/pattern-manage.js list --type procedure: --min-rate 0.8
```

### get

Show full details for a single pattern including complete approach text.

```bash
node .claude/skills/pattern-manage/scripts/pattern-manage.js get 17
```

### delete

Hard delete patterns. Cascade-deletes associated embeddings via FK constraint.

```bash
node .claude/skills/pattern-manage/scripts/pattern-manage.js delete 17
node .claude/skills/pattern-manage/scripts/pattern-manage.js delete 17 18 19 20
```

**Use when**: Pattern is completely wrong, references deleted code, or is a duplicate that should not exist.

### deprecate

Soft delete — sets `success_rate=0` and `avg_reward=0`. Pattern still exists but ranks last in similarity searches.

```bash
node .claude/skills/pattern-manage/scripts/pattern-manage.js deprecate 29
node .claude/skills/pattern-manage/scripts/pattern-manage.js deprecate 29 32
```

**Use when**: Pattern is outdated but you want to keep it as a historical record. Prefer this over hard delete when the pattern was once valid.

### update

Update specific fields on a pattern.

```bash
# Fix success rate
node .claude/skills/pattern-manage/scripts/pattern-manage.js update 5 --success_rate 0.8

# Fix category
node .claude/skills/pattern-manage/scripts/pattern-manage.js update 5 --task_type "procedure:add-stream-v2"

# Update tags
node .claude/skills/pattern-manage/scripts/pattern-manage.js update 5 --tags '["updated","v2","streams"]'

# Rewrite approach text
node .claude/skills/pattern-manage/scripts/pattern-manage.js update 5 --approach "New description..."

# Multiple fields at once
node .claude/skills/pattern-manage/scripts/pattern-manage.js update 5 --success_rate 0.95 --tags '["proven"]'
```

**Updatable fields**: `task_type`, `approach`, `success_rate`, `tags`, `metadata`, `uses`, `avg_reward`

### stats

Category breakdown, embedding coverage, health checks.

```bash
node .claude/skills/pattern-manage/scripts/pattern-manage.js stats
```

Reports:
- Total patterns, episodes, embeddings, RL sessions, causal edges
- Patterns grouped by category prefix with average success rate
- Health checks: deprecated count, missing embeddings, duplicate task_types

### search

Text search across `task_type` and `approach` columns. This is a SQL LIKE search, not semantic — use `get-pattern` for semantic similarity search.

```bash
node .claude/skills/pattern-manage/scripts/pattern-manage.js search "domain adapter"
node .claude/skills/pattern-manage/scripts/pattern-manage.js search ops-007
```

### duplicates

Find patterns that share the same `task_type`. Useful for cleanup after multiple agents stored overlapping patterns.

```bash
node .claude/skills/pattern-manage/scripts/pattern-manage.js duplicates
```

---

## When to Use Each Operation

| Situation | Command | Why |
|-----------|---------|-----|
| Pattern references deleted code paths | `delete` | Hard remove — wrong information is worse than none |
| Pattern superseded by v2 | `deprecate` old, keep new | Historical record, but won't pollute searches |
| Multiple patterns with same task_type | `duplicates` then `delete` extras | Clean up agent-created duplication |
| Pattern success rate needs correction | `update --success_rate` | Reflect actual experience |
| Auditing pattern health | `stats` | Find missing embeddings, duplicates, deprecated |
| Finding patterns by keyword | `search` | Quick text lookup when you know the term |
| Full pattern review | `list` then `get <id>` | Inspect before deciding to delete/deprecate |

---

## Relationship to Other Skills

| Skill | Operation | Tool |
|-------|-----------|------|
| `/save-pattern` | **Create** | MCP `agentdb_pattern_store` |
| `/get-pattern` | **Read** (semantic search) | MCP `agentdb_pattern_search` |
| `/reflexion` | **Feedback** on patterns | MCP `reflexion_store` |
| `/pattern-manage` | **Update, Delete, Deprecate, Stats** | Direct SQLite (this skill) |

Together these four skills provide full CRUD + feedback on the patterns table.

---

## Database Details

- **Table**: `reasoning_patterns` (not `patterns` — the MCP tools abstract this)
- **Embeddings**: `pattern_embeddings` (FK cascade from `reasoning_patterns.id`)
- **DB path**: Auto-detected by walking up from cwd; override with `AGENTDB_PATH` env var
- **Engine**: sql.js (WASM) bundled with agentdb — no native sqlite3 needed
- **FK enforcement**: Script runs `PRAGMA foreign_keys = ON` before every operation

---

## Troubleshooting

### "Cannot find sql.js"
agentdb must be installed globally: `npm i -g agentdb`

### "agentdb.db not found"
Run from the project root, or set `AGENTDB_PATH=/path/to/agentdb.db`

### Delete didn't remove embeddings
Verify FK cascade is working: `node -e "..."` with `PRAGMA foreign_keys = ON`. The script does this automatically, but if you used raw SQL elsewhere, FKs are off by default in SQLite.

### Pattern still appears in MCP searches after delete
The MCP server may cache results. Restart the agentdb MCP server to clear its in-memory HNSW index.
