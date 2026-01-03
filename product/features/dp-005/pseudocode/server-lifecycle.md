# Server Lifecycle - Bronze MCP Server

## Overview

This document defines the server startup, configuration, request handling, and graceful shutdown algorithms for the Bronze MCP Server.

---

## Lifecycle Diagram

```
                    +-----------------+
                    |   Process Start |
                    +-----------------+
                            |
                            v
                    +-----------------+
                    | Load Config     |
                    | from Env        |
                    +-----------------+
                            |
                            v
                    +-----------------+
                    | Initialize      |
                    | Tracing/Logging |
                    +-----------------+
                            |
                            v
                    +-----------------+
                    | Connect to etcd |<----+
                    +-----------------+     |
                            |               | Retry with
                            | Success?      | backoff
                            +--------->-----+
                            | Yes
                            v
                    +-----------------+
                    | Validate Config |
                    | (etcd readable) |
                    +-----------------+
                            |
                            v
                    +-----------------+
                    | Build App State |
                    | - EtcdClient    |
                    | - BronzeStorage |
                    | - ToolRegistry  |
                    +-----------------+
                            |
                            v
                    +-----------------+
                    | Setup Axum      |
                    | Routes          |
                    +-----------------+
                            |
                            v
                    +-----------------+
                    | Bind Listener   |
                    +-----------------+
                            |
                            v
              +-------------+-------------+
              |                           |
              v                           v
    +-----------------+         +-----------------+
    | Serve Requests  |<------->| Signal Handler  |
    | (event loop)    |         | (SIGTERM/INT)   |
    +-----------------+         +-----------------+
              |                           |
              |   Shutdown Signal         |
              +<--------------------------+
              |
              v
    +-----------------+
    | Graceful        |
    | Shutdown        |
    +-----------------+
              |
              v
    +-----------------+
    | Close etcd      |
    | connection      |
    +-----------------+
              |
              v
    +-----------------+
    | Process Exit    |
    +-----------------+
```

---

## 1. Configuration Loading

### Algorithm

```
ALGORITHM: LoadConfiguration
INPUT: Environment variables
OUTPUT: ServerConfig

CONSTANTS:
    DEFAULT_LISTEN = "0.0.0.0:9100"
    DEFAULT_LOG_LEVEL = "info"
    DEFAULT_ETCD_ENDPOINTS = "http://localhost:2379"
    DEFAULT_ETCD_PREFIX = "/streams"
    DEFAULT_RAW_PATH = "/data/raw"
    DEFAULT_ETCD_CONNECT_TIMEOUT_MS = 5000
    DEFAULT_ETCD_REQUEST_TIMEOUT_MS = 3000

BEGIN
    // Required configuration (fail if missing)
    // None required for MVP - all have defaults

    // Optional configuration with defaults
    config <- ServerConfig {
        // Server settings
        listen_addr: env_or_default("NDP_MCP_LISTEN", DEFAULT_LISTEN),
        log_level: env_or_default("NDP_MCP_LOG_LEVEL", DEFAULT_LOG_LEVEL),

        // etcd settings
        etcd: EtcdConfig {
            endpoints: env_or_default("NDP_ETCD_ENDPOINTS", DEFAULT_ETCD_ENDPOINTS)
                         .split(",").map(trim),
            prefix: env_or_default("NDP_ETCD_PREFIX", DEFAULT_ETCD_PREFIX),
            connect_timeout_ms: parse_int(env_or_default("NDP_ETCD_CONNECT_TIMEOUT_MS",
                                                         DEFAULT_ETCD_CONNECT_TIMEOUT_MS)),
            request_timeout_ms: parse_int(env_or_default("NDP_ETCD_REQUEST_TIMEOUT_MS",
                                                         DEFAULT_ETCD_REQUEST_TIMEOUT_MS)),
        },

        // Storage settings
        raw_path: PathBuf::from(env_or_default("NDP_RAW_PATH", DEFAULT_RAW_PATH)),

        // Auth settings (disabled for MVP)
        auth: AuthConfig {
            enabled: parse_bool(env_or_default("NDP_AUTH_ENABLED", "false")),
            // Future: issuer, audience, etc.
        },
    }

    // Validate configuration
    ValidateConfig(config)

    RETURN config
END

SUBROUTINE: ValidateConfig
INPUT: config (ServerConfig)
OUTPUT: void (throws on error)

BEGIN
    // Validate listen address
    IF NOT is_valid_socket_addr(config.listen_addr) THEN
        THROW ConfigError("Invalid NDP_MCP_LISTEN: {config.listen_addr}")
    END IF

    // Validate log level
    valid_levels <- ["trace", "debug", "info", "warn", "error"]
    IF config.log_level NOT IN valid_levels THEN
        THROW ConfigError("Invalid NDP_MCP_LOG_LEVEL: {config.log_level}")
    END IF

    // Validate etcd endpoints
    IF config.etcd.endpoints.is_empty() THEN
        THROW ConfigError("NDP_ETCD_ENDPOINTS cannot be empty")
    END IF

    FOR EACH endpoint IN config.etcd.endpoints DO
        IF NOT is_valid_url(endpoint) THEN
            THROW ConfigError("Invalid etcd endpoint: {endpoint}")
        END IF
    END FOR

    // Validate raw path (just check format, not existence - may not exist yet)
    IF config.raw_path.is_empty() THEN
        THROW ConfigError("NDP_RAW_PATH cannot be empty")
    END IF
END
```

