# SPARC Pseudocode: Neural Data Platform - Air Quality (air-001)

**Document Version**: 1.1.0
**Date**: 2025-12-13
**Revision**: Docker Deployment + Complete AirGradient Fields (29+ sensors)

## Document Overview

This document provides algorithmic specifications for the Air Quality monitoring feature (air-001) of the Neural Data Platform. The pseudocode follows SPARC methodology standards and is designed to be implementation-agnostic while providing clear guidance for Rust implementation.

**Architecture Decisions:**
- Storage: Parquet via Polars (columnar, efficient compression)
- Ingestion: MQTT subscriber + HTTP Local API polling (dual source)
- Forecasting: ruv-FANN models (NHITS, NBEATSx)
- Deployment: Docker containers (multi-arch: amd64/arm64)
- Pattern: Generic traits, domain-specific implementations

---

## 1. Core Trait Definitions

### 1.1 TimeSeriesPoint Trait

```
TRAIT: TimeSeriesPoint
PURPOSE: Generic interface for any time-stamped data point
RATIONALE: Enables storage/querying of any domain (air quality, trading, IoT)

INTERFACE:
    METHOD timestamp() -> DateTime<Utc>
        // Returns the observation timestamp
        // MUST be timezone-aware (UTC)

    METHOD series_name() -> String
        // Returns unique identifier for this series
        // Format: "{domain}/{location}/{metric}"
        // Example: "air-quality/sensor-001/pm25"

    METHOD value() -> f64
        // Returns primary numeric value
        // MUST be finite (reject NaN, Infinity)

    METHOD metadata() -> HashMap<String, Value>
        // Returns additional context
        // Examples: sensor_id, location, calibration_version

    METHOD quality_score() -> f64
        // Returns data quality indicator [0.0, 1.0]
        // Based on: sensor health, validation checks, freshness

IMPLEMENTATION NOTES:
    - All methods MUST be const/pure (no side effects)
    - timestamp() determines sort order in storage
    - series_name() determines partition key
```

### 1.2 TimeSeriesStore Trait

```
TRAIT: TimeSeriesStore<T: TimeSeriesPoint>
PURPOSE: Generic time-series persistence layer
RATIONALE: Abstract storage backend (Parquet, TimescaleDB, InfluxDB)

INTERFACE:
    METHOD append(point: T) -> Result<()>
        // Append single data point
        // MUST handle concurrent writes safely
        // MUST validate point before storing

    METHOD append_batch(points: Vec<T>) -> Result<()>
        // Bulk append for efficiency
        // MUST be transactional (all-or-nothing)
        // MUST maintain sort order by timestamp

    METHOD query_range(
        series: String,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        filters: QueryFilters
    ) -> Result<Vec<T>>
        // Range query with optional filters
        // MUST scan minimal partitions
        // MUST respect memory limits (streaming)

    METHOD aggregate(
        series: String,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        aggregation: AggregationType,
        interval: Duration
    ) -> Result<Vec<AggregatePoint>>
        // Downsample/aggregate data
        // Examples: 1-minute averages, hourly max

    METHOD latest(series: String) -> Result<Option<T>>
        // Get most recent point
        // MUST be O(1) or cached

    METHOD compact() -> Result<CompactionStats>
        // Merge small files, optimize layout
        // SHOULD run periodically (background task)

    METHOD health_check() -> Result<HealthStatus>
        // Verify storage is operational
        // Check: disk space, I/O performance, corruption

COMPLEXITY ANALYSIS:
    - append: O(1) amortized (buffered writes)
    - append_batch: O(n) where n = batch size
    - query_range: O(p * log m) where p = partitions, m = points/partition
    - aggregate: O(p * m) with early aggregation optimization
    - latest: O(1) with LRU cache
    - compact: O(n log n) for sort + merge
```

### 1.3 DataSource Trait

```
TRAIT: DataSource<T: TimeSeriesPoint>
PURPOSE: Generic data ingestion interface
RATIONALE: Support multiple sources (MQTT, HTTP, gRPC, file import)

INTERFACE:
    METHOD connect() -> Result<Connection>
        // Establish connection to data source
        // MUST implement reconnection logic
        // MUST validate credentials/config

    METHOD stream() -> AsyncStream<Result<T>>
        // Continuous data stream
        // MUST handle backpressure
        // MUST implement error recovery

    METHOD validate(point: T) -> Result<ValidationReport>
        // Check point validity
        // Examples: range checks, schema validation

    METHOD health() -> SourceHealth
        // Report source status
        // Metrics: latency, error rate, last success

IMPLEMENTATION NOTES:
    - stream() MUST be cancellation-safe
    - validate() runs before storage
    - reconnect on transient errors (exponential backoff)
```

### 1.4 Validator Trait

```
TRAIT: Validator<T>
PURPOSE: Domain-specific validation logic
RATIONALE: Each domain has unique constraints

INTERFACE:
    METHOD validate(data: &T) -> ValidationResult
        // Comprehensive validation
        // RETURNS: ValidationResult { is_valid, errors, warnings, score }

    METHOD sanitize(data: &mut T) -> SanitizeResult
        // Fix common issues (trim whitespace, clamp ranges)
        // RETURNS: SanitizeResult { modified, changes }

    METHOD quality_score(data: &T) -> f64
        // Calculate quality metric [0.0, 1.0]
        // Based on: completeness, consistency, freshness

VALIDATION CATEGORIES:
    1. Schema validation (required fields, types)
    2. Range validation (physical constraints)
    3. Consistency checks (cross-field relationships)
    4. Temporal checks (timestamp ordering, gaps)
```

### 1.5 AlertChecker Trait

```
TRAIT: AlertChecker<T>
PURPOSE: Threshold monitoring and alerting
RATIONALE: Domain-specific alert conditions

INTERFACE:
    METHOD check_thresholds(point: &T) -> Vec<Alert>
        // Evaluate all configured thresholds
        // RETURNS: List of triggered alerts (empty if none)

    METHOD should_notify(alert: &Alert) -> bool
        // Apply rate limiting, deduplication
        // Prevent alert fatigue

    METHOD priority(alert: &Alert) -> AlertPriority
        // Calculate alert severity
        // Based on: duration, magnitude, trend

ALERT STATES:
    - Triggered: Condition met
    - Active: Still above threshold
    - Resolved: Returned to normal
    - Acknowledged: Human reviewed
```

---

## 2. Parquet Storage Engine

### 2.1 Partition Strategy

```
ALGORITHM: DeterminePartition
INPUT: point (AirQualityReading)
OUTPUT: partition_path (String)

PURPOSE: Calculate file path for a data point
RATIONALE: Daily partitions balance query performance vs. file count

BEGIN
    // Extract date components
    date ← point.timestamp.date()
    year ← date.year
    month ← date.month (zero-padded)
    day ← date.day (zero-padded)

    // Extract series components
    location_id ← point.location_id

    // Build hierarchical path
    // Format: data/{location}/{year}/{month}/{day}.parquet
    partition_path ← FORMAT(
        "data/{}/{}/{}/{}.parquet",
        location_id,
        year,
        month,
        day
    )

    RETURN partition_path
END

EXAMPLES:
    timestamp=2025-12-13T15:30:00Z, location=sensor-001
    → data/sensor-001/2025/12/13.parquet

COMPLEXITY: O(1)
SPACE: O(1)

PARTITION BENEFITS:
    - Time-based queries scan minimal files
    - Easy to expire old data (delete old directories)
    - Parallel writes to different dates
    - Manageable file sizes (~1-10 MB/day per sensor)
```

### 2.2 Append Algorithm

```
ALGORITHM: AppendToParquet
INPUT: point (AirQualityReading)
OUTPUT: Result<()>

PURPOSE: Append data point to appropriate partition
RATIONALE: Buffer writes for efficiency, handle concurrency

CONSTANTS:
    BUFFER_SIZE = 1000 points
    FLUSH_INTERVAL = 60 seconds
    MAX_RETRIES = 3

STATE:
    write_buffer: HashMap<PartitionKey, Vec<AirQualityReading>>
    locks: HashMap<PartitionKey, Mutex>

BEGIN
    // Step 1: Validate point
    validation_result ← Validator.validate(point)
    IF NOT validation_result.is_valid THEN
        LOG warning("Invalid point", validation_result.errors)
        RETURN Error("Validation failed")
    END IF

    // Step 2: Determine partition
    partition_key ← DeterminePartition(point)

    // Step 3: Acquire partition lock
    lock ← locks.get_or_create(partition_key).lock()

    TRY
        // Step 4: Add to buffer
        buffer ← write_buffer.get_or_create(partition_key)
        buffer.append(point)

        // Step 5: Flush if buffer full
        IF buffer.length >= BUFFER_SIZE THEN
            FlushBuffer(partition_key, buffer)
            buffer.clear()
        END IF

    FINALLY
        lock.unlock()
    END TRY

    RETURN Ok()
END

SUBROUTINE: FlushBuffer
INPUT: partition_key (String), buffer (Vec<AirQualityReading>)
OUTPUT: Result<()>

BEGIN
    // Step 1: Sort by timestamp (required for Parquet)
    buffer.sort_by(|a, b| a.timestamp.cmp(b.timestamp))

    // Step 2: Convert to Polars DataFrame
    df ← DataFrame.new([
        Series("timestamp", buffer.map(|p| p.timestamp)),
        Series("pm25", buffer.map(|p| p.pm25)),
        Series("pm10", buffer.map(|p| p.pm10)),
        Series("temperature", buffer.map(|p| p.temperature)),
        Series("humidity", buffer.map(|p| p.humidity)),
        // ... all fields
    ])

    // Step 3: Write to Parquet (append mode)
    file_path ← partition_key

    IF file_exists(file_path) THEN
        // Append to existing file
        existing_df ← read_parquet(file_path)
        merged_df ← concat([existing_df, df])
        merged_df.sort("timestamp")  // Maintain sort order
        write_parquet(merged_df, file_path, compression="snappy")
    ELSE
        // Create new file
        ensure_directory_exists(parent_directory(file_path))
        write_parquet(df, file_path, compression="snappy")
    END IF

    // Step 4: Update metrics
    METRICS.increment("points_written", buffer.length)
    METRICS.increment("flush_operations", 1)

    RETURN Ok()
END

CONCURRENCY NOTES:
    - Per-partition locking allows parallel writes to different dates
    - Buffer reduces file I/O (batching)
    - Sort ensures efficient Parquet compression

COMPLEXITY:
    - Time: O(1) for append, O(n log n) for flush (sort)
    - Space: O(BUFFER_SIZE) per active partition
```

### 2.3 Query Algorithm

```
ALGORITHM: QueryRange
INPUT:
    series (String),
    start (DateTime<Utc>),
    end (DateTime<Utc>),
    filters (QueryFilters)
OUTPUT: Vec<AirQualityReading>

PURPOSE: Retrieve data within time range
RATIONALE: Scan only relevant partitions, filter efficiently

BEGIN
    // Step 1: Determine partitions to scan
    partitions ← CalculatePartitionRange(series, start, end)

    // Example: 2025-12-01 to 2025-12-13 = 13 daily files
    // Partitions = ["data/sensor-001/2025/12/01.parquet", ..., "13.parquet"]

    IF partitions.length > 100 THEN
        LOG warning("Large query range", partitions.length)
        // Consider pagination or aggregation
    END IF

    // Step 2: Parallel scan partitions
    results ← []
    FOR EACH partition IN partitions PARALLEL DO
        IF NOT file_exists(partition) THEN
            CONTINUE
        END IF

        // Step 3: Read Parquet with column/row pruning
        df ← read_parquet(partition,
            columns = ["timestamp", "pm25", "pm10", ...],
            predicate = timestamp >= start AND timestamp <= end
        )

        // Step 4: Apply additional filters
        IF filters.pm25_min IS NOT NULL THEN
            df ← df.filter(col("pm25") >= filters.pm25_min)
        END IF

        IF filters.quality_score_min IS NOT NULL THEN
            df ← df.filter(col("quality_score") >= filters.quality_score_min)
        END IF

        // Step 5: Convert to domain objects
        partition_results ← df.rows().map(|row| AirQualityReading.from_row(row))
        results.extend(partition_results)
    END FOR

    // Step 6: Sort merged results
    results.sort_by(|a, b| a.timestamp.cmp(b.timestamp))

    // Step 7: Limit results (prevent OOM)
    IF results.length > 100_000 THEN
        LOG warning("Large result set", results.length)
        results ← results.slice(0, 100_000)
    END IF

    RETURN results
END

SUBROUTINE: CalculatePartitionRange
INPUT: series (String), start (DateTime), end (DateTime)
OUTPUT: Vec<String>

BEGIN
    partitions ← []
    current_date ← start.date()
    end_date ← end.date()

    WHILE current_date <= end_date DO
        partition_path ← DeterminePartition(
            create_dummy_point(series, current_date)
        )
        partitions.append(partition_path)
        current_date ← current_date + 1 day
    END WHILE

    RETURN partitions
END

OPTIMIZATION NOTES:
    - Parquet column pruning: Read only needed columns
    - Parquet row pruning: Push down timestamp filter
    - Parallel reads: Each partition scanned independently
    - Memory streaming: Process large results in chunks

COMPLEXITY:
    - Time: O(p * m) where p = partitions, m = avg points/partition
    - Space: O(r) where r = result size (bounded to 100k)
    - Parallelism: O(p) concurrent reads
```

