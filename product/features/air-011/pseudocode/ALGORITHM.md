# AIR-011: Eliminate Duplicative Parser Processing - Algorithm Design

## 1. Current Flow Diagram (Wasteful Path)

```
                                   SOURCE MANAGER
                                        |
    +-----------------------------------+------------------------------------+
    |                                                                        |
    v                                                                        v
spawn_source()                                                    run_http_polling_source()
    |                                                                        |
    v                                                                        v
source.start() ---------> spawns polling_loop() ---------> PARSES JSON     loop { fetch_raw_batch() }
    |                           |                              |                     |
    |                           v                              v                     v
    |                   poll_all_sensors()              TimeSeriesPoint      RawDataPoint (no parse)
    |                           |                              |                     |
    |                           v                              v                     v
    |                   parser.parse()                  sender.send(point)    ingestion_sender.send()
    |                           |                              |                     |
    |                           v                              v                     v
    |                   Vec<TimeSeriesPoint>        internal channel         Bronze Layer
    |                           |                    (NEVER CONSUMED!)       (actual storage)
    |                           v
    |                   sender.send(point)
    |                           |
    |                           v
    +---------------> mpsc::Receiver<TimeSeriesPoint>
                           |
                           v
                    ABANDONED! fetch() never called
                    Memory accumulates until lockup
```

### Problem Analysis

**Double Work Path:**
1. `source.start()` spawns `polling_loop()` which:
   - Polls endpoints at `poll_interval`
   - Parses JSON into `Vec<TimeSeriesPoint>` via parser
   - Sends points to internal `mpsc::channel` (capacity: 1000)
   - **NEVER CONSUMED** - `fetch()` is never called by `run_http_polling_source`

2. `run_http_polling_source` has its own loop:
   - Calls `source.fetch_raw_batch()` every 1 second
   - Gets raw JSON without parsing
   - Sends `RawDataPoint` to `ingestion_sender`
   - This is the **actual Bronze layer path**

**Memory Impact:**
- Each parse cycle creates `Vec<TimeSeriesPoint>` (~1000+ points for 100KB JSON)
- Points accumulate in unbounded channel (only bounded by capacity)
- Channel capacity: 1000 points per source
- After hours: millions of orphaned points consume memory
- Pi has 4GB RAM - exhausted after sustained operation

---

## 2. Proposed Flow Diagram (Clean Path)

```
                              SOURCE MANAGER
                                    |
                                    v
                          spawn_source() -> run_http_polling_source()
                                    |
                                    v
                          HttpPollingSource::with_raw_config()
                                    |
                                    |   NO source.start()!
                                    |   NO polling_loop spawn!
                                    |   NO parser invocation!
                                    v
                          loop at poll_interval {
                                    |
                                    v
                          source.fetch_raw_batch()
                                    |
                                    v
                          HTTP GET -> raw JSON
                                    |
                                    v
                          RawDataPoint (no parsing)
                                    |
                                    v
                          ingestion_sender.send()
                                    |
                                    v
                          Bronze Layer (Parquet)
                          }
```

### Key Changes
1. **Remove `source.start()` call** - No background polling_loop
2. **Single control loop** - SourceManager controls polling interval
3. **Direct raw fetch** - Use `fetch_raw_batch()` only
4. **Parser preserved** - Available for future Silver layer ETL

---

## 3. Pseudocode for the Fix

### Option A: Don't Call source.start()

```pseudocode
ALGORITHM: run_http_polling_source_fixed
INPUT: stream_id, config, ingestion_sender, cancel_token
OUTPUT: Result<(), Error>

BEGIN
    // Create source WITHOUT starting background loop
    source ← HttpPollingSource::with_raw_config(
        config,
        dummy_parser,  // Parser never used, can be minimal
        stream_id,
        ndp_id,
        context
    )

    // REMOVED: source.start()  // <-- This was spawning the wasteful loop

    // Control polling ourselves at the correct interval
    interval ← config.poll_interval  // e.g., 60 seconds

    LOOP
        SELECT
            CASE cancel_token.cancelled():
                // REMOVED: source.stop()  // Nothing to stop
                BREAK

            CASE interval.tick():
                // Direct raw fetch - no parsing involved
                raw_points ← source.fetch_raw_batch()

                FOR EACH point IN raw_points DO
                    ingestion_sender.send(point)
                END FOR
        END SELECT
    END LOOP

    RETURN Ok(())
END
```