---

## 2. Initialization Sequence

### Algorithm

```
ALGORITHM: Initialize
INPUT: config (ServerConfig)
OUTPUT: AppState

BEGIN
    // Phase 1: Initialize tracing/logging
    InitializeTracing(config.log_level)
    tracing::info!("Starting NDP Bronze MCP Server")
    tracing::info!("Config: listen={}, etcd={}", config.listen_addr,
                   config.etcd.endpoints.join(","))

    // Phase 2: Connect to etcd with retry
    etcd_client <- ConnectToEtcd(config.etcd)
    tracing::info!("Connected to etcd")

    // Phase 3: Validate etcd connectivity
    ValidateEtcdConnection(etcd_client, config.etcd.prefix)
    tracing::info!("etcd connection validated")

    // Phase 4: Initialize storage
    bronze_storage <- LocalBronzeStorage::new(config.raw_path)
    tracing::info!("Bronze storage initialized at {}", config.raw_path)

    // Phase 5: Build tool registry
    tool_registry <- BuildToolRegistry(etcd_client, bronze_storage)
    tracing::info!("Tool registry built with {} tools", tool_registry.count())

    // Phase 6: Build app state
    app_state <- AppState {
        config: config,
        etcd: Arc::new(etcd_client),
        storage: Arc::new(bronze_storage),
        tools: Arc::new(tool_registry),
        shutdown: Arc::new(AtomicBool::new(false)),
    }

    RETURN app_state
END

SUBROUTINE: InitializeTracing
INPUT: log_level (string)
OUTPUT: void

BEGIN
    // Configure tracing subscriber
    subscriber <- tracing_subscriber::fmt()
        .with_env_filter(log_level)
        .with_target(true)
        .with_thread_ids(false)
        .with_file(false)
        .with_line_number(false)
        .json()  // Structured JSON logs for production
        .init()
END

SUBROUTINE: ConnectToEtcd
INPUT: config (EtcdConfig)
OUTPUT: EtcdClient

CONSTANTS:
    MAX_RETRIES = 5
    INITIAL_BACKOFF_MS = 100
    MAX_BACKOFF_MS = 5000
    BACKOFF_MULTIPLIER = 2

BEGIN
    backoff <- INITIAL_BACKOFF_MS
    last_error <- null

    FOR attempt IN 1..MAX_RETRIES DO
        TRY
            tracing::debug!("Connecting to etcd (attempt {}/{})", attempt, MAX_RETRIES)

            client <- EtcdClient::connect(
                endpoints: config.endpoints,
                options: ConnectOptions {
                    connect_timeout: Duration::from_millis(config.connect_timeout_ms),
                    keep_alive_interval: Duration::from_secs(10),
                    keep_alive_timeout: Duration::from_secs(3),
                }
            ).await

            // Test connection with a simple operation
            client.status().await

            tracing::info!("etcd connected after {} attempts", attempt)
            RETURN client

        CATCH error:
            last_error <- error
            tracing::warn!("etcd connection failed (attempt {}): {}", attempt, error)

            IF attempt < MAX_RETRIES THEN
                // Exponential backoff with jitter
                jitter <- random(0, backoff / 4)
                sleep_duration <- backoff + jitter
                tracing::debug!("Retrying in {}ms", sleep_duration)
                tokio::time::sleep(Duration::from_millis(sleep_duration)).await
                backoff <- MIN(backoff * BACKOFF_MULTIPLIER, MAX_BACKOFF_MS)
            END IF
        END TRY
    END FOR

    // All retries exhausted
    THROW InitError("Failed to connect to etcd after {} attempts: {}",
                    MAX_RETRIES, last_error)
END

SUBROUTINE: ValidateEtcdConnection
INPUT: client (EtcdClient), prefix (string)
OUTPUT: void (throws on error)

BEGIN
    TRY
        // Try to list keys under our prefix
        response <- client.get_prefix(prefix, limit=1).await

        tracing::debug!("etcd validation: found {} keys under {}",
                        response.count(), prefix)

    CATCH error:
        THROW InitError("etcd validation failed: {} - ensure etcd is running " +
                        "and {} prefix is accessible", error, prefix)
    END TRY
END

SUBROUTINE: BuildToolRegistry
INPUT: etcd_client, bronze_storage
OUTPUT: ToolRegistry

BEGIN
    registry <- ToolRegistry::new()

    // Register all tools
    registry.register(ListStreamsTool::new(
        etcd: etcd_client.clone(),
        storage: bronze_storage.clone(),
    ))

    registry.register(DescribeSchemaTool::new(
        etcd: etcd_client.clone(),
        storage: bronze_storage.clone(),
    ))

    registry.register(ValidateConfigTool::new(
        etcd: etcd_client.clone(),
        storage: bronze_storage.clone(),
    ))

    registry.register(SampleDataTool::new(
        storage: bronze_storage.clone(),
    ))

    RETURN registry
END
```

