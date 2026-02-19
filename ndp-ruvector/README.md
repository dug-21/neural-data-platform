# RuVector PostgreSQL Setup

This directory contains the Docker configuration for RuVector PostgreSQL with Claude-Flow V3.

## Quick Start

```bash
# Start the container
docker-compose up -d

# Verify it's running
docker-compose ps

# Check RuVector version
docker exec ruvector-postgres psql -U claude -d claude_flow -c "SELECT ruvector_version();"
```

## Connection Details

| Setting | Value |
|---------|-------|
| Host | localhost |
| Port | 5432 |
| Database | claude_flow |
| Username | claude |
| Password | claude-flow-test |
| Schema | claude_flow |

## RuVector Syntax

### Extension Installation
```sql
-- IMPORTANT: Requires explicit version
CREATE EXTENSION IF NOT EXISTS ruvector VERSION '0.1.0';
```

### Vector Type
```sql
-- Use ruvector(384), NOT vector(384)
CREATE TABLE embeddings (
    id UUID PRIMARY KEY,
    embedding ruvector(384)
);
```

### Distance Operators
| Operator | Description |
|----------|-------------|
| `<=>` | Cosine distance |
| `<->` | L2 (Euclidean) distance |
| `<#>` | Negative inner product |

### HNSW Index
```sql
CREATE INDEX idx_embeddings_hnsw
ON embeddings
USING hnsw (embedding ruvector_cosine_ops)
WITH (m = 16, ef_construction = 100);
```

## MCP Memory Backend Patch

claude-flow v3.1.0-alpha.44 ships with sql.js + HNSW as the only memory backend.
The `mcp-patch/` directory contains a drop-in PostgreSQL backend that routes
`memory_store`, `memory_search`, `memory_retrieve`, `memory_delete`, and `memory_list`
MCP tools to PostgreSQL via the ruvector extension.

### Apply the patch

```bash
# Prerequisites: docker running, claude-flow installed, pg driver in claude-flow
bash ndp-ruvector/mcp-patch/apply.sh

# Restart MCP server (restart Claude Code or your terminal)
```

### What it does

- Adds `postgres-backend.js` to the memory subsystem (same interface as sql.js backend)
- Patches `memory-tools.js` to read `memory.backend` from config and dispatch accordingly
- Uses PG stored functions: `upsert_memory()`, `search_memory()` for HNSW vector search
- Embedding generation still uses local ONNX (all-MiniLM-L6-v2, 384-dim)
- Falls back to sql.js if PG is unavailable

### Config required

`.claude-flow/config.json` must have:
```json
{
  "values": {
    "memory.backend": "postgres",
    "ruvector.host": "localhost",
    "ruvector.port": 5432,
    "ruvector.database": "claude_flow",
    "ruvector.user": "claude",
    "ruvector.password": "claude-flow-test",
    "ruvector.schema": "claude_flow"
  }
}
```

## Import from sql.js/JSON

```bash
# Export current Claude-Flow memory
npx claude-flow memory list --format json > memory-export.json

# Import to RuVector PostgreSQL
npx claude-flow ruvector import --input memory-export.json
```

## pgAdmin (Optional)

```bash
docker-compose --profile gui up -d
```

Access at: http://localhost:5050
- Email: admin@claude-flow.local
- Password: admin

## Troubleshooting

### Extension creation fails
Use explicit version: `CREATE EXTENSION ruvector VERSION '0.1.0';`

### Container won't start
```bash
docker-compose logs postgres
docker-compose down -v
docker-compose up -d
```

## Learn More
- [RuVector Docker Hub](https://hub.docker.com/r/ruvnet/ruvector-postgres)
- [Claude-Flow Documentation](https://github.com/ruvnet/claude-flow)