### Option B: Create RawHttpPollingSource (No Parser Required)

```pseudocode
STRUCT: RawHttpPollingSource
    config: HttpPollingConfig
    client: HttpClient
    stream_id: String
    ndp_id: Option<String>
    context: Option<Value>
    // NO parser field
    // NO internal channel
    // NO is_running flag
    // NO background task

ALGORITHM: RawHttpPollingSource::new
INPUT: config, stream_id, ndp_id, context
OUTPUT: Self

BEGIN
    client ← HttpClient::new(timeout: config.timeout)

    RETURN RawHttpPollingSource {
        config,
        client,
        stream_id,
        ndp_id,
        context
    }
END

ALGORITHM: RawHttpPollingSource::fetch_raw_batch
INPUT: self
OUTPUT: Result<Vec<RawDataPoint>, Error>

BEGIN
    points ← []

    // Parallel fetch from all sensors
    FOR EACH sensor IN config.sensors PARALLEL DO
        MATCH http_get(sensor.url):
            Ok(response) IF response.status.is_success():
                body ← response.text()
                json ← parse_json(body)

                point ← RawDataPoint::new(
                    source_id: generate_source_id(stream_id, "Http"),
                    raw_payload: json
                )

                IF ndp_id IS SOME THEN
                    point ← point.with_ndp_id(ndp_id)
                END IF

                IF context IS SOME THEN
                    point ← point.with_context(context)
                END IF

                points.push(point)

            Err(e):
                log_warn("Failed to fetch from {}: {}", sensor.url, e)
    END FOR

    RETURN Ok(points)
END
```

---

## 4. Decision Matrix: Option A vs Option B

| Criteria | Option A: Skip start() | Option B: RawHttpPollingSource | Winner |
|----------|------------------------|--------------------------------|--------|
| **Lines Changed** | ~10 lines | ~200+ lines | A |
| **Risk Level** | Low (remove code) | Medium (new code) | A |
| **Parser Code** | Preserved (unused) | Not present | A |
| **Memory Overhead** | Has unused mpsc channel | Minimal | B |
| **Constructor Clarity** | Requires dummy parser | Clean, no parser arg | B |
| **API Surface** | No change to trait | New type | A |
| **Testing Impact** | Minimal | New test suite needed | A |
| **Backward Compat** | Full | Would need migration | A |
| **Implementation Time** | 30 minutes | 2-4 hours | A |
| **Future Silver ETL** | Parser available | Need separate ETL source | A |

### Scoring
- Option A: 7/10 wins
- Option B: 3/10 wins

---

## 5. Recommended Approach

**Recommendation: Option A - Skip source.start()**

### Rationale

1. **Minimal Risk**: Removing code is safer than adding code
2. **Immediate Fix**: Can be deployed in single PR
3. **Preserves Parsers**: Parsers remain available for Silver layer ETL
4. **No API Changes**: Existing trait implementations unchanged
5. **Easy Rollback**: If issues arise, simply add back start() call

### Implementation Strategy

```
Phase 1: Remove source.start() calls (this PR)
    - run_http_polling_source: Remove source.start(), source.stop()
    - run_generic_http_polling_source: Remove source.start(), source.stop()
    - Change interval from 1s to config.poll_interval

Phase 2: Archive parsers (future AIR-012)
    - Move parsers to core/archive/parsers/
    - Keep public API for Silver layer ETL
    - Document parser usage for ETL

Phase 3: Clean up HttpPollingSource (future AIR-013)
    - Remove internal channel
    - Remove is_running flag
    - Simplify to pure RawSource implementation
```

