# ADR-021-001: Hot-Reload Scope - Sources Only

**Status**: Proposed
**Date**: 2026-02-02
**Decision Makers**: NDP Architecture Team
**Feature**: dp-021 Config Lifecycle & Release Management

---

## Context

dp-021 Phase 4 implements hot-reload capability for configuration changes. The question is: which components should support hot-reload?

### Current Architecture

The air-quality-app has three main component types that consume configuration:

```
Stream Config
     |
     +---> Sources (MQTT, HTTP)
     |       - Topic subscriptions
     |       - Polling intervals
     |       - Authentication
     |
     +---> BronzeSubscriber
     |       - Parquet writer configuration
     |       - WAL settings
     |       - Partitioning scheme
     |
     +---> SilverSubscriber
             - TimescaleDB connection
             - Field mappings
             - DQ rules
```

### Component Ownership Analysis

| Component | State Complexity | Resource Ownership | Reconfiguration Risk |
|-----------|------------------|-------------------|---------------------|
| **MqttSource** | Low (connection handle) | MQTT connection | Low - can disconnect/reconnect |
| **HttpPollingSource** | Low (timer handle) | HTTP client | Low - can cancel/restart timer |
| **BronzeSubscriber** | High (Parquet writer) | Open file handles, WAL state | High - mid-write corruption |
| **SilverSubscriber** | High (DB transaction) | Active DB connection, batch state | High - partial write, inconsistency |

### Coordinator Pattern Required for Subscribers

The current subscriber implementations own their writer state directly:

```rust
// BronzeSubscriber - owns Parquet writer
pub struct BronzeSubscriber {
    writer: ParquetWriter,     // Active file handles
    wal: WriteAheadLog,        // In-progress entries
    current_partition: String, // Open partition
}

// SilverSubscriber - owns DB connection and batch
pub struct SilverSubscriber {
    pool: PgPool,              // Connection pool
    batch: Vec<Row>,           // Uncommitted batch
    field_mappings: Vec<...>,  // Current config
}
```

To hot-reload subscribers, we would need a coordinator pattern:

```rust
// Required architecture for subscriber hot-reload
pub struct SubscriberCoordinator {
    config: Arc<RwLock<StreamConfig>>,
    subscriber: Arc<RwLock<Box<dyn Subscriber>>>,
}

impl SubscriberCoordinator {
    async fn on_config_change(&self, new_config: StreamConfig) {
        // 1. Pause incoming messages
        // 2. Flush current batch
        // 3. Close current writer cleanly
        // 4. Create new subscriber with new config
        // 5. Resume message processing
    }
}
```

This is a significant refactoring effort that extends beyond dp-021 scope.

---

## Decision

**Hot-reload is scoped to Sources only (MQTT and HTTP). Subscribers (Bronze and Silver) continue to require application restart for configuration changes.**

### What This Means

**In Scope - Hot-Reload Supported**:
- MQTT source topic changes
- MQTT broker connection changes
- MQTT authentication changes
- HTTP polling URL changes
- HTTP polling interval changes
- HTTP header/authentication changes
- Source addition/removal

**Out of Scope - Requires Restart**:
- Bronze storage format changes
- Bronze partitioning scheme changes
- Silver target table changes
- Silver field mapping changes
- DQ rule changes
- WAL configuration changes

### Implementation Approach

```rust
// SourceManager with hot-reload
impl SourceManager {
    /// Called when etcd watch detects config change
    pub async fn on_config_change(&self, stream_id: &str, new_config: StreamConfig) {
        // Validate new config
        if let Err(e) = validate_config(&new_config) {
            error!("Invalid config for {}, keeping current: {}", stream_id, e);
            return;
        }

        // Get current sources for this stream
        let current_sources = self.sources.get(stream_id);

        // Diff old vs new source configs
        let (to_add, to_modify, to_remove) = diff_source_configs(
            current_sources.map(|s| &s.config),
            &new_config.sources
        );

        // Remove old sources (graceful disconnect)
        for source_id in to_remove {
            if let Some(source) = self.sources.remove(&source_id) {
                source.graceful_shutdown().await;
            }
        }

        // Modify existing sources (disconnect + reconnect)
        for (source_id, new_source_config) in to_modify {
            if let Some(source) = self.sources.get_mut(&source_id) {
                source.reconfigure(new_source_config).await;
            }
        }

        // Add new sources
        for source_config in to_add {
            let source = create_source(source_config);
            self.sources.insert(source_config.id.clone(), source);
            source.start().await;
        }

        info!("Hot-reloaded sources for stream {}: +{} ~{} -{}",
              stream_id, to_add.len(), to_modify.len(), to_remove.len());
    }
}
```

### MQTT Reconnection Strategy

```rust
impl MqttSource {
    pub async fn reconfigure(&mut self, new_config: MqttSourceConfig) {
        // 1. Unsubscribe from current topics
        for topic in &self.current_topics {
            self.client.unsubscribe(topic).await;
        }

        // 2. Check if broker changed
        let broker_changed = self.config.broker != new_config.broker;

        if broker_changed {
            // 3a. Full disconnect/reconnect for broker change
            self.client.disconnect().await;
            self.client = create_mqtt_client(&new_config).await;
        }

        // 4. Subscribe to new topics
        for topic in &new_config.topics {
            self.client.subscribe(topic, QoS::AtLeastOnce).await;
        }

        self.config = new_config;
        self.current_topics = new_config.topics.clone();
    }
}
```