### 2.4 Compaction Strategy

```
ALGORITHM: CompactPartitions
INPUT: location_id (String), date_range (DateRange)
OUTPUT: CompactionStats

PURPOSE: Merge small files, optimize storage layout
RATIONALE: Buffered writes create many small files; compact for efficiency

TRIGGER CONDITIONS:
    - Scheduled: Daily at 2 AM (low traffic)
    - On-demand: After bulk import
    - Threshold: >10 files per day per location

BEGIN
    stats ← CompactionStats.new()

    FOR EACH date IN date_range DO
        partition_dir ← FORMAT("data/{}/{}/{}/{}",
            location_id, date.year, date.month, date.day
        )

        IF NOT directory_exists(partition_dir) THEN
            CONTINUE
        END IF

        // Step 1: Find all fragment files
        fragments ← list_files(partition_dir, pattern="*.parquet")

        IF fragments.length <= 1 THEN
            CONTINUE  // Already optimal
        END IF

        // Step 2: Read all fragments
        dataframes ← []
        FOR EACH fragment IN fragments DO
            df ← read_parquet(fragment)
            dataframes.append(df)
        END FOR

        // Step 3: Merge and sort
        merged_df ← concat(dataframes)
        merged_df.sort("timestamp")

        // Step 4: Deduplicate (same timestamp + location)
        merged_df ← merged_df.unique(subset=["timestamp", "location_id"])

        // Step 5: Write optimized file
        output_file ← FORMAT("{}/{}.parquet", partition_dir, date.day)
        write_parquet(merged_df, output_file,
            compression = "snappy",
            row_group_size = 10_000,  // Optimize for query performance
            statistics = true          // Enable min/max stats
        )

        // Step 6: Delete fragments
        FOR EACH fragment IN fragments DO
            delete_file(fragment)
        END FOR

        // Update stats
        stats.files_merged += fragments.length
        stats.files_created += 1
        stats.bytes_before += sum(fragments.map(|f| file_size(f)))
        stats.bytes_after += file_size(output_file)
    END FOR

    stats.compression_ratio ← stats.bytes_before / stats.bytes_after
    RETURN stats
END

COMPACTION BENEFITS:
    - Fewer files → faster queries (less metadata)
    - Better compression (larger row groups)
    - Deduplication removes accidental duplicates
    - Statistics enable predicate pushdown

COMPLEXITY:
    - Time: O(n log n) where n = total points in date
    - Space: O(n) for merged DataFrame
    - Disk I/O: Read all + Write once + Delete fragments
```

---

## 3. MQTT Ingestion Pipeline

### 3.1 Connection Management

```
ALGORITHM: MQTTConnectionManager
INPUT: config (MQTTConfig)
OUTPUT: Connection handle

PURPOSE: Establish and maintain MQTT connection
RATIONALE: Handle network interruptions, reconnect automatically

CONSTANTS:
    INITIAL_RETRY_DELAY = 1 second
    MAX_RETRY_DELAY = 60 seconds
    BACKOFF_MULTIPLIER = 2
    KEEPALIVE_INTERVAL = 30 seconds

STATE:
    current_delay: Duration
    connection_attempts: u32
    last_successful_connect: DateTime<Utc>

BEGIN
    current_delay ← INITIAL_RETRY_DELAY
    connection_attempts ← 0

    LOOP
        TRY
            // Step 1: Create MQTT client options
            mqtt_options ← MqttOptions.new(
                client_id = config.client_id,
                host = config.broker_host,
                port = config.broker_port
            )

            mqtt_options.set_keep_alive(KEEPALIVE_INTERVAL)
            mqtt_options.set_clean_session(false)  // Persistent session

            // Step 2: Set credentials if provided
            IF config.username IS NOT NULL THEN
                mqtt_options.set_credentials(
                    config.username,
                    config.password
                )
            END IF

            // Step 3: Configure TLS if required
            IF config.use_tls THEN
                mqtt_options.set_ca_certificate(config.ca_cert_path)
            END IF

            // Step 4: Create client and event loop
            (client, event_loop) ← AsyncClient.new(mqtt_options, 100)

            // Step 5: Subscribe to topics
            FOR EACH topic IN config.topics DO
                client.subscribe(topic, QoS::AtLeastOnce).await
                LOG info("Subscribed to topic", topic)
            END FOR

            // Step 6: Connection successful
            connection_attempts ← 0
            current_delay ← INITIAL_RETRY_DELAY
            last_successful_connect ← Utc::now()

            LOG info("MQTT connected successfully")
            RETURN (client, event_loop)

        CATCH error AS e
            connection_attempts += 1
            LOG error("MQTT connection failed",
                attempt = connection_attempts,
                error = e,
                next_retry = current_delay
            )

            // Step 7: Exponential backoff
            sleep(current_delay)
            current_delay ← MIN(
                current_delay * BACKOFF_MULTIPLIER,
                MAX_RETRY_DELAY
            )

            // Step 8: Alert on persistent failure
            IF connection_attempts >= 10 THEN
                Alert.send("MQTT connection persistently failing")
            END IF
        END TRY
    END LOOP
END

CONNECTION LIFECYCLE:
    1. Initial connect (clean or resume session)
    2. Subscribe to topics
    3. Process messages (see MessageProcessingLoop)
    4. Heartbeat keepalive
    5. Reconnect on disconnect
    6. Graceful shutdown on termination signal
```

### 3.2 Message Parsing (AirGradient Complete Field Set)

```
ALGORITHM: ParseAirGradientMessage
INPUT: payload (bytes), source (DataSourceType)
OUTPUT: Result<AirQualityReading>

PURPOSE: Convert AirGradient JSON to typed struct
RATIONALE: Validate and sanitize external data from MQTT or Local HTTP API

DATA SOURCES:
    1. MQTT: airgradient/readings/{SERIAL_NUMBER} (12 fields)
    2. Local API: http://airgradient_{SERIAL}.local/measures/current (29+ fields)

COMPLETE JSON FORMAT (Local API - Firmware 3.1.4+):
    {
        // Device Metadata
        "wifi": -46,                    // WiFi signal strength (dBm)
        "serialno": "ecda3b1eaaaf",     // Device serial number
        "boot": 6,                      // Boot count
        "bootCount": 6,                 // Same as boot
        "ledMode": "pm",                // LED display mode
        "firmware": "3.1.4",            // Firmware version
        "model": "I-9PSL",              // Model identifier (I-9PSL = ONE Indoor)

        // CO2 (Senseair S8)
        "rco2": 447,                    // CO2 concentration (ppm)

        // Particulate Matter - Mass Concentration (Plantower PMS5003)
        "pm01": 3,                      // PM1.0 (μg/m³)
        "pm02": 7,                      // PM2.5 (μg/m³)
        "pm10": 8,                      // PM10 (μg/m³)
        "pm02Compensated": 6,           // PM2.5 with humidity compensation

        // Particulate Matter - Standard (atmospheric)
        "pm01Standard": 3,              // PM1.0 standard (μg/m³)
        "pm02Standard": 7,              // PM2.5 standard (μg/m³)
        "pm10Standard": 8,              // PM10 standard (μg/m³)

        // Particle Counts (per 0.1L / deciliter)
        "pm003Count": 442,              // Particles >0.3μm
        "pm005Count": 380,              // Particles >0.5μm
        "pm01Count": 98,                // Particles >1.0μm
        "pm02Count": 12,                // Particles >2.5μm
        "pm50Count": 2,                 // Particles >5.0μm
        "pm10Count": 1,                 // Particles >10μm

        // Temperature & Humidity (Sensirion SHT4x)
        "atmp": 25.87,                  // Temperature raw (°C)
        "atmpCompensated": 24.47,       // Temperature compensated (°C)
        "rhum": 43,                     // Relative humidity raw (%)
        "rhumCompensated": 49,          // Humidity compensated (%)

        // VOC/NOx (Sensirion SGP41)
        "tvocIndex": 100,               // TVOC index (1-500, baseline 100)
        "tvocRaw": 33051,               // TVOC raw sensor value
        "noxIndex": 1,                  // NOx index (1-500, baseline 1)
        "noxRaw": 16307                 // NOx raw sensor value
    }

BEGIN
    // Step 1: Parse JSON
    TRY
        json ← serde_json::from_slice(payload)
    CATCH error AS e
        LOG error("JSON parse failed", error = e, source = source)
        RETURN Error("Invalid JSON")
    END CATCH

    // Step 2: Extract required fields
    serialno ← json.get("serialno").ok_or("Missing serialno")?

    // Step 3: Generate timestamp (Local API doesn't include timestamp)
    timestamp ← IF json.has("timestamp") THEN
        DateTime::parse_from_rfc3339(json.get("timestamp"))?
    ELSE
        Utc::now()  // Use current time for Local API
    END IF

    // Step 4: Validate timestamp (reject future or too old)
    now ← Utc::now()
    IF timestamp > now + 5 minutes THEN
        RETURN Error("Timestamp in future")
    END IF

    IF timestamp < now - 7 days THEN
        LOG warning("Old timestamp", timestamp = timestamp)
        // Allow but flag for quality score
    END IF

    // Step 5: Extract ALL sensor readings (29+ fields)
    reading ← AirQualityReading {
        // Identity
        location_id: serialno,
        timestamp: timestamp,

        // Device Metadata
        wifi_signal: json.get("wifi").as_i8(),
        boot_count: json.get("bootCount").or(json.get("boot")).as_u32(),
        firmware: json.get("firmware").as_string(),
        model: json.get("model").as_string(),
        led_mode: json.get("ledMode").as_string(),

        // CO2
        co2: json.get("rco2").as_u16(),

        // PM Mass Concentration
        pm01: json.get("pm01").as_u16(),
        pm25: json.get("pm02").as_u16(),
        pm10: json.get("pm10").as_u16(),
        pm25_compensated: json.get("pm02Compensated").as_u16(),

        // PM Standard (atmospheric)
        pm01_standard: json.get("pm01Standard").as_u16(),
        pm25_standard: json.get("pm02Standard").as_u16(),
        pm10_standard: json.get("pm10Standard").as_u16(),

        // Particle Counts (per dL)
        pm003_count: json.get("pm003Count").as_u32(),
        pm005_count: json.get("pm005Count").as_u32(),
        pm01_count: json.get("pm01Count").as_u32(),
        pm02_count: json.get("pm02Count").as_u32(),
        pm50_count: json.get("pm50Count").as_u32(),
        pm10_count: json.get("pm10Count").as_u32(),

        // Temperature & Humidity
        temperature: json.get("atmp").as_f32(),
        temperature_compensated: json.get("atmpCompensated").as_f32(),
        humidity: json.get("rhum").as_f32(),
        humidity_compensated: json.get("rhumCompensated").as_f32(),

        // VOC/NOx
        tvoc_index: json.get("tvocIndex").as_u16(),
        tvoc_raw: json.get("tvocRaw").as_u32(),
        nox_index: json.get("noxIndex").as_u16(),
        nox_raw: json.get("noxRaw").as_u32(),

        // Calculated fields (populated after validation)
        aqi: None,
        aqi_category: None,
        quality_score: 0.0,
        data_source: source,
    }

    // Step 6: Validate ranges (see ValidateAirQualityReading)
    validation_result ← ValidateAirQualityReading(reading)
    IF NOT validation_result.is_valid THEN
        RETURN Error(validation_result.errors)
    END IF

    // Step 7: Calculate quality score
    reading.quality_score ← CalculateQualityScore(reading, json)

    RETURN Ok(reading)
END

FIELD AVAILABILITY BY SOURCE:
    | Field              | MQTT | Local API | Notes                    |
    |--------------------|------|-----------|--------------------------|
    | serialno           | ✓    | ✓         | Required                 |
    | rco2               | ✓    | ✓         | CO2 ppm                  |
    | pm01/pm02/pm10     | ✓    | ✓         | Mass concentration       |
    | pm02Compensated    | ✗    | ✓         | Humidity-corrected PM2.5 |
    | pm*Standard        | ✗    | ✓         | Atmospheric standard     |
    | pm*Count (6 sizes) | ✗    | ✓         | Particle counts          |
    | atmp/rhum          | ✓    | ✓         | Raw temp/humidity        |
    | *Compensated       | ✗    | ✓         | Compensated values       |
    | tvocIndex/noxIndex | ✓    | ✓         | Index values             |
    | tvocRaw/noxRaw     | ✗    | ✓         | Raw sensor values        |
    | firmware/model     | ✗    | ✓         | Device metadata          |
    | wifi               | ✓    | ✓         | Signal strength          |

ERROR HANDLING:
    - Invalid JSON → Log + Dead Letter Queue
    - Missing serialno → Reject (required)
    - Missing sensor fields → Store as NULL (optional)
    - Out of range → Warning + clamp or reject
    - Duplicate timestamp → Deduplicate in storage
```

