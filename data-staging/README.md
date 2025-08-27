# Data-Staging Service

The Data-Staging service is the critical bridge between raw JSON data from the data-ingestion service and the proto-only EventBus system in Neural Trader V2 Phase 4. It acts as the sole quality gate ensuring only validated protobuf messages reach downstream consumers.

## Architecture Overview

```
Data-Ingestion (JSON) → Redis Streams → Data-Staging → EventBus (Proto Only)
                                            ↓
                                       DLQ (Invalid Data)
```

## Key Responsibilities

1. **Raw JSON Consumption**: Consumes raw market data JSON from Redis streams
2. **Strict Validation**: Validates all data fields, formats, and business rules
3. **Quality Scoring**: Calculates comprehensive data quality metrics
4. **Proto Transformation**: Converts valid JSON to EventEnvelope protobuf messages
5. **Dead Letter Queue**: Handles invalid data with detailed error logging
6. **Metrics Collection**: Comprehensive monitoring and performance tracking

## Core Design Principles

- **Proto-Only Output**: ONLY publishes validated protobuf messages to EventBus
- **Zero Data Loss**: Invalid data goes to DLQ, nothing is silently dropped
- **High Performance**: Designed for >10,000 messages/second throughput
- **Comprehensive Monitoring**: Full observability with Prometheus metrics
- **Strict Quality Gates**: No data passes without meeting quality thresholds

## Module Structure

### Core Processing Modules

- **`redis_consumer.rs`** - Consumes raw JSON from Redis streams with consumer groups
- **`json_validator.rs`** - Strict validation of JSON structure and business rules  
- **`quality_scorer.rs`** - Calculates data quality metrics (freshness, completeness, validity)
- **`proto_transformer.rs`** - Transforms validated JSON to EventEnvelope protobuf
- **`eventbus_publisher.rs`** - Publishes proto messages to EventBus (proto-only)

### Supporting Infrastructure

- **`dlq_manager.rs`** - Dead Letter Queue for invalid data with categorized errors
- **`metrics.rs`** - Prometheus metrics collection and health monitoring

## Quality Scoring Algorithm

The service calculates a comprehensive quality score (0-1) based on:

### Freshness Score (30% weight)
- **1.0**: 0-5 seconds old
- **0.9**: 6-30 seconds old  
- **0.8**: 31-60 seconds old
- **0.6**: 1-5 minutes old
- **0.0**: >30 minutes old

### Completeness Score (30% weight)
- Required fields: 70% weight
- Optional fields: 30% weight
- Metadata richness bonus

### Validity Score (40% weight)
- Price ranges and formats
- Volume validation
- Bid/ask spread consistency
- OHLC data relationships
- Symbol format validation
- Exchange validation

## Configuration

### Quality Thresholds
```toml
[quality_thresholds]
minimum_quality_score = 0.7
max_age_seconds = 300
required_fields = ["symbol", "price", "timestamp"]
```

### Processing Limits
```toml
[processing_limits]
max_batch_size = 100
message_timeout_ms = 1000
max_retries = 3
```

## Metrics

The service exports comprehensive Prometheus metrics:

### Processing Metrics
- `data_staging_messages_processed_total`
- `data_staging_messages_failed_total`
- `data_staging_messages_dlq_total`
- `data_staging_processing_duration_seconds`

### Quality Metrics
- `data_staging_quality_score`
- `data_staging_freshness_score`
- `data_staging_completeness_score`
- `data_staging_validity_score`

### System Metrics
- `data_staging_eventbus_publish_total`
- `data_staging_redis_consume_total`
- `data_staging_memory_usage_bytes`
- `data_staging_cpu_usage_percent`

## Error Handling

### Error Categories
- **JSON_PARSING**: Malformed JSON data
- **VALIDATION**: Business rule violations
- **PROTO_TRANSFORMATION**: Protobuf conversion failures  
- **QUALITY_CHECK**: Quality score below threshold
- **INFRASTRUCTURE**: Redis/EventBus connectivity issues

### Dead Letter Queue
All failed messages are preserved in the DLQ with:
- Original data and error details
- Error categorization and failure stage
- Retry count and processing metadata
- 24-hour retention with automatic cleanup

## Testing

### Unit Tests
- Comprehensive TDD test suite with >90% coverage
- All modules have dedicated test files in `/tests/`
- Mock implementations for external dependencies

### Integration Tests
- End-to-end data flow validation
- Error handling and DLQ functionality
- Performance and load testing scenarios

## Performance Characteristics

### Target Performance
- **Throughput**: >10,000 messages/second
- **Latency**: <5ms P95 transformation time
- **Quality Gate**: <1ms validation time
- **Uptime**: 99.9% availability target

### Resource Requirements
- **Memory**: ~100MB baseline + processing buffers
- **CPU**: ~5-10% under normal load
- **Redis**: Persistent connection with automatic reconnection

## Data Flow Example

1. **Raw JSON Input** (from data-ingestion):
```json
{
  "symbol": "AAPL",
  "price": 150.25,
  "volume": 1000.0,
  "timestamp": 1640995200,
  "bid": 150.20,
  "ask": 150.30,
  "exchange": "NASDAQ"
}
```

2. **Validation**: Check required fields, price ranges, timestamp freshness

3. **Quality Scoring**: Calculate freshness=0.95, completeness=0.85, validity=1.0

4. **Proto Transformation**: Create EventEnvelope with market data payload

5. **EventBus Publishing**: Send validated proto message to consumers

## Deployment

### Build Requirements
- Protocol Buffer compiler (`protoc`)
- Rust 1.70+ with stable toolchain
- Access to Redis and neural-core EventBus

### Environment Variables
- `REDIS_URL`: Redis connection string
- `INPUT_STREAM`: Redis stream name for raw data
- `OUTPUT_TOPIC`: EventBus topic for proto messages
- `LOG_LEVEL`: Logging verbosity (debug, info, warn, error)

### Health Checks
- `/health` endpoint with service status
- `/metrics` endpoint with Prometheus metrics
- `/ready` endpoint for deployment readiness

## Monitoring and Alerting

### Key Alerts
- Processing success rate < 95%
- Quality score < 0.7 (sustained)
- DLQ growth rate > 100 messages/hour
- EventBus publish failures > 5%
- Redis connectivity issues

### Dashboards
- Real-time processing metrics
- Quality score trends
- Error categorization breakdown
- Performance and latency tracking

## Future Enhancements

- Machine learning-based anomaly detection
- Adaptive quality thresholds
- Multi-datacenter deployment support
- Enhanced proto schema evolution handling
- Advanced DLQ replay mechanisms

---

The Data-Staging service is a critical component ensuring data quality and system reliability in the Neural Trader platform. It enforces strict proto-only communication while maintaining high throughput and comprehensive observability.