---

## 3. Axum Router Setup

### Algorithm

```
ALGORITHM: BuildRouter
INPUT: app_state (AppState)
OUTPUT: Router

BEGIN
    // Build MCP routes
    mcp_router <- Router::new()
        .route("/mcp", post(handle_mcp_request))
        .with_state(app_state.clone())

    // Build health routes
    health_router <- Router::new()
        .route("/health", get(handle_health))
        .route("/health/ready", get(handle_readiness))
        .route("/health/live", get(handle_liveness))
        .with_state(app_state.clone())

    // Combine routers
    app <- Router::new()
        .merge(mcp_router)
        .merge(health_router)
        // Add middleware layers
        .layer(
            ServiceBuilder::new()
                // Request tracing
                .layer(TraceLayer::new_for_http())
                // Request timeout
                .layer(TimeoutLayer::new(Duration::from_secs(30)))
                // CORS (permissive for development)
                .layer(CorsLayer::permissive())
        )

    RETURN app
END

SUBROUTINE: handle_mcp_request
INPUT: State(state), Json(request)
OUTPUT: Json<McpResponse>

BEGIN
    request_id <- generate_uuid()
    tracing::info!(request_id = %request_id, method = %request.method, "MCP request")

    response <- MATCH request.method:
        "initialize" => handle_initialize(state, request),
        "tools/list" => handle_tools_list(state),
        "tools/call" => handle_tools_call(state, request),
        _ => McpResponse::error("Unknown method: {request.method}")
    END MATCH

    tracing::info!(request_id = %request_id, success = response.is_success(), "MCP response")

    RETURN Json(response)
END

SUBROUTINE: handle_initialize
INPUT: state, request
OUTPUT: McpResponse

BEGIN
    // MCP protocol initialization
    RETURN McpResponse::success({
        protocolVersion: "2024-11-25",
        serverInfo: {
            name: "ndp-bronze-mcp",
            version: env!("CARGO_PKG_VERSION"),
        },
        capabilities: {
            tools: { listChanged: false },
        },
    })
END

SUBROUTINE: handle_tools_list
INPUT: state
OUTPUT: McpResponse

BEGIN
    tools <- state.tools.list()

    RETURN McpResponse::success({
        tools: tools.map(t -> {
            name: t.name(),
            description: t.description(),
            inputSchema: t.input_schema(),
        })
    })
END

SUBROUTINE: handle_tools_call
INPUT: state, request
OUTPUT: McpResponse

BEGIN
    tool_name <- request.params.name
    arguments <- request.params.arguments OR {}

    TRY
        result <- state.tools.call(tool_name, arguments).await

        RETURN McpResponse::success(result)

    CATCH ToolError::NotFound(name):
        RETURN McpResponse::error("Unknown tool: {name}")

    CATCH ToolError::InvalidArguments(msg):
        RETURN McpResponse::error("Invalid arguments: {msg}")

    CATCH ToolError::Execution(msg):
        RETURN McpResponse::error("Tool execution failed: {msg}")

    CATCH error:
        tracing::error!(error = %error, tool = tool_name, "Tool execution error")
        RETURN McpResponse::error("Internal error processing tool request")
    END TRY
END

SUBROUTINE: handle_health
INPUT: State(state)
OUTPUT: Json<HealthResponse>

BEGIN
    // Basic health check
    etcd_healthy <- check_etcd_health(state.etcd).await
    storage_healthy <- check_storage_health(state.storage)

    status <- IF etcd_healthy AND storage_healthy THEN "healthy" ELSE "unhealthy"

    RETURN Json(HealthResponse {
        status: status,
        version: env!("CARGO_PKG_VERSION"),
        uptime_seconds: state.start_time.elapsed().as_secs(),
        components: {
            etcd: IF etcd_healthy THEN "ok" ELSE "error",
            storage: IF storage_healthy THEN "ok" ELSE "error",
        },
    })
END

SUBROUTINE: handle_readiness
INPUT: State(state)
OUTPUT: Response

BEGIN
    // Readiness: can we serve requests?
    IF state.shutdown.load() THEN
        RETURN StatusCode::SERVICE_UNAVAILABLE
    END IF

    etcd_healthy <- check_etcd_health(state.etcd).await

    IF etcd_healthy THEN
        RETURN StatusCode::OK
    ELSE
        RETURN StatusCode::SERVICE_UNAVAILABLE
    END IF
END

SUBROUTINE: handle_liveness
INPUT: State(state)
OUTPUT: Response

BEGIN
    // Liveness: is the process alive and not deadlocked?
    // Simple check - if we can respond, we're alive
    RETURN StatusCode::OK
END
```