### 3.3 Dual-Source Ingestion

```
ALGORITHM: DualSourceIngestion
PURPOSE: Combine MQTT streaming with Local API polling
RATIONALE: MQTT provides real-time updates, Local API provides full field set

CONSTANTS:
    MQTT_TOPIC = "airgradient/readings/{SERIAL}"
    LOCAL_API_URL = "http://airgradient_{SERIAL}.local/measures/current"
    LOCAL_API_POLL_INTERVAL = 60 seconds  // Don't poll too frequently

STATE:
    mqtt_stream: MQTTSubscription
    local_api_timer: Interval
    last_reading: HashMap<SerialNo, AirQualityReading>
    pending_local_fields: HashMap<SerialNo, PartialReading>

BEGIN
    // Initialize both sources
    mqtt_stream ← MQTTSource.subscribe(MQTT_TOPIC)
    local_api_timer ← Interval::every(LOCAL_API_POLL_INTERVAL)

    LOOP
        SELECT
            // MQTT message received (real-time, 12 fields)
            mqtt_payload ← mqtt_stream.next() =>
                reading ← ParseAirGradientMessage(mqtt_payload, DataSource::MQTT)

                // Merge with pending Local API fields if available
                IF pending_local_fields.contains(reading.serialno) THEN
                    local_fields ← pending_local_fields.remove(reading.serialno)
                    reading ← MergeReadings(reading, local_fields)
                END IF

                ProcessReading(reading)

            // Local API poll timer (full field set)
            _ ← local_api_timer.tick() =>
                FOR EACH sensor IN configured_sensors DO
                    TRY
                        response ← http_client.get(LOCAL_API_URL.replace("{SERIAL}", sensor.serial))
                        reading ← ParseAirGradientMessage(response.body, DataSource::LocalAPI)

                        // Store extra fields for MQTT merge, or process directly
                        IF data_source_mode == "both" THEN
                            pending_local_fields.insert(sensor.serial, reading.extra_fields())
                        ELSE IF data_source_mode == "local_api" THEN
                            ProcessReading(reading)
                        END IF
                    CATCH error AS e
                        LOG warning("Local API poll failed", sensor = sensor.serial, error = e)
                    END CATCH
                END FOR

            // Shutdown signal
            _ ← shutdown_signal =>
                BREAK
        END SELECT
    END LOOP
END

SUBROUTINE: MergeReadings
INPUT: mqtt_reading (AirQualityReading), local_fields (PartialReading)
OUTPUT: AirQualityReading

PURPOSE: Combine MQTT real-time data with Local API extended fields
BEGIN
    merged ← mqtt_reading.clone()

    // Fill in fields only available from Local API
    merged.pm25_compensated ← local_fields.pm25_compensated
    merged.pm01_standard ← local_fields.pm01_standard
    merged.pm25_standard ← local_fields.pm25_standard
    merged.pm10_standard ← local_fields.pm10_standard
    merged.pm003_count ← local_fields.pm003_count
    merged.pm005_count ← local_fields.pm005_count
    merged.pm01_count ← local_fields.pm01_count
    merged.pm02_count ← local_fields.pm02_count
    merged.pm50_count ← local_fields.pm50_count
    merged.pm10_count ← local_fields.pm10_count
    merged.temperature_compensated ← local_fields.temperature_compensated
    merged.humidity_compensated ← local_fields.humidity_compensated
    merged.tvoc_raw ← local_fields.tvoc_raw
    merged.nox_raw ← local_fields.nox_raw
    merged.firmware ← local_fields.firmware
    merged.model ← local_fields.model

    RETURN merged
END
```

### 3.3 Backpressure Handling

```
ALGORITHM: MessageProcessingLoop
INPUT: event_loop (MQTTEventLoop)
OUTPUT: Never (runs until shutdown)

PURPOSE: Process MQTT messages with backpressure control
RATIONALE: Prevent memory exhaustion during traffic spikes

CONSTANTS:
    MAX_PENDING_MESSAGES = 10_000
    BATCH_SIZE = 100
    PROCESSING_TIMEOUT = 5 seconds

STATE:
    pending_queue: BoundedQueue<MQTTMessage>
    storage_buffer: Vec<AirQualityReading>
    metrics: IngestionMetrics

BEGIN
    LOOP
        // Step 1: Poll next MQTT event
        event ← event_loop.poll().await

        MATCH event
            Event::Incoming(Packet::Publish(publish)) =>
                // Step 2: Check queue capacity
                IF pending_queue.length >= MAX_PENDING_MESSAGES THEN
                    LOG warning("Backpressure: dropping message")
                    METRICS.increment("messages_dropped")
                    CONTINUE  // Drop message
                END IF

                // Step 3: Enqueue message
                pending_queue.push(publish)
                METRICS.increment("messages_received")

                // Step 4: Process batch if ready
                IF pending_queue.length >= BATCH_SIZE THEN
                    ProcessBatch()
                END IF

            Event::Incoming(Packet::ConnAck) =>
                LOG info("MQTT connection acknowledged")
                METRICS.set("connected", 1)

            Event::Outgoing(_) =>
                // Ignore outgoing packets
                CONTINUE

            Event::Disconnect =>
                LOG warning("MQTT disconnected")
                METRICS.set("connected", 0)
                // Connection manager will reconnect
        END MATCH
    END LOOP
END

SUBROUTINE: ProcessBatch
BEGIN
    // Step 1: Dequeue messages
    batch ← []
    WHILE batch.length < BATCH_SIZE AND NOT pending_queue.is_empty DO
        batch.append(pending_queue.pop())
    END WHILE

    // Step 2: Parse messages in parallel
    parsed_readings ← []
    FOR EACH message IN batch PARALLEL DO
        result ← ParseAirGradientMessage(message.payload)

        MATCH result
            Ok(reading) =>
                parsed_readings.append(reading)
            Error(e) =>
                LOG error("Parse failed", error = e)
                DeadLetterQueue.send(message, e)
                METRICS.increment("parse_errors")
        END MATCH
    END FOR

    // Step 3: Validate batch
    valid_readings ← []
    FOR EACH reading IN parsed_readings DO
        validation ← ValidateAirQualityReading(reading)

        IF validation.is_valid THEN
            valid_readings.append(reading)
        ELSE IF validation.can_sanitize THEN
            sanitized ← SanitizeReading(reading, validation)
            valid_readings.append(sanitized)
            METRICS.increment("readings_sanitized")
        ELSE
            LOG error("Validation failed", errors = validation.errors)
            METRICS.increment("validation_errors")
        END IF
    END FOR

    // Step 4: Store batch
    TRY
        storage.append_batch(valid_readings).await
        METRICS.increment("readings_stored", valid_readings.length)
    CATCH error AS e
        LOG error("Storage failed", error = e)
        METRICS.increment("storage_errors")
        // Retry with exponential backoff
    END CATCH

    // Step 5: Check alerts (async, non-blocking)
    spawn_task(CheckAlerts(valid_readings))
END

BACKPRESSURE MECHANISMS:
    1. Bounded queue (drop on overflow)
    2. Batch processing (reduce overhead)
    3. Parallel parsing (utilize CPU)
    4. Async storage (don't block event loop)
    5. Metrics (observe performance)
```

### 3.4 Error Recovery

```
ALGORITHM: ErrorRecoveryStrategy
INPUT: error (Error), context (ErrorContext)
OUTPUT: RecoveryAction

PURPOSE: Determine how to handle different error types
RATIONALE: Some errors are transient, others require intervention

ERROR CATEGORIES:
    1. Transient: Network timeouts, temporary unavailability
    2. Validation: Bad data format, out-of-range values
    3. Fatal: Invalid config, authentication failure
    4. Resource: Disk full, memory exhausted

BEGIN
    MATCH error.kind
        NetworkTimeout | ConnectionReset =>
            // Transient network issue
            RETURN RecoveryAction.Retry(
                delay = exponential_backoff(context.retry_count),
                max_retries = 5
            )

        InvalidJSON | SchemaViolation =>
            // Bad data - don't retry
            DeadLetterQueue.send(context.message, error)
            RETURN RecoveryAction.Skip

        OutOfRange | ValidationFailed =>
            // Try to sanitize
            IF can_sanitize(context.data, error) THEN
                RETURN RecoveryAction.Sanitize
            ELSE
                DeadLetterQueue.send(context.message, error)
                RETURN RecoveryAction.Skip
            END IF

        AuthenticationFailed | InvalidConfig =>
            // Fatal - cannot continue
            Alert.send_critical("Fatal MQTT error", error)
            RETURN RecoveryAction.Shutdown

        DiskFull | OutOfMemory =>
            // Resource exhaustion
            Alert.send_critical("Resource exhausted", error)
            RETURN RecoveryAction.Pause(duration = 60 seconds)

        _ =>
            // Unknown error - be conservative
            LOG error("Unknown error type", error)
            RETURN RecoveryAction.Retry(
                delay = 10 seconds,
                max_retries = 3
            )
    END MATCH
END

DEAD LETTER QUEUE STRUCTURE:
    {
        "original_message": bytes,
        "error": String,
        "timestamp": DateTime<Utc>,
        "retry_count": u32,
        "context": {
            "topic": String,
            "qos": u8,
            "retain": bool
        }
    }

DLQ PROCESSING:
    - Store in separate Parquet partition (dlq/{date}.parquet)
    - Periodic review (manual or automated)
    - Reprocess after fix (e.g., schema update)
```

---

## 4. Air Quality Domain Types

### 4.1 AirQualityReading Struct

```
STRUCT: AirQualityReading
PURPOSE: Domain-specific implementation of TimeSeriesPoint
RATIONALE: Capture all AirGradient sensor data + calculated fields

FIELDS:
    // Identity
    location_id: String              // Sensor serial number or location ID
    timestamp: DateTime<Utc>         // Observation time

    // Particulate Matter (μg/m³)
    pm01: Option<f64>                // PM1.0 (particles ≤1.0 μm)
    pm25: Option<f64>                // PM2.5 (particles ≤2.5 μm)
    pm10: Option<f64>                // PM10 (particles ≤10 μm)
    particle_count: Option<u32>      // Ultra-fine particles (0.3-1.0 μm)

    // Gases
    co2: Option<u32>                 // CO₂ concentration (ppm)
    tvoc: Option<u16>                // Total Volatile Organic Compounds (ppb)
    nox_index: Option<u16>           // NOx index (0-500)

    // Environmental
    temperature: Option<f64>         // Celsius
    humidity: Option<u8>             // Relative humidity (%)

    // Metadata
    wifi_signal: Option<i8>          // WiFi signal strength (dBm)
    quality_score: f64               // Data quality [0.0, 1.0]

    // Calculated (derived from raw data)
    aqi: Option<u16>                 // Air Quality Index (EPA)
    aqi_category: Option<AQICategory>
    health_risk: Option<HealthRisk>

TRAIT IMPLEMENTATIONS:
    TimeSeriesPoint =>
        timestamp() -> self.timestamp
        series_name() -> format!("air-quality/{}/pm25", self.location_id)
        value() -> self.pm25.unwrap_or(f64::NAN)
        metadata() -> HashMap with all optional fields
        quality_score() -> self.quality_score

    Serialize/Deserialize => serde JSON/Parquet

    Clone => Deep copy

    Debug => Formatted output

INVARIANTS:
    - timestamp MUST be valid UTC datetime
    - At least ONE pollutant field MUST be Some
    - quality_score in range [0.0, 1.0]
    - All numeric values MUST be finite (not NaN/Infinity)
```

