# Docker E2E Test Architecture

**Version:** 1.0.0
**Date:** December 14, 2025
**Purpose:** Docker Compose configuration for end-to-end testing of the Air Quality Platform

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                    Docker Network: air-quality-e2e               │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌─────────────┐    MQTT     ┌──────────────────┐              │
│  │  Mosquitto  │◄───────────►│  Air Quality App │              │
│  │   (broker)  │             │   (under test)   │              │
│  └──────┬──────┘             └────────┬─────────┘              │
│         │                             │                         │
│         │ publish                     │ REST API                │
│         │                             │                         │
│  ┌──────▼──────┐             ┌────────▼─────────┐              │
│  │   Sensor    │             │   Test Runner    │              │
│  │  Simulator  │             │   (assertions)   │              │
│  └─────────────┘             └──────────────────┘              │
│                                                                 │
│  ┌─────────────┐             ┌──────────────────┐              │
│  │    Test     │◄────────────│   Prometheus     │  (optional)  │
│  │  Observer   │   metrics   │                  │              │
│  └─────────────┘             └──────────────────┘              │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

---

## Service Definitions

### 1. MQTT Broker (Mosquitto)

```yaml
mosquitto:
  image: eclipse-mosquitto:2.0
  container_name: e2e-mosquitto
  ports:
    - "1883:1883"
  volumes:
    - ./mosquitto/config/mosquitto.conf:/mosquitto/config/mosquitto.conf:ro
    - mosquitto-data:/mosquitto/data
    - mosquitto-logs:/mosquitto/log
  healthcheck:
    test: ["CMD", "mosquitto_sub", "-t", "$$SYS/broker/uptime", "-C", "1", "-W", "3"]
    interval: 10s
    timeout: 5s
    retries: 3
    start_period: 10s
  networks:
    - air-quality-e2e
```

### 2. Air Quality Application (Under Test)

```yaml
air-quality-app:
  build:
    context: ../..
    dockerfile: Dockerfile
  container_name: e2e-air-quality
  depends_on:
    mosquitto:
      condition: service_healthy
  ports:
    - "8080:8080"
    - "9090:9090"
  environment:
    - RUST_LOG=info,air_quality=debug
    - MQTT_BROKER_URL=mqtt://mosquitto:1883
    - MQTT_CLIENT_ID=air-quality-e2e
    - MQTT_TOPIC_PATTERN=airgradient/readings/+
    - DATA_DIR=/data
    - MODELS_DIR=/models
  volumes:
    - e2e-data:/data
    - e2e-models:/models
    - ./config/air-quality-e2e.yaml:/config/air-quality.yaml:ro
  healthcheck:
    test: ["CMD", "curl", "-f", "http://localhost:8080/health"]
    interval: 10s
    timeout: 5s
    retries: 5
    start_period: 30s
  networks:
    - air-quality-e2e
```

### 3. Sensor Simulator

```yaml
sensor-simulator:
  build:
    context: ./sensor-simulator
    dockerfile: Dockerfile
  container_name: e2e-sensor-simulator
  depends_on:
    mosquitto:
      condition: service_healthy
  environment:
    - MQTT_BROKER=mosquitto:1883
    - SENSOR_SERIAL=ecda3b1eaaaf
    - SENSOR_MODEL=I-9PSL
    - FIRMWARE_VERSION=3.1.4
    - PUBLISH_INTERVAL_SECONDS=5
    - SCENARIO=normal  # normal, high_co2, high_pm25, volatile, missing_fields
  networks:
    - air-quality-e2e
```

### 4. Test Observer

```yaml
test-observer:
  build:
    context: ./test-observer
    dockerfile: Dockerfile
  container_name: e2e-test-observer
  depends_on:
    mosquitto:
      condition: service_healthy
    air-quality-app:
      condition: service_healthy
  environment:
    - MQTT_BROKER=mosquitto:1883
    - MQTT_TOPICS=airgradient/readings/#,neural/predictions/#
    - OBSERVE_DURATION_SECONDS=300
    - OUTPUT_FILE=/results/observations.json
  volumes:
    - e2e-results:/results
  networks:
    - air-quality-e2e
```

### 5. Test Runner

```yaml
test-runner:
  build:
    context: ./test-runner
    dockerfile: Dockerfile
  container_name: e2e-test-runner
  depends_on:
    air-quality-app:
      condition: service_healthy
    sensor-simulator:
      condition: service_started
  environment:
    - API_BASE_URL=http://air-quality-app:8080
    - MQTT_BROKER=mosquitto:1883
    - TEST_TIMEOUT_SECONDS=300
    - REPORT_FORMAT=junit
  volumes:
    - e2e-results:/results
  command: ["cargo", "test", "--test", "e2e", "--", "--test-threads=1"]
  networks:
    - air-quality-e2e
```

