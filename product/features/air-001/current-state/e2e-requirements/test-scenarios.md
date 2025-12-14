# E2E Test Scenarios for Air Quality Platform

**Version:** 1.0.0
**Date:** December 14, 2025
**Purpose:** Comprehensive end-to-end test scenarios demonstrating the air-001 vision

---

## Test Environment Prerequisites

### Docker Services Required
1. **mosquitto** - MQTT broker (Eclipse Mosquitto 2.0)
2. **air-quality-app** - Main application under test
3. **sensor-simulator** - Mock AirGradient sensor
4. **test-observer** - MQTT subscriber for validation
5. **prometheus** - Metrics collection (optional)

### Test Data
- Mock AirGradient readings (29 fields)
- Various quality scenarios (complete, partial, invalid)
- Threshold violation scenarios (high CO2, PM2.5, VOC)

---

## Scenario Category 1: Data Ingestion

### TC-1.1: Basic MQTT Message Processing

```gherkin
Feature: MQTT Message Ingestion
  As an air quality monitoring system
  I want to receive sensor data via MQTT
  So that I can track indoor air quality

Scenario: Complete AirGradient reading ingestion
  Given the air-quality-app is running
  And connected to MQTT broker
  When a sensor publishes a complete reading to "airgradient/readings/ecda3b1eaaaf"
    | Field           | Value    |
    | wifi            | -46      |
    | serialno        | ecda3b1eaaaf |
    | rco2            | 850      |
    | pm02            | 8.5      |
    | atmp            | 22.5     |
    | rhum            | 45       |
  Then the reading is parsed successfully
  And stored in Parquet within 5 seconds
  And GET /api/v1/readings/latest returns the reading

Scenario: Partial payload handling
  Given the air-quality-app is running
  When a sensor publishes a minimal reading (only serialno and rco2)
  Then the reading is stored with null values for missing fields
  And quality score reflects incomplete data

Scenario: Invalid payload rejection
  Given the air-quality-app is running
  When a sensor publishes invalid JSON
  Then the message is sent to Dead Letter Queue
  And ingestion continues for valid messages

Scenario: Out-of-range value validation
  Given the air-quality-app is running
  When a sensor publishes CO2 value of 50000 (exceeds 10000 max)
  Then the reading is rejected with validation error
  And error is logged with field details
```

### TC-1.2: MQTT Connection Resilience

```gherkin
Scenario: Auto-reconnect after broker restart
  Given the air-quality-app is connected to MQTT
  When the MQTT broker restarts
  Then the app reconnects within 60 seconds
  And message processing resumes
  And no data is lost during reconnection

Scenario: Exponential backoff on connection failure
  Given the air-quality-app cannot connect to MQTT
  Then reconnection attempts use exponential backoff
  And max backoff is 30 seconds
  And connection events are logged
```

---

## Scenario Category 2: Data Storage

### TC-2.1: Parquet Persistence

```gherkin
Feature: Durable Time-Series Storage
  As an air quality monitoring system
  I want to persist readings to Parquet files
  So that I can query historical data

Scenario: Daily partition creation
  Given readings are ingested on 2025-12-14
  Then a partition is created at data/{location}/year=2025/month=12/day=14/
  And readings are stored in readings.parquet

Scenario: WAL crash recovery
  Given 100 readings are ingested
  And 50 are committed to Parquet
  And 50 are in WAL
  When the application crashes (kill -9)
  And restarts
  Then all 100 readings are recovered
  And WAL is replayed successfully
  And no data is lost

Scenario: Storage capacity limits
  Given 1 year of data (525,600 readings)
  Then total storage is less than 500MB
  And daily partitions average ~1.5MB
```

### TC-2.2: Time-Range Queries

```gherkin
Scenario: 24-hour query performance
  Given 1440 readings for the last 24 hours
  When GET /api/v1/readings?start=2025-12-13T00:00:00Z&end=2025-12-14T00:00:00Z
  Then response returns 1440 readings
  And response time is less than 100ms

Scenario: Aggregation query
  Given hourly readings for 7 days
  When GET /api/v1/aggregate?interval=1h&agg=mean&metric=pm25
  Then response returns 168 hourly averages
  And calculations are accurate within 0.01%
```

---

## Scenario Category 3: Health Monitoring

### TC-3.1: Health Endpoint

```gherkin
Feature: System Health Monitoring
  As an operator
  I want to check system health
  So that I can ensure reliable operation

Scenario: Healthy system status
  Given MQTT is connected
  And storage is operational
  And last reading is within 120 seconds
  When GET /health
  Then response status is 200
  And body contains:
    | Field                    | Value     |
    | status                   | healthy   |
    | mqtt                     | connected |
    | storage                  | ok        |
    | last_reading_age_seconds | < 120     |

Scenario: Degraded status on MQTT disconnect
  Given MQTT connection is lost
  When GET /health
  Then response status is 503
  And body contains:
    | Field  | Value        |
    | status | degraded     |
    | mqtt   | disconnected |

Scenario: Stale data warning
  Given last reading is older than 5 minutes
  When GET /health
  Then response includes warning about stale data
```

---

## Scenario Category 4: Alerting

### TC-4.1: Threshold-Based Alerts