### 4.2 AQI Calculation Algorithm

```
ALGORITHM: CalculateAQI
INPUT: reading (AirQualityReading)
OUTPUT: (aqi: u16, category: AQICategory, pollutant: String)

PURPOSE: Calculate EPA Air Quality Index
RATIONALE: Standardized health risk communication

REFERENCE: EPA AQI Breakpoints (40 CFR Part 58 Appendix G)

AQI BREAKPOINTS (PM2.5 example):
    Category        | PM2.5 (μg/m³) | AQI Range | Color
    ----------------|---------------|-----------|-------
    Good            | 0.0 - 12.0    | 0 - 50    | Green
    Moderate        | 12.1 - 35.4   | 51 - 100  | Yellow
    Unhealthy (SG)  | 35.5 - 55.4   | 101 - 150 | Orange
    Unhealthy       | 55.5 - 150.4  | 151 - 200 | Red
    Very Unhealthy  | 150.5 - 250.4 | 201 - 300 | Purple
    Hazardous       | 250.5 - 500.4 | 301 - 500 | Maroon

BEGIN
    // Step 1: Calculate AQI for each pollutant
    aqis ← []

    IF reading.pm25 IS NOT NULL THEN
        aqi_pm25 ← CalculatePollutantAQI(
            concentration = reading.pm25,
            breakpoints = PM25_BREAKPOINTS
        )
        aqis.append(("PM2.5", aqi_pm25))
    END IF

    IF reading.pm10 IS NOT NULL THEN
        aqi_pm10 ← CalculatePollutantAQI(
            concentration = reading.pm10,
            breakpoints = PM10_BREAKPOINTS
        )
        aqis.append(("PM10", aqi_pm10))
    END IF

    IF reading.co2 IS NOT NULL THEN
        // Note: CO₂ not in EPA AQI, use custom scale
        aqi_co2 ← CalculateCO2Index(reading.co2)
        aqis.append(("CO2", aqi_co2))
    END IF

    IF reading.tvoc IS NOT NULL THEN
        aqi_tvoc ← CalculateTVOCIndex(reading.tvoc)
        aqis.append(("TVOC", aqi_tvoc))
    END IF

    // Step 2: Overall AQI is maximum (worst pollutant)
    IF aqis.is_empty THEN
        RETURN (aqi: 0, category: AQICategory.Unknown, pollutant: "None")
    END IF

    (max_pollutant, max_aqi) ← aqis.max_by(|(_, aqi)| aqi)

    // Step 3: Determine category
    category ← MATCH max_aqi
        0..=50 => AQICategory.Good
        51..=100 => AQICategory.Moderate
        101..=150 => AQICategory.UnhealthyForSensitive
        151..=200 => AQICategory.Unhealthy
        201..=300 => AQICategory.VeryUnhealthy
        301.. => AQICategory.Hazardous
    END MATCH

    RETURN (aqi: max_aqi, category: category, pollutant: max_pollutant)
END

SUBROUTINE: CalculatePollutantAQI
INPUT: concentration (f64), breakpoints (Vec<Breakpoint>)
OUTPUT: aqi (u16)

PURPOSE: Linear interpolation between AQI breakpoints
FORMULA: AQI = ((AQI_hi - AQI_lo) / (C_hi - C_lo)) * (C - C_lo) + AQI_lo

BEGIN
    // Find applicable breakpoint range
    FOR EACH breakpoint IN breakpoints DO
        IF concentration >= breakpoint.c_lo AND concentration <= breakpoint.c_hi THEN
            // Linear interpolation
            aqi ← (
                (breakpoint.aqi_hi - breakpoint.aqi_lo) /
                (breakpoint.c_hi - breakpoint.c_lo)
            ) * (concentration - breakpoint.c_lo) + breakpoint.aqi_lo

            RETURN round(aqi) AS u16
        END IF
    END FOR

    // Concentration exceeds highest breakpoint
    IF concentration > breakpoints.last().c_hi THEN
        RETURN 500  // Hazardous (beyond scale)
    END IF

    // Concentration below lowest breakpoint
    RETURN 0
END

COMPLEXITY: O(k) where k = number of breakpoints (constant, ~6)
SPACE: O(1)

HEALTH IMPLICATIONS MAPPING:
    Good (0-50):
        - Air quality satisfactory
        - Little to no health risk
        - Actions: None needed

    Moderate (51-100):
        - Acceptable air quality
        - Sensitive individuals may experience minor effects
        - Actions: Sensitive groups should consider reducing prolonged outdoor exertion

    Unhealthy for Sensitive (101-150):
        - General public not likely affected
        - Sensitive groups experience health effects
        - Actions: Sensitive groups should reduce prolonged outdoor exertion

    Unhealthy (151-200):
        - Everyone may begin to experience health effects
        - Sensitive groups experience more serious effects
        - Actions: Everyone should reduce prolonged outdoor exertion

    Very Unhealthy (201-300):
        - Health alert
        - Everyone likely to be affected
        - Actions: Everyone should avoid prolonged outdoor exertion

    Hazardous (301+):
        - Health emergency
        - Entire population likely affected
        - Actions: Everyone should avoid all outdoor exertion
```

### 4.3 Validation Rules

```
ALGORITHM: ValidateAirQualityReading
INPUT: reading (AirQualityReading)
OUTPUT: ValidationResult

PURPOSE: Ensure data quality and physical validity
RATIONALE: Sensor errors, transmission corruption, or miscalibration

VALIDATION CHECKS:
    1. Required Fields
    2. Physical Range Constraints
    3. Cross-Field Consistency
    4. Temporal Consistency
    5. Statistical Outliers

BEGIN
    errors ← []
    warnings ← []
    quality_penalties ← 0

    // CHECK 1: Required fields
    IF reading.location_id.is_empty THEN
        errors.append("Missing location_id")
    END IF

    IF reading.timestamp IS NULL THEN
        errors.append("Missing timestamp")
    END IF

    // CHECK 2: At least one pollutant measured
    IF reading.pm25 IS NULL AND reading.pm10 IS NULL AND
       reading.co2 IS NULL AND reading.tvoc IS NULL THEN
        errors.append("No pollutant data")
    END IF

    // CHECK 3: Physical range constraints
    IF reading.pm25 IS NOT NULL THEN
        IF reading.pm25 < 0.0 OR reading.pm25 > 1000.0 THEN
            errors.append("PM2.5 out of range [0, 1000]")
        ELSE IF reading.pm25 > 500.0 THEN
            warnings.append("PM2.5 unusually high (>500)")
            quality_penalties += 0.1
        END IF
    END IF

    IF reading.pm10 IS NOT NULL THEN
        IF reading.pm10 < 0.0 OR reading.pm10 > 1000.0 THEN
            errors.append("PM10 out of range [0, 1000]")
        END IF
    END IF

    IF reading.co2 IS NOT NULL THEN
        IF reading.co2 < 400 OR reading.co2 > 10000 THEN
            errors.append("CO2 out of range [400, 10000] ppm")
        ELSE IF reading.co2 < 350 THEN
            warnings.append("CO2 below atmospheric baseline")
            quality_penalties += 0.05
        END IF
    END IF

    IF reading.temperature IS NOT NULL THEN
        IF reading.temperature < -40.0 OR reading.temperature > 85.0 THEN
            errors.append("Temperature out of sensor range [-40, 85]°C")
        END IF
    END IF

    IF reading.humidity IS NOT NULL THEN
        IF reading.humidity > 100 THEN
            errors.append("Humidity >100%")
        END IF
    END IF

    // CHECK 4: Cross-field consistency
    IF reading.pm25 IS NOT NULL AND reading.pm10 IS NOT NULL THEN
        // PM2.5 should be ≤ PM10 (PM2.5 is subset of PM10)
        IF reading.pm25 > reading.pm10 + 1.0 THEN
            warnings.append("PM2.5 > PM10 (physically inconsistent)")
            quality_penalties += 0.2
        END IF
    END IF

    IF reading.pm01 IS NOT NULL AND reading.pm25 IS NOT NULL THEN
        IF reading.pm01 > reading.pm25 + 1.0 THEN
            warnings.append("PM1.0 > PM2.5 (physically inconsistent)")
            quality_penalties += 0.2
        END IF
    END IF

    // CHECK 5: Temporal consistency (requires historical context)
    IF HAS_RECENT_HISTORY(reading.location_id) THEN
        recent ← GET_RECENT_READINGS(reading.location_id, last_10_minutes)

        IF reading.pm25 IS NOT NULL AND recent.has_pm25 THEN
            avg_recent_pm25 ← recent.avg_pm25

            // Detect sudden spikes (>5x increase)
            IF reading.pm25 > avg_recent_pm25 * 5 THEN
                warnings.append("Sudden PM2.5 spike (potential sensor error)")
                quality_penalties += 0.15
            END IF
        END IF
    END IF

    // CHECK 6: Statistical outliers (Z-score)
    IF HAS_HISTORICAL_DATA(reading.location_id) THEN
        stats ← GET_STATISTICS(reading.location_id, last_30_days)

        IF reading.pm25 IS NOT NULL THEN
            z_score ← (reading.pm25 - stats.pm25_mean) / stats.pm25_stddev

            IF abs(z_score) > 3.0 THEN
                warnings.append("PM2.5 is statistical outlier (Z-score > 3)")
                quality_penalties += 0.1
            END IF
        END IF
    END IF

    // Calculate final quality score
    base_quality ← 1.0
    quality_score ← MAX(0.0, base_quality - quality_penalties)

    // Account for missing fields
    total_fields ← 10  // Total number of optional fields
    populated_fields ← count_non_null_fields(reading)
    completeness ← populated_fields / total_fields
    quality_score ← quality_score * (0.5 + 0.5 * completeness)

    RETURN ValidationResult {
        is_valid: errors.is_empty,
        errors: errors,
        warnings: warnings,
        quality_score: quality_score,
        can_sanitize: is_sanitizable(errors)
    }
END

SANITIZATION RULES:
    - Clamp values to valid ranges (if only slightly out of bounds)
    - Replace NaN/Infinity with NULL
    - Trim whitespace from strings
    - Convert future timestamps to current time
    - Round to appropriate precision (e.g., PM2.5 to 1 decimal)
```

### 4.4 Threshold Checking

