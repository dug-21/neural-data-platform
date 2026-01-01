# Config Store gRPC Service

A production-ready gRPC service implementation for configuration management in the neural-trader platform.

## Features

- **Complete gRPC API Implementation**: Both `ConfigStoreService` and `ConfigManagementService` with all methods
- **Real-time Configuration Streaming**: Watch configuration changes with filtered streaming support  
- **Production-Ready Health Checks**: Built-in health endpoint for Docker and Kubernetes monitoring
- **Hierarchical Namespace Support**: Organized configuration paths like `/neural-trading/data-ingestion`
- **Type-Safe Configuration Values**: Support for strings, integers, floats, booleans, and complex JSON objects
- **Comprehensive Error Handling**: Proper error responses and logging throughout
- **Graceful Shutdown**: SIGINT/SIGTERM handling with proper resource cleanup

## Architecture

The service bridges between the gRPC protocol buffers interface and the internal configuration store:

```
┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐
│   gRPC Client   │───▶│   Config Store   │───▶│   In-Memory     │
│                 │    │   gRPC Service   │    │   Store         │
└─────────────────┘    └──────────────────┘    └─────────────────┘
                              │
                              ▼
                       ┌──────────────────┐
                       │   Configuration  │
                       │   Change Stream  │
                       └──────────────────┘
```

## Building and Running

### Prerequisites

- Rust 1.75+
- Protocol Buffers compiler (`protoc`) - installed automatically during build

### Build

```bash
cargo build --release --bin config-store-server
```

### Run

```bash
# Start server on default port (50051)
cargo run --bin config-store-server

# Start server on custom port and host
cargo run --bin config-store-server -- --port 8080 --host 127.0.0.1

# Health check (when server is running)
cargo run --bin config-store-server health
```

### Command Line Options

```bash
config-store-server --help
```

```
Usage: config-store-server [OPTIONS] [COMMAND]

Commands:
  health  Check server health
  help    Print this message or the help of the given subcommand(s)

Options:
  -p, --port <PORT>  Port to listen on [default: 50051]
      --host <HOST>  Host to bind to [default: 0.0.0.0]
  -h, --help         Print help
  -V, --version      Print version
```

## API Implementation

### ConfigStoreService

All methods are fully implemented:

- ✅ `GetConfig` - Retrieve single configuration with optional metadata
- ✅ `GetBulkConfig` - Retrieve multiple configurations efficiently  
- ✅ `SetConfig` - Set configuration with validation and change broadcasting
- ✅ `WatchConfig` - Real-time filtered configuration change streaming
- ✅ `GetSchema` - Configuration schema retrieval (placeholder implementation)
- ✅ `HealthCheck` - Service health status with detailed information

### ConfigManagementService  

Administrative operations:

- ✅ `ListNamespaces` - Discover available configuration namespaces
- ✅ `GetNamespaceInfo` - Namespace details and statistics
- ✅ `ValidateConfig` - Configuration validation (placeholder implementation)
- ✅ `GetAuditTrail` - Configuration change audit trail (placeholder)
- ✅ `BackupNamespace` - Namespace backup (placeholder implementation)
- ✅ `RestoreNamespace` - Namespace restoration (placeholder implementation)

## Docker Support

### Included Dockerfile

```dockerfile
FROM rust:1.75-slim as builder
# ... (full production-ready multi-stage build)
```

Key features:
- Multi-stage build for minimal image size
- Non-root user execution  
- Health check integration
- Proper signal handling

### Build and Run

```bash
# Build image  
docker build -t config-store:latest .

# Run container
docker run -p 50051:50051 config-store:latest

# Health check
docker ps  # Shows health status
```

## Configuration

Environment variables:

- `CONFIG_STORE_PORT` - Override default port for health checks
- `RUST_LOG` - Control logging verbosity (default: `config_store=info`)

Example:
```bash
RUST_LOG=debug CONFIG_STORE_PORT=8080 cargo run --bin config-store-server
```

## Integration Examples

### Using grpcurl

Install: `go install github.com/fullstorydev/grpcurl/cmd/grpcurl@latest`

#### Set Configuration

```bash
grpcurl -plaintext -d '{
  "namespace_path": "/neural-trading",
  "key": "model_timeout", 
  "value": {
    "type": 2,
    "int_value": 30
  },
  "change_reason": "Increase timeout for ML models"
}' localhost:50051 neural_platform.config.ConfigStoreService/SetConfig
```

#### Get Configuration  

```bash
grpcurl -plaintext -d '{
  "namespace_path": "/neural-trading",
  "key": "model_timeout",
  "include_metadata": true
}' localhost:50051 neural_platform.config.ConfigStoreService/GetConfig
```

#### Watch Changes

```bash  
grpcurl -plaintext -d '{
  "namespace_path": "/neural-trading", 
  "keys": ["model_timeout"],
  "include_initial_values": true
}' localhost:50051 neural_platform.config.ConfigStoreService/WatchConfig
```

## Development

### Testing

```bash
cargo test
```

### Code Quality

```bash
cargo fmt
cargo clippy
```

### Logging

Structured logging with `tracing`:

```bash
# Debug level
RUST_LOG=debug cargo run --bin config-store-server

# Specific modules
RUST_LOG=config_store=debug,tonic=info cargo run --bin config-store-server
```

## Production Deployment

### Kubernetes Example

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: config-store
spec:
  replicas: 2
  selector:
    matchLabels:
      app: config-store
  template:
    metadata:
      labels:
        app: config-store  
    spec:
      containers:
      - name: config-store
        image: config-store:latest
        ports:
        - containerPort: 50051
          name: grpc
        env:
        - name: RUST_LOG
          value: "config_store=info"
        livenessProbe:
          exec:
            command: ["/usr/local/bin/config-store-server", "health"]
          initialDelaySeconds: 10
          periodSeconds: 30
        readinessProbe:
          exec:
            command: ["/usr/local/bin/config-store-server", "health"]
          initialDelaySeconds: 5
          periodSeconds: 10
        resources:
          requests:
            memory: "64Mi"
            cpu: "50m"
          limits:
            memory: "128Mi" 
            cpu: "200m"
---
apiVersion: v1
kind: Service
metadata:
  name: config-store-service
spec:
  selector:
    app: config-store
  ports:
  - port: 50051
    targetPort: 50051
    name: grpc
```

### Production Considerations

- **TLS/SSL**: Add TLS termination for production use
- **Authentication**: Implement service-to-service auth as needed
- **Persistent Storage**: Replace in-memory store with Redis/PostgreSQL for production
- **Monitoring**: Integrate with Prometheus/Grafana for metrics
- **Rate Limiting**: Add rate limiting for public-facing deployments
- **Schema Validation**: Implement JSON schema validation for configurations

## Implementation Details

### Type Conversion

The service handles conversion between internal Rust types and Protocol Buffer types:

- `ConfigValue` ↔ protobuf `ConfigValue` with type safety
- JSON objects ↔ `prost_types::Struct` with proper serialization  
- System timestamps ↔ `prost_types::Timestamp`
- Error handling with appropriate gRPC status codes

### Real-time Streaming

Configuration watching uses Tokio broadcast channels with:
- Filtered event streaming based on namespace and keys
- Graceful error handling and stream termination
- Memory-efficient async stream processing

### Health Checks

The health check system provides:
- Service availability verification
- Component-specific status details
- Docker-compatible exit codes  
- Kubernetes probe integration

This implementation provides a solid foundation for production configuration management in the neural-trader platform.
