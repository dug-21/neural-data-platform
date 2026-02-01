# dp-017: Integration Test Harness - Pseudocode

## Overview

This document defines the algorithmic design for the integration test harness that validates deploy.sh commands work correctly in integration mode. The harness is designed for local testing and future CI/CD integration.

---

## 1. Main Test Harness Algorithm

### ALGORITHM: run_integration_tests

```
ALGORITHM: run_integration_tests
INPUT: test_filter (optional string), verbose (boolean), cleanup_on_failure (boolean)
OUTPUT: test_results (TestReport object)

CONSTANTS:
    DEFAULT_TIMEOUT = 300 seconds
    COMPOSE_FILE = "docker-compose.integration.yml"
    DEPLOY_ENV = "integration"
    ETCD_CONTAINER = "integration-etcd"
    TIMESCALE_CONTAINER = "integration-timescaledb"
    MOSQUITTO_CONTAINER = "integration-mosquitto"
    AIR_QUALITY_CONTAINER = "integration-air-quality"
    MCP_CONTAINER = "integration-mcp-server"

DATA STRUCTURES:
    TestResult:
        name: string
        status: enum (PASS, FAIL, SKIP, TIMEOUT)
        duration_ms: integer
        error_message: string (optional)
        details: map<string, any>

    TestReport:
        start_time: timestamp
        end_time: timestamp
        total_tests: integer
        passed: integer
        failed: integer
        skipped: integer
        results: array of TestResult
        environment: map<string, string>

BEGIN
    // Initialize test report
    report <- new TestReport()
    report.start_time <- GetCurrentTimestamp()
    report.environment <- capture_environment()

    TRY
        // Phase 1: Setup
        log_phase("SETUP")
        ensure_clean_state()
        set_environment_variable("DEPLOY_ENV", "integration")

        // Phase 2: Infrastructure Tests
        log_phase("INFRASTRUCTURE")

        IF test_filter is null OR matches(test_filter, "deploy") THEN
            result <- test_deploy_command()
            report.results.append(result)
        END IF

        IF test_filter is null OR matches(test_filter, "status") THEN
            result <- test_status_command()
            report.results.append(result)
        END IF

        // Phase 3: Sync Command Tests
        log_phase("SYNC COMMANDS")

        IF test_filter is null OR matches(test_filter, "sync") THEN
            result <- test_sync_commands()
            report.results.append(result)
        END IF

        IF test_filter is null OR matches(test_filter, "init-streams") THEN
            result <- test_init_streams()
            report.results.append(result)
        END IF

        IF test_filter is null OR matches(test_filter, "sync-dictionary") THEN
            result <- test_sync_dictionary()
            report.results.append(result)
        END IF

        // Phase 4: Data Flow Tests
        log_phase("DATA FLOW")

        IF test_filter is null OR matches(test_filter, "data-flow") THEN
            result <- test_data_flow()
            report.results.append(result)
        END IF

        // Phase 5: Cleanup and Report
        log_phase("CLEANUP")
        cleanup()

    CATCH error
        log_error("Test harness failed: " + error.message)
        IF cleanup_on_failure THEN
            cleanup()
        END IF
        report.results.append(create_failure_result("harness", error.message))
    END TRY

    // Finalize report
    report.end_time <- GetCurrentTimestamp()
    report <- calculate_summary(report)
    report_results(report)

    RETURN report
END
```

---

## 2. Individual Test Functions

### 2.1 ALGORITHM: test_deploy_command

```
ALGORITHM: test_deploy_command
INPUT: none
OUTPUT: TestResult

CONSTANTS:
    DEPLOY_TIMEOUT = 300 seconds
    EXPECTED_SERVICES = ["mosquitto", "etcd", "timescaledb", "air-quality-app", "ndp-mcp-server"]

BEGIN
    result <- new TestResult(name: "deploy_command")
    start_time <- GetCurrentTimestamp()

    TRY
        // Step 1: Run deploy command
        log_info("Running: DEPLOY_ENV=integration ./deploy.sh deploy")

        exit_code <- execute_with_timeout(
            command: "cd /workspaces/neural-data-platform && DEPLOY_ENV=integration ./deploy/pi/deploy.sh deploy",
            timeout: DEPLOY_TIMEOUT,
            capture_output: true
        )

        IF exit_code != 0 THEN
            result.status <- FAIL
            result.error_message <- "Deploy command exited with code " + exit_code
            RETURN result
        END IF

        // Step 2: Verify all expected services are running
        log_info("Verifying services are running...")

        FOR EACH service IN EXPECTED_SERVICES DO
            container_name <- get_integration_container_name(service)
            is_running <- docker_container_running(container_name)

            IF NOT is_running THEN
                result.status <- FAIL
                result.error_message <- "Service not running: " + service
                RETURN result
            END IF
        END FOR

        // Step 3: Verify services become healthy
        log_info("Waiting for services to become healthy...")

        FOR EACH service IN EXPECTED_SERVICES DO
            container_name <- get_integration_container_name(service)
            healthy <- wait_for_healthy(container_name, timeout: 120)

            IF NOT healthy THEN
                result.status <- FAIL
                result.error_message <- "Service did not become healthy: " + service
                result.details["unhealthy_logs"] <- get_container_logs(container_name, tail: 50)
                RETURN result
            END IF
        END FOR

        // Success
        result.status <- PASS
        result.details["services"] <- EXPECTED_SERVICES

    CATCH TimeoutException
        result.status <- TIMEOUT
        result.error_message <- "Deploy command timed out after " + DEPLOY_TIMEOUT + " seconds"

    CATCH error
        result.status <- FAIL
        result.error_message <- error.message
    END TRY

    result.duration_ms <- GetCurrentTimestamp() - start_time
    RETURN result
END

SUBROUTINE: get_integration_container_name
INPUT: service_name (string)
OUTPUT: container_name (string)

BEGIN
    // Map service names to integration container names
    mapping <- {
        "mosquitto": "integration-mosquitto",
        "etcd": "integration-etcd",
        "timescaledb": "integration-timescaledb",
        "air-quality-app": "integration-air-quality",
        "ndp-mcp-server": "integration-mcp-server",
        "grafana": "integration-grafana"
    }

    RETURN mapping[service_name] OR "integration-" + service_name
END
```