```
ALGORITHM: CheckThresholds
INPUT: reading (AirQualityReading), thresholds (Vec<ThresholdRule>)
OUTPUT: Vec<Alert>

PURPOSE: Evaluate configured alert conditions
RATIONALE: Notify users/systems when air quality degrades

THRESHOLD TYPES:
    1. Absolute (PM2.5 > 35.4)
    2. Relative (PM2.5 increased >50% in 1 hour)
    3. Duration (PM2.5 > 12.0 for >24 hours)
    4. Composite (AQI > 100 OR CO2 > 1000)

BEGIN
    alerts ← []

    FOR EACH threshold IN thresholds DO
        // Check if threshold applies to this location
        IF NOT threshold.applies_to(reading.location_id) THEN
            CONTINUE
        END IF

        triggered ← MATCH threshold.condition_type
            AbsoluteThreshold =>
                EvaluateAbsolute(reading, threshold)

            RelativeThreshold =>
                EvaluateRelative(reading, threshold)

            DurationThreshold =>
                EvaluateDuration(reading, threshold)

            CompositeThreshold =>
                EvaluateComposite(reading, threshold)
        END MATCH

        IF triggered THEN
            alert ← CreateAlert(
                reading = reading,
                threshold = threshold,
                severity = CalculateSeverity(reading, threshold)
            )

            alerts.append(alert)
        END IF
    END FOR

    RETURN alerts
END

SUBROUTINE: EvaluateAbsolute
INPUT: reading, threshold
OUTPUT: bool (triggered)

BEGIN
    value ← reading.get_field(threshold.field_name)

    IF value IS NULL THEN
        RETURN false
    END IF

    RETURN MATCH threshold.operator
        GreaterThan => value > threshold.value
        LessThan => value < threshold.value
        Equals => abs(value - threshold.value) < threshold.tolerance
        Between => value >= threshold.min AND value <= threshold.max
    END MATCH
END

SUBROUTINE: EvaluateRelative
INPUT: reading, threshold
OUTPUT: bool (triggered)

PURPOSE: Detect sudden changes (spikes/drops)

BEGIN
    current_value ← reading.get_field(threshold.field_name)
    IF current_value IS NULL THEN
        RETURN false
    END IF

    // Get historical value for comparison
    lookback_time ← reading.timestamp - threshold.lookback_duration
    historical ← storage.query_range(
        series = format!("air-quality/{}/{}",
            reading.location_id,
            threshold.field_name
        ),
        start = lookback_time,
        end = lookback_time + 5 minutes  // Allow 5-minute window
    )

    IF historical.is_empty THEN
        RETURN false  // Insufficient data
    END IF

    baseline_value ← historical.first().value

    // Calculate percent change
    percent_change ← ((current_value - baseline_value) / baseline_value) * 100

    RETURN MATCH threshold.operator
        IncreaseBy => percent_change >= threshold.percent
        DecreaseBy => percent_change <= -threshold.percent
        ChangeBy => abs(percent_change) >= threshold.percent
    END MATCH
END

SUBROUTINE: EvaluateDuration
INPUT: reading, threshold
OUTPUT: bool (triggered)

PURPOSE: Alert when condition persists over time

STATE:
    active_durations: HashMap<ThresholdID, DurationState>

BEGIN
    current_value ← reading.get_field(threshold.field_name)
    IF current_value IS NULL THEN
        RETURN false
    END IF

    // Check if threshold exceeded
    threshold_exceeded ← EvaluateAbsolute(reading, threshold)

    IF NOT threshold_exceeded THEN
        // Reset duration tracking
        active_durations.remove(threshold.id)
        RETURN false
    END IF

    // Update duration state
    state ← active_durations.get_or_create(threshold.id)

    IF state.is_new THEN
        state.start_time ← reading.timestamp
        state.first_value ← current_value
        RETURN false  // Not yet duration threshold
    END IF

    // Calculate how long condition has persisted
    duration ← reading.timestamp - state.start_time

    IF duration >= threshold.min_duration THEN
        RETURN true  // Duration threshold met
    ELSE
        RETURN false  // Still accumulating
    END IF
END

SUBROUTINE: CalculateSeverity
INPUT: reading, threshold
OUTPUT: AlertSeverity

BEGIN
    value ← reading.get_field(threshold.field_name)

    // Calculate how much threshold is exceeded
    IF threshold.has_levels THEN
        FOR EACH level IN threshold.levels.sort_descending() DO
            IF value >= level.value THEN
                RETURN level.severity
            END IF
        END FOR
    END IF

    // Default severity based on magnitude
    excess_percent ← ((value - threshold.value) / threshold.value) * 100

    RETURN MATCH excess_percent
        0..=25 => AlertSeverity.Low
        26..=75 => AlertSeverity.Medium
        76..=150 => AlertSeverity.High
        151.. => AlertSeverity.Critical
    END MATCH
END

EXAMPLE THRESHOLD CONFIGURATION:
    {
        "id": "pm25-moderate",
        "name": "PM2.5 Moderate Level",
        "field": "pm25",
        "condition_type": "absolute",
        "operator": "greater_than",
        "value": 12.0,
        "severity": "low",
        "locations": ["*"],  // All locations
        "enabled": true
    }

    {
        "id": "pm25-unhealthy",
        "name": "PM2.5 Unhealthy Level",
        "field": "pm25",
        "condition_type": "absolute",
        "operator": "greater_than",
        "value": 35.5,
        "severity": "high",
        "locations": ["*"]
    }

    {
        "id": "pm25-spike",
        "name": "Sudden PM2.5 Increase",
        "field": "pm25",
        "condition_type": "relative",
        "operator": "increase_by",
        "percent": 50,
        "lookback_duration": "1 hour",
        "severity": "medium",
        "locations": ["*"]
    }
```

---

## 5. Forecasting Pipeline

### 5.1 Data Preparation

```
ALGORITHM: PrepareTrainingData
INPUT:
    location_id (String),
    start_date (Date),
    end_date (Date),
    target_metric (String)  // e.g., "pm25"
OUTPUT: TrainingDataset

PURPOSE: Convert Parquet storage to ruv-FANN input format
RATIONALE: Models require specific input shape, normalization

RUVM-FANN INPUT REQUIREMENTS:
    - Time series as Polars DataFrame
    - Target column (what to predict)
    - Temporal features (hour, day of week, etc.)
    - Exogenous variables (temperature, humidity, etc.)
    - No missing values (imputed or interpolated)

BEGIN
    // Step 1: Query raw data
    readings ← storage.query_range(
        series = format!("air-quality/{}/{}", location_id, target_metric),
        start = start_date.to_datetime(),
        end = end_date.to_datetime(),
        filters = QueryFilters { quality_score_min: 0.5 }
    )

    IF readings.length < 1000 THEN
        RETURN Error("Insufficient training data (<1000 points)")
    END IF

    // Step 2: Convert to Polars DataFrame
    df ← DataFrame.new([
        Series("timestamp", readings.map(|r| r.timestamp)),
        Series("pm25", readings.map(|r| r.pm25)),
        Series("pm10", readings.map(|r| r.pm10)),
        Series("temperature", readings.map(|r| r.temperature)),
        Series("humidity", readings.map(|r| r.humidity)),
        Series("co2", readings.map(|r| r.co2)),
    ])

    // Step 3: Handle missing values
    df ← df.fill_null_strategy(FillNullStrategy.Forward)  // Forward-fill
    df ← df.fill_null_strategy(FillNullStrategy.Backward)  // Backward-fill remaining
    df ← df.drop_nulls()  // Drop any remaining nulls

    // Step 4: Create temporal features
    df ← df.with_column(
        df["timestamp"].dt.hour().alias("hour")
    )
    df ← df.with_column(
        df["timestamp"].dt.day_of_week().alias("day_of_week")
    )
    df ← df.with_column(
        df["timestamp"].dt.month().alias("month")
    )

    // Step 5: Create lag features (previous values)
    FOR lag IN [1, 2, 3, 6, 12, 24] DO  // 1h, 2h, 3h, 6h, 12h, 24h
        df ← df.with_column(
            df[target_metric].shift(lag).alias(format!("{}_lag_{}", target_metric, lag))
        )
    END FOR

    // Step 6: Create rolling statistics
    df ← df.with_column(
        df[target_metric].rolling_mean(window_size=12).alias("pm25_rolling_mean_12h")
    )
    df ← df.with_column(
        df[target_metric].rolling_std(window_size=12).alias("pm25_rolling_std_12h")
    )

    // Step 7: Drop rows with NaN (from lag/rolling features)
    df ← df.drop_nulls()

    // Step 8: Normalize features
    scaler ← StandardScaler.fit(df.select(numeric_columns))
    df_normalized ← scaler.transform(df)

    // Step 9: Train/validation split (80/20)
    split_idx ← floor(df_normalized.length * 0.8)
    train_df ← df_normalized.slice(0, split_idx)
    val_df ← df_normalized.slice(split_idx, df_normalized.length)

    RETURN TrainingDataset {
        train: train_df,
        validation: val_df,
        scaler: scaler,
        target_column: target_metric,
        feature_columns: df.columns.filter(|c| c != target_metric),
        metadata: {
            location_id: location_id,
            date_range: (start_date, end_date),
            num_samples: df_normalized.length
        }
    }
END

FEATURE ENGINEERING RATIONALE:
    - Temporal features: Capture daily/weekly patterns
    - Lag features: Autocorrelation (past values predict future)
    - Rolling stats: Trend and volatility
    - Exogenous vars: External factors (weather)

COMPLEXITY:
    - Time: O(n * f) where n = samples, f = features
    - Space: O(n * f) for DataFrame
```

### 5.2 Model Selection

```
ALGORITHM: SelectForecastModel
INPUT:
    dataset (TrainingDataset),
    forecast_horizon (u32),  // minutes ahead
    use_case (ForecastUseCase)
OUTPUT: ModelType

PURPOSE: Choose optimal model for the task
RATIONALE: Different models excel at different patterns/horizons

AVAILABLE MODELS (ruv-FANN):
    1. NHITS (Neural Hierarchical Interpolation for Time Series)
       - Best for: Multi-horizon, long-range forecasting
       - Strengths: Captures hierarchical patterns
       - Horizons: 1h - 7 days

    2. NBEATSx (Neural Basis Expansion Analysis + eXogenous)
       - Best for: Incorporating external variables
       - Strengths: Weather/temporal features
       - Horizons: 1h - 3 days

    3. Temporal Fusion Transformer (TFT)
       - Best for: Complex multi-variate
       - Strengths: Attention mechanism
       - Horizons: 1h - 2 days

    4. DeepAR
       - Best for: Probabilistic forecasting
       - Strengths: Uncertainty quantification
       - Horizons: 1h - 7 days

BEGIN
    // Decision tree for model selection

    // Rule 1: Short-term tactical forecasts (< 6 hours)
    IF forecast_horizon <= 360 THEN  // ≤ 6 hours
        IF dataset.has_exogenous_features THEN
            RETURN ModelType.NBEATSx
        ELSE
            RETURN ModelType.NHITS
        END IF
    END IF

    // Rule 2: Medium-term forecasts (6h - 48h)
    IF forecast_horizon <= 2880 THEN  // ≤ 48 hours
        IF use_case == ForecastUseCase.ProbabilisticAlert THEN
            RETURN ModelType.DeepAR  // Uncertainty needed
        ELSE IF dataset.feature_count > 10 THEN
            RETURN ModelType.TemporalFusionTransformer
        ELSE
            RETURN ModelType.NHITS
        END IF
    END IF

    // Rule 3: Long-term forecasts (> 48 hours)
    IF forecast_horizon > 2880 THEN
        IF use_case == ForecastUseCase.TrendAnalysis THEN
            RETURN ModelType.NHITS  // Best for hierarchical patterns
        ELSE
            RETURN ModelType.DeepAR  // Uncertainty increases with horizon
        END IF
    END IF

    // Default fallback
    RETURN ModelType.NHITS
END

MODEL HYPERPARAMETERS:
    NHITS:
        stack_types: ["identity", "trend", "seasonality"]
        n_blocks: [1, 1, 1]
        mlp_units: [[256, 256], [256, 256], [256, 256]]
        pooling_sizes: [2, 4, 8]
        dropout: 0.1

    NBEATSx:
        stack_types: ["trend", "seasonality", "exogenous"]
        n_blocks: [1, 1, 1]
        thetas_dim: [8, 8, 8]
        mlp_units: [[256, 256]]
        dropout: 0.1
```

### 5.3 Prediction Generation

```
ALGORITHM: GenerateForecast
INPUT:
    model (TrainedModel),
    current_data (DataFrame),
    horizon (u32),  // minutes ahead
    include_confidence (bool)
OUTPUT: ForecastResult

PURPOSE: Generate future predictions
RATIONALE: Provide actionable forecasts with uncertainty

BEGIN
    // Step 1: Prepare input features
    input_features ← PrepareInferenceFeatures(current_data)

    // Step 2: Generate predictions
    predictions ← model.predict(
        features = input_features,
        horizon = horizon,
        num_samples = 100  // For Monte Carlo confidence intervals
    )

    // Step 3: Inverse transform (denormalize)
    scaler ← model.get_scaler()
    denormalized_predictions ← scaler.inverse_transform(predictions)

    // Step 4: Calculate confidence intervals
    confidence_intervals ← []

    IF include_confidence THEN
        FOR EACH timestep IN denormalized_predictions DO
            // Calculate percentiles from Monte Carlo samples
            p10 ← percentile(timestep.samples, 10)
            p25 ← percentile(timestep.samples, 25)
            p50 ← percentile(timestep.samples, 50)  // Median
            p75 ← percentile(timestep.samples, 75)
            p90 ← percentile(timestep.samples, 90)

            confidence_intervals.append(ConfidenceInterval {
                median: p50,
                lower_80: p10,
                upper_80: p90,
                lower_50: p25,
                upper_50: p75
            })
        END FOR
    END IF

    // Step 5: Convert to domain objects
    forecast_points ← []
    base_timestamp ← current_data.last_timestamp()

    FOR i IN 0..denormalized_predictions.length DO
        timestamp ← base_timestamp + (i + 1) * 1 minute

        prediction_point ← Prediction.new(
            value = denormalized_predictions[i].mean,
            confidence = CalculatePredictionConfidence(
                model.metrics,
                horizon_step = i,
                total_horizon = horizon
            )
        )

        IF include_confidence THEN
            prediction_point ← prediction_point.with_bounds(
                lower = confidence_intervals[i].lower_80,
                upper = confidence_intervals[i].upper_80
            )
        END IF

        prediction_point ← prediction_point.with_horizon(i + 1)
        forecast_points.append(prediction_point)
    END FOR

    // Step 6: Create forecast result
    RETURN ForecastResult {
        predictions: forecast_points,
        model_name: model.name,
        model_version: model.version,
        horizon_minutes: horizon,
        generated_at: Utc::now(),
        confidence_intervals: confidence_intervals,
        feature_importance: model.get_feature_importance()
    }
END

SUBROUTINE: CalculatePredictionConfidence
INPUT: model_metrics, horizon_step, total_horizon
OUTPUT: confidence (f64)

PURPOSE: Estimate prediction reliability
RATIONALE: Confidence degrades with forecast horizon

BEGIN
    // Base confidence from model performance
    base_confidence ← model_metrics.validation_r2  // R² score

    // Decay confidence with horizon
    decay_rate ← 0.02  // 2% per hour
    hours_ahead ← horizon_step / 60.0
    horizon_penalty ← exp(-decay_rate * hours_ahead)

    confidence ← base_confidence * horizon_penalty

    // Clamp to [0, 1]
    RETURN MAX(0.0, MIN(1.0, confidence))
END

COMPLEXITY:
    - Time: O(h * s) where h = horizon, s = Monte Carlo samples
    - Space: O(h * s) for samples, O(h) for output
```

