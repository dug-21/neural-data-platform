# Neural Trader - gRPC Protocol Buffer Definitions

This directory contains the Protocol Buffer definitions for the neural-trader platform's configuration management system, implementing the interface contracts defined in the SPARC Phase 2 specification.

## 📁 Files

- **`config_store.proto`** - Main protocol buffer definition for ConfigStoreService and ConfigManagementService
- **`compile_proto.sh`** - Compilation script for generating Python and Rust bindings
- **`README.md`** - This documentation file

## 🚀 Quick Start

### Generate Python Bindings

```bash
# Simple compilation
./proto/compile_proto.sh

# Clean compilation with dependency installation
./proto/compile_proto.sh --clean --install-deps

# Python bindings only
./proto/compile_proto.sh --python-only
```

### Generate Rust Bindings

```bash
# Rust bindings are auto-generated via build.rs
cargo build

# Or generate build configuration only
./proto/compile_proto.sh --rust-only
```

### Check Dependencies

```bash
./proto/compile_proto.sh --check-deps
```

## 📋 Generated Files

### Python Output (`data_ingestion/proto/`)
- `config_store_pb2.py` - Protocol buffer message classes
- `config_store_pb2_grpc.py` - gRPC service stubs
- `config_store_pb2.pyi` - Type hints for Python
- `config_client_example.py` - Example client implementation

### Rust Output (`src/proto/`)
- Generated automatically during cargo build via `build.rs`
- Includes both client and server implementations
- Uses `tonic` for gRPC and `prost` for protobuf

## 🔧 Interface Overview

### ConfigStoreService

The main configuration service providing CRUD operations with real-time updates:

```protobuf
service ConfigStoreService {
  rpc GetConfig(GetConfigRequest) returns (GetConfigResponse);
  rpc GetBulkConfig(GetBulkConfigRequest) returns (GetBulkConfigResponse);
  rpc SetConfig(SetConfigRequest) returns (SetConfigResponse);
  rpc WatchConfig(WatchConfigRequest) returns (stream ConfigChangeEvent);
  rpc GetSchema(GetSchemaRequest) returns (GetSchemaResponse);
  rpc HealthCheck(google.protobuf.Empty) returns (HealthStatus);
}
```

### ConfigManagementService

Administrative operations for namespace and audit management:

```protobuf
service ConfigManagementService {
  rpc ListNamespaces(ListNamespacesRequest) returns (ListNamespacesResponse);
  rpc GetNamespaceInfo(GetNamespaceInfoRequest) returns (GetNamespaceInfoResponse);
  rpc ValidateConfig(ValidateConfigRequest) returns (ValidateConfigResponse);
  rpc GetAuditTrail(GetAuditTrailRequest) returns (GetAuditTrailResponse);
  rpc BackupNamespace(BackupNamespaceRequest) returns (BackupNamespaceResponse);
  rpc RestoreNamespace(RestoreNamespaceRequest) returns (RestoreNamespaceResponse);
}
```

## 📊 Key Features

### 🏷️ Strongly Typed Configuration Values

```protobuf
message ConfigValue {
  ValueType type = 1;
  oneof value {
    string string_value = 2;
    int64 int_value = 3;
    double float_value = 4;
    bool bool_value = 5;
    google.protobuf.Struct json_value = 6;
    bytes binary_value = 7;
  }
}
```

### 🔍 Namespace Isolation

Configuration organized by hierarchical namespaces:
- `/neural-platform/shared/eventbus`
- `/neural-platform/shared/ml-ops`
- `/neural-trading/data-ingestion`
- `/neural-trading/model-execution`
- `/neural-trading/action-layer`

### ⚡ Real-time Updates

Streaming configuration changes with comprehensive event information:

```protobuf
message ConfigChangeEvent {
  string namespace_path = 1;
  string key = 2;
  ChangeType change_type = 3;
  ConfigValue old_value = 4;
  ConfigValue new_value = 5;
  google.protobuf.Timestamp timestamp = 6;
  string change_reason = 7;
  string changed_by = 8;
  string version = 9;
}
```

### 📝 Audit Trail & Versioning

Complete audit trail with:
- Timestamps and change reasons
- Version tracking for optimistic concurrency
- User/service attribution
- Configuration history

### 🛡️ Schema Validation

JSON Schema-based validation with:
- Version compatibility checking
- Structured error reporting
- Dry-run validation support

## 🔗 Python Usage Example

```python
import grpc
from data_ingestion.proto import config_store_pb2, config_store_pb2_grpc

# Create client
channel = grpc.insecure_channel('localhost:50051')
stub = config_store_pb2_grpc.ConfigStoreServiceStub(channel)

# Get configuration
request = config_store_pb2.GetConfigRequest(
    namespace_path="/neural-trading/data-ingestion",
    key="sources.primary.symbols",
    include_metadata=True
)

response = stub.GetConfig(request)
if response.success:
    print(f"Config value: {response.value}")
    print(f"Last modified: {response.metadata.modified_at}")
else:
    print(f"Error: {response.error_message}")

# Watch for changes
watch_request = config_store_pb2.WatchConfigRequest(
    namespace_path="/neural-trading/data-ingestion",
    keys=["sources.primary.symbols"],
    include_initial_values=True
)

for event in stub.WatchConfig(watch_request):
    print(f"Config changed: {event.key} -> {event.new_value}")

channel.close()
```

## 🦀 Rust Usage Example

