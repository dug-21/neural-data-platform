# Hot-Reload Feasibility Analysis

**Feature**: dp-016 Config Reload Capability
**Date**: 2026-02-01
**Status**: Analysis Complete

## Executive Summary

The current NDP architecture has **partial infrastructure** for hot-reload but lacks the integration layer to make it functional. The key finding is that:

1. **etcd watch capability EXISTS** in config-client (watch.rs)
2. **Source lifecycle methods EXIST** (start/stop/restart in SourceManager)
3. **Subscriber reconfigure trait EXISTS** but returns `HotReloadNotSupported` by default
4. **Missing**: Integration layer connecting watches to lifecycle management

## Current State Analysis

### 1. etcd Watch Capability

**Location**: `/workspaces/neural-data-platform/config-client/src/watch.rs`

The config-client already supports watching etcd keys for changes:

```rust
// watch.rs:12-18 - WatchHandle creation
pub(crate) async fn new<F>(
    client: Client,
    prefix: &str,
    callback: F,
) -> Result<Self, ConfigError>
where
    F: Fn(String, Option<serde_json::Value>) + Send + Sync + 'static,
```

**Evidence**:
- `watch.rs:24-27`: Uses `WatchOptions::new().with_prefix()` for prefix-based watching
- `watch.rs:40-48`: Handles both `Put` (config changed) and `Delete` (config removed) events
- `watch.rs:77-79`: Provides `cancel()` method for cleanup

**Gap**: The watch capability is **not used** anywhere in the application code. The `ConfigClient::watch()` method (client.rs:147-154) is exposed but never called.

### 2. Source Lifecycle Management

**Location**: `/workspaces/neural-data-platform/apps/air-quality-app/src/coordinator/source_manager.rs`

SourceManager has complete lifecycle methods:

| Method | Line | Purpose |
|--------|------|---------|
| `spawn_source()` | 151-371 | Start a single source |
| `stop_source()` | 944-986 | Stop and cleanup a source |
| `restart_source()` | 1023-1064 | Stop + reload config + spawn |
| `update_sources_for_stream()` | 1066-1098 | Reload all sources for a stream |

**Evidence** (source_manager.rs:1066-1098):
```rust
/// Update sources based on new stream configuration
pub async fn update_sources_for_stream(
    &mut self,
    stream_id: &str,
) -> Result<(), SourceManagerError> {
    // Load new configuration
    let config = self.registry.load_stream(stream_id).await?;

    // Stop existing sources for this stream
    for source_id in source_ids {
        self.stop_source(&source_id).await?;
    }

    // Start new sources
    self.start_sources_for_stream(&config).await?;
}
```

**Gap**: `update_sources_for_stream()` requires **mutable access** to SourceManager, which is held behind `Arc<RwLock<SourceManager>>`. The main.rs has no mechanism to trigger this.

### 3. Subscriber Lifecycle

**Location**: `/workspaces/neural-data-platform/core/src/subscribers/mod.rs`

The Subscriber trait includes a reconfigure method:

```rust
// subscribers/mod.rs:204-210
/// Reconfigure subscriber (hot reload)
async fn reconfigure(&mut self, _config: serde_json::Value) -> Result<(), SubscriberError> {
    Err(SubscriberError::HotReloadNotSupported)
}
```

**Evidence**:
- `subscribers/mod.rs:67-68`: `SubscriberError::HotReloadNotSupported` variant exists
- All subscriber implementations use the default (which returns error)

**Gap**: No subscriber implements `reconfigure()`. The SubscriberCoordinator (coordinator.rs:120-126) only allows registration in `Idle` state:

```rust
// coordinator.rs:121-126
if self.state != CoordinatorState::Idle {
    return Err(SubscriberError::ConfigError(format!(
        "Cannot register subscriber while coordinator is {:?}",
        self.state
    )));
}
```

### 4. SubscriberCoordinator Limitations

**Location**: `/workspaces/neural-data-platform/core/src/subscribers/coordinator.rs`

The coordinator moves subscribers into tasks on `start_all()`:

```rust
// coordinator.rs:181-191
// Drain subscribers and spawn each in its own task
let subscribers = std::mem::take(&mut self.subscribers);
for mut subscriber in subscribers {
    let handle = tokio::spawn(async move { subscriber.start(receiver).await });
    self.running_tasks.push((id, handle));
}
```

**Gap**: Once started, subscribers are **owned by tokio tasks**. There is no mechanism to:
- Send reconfigure messages to running subscribers
- Add new subscribers without restarting
- Remove subscribers without stopping the coordinator

### 5. Application Startup (main.rs)