---

## 4. Server Startup

### Algorithm

```
ALGORITHM: StartServer
INPUT: app (Router), config (ServerConfig), shutdown_signal (Future)
OUTPUT: void

BEGIN
    // Parse listen address
    addr <- config.listen_addr.parse::<SocketAddr>()
    IF addr IS error THEN
        THROW StartupError("Invalid listen address: {config.listen_addr}")
    END IF

    // Bind to address
    tracing::info!("Binding to {}", addr)

    listener <- TcpListener::bind(addr).await
    IF listener IS error THEN
        THROW StartupError("Failed to bind to {}: {}", addr, error)
    END IF

    tracing::info!("NDP Bronze MCP Server listening on {}", addr)
    tracing::info!("Health endpoint: http://{}/health", addr)
    tracing::info!("MCP endpoint: http://{}/mcp", addr)

    // Serve with graceful shutdown
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal)
        .await

    tracing::info!("Server stopped")
END

SUBROUTINE: SetupShutdownSignal
INPUT: app_state (AppState)
OUTPUT: Future<()>

BEGIN
    // Create signal handlers for SIGTERM and SIGINT
    sigterm <- tokio::signal::unix::signal(SignalKind::terminate())
    sigint <- tokio::signal::ctrl_c()

    // Wait for either signal
    async {
        tokio::select! {
            _ = sigterm.recv() => {
                tracing::info!("Received SIGTERM, initiating shutdown")
            }
            _ = sigint => {
                tracing::info!("Received SIGINT, initiating shutdown")
            }
        }

        // Set shutdown flag
        app_state.shutdown.store(true)

        // Allow in-flight requests to complete (grace period)
        tracing::info!("Waiting for in-flight requests to complete...")
    }
END
```

---

## 5. Graceful Shutdown

### Algorithm

```
ALGORITHM: GracefulShutdown
INPUT: app_state (AppState)
OUTPUT: void

CONSTANTS:
    SHUTDOWN_TIMEOUT_SECS = 30
    DRAIN_CHECK_INTERVAL_MS = 100

BEGIN
    tracing::info!("Initiating graceful shutdown")
    shutdown_start <- Instant::now()

    // Phase 1: Stop accepting new connections
    // (Handled by axum's graceful shutdown)
    app_state.shutdown.store(true)

    // Phase 2: Wait for in-flight requests
    // The server will automatically wait for active connections

    // Phase 3: Close etcd connection
    TRY
        tracing::debug!("Closing etcd connection")
        // etcd-client handles connection cleanup on drop
        // but we can explicitly close if needed
    CATCH error:
        tracing::warn!("Error closing etcd connection: {}", error)
    END TRY

    // Phase 4: Flush any pending logs
    tracing::debug!("Flushing logs")

    elapsed <- shutdown_start.elapsed()
    tracing::info!("Graceful shutdown complete in {:?}", elapsed)
END
```

