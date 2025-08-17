# Domain Registry Specification

## Overview
The Domain Registry is a centralized metadata store that solves the configuration complexity problem identified in V1. It maintains all domain-specific configuration, metadata, and stream channel mappings in a single source of truth.

## Core Responsibilities

### 1. Domain Configuration Management
- **Market Hours**: Trading hours per exchange and asset class
- **Data Schemas**: Domain-specific data structures and validation rules  
- **Stream Channels**: Mapping of data types to Redis stream keys
- **Rate Limits**: Per-source API limits and throttling rules
- **Authentication**: API keys and credentials per data source

### 2. Dynamic Discovery
- **Available Data Sources**: List of active connectors and their capabilities
- **Data Types**: What data each source provides (quotes, trades, news, etc.)
- **Stream Topology**: Which streams carry which data types
- **Quality Metrics**: Data quality scores and latency per source

### 3. Configuration API

```rust
pub trait DomainRegistry {
    // Domain Management
    async fn register_domain(&self, domain: DomainConfig) -> Result<DomainId>;
    async fn get_domain(&self, id: &DomainId) -> Result<DomainConfig>;
    async fn list_domains(&self) -> Result<Vec<DomainSummary>>;
    
    // Stream Discovery
    async fn get_stream_for(&self, domain: &str, source: &str, asset: &str) -> Result<StreamKey>;
    async fn list_streams_by_domain(&self, domain: &str) -> Result<Vec<StreamInfo>>;
    
    // Configuration
    async fn get_market_hours(&self, exchange: &str) -> Result<MarketHours>;
    async fn get_schema(&self, domain: &str, data_type: &str) -> Result<Schema>;
    async fn get_rate_limits(&self, source: &str) -> Result<RateLimits>;
}
```

## Domain Configuration Structure

```rust
pub struct DomainConfig {
    pub id: DomainId,
    pub name: String,
    pub description: String,
    pub sources: Vec<SourceConfig>,
    pub schemas: HashMap<String, Schema>,
    pub metadata: DomainMetadata,
}

pub struct SourceConfig {
    pub name: String,
    pub connector_type: String,
    pub endpoints: Vec<Endpoint>,
    pub rate_limits: RateLimits,
    pub auth: AuthConfig,
    pub stream_mappings: HashMap<DataType, StreamKey>,
}

pub struct DomainMetadata {
    pub market_hours: Option<MarketHours>,
    pub update_frequency: Duration,
    pub retention_policy: RetentionPolicy,
    pub quality_requirements: QualityThresholds,
}
```

## Example: Market Domain Configuration

```yaml
domain:
  id: market
  name: Financial Markets
  description: Real-time and historical market data
  
  sources:
    - name: polygon
      connector_type: polygon_websocket
      endpoints:
        - wss://socket.polygon.io/stocks
        - https://api.polygon.io/v2
      rate_limits:
        websocket: unlimited
        rest: 5_per_second
      stream_mappings:
        quotes: "market:polygon:quotes"
        trades: "market:polygon:trades"
        aggregates: "market:polygon:bars"
    
    - name: alpaca
      connector_type: alpaca_websocket
      endpoints:
        - wss://stream.data.alpaca.markets/v2
      rate_limits:
        websocket: unlimited
      stream_mappings:
        quotes: "market:alpaca:quotes"
        trades: "market:alpaca:trades"
  
  metadata:
    market_hours:
      nyse:
        timezone: America/New_York
        regular:
          open: "09:30"
          close: "16:00"
        premarket:
          open: "04:00"
          close: "09:30"
        afterhours:
          open: "16:00"
          close: "20:00"
    
    update_frequency: 100ms
    retention_policy:
      hot_storage: 24h
      warm_storage: 7d
      cold_storage: 1y
```

## Benefits Over V1

1. **Single Source of Truth**: No duplicate market hours configuration across layers
2. **Dynamic Discovery**: Components can query available data without hardcoding
3. **Runtime Flexibility**: Add new domains/sources without code changes
4. **Consistent Configuration**: All layers read from same registry
5. **Version Control**: Configuration changes tracked and auditable

## Integration Points

### Data Ingestion Layer
- Queries registry on startup for active domains
- Uses stream mappings for publishing
- Applies rate limits and auth from registry

### ML Ops Platform  
- Discovers available data streams
- Gets schemas for feature engineering
- Reads market hours for time-based features

### Model Execution Layer
- Queries available data for decision making
- Gets latency requirements per domain
- Discovers new data sources dynamically

### Action Layer
- Reads market hours for trade execution
- Gets broker-specific configuration
- Applies domain-specific rules

## MCP Tool Integration

```typescript
// MCP tools for registry management
interface RegistryTools {
  "mcp.ingestion.registry.list_domains": () => DomainSummary[];
  "mcp.ingestion.registry.get_domain": (id: string) => DomainConfig;
  "mcp.ingestion.registry.update_domain": (id: string, config: Partial<DomainConfig>) => void;
  "mcp.ingestion.registry.discover_streams": (filter: StreamFilter) => StreamInfo[];
  "mcp.ingestion.registry.get_market_hours": (exchange: string) => MarketHours;
}
```

## Migration from V1

1. Extract all hardcoded configuration from V1 layers
2. Consolidate into domain registry schemas
3. Update components to query registry instead of local config
4. Remove duplicate configuration code
5. Add monitoring for configuration access patterns