# Claude Flow Hooks Memory Fix

## Problem Summary

Your current hooks configuration is causing memory leaks and errors due to:
1. **Duplicate hooks** executing for the same events
2. **Long timeouts** (up to 300s) without proper cleanup
3. **No error handling** causing orphaned processes
4. **Heavy neural training** in session-end hooks
5. **Missing cleanup mechanisms** for failed operations

## Key Changes Made

### 1. Consolidated Duplicate Hooks
- Merged multiple pre-edit hooks into a single comprehensive hook
- Combined post-command hooks to avoid duplicate processing
- Unified session-end hooks to prevent conflicts

### 2. Reduced Timeouts
- Pre/Post operation hooks: 30s → 10s
- Session-end hooks: 300s → 30s  
- UserPromptSubmit: 60s → 5s

### 3. Added Error Handling
- All commands now use `|| true` to prevent failures from blocking
- Wrapped commands in `bash -c` for better error handling
- Added `2>/dev/null` to suppress error spam

### 4. Memory Management Features
- Added `--cleanup-on-exit true` flag
- Added `--cleanup-on-timeout true` flag
- Added `--memory-limit 50` (MB) constraints
- Added `--ttl 300` (5 min) for temporary data
- Added `--no-persist true` for non-critical operations

### 5. Deferred Training
- Replaced immediate neural training with `--defer-training true`
- Added `queue-training` instead of immediate `neural-train`
- Batch training triggers only on session end with limits
- Maximum 5 epochs instead of 50

### 6. New Cleanup Mechanisms
- Added `PeriodicCleanup` section for automatic memory cleanup
- Session-end now includes `--cleanup-memory true`
- Added `--compact-storage true` for database optimization
- Memory cleanup runs every 5 minutes

### 7. Safety Limits
- Character limit on prompt analysis (1000 chars)
- Batch size limits for training
- Memory-safe training mode
- Conditional training only if pending data exists

## Migration Instructions

1. **Backup current settings:**
   ```bash
   cp .claude/settings.json .claude/settings.backup.json
   ```

2. **Replace with improved version:**
   ```bash
   cp .claude/settings-improved.json .claude/settings.json
   ```

3. **Clear existing memory orphans:**
   ```bash
   npx claude-flow@alpha memory cleanup --all --force
   ```

4. **Restart Claude Code to apply changes**

## Monitoring

After applying these changes, monitor memory usage:

```bash
# Check memory usage
npx claude-flow@alpha memory status

# View active hooks
npx claude-flow@alpha hooks list --active

# Check for orphaned processes
ps aux | grep "claude-flow" | grep -v grep
```

## Additional Recommendations

1. **Consider disabling training temporarily** if memory issues persist:
   ```json
   "CLAUDE_FLOW_TRAINING_ENABLED": "false"
   ```

2. **Reduce hook frequency** by being more selective with matchers

3. **Implement hook rate limiting** to prevent excessive executions

4. **Use environment variables** for memory limits:
   ```bash
   export CLAUDE_FLOW_MAX_MEMORY_MB=100
   export CLAUDE_FLOW_HOOK_TIMEOUT_MS=10000
   ```

5. **Regular maintenance:**
   - Weekly: `npx claude-flow@alpha memory compact`
   - Monthly: `npx claude-flow@alpha training reset --keep-best`

This configuration should eliminate the memory leak issues while maintaining the core training functionality you need.