### 2.2 ALGORITHM: test_status_command

```
ALGORITHM: test_status_command
INPUT: none
OUTPUT: TestResult

BEGIN
    result <- new TestResult(name: "status_command")
    start_time <- GetCurrentTimestamp()

    TRY
        // Run status command
        log_info("Running: DEPLOY_ENV=integration ./deploy.sh status")

        output, exit_code <- execute_capture_output(
            command: "cd /workspaces/neural-data-platform && DEPLOY_ENV=integration ./deploy/pi/deploy.sh status"
        )

        IF exit_code != 0 THEN
            result.status <- FAIL
            result.error_message <- "Status command exited with code " + exit_code
            result.details["output"] <- output
            RETURN result
        END IF

        // Verify expected output sections exist
        expected_sections <- [
            "Service Status",
            "Health Checks",
            "Silver Layer Status",
            "Useful URLs"
        ]

        FOR EACH section IN expected_sections DO
            IF NOT contains(output, section) THEN
                result.status <- FAIL
                result.error_message <- "Missing expected section: " + section
                result.details["output"] <- output
                RETURN result
            END IF
        END FOR

        // Verify key services show as healthy/running
        health_indicators <- [
            "etcd",
            "TimescaleDB",
            "Air Quality"
        ]

        FOR EACH indicator IN health_indicators DO
            IF NOT contains(output, indicator) THEN
                result.status <- FAIL
                result.error_message <- "Health check missing for: " + indicator
                RETURN result
            END IF
        END FOR

        result.status <- PASS
        result.details["output_preview"] <- truncate(output, 500)

    CATCH error
        result.status <- FAIL
        result.error_message <- error.message
    END TRY

    result.duration_ms <- GetCurrentTimestamp() - start_time
    RETURN result
END
```

### 2.3 ALGORITHM: test_sync_commands

```
ALGORITHM: test_sync_commands
INPUT: none
OUTPUT: TestResult

BEGIN
    result <- new TestResult(name: "sync_commands")
    start_time <- GetCurrentTimestamp()

    TRY
        // Test 1: Sync config to etcd
        log_info("Testing: ./deploy.sh sync")

        output, exit_code <- execute_capture_output(
            command: "cd /workspaces/neural-data-platform && DEPLOY_ENV=integration ./deploy/pi/deploy.sh sync"
        )

        IF exit_code != 0 THEN
            result.status <- FAIL
            result.error_message <- "Sync command failed with exit code " + exit_code
            result.details["sync_output"] <- output
            RETURN result
        END IF

        // Verify etcd has configuration data
        log_info("Verifying etcd has configuration...")

        // Check for air-quality namespace
        has_config <- assert_etcd_has_key("/air-quality/")

        IF NOT has_config THEN
            result.status <- FAIL
            result.error_message <- "etcd missing /air-quality/ configuration after sync"
            RETURN result
        END IF

        // Check for environment-specific config
        env_config <- get_etcd_key("/ndp/environment")
        result.details["environment"] <- env_config

        result.status <- PASS
        result.details["sync_completed"] <- true

    CATCH error
        result.status <- FAIL
        result.error_message <- error.message
    END TRY

    result.duration_ms <- GetCurrentTimestamp() - start_time
    RETURN result
END
```

### 2.4 ALGORITHM: test_init_streams