### HTTP Source Reconfiguration

```rust
impl HttpPollingSource {
    pub async fn reconfigure(&mut self, new_config: HttpSourceConfig) {
        // 1. Cancel current polling timer
        self.polling_handle.cancel();

        // 2. Update config
        self.config = new_config;

        // 3. Start new polling timer
        self.polling_handle = spawn_polling_timer(
            self.config.url.clone(),
            self.config.interval,
            self.sender.clone()
        );
    }
}
```

---

## Consequences

### Positive

1. **Low risk implementation** - Sources have minimal state, safe to reconfigure
2. **Common use case covered** - Most config changes are source-related (topics, URLs)
3. **Clear boundary** - Operators know which changes require restart
4. **Faster iteration** - Source config testing without restart cycles
5. **Foundation for future** - Establishes hot-reload pattern for later expansion

### Negative

1. **Subscriber changes require restart** - DQ rule changes, field mapping changes need restart
2. **Incomplete hot-reload** - Mixed experience (some changes hot, some cold)
3. **Documentation burden** - Must clearly document what requires restart

### Mitigation

| Limitation | Mitigation |
|------------|------------|
| Subscriber restart required | Document in RELEASE-POLICY.md |
| Mixed experience | Log clearly: "Restart required for field_mappings change" |
| Future expansion | Plan subscriber coordinator in dp-024+ |

---

## Alternatives Considered

### Alternative 1: Full Hot-Reload (Sources + Subscribers)

Implement coordinator pattern for all components.

**Rejected because**:
- Significant refactoring effort (3-5 additional days)
- Increases risk of data corruption bugs
- Subscriber changes are less frequent than source changes
- Can be done in future feature (dp-024+)

### Alternative 2: No Hot-Reload

Keep current behavior: all config changes require restart.

**Rejected because**:
- Source changes are common (topic adjustments, interval tuning)
- Restart means missed messages during transition
- Hot-reload is standard expectation for modern systems
- Foundation work already done (etcd watch exists)

### Alternative 3: Hot-Reload for Sources + Bronze Only

Extend scope to include Bronze subscriber (but not Silver).

**Rejected because**:
- Bronze has similar complexity to Silver (writer state)
- Partial subscriber support is confusing
- Better to have clear boundary: all sources vs all subscribers

---

## Future Considerations

### dp-024+: Subscriber Hot-Reload

If subscriber hot-reload becomes necessary, the approach would be:

1. **Introduce SubscriberCoordinator** - Manages subscriber lifecycle
2. **Pause-Flush-Swap pattern** - Safe transition between configs
3. **State handoff** - Transfer in-flight batches to new subscriber
4. **Rollback capability** - Restore old subscriber if new fails

This is substantial work (5-7 days) and should be a separate feature.

### What Triggers Subscriber Restart

The deploy system will detect when subscriber config changes and log appropriately:

```bash
# In deploy.sh apply
if config_requires_restart "$stream_id"; then
    warn "Stream $stream_id has subscriber config changes - restart required"
    RESTART_REQUIRED=true
else
    log "Stream $stream_id has source-only changes - hot-reload"
    schedule_hot_reload "$stream_id"
fi
```

---

## Implementation Notes

### etcd Watch Integration

The existing etcd watch in air-quality-app will be extended:

```rust
// Current: watches for initial config load
// Extended: notifies SourceManager on changes

async fn watch_config(source_manager: Arc<SourceManager>) {
    let mut watch = etcd_client.watch("/streams/*/config").await;

    while let Some(event) = watch.next().await {
        match event {
            WatchEvent::Put { key, value } => {
                let stream_id = extract_stream_id(&key);
                let config: StreamConfig = serde_json::from_slice(&value)?;

                // Notify SourceManager for hot-reload
                source_manager.on_config_change(&stream_id, config).await;
            }
            WatchEvent::Delete { key } => {
                let stream_id = extract_stream_id(&key);
                source_manager.on_stream_removed(&stream_id).await;
            }
        }
    }
}
```

### Manifest Declaration

Hot-reload is indicated in the manifest:

```json
{
  "type": "stream",
  "id": "air-quality",
  "action": "update",
  "reload": "sources"  // Hot-reload sources only
}
```

vs restart:

```json
{
  "type": "stream",
  "id": "air-quality",
  "action": "update",
  "reload": "app"  // Requires app restart
}
```

---

## Related Decisions

- **ADR-021-002**: Schema Migration Approach
- **ADR-021-003**: Release Methodology
- **ADR-020-001**: Extensible Handlers (manifest processing)

---

## References

- `/workspaces/neural-data-platform/product/features/dp-021/SCOPE.md` - Phase 4 requirements
- `/workspaces/neural-data-platform/product/features/dp-016/IMPLEMENTATION-ROADMAP.md` - Hot-reload section

---

*ADR created: 2026-02-02*
*Feature: dp-021 Config Lifecycle & Release Management*