---

## 6. Code Snippets: Before/After

### 6.1 run_http_polling_source

**BEFORE (Current - Wasteful):**
```rust
async fn run_http_polling_source(
    stream_id: String,
    _source_id: String,
    config: HttpPollingConfig,
    ingestion_sender: mpsc::Sender<RawDataPoint>,
    cancel_token: CancellationToken,
    ndp_id: Option<String>,
    context: Option<serde_json::Value>,
) -> Result<(), SourceManagerError> {
    info!("Starting HTTP polling source for stream {}", stream_id);

    // Create parser (WASTEFUL - never actually used for Bronze layer)
    let parser_config = ParserConfig {
        parser_type: ParserType::FlatJson,
        // ... parser config ...
    };
    let parser = create_parser_from_config(parser_config)?;

    // DP-004: Create source
    let mut source = HttpPollingSource::with_raw_config(
        config,
        parser,  // <-- Parser passed but raw_fetch doesn't use it
        Some(stream_id.clone()),
        ndp_id.clone(),
        context.clone(),
    )?;

    // PROBLEM: Start spawns background polling_loop that PARSES
    source.start().await?;  // <-- WASTEFUL WORK STARTS HERE

    // Our poll loop - fetches raw, doesn't use parser
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));

    loop {
        tokio::select! {
            _ = cancel_token.cancelled() => {
                source.stop().await?;  // <-- Stops the wasteful loop
                break;
            }
            _ = interval.tick() => {
                // fetch_raw_batch() doesn't use parser - raw JSON only
                match source.fetch_raw_batch().await {
                    Ok(raw_points) => {
                        for raw_point in raw_points {
                            ingestion_sender.send(raw_point).await?;
                        }
                    }
                    Err(e) => warn!("Failed to fetch: {}", e),
                }
            }
        }
    }

    Ok(())
}
```

**AFTER (Fixed - Clean):**
```rust
async fn run_http_polling_source(
    stream_id: String,
    _source_id: String,
    config: HttpPollingConfig,
    ingestion_sender: mpsc::Sender<RawDataPoint>,
    cancel_token: CancellationToken,
    ndp_id: Option<String>,
    context: Option<serde_json::Value>,
) -> Result<(), SourceManagerError> {
    info!("Starting HTTP polling source for stream {}", stream_id);

    // Create minimal parser (unused but required by constructor)
    // TODO: AIR-013 will make parser optional
    let parser_config = ParserConfig {
        parser_type: ParserType::FlatJson,
        location_id_field: "serialno".to_string(),
        default_location_id: None,
        skip_fields: vec![],  // Minimal - not used
        field_mappings: None,
        default_tags: std::collections::HashMap::new(),
        array_config: None,
        column_config: None,
    };
    let parser = create_parser_from_config(parser_config)?;

    // Create source - NO background polling
    let source = HttpPollingSource::with_raw_config(
        config.clone(),
        parser,
        Some(stream_id.clone()),
        ndp_id.clone(),
        context.clone(),
    )?;

    // REMOVED: source.start() - no background loop, no parsing

    // Use configured poll_interval, not hardcoded 1 second
    let poll_interval = config.poll_interval;
    let mut interval = tokio::time::interval(poll_interval);

    loop {
        tokio::select! {
            _ = cancel_token.cancelled() => {
                info!("HTTP polling source for stream {} cancelled", stream_id);
                // REMOVED: source.stop() - nothing to stop
                break;
            }
            _ = interval.tick() => {
                // Direct raw fetch - no parsing involved
                match source.fetch_raw_batch().await {
                    Ok(raw_points) => {
                        for raw_point in raw_points {
                            if let Err(e) = ingestion_sender.send(raw_point).await {
                                error!("Failed to send point: {}", e);
                            }
                        }
                    }
                    Err(e) => warn!("Failed to fetch from HTTP source: {}", e),
                }
            }
        }
    }

    Ok(())
}
```

