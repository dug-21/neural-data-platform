#!/usr/bin/env node
/**
 * pattern-manage.js — Direct SQLite operations on AgentDB's reasoning_patterns table.
 *
 * Workaround for missing MCP delete/update tools (GH Issue #42).
 * Uses agentdb's bundled sql.js (WASM) — no native sqlite3 dependency.
 *
 * Usage:
 *   node pattern-manage.js list [--type <prefix>] [--min-rate <0-1>]
 *   node pattern-manage.js get <id>
 *   node pattern-manage.js delete <id> [<id2> ...]
 *   node pattern-manage.js deprecate <id> [<id2> ...]
 *   node pattern-manage.js update <id> --field <value> [--field2 <value2>]
 *   node pattern-manage.js stats
 *   node pattern-manage.js search <query>     (text search, not semantic)
 *   node pattern-manage.js duplicates
 */

'use strict';

const fs = require('fs');
const path = require('path');

// Resolve agentdb's bundled sql.js — works without native sqlite3
const SQL_JS_PATHS = [
  // Global npm install (nvm)
  path.join(process.env.HOME || '', '.nvm/versions/node', process.version, 'lib/node_modules/agentdb/node_modules/sql.js'),
  // Global npm install (standard)
  '/usr/local/lib/node_modules/agentdb/node_modules/sql.js',
  // nvm with explicit path
  '/usr/local/share/nvm/versions/node/v24.12.0/lib/node_modules/agentdb/node_modules/sql.js',
  // Project-local
  path.join(process.cwd(), 'node_modules/sql.js'),
  // Standalone sql.js
  'sql.js',
];

// Find agentdb.db — walk up from cwd to find it
function findDb() {
  // Explicit env override
  if (process.env.AGENTDB_PATH) return process.env.AGENTDB_PATH;

  let dir = process.cwd();
  while (dir !== path.dirname(dir)) {
    const candidate = path.join(dir, 'agentdb.db');
    if (fs.existsSync(candidate)) return candidate;
    dir = path.dirname(dir);
  }
  return null;
}

function resolveSqlJs() {
  for (const p of SQL_JS_PATHS) {
    try { return require(p); } catch (_) { /* next */ }
  }
  console.error('ERROR: Cannot find sql.js. Install agentdb globally: npm i -g agentdb');
  process.exit(1);
}

async function openDb() {
  const initSqlJs = resolveSqlJs();
  const SQL = await initSqlJs();
  const dbPath = findDb();
  if (!dbPath || !fs.existsSync(dbPath)) {
    console.error(`ERROR: agentdb.db not found. Set AGENTDB_PATH or run from project root.`);
    process.exit(1);
  }
  const db = new SQL.Database(fs.readFileSync(dbPath));
  db.run('PRAGMA foreign_keys = ON');
  return { db, dbPath };
}

function saveDb(db, dbPath) {
  fs.writeFileSync(dbPath, Buffer.from(db.export()));
}

// --- Commands ---

async function cmdList(args) {
  const { db } = await openDb();
  let sql = 'SELECT id, task_type, substr(approach, 1, 80) as preview, success_rate, uses, tags FROM reasoning_patterns';
  const conditions = [];
  const params = [];

  const typeIdx = args.indexOf('--type');
  if (typeIdx !== -1 && args[typeIdx + 1]) {
    conditions.push('task_type LIKE ?');
    params.push(args[typeIdx + 1] + '%');
  }

  const rateIdx = args.indexOf('--min-rate');
  if (rateIdx !== -1 && args[rateIdx + 1]) {
    conditions.push('success_rate >= ?');
    params.push(parseFloat(args[rateIdx + 1]));
  }

  if (conditions.length) sql += ' WHERE ' + conditions.join(' AND ');
  sql += ' ORDER BY id';

  const stmt = db.prepare(sql);
  if (params.length) stmt.bind(params);

  console.log('ID  | Success | Uses | Type                                    | Preview');
  console.log('----|---------|------|----------------------------------------|--------');
  while (stmt.step()) {
    const r = stmt.getAsObject();
    const id = String(r.id).padStart(3);
    const rate = (r.success_rate * 100).toFixed(0).padStart(5) + '%';
    const uses = String(r.uses).padStart(4);
    const type = (r.task_type || '').padEnd(40).slice(0, 40);
    const preview = (r.preview || '').replace(/\n/g, ' ');
    console.log(`${id} | ${rate} | ${uses} | ${type}| ${preview}`);
  }
  stmt.free();

  const count = db.exec('SELECT COUNT(*) FROM reasoning_patterns');
  console.log(`\nTotal: ${count[0].values[0][0]} patterns`);
  db.close();
}