```
ALGORITHM: test_init_streams
INPUT: none
OUTPUT: TestResult

CONSTANTS:
    EXPECTED_DEFAULT_STREAMS = ["airgradient-001", "airgradient-002"]

BEGIN
    result <- new TestResult(name: "init_streams")
    start_time <- GetCurrentTimestamp()

    TRY
        // Run init-streams command
        log_info("Testing: ./deploy.sh init-streams")

        output, exit_code <- execute_capture_output(
            command: "cd /workspaces/neural-data-platform && DEPLOY_ENV=integration ./deploy/pi/deploy.sh init-streams"
        )

        IF exit_code != 0 THEN
            result.status <- FAIL
            result.error_message <- "init-streams failed with exit code " + exit_code
            result.details["output"] <- output
            RETURN result
        END IF

        // Verify streams exist in etcd
        log_info("Verifying streams in etcd...")

        streams_found <- []

        FOR EACH stream_id IN EXPECTED_DEFAULT_STREAMS DO
            key <- "/air-quality/streams/" + stream_id + "/id"
            exists <- assert_etcd_has_key(key)

            IF exists THEN
                streams_found.append(stream_id)

                // Verify stream metadata
                name_key <- "/air-quality/streams/" + stream_id + "/name"
                topic_key <- "/air-quality/streams/" + stream_id + "/mqtt_topic"
                enabled_key <- "/air-quality/streams/" + stream_id + "/enabled"

                stream_name <- get_etcd_key(name_key)
                mqtt_topic <- get_etcd_key(topic_key)
                enabled <- get_etcd_key(enabled_key)

                result.details[stream_id] <- {
                    "name": stream_name,
                    "mqtt_topic": mqtt_topic,
                    "enabled": enabled
                }
            END IF
        END FOR

        IF length(streams_found) == 0 THEN
            result.status <- FAIL
            result.error_message <- "No streams found in etcd after init-streams"
            RETURN result
        END IF

        // Verify multi-stream configuration
        multi_enabled <- get_etcd_key("/air-quality/multi_stream/enabled")

        IF multi_enabled != "true" THEN
            result.status <- FAIL
            result.error_message <- "Multi-stream not enabled after init-streams"
            RETURN result
        END IF

        result.status <- PASS
        result.details["streams_found"] <- streams_found
        result.details["multi_stream_enabled"] <- true

    CATCH error
        result.status <- FAIL
        result.error_message <- error.message
    END TRY

    result.duration_ms <- GetCurrentTimestamp() - start_time
    RETURN result
END
```

### 2.5 ALGORITHM: test_sync_dictionary

```
ALGORITHM: test_sync_dictionary
INPUT: none
OUTPUT: TestResult

CONSTANTS:
    EXPECTED_TABLES = [
        "data_dictionary.streams",
        "data_dictionary.entity_schemas",
        "data_dictionary.silver_tables",
        "data_dictionary.silver_columns"
    ]

BEGIN
    result <- new TestResult(name: "sync_dictionary")
    start_time <- GetCurrentTimestamp()

    TRY
        // Run sync-dictionary command
        log_info("Testing: ./deploy.sh sync-dictionary")

        output, exit_code <- execute_capture_output(
            command: "cd /workspaces/neural-data-platform && DEPLOY_ENV=integration ./deploy/pi/deploy.sh sync-dictionary"
        )

        IF exit_code != 0 THEN
            result.status <- FAIL
            result.error_message <- "sync-dictionary failed with exit code " + exit_code
            result.details["output"] <- output
            RETURN result
        END IF

        // Verify data dictionary tables have data
        log_info("Verifying data dictionary tables...")

        FOR EACH table IN EXPECTED_TABLES DO
            count <- query_timescale_count(table)
            result.details[table + "_count"] <- count

            // streams table should have at least 1 entry
            IF table == "data_dictionary.streams" AND count == 0 THEN
                result.status <- FAIL
                result.error_message <- "No streams synced to data dictionary"
                RETURN result
            END IF
        END FOR

        // Verify sync_status shows success
        sync_status <- query_timescale(
            "SELECT status, streams_synced, schemas_synced
             FROM data_dictionary.sync_status
             ORDER BY id DESC LIMIT 1"
        )

        IF sync_status.status != "success" THEN
            result.status <- FAIL
            result.error_message <- "Sync status not 'success': " + sync_status.status
            RETURN result
        END IF

        result.status <- PASS
        result.details["sync_status"] <- sync_status

    CATCH error
        result.status <- FAIL
        result.error_message <- error.message
    END TRY

    result.duration_ms <- GetCurrentTimestamp() - start_time
    RETURN result
END
```

### 2.6 ALGORITHM: test_data_flow