### 5.4 Confidence Interval Calculation

```
ALGORITHM: CalculateConfidenceIntervals
INPUT: predictions (Vec<Vec<f64>>)  // Monte Carlo samples
OUTPUT: Vec<ConfidenceInterval>

PURPOSE: Quantify forecast uncertainty
RATIONALE: Communicate prediction reliability to users

METHODS:
    1. Monte Carlo (sampling-based)
    2. Quantile Regression (direct percentile prediction)
    3. Bootstrap (resampling historical errors)

BEGIN (Monte Carlo approach)
    confidence_intervals ← []

    FOR EACH timestep_samples IN predictions DO
        // Sort samples
        sorted_samples ← sort(timestep_samples)
        n_samples ← sorted_samples.length

        // Calculate percentiles
        p05 ← sorted_samples[floor(n_samples * 0.05)]
        p10 ← sorted_samples[floor(n_samples * 0.10)]
        p25 ← sorted_samples[floor(n_samples * 0.25)]
        p50 ← sorted_samples[floor(n_samples * 0.50)]
        p75 ← sorted_samples[floor(n_samples * 0.75)]
        p90 ← sorted_samples[floor(n_samples * 0.90)]
        p95 ← sorted_samples[floor(n_samples * 0.95)]

        confidence_intervals.append(ConfidenceInterval {
            median: p50,
            mean: mean(timestep_samples),
            std_dev: std_dev(timestep_samples),

            // 50% confidence interval (IQR)
            lower_50: p25,
            upper_50: p75,

            // 80% confidence interval
            lower_80: p10,
            upper_80: p90,

            // 90% confidence interval
            lower_90: p05,
            upper_90: p95
        })
    END FOR

    RETURN confidence_intervals
END

INTERPRETATION:
    - 50% CI: Half the outcomes fall in this range (narrow, high confidence)
    - 80% CI: 80% of outcomes in this range (wider, more certain)
    - 90% CI: 90% of outcomes in this range (widest, very certain)

VISUALIZATION:
    - Plot median as line
    - Shade 50% CI (dark)
    - Shade 80% CI (light)
    - Show 90% CI as dotted lines
```

---

## 6. Alert System

### 6.1 Threshold Evaluation

```
ALGORITHM: EvaluateAlerts
INPUT: reading (AirQualityReading)
OUTPUT: Vec<Alert>

PURPOSE: Check all configured alert rules
RATIONALE: Multi-criteria alerting (absolute, relative, forecast-based)

BEGIN
    alerts ← []

    // Step 1: Absolute threshold alerts (see CheckThresholds)
    absolute_alerts ← CheckThresholds(reading, ABSOLUTE_THRESHOLDS)
    alerts.extend(absolute_alerts)

    // Step 2: AQI-based alerts
    IF reading.aqi IS NOT NULL THEN
        aqi_alert ← MATCH reading.aqi_category
            AQICategory.Good | AQICategory.Moderate =>
                None

            AQICategory.UnhealthyForSensitive =>
                Alert {
                    level: AlertLevel.Info,
                    title: "Air Quality: Unhealthy for Sensitive Groups",
                    message: format!("AQI {} at {}", reading.aqi, reading.location_id),
                    category: AlertCategory.HealthAdvisory
                }

            AQICategory.Unhealthy =>
                Alert {
                    level: AlertLevel.Warning,
                    title: "Air Quality: Unhealthy",
                    message: format!("AQI {} - Everyone should reduce outdoor activity", reading.aqi),
                    category: AlertCategory.HealthAlert
                }

            AQICategory.VeryUnhealthy | AQICategory.Hazardous =>
                Alert {
                    level: AlertLevel.Critical,
                    title: "Air Quality Emergency",
                    message: format!("AQI {} - Avoid all outdoor activity", reading.aqi),
                    category: AlertCategory.Emergency
                }
        END MATCH

        IF aqi_alert IS NOT NULL THEN
            alerts.append(aqi_alert)
        END IF
    END IF

    // Step 3: Forecast-based alerts (predictive)
    forecast_alerts ← CheckForecastAlerts(reading)
    alerts.extend(forecast_alerts)

    // Step 4: Sensor health alerts
    sensor_alerts ← CheckSensorHealth(reading)
    alerts.extend(sensor_alerts)

    RETURN alerts
END

SUBROUTINE: CheckForecastAlerts
INPUT: reading (AirQualityReading)
OUTPUT: Vec<Alert>

PURPOSE: Alert on predicted future conditions
RATIONALE: Give users advance warning

BEGIN
    alerts ← []

    // Get forecast for next 24 hours
    forecast ← GenerateForecast(
        model = get_model(reading.location_id, "pm25"),
        current_data = get_recent_data(reading.location_id, last_48_hours),
        horizon = 24 * 60,  // 24 hours
        include_confidence = true
    )

    // Check if forecast predicts threshold exceedance
    FOR EACH prediction IN forecast.predictions DO
        IF prediction.value > 35.5 AND prediction.confidence > 0.7 THEN
            hours_ahead ← prediction.horizon_minutes / 60

            alerts.append(Alert {
                level: AlertLevel.Warning,
                title: "Forecast: Unhealthy Air Quality Expected",
                message: format!(
                    "PM2.5 predicted to reach {} in {} hours",
                    prediction.value,
                    hours_ahead
                ),
                category: AlertCategory.ForecastAlert,
                scheduled_for: reading.timestamp + prediction.horizon_minutes
            })

            BREAK  // Only alert on first exceedance
        END IF
    END FOR

    RETURN alerts
END

SUBROUTINE: CheckSensorHealth
INPUT: reading (AirQualityReading)
OUTPUT: Vec<Alert>

BEGIN
    alerts ← []

    // Check data quality
    IF reading.quality_score < 0.5 THEN
        alerts.append(Alert {
            level: AlertLevel.Warning,
            title: "Sensor Data Quality Low",
            message: format!("Quality score {} at {}", reading.quality_score, reading.location_id),
            category: AlertCategory.SensorHealth
        })
    END IF

    // Check WiFi signal
    IF reading.wifi_signal IS NOT NULL AND reading.wifi_signal < -80 THEN
        alerts.append(Alert {
            level: AlertLevel.Info,
            title: "Weak WiFi Signal",
            message: format!("Signal {} dBm - may affect data transmission", reading.wifi_signal),
            category: AlertCategory.SensorHealth
        })
    END IF

    // Check last seen (requires context)
    last_seen_duration ← Utc::now() - reading.timestamp
    IF last_seen_duration > 15 minutes THEN
        alerts.append(Alert {
            level: AlertLevel.Error,
            title: "Sensor Offline",
            message: format!("No data received for {} minutes", last_seen_duration.minutes),
            category: AlertCategory.SensorHealth
        })
    END IF

    RETURN alerts
END
```

### 6.2 Rate Limiting

```
ALGORITHM: ShouldSendAlert
INPUT: alert (Alert)
OUTPUT: bool (should_send)

PURPOSE: Prevent alert fatigue
RATIONALE: Don't spam users with repeated identical alerts

RATE LIMITING STRATEGIES:
    1. Cooldown Period (don't repeat same alert within N minutes)
    2. Escalation (send info → warning → critical progressively)
    3. Deduplication (suppress identical consecutive alerts)
    4. Aggregation (batch similar alerts)

STATE:
    recent_alerts: LRU<AlertKey, AlertHistory>

BEGIN
    // Step 1: Generate alert key (fingerprint)
    alert_key ← AlertKey {
        location_id: alert.location_id,
        category: alert.category,
        threshold_id: alert.threshold_id
    }

    // Step 2: Check recent history
    history ← recent_alerts.get(alert_key)

    IF history IS NULL THEN
        // First alert of this type
        recent_alerts.insert(alert_key, AlertHistory {
            first_sent: Utc::now(),
            last_sent: Utc::now(),
            count: 1,
            last_level: alert.level
        })
        RETURN true  // Send it
    END IF

    // Step 3: Apply cooldown period
    cooldown_duration ← MATCH alert.level
        AlertLevel.Info => 60 minutes
        AlertLevel.Warning => 30 minutes
        AlertLevel.Error => 15 minutes
        AlertLevel.Critical => 5 minutes  // More frequent for critical
    END MATCH

    time_since_last ← Utc::now() - history.last_sent

    IF time_since_last < cooldown_duration THEN
        // Still in cooldown - suppress
        history.count += 1  // Track suppressed alerts
        RETURN false
    END IF

    // Step 4: Check escalation
    IF alert.level > history.last_level THEN
        // Severity increased - send immediately
        history.last_sent ← Utc::now()
        history.last_level ← alert.level
        RETURN true
    END IF

    // Step 5: Allow alert (cooldown expired)
    history.last_sent ← Utc::now()
    history.count += 1
    RETURN true
END

ALERT AGGREGATION:
    // If multiple similar alerts, send summary
    IF COUNT(alerts) > 5 AND ALL_SAME_CATEGORY THEN
        aggregated_alert ← Alert {
            level: MAX(alerts.map(|a| a.level)),
            title: format!("{} Air Quality Alerts", alerts.length),
            message: format!(
                "Multiple locations reporting issues: {}",
                alerts.map(|a| a.location_id).join(", ")
            ),
            category: alerts[0].category
        }
        RETURN [aggregated_alert]
    END IF
```

### 6.3 Notification Dispatch

```
ALGORITHM: DispatchAlert
INPUT: alert (Alert)
OUTPUT: Result<()>

PURPOSE: Send alert through configured channels
RATIONALE: Multi-channel delivery (email, SMS, webhook, dashboard)

CHANNELS:
    1. WebSocket (real-time dashboard)
    2. Webhook (external integrations)
    3. Email (async notifications)
    4. SMS (critical alerts only)
    5. Push Notifications (mobile app)

BEGIN
    // Step 1: Determine channels based on severity
    channels ← MATCH alert.level
        AlertLevel.Info =>
            [Channel.WebSocket, Channel.Dashboard]

        AlertLevel.Warning =>
            [Channel.WebSocket, Channel.Dashboard, Channel.Email]

        AlertLevel.Error =>
            [Channel.WebSocket, Channel.Dashboard, Channel.Email, Channel.Webhook]

        AlertLevel.Critical =>
            [Channel.WebSocket, Channel.Dashboard, Channel.Email,
             Channel.Webhook, Channel.SMS, Channel.PushNotification]
    END MATCH

    // Step 2: Get subscribers for this location/category
    subscribers ← get_subscribers(
        location_id = alert.location_id,
        category = alert.category,
        min_severity = alert.level
    )

    // Step 3: Dispatch to each channel (parallel)
    results ← []

    FOR EACH channel IN channels PARALLEL DO
        TRY
            result ← MATCH channel
                Channel.WebSocket =>
                    SendWebSocketAlert(alert, subscribers)

                Channel.Email =>
                    SendEmailAlert(alert, subscribers)

                Channel.SMS =>
                    SendSMSAlert(alert, subscribers.filter(|s| s.phone IS NOT NULL))

                Channel.Webhook =>
                    SendWebhookAlert(alert)

                Channel.Dashboard =>
                    UpdateDashboard(alert)

                Channel.PushNotification =>
                    SendPushNotification(alert, subscribers)
            END MATCH

            results.append(Ok(channel))

        CATCH error AS e
            LOG error("Alert dispatch failed", channel = channel, error = e)
            results.append(Error(channel, e))

            // Retry critical alerts
            IF alert.level == AlertLevel.Critical THEN
                spawn_retry_task(alert, channel)
            END IF
        END CATCH
    END FOR

    // Step 4: Log alert
    alert_log.insert(AlertLogEntry {
        alert: alert,
        timestamp: Utc::now(),
        channels_attempted: channels,
        channels_succeeded: results.filter(|r| r.is_ok()),
        channels_failed: results.filter(|r| r.is_err())
    })

    // Step 5: Update metrics
    METRICS.increment("alerts_dispatched", 1)
    METRICS.increment(format!("alerts_{}", alert.level), 1)
    FOR EACH channel IN channels DO
        METRICS.increment(format!("alert_channel_{}", channel), 1)
    END FOR

    RETURN Ok()
END

SUBROUTINE: SendWebhookAlert
INPUT: alert (Alert)
OUTPUT: Result<()>

BEGIN
    webhooks ← get_webhooks(alert.location_id)

    FOR EACH webhook IN webhooks DO
        payload ← json!({
            "event": "alert",
            "alert": {
                "id": alert.id,
                "level": alert.level,
                "title": alert.title,
                "message": alert.message,
                "category": alert.category,
                "location_id": alert.location_id,
                "timestamp": alert.timestamp
            }
        })

        response ← http_client.post(webhook.url)
            .header("Content-Type", "application/json")
            .header("X-Alert-Signature", sign_payload(payload, webhook.secret))
            .json(payload)
            .timeout(10 seconds)
            .send()
            .await?

        IF NOT response.status().is_success() THEN
            RETURN Error(format!("Webhook failed: {}", response.status()))
        END IF
    END FOR

    RETURN Ok()
END
```

