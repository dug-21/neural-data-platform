# Error Handling - Bronze MCP Server

## Overview

This document defines the error handling strategy, error types, and recovery mechanisms for the Bronze MCP Server. The approach follows NDP patterns: use `thiserror` for library errors and structured error responses for MCP clients.

---

## Error Classification

```
+------------------+     +------------------+     +------------------+
|   Infrastructure |     |    Application   |     |      Client      |
|     Errors       |     |      Errors      |     |      Errors      |
+------------------+     +------------------+     +------------------+
        |                        |                        |
        v                        v                        v
+------------------+     +------------------+     +------------------+
| - etcd unavail   |     | - Stream not     |     | - Invalid args   |
| - Network timeout|     |   found          |     | - Unknown tool   |
| - Disk I/O error |     | - No data exists |     | - Malformed JSON |
| - Parse failure  |     | - Schema mismatch|     | - Missing field  |
+------------------+     +------------------+     +------------------+
        |                        |                        |
        v                        v                        v
    FAIL FAST              STRUCTURED              STRUCTURED
    (startup)               RESPONSE                RESPONSE
    or RETRY               (tool result)           (validation)
    (runtime)
```

---

## Error Type Hierarchy

### Algorithm

```
ALGORITHM: ErrorTypeDefinition
PURPOSE: Define comprehensive error types using thiserror pattern

// ============ Infrastructure Errors ============

ENUM EtcdError:
    ConnectionFailed(endpoints: String, cause: String)
        // Display: "Failed to connect to etcd at {endpoints}: {cause}"
        // Action: Fail fast at startup, retry with backoff at runtime

    Timeout(operation: String, duration_ms: u64)
        // Display: "etcd operation '{operation}' timed out after {duration_ms}ms"
        // Action: Return error to client, do not retry (etcd should be fast)

    KeyNotFound(key: String)
        // Display: "Key not found: {key}"
        // Action: May be expected, handle gracefully

    Unavailable(message: String)
        // Display: "etcd unavailable: {message}"
        // Action: Fail fast - no stale config for validation

    Internal(message: String)
        // Display: "etcd internal error: {message}"
        // Action: Log and return error to client

// ============ Storage Errors ============

ENUM StorageError:
    StreamNotFound(stream_id: String)
        // Display: "No data found for stream: {stream_id}"
        // Action: Return structured error with stream_id

    PartitionNotFound(stream_id: String, partition: String)
        // Display: "Partition not found: {stream_id}/{partition}"
        // Action: Return null storage stats in list_streams

    ParquetReadError(path: String, cause: String)
        // Display: "Failed to read Parquet file {path}: {cause}"
        // Action: Graceful degradation - skip file, continue if possible

    ParquetSchemaError(path: String, message: String)
        // Display: "Invalid Parquet schema in {path}: {message}"
        // Action: Return error with actionable context

    IoError(operation: String, path: String, cause: String)
        // Display: "I/O error during {operation} on {path}: {cause}"
        // Action: Log and return error

    JsonParseError(context: String, cause: String)
        // Display: "Failed to parse JSON in {context}: {cause}"
        // Action: Return error with sample of invalid JSON

// ============ Tool Errors ============

ENUM ToolError:
    NotFound(tool_name: String)
        // Display: "Unknown tool: {tool_name}"
        // Action: Return error with list of valid tools

    InvalidArguments(tool_name: String, message: String)
        // Display: "Invalid arguments for {tool_name}: {message}"
        // Action: Return error with expected schema

    StreamNotFound(stream_id: String)
        // Display: "Stream not found: {stream_id}"
        // Action: Return error suggesting list_streams

    NoDataAvailable(stream_id: String)
        // Display: "No Bronze data available for stream: {stream_id}"
        // Action: Return structured response (not error) with status

    ExecutionFailed(tool_name: String, cause: String)
        // Display: "Tool '{tool_name}' execution failed: {cause}"
        // Action: Log full error, return sanitized message

// ============ MCP Protocol Errors ============

ENUM McpError:
    InvalidRequest(message: String)
        // Display: "Invalid MCP request: {message}"
        // Action: Return MCP error response

    UnknownMethod(method: String)
        // Display: "Unknown MCP method: {method}"
        // Action: Return MCP error response

    MissingField(field: String)
        // Display: "Missing required field: {field}"
        // Action: Return MCP error response with schema

// ============ Configuration Errors ============

ENUM ConfigError:
    MissingRequired(name: String)
        // Display: "Missing required configuration: {name}"
        // Action: Fail fast at startup

    InvalidValue(name: String, value: String, expected: String)
        // Display: "Invalid configuration {name}={value}, expected {expected}"
        // Action: Fail fast at startup

    ParseError(name: String, cause: String)
        // Display: "Failed to parse configuration {name}: {cause}"
        // Action: Fail fast at startup
```