```
ALGORITHM: test_data_flow
INPUT: none
OUTPUT: TestResult

CONSTANTS:
    TEST_TOPIC = "airgradient/test/measures"
    BRONZE_WAIT_TIMEOUT = 30 seconds
    SILVER_WAIT_TIMEOUT = 60 seconds

DATA STRUCTURES:
    TestMessage:
        wifi: integer
        pm02: integer
        rco2: integer
        atmp: float
        rhum: float
        timestamp: string

BEGIN
    result <- new TestResult(name: "data_flow")
    start_time <- GetCurrentTimestamp()

    TRY
        // Step 1: Create test message with unique identifier
        test_id <- generate_uuid()
        test_timestamp <- GetCurrentTimestamp()

        test_message <- new TestMessage(
            wifi: -50,
            pm02: 15,
            rco2: 650,
            atmp: 22.5,
            rhum: 55.0,
            timestamp: format_iso8601(test_timestamp)
        )

        log_info("Injecting test message with ID: " + test_id)
        result.details["test_message"] <- test_message

        // Step 2: Inject MQTT message
        inject_mqtt_message(TEST_TOPIC, to_json(test_message))

        // Step 3: Verify Bronze layer received data
        log_info("Waiting for Bronze layer to persist data...")

        bronze_verified <- wait_for_condition(
            condition: verify_bronze_parquet("air-quality", test_timestamp),
            timeout: BRONZE_WAIT_TIMEOUT,
            poll_interval: 2 seconds
        )

        IF NOT bronze_verified THEN
            result.status <- FAIL
            result.error_message <- "Data not found in Bronze layer within timeout"
            result.details["bronze_timeout"] <- BRONZE_WAIT_TIMEOUT
            RETURN result
        END IF

        result.details["bronze_verified"] <- true
        result.details["bronze_latency_ms"] <- GetCurrentTimestamp() - test_timestamp

        // Step 4: Verify Silver layer received data (if ETL is enabled)
        log_info("Waiting for Silver layer ETL...")

        silver_verified <- wait_for_condition(
            condition: verify_silver_row("silver.air_quality_readings", test_timestamp),
            timeout: SILVER_WAIT_TIMEOUT,
            poll_interval: 5 seconds
        )

        IF NOT silver_verified THEN
            // Silver ETL may not be running in all test modes
            log_warning("Silver layer data not found - ETL may not be active")
            result.details["silver_verified"] <- false
            result.details["silver_note"] <- "ETL may require daemon mode"
        ELSE
            result.details["silver_verified"] <- true
            result.details["silver_latency_ms"] <- GetCurrentTimestamp() - test_timestamp
        END IF

        result.status <- PASS

    CATCH error
        result.status <- FAIL
        result.error_message <- error.message
    END TRY

    result.duration_ms <- GetCurrentTimestamp() - start_time
    RETURN result
END
```

---

## 3. Helper Functions

### 3.1 ALGORITHM: ensure_clean_state

```
ALGORITHM: ensure_clean_state
INPUT: none
OUTPUT: success (boolean)

CONSTANTS:
    INTEGRATION_CONTAINERS = [
        "integration-mosquitto",
        "integration-etcd",
        "integration-timescaledb",
        "integration-air-quality",
        "integration-mcp-server",
        "integration-grafana"
    ]
    INTEGRATION_VOLUMES = [
        "neural-data-platform_mosquitto-data",
        "neural-data-platform_mosquitto-logs",
        "neural-data-platform_etcd-data",
        "neural-data-platform_timescaledb-data",
        "neural-data-platform_bronze-data",
        "neural-data-platform_grafana-data"
    ]
    COMPOSE_FILE = "docker-compose.integration.yml"

BEGIN
    log_info("Ensuring clean state for integration tests...")

    TRY
        // Step 1: Stop any running integration containers
        log_info("Stopping existing integration containers...")

        execute_command(
            "docker compose -f " + COMPOSE_FILE + " down --timeout 10"
        )

        // Step 2: Remove orphaned containers by name
        FOR EACH container IN INTEGRATION_CONTAINERS DO
            IF docker_container_exists(container) THEN
                log_info("Removing orphaned container: " + container)
                execute_command("docker rm -f " + container)
            END IF
        END FOR

        // Step 3: Remove volumes for clean slate
        log_info("Removing integration volumes for clean slate...")

        FOR EACH volume IN INTEGRATION_VOLUMES DO
            IF docker_volume_exists(volume) THEN
                execute_command("docker volume rm " + volume)
            END IF
        END FOR

        // Step 4: Prune any dangling resources
        execute_command("docker network prune -f --filter label!=keep")

        // Step 5: Verify clean state
        FOR EACH container IN INTEGRATION_CONTAINERS DO
            IF docker_container_exists(container) THEN
                log_error("Failed to remove container: " + container)
                RETURN false
            END IF
        END FOR

        log_info("Clean state verified")
        RETURN true

    CATCH error
        log_error("Failed to ensure clean state: " + error.message)
        RETURN false
    END TRY
END
```

### 3.2 ALGORITHM: wait_for_healthy