async function cmdGet(args) {
  const id = parseInt(args[0]);
  if (!id) { console.error('Usage: pattern-manage get <id>'); process.exit(1); }

  const { db } = await openDb();
  const stmt = db.prepare('SELECT * FROM reasoning_patterns WHERE id = ?');
  stmt.bind([id]);

  if (stmt.step()) {
    const r = stmt.getAsObject();
    console.log(`ID:           ${r.id}`);
    console.log(`Type:         ${r.task_type}`);
    console.log(`Success Rate: ${(r.success_rate * 100).toFixed(0)}%`);
    console.log(`Uses:         ${r.uses}`);
    console.log(`Avg Reward:   ${r.avg_reward}`);
    console.log(`Tags:         ${r.tags || '(none)'}`);
    console.log(`Metadata:     ${r.metadata || '(none)'}`);
    console.log(`Created:      ${new Date(r.ts * 1000).toISOString()}`);
    console.log(`---`);
    console.log(r.approach);
  } else {
    console.error(`Pattern ${id} not found.`);
    process.exit(1);
  }
  stmt.free();
  db.close();
}

async function cmdDelete(args) {
  const ids = args.filter(a => !a.startsWith('-')).map(Number).filter(n => n > 0);
  if (!ids.length) { console.error('Usage: pattern-manage delete <id> [<id2> ...]'); process.exit(1); }

  const { db, dbPath } = await openDb();

  // Show what will be deleted
  for (const id of ids) {
    const r = db.exec(`SELECT id, task_type, substr(approach, 1, 60) FROM reasoning_patterns WHERE id = ${id}`);
    if (r.length && r[0].values.length) {
      const [rid, type, preview] = r[0].values[0];
      console.log(`DELETE: ID ${rid} | ${type} | ${preview}...`);
    } else {
      console.log(`SKIP:   ID ${id} (not found)`);
    }
  }

  const placeholders = ids.map(() => '?').join(',');
  db.run(`DELETE FROM reasoning_patterns WHERE id IN (${placeholders})`, ids);
  const changes = db.getRowsModified();
  saveDb(db, dbPath);
  console.log(`\nDeleted ${changes} pattern(s). Embeddings cascade-deleted via FK.`);
  db.close();
}

async function cmdDeprecate(args) {
  const ids = args.filter(a => !a.startsWith('-')).map(Number).filter(n => n > 0);
  if (!ids.length) { console.error('Usage: pattern-manage deprecate <id> [<id2> ...]'); process.exit(1); }

  const { db, dbPath } = await openDb();

  for (const id of ids) {
    const r = db.exec(`SELECT task_type FROM reasoning_patterns WHERE id = ${id}`);
    if (r.length && r[0].values.length) {
      const type = r[0].values[0][0];
      db.run('UPDATE reasoning_patterns SET success_rate = 0.0, avg_reward = 0.0 WHERE id = ?', [id]);
      console.log(`DEPRECATED: ID ${id} | ${type} (success_rate=0, avg_reward=0)`);
    } else {
      console.log(`SKIP: ID ${id} (not found)`);
    }
  }

  saveDb(db, dbPath);
  db.close();
}