**Location**: `/workspaces/neural-data-platform/apps/air-quality-app/src/main.rs`

Current startup sequence (lines 257-420):

1. Load config from StreamRegistry (one-time read)
2. Create EventBus via IngestionCoordinator
3. Register subscribers with SubscriberCoordinator
4. Call `subscriber_coordinator.start_all()` (moves subscribers)
5. Call `coordinator.start()` (starts sources)
6. Enter infinite select! loop (no config watch)

**Gap**: No config watch is set up. The system only reads configuration at startup.

## Blockers for Hot-Reload

### Critical Blockers (Must Fix)

| Blocker | Component | Required Change |
|---------|-----------|-----------------|
| No watch integration | main.rs | Add etcd watch loop that detects changes |
| Subscriber ownership | coordinator.rs | Use Arc<Mutex<Subscriber>> or channels for reconfigure |
| No dynamic subscriber add/remove | coordinator.rs | New methods: `add_subscriber()`, `remove_subscriber()` |
| Stream filter immutability | SilverSubscriber | Stream filter is read-only after creation |

### Moderate Blockers (Should Fix)

| Blocker | Component | Required Change |
|---------|-----------|-----------------|
| RwLock contention | SourceManager | Refactor to use message passing for updates |
| Cache invalidation | StreamRegistry | Cache must be cleared on config change |
| No config diffing | ConfigSyncService | Compare old vs new to detect actual changes |

### Minor Blockers (Nice to Have)

| Blocker | Component | Required Change |
|---------|-----------|-----------------|
| No graceful message draining | BronzeSubscriber | Flush buffer before reconfigure |
| No health status during reload | All components | Add "reloading" state to health checks |

## Architectural Changes Required

### Option 1: Restart-Based Reload (Simpler)

1. Set up etcd watch on `/streams/` prefix
2. On change detected:
   - Stop all sources for affected stream
   - Clear registry cache for affected stream
   - Start sources for affected stream (reads new config)
3. Subscribers continue with EventBus (no change needed)

**Pros**: Minimal code changes, sources already support restart
**Cons**: Brief data gap during restart, doesn't handle subscriber config changes

### Option 2: Channel-Based Reconfigure (Complex)

1. Add `mpsc::Sender<ReconfigureMessage>` to each subscriber
2. SubscriberCoordinator holds send ends, passes receive to tasks
3. On config change, send reconfigure message via channel
4. Subscriber applies config without stopping

**Pros**: Zero-downtime, supports all config changes
**Cons**: Significant refactoring, each subscriber needs reconfigure logic

### Recommendation

**Phase 1** (dp-016): Implement Option 1 (restart-based) for sources only
- Low risk, known patterns exist
- Covers most use cases (poll intervals, endpoints, API keys)
- 2-3 days of work

**Phase 2** (future feature): Implement Option 2 for subscribers
- Higher complexity, deferred to later
- Required for Silver table mapping changes
- 1-2 weeks of work

## Code Evidence Summary

| Finding | File | Lines | Status |
|---------|------|-------|--------|
| etcd watch capability | config-client/src/watch.rs | 1-80 | EXISTS, unused |
| ConfigClient.watch() | config-client/src/client.rs | 147-154 | EXISTS, unused |
| source restart method | source_manager.rs | 1023-1064 | EXISTS, not triggered |
| source update method | source_manager.rs | 1066-1098 | EXISTS, not triggered |
| Subscriber.reconfigure() | subscribers/mod.rs | 204-210 | EXISTS, returns error |
| HotReloadNotSupported | subscribers/mod.rs | 67-68 | ERROR variant exists |
| Coordinator state check | coordinator.rs | 121-126 | BLOCKS registration |
| Subscriber ownership move | coordinator.rs | 181-191 | BLOCKS reconfigure |

## Conclusion

Hot-reload is **feasible** with moderate effort for sources (Option 1). The building blocks exist but are not connected. Subscriber hot-reload (Option 2) requires significant refactoring and should be deferred.

The recommended approach for dp-016:

1. Add etcd watch in main.rs for `/streams/` prefix
2. Create a `ConfigChangeHandler` that calls `source_manager.update_sources_for_stream()`
3. Clear StreamRegistry cache on changes
4. Document which config changes require full restart

## Related ADRs

- ADR-012-002: EventBus Architecture
- ADR-003: GitOps Configuration Pattern

## References

- config-client/src/watch.rs - etcd watch implementation
- apps/air-quality-app/src/coordinator/source_manager.rs - source lifecycle
- core/src/subscribers/coordinator.rs - subscriber coordinator
- apps/air-quality-app/src/main.rs - application startup