```
ALGORITHM: wait_for_healthy
INPUT: container_name (string), timeout (integer seconds)
OUTPUT: healthy (boolean)

CONSTANTS:
    POLL_INTERVAL = 5 seconds
    HEALTH_STATES = {
        "healthy": true,
        "unhealthy": false,
        "starting": null,  // continue waiting
        "none": null       // no healthcheck defined
    }

BEGIN
    start_time <- GetCurrentTimestamp()
    elapsed <- 0

    log_info("Waiting for " + container_name + " to become healthy (timeout: " + timeout + "s)...")

    WHILE elapsed < timeout DO
        TRY
            // Check if container is running
            IF NOT docker_container_running(container_name) THEN
                log_warning(container_name + " is not running, waiting...")
                sleep(POLL_INTERVAL)
                elapsed <- GetCurrentTimestamp() - start_time
                CONTINUE
            END IF

            // Get health status
            health_status <- get_container_health_status(container_name)

            CASE health_status OF
                "healthy":
                    log_info(container_name + " is healthy after " + elapsed + "s")
                    RETURN true

                "unhealthy":
                    log_warning(container_name + " reported unhealthy")
                    // Log recent container logs for debugging
                    logs <- get_container_logs(container_name, tail: 20)
                    log_debug("Recent logs: " + logs)
                    // Continue waiting - may recover

                "starting":
                    log_debug(container_name + " health check starting...")

                "none":
                    // No healthcheck defined - check if container is running
                    IF docker_container_running(container_name) THEN
                        log_info(container_name + " running (no healthcheck)")
                        RETURN true
                    END IF
            END CASE

        CATCH error
            log_warning("Error checking health: " + error.message)
        END TRY

        sleep(POLL_INTERVAL)
        elapsed <- GetCurrentTimestamp() - start_time
    END WHILE

    log_error(container_name + " did not become healthy within " + timeout + "s")
    RETURN false
END

SUBROUTINE: get_container_health_status
INPUT: container_name (string)
OUTPUT: status (string)

BEGIN
    // Use docker inspect to get health status
    result <- execute_capture_output(
        "docker inspect --format='{{.State.Health.Status}}' " + container_name
    )

    IF result is empty OR result contains "no such container" THEN
        RETURN "none"
    END IF

    RETURN trim(result)
END
```

### 3.3 ALGORITHM: assert_etcd_has_key

```
ALGORITHM: assert_etcd_has_key
INPUT: key_prefix (string)
OUTPUT: exists (boolean)

CONSTANTS:
    ETCD_CONTAINER = "integration-etcd"

BEGIN
    TRY
        // Use etcdctl to check if key exists
        command <- "docker exec " + ETCD_CONTAINER +
                   " etcdctl get --prefix '" + key_prefix + "' --keys-only --limit 1"

        output, exit_code <- execute_capture_output(command)

        IF exit_code != 0 THEN
            log_warning("etcdctl failed: " + output)
            RETURN false
        END IF

        // If output is non-empty, key exists
        RETURN length(trim(output)) > 0

    CATCH error
        log_error("Failed to check etcd key: " + error.message)
        RETURN false
    END TRY
END

SUBROUTINE: get_etcd_key
INPUT: key (string)
OUTPUT: value (string or null)

CONSTANTS:
    ETCD_CONTAINER = "integration-etcd"

BEGIN
    TRY
        command <- "docker exec " + ETCD_CONTAINER +
                   " etcdctl get '" + key + "' --print-value-only"

        output, exit_code <- execute_capture_output(command)

        IF exit_code != 0 OR length(trim(output)) == 0 THEN
            RETURN null
        END IF

        RETURN trim(output)

    CATCH error
        RETURN null
    END TRY
END
```

### 3.4 ALGORITHM: assert_timescale_has_table

```
ALGORITHM: assert_timescale_has_table
INPUT: table_name (string)
OUTPUT: exists (boolean)

CONSTANTS:
    TIMESCALE_CONTAINER = "integration-timescaledb"
    DB_NAME = "ndp"
    DB_USER = "postgres"

BEGIN
    TRY
        // Parse schema.table format
        IF contains(table_name, ".") THEN
            parts <- split(table_name, ".")
            schema <- parts[0]
            table <- parts[1]
        ELSE
            schema <- "public"
            table <- table_name
        END IF

        query <- "SELECT EXISTS (
            SELECT FROM information_schema.tables
            WHERE table_schema = '" + schema + "'
            AND table_name = '" + table + "'
        )"

        command <- "docker exec " + TIMESCALE_CONTAINER +
                   " psql -U " + DB_USER + " -d " + DB_NAME +
                   " -tAc \"" + query + "\""

        output, exit_code <- execute_capture_output(command)

        RETURN trim(output) == "t"

    CATCH error
        log_error("Failed to check table existence: " + error.message)
        RETURN false
    END TRY
END

SUBROUTINE: query_timescale_count
INPUT: table_name (string)
OUTPUT: count (integer)

CONSTANTS:
    TIMESCALE_CONTAINER = "integration-timescaledb"
    DB_NAME = "ndp"
    DB_USER = "postgres"

BEGIN
    TRY
        query <- "SELECT COUNT(*) FROM " + table_name

        command <- "docker exec " + TIMESCALE_CONTAINER +
                   " psql -U " + DB_USER + " -d " + DB_NAME +
                   " -tAc \"" + query + "\""

        output, exit_code <- execute_capture_output(command)

        IF exit_code != 0 THEN
            RETURN 0
        END IF

        RETURN parse_integer(trim(output))

    CATCH error
        RETURN 0
    END TRY
END

SUBROUTINE: query_timescale
INPUT: query (string)
OUTPUT: result (map or null)

CONSTANTS:
    TIMESCALE_CONTAINER = "integration-timescaledb"
    DB_NAME = "ndp"
    DB_USER = "postgres"

BEGIN
    TRY
        command <- "docker exec " + TIMESCALE_CONTAINER +
                   " psql -U " + DB_USER + " -d " + DB_NAME +
                   " -tAc \"" + query + "\""

        output, exit_code <- execute_capture_output(command)

        IF exit_code != 0 THEN
            RETURN null
        END IF

        // Parse pipe-delimited output into map
        RETURN parse_psql_output(output)

    CATCH error
        RETURN null
    END TRY
END
```