---

## Error Flow: etcd Unavailable

### Startup Behavior

```
ALGORITHM: HandleEtcdUnavailableAtStartup
INPUT: EtcdConfig
OUTPUT: EtcdClient OR panic

BEGIN
    // etcd is REQUIRED for MCP server operation
    // Config validation uses etcd as source of truth
    // Therefore: FAIL FAST if etcd is unavailable

    TRY
        client <- ConnectToEtcdWithRetry(config, max_retries=5)
        RETURN client

    CATCH EtcdError::ConnectionFailed(endpoints, cause):
        // Log detailed error for debugging
        tracing::error!(
            endpoints = %endpoints,
            cause = %cause,
            "Failed to connect to etcd - server cannot start"
        )

        // Exit with clear error message
        eprintln!("FATAL: Cannot connect to etcd at {}", endpoints)
        eprintln!("Cause: {}", cause)
        eprintln!("")
        eprintln!("Troubleshooting:")
        eprintln!("  1. Verify etcd is running: docker ps | grep etcd")
        eprintln!("  2. Check endpoint: curl {}/health", endpoints)
        eprintln!("  3. Verify NDP_ETCD_ENDPOINTS environment variable")

        std::process::exit(1)
    END TRY
END
```

### Runtime Behavior

```
ALGORITHM: HandleEtcdUnavailableAtRuntime
INPUT: EtcdError
OUTPUT: McpResponse (error)

BEGIN
    // During request handling, etcd errors should not crash server
    // But we DO NOT use stale/cached data - return error immediately

    MATCH error:
        EtcdError::Timeout(operation, duration):
            tracing::warn!(
                operation = %operation,
                duration_ms = duration,
                "etcd request timeout"
            )

            RETURN McpResponse::error({
                success: false,
                error: "Configuration service temporarily unavailable",
                code: "ETCD_TIMEOUT",
                retry_after_ms: 1000,
            })

        EtcdError::Unavailable(message):
            tracing::error!(message = %message, "etcd unavailable during request")

            RETURN McpResponse::error({
                success: false,
                error: "Configuration service unavailable",
                code: "ETCD_UNAVAILABLE",
                message: "Cannot validate configuration without etcd",
            })

        EtcdError::Internal(message):
            tracing::error!(message = %message, "etcd internal error")

            RETURN McpResponse::error({
                success: false,
                error: "Configuration service error",
                code: "ETCD_ERROR",
            })
    END MATCH
END

// Design Decision: NO CACHING of etcd data
//
// Rationale:
// 1. This MCP server validates config - stale config defeats the purpose
// 2. etcd should be fast (<10ms) - if it's slow, that's a problem to surface
// 3. Config changes should be immediately visible
// 4. Caching adds complexity (invalidation, consistency)
//
// If etcd is unavailable, we return an error rather than potentially
// misleading results from cached data.
```

---

## Error Flow: Stream Not Found

```
ALGORITHM: HandleStreamNotFound
INPUT: stream_id (string), operation (string)
OUTPUT: McpResponse (error)

BEGIN
    // Stream not found could mean:
    // 1. Typo in stream_id
    // 2. Stream not yet configured in etcd
    // 3. Stream disabled

    tracing::info!(stream_id = %stream_id, operation = %operation, "Stream not found")

    // Get list of available streams for helpful error message
    available_streams <- TRY EtcdClient.list_stream_ids() CATCH []

    RETURN McpResponse::error({
        success: false,
        error: "Stream not found: {stream_id}",
        code: "STREAM_NOT_FOUND",
        available_streams: available_streams,
        suggestion: "Use list_streams tool to see available streams",
    })
END
```