### 6. Prometheus (Optional)

```yaml
prometheus:
  image: prom/prometheus:latest
  container_name: e2e-prometheus
  profiles:
    - monitoring
  ports:
    - "9091:9090"
  volumes:
    - ./config/prometheus-e2e.yml:/etc/prometheus/prometheus.yml:ro
    - prometheus-data:/prometheus
  command:
    - '--config.file=/etc/prometheus/prometheus.yml'
    - '--storage.tsdb.retention.time=1d'
  networks:
    - air-quality-e2e
```

---

## Complete Docker Compose File

```yaml
# docker-compose.e2e.yml
version: "3.8"

services:
  # Core Infrastructure
  mosquitto:
    image: eclipse-mosquitto:2.0
    container_name: e2e-mosquitto
    ports:
      - "1883:1883"
    volumes:
      - ./mosquitto/config/mosquitto.conf:/mosquitto/config/mosquitto.conf:ro
      - mosquitto-data:/mosquitto/data
      - mosquitto-logs:/mosquitto/log
    healthcheck:
      test: ["CMD", "mosquitto_sub", "-t", "$$SYS/broker/uptime", "-C", "1", "-W", "3"]
      interval: 10s
      timeout: 5s
      retries: 3
      start_period: 10s
    networks:
      - air-quality-e2e

  # Application Under Test
  air-quality-app:
    build:
      context: ../..
      dockerfile: Dockerfile
      args:
        - BUILD_MODE=release
    container_name: e2e-air-quality
    depends_on:
      mosquitto:
        condition: service_healthy
    ports:
      - "8080:8080"
      - "9090:9090"
    environment:
      - RUST_LOG=info,air_quality=debug
      - MQTT_BROKER_URL=mqtt://mosquitto:1883
      - MQTT_CLIENT_ID=air-quality-e2e
      - MQTT_TOPIC_PATTERN=airgradient/readings/+
      - DATA_DIR=/data
      - MODELS_DIR=/models
    volumes:
      - e2e-data:/data
      - e2e-models:/models
      - ./config/air-quality-e2e.yaml:/config/air-quality.yaml:ro
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8080/health"]
      interval: 10s
      timeout: 5s
      retries: 5
      start_period: 30s
    networks:
      - air-quality-e2e

  # Test Components
  sensor-simulator:
    build:
      context: ./sensor-simulator
    container_name: e2e-sensor-simulator
    depends_on:
      mosquitto:
        condition: service_healthy
    environment:
      - MQTT_BROKER=mosquitto:1883
      - SENSOR_SERIAL=ecda3b1eaaaf
      - SENSOR_MODEL=I-9PSL
      - FIRMWARE_VERSION=3.1.4
      - PUBLISH_INTERVAL_SECONDS=5
      - SCENARIO=${TEST_SCENARIO:-normal}
    networks:
      - air-quality-e2e

  test-observer:
    build:
      context: ./test-observer
    container_name: e2e-test-observer
    depends_on:
      mosquitto:
        condition: service_healthy
      air-quality-app:
        condition: service_healthy
    environment:
      - MQTT_BROKER=mosquitto:1883
      - MQTT_TOPICS=airgradient/readings/#,neural/predictions/#
      - OBSERVE_DURATION_SECONDS=300
    volumes:
      - e2e-results:/results
    networks:
      - air-quality-e2e

  test-runner:
    build:
      context: ./test-runner
    container_name: e2e-test-runner
    depends_on:
      air-quality-app:
        condition: service_healthy
      sensor-simulator:
        condition: service_started
    environment:
      - API_BASE_URL=http://air-quality-app:8080
      - MQTT_BROKER=mosquitto:1883
      - TEST_TIMEOUT_SECONDS=300
    volumes:
      - e2e-results:/results
    command: ["cargo", "test", "--test", "e2e", "--", "--test-threads=1", "--nocapture"]
    networks:
      - air-quality-e2e

  # Optional Monitoring
  prometheus:
    image: prom/prometheus:latest
    container_name: e2e-prometheus
    profiles:
      - monitoring
    ports:
      - "9091:9090"
    volumes:
      - ./config/prometheus-e2e.yml:/etc/prometheus/prometheus.yml:ro
    networks:
      - air-quality-e2e

volumes:
  mosquitto-data:
  mosquitto-logs:
  e2e-data:
  e2e-models:
  e2e-results:
  prometheus-data:

networks:
  air-quality-e2e:
    name: air-quality-e2e
    driver: bridge
```