---

## 7. Configuration Management (config-store)

### 7.1 Configuration Loading Algorithm

```
ALGORITHM: LoadConfiguration
INPUT: config_path (String), environment (String)
OUTPUT: Result<ConfigManager, ConfigError>

PURPOSE: Load and validate configuration from YAML files using config-store
RATIONALE: Centralized, versioned configuration with GitOps support

CONFIGURATION SOURCES (in priority order):
    1. Environment variables (highest priority)
    2. Environment overlay (overlays/{environment}/overrides.yaml)
    3. Base configuration (base/*.yaml)
    4. GitHub repository (remote source, if configured)

CONSTANTS:
    CONFIG_NAMESPACE = "/air-quality"
    MAX_CONFIG_DEPTH = 6
    SCHEMA_PATH = "schemas/air-quality-config.json"

BEGIN
    LOG info("Loading configuration", environment = environment)

    // Step 1: Initialize config-store backend
    store ← SecureInMemoryConfigStore::new()

    // Step 2: Initialize GitOps loader
    gitops_loader ← GitOpsLoader {
        base_path: config_path,
        environment: environment,
    }

    // Step 3: Load JSON Schema for validation
    TRY
        schema_validator ← SchemaValidator::from_file(SCHEMA_PATH).await?
    CATCH error AS e
        LOG warning("Schema not found, skipping validation", error = e)
        schema_validator ← None
    END CATCH

    // Step 4: Load base configuration files
    LOG debug("Loading base configuration")
    base_configs ← HashMap::new()

    FOR EACH file IN glob("base/*.yaml") DO
        yaml_content ← read_file(file).await?
        parsed ← serde_yaml::from_str(yaml_content)?

        // Convert YAML to ConfigValue
        config_value ← yaml_to_config_value(parsed)

        // Extract namespace from filename (e.g., "air-quality.yaml" → "/air-quality")
        namespace ← extract_namespace(file)
        base_configs.insert(namespace, config_value)
    END FOR

    // Step 5: Load environment overlay
    LOG debug("Loading environment overlay", environment = environment)
    overlay_path ← format!("overlays/{}/overrides.yaml", environment)

    overlay_configs ← IF file_exists(overlay_path) THEN
        yaml_content ← read_file(overlay_path).await?
        yaml_to_config_value(serde_yaml::from_str(yaml_content)?)
    ELSE
        ConfigValue::Object(HashMap::new())
    END IF

    // Step 6: Deep merge base + overlay
    merged_configs ← deep_merge(base_configs, overlay_configs)

    // Step 7: Apply environment variable substitution
    merged_configs ← substitute_env_vars(merged_configs)

    // Step 8: Validate against schema (if available)
    IF schema_validator IS Some THEN
        validation_result ← schema_validator.validate(merged_configs)?

        IF NOT validation_result.is_valid() THEN
            LOG error("Configuration validation failed", errors = validation_result.errors)
            RETURN Error(ConfigError::ValidationFailed(validation_result.errors))
        END IF
    END IF

    // Step 9: Store configuration in config-store with hierarchical paths
    store_hierarchical(store, CONFIG_NAMESPACE, merged_configs).await?

    LOG info("Configuration loaded successfully",
        environment = environment,
        keys_loaded = merged_configs.len())

    RETURN Ok(ConfigManager {
        store: Arc::new(store),
        gitops_loader: gitops_loader,
        schema_validator: schema_validator,
        watch_handle: None,
    })
END

ERROR HANDLING:
    - File not found → Use defaults or fail startup
    - YAML parse error → Fail with descriptive error and line number
    - Schema validation error → Fail with validation details
    - Environment variable not set → Use default or fail (depends on required flag)
```

### 7.2 Environment Variable Substitution

```
ALGORITHM: SubstituteEnvVars
INPUT: config (ConfigValue)
OUTPUT: ConfigValue

PURPOSE: Replace ${VAR_NAME} and ${VAR_NAME:default} patterns with environment values
RATIONALE: Support 12-factor app configuration without hardcoding secrets

PATTERN: ${VAR_NAME} or ${VAR_NAME:default_value}

BEGIN
    MATCH config
        ConfigValue::String(s) =>
            // Pattern: ${VAR_NAME} or ${VAR_NAME:default}
            regex ← Regex::new(r"\$\{([^}:]+)(?::([^}]*))?\}")

            result ← regex.replace_all(s, |captures| {
                var_name ← captures[1]
                default_value ← captures.get(2).map(|m| m.as_str())

                MATCH std::env::var(var_name)
                    Ok(value) => value
                    Err(_) =>
                        IF default_value IS Some THEN
                            default_value.to_string()
                        ELSE
                            LOG warning("Environment variable not set", var = var_name)
                            format!("${{{}}}", var_name)  // Leave unsubstituted
                        END IF
                END MATCH
            })

            RETURN ConfigValue::String(result.to_string())

        ConfigValue::Object(map) =>
            new_map ← HashMap::new()
            FOR (key, value) IN map DO
                new_map.insert(key, SubstituteEnvVars(value))
            END FOR
            RETURN ConfigValue::Object(new_map)

        ConfigValue::Array(arr) =>
            RETURN ConfigValue::Array(
                arr.iter().map(|v| SubstituteEnvVars(v)).collect()
            )

        _ => RETURN config  // Primitives unchanged
    END MATCH
END

EXAMPLES:
    "${MQTT_BROKER_URL}" → "mqtt://localhost:1883"
    "${WEBHOOK_URL:http://localhost:8000}" → default if not set
    "${API_KEY}" → Warning logged if not set
```

### 7.3 GitHub Configuration Sourcing

```
ALGORITHM: LoadFromGitHub
INPUT: repo (String), branch (String), path (String), auth_token (Option<String>)
OUTPUT: Result<ConfigValue, ConfigError>

PURPOSE: Fetch configuration from GitHub repository for GitOps workflows
RATIONALE: Enable centralized configuration management with version control

CONSTANTS:
    GITHUB_RAW_URL = "https://raw.githubusercontent.com"
    FETCH_TIMEOUT = 30 seconds
    CACHE_TTL = 300 seconds  // 5 minutes

STATE:
    cache: HashMap<String, (ConfigValue, Instant)>

BEGIN
    LOG debug("Fetching configuration from GitHub", repo = repo, branch = branch, path = path)

    // Step 1: Check cache
    cache_key ← format!("{}:{}:{}", repo, branch, path)

    IF cache.contains(cache_key) THEN
        (cached_value, cached_at) ← cache.get(cache_key)

        IF Instant::now() - cached_at < CACHE_TTL THEN
            LOG debug("Using cached GitHub configuration")
            RETURN Ok(cached_value.clone())
        END IF
    END IF

    // Step 2: Build GitHub raw URL
    url ← format!("{}/{}/{}/{}", GITHUB_RAW_URL, repo, branch, path)

    // Step 3: Prepare request with optional authentication
    request ← http_client.get(url)
        .timeout(FETCH_TIMEOUT)

    IF auth_token IS Some THEN
        request ← request.header("Authorization", format!("token {}", auth_token))
    END IF

    // Step 4: Fetch and parse
    TRY
        response ← request.send().await?

        IF NOT response.status().is_success() THEN
            RETURN Error(ConfigError::FetchFailed(response.status()))
        END IF

        content ← response.text().await?
        config_value ← yaml_to_config_value(serde_yaml::from_str(content)?)

        // Step 5: Update cache
        cache.insert(cache_key, (config_value.clone(), Instant::now()))

        LOG info("Fetched configuration from GitHub", repo = repo, files = 1)
        RETURN Ok(config_value)

    CATCH error AS e
        LOG warning("GitHub fetch failed, using cached/local fallback", error = e)

        // Return cached value if available (even if expired)
        IF cache.contains(cache_key) THEN
            (cached_value, _) ← cache.get(cache_key)
            RETURN Ok(cached_value.clone())
        END IF

        RETURN Error(ConfigError::FetchFailed(e))
    END CATCH
END
```

### 7.4 Configuration Hot-Reload

```
ALGORITHM: WatchConfiguration
INPUT: config_manager (Arc<ConfigManager>), callback (Fn(&str, ConfigValue))
OUTPUT: JoinHandle<()>

PURPOSE: Watch for configuration changes and apply hot-reloads
RATIONALE: Enable threshold updates without service restart

RELOADABLE SETTINGS:
    - /air-quality/thresholds/* (all threshold values)
    - /air-quality/alerting/* (alert channels, rate limits)
    - /air-quality/forecasting/horizon_hours
    - /air-quality/forecasting/confidence_intervals

NON-RELOADABLE SETTINGS (require restart):
    - /air-quality/ingestion/mqtt/broker_url
    - Storage configuration
    - Sensor serial numbers

BEGIN
    LOG info("Starting configuration watcher")

    handle ← tokio::spawn(async move {
        // Option 1: File system watcher (for local files)
        watcher ← notify::recommended_watcher(|event| {
            MATCH event
                Event::Modify(path) =>
                    IF path.ends_with(".yaml") OR path.ends_with(".yml") THEN
                        LOG info("Configuration file changed", path = path)
                        reload_config(config_manager, path, callback)
                    END IF

                _ => ()  // Ignore other events
            END MATCH
        })

        watcher.watch("config/", RecursiveMode::Recursive)?

        // Option 2: Polling for remote sources (GitHub)
        poll_interval ← Duration::from_secs(300)  // 5 minutes

        LOOP
            tokio::time::sleep(poll_interval).await

            TRY
                new_config ← load_from_github().await?
                current_config ← config_manager.store.get_tree("/air-quality").await?

                changes ← diff_configs(current_config, new_config)

                FOR EACH change IN changes DO
                    IF is_reloadable(change.path) THEN
                        LOG info("Hot-reloading configuration", path = change.path)
                        config_manager.store.set(change.path, change.new_value).await?
                        callback(change.path, change.new_value)
                    ELSE
                        LOG warning("Configuration change requires restart",
                            path = change.path)
                    END IF
                END FOR

            CATCH error AS e
                LOG warning("Configuration poll failed", error = e)
            END CATCH
        END LOOP
    })

    RETURN handle
END

SUBROUTINE: is_reloadable
INPUT: path (String)
OUTPUT: bool

BEGIN
    reloadable_prefixes ← [
        "/air-quality/thresholds/",
        "/air-quality/alerting/",
        "/air-quality/forecasting/horizon",
        "/air-quality/forecasting/confidence",
    ]

    RETURN reloadable_prefixes.iter().any(|prefix| path.starts_with(prefix))
END
```

### 7.5 Typed Configuration Access

```
ALGORITHM: GetTypedConfig
INPUT: path (String), store (Arc<dyn ConfigStore>)
OUTPUT: Result<T, ConfigError> where T: DeserializeOwned

PURPOSE: Retrieve configuration as typed Rust structs
RATIONALE: Compile-time type safety for configuration values

BEGIN
    // Step 1: Get raw ConfigValue from store
    value ← store.get(path).await?

    // Step 2: Convert to JSON for serde
    json_value ← config_value_to_json(value)

    // Step 3: Deserialize to target type
    TRY
        typed_value ← serde_json::from_value(json_value)?
        RETURN Ok(typed_value)
    CATCH error AS e
        RETURN Error(ConfigError::DeserializationFailed(path, e.to_string()))
    END CATCH
END

EXAMPLE USAGE:
    // Get all thresholds as struct
    thresholds: ThresholdConfig ← config.get("/air-quality/thresholds").await?

    // Get single value
    co2_good: u16 ← config.get("/air-quality/thresholds/co2/good").await?

    // Get sensor list
    sensors: Vec<SensorConfig> ← config.get("/air-quality/sensors").await?
```