---

## Error Flow: Parquet Read Failure

```
ALGORITHM: HandleParquetReadFailure
INPUT: path (string), error (StorageError), context (string)
OUTPUT: Result (graceful degradation or error)

BEGIN
    MATCH context:
        "list_streams":
            // For listing, skip failed files and continue
            tracing::warn!(
                path = %path,
                error = %error,
                "Skipping corrupted Parquet file in list_streams"
            )

            // Return null storage stats for this stream
            RETURN Ok(StorageStats { status: "error", error: error.to_string() })

        "sample_data":
            // For sampling, we need the file - return error
            tracing::error!(
                path = %path,
                error = %error,
                "Cannot read Parquet file for sample_data"
            )

            RETURN McpResponse::error({
                success: false,
                error: "Failed to read data file",
                code: "PARQUET_READ_ERROR",
                file: path,
                cause: error.to_string(),
                suggestion: "File may be corrupted or incomplete. Check recent writes.",
            })

        "describe_schema":
            // For schema analysis, we need the file - return error
            RETURN McpResponse::error({
                success: false,
                error: "Cannot analyze schema - file read failed",
                code: "PARQUET_READ_ERROR",
                file: path,
                cause: error.to_string(),
            })

        "validate_config":
            // For validation, return status indicating we couldn't compare
            RETURN Ok(ValidationResult {
                status: "error",
                message: "Cannot validate - Bronze data unreadable",
                error: error.to_string(),
            })
    END MATCH
END
```

---

## Error Flow: JSON Parse Failure in raw_payload

```
ALGORITHM: HandleJsonParseFailure
INPUT: raw_payload_string (string), row_index (int), path (string)
OUTPUT: Handled row or error

BEGIN
    // raw_payload SHOULD always be valid JSON (written by our ingestion)
    // Parse failure indicates data corruption or bug

    tracing::error!(
        path = %path,
        row = row_index,
        payload_preview = %raw_payload_string.chars().take(100).collect::<String>(),
        "Invalid JSON in raw_payload column"
    )

    // Decision: Skip row, don't fail entire operation
    // Rationale: One bad row shouldn't prevent access to other data

    RETURN SkipRow {
        reason: "Invalid JSON in raw_payload",
        row_index: row_index,
        preview: raw_payload_string.chars().take(50).collect(),
    }
END

SUBROUTINE: HandleMultipleJsonParseFailures
INPUT: failures (Array<SkipRow>), total_rows (int)
OUTPUT: Warning or error

BEGIN
    failure_rate <- failures.length / total_rows

    IF failure_rate > 0.5 THEN
        // More than half the rows are corrupt - something is very wrong
        RETURN McpResponse::error({
            success: false,
            error: "Data corruption detected",
            code: "DATA_CORRUPTION",
            failed_rows: failures.length,
            total_rows: total_rows,
            message: "More than 50% of rows have invalid JSON. " +
                     "Check ingestion pipeline for bugs.",
        })
    ELSE IF failures.length > 0 THEN
        // Some rows failed but we have usable data
        tracing::warn!(
            failed = failures.length,
            total = total_rows,
            "Some rows skipped due to invalid JSON"
        )

        // Include warning in response
        RETURN Ok(ResultWithWarning {
            data: valid_rows,
            warning: {
                message: "{} of {} rows skipped due to invalid JSON",
                skipped_rows: failures,
            },
        })
    END IF

    RETURN Ok(valid_rows)
END
```

---

## MCP Error Response Format