async function cmdUpdate(args) {
  const id = parseInt(args[0]);
  if (!id) { console.error('Usage: pattern-manage update <id> --field <value>'); process.exit(1); }

  const ALLOWED = ['task_type', 'approach', 'success_rate', 'tags', 'metadata', 'uses', 'avg_reward'];
  const updates = [];
  const params = [];

  for (let i = 1; i < args.length; i += 2) {
    const field = args[i].replace(/^--/, '');
    const value = args[i + 1];
    if (!ALLOWED.includes(field)) {
      console.error(`Invalid field: ${field}. Allowed: ${ALLOWED.join(', ')}`);
      process.exit(1);
    }
    if (value === undefined) {
      console.error(`Missing value for --${field}`);
      process.exit(1);
    }
    updates.push(`${field} = ?`);
    params.push(['success_rate', 'avg_reward', 'uses'].includes(field) ? parseFloat(value) : value);
  }

  if (!updates.length) { console.error('No fields to update.'); process.exit(1); }

  const { db, dbPath } = await openDb();
  params.push(id);
  db.run(`UPDATE reasoning_patterns SET ${updates.join(', ')} WHERE id = ?`, params);
  const changes = db.getRowsModified();

  if (changes) {
    saveDb(db, dbPath);
    console.log(`Updated pattern ${id}: ${updates.join(', ')}`);
  } else {
    console.error(`Pattern ${id} not found.`);
    process.exit(1);
  }
  db.close();
}

async function cmdStats() {
  const { db } = await openDb();

  const total = db.exec('SELECT COUNT(*) FROM reasoning_patterns')[0].values[0][0];
  const episodes = db.exec('SELECT COUNT(*) FROM episodes')[0].values[0][0];

  // Category breakdown
  const cats = db.exec(`
    SELECT
      substr(task_type, 1, instr(task_type || ':', ':') - 1) as category,
      COUNT(*) as cnt,
      ROUND(AVG(success_rate), 2) as avg_rate,
      SUM(uses) as total_uses
    FROM reasoning_patterns
    GROUP BY category
    ORDER BY cnt DESC
  `);

  console.log('=== AgentDB Pattern Statistics ===\n');
  console.log(`Patterns:  ${total}`);
  console.log(`Episodes:  ${episodes}`);

  // Embeddings coverage
  const embedded = db.exec('SELECT COUNT(*) FROM pattern_embeddings')[0].values[0][0];
  console.log(`Embeddings: ${embedded}/${total} (${total > 0 ? Math.round(embedded / total * 100) : 0}% coverage)`);

  // Learning sessions
  const sessions = db.exec('SELECT COUNT(*) FROM learning_sessions')[0].values[0][0];
  console.log(`RL Sessions: ${sessions}`);

  // Causal edges
  const edges = db.exec('SELECT COUNT(*) FROM causal_edges')[0].values[0][0];
  console.log(`Causal Edges: ${edges}`);

  console.log('\n--- By Category ---');
  console.log('Category             | Count | Avg Rate | Total Uses');
  console.log('---------------------|-------|----------|----------');
  if (cats.length && cats[0].values.length) {
    for (const [cat, cnt, rate, uses] of cats[0].values) {
      const c = (cat || 'uncategorized').padEnd(21).slice(0, 21);
      const n = String(cnt).padStart(5);
      const r = ((rate || 0) * 100).toFixed(0).padStart(6) + '%';
      const u = String(uses || 0).padStart(10);
      console.log(`${c}| ${n} | ${r} | ${u}`);
    }
  }

  // Health checks
  console.log('\n--- Health Checks ---');

  const deprecated = db.exec('SELECT COUNT(*) FROM reasoning_patterns WHERE success_rate = 0')[0].values[0][0];
  console.log(`Deprecated (rate=0): ${deprecated}`);

  const noEmbed = db.exec(`
    SELECT COUNT(*) FROM reasoning_patterns rp
    LEFT JOIN pattern_embeddings pe ON rp.id = pe.pattern_id
    WHERE pe.pattern_id IS NULL
  `)[0].values[0][0];
  console.log(`Missing embeddings:  ${noEmbed}`);

  const dupes = db.exec(`
    SELECT task_type, COUNT(*) as cnt FROM reasoning_patterns
    GROUP BY task_type HAVING cnt > 1
  `);
  const dupeCount = dupes.length && dupes[0].values.length ? dupes[0].values.length : 0;
  console.log(`Duplicate task_types: ${dupeCount}`);

  if (dupeCount > 0) {
    console.log('\n  Duplicates:');
    for (const [type, cnt] of dupes[0].values) {
      console.log(`    ${type} (${cnt}x)`);
    }
  }

  db.close();
}

