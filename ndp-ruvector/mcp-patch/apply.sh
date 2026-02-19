#!/usr/bin/env bash
# Apply RuVector PostgreSQL memory backend patch to claude-flow MCP server.
# Patches the globally installed claude-flow to route memory_store/search/retrieve/delete
# to PostgreSQL when config.json has memory.backend = "postgres".
#
# Prerequisites:
#   - claude-flow installed globally: npm i -g claude-flow
#   - pg driver installed: npm i -g pg  (or: cd $(npm root -g)/claude-flow && npm i pg)
#   - PostgreSQL running with ruvector extension and initialized schema
#   - .claude-flow/config.json with memory.backend = "postgres" and ruvector.* keys
#
# Usage:
#   bash ndp-ruvector/mcp-patch/apply.sh
#
# After applying, restart the MCP server (restart Claude Code or `claude-flow mcp restart`).

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CF_ROOT="$(npm root -g)/claude-flow"
MEMORY_DIR="$CF_ROOT/v3/@claude-flow/cli/dist/src/memory"
TOOLS_DIR="$CF_ROOT/v3/@claude-flow/cli/dist/src/mcp-tools"

# Verify claude-flow is installed
if [ ! -d "$CF_ROOT" ]; then
    echo "ERROR: claude-flow not found at $CF_ROOT"
    echo "Install with: npm i -g claude-flow"
    exit 1
fi

# Verify pg driver
if [ ! -d "$CF_ROOT/node_modules/pg" ]; then
    echo "ERROR: pg driver not found in claude-flow node_modules"
    echo "Install with: cd $CF_ROOT && npm i pg"
    exit 1
fi

echo "Applying RuVector PostgreSQL memory backend patch..."
echo "  claude-flow: $CF_ROOT"
echo "  memory dir:  $MEMORY_DIR"
echo "  tools dir:   $TOOLS_DIR"
echo ""

# 1. Copy postgres-backend.js
cp "$SCRIPT_DIR/postgres-backend.js" "$MEMORY_DIR/postgres-backend.js"
echo "  [1/3] Copied postgres-backend.js"

# 2. Patch memory-tools.js: add backend detection
TOOLS_FILE="$TOOLS_DIR/memory-tools.js"
if grep -q "detectBackend" "$TOOLS_FILE"; then
    echo "  [2/3] memory-tools.js already patched (detectBackend found)"
else
    # Backup
    cp "$TOOLS_FILE" "$TOOLS_FILE.bak"

    # Add backend detection after the first 'import' block
    sed -i '/^import.*from .path.;$/a\
// ── Backend detection (cached) ──────────────────────────────────────\
let _backendName = null;\
function detectBackend() {\
    if (_backendName) return _backendName;\
    try {\
        const configPath = resolve(join(process.cwd(), '"'"'.claude-flow'"'"', '"'"'config.json'"'"'));\
        if (existsSync(configPath)) {\
            const config = JSON.parse(readFileSync(configPath, '"'"'utf-8'"'"'));\
            const v = config.values || config;\
            if (v['"'"'memory.backend'"'"'] === '"'"'postgres'"'"') {\
                _backendName = '"'"'ruvector-postgres'"'"';\
                return _backendName;\
            }\
        }\
    } catch { /* ignore */ }\
    _backendName = '"'"'sql.js + HNSW'"'"';\
    return _backendName;\
}' "$TOOLS_FILE"

    # Replace getMemoryFunctions
    sed -i '/async function getMemoryFunctions/,/^}/c\
async function getMemoryFunctions() {\
    if (detectBackend() === '"'"'ruvector-postgres'"'"') {\
        try {\
            const pg = await import('"'"'../memory/postgres-backend.js'"'"');\
            return {\
                storeEntry: pg.pgStoreEntry,\
                searchEntries: pg.pgSearchEntries,\
                listEntries: pg.pgListEntries,\
                getEntry: pg.pgGetEntry,\
                deleteEntry: pg.pgDeleteEntry,\
                initializeMemoryDatabase: async () => ({ success: true }),\
                checkMemoryInitialization: pg.pgCheckInitialization,\
            };\
        } catch (e) {\
            console.error('"'"'[MCP Memory] postgres-backend load failed:'"'"', e.message);\
            _backendName = '"'"'sql.js + HNSW'"'"';\
        }\
    }\
    const { storeEntry, searchEntries, listEntries, getEntry, deleteEntry, initializeMemoryDatabase, checkMemoryInitialization, } = await import('"'"'../memory/memory-initializer.js'"'"');\
    return { storeEntry, searchEntries, listEntries, getEntry, deleteEntry, initializeMemoryDatabase, checkMemoryInitialization, };\
}' "$TOOLS_FILE"

    # Replace hardcoded backend strings
    sed -i "s/backend: 'sql.js + HNSW'/backend: detectBackend()/g" "$TOOLS_FILE"
    sed -i "s/backend: 'HNSW + sql.js'/backend: detectBackend()/g" "$TOOLS_FILE"

    echo "  [2/3] Patched memory-tools.js (backup: memory-tools.js.bak)"
fi

# 3. Fix embeddings.json ruvector type (boolean → object)
EMB_FILE="${PWD}/.claude-flow/embeddings.json"
if [ -f "$EMB_FILE" ]; then
    if grep -q '"ruvector": true' "$EMB_FILE"; then
        sed -i 's/"ruvector": true/"ruvector": { "enabled": true }/g' "$EMB_FILE"
        echo "  [3/3] Fixed embeddings.json (ruvector: true → { enabled: true })"
    else
        echo "  [3/3] embeddings.json already correct"
    fi
else
    echo "  [3/3] No embeddings.json found (skipped)"
fi

echo ""
echo "Patch applied. Restart the MCP server for changes to take effect."
echo "  Restart Claude Code, or: claude-flow mcp restart"