```
ALGORITHM: BuildMcpErrorResponse
INPUT: error (any Error type)
OUTPUT: McpResponse

BEGIN
    // MCP spec: errors wrapped in content with isError flag

    error_payload <- MATCH error:
        ToolError::NotFound(name):
            {
                success: false,
                error: "Unknown tool: {name}",
                code: "UNKNOWN_TOOL",
                available_tools: get_tool_names(),
            }

        ToolError::InvalidArguments(tool, msg):
            {
                success: false,
                error: "Invalid arguments for {tool}: {msg}",
                code: "INVALID_ARGUMENTS",
                expected_schema: get_tool_schema(tool),
            }

        ToolError::StreamNotFound(id):
            {
                success: false,
                error: "Stream not found: {id}",
                code: "STREAM_NOT_FOUND",
            }

        ToolError::NoDataAvailable(id):
            // This is a structured response, NOT an error
            // Stream exists but has no Bronze data yet
            {
                success: true,  // Note: success=true
                stream_id: id,
                status: "no_data",
                message: "Stream is configured but no data has been ingested",
            }

        StorageError::ParquetReadError(path, cause):
            {
                success: false,
                error: "Failed to read data",
                code: "STORAGE_ERROR",
                details: cause,
            }

        EtcdError::Unavailable(_):
            {
                success: false,
                error: "Configuration service unavailable",
                code: "SERVICE_UNAVAILABLE",
                retry_after_seconds: 5,
            }

        _:
            // Generic fallback - log details, return sanitized message
            tracing::error!(error = %error, "Unhandled error type")
            {
                success: false,
                error: "Internal server error",
                code: "INTERNAL_ERROR",
            }
    END MATCH

    // Build MCP response structure
    RETURN McpResponse {
        content: [{
            type: "text",
            text: serde_json::to_string(error_payload),
        }],
        isError: NOT error_payload.success,
    }
END
```

---

## Error Logging Strategy

```
ALGORITHM: ErrorLogging
PURPOSE: Consistent logging for debugging while protecting sensitive data

BEGIN
    // Log levels by error severity:
    //
    // ERROR: Unexpected failures, needs investigation
    //   - etcd connection lost
    //   - Parquet corruption
    //   - Internal panics
    //
    // WARN: Expected failures, may need attention
    //   - etcd timeout (occasional)
    //   - Skipped corrupt rows
    //   - Invalid tool arguments
    //
    // INFO: Normal operation events
    //   - Stream not found (user error)
    //   - No data available
    //   - Request/response summary
    //
    // DEBUG: Detailed troubleshooting
    //   - Full request/response bodies
    //   - etcd key lookups
    //   - File path resolution

    SUBROUTINE LogError(error, context):
        // Structured logging with spans
        tracing::error!(
            error.type = %error.type_name(),
            error.message = %error.to_string(),
            context.operation = %context.operation,
            context.stream_id = ?context.stream_id,
            context.request_id = %context.request_id,
        )

        // DO NOT log:
        // - Raw API responses (may contain secrets)
        // - Full Parquet row contents
        // - etcd values (may contain credentials)
    END SUBROUTINE
END
```

---

## Recovery Patterns

### Pattern: Retry with Exponential Backoff

```
ALGORITHM: RetryWithBackoff
INPUT: operation (async fn), config (RetryConfig)
OUTPUT: Result<T, Error>

CONSTANTS:
    DEFAULT_MAX_RETRIES = 3
    DEFAULT_INITIAL_BACKOFF_MS = 100
    DEFAULT_MAX_BACKOFF_MS = 5000
    DEFAULT_BACKOFF_MULTIPLIER = 2.0
    DEFAULT_JITTER_FACTOR = 0.25

BEGIN
    backoff <- config.initial_backoff_ms
    last_error <- null

    FOR attempt IN 1..config.max_retries DO
        TRY
            result <- operation().await
            RETURN Ok(result)

        CATCH error IF is_retryable(error):
            last_error <- error

            IF attempt < config.max_retries THEN
                jitter <- random(-backoff * JITTER_FACTOR, backoff * JITTER_FACTOR)
                sleep_ms <- backoff + jitter

                tracing::debug!(
                    attempt = attempt,
                    backoff_ms = sleep_ms,
                    error = %error,
                    "Retrying after transient error"
                )

                tokio::time::sleep(Duration::from_millis(sleep_ms)).await
                backoff <- MIN(backoff * BACKOFF_MULTIPLIER, config.max_backoff_ms)
            END IF

        CATCH error IF NOT is_retryable(error):
            // Permanent error - don't retry
            RETURN Err(error)
        END TRY
    END FOR

    RETURN Err(last_error)
END

SUBROUTINE is_retryable
INPUT: error
OUTPUT: bool

BEGIN
    RETURN MATCH error:
        EtcdError::Timeout(_) => true,      // Network glitch
        EtcdError::Unavailable(_) => false, // Don't retry - fail fast
        StorageError::IoError(_, _, _) => false,  // Usually permanent
        _ => false,
    END MATCH
END
```