### 3.5 ALGORITHM: inject_mqtt_message

```
ALGORITHM: inject_mqtt_message
INPUT: topic (string), payload (string)
OUTPUT: success (boolean)

CONSTANTS:
    MOSQUITTO_CONTAINER = "integration-mosquitto"
    MQTT_PORT = 1883

BEGIN
    TRY
        log_info("Publishing to " + topic + ": " + truncate(payload, 100))

        // Use mosquitto_pub inside the container or from host
        // Option 1: From container
        command <- "docker exec " + MOSQUITTO_CONTAINER +
                   " mosquitto_pub -h localhost -p " + MQTT_PORT +
                   " -t '" + topic + "'" +
                   " -m '" + escape_shell(payload) + "'"

        output, exit_code <- execute_capture_output(command)

        IF exit_code != 0 THEN
            // Fallback: Try from host if mosquitto_pub is installed
            command <- "mosquitto_pub -h localhost -p " + MQTT_PORT +
                       " -t '" + topic + "'" +
                       " -m '" + escape_shell(payload) + "'"

            output, exit_code <- execute_capture_output(command)
        END IF

        IF exit_code != 0 THEN
            log_error("Failed to publish MQTT message: " + output)
            RETURN false
        END IF

        log_info("Message published successfully")
        RETURN true

    CATCH error
        log_error("MQTT publish error: " + error.message)
        RETURN false
    END TRY
END
```

### 3.6 ALGORITHM: verify_bronze_parquet

```
ALGORITHM: verify_bronze_parquet
INPUT: stream_id (string), since_timestamp (timestamp)
OUTPUT: found (boolean)

CONSTANTS:
    BRONZE_DATA_PATH = "/data/raw"
    AIR_QUALITY_CONTAINER = "integration-air-quality"

BEGIN
    TRY
        // Bronze layer stores data in date-partitioned directories
        // Pattern: /data/raw/{stream_id}/YYYY/MM/DD/*.parquet

        date_path <- format_date(since_timestamp, "YYYY/MM/DD")
        search_path <- BRONZE_DATA_PATH + "/" + stream_id + "/" + date_path

        // Check if any Parquet files exist in the expected path
        command <- "docker exec " + AIR_QUALITY_CONTAINER +
                   " find " + search_path + " -name '*.parquet' -type f 2>/dev/null | head -1"

        output, exit_code <- execute_capture_output(command)

        IF length(trim(output)) > 0 THEN
            log_info("Found Bronze Parquet at: " + trim(output))
            RETURN true
        END IF

        // Also check for WAL files (data may not be flushed yet)
        wal_path <- BRONZE_DATA_PATH + "/" + stream_id + "/wal"
        command <- "docker exec " + AIR_QUALITY_CONTAINER +
                   " ls -la " + wal_path + " 2>/dev/null | grep -c '.wal' || echo 0"

        output, exit_code <- execute_capture_output(command)
        wal_count <- parse_integer(trim(output))

        IF wal_count > 0 THEN
            log_info("Found " + wal_count + " WAL files (data pending flush)")
            RETURN true
        END IF

        RETURN false

    CATCH error
        log_debug("Bronze verification error: " + error.message)
        RETURN false
    END TRY
END
```

### 3.7 ALGORITHM: verify_silver_row

```
ALGORITHM: verify_silver_row
INPUT: table_name (string), since_timestamp (timestamp)
OUTPUT: found (boolean)

CONSTANTS:
    TIMESCALE_CONTAINER = "integration-timescaledb"
    DB_NAME = "ndp"
    DB_USER = "postgres"

BEGIN
    TRY
        // Query Silver table for recent data
        // Assumes table has observation_time or timestamp column

        timestamp_str <- format_iso8601(since_timestamp)

        query <- "SELECT COUNT(*) FROM " + table_name +
                 " WHERE observation_time >= '" + timestamp_str + "'"

        command <- "docker exec " + TIMESCALE_CONTAINER +
                   " psql -U " + DB_USER + " -d " + DB_NAME +
                   " -tAc \"" + query + "\""

        output, exit_code <- execute_capture_output(command)

        IF exit_code != 0 THEN
            log_debug("Silver query failed: " + output)
            RETURN false
        END IF

        count <- parse_integer(trim(output))

        IF count > 0 THEN
            log_info("Found " + count + " rows in Silver table since " + timestamp_str)
            RETURN true
        END IF

        RETURN false

    CATCH error
        log_debug("Silver verification error: " + error.message)
        RETURN false
    END TRY
END
```