```gherkin
Feature: Health Threshold Alerting
  As a building occupant
  I want alerts when air quality degrades
  So that I can take corrective action

Scenario: CO2 threshold violation
  Given alert thresholds are configured:
    | Metric | Moderate | Poor | VeryPoor |
    | CO2    | 1000     | 1500 | 2000     |
  When a reading with CO2 = 1650 ppm is ingested
  Then an alert is generated with severity "Poor"
  And message indicates ventilation is needed
  And alert appears in GET /api/v1/alerts?time_range=active

Scenario: PM2.5 threshold violation
  Given EPA PM2.5 thresholds are configured
  When a reading with PM2.5 = 45 µg/m³ is ingested
  Then an alert is generated with severity "Unhealthy"
  And alert includes health recommendations

Scenario: Alert deduplication
  Given an active CO2 alert at severity "Poor"
  When subsequent readings remain above threshold
  Then no duplicate alerts are generated
  When CO2 drops below (threshold - 10%)
  Then alert is automatically cleared
```

### TC-4.2: Predictive Alerts (Future)

```gherkin
Scenario: Forecast-based proactive alert
  Given current CO2 is 900 ppm (acceptable)
  And forecast predicts 1200 ppm in 2 hours (p90 confidence)
  Then a predictive alert is generated
  And message indicates future threshold violation
  And lead time of 2 hours is specified
```

---

## Scenario Category 5: Forecasting

### TC-5.1: ML Predictions

```gherkin
Feature: Air Quality Forecasting
  As an air quality monitoring system
  I want to predict future conditions
  So that I can provide proactive recommendations

Scenario: 6-hour PM2.5 forecast
  Given 24 hours of historical PM2.5 data
  When GET /api/v1/forecast?metric=pm25&horizon=6
  Then response contains 72 predictions (6h × 12 per hour)
  And each prediction includes p10, p50, p90 confidence intervals
  And model version is specified

Scenario: Cold start performance
  Given models are not loaded
  When first forecast request is made
  Then model loads within 30 seconds
  And prediction is returned

Scenario: Warm cache performance
  Given models are loaded
  When forecast request is made
  Then response is returned within 2 seconds
```

---

## Scenario Category 6: API Completeness

### TC-6.1: REST API Endpoints

```gherkin
Scenario Outline: API endpoint availability
  Given the air-quality-app is running
  When <Method> <Endpoint> is called
  Then response status is <Status>
  And response format is valid JSON

Examples:
  | Method | Endpoint                          | Status |
  | GET    | /health                           | 200    |
  | GET    | /api/v1/readings/latest           | 200    |
  | GET    | /api/v1/readings                  | 200    |
  | GET    | /api/v1/aggregate                 | 200    |
  | GET    | /api/v1/forecast                  | 200    |
  | GET    | /api/v1/alerts                    | 200    |
  | GET    | /api/v1/locations                 | 200    |
  | GET    | /metrics                          | 200    |
```

---

## Scenario Category 7: Docker Deployment

### TC-7.1: Container Orchestration

```gherkin
Feature: Docker Deployment
  As a system administrator
  I want to deploy via Docker Compose
  So that I have a consistent deployment

Scenario: Fresh deployment startup
  Given a clean Docker environment
  When docker-compose up is executed
  Then all services start within 60 seconds
  And health checks pass
  And MQTT broker is accessible
  And API is responding

Scenario: Graceful shutdown
  Given the system is running with active data
  When docker-compose down is executed
  Then WAL is committed to Parquet
  And all containers stop gracefully
  And no data is lost

Scenario: Volume persistence across restarts
  Given data has been ingested
  When containers are restarted
  Then all historical data is preserved
  And query results are consistent
```

### TC-7.2: Multi-Architecture Support

```gherkin
Scenario: ARM64 deployment (Pi 5)
  Given a Raspberry Pi 5 with Docker
  When the arm64 image is deployed
  Then all services start successfully
  And performance is acceptable (<2s query latency)
  And memory usage stays below 2GB

Scenario: AMD64 deployment (Development)
  Given a Linux/macOS development machine
  When the amd64 image is deployed
  Then all services start successfully
  And performance is optimal
```

---

## Scenario Category 8: Integration

### TC-8.1: End-to-End Data Flow

```gherkin
Feature: Complete Data Pipeline
  As a user
  I want sensor data to flow through the entire system
  So that I can monitor, query, and receive alerts

Scenario: Full pipeline validation
  Given the complete system is running:
    | Service         | Status  |
    | mosquitto       | healthy |
    | air-quality-app | healthy |
    | sensor-simulator| running |

  When the sensor simulator publishes 60 readings (1 per second)
  Then all readings are stored in Parquet
  And GET /api/v1/readings returns 60 readings
  And GET /api/v1/aggregate returns correct mean/min/max
  And GET /health shows last_reading_age_seconds < 5
  And any threshold violations generate alerts
```

---

## Test Execution Order

### Smoke Tests (5 minutes)
1. TC-7.1: Docker startup
2. TC-3.1: Health endpoint healthy
3. TC-1.1: Basic message ingestion

### Functional Tests (15 minutes)
4. TC-1.1: All ingestion scenarios
5. TC-1.2: Connection resilience
6. TC-2.1: Storage scenarios
7. TC-2.2: Query scenarios

### Integration Tests (10 minutes)
8. TC-4.1: Alerting scenarios
9. TC-5.1: Forecasting scenarios
10. TC-8.1: Full pipeline validation

### Performance Tests (5 minutes)
11. TC-2.2: Query latency (<100ms)
12. TC-5.1: Forecast latency (<30s cold, <2s warm)

### Stress Tests (10 minutes)
13. High-frequency ingestion (100 msg/sec)
14. Large query (7 days, 10k readings)
15. Concurrent requests (10 parallel queries)

**Total E2E Test Time:** ~45 minutes