### Pattern: Circuit Breaker (Future Enhancement)

```
// For MVP, we use simple retry
// Future: Add circuit breaker for etcd if we see repeated failures

STRUCT CircuitBreaker:
    state: Closed | Open | HalfOpen
    failure_count: int
    last_failure_time: Instant
    failure_threshold: int = 5
    reset_timeout: Duration = 30s

// When circuit is Open:
// - Immediately return error without trying
// - After reset_timeout, move to HalfOpen
// - HalfOpen: Try one request, if success -> Closed, if fail -> Open
```

---

## Error Propagation Flow

```
                    +------------------+
                    |   MCP Request    |
                    +------------------+
                            |
                            v
                    +------------------+
                    | Request Handler  |  <-- Catches all errors
                    +------------------+       Returns McpResponse
                            |
              +-------------+-------------+
              |                           |
              v                           v
    +------------------+        +------------------+
    | Tool Execution   |        | Tool Registry    |
    +------------------+        +------------------+
              |                           |
              |   ToolError               | ToolError::NotFound
              v                           v
    +------------------+        +------------------+
    | etcd Client      |        | Bronze Storage   |
    +------------------+        +------------------+
              |                           |
              | EtcdError                 | StorageError
              v                           v
    +------------------+        +------------------+
    | etcd Server      |        | Filesystem       |
    +------------------+        +------------------+


Error Flow:
1. Low-level error occurs (etcd timeout, IO error)
2. Wrapped in typed error (EtcdError, StorageError)
3. Mapped to ToolError at tool layer
4. Converted to McpResponse at handler layer
5. Serialized to JSON and returned to client
```

---

## Testing Error Scenarios

```
ALGORITHM: ErrorTestCases
PURPOSE: Define test scenarios for error handling

TEST_CASES:

    // Infrastructure Errors
    test_etcd_connection_failure_at_startup:
        Setup: etcd not running
        Expected: Server fails to start with clear error message
        Verify: Exit code 1, stderr contains troubleshooting steps

    test_etcd_timeout_during_request:
        Setup: Mock etcd with 10s delay
        Expected: Tool returns ETCD_TIMEOUT error
        Verify: Response within timeout + small margin

    test_etcd_unavailable_during_request:
        Setup: Kill etcd after startup
        Expected: Tool returns ETCD_UNAVAILABLE error
        Verify: No stale/cached data returned

    // Application Errors
    test_stream_not_found:
        Setup: Request describe_schema("nonexistent-stream")
        Expected: STREAM_NOT_FOUND error with available streams list
        Verify: available_streams includes real streams

    test_no_bronze_data:
        Setup: Stream configured in etcd, but no Parquet files
        Expected: Success response with status="no_data"
        Verify: success=true (not an error condition)

    test_parquet_corruption:
        Setup: Write invalid bytes to data.parquet
        Expected: Graceful handling based on operation
        Verify: list_streams continues, sample_data returns error

    // Client Errors
    test_unknown_tool:
        Setup: Request tools/call with name="fake_tool"
        Expected: UNKNOWN_TOOL error
        Verify: available_tools in response

    test_invalid_arguments:
        Setup: sample_data with n=1000 (exceeds max)
        Expected: Value clamped to 100, no error
        Verify: row_count <= 100

    test_malformed_json:
        Setup: POST non-JSON to /mcp
        Expected: 400 Bad Request
        Verify: Body explains JSON parse error
```

---

## Summary: Error Handling Principles

1. **Fail Fast at Startup**: Missing etcd = no start. Don't run in degraded mode.

2. **No Stale Data**: Never cache etcd config. Return error rather than potentially wrong data.

3. **Graceful Degradation**: Skip corrupt rows/files when possible, but surface warnings.

4. **Structured Errors**: All errors include code, message, and actionable suggestions.

5. **Appropriate Logging**: ERROR for unexpected, WARN for recoverable, INFO for user errors.

6. **Security**: Never log sensitive data. Sanitize internal details in client responses.

---

*Pseudocode ready for implementation using thiserror crate and axum error handling.*