---

## 4. Cleanup and Reporting

### 4.1 ALGORITHM: cleanup

```
ALGORITHM: cleanup
INPUT: preserve_volumes (boolean, default: false)
OUTPUT: success (boolean)

CONSTANTS:
    COMPOSE_FILE = "docker-compose.integration.yml"

BEGIN
    log_info("Cleaning up integration environment...")

    TRY
        // Stop all containers gracefully
        log_info("Stopping containers...")

        IF preserve_volumes THEN
            execute_command(
                "docker compose -f " + COMPOSE_FILE + " down --timeout 30"
            )
        ELSE
            // Remove volumes as well for full cleanup
            execute_command(
                "docker compose -f " + COMPOSE_FILE + " down -v --timeout 30"
            )
        END IF

        // Prune unused networks
        execute_command("docker network prune -f --filter label!=keep")

        log_info("Cleanup complete")
        RETURN true

    CATCH error
        log_warning("Cleanup encountered error: " + error.message)
        // Force cleanup
        execute_command(
            "docker compose -f " + COMPOSE_FILE + " down -v --timeout 5"
        )
        RETURN false
    END TRY
END
```

### 4.2 ALGORITHM: report_results

```
ALGORITHM: report_results
INPUT: report (TestReport)
OUTPUT: none (prints to stdout)

CONSTANTS:
    STATUS_COLORS = {
        "PASS": GREEN,
        "FAIL": RED,
        "SKIP": YELLOW,
        "TIMEOUT": ORANGE
    }

BEGIN
    // Print header
    print_separator("=", 70)
    print_centered("INTEGRATION TEST RESULTS")
    print_separator("=", 70)

    // Print environment info
    print_section("Environment")
    print_key_value("DEPLOY_ENV", report.environment["DEPLOY_ENV"])
    print_key_value("Started", format_timestamp(report.start_time))
    print_key_value("Ended", format_timestamp(report.end_time))
    print_key_value("Duration", format_duration(report.end_time - report.start_time))

    // Print individual test results
    print_section("Test Results")
    print_separator("-", 70)

    FOR EACH result IN report.results DO
        color <- STATUS_COLORS[result.status]
        status_str <- colorize(result.status, color)
        duration_str <- format_duration_ms(result.duration_ms)

        print_row(result.name, status_str, duration_str)

        IF result.status == FAIL THEN
            print_indented("Error: " + result.error_message, indent: 4)
        END IF
    END FOR

    print_separator("-", 70)

    // Print summary
    print_section("Summary")
    print_key_value("Total Tests", report.total_tests)
    print_key_value("Passed", colorize(report.passed, GREEN))
    print_key_value("Failed", colorize(report.failed, RED))
    print_key_value("Skipped", colorize(report.skipped, YELLOW))

    // Calculate pass rate
    pass_rate <- (report.passed / report.total_tests) * 100
    print_key_value("Pass Rate", format_percentage(pass_rate))

    print_separator("=", 70)

    // Exit status indication
    IF report.failed > 0 THEN
        print_colored("TESTS FAILED", RED)
    ELSE
        print_colored("ALL TESTS PASSED", GREEN)
    END IF

    // Output machine-readable report if requested
    IF environment_variable("TEST_OUTPUT_FORMAT") == "json" THEN
        output_json_report(report)
    END IF
END
```

---

## 5. Error Handling Strategy

### 5.1 Timeout Handling

```
ALGORITHM: execute_with_timeout
INPUT: command (string), timeout (integer seconds), capture_output (boolean)
OUTPUT: result (CommandResult)

DATA STRUCTURES:
    CommandResult:
        exit_code: integer
        stdout: string
        stderr: string
        timed_out: boolean

BEGIN
    result <- new CommandResult()

    // Create background process
    process <- start_background_process(command)

    // Wait with timeout
    elapsed <- 0
    WHILE process.is_running() AND elapsed < timeout DO
        sleep(1)
        elapsed <- elapsed + 1
    END WHILE

    IF process.is_running() THEN
        // Timeout occurred
        log_warning("Command timed out after " + timeout + "s, killing process")
        process.kill()
        result.timed_out <- true
        result.exit_code <- -1
        result.stderr <- "Command timed out after " + timeout + " seconds"
    ELSE
        result.timed_out <- false
        result.exit_code <- process.exit_code()

        IF capture_output THEN
            result.stdout <- process.stdout()
            result.stderr <- process.stderr()
        END IF
    END IF

    RETURN result
END
```

### 5.2 Cleanup on Failure