```rust
use tonic::transport::Channel;
use config_store_pb::{
    config_store_service_client::ConfigStoreServiceClient,
    GetConfigRequest, SetConfigRequest, ConfigValue, ValueType,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = ConfigStoreServiceClient::connect("http://localhost:50051").await?;

    // Get configuration
    let request = tonic::Request::new(GetConfigRequest {
        namespace_path: "/neural-trading/data-ingestion".to_string(),
        key: "sources.primary.symbols".to_string(),
        version: "".to_string(),
        context: std::collections::HashMap::new(),
        include_metadata: true,
    });

    let response = client.get_config(request).await?;
    println!("Config response: {:?}", response.get_ref());

    // Set configuration
    let config_value = ConfigValue {
        type_: ValueType::String as i32,
        value: Some(config_value::Value::StringValue("AAPL,GOOGL,MSFT".to_string())),
    };

    let set_request = tonic::Request::new(SetConfigRequest {
        namespace_path: "/neural-trading/data-ingestion".to_string(),
        key: "sources.primary.symbols".to_string(),
        value: Some(config_value),
        change_reason: "Updated symbol list".to_string(),
        validate_only: false,
        expected_version: "".to_string(),
    });

    let set_response = client.set_config(set_request).await?;
    println!("Set config response: {:?}", set_response.get_ref());

    Ok(())
}
```

## 🔧 Configuration Schema Examples

### Data Ingestion Service Configuration

```yaml
# Namespace: /neural-trading/data-ingestion
sources:
  primary:
    provider: "alpaca"
    api_url: "${ALPACA_API_URL}"
    websocket_url: "${ALPACA_WS_URL}"
    symbols: ["AAPL", "GOOGL", "MSFT", "AMZN", "TSLA"]
    rate_limits:
      requests_per_minute: 200
      websocket_connections: 5
    retry_policy:
      max_attempts: 3
      backoff_multiplier: 2.0
      initial_delay_ms: 1000

eventbus:
  streams:
    market_data: "trading:market-data"
    system_events: "trading:system"
  consumer_groups:
    - "data-ingestion-group"
    - "model-execution-group"

validation:
  price_range:
    min_price: 0.01
    max_price: 10000.0
  timestamp_tolerance_ms: 300000
```

### Shared Platform Configuration

```yaml
# Namespace: /neural-platform/shared
eventbus:
  connection: "redis://redis:6379"
  consumer_groups:
    - "data-ingestion-group"
    - "model-execution-group"
    - "action-execution-group"

ml-ops:
  model_registry: "/opt/models"
  training_schedule: "0 2 * * *"
  performance_thresholds:
    accuracy: 0.85
    latency_ms: 100

monitoring:
  prometheus_url: "http://prometheus:9090"
  grafana_url: "http://grafana:3000"
  log_level: "info"
```

## 🚀 Performance Characteristics

- **Configuration Access**: <10ms P95 for cached configurations
- **Update Propagation**: <30 seconds for real-time updates
- **Throughput**: 10,000 reads/sec, 100 writes/sec
- **Availability**: 99.9% uptime target during market hours

## 🔐 Security Features

- **Namespace Isolation**: Service-level access control
- **Audit Logging**: Complete configuration change tracking  
- **Schema Validation**: Prevents invalid configurations
- **Secrets Separation**: Configuration vs. environment variables

## 🛠️ Development & Testing

### Dependencies

- **System**: `protobuf-compiler`
- **Python**: `grpcio-tools`, `protobuf`
- **Rust**: `prost`, `tonic` (added to Cargo.toml)

### Testing the Generated Code

```bash
# Test Python bindings
cd data_ingestion/proto
python3 -c "import config_store_pb2, config_store_pb2_grpc; print('✓ Python imports work')"

# Test Rust bindings (generates during build)
cargo build

# Run the example client
python3 config_client_example.py
```

### Integration with Data Ingestion Service

The generated Python bindings can be imported directly in the data ingestion service:

```python
# In data_ingestion/config/settings.py
from data_ingestion.proto import config_store_pb2, config_store_pb2_grpc

class ConfigStoreClient:
    def __init__(self, config_server_url: str):
        self.channel = grpc.insecure_channel(config_server_url)
        self.stub = config_store_pb2_grpc.ConfigStoreServiceStub(self.channel)
    
    async def load_data_ingestion_config(self) -> dict:
        # Load configuration from config-store service
        # Implementation details...
        pass
```

## 📝 Interface Contract Compliance

This implementation fully complies with the SPARC Phase 2 specification requirements:

- ✅ **FR-1.1.1**: Hierarchical configuration management with namespace isolation
- ✅ **FR-1.1.2**: Real-time configuration updates via WatchConfig stream
- ✅ **FR-1.1.3**: Configuration versioning and complete audit trail
- ✅ **FR-1.2.1**: Multi-source configuration support for data ingestion
- ✅ **FR-1.2.2**: Market data configuration management
- ✅ **FR-1.2.3**: EventBus integration configuration
- ✅ **FR-1.3.1**: Standardized gRPC interface implementation
- ✅ **FR-1.3.2**: Schema registry integration support

## 🔗 Related Documentation

- [SPARC Phase 2 Specification](/workspaces/neural-trader/product/features/v2Planning/phase2/1-SPARC-Specification.md)
- [Config-Store Service Documentation](/workspaces/neural-trader/config-store/README.md)
- [Data Ingestion Service Integration Guide](/workspaces/neural-trader/data_ingestion/README.md)

---

For questions or issues, please refer to the main project documentation or create an issue in the repository.