### 6.2 run_generic_http_polling_source

**BEFORE:**
```rust
async fn run_generic_http_polling_source(
    stream_id: String,
    _source_id: String,
    config: GenericHttpPollingConfig,
    parser_config: ParserConfig,
    ingestion_sender: mpsc::Sender<RawDataPoint>,
    cancel_token: CancellationToken,
    ndp_id: Option<String>,
    context: Option<serde_json::Value>,
) -> Result<(), SourceManagerError> {
    // Create parser (WASTEFUL)
    let parser = create_parser_from_config(parser_config)?;

    let mut source = GenericHttpPollingSource::with_raw_config(
        config,
        parser,
        Some(stream_id.clone()),
        ndp_id.clone(),
        context.clone(),
    )?;

    // PROBLEM: Start spawns background polling_loop
    source.start().await?;

    let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));

    loop {
        tokio::select! {
            _ = cancel_token.cancelled() => {
                source.stop().await?;
                break;
            }
            _ = interval.tick() => {
                match source.fetch_raw_batch().await {
                    // ... send to ingestion channel ...
                }
            }
        }
    }
    Ok(())
}
```

**AFTER:**
```rust
async fn run_generic_http_polling_source(
    stream_id: String,
    _source_id: String,
    config: GenericHttpPollingConfig,
    parser_config: ParserConfig,
    ingestion_sender: mpsc::Sender<RawDataPoint>,
    cancel_token: CancellationToken,
    ndp_id: Option<String>,
    context: Option<serde_json::Value>,
) -> Result<(), SourceManagerError> {
    // Create parser (unused but required by constructor)
    let parser = create_parser_from_config(parser_config)?;

    // Create source - NO background polling
    let source = GenericHttpPollingSource::with_raw_config(
        config.clone(),
        parser,
        Some(stream_id.clone()),
        ndp_id.clone(),
        context.clone(),
    )?;

    // REMOVED: source.start()

    // Use configured poll_interval
    let poll_interval = config.poll_interval;
    let mut interval = tokio::time::interval(poll_interval);

    loop {
        tokio::select! {
            _ = cancel_token.cancelled() => {
                info!("Generic HTTP source for stream {} cancelled", stream_id);
                // REMOVED: source.stop()
                break;
            }
            _ = interval.tick() => {
                match source.fetch_raw_batch().await {
                    Ok(raw_points) => {
                        for raw_point in raw_points {
                            if let Err(e) = ingestion_sender.send(raw_point).await {
                                error!("Failed to send point: {}", e);
                            }
                        }
                    }
                    Err(e) => warn!("Failed to fetch: {}", e),
                }
            }
        }
    }
    Ok(())
}
```

### 6.3 HttpPollingSource Constructor (Future AIR-013)

**Current (Parser Required):**
```rust
pub fn with_raw_config(
    config: HttpPollingConfig,
    parser: Box<dyn Parser + Send + Sync>,  // Required even if unused
    stream_id: Option<String>,
    ndp_id: Option<String>,
    context: Option<serde_json::Value>,
) -> CoreResult<Self> {
    let (sender, receiver) = mpsc::channel(config.buffer_capacity);
    // ... creates internal channel that won't be used
}
```

**Future (Parser Optional for RawSource usage):**
```rust
pub fn for_raw_ingestion(
    config: HttpPollingConfig,
    stream_id: String,
    ndp_id: Option<String>,
    context: Option<serde_json::Value>,
) -> CoreResult<Self> {
    // No parser needed
    // No internal channel needed
    // Minimal memory footprint
    let client = Client::builder()
        .timeout(config.timeout)
        .build()?;

    Ok(Self {
        config,
        client,
        stream_id: Some(stream_id),
        ndp_id,
        context,
        // NO: parser, receiver, sender, is_running
    })
}
```

---

## 7. Impact Analysis

### Files to Modify

| File | Change Type | Risk |
|------|-------------|------|
| `apps/air-quality-app/src/coordinator/source_manager.rs` | Remove start()/stop() calls | Low |
| `apps/air-quality-app/src/coordinator/source_manager.rs` | Use config.poll_interval | Low |