---

## 6. Main Entry Point

### Algorithm

```
ALGORITHM: Main
INPUT: Command line args (unused for MVP)
OUTPUT: Exit code (0 = success, 1 = error)

BEGIN
    // Load configuration
    TRY
        config <- LoadConfiguration()
    CATCH ConfigError(msg):
        eprintln!("Configuration error: {}", msg)
        RETURN 1
    END TRY

    // Initialize async runtime
    runtime <- tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()

    // Run async main
    result <- runtime.block_on(async {
        TRY
            // Initialize application
            app_state <- Initialize(config)

            // Build router
            app <- BuildRouter(app_state.clone())

            // Setup shutdown signal
            shutdown_signal <- SetupShutdownSignal(app_state.clone())

            // Start server (blocks until shutdown)
            StartServer(app, config, shutdown_signal).await

            // Graceful shutdown
            GracefulShutdown(app_state).await

            RETURN Ok(())

        CATCH InitError(msg):
            tracing::error!("Initialization failed: {}", msg)
            RETURN Err(1)

        CATCH error:
            tracing::error!("Server error: {}", error)
            RETURN Err(1)
        END TRY
    })

    MATCH result:
        Ok(()) => RETURN 0
        Err(code) => RETURN code
    END MATCH
END
```

---

## State Diagram

```
+----------+     config      +--------------+
|  INIT    | ------+-------> | CONNECTING   |
+----------+       |         +--------------+
                   |               |
                   | error         | success
                   v               v
            +-----------+    +--------------+
            |  FAILED   |<-- | VALIDATING   |
            +-----------+    +--------------+
                   ^               |
                   | error         | success
                   |               v
                   |         +--------------+
                   +-------- |   READY      |
                   |         +--------------+
                   |               |
                   |               | bind error
                   |               v
                   |         +--------------+
                   +-------- |  LISTENING   |
                             +--------------+
                                   |
                                   | SIGTERM/SIGINT
                                   v
                             +--------------+
                             | SHUTTING_DOWN|
                             +--------------+
                                   |
                                   | drain complete
                                   v
                             +--------------+
                             |   STOPPED    |
                             +--------------+
```

---

## Configuration Reference

| Variable | Default | Description |
|----------|---------|-------------|
| `NDP_MCP_LISTEN` | `0.0.0.0:9100` | Server listen address |
| `NDP_MCP_LOG_LEVEL` | `info` | Log level (trace/debug/info/warn/error) |
| `NDP_ETCD_ENDPOINTS` | `http://localhost:2379` | Comma-separated etcd endpoints |
| `NDP_ETCD_PREFIX` | `/streams` | etcd key prefix for stream configs |
| `NDP_ETCD_CONNECT_TIMEOUT_MS` | `5000` | etcd connection timeout |
| `NDP_ETCD_REQUEST_TIMEOUT_MS` | `3000` | etcd request timeout |
| `NDP_RAW_PATH` | `/data/raw` | Bronze layer storage path |
| `NDP_AUTH_ENABLED` | `false` | Enable authentication (future) |

---

## Health Check Endpoints

| Endpoint | Purpose | Response |
|----------|---------|----------|
| `GET /health` | Full health status | JSON with component status |
| `GET /health/ready` | Kubernetes readiness | 200 OK or 503 |
| `GET /health/live` | Kubernetes liveness | 200 OK |

---

## Resource Management

### Memory Targets (Raspberry Pi 5)

| Component | Target | Notes |
|-----------|--------|-------|
| Base server | < 20MB | Rust binary + runtime |
| etcd client | < 10MB | Connection pool |
| Tool execution | < 20MB | Per-request working memory |
| **Total** | < 50MB | Well under 512MB container limit |

### Connection Pools

- **etcd**: Single persistent connection with keep-alive
- **Filesystem**: No pool needed (direct file access)

---

*Pseudocode ready for implementation with tokio, axum, and etcd-client crates.*