```
ALGORITHM: run_with_cleanup_on_failure
INPUT: test_function (function), cleanup_function (function)
OUTPUT: TestResult

BEGIN
    TRY
        result <- test_function()
        RETURN result

    CATCH error
        log_error("Test failed with error: " + error.message)

        TRY
            log_info("Performing cleanup after failure...")
            cleanup_function()
        CATCH cleanup_error
            log_warning("Cleanup also failed: " + cleanup_error.message)
        END TRY

        // Re-throw original error
        THROW error
    END TRY
END
```

### 5.3 Detailed Error Reporting

```
ALGORITHM: create_failure_result
INPUT: test_name (string), error_message (string), context (map, optional)
OUTPUT: TestResult

BEGIN
    result <- new TestResult()
    result.name <- test_name
    result.status <- FAIL
    result.error_message <- error_message

    // Capture debugging context
    result.details["error_time"] <- GetCurrentTimestamp()
    result.details["error_type"] <- classify_error(error_message)

    IF context is not null THEN
        result.details.merge(context)
    END IF

    // Capture container states for debugging
    result.details["container_states"] <- capture_container_states()

    // Capture recent logs from key containers
    result.details["etcd_logs"] <- get_container_logs("integration-etcd", tail: 20)
    result.details["timescale_logs"] <- get_container_logs("integration-timescaledb", tail: 20)
    result.details["app_logs"] <- get_container_logs("integration-air-quality", tail: 50)

    RETURN result
END

SUBROUTINE: classify_error
INPUT: error_message (string)
OUTPUT: error_type (string)

BEGIN
    IF contains(error_message, "timeout") THEN
        RETURN "TIMEOUT"
    ELSE IF contains(error_message, "connection refused") THEN
        RETURN "CONNECTION_ERROR"
    ELSE IF contains(error_message, "not found") THEN
        RETURN "NOT_FOUND"
    ELSE IF contains(error_message, "permission denied") THEN
        RETURN "PERMISSION_ERROR"
    ELSE
        RETURN "UNKNOWN"
    END IF
END
```

---

## 6. Complexity Analysis

### Time Complexity

| Function | Complexity | Notes |
|----------|------------|-------|
| run_integration_tests | O(n * t) | n = number of tests, t = average test time |
| test_deploy_command | O(s * h) | s = services, h = health check timeout |
| wait_for_healthy | O(t / p) | t = timeout, p = poll interval |
| ensure_clean_state | O(c + v) | c = containers, v = volumes |
| verify_bronze_parquet | O(1) | Single file system check |
| verify_silver_row | O(log n) | Index-optimized query |

### Space Complexity

| Component | Space | Notes |
|-----------|-------|-------|
| TestReport | O(n) | n = number of test results |
| Container logs | O(l * t) | l = log lines, t = tail count |
| Error context | O(1) | Fixed size debug info |

---

## 7. Implementation Notes

### File Organization

```
tests/
  integration/
    integration-test.sh       # Main test harness (implements run_integration_tests)
    lib/
      docker-helpers.sh       # Docker utility functions
      etcd-helpers.sh         # etcd assertion functions
      timescale-helpers.sh    # TimescaleDB assertion functions
      mqtt-helpers.sh         # MQTT injection functions
      reporting.sh            # Test reporting utilities
    tests/
      test-deploy.sh          # test_deploy_command
      test-status.sh          # test_status_command
      test-sync.sh            # test_sync_commands
      test-init-streams.sh    # test_init_streams
      test-sync-dictionary.sh # test_sync_dictionary
      test-data-flow.sh       # test_data_flow
```

### Environment Variables

```
TEST_FILTER         # Optional: run only matching tests
TEST_VERBOSE        # Enable verbose output
TEST_CLEANUP_ON_FAIL # Cleanup even on failure (default: true)
TEST_TIMEOUT        # Global timeout override
TEST_OUTPUT_FORMAT  # "text" (default) or "json"
TEST_PRESERVE_VOLUMES # Keep volumes after test (for debugging)
```

### Exit Codes

| Code | Meaning |
|------|---------|
| 0 | All tests passed |
| 1 | One or more tests failed |
| 2 | Test harness error (setup/cleanup failed) |
| 3 | Timeout occurred |
| 4 | Prerequisites not met |

---

## 8. Design Patterns Used

1. **Template Method Pattern**: run_integration_tests defines the skeleton, individual tests fill in specifics
2. **Strategy Pattern**: Configurable cleanup behavior (preserve_volumes)
3. **Observer Pattern**: Test results collected and reported after all tests complete
4. **Factory Pattern**: create_failure_result constructs standardized error results
5. **Command Pattern**: Each test is encapsulated as an executable unit

---

## 9. Future Enhancements

1. **Parallel Test Execution**: Run independent tests concurrently
2. **Test Fixtures**: Reusable setup/teardown for test groups
3. **Retry Logic**: Automatic retry for flaky tests
4. **CI/CD Integration**: GitHub Actions workflow generation
5. **Performance Metrics**: Track test duration trends
6. **Coverage Reporting**: Map tests to deploy.sh commands covered

---

*Document Version: 1.0*
*Created: 2026-02-01*
*SPARC Phase: Pseudocode*