### Files NOT Modified (Preserved for Silver ETL)

| File | Reason |
|------|--------|
| `core/src/sources/http_poll.rs` | Parser logic preserved for ETL |
| `core/src/parsers/` | All parsers preserved |
| `core/src/traits.rs` | Source/RawSource traits unchanged |

### Memory Impact

| Metric | Before | After | Improvement |
|--------|--------|-------|-------------|
| Per-source channel buffer | 1000 TimeSeriesPoints | 0 | 100% |
| Background tasks per source | 2 (polling_loop + our loop) | 1 | 50% |
| Parser CPU cycles per poll | O(n) where n=JSON fields | 0 | 100% |
| TimeSeriesPoint allocations | ~100 per sensor per poll | 0 | 100% |

### Stability Impact

| Metric | Before | After |
|--------|--------|-------|
| Pi uptime | ~4-8 hours | 24+ hours (expected) |
| Memory growth | Linear (unbounded) | Constant |
| CPU utilization | Higher (parsing) | Lower |

---

## 8. Complexity Analysis

### Current (Wasteful)

```
Time Complexity per poll cycle:
    - HTTP fetch: O(S) where S = sensor count
    - JSON parsing: O(F) where F = total fields across responses
    - Channel send: O(P) where P = parsed points
    - Total: O(S * F) per poll cycle

Space Complexity:
    - Parser intermediate: O(F)
    - Channel buffer: O(B) where B = buffer_capacity (1000)
    - Accumulated: O(B * T) where T = time since start (UNBOUNDED!)
```

### After Fix

```
Time Complexity per poll cycle:
    - HTTP fetch: O(S) where S = sensor count
    - JSON validation: O(1) - just serde_json::from_str
    - Channel send: O(S) - one RawDataPoint per sensor
    - Total: O(S) per poll cycle

Space Complexity:
    - No parser intermediate: 0
    - No internal buffer: 0
    - Per-poll temporary: O(S) - Vec<RawDataPoint>
    - Total: O(S) per cycle - BOUNDED
```

---

## 9. Test Plan

### Unit Tests (Existing - Verify No Regression)
```
- test_http_polling_source_fetch_raw_returns_raw_payload
- test_http_polling_source_fetch_raw_batch_multiple_sensors
- test_generic_http_source_fetch_raw_batch_multiple_endpoints
```

### Integration Tests (New)
```
test_source_manager_http_poll_without_start():
    GIVEN: HTTP polling source config
    WHEN: spawn_source() called
    THEN: No background polling_loop spawned
    AND: fetch_raw_batch() works correctly
    AND: No parser code executed
    AND: Memory stable over 1000 poll cycles

test_source_manager_respects_config_poll_interval():
    GIVEN: config with poll_interval = 30 seconds
    WHEN: source running for 2 minutes
    THEN: fetch_raw_batch() called exactly 4 times (not 120)
```

### Acceptance Tests
```
AT-011-001: Pi runs stable for 24 hours
AT-011-002: Memory usage stable (within 10% variance)
AT-011-003: No parser code in flamegraph during ingestion
AT-011-004: CPU utilization reduced
```

---

## 10. Rollback Plan

If issues discovered after deployment:

```rust
// Revert: Re-add source.start() call
source.start().await?;  // <-- Add back if needed

// And update interval back to 1 second
let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));

// And add stop() back
source.stop().await?;
```

Single-line revert for each of the 3 changes.

---

## Summary

The fix is straightforward: **remove `source.start()` calls** from `run_http_polling_source` and `run_generic_http_polling_source`. This eliminates the background polling_loop that parses JSON into TimeSeriesPoints that are never consumed.

**Key insight**: The `RawSource` trait's `fetch_raw_batch()` method was added later (DP-004) and bypasses all parser logic. But the old `source.start()` still spawned the parser-based polling loop. We simply stop calling start().