async function cmdSearch(args) {
  const query = args.join(' ');
  if (!query) { console.error('Usage: pattern-manage search <query>'); process.exit(1); }

  const { db } = await openDb();
  const stmt = db.prepare(`
    SELECT id, task_type, substr(approach, 1, 80) as preview, success_rate
    FROM reasoning_patterns
    WHERE task_type LIKE ? OR approach LIKE ?
    ORDER BY success_rate DESC, id
  `);
  const like = `%${query}%`;
  stmt.bind([like, like]);

  console.log(`Text search for: "${query}"\n`);
  let count = 0;
  while (stmt.step()) {
    const r = stmt.getAsObject();
    const rate = (r.success_rate * 100).toFixed(0) + '%';
    console.log(`  ID ${r.id} [${rate}] ${r.task_type}`);
    console.log(`    ${r.preview.replace(/\n/g, ' ')}`);
    count++;
  }
  stmt.free();
  console.log(`\n${count} result(s)`);
  db.close();
}

async function cmdDuplicates() {
  const { db } = await openDb();

  const dupes = db.exec(`
    SELECT rp1.id, rp1.task_type, rp1.success_rate, substr(rp1.approach, 1, 60) as preview
    FROM reasoning_patterns rp1
    INNER JOIN (
      SELECT task_type FROM reasoning_patterns GROUP BY task_type HAVING COUNT(*) > 1
    ) d ON rp1.task_type = d.task_type
    ORDER BY rp1.task_type, rp1.id
  `);

  if (!dupes.length || !dupes[0].values.length) {
    console.log('No duplicate task_types found.');
    db.close();
    return;
  }

  console.log('Patterns with duplicate task_types:\n');
  let lastType = '';
  for (const [id, type, rate, preview] of dupes[0].values) {
    if (type !== lastType) {
      console.log(`\n  ${type}:`);
      lastType = type;
    }
    console.log(`    ID ${id} [${(rate * 100).toFixed(0)}%] ${preview.replace(/\n/g, ' ')}`);
  }

  console.log('\nUse `pattern-manage delete <id>` to remove unwanted duplicates.');
  db.close();
}

// --- Main ---

async function main() {
  const [cmd, ...args] = process.argv.slice(2);

  switch (cmd) {
    case 'list':       return cmdList(args);
    case 'get':        return cmdGet(args);
    case 'delete':     return cmdDelete(args);
    case 'deprecate':  return cmdDeprecate(args);
    case 'update':     return cmdUpdate(args);
    case 'stats':      return cmdStats(args);
    case 'search':     return cmdSearch(args);
    case 'duplicates': return cmdDuplicates(args);
    default:
      console.log(`pattern-manage — AgentDB pattern lifecycle management

Commands:
  list [--type <prefix>] [--min-rate <0-1>]   List all patterns (filterable)
  get <id>                                     Show full pattern details
  delete <id> [<id2> ...]                      Delete patterns (cascades embeddings)
  deprecate <id> [<id2> ...]                   Set success_rate=0 (soft delete)
  update <id> --field <value> [...]            Update pattern fields
  stats                                        Category breakdown + health checks
  search <query>                               Text search across type and approach
  duplicates                                   Find patterns with duplicate task_types

Environment:
  AGENTDB_PATH   Override agentdb.db location (default: auto-detect from cwd)

Fields for update:
  task_type, approach, success_rate, tags, metadata, uses, avg_reward

Examples:
  pattern-manage list --type adr:
  pattern-manage get 17
  pattern-manage delete 17 18 19
  pattern-manage deprecate 29 32
  pattern-manage update 5 --success_rate 0.8 --tags '["updated","v2"]'
  pattern-manage stats
  pattern-manage search "domain adapter"
  pattern-manage duplicates`);
  }
}

main().catch(err => { console.error(err.message); process.exit(1); });