---

## Sensor Simulator Implementation

### Dockerfile

```dockerfile
# tests/e2e/sensor-simulator/Dockerfile
FROM rust:1.75-slim-bookworm as builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/sensor-simulator /usr/local/bin/
ENTRYPOINT ["sensor-simulator"]
```

### Source Code Outline

```rust
// tests/e2e/sensor-simulator/src/main.rs

use rumqttc::{AsyncClient, MqttOptions, QoS};
use serde_json::json;
use std::time::Duration;

#[tokio::main]
async fn main() {
    let broker = std::env::var("MQTT_BROKER").unwrap_or("localhost:1883".to_string());
    let serial = std::env::var("SENSOR_SERIAL").unwrap_or("ecda3b1eaaaf".to_string());
    let interval: u64 = std::env::var("PUBLISH_INTERVAL_SECONDS")
        .unwrap_or("5".to_string())
        .parse()
        .unwrap();
    let scenario = std::env::var("SCENARIO").unwrap_or("normal".to_string());

    let mut mqttoptions = MqttOptions::new("sensor-simulator", &broker, 1883);
    mqttoptions.set_keep_alive(Duration::from_secs(30));

    let (client, mut eventloop) = AsyncClient::new(mqttoptions, 10);

    tokio::spawn(async move {
        loop {
            if let Err(e) = eventloop.poll().await {
                eprintln!("MQTT error: {:?}", e);
            }
        }
    });

    let topic = format!("airgradient/readings/{}", serial);
    let mut counter = 0u32;

    loop {
        let reading = generate_reading(&scenario, &serial, counter);
        let payload = serde_json::to_string(&reading).unwrap();

        client.publish(&topic, QoS::AtLeastOnce, false, payload)
            .await
            .expect("Failed to publish");

        println!("Published reading {} to {}", counter, topic);
        counter += 1;
        tokio::time::sleep(Duration::from_secs(interval)).await;
    }
}

fn generate_reading(scenario: &str, serial: &str, counter: u32) -> serde_json::Value {
    match scenario {
        "high_co2" => json!({
            "wifi": -46,
            "serialno": serial,
            "rco2": 1800 + (counter % 500) as i32,
            "pm02": 8.5,
            "atmp": 22.5,
            "rhum": 45,
            "tvocIndex": 100,
            "noxIndex": 1,
            "boot": counter,
            "firmware": "3.1.4",
            "model": "I-9PSL"
        }),
        "high_pm25" => json!({
            "wifi": -46,
            "serialno": serial,
            "rco2": 600,
            "pm02": 55.0 + (counter % 30) as f32,
            "atmp": 22.5,
            "rhum": 45,
            "tvocIndex": 100,
            "noxIndex": 1,
            "boot": counter,
            "firmware": "3.1.4",
            "model": "I-9PSL"
        }),
        "volatile" => {
            // Simulate rapid changes
            let variance = ((counter as f32 * 0.5).sin() * 500.0) as i32;
            json!({
                "wifi": -46,
                "serialno": serial,
                "rco2": 800 + variance,
                "pm02": 15.0 + (variance as f32 / 50.0),
                "atmp": 22.5,
                "rhum": 45,
                "boot": counter,
                "firmware": "3.1.4",
                "model": "I-9PSL"
            })
        },
        _ => json!({
            "wifi": -46,
            "serialno": serial,
            "rco2": 600 + (counter % 200) as i32,
            "pm01": 3.0,
            "pm02": 7.0 + (counter % 5) as f32,
            "pm10": 8.0,
            "pm02Compensated": 6.0,
            "atmp": 22.5 + (counter % 10) as f32 * 0.1,
            "atmpCompensated": 21.5,
            "rhum": 45 + (counter % 10) as i32,
            "rhumCompensated": 48,
            "tvocIndex": 100,
            "tvocRaw": 33000,
            "noxIndex": 1,
            "noxRaw": 16000,
            "boot": counter,
            "bootCount": counter,
            "ledMode": "pm",
            "firmware": "3.1.4",
            "model": "I-9PSL"
        })
    }
}
```

---

## Test Runner Implementation

### Dockerfile

