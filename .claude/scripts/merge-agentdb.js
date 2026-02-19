#!/usr/bin/env node
/**
 * Merge orphaned agentdb.db from core/ndp-mcp-server/ into active agentdb.db at project root.
 * Deduplicates by checking first 100 chars of approach text.
 */
const initSqlJs = require('/usr/local/share/nvm/versions/node/v24.12.0/lib/node_modules/agentdb/node_modules/sql.js');
const fs = require('fs');

const OLD_PATH = '/workspaces/neural-data-platform/core/ndp-mcp-server/agentdb.db';
const ACTIVE_PATH = '/workspaces/neural-data-platform/agentdb.db';

async function main() {
  const SQL = await initSqlJs();

  const oldBuf = fs.readFileSync(OLD_PATH);
  const oldDb = new SQL.Database(oldBuf);

  const activeBuf = fs.readFileSync(ACTIVE_PATH);
  const activeDb = new SQL.Database(activeBuf);

  // Collect existing approach prefixes for dedup
  const activeApproaches = new Set();
  const activeResult = activeDb.exec('SELECT approach FROM reasoning_patterns');
  if (activeResult[0]) {
    activeResult[0].values.forEach(v => activeApproaches.add((v[0] || '').substring(0, 100)));
  }
  console.log('Active DB has', activeApproaches.size, 'existing patterns');

  // Get all patterns from old DB
  const oldPatterns = oldDb.exec('SELECT task_type, approach, success_rate, uses, avg_reward, tags, metadata FROM reasoning_patterns');
  if (!oldPatterns[0]) {
    console.log('No patterns in old DB');
    return;
  }

  let imported = 0;
  let skipped = 0;

  const stmt = activeDb.prepare('INSERT INTO reasoning_patterns (ts, task_type, approach, success_rate, uses, avg_reward, tags, metadata) VALUES (?, ?, ?, ?, ?, ?, ?, ?)');

  for (const row of oldPatterns[0].values) {
    const [taskType, approach, successRate, uses, avgReward, tags, metadata] = row;
    const prefix = (approach || '').substring(0, 100);

    if (activeApproaches.has(prefix)) {
      skipped++;
      continue;
    }

    try {
      stmt.bind([Math.floor(Date.now() / 1000), taskType, approach, successRate, uses || 0, avgReward || 0, tags, metadata]);
      stmt.step();
      stmt.reset();
      imported++;
      activeApproaches.add(prefix);
    } catch (e) {
      console.log('Error importing', taskType, ':', e.message);
    }
  }
  stmt.free();

  // Merge episodes
  const oldEpisodes = oldDb.exec('SELECT session_id, task, input, output, critique, reward, success, latency_ms, tokens_used, tags, metadata FROM episodes');
  let episodesImported = 0;
  if (oldEpisodes[0]) {
    const epStmt = activeDb.prepare('INSERT INTO episodes (ts, session_id, task, input, output, critique, reward, success, latency_ms, tokens_used, tags, metadata) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)');
    for (const row of oldEpisodes[0].values) {
      try {
        epStmt.bind([Math.floor(Date.now() / 1000), ...row]);
        epStmt.step();
        epStmt.reset();
        episodesImported++;
      } catch (e) {
        // skip
      }
    }
    epStmt.free();
  }

  // Save merged DB
  const data = activeDb.export();
  fs.writeFileSync(ACTIVE_PATH, Buffer.from(data));

  // Verify
  const check = activeDb.exec('SELECT COUNT(*) FROM reasoning_patterns');
  const checkEp = activeDb.exec('SELECT COUNT(*) FROM episodes');

  console.log('\nResults:');
  console.log('  Patterns imported:', imported);
  console.log('  Patterns skipped (duplicates):', skipped);
  console.log('  Episodes imported:', episodesImported);
  console.log('  Active DB now has:', check[0].values[0][0], 'patterns,', checkEp[0].values[0][0], 'episodes');

  activeDb.close();
  oldDb.close();
}

main().catch(console.error);