---

## 8. Main Event Loop

### 8.1 Startup Sequence

```
ALGORITHM: StartupSequence
INPUT: config_path (String), environment (String)
OUTPUT: RunningApplication

PURPOSE: Initialize all components in correct order
RATIONALE: Dependencies must be initialized before dependents

STARTUP STEPS:
    1. Load configuration from config-store (YAML + GitHub + env vars)
    2. Validate configuration against JSON Schema
    3. Initialize storage
    4. Initialize MQTT client
    5. Load/train models
    6. Start configuration watcher (hot-reload)
    7. Start background tasks
    8. Begin message processing

BEGIN
    LOG info("Starting Neural Data Platform - Air Quality")

    // Step 1: Load configuration using config-store
    LOG info("Loading configuration", environment = environment)
    config_manager ← LoadConfiguration(config_path, environment).await?

    // Step 2: Get typed configuration
    app_config: ApplicationConfig ← config_manager.get("/air-quality").await?
    LOG info("Configuration validated successfully")

    // Step 2: Initialize storage
    LOG info("Initializing storage backend")
    storage ← MATCH config.storage.backend
        StorageBackend.Parquet =>
            ParquetStorage.new(config.storage.parquet_config)?

        StorageBackend.TimescaleDB =>
            TimescaleDBStorage.new(config.storage.timescale_config)?

        _ =>
            RETURN Error("Unsupported storage backend")
    END MATCH

    storage.health_check().await?
    LOG info("Storage initialized successfully")

    // Step 3: Initialize MQTT connection
    LOG info("Connecting to MQTT broker", broker = config.mqtt.broker_host)
    (mqtt_client, mqtt_event_loop) ← MQTTConnectionManager(config.mqtt)?
    LOG info("MQTT connected successfully")

    // Step 4: Load forecasting models
    LOG info("Loading forecasting models")
    models ← HashMap.new()

    FOR EACH location IN config.locations DO
        FOR EACH metric IN config.forecast_metrics DO
            model_key ← format!("{}-{}", location.id, metric)

            // Try to load existing model
            model_path ← format!("models/{}.safetensors", model_key)

            model ← IF file_exists(model_path) THEN
                LOG info("Loading model", path = model_path)
                load_model(model_path)?
            ELSE
                LOG warning("Model not found, will train on first data", model_key)
                None
            END IF

            models.insert(model_key, model)
        END FOR
    END FOR

    // Step 5: Start background tasks
    LOG info("Starting background tasks")

    // Task: Periodic compaction
    compaction_task ← spawn_task(async {
        LOOP
            sleep(24 hours)
            TRY
                stats ← storage.compact().await?
                LOG info("Compaction completed", stats)
            CATCH error AS e
                LOG error("Compaction failed", error = e)
            END CATCH
        END LOOP
    })

    // Task: Model retraining
    retraining_task ← spawn_task(async {
        LOOP
            sleep(7 days)
            TRY
                RetainAllModels(storage, models).await?
                LOG info("Model retraining completed")
            CATCH error AS e
                LOG error("Retraining failed", error = e)
            END CATCH
        END LOOP
    })

    // Task: Metrics export
    metrics_task ← spawn_task(async {
        LOOP
            sleep(60 seconds)
            ExportMetrics().await
        END LOOP
    })

    // Step 6: Register shutdown handler
    shutdown_signal ← register_shutdown_handler()

    LOG info("Startup complete, processing messages")

    RETURN RunningApplication {
        config: config,
        storage: storage,
        mqtt_client: mqtt_client,
        mqtt_event_loop: mqtt_event_loop,
        models: models,
        background_tasks: [compaction_task, retraining_task, metrics_task],
        shutdown_signal: shutdown_signal
    }
END

CONFIGURATION VALIDATION:
    - MQTT broker reachable
    - Storage backend accessible
    - Model paths exist or can be created
    - Alert webhooks configured correctly
    - Required fields present
```

### 7.2 Message Processing Loop

```
ALGORITHM: MainEventLoop
INPUT: app (RunningApplication)
OUTPUT: Never (runs until shutdown)

PURPOSE: Core application loop
RATIONALE: Process MQTT messages, handle errors, coordinate tasks

BEGIN
    LOOP
        SELECT
            // Case 1: MQTT event
            event ← app.mqtt_event_loop.poll() =>
                MATCH event
                    Ok(Event::Incoming(Packet::Publish(msg))) =>
                        TRY
                            ProcessMessage(msg, app).await
                        CATCH error AS e
                            LOG error("Message processing failed", error = e)
                            METRICS.increment("processing_errors")
                        END CATCH

                    Ok(Event::Incoming(Packet::ConnAck)) =>
                        LOG info("MQTT reconnected")
                        METRICS.set("mqtt_connected", 1)

                    Err(error) =>
                        LOG error("MQTT error", error = error)
                        METRICS.set("mqtt_connected", 0)
                        // Connection manager will reconnect

                    _ =>
                        CONTINUE
                END MATCH

            // Case 2: Shutdown signal
            _ ← app.shutdown_signal =>
                LOG info("Shutdown signal received")
                BREAK

            // Case 3: Timeout (heartbeat)
            _ ← sleep(30 seconds) =>
                // Heartbeat: Update health metrics
                UpdateHealthMetrics(app).await
        END SELECT
    END LOOP

    // Graceful shutdown
    GracefulShutdown(app).await
END

SUBROUTINE: ProcessMessage
INPUT: msg (MQTTMessage), app (RunningApplication)
OUTPUT: Result<()>

BEGIN
    start_time ← Utc::now()

    // Step 1: Parse message
    reading ← ParseAirGradientMessage(msg.payload)?
    METRICS.increment("messages_parsed")

    // Step 2: Validate
    validation ← ValidateAirQualityReading(reading)
    IF NOT validation.is_valid THEN
        DeadLetterQueue.send(msg, validation.errors)
        RETURN Error("Validation failed")
    END IF
    METRICS.increment("readings_validated")

    // Step 3: Calculate AQI
    (aqi, category, pollutant) ← CalculateAQI(reading)
    reading.aqi ← Some(aqi)
    reading.aqi_category ← Some(category)
    METRICS.histogram("aqi_values", aqi)

    // Step 4: Store reading
    app.storage.append(reading).await?
    METRICS.increment("readings_stored")

    // Step 5: Check alerts (async, non-blocking)
    spawn_task(async {
        alerts ← EvaluateAlerts(reading)
        FOR EACH alert IN alerts DO
            IF ShouldSendAlert(alert) THEN
                DispatchAlert(alert).await
            END IF
        END FOR
    })

    // Step 6: Update forecast (periodic, not every message)
    IF should_update_forecast(reading.location_id) THEN
        spawn_task(async {
            UpdateForecast(reading.location_id, app).await
        })
    END IF

    // Metrics
    processing_time ← (Utc::now() - start_time).milliseconds
    METRICS.histogram("message_processing_time_ms", processing_time)

    RETURN Ok()
END

SUBROUTINE: UpdateHealthMetrics
INPUT: app (RunningApplication)
OUTPUT: ()

BEGIN
    // Storage health
    storage_health ← app.storage.health_check().await
    METRICS.set("storage_healthy", IF storage_health.is_healthy THEN 1 ELSE 0)
    METRICS.histogram("storage_response_time_ms", storage_health.response_time_ms)

    // Model status
    FOR EACH (model_key, model) IN app.models DO
        IF model IS NOT NULL THEN
            METRICS.set(format!("model_loaded_{}", model_key), 1)
        ELSE
            METRICS.set(format!("model_loaded_{}", model_key), 0)
        END IF
    END FOR

    // Memory usage
    memory_usage ← get_memory_usage()
    METRICS.gauge("memory_usage_bytes", memory_usage)

    // Uptime
    uptime ← Utc::now() - app.start_time
    METRICS.gauge("uptime_seconds", uptime.seconds)
END
```

### 7.3 Graceful Shutdown

```
ALGORITHM: GracefulShutdown
INPUT: app (RunningApplication)
OUTPUT: ()

PURPOSE: Clean shutdown without data loss
RATIONALE: Flush buffers, close connections, save state

SHUTDOWN STEPS:
    1. Stop accepting new messages
    2. Process remaining messages in queue
    3. Flush storage buffers
    4. Disconnect MQTT
    5. Stop background tasks
    6. Export final metrics

BEGIN
    LOG info("Starting graceful shutdown")

    // Step 1: Stop accepting new messages
    app.mqtt_client.disconnect().await
    LOG info("Stopped accepting new messages")

    // Step 2: Process remaining buffered messages (timeout 30s)
    SELECT
        _ ← process_pending_messages(app) =>
            LOG info("All pending messages processed")

        _ ← sleep(30 seconds) =>
            LOG warning("Shutdown timeout, forcing exit")
    END SELECT

    // Step 3: Flush storage buffers
    TRY
        app.storage.flush_all_buffers().await
        LOG info("Storage buffers flushed")
    CATCH error AS e
        LOG error("Failed to flush buffers", error = e)
    END CATCH

    // Step 4: Save model checkpoints
    FOR EACH (model_key, model) IN app.models DO
        IF model IS NOT NULL THEN
            model_path ← format!("models/{}.safetensors", model_key)
            TRY
                model.save(model_path).await
                LOG info("Saved model", path = model_path)
            CATCH error AS e
                LOG error("Failed to save model", model_key, error = e)
            END CATCH
        END IF
    END FOR

    // Step 5: Stop background tasks
    FOR EACH task IN app.background_tasks DO
        task.cancel().await
    END FOR
    LOG info("Background tasks stopped")

    // Step 6: Export final metrics
    ExportMetrics().await
    LOG info("Final metrics exported")

    // Step 7: Close storage connection
    app.storage.close().await
    LOG info("Storage connection closed")

    LOG info("Graceful shutdown complete")
END

ERROR RECOVERY:
    - If shutdown fails, dump state to recovery file
    - On restart, check for recovery file
    - Replay unsaved messages from recovery file
```

---

## Complexity Summary

### Time Complexity Analysis

| Algorithm | Best Case | Average Case | Worst Case | Notes |
|-----------|-----------|--------------|------------|-------|
| Append to Parquet | O(1) | O(1) amortized | O(n log n) | Flush triggers sort |
| Query Range | O(log p) | O(p * m) | O(p * m) | p=partitions, m=points/partition |
| Compaction | O(n log n) | O(n log n) | O(n log n) | Sort + merge |
| MQTT Message Parse | O(1) | O(1) | O(1) | Fixed schema |
| AQI Calculation | O(k) | O(k) | O(k) | k=pollutant types (~6) |
| Forecast Generation | O(h * s) | O(h * s) | O(h * s) | h=horizon, s=samples |
| Alert Evaluation | O(t) | O(t) | O(t) | t=threshold count |

### Space Complexity Analysis

| Component | Space Usage | Notes |
|-----------|-------------|-------|
| Write Buffer | O(b * p) | b=buffer size, p=active partitions |
| Query Result | O(r) | r=result size (capped at 100k) |
| Model Cache | O(m * w) | m=models, w=weights |
| Alert History | O(a) | a=active alerts (LRU bounded) |
| MQTT Queue | O(q) | q=queue size (bounded to 10k) |

---

## Document Summary

This pseudocode document provides comprehensive algorithmic specifications for the Neural Data Platform Air Quality feature (air-001). All algorithms are designed to be:

1. **Domain-agnostic**: Core traits can be reused for other time-series applications
2. **Scalable**: Partitioned storage, parallel processing, bounded resources
3. **Reliable**: Error recovery, validation, quality scoring
4. **Observable**: Metrics, logging, health checks throughout

**Key Design Decisions:**
- Parquet storage for efficient compression and query performance
- MQTT for real-time ingestion with backpressure handling
- ruv-FANN models for accurate forecasting with uncertainty quantification
- Multi-channel alerting with rate limiting to prevent fatigue
- Graceful degradation and error recovery at every layer

**Next Steps (Architecture Phase):**
- Define module boundaries and interfaces
- Specify Rust crate structure
- Design proto schemas for events
- Document configuration schema
- Plan deployment architecture