```dockerfile
# tests/e2e/test-runner/Dockerfile
FROM rust:1.75-slim-bookworm as builder
WORKDIR /app
COPY . .
RUN cargo build --release --tests

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates curl && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/deps/e2e* /usr/local/bin/
WORKDIR /app
ENTRYPOINT ["e2e-tests"]
```

### E2E Test Examples

```rust
// tests/e2e/test-runner/tests/e2e.rs

use reqwest::Client;
use std::time::Duration;

#[tokio::test]
async fn test_health_endpoint() {
    let client = Client::new();
    let base_url = std::env::var("API_BASE_URL").unwrap();

    let response = client.get(format!("{}/health", base_url))
        .timeout(Duration::from_secs(10))
        .send()
        .await
        .expect("Health check failed");

    assert_eq!(response.status(), 200);

    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["status"], "healthy");
    assert_eq!(body["mqtt"], "connected");
}

#[tokio::test]
async fn test_data_ingestion_flow() {
    let client = Client::new();
    let base_url = std::env::var("API_BASE_URL").unwrap();

    // Wait for sensor simulator to publish some readings
    tokio::time::sleep(Duration::from_secs(30)).await;

    let response = client.get(format!("{}/api/v1/readings/latest", base_url))
        .query(&[("location_id", "ecda3b1eaaaf")])
        .send()
        .await
        .expect("Failed to query readings");

    assert_eq!(response.status(), 200);

    let body: serde_json::Value = response.json().await.unwrap();
    assert!(body["data"]["rco2"].as_i64().is_some());
    assert!(body["data"]["pm02"].as_f64().is_some());
}

#[tokio::test]
async fn test_aggregation() {
    let client = Client::new();
    let base_url = std::env::var("API_BASE_URL").unwrap();

    // Wait for enough data
    tokio::time::sleep(Duration::from_secs(60)).await;

    let response = client.get(format!("{}/api/v1/aggregate", base_url))
        .query(&[
            ("location_id", "ecda3b1eaaaf"),
            ("interval", "1m"),
            ("agg", "mean"),
            ("metric", "pm25"),
        ])
        .send()
        .await
        .expect("Aggregation failed");

    assert_eq!(response.status(), 200);
}
```

---

## Running E2E Tests

### Basic Run

```bash
cd product/features/air-002/docker
docker compose -f docker-compose.e2e.yml up --build
```

### With Specific Scenario

```bash
TEST_SCENARIO=high_co2 docker compose -f docker-compose.e2e.yml up --build
```

### With Monitoring

```bash
docker compose -f docker-compose.e2e.yml --profile monitoring up --build
```

### Run Tests Only

```bash
# Start infrastructure
docker compose -f docker-compose.e2e.yml up -d mosquitto air-quality-app sensor-simulator

# Wait for healthy
docker compose -f docker-compose.e2e.yml exec air-quality-app curl -f http://localhost:8080/health

# Run tests
docker compose -f docker-compose.e2e.yml run test-runner
```

### Clean Up

```bash
docker compose -f docker-compose.e2e.yml down -v
```

---

## CI/CD Integration

### GitHub Actions Workflow

```yaml
# .github/workflows/e2e-tests.yml
name: E2E Tests

on:
  push:
    branches: [main, feature/*]
  pull_request:
    branches: [main]

jobs:
  e2e:
    runs-on: ubuntu-latest
    timeout-minutes: 30

    steps:
      - uses: actions/checkout@v4

      - name: Set up Docker Buildx
        uses: docker/setup-buildx-action@v3

      - name: Build and run E2E tests
        run: |
          cd product/features/air-002/docker
          docker compose -f docker-compose.e2e.yml up --build --abort-on-container-exit --exit-code-from test-runner

      - name: Upload test results
        if: always()
        uses: actions/upload-artifact@v4
        with:
          name: e2e-results
          path: product/features/air-002/docker/results/

      - name: Cleanup
        if: always()
        run: |
          docker compose -f product/features/air-002/docker/docker-compose.e2e.yml down -v
```

---

## Directory Structure

```
product/features/air-002/docker/
├── docker-compose.e2e.yml
├── config/
│   ├── air-quality-e2e.yaml
│   ├── prometheus-e2e.yml
│   └── mosquitto.conf
├── sensor-simulator/
│   ├── Dockerfile
│   ├── Cargo.toml
│   └── src/
│       └── main.rs
├── test-observer/
│   ├── Dockerfile
│   ├── Cargo.toml
│   └── src/
│       └── main.rs
├── test-runner/
│   ├── Dockerfile
│   ├── Cargo.toml
│   └── tests/
│       └── e2e.rs
└── results/
    └── .gitkeep
```
