# MQTT Broker Sharing Patterns for NDP + Home Assistant Integration

## Executive Summary

This research document analyzes MQTT broker sharing patterns to enable the Neural Data Platform (NDP) and Home Assistant (HA) to coexist on a shared Mosquitto broker. The goal is to establish a consistent topic namespace that supports:

1. NDP subscribing to device sensor data (AirGradient, future sensors)
2. Home Assistant publishing/subscribing for home automation
3. Future Matter/Thread devices via Home Assistant bridge
4. Multi-subscriber scenarios without message conflicts

---

## Current NDP MQTT Architecture

### Mosquitto Configuration

**File**: `/workspaces/neural-data-platform/deploy/pi/mosquitto/mosquitto.conf`

Current configuration is minimal:
```
listener 1883
allow_anonymous true
persistence true
persistence_location /mosquitto/data/
log_dest stdout
```

Key observations:
- Anonymous access enabled (development mode)
- No ACL restrictions
- No topic namespace enforcement
- Single listener on port 1883

### Docker Deployment

**File**: `/workspaces/neural-data-platform/deploy/pi/docker-compose.yml`

Mosquitto runs as a Docker service with:
- Port 1883 exposed
- Persistent data volume at `/mosquitto/data/`
- Memory limit: 128MB
- Named container: `pi5-mosquitto`

### Current Topic Structure

NDP currently uses:
```
airgradient/readings/+     # AirGradient sensor data
airgradient/+/measures     # Alternative topic pattern
neural/predictions         # ML prediction output
```

The topic pattern is defined in:
- `/workspaces/neural-data-platform/config/base/streams/air-quality/config.yaml`
- etcd path: `/air-quality/mqtt/topic_pattern`

### MqttSource Implementation

**File**: `/workspaces/neural-data-platform/core/src/sources/mqtt.rs`

Key characteristics:
- Uses `rumqttc` async MQTT client
- Subscribes to configured topic pattern with wildcard support
- Parses JSON payloads into `TimeSeriesPoint` structures
- Implements the `Source` trait for the Domain Adapter pattern
- Supports QoS 1 (at least once delivery)

```rust
pub struct MqttConfig {
    pub broker_url: String,
    pub port: u16,
    pub client_id: String,
    pub topic_pattern: String,  // e.g., "airgradient/readings/+"
    pub qos: i32,
    // ...
}
```

---

## Recommended Topic Namespace Design

### Hierarchical Namespace Pattern

Based on industry best practices (UNS - Unified Namespace, ISA-95), we recommend a hierarchical topic structure:

```
{domain}/{location}/{device_type}/{device_id}/{data_category}
```

### Proposed NDP Namespace

```
ndp/                           # NDP namespace root
  sensors/                     # Sensor data ingestion
    air-quality/               # Air quality sensors
      {device_id}/             # Specific sensor
        readings               # Sensor readings
        status                 # Device status
    weather/                   # Weather-related data
      {location}/
        current
        forecast
  predictions/                 # ML model outputs
    air-quality/
      {model_id}
  events/                      # System events
    ingestion/
    storage/
```

### Proposed Home Assistant Namespace

```
homeassistant/                 # HA discovery prefix (standard)
  sensor/                      # Sensor entities
  binary_sensor/               # Binary sensors
  switch/                      # Switches
  climate/                     # HVAC/thermostats
  light/                       # Lighting

home/                          # Home state/events
  {location}/                  # Room/area
    {entity}/                  # Entity (window, door, etc.)
      state                    # Current state
      set                      # Command topic
```

### Cross-Domain Shared Topics

For data that both NDP and HA need to access:

```
shared/                        # Cross-platform data
  environment/                 # Environmental readings
    {location}/
      temperature
      humidity
      air_quality
  events/                      # Home events
    occupancy/
    window_state/
```

---

## Multi-Subscriber Patterns

### Pattern 1: Parallel Subscription (Recommended for NDP+HA)

Both NDP and HA subscribe to the same topics independently. MQTT brokers deliver messages to all subscribers.

```
AirGradient Sensor
       |
       | publish: ndp/sensors/air-quality/ag-one-001/readings
       v
  Mosquitto Broker
       |
       +---> NDP (subscriber 1) --> Parquet Storage
       |
       +---> HA (subscriber 2) --> Dashboard/Automation
```

**Advantages**:
- No message duplication needed
- Each subscriber processes independently
- Native MQTT fan-out behavior
- Clean separation of concerns

**Configuration**:
```yaml
# NDP subscription
topic_pattern: "ndp/sensors/+/+/readings"

# HA MQTT sensor configuration
mqtt:
  sensor:
    - state_topic: "ndp/sensors/air-quality/+/readings"
      json_attributes_topic: "ndp/sensors/air-quality/+/readings"
```

### Pattern 2: Topic Bridging (For Legacy Compatibility)

When devices use non-standard topics, use Mosquitto bridge to republish:

```
# mosquitto.conf bridge configuration
connection airgradient-bridge
address localhost:1883
topic airgradient/# in 0
topic ndp/sensors/air-quality/# out 0
```

### Pattern 3: Shared Subscriptions (Load Balancing)

For high-volume scenarios, use MQTT 5.0 shared subscriptions:

```
$share/ndp-workers/ndp/sensors/+/+/readings
```

This distributes messages across multiple NDP worker instances.

---

## Bridge vs Shared Broker Tradeoffs

### Option A: Shared Broker (Recommended)

**Architecture**:
```
                    Mosquitto
                       |
    +--------+---------+---------+--------+
    |        |         |         |        |
AirGradient  HA     NDP App   Future   Matter
  Sensor   Bridge             Sensors   Bridge
```

**Advantages**:
- Single point of management
- Lower resource usage (one broker)
- Native multi-subscriber support
- Simpler deployment on Pi 5
- Unified logging and monitoring

**Disadvantages**:
- Shared failure domain
- Topic collision risk (mitigated by namespace)
- Security requires ACLs for isolation

**Recommended for NDP** because:
1. Pi 5 resource constraints favor single broker
2. NDP and HA both need same sensor data
3. Simpler operational model

### Option B: Bridged Brokers

**Architecture**:
```
HA Mosquitto  <--bridge-->  NDP Mosquitto
     |                           |
     |                           |
  HA Core                    NDP App
```

**Advantages**:
- Complete isolation between systems
- Independent upgrades/restarts
- Separate security domains

**Disadvantages**:
- Double resource usage
- Bridge configuration complexity
- Potential message delays
- Additional failure points

**Use Case**: Enterprise environments requiring strict isolation

### Option C: Hierarchical Bridging

**Architecture**:
```
Edge Broker (Pi 5)
       |
       | bridge
       v
Cloud Broker (AWS IoT, HiveMQ)
       |
       +---> Cloud NDP Instance
       +---> HA Cloud Connect
```

**Use Case**: Hybrid cloud deployments

---

## Security Considerations

### ACL Configuration for Multi-Tenant

For production shared broker:

```
# /mosquitto/config/acl.conf

# NDP service user
user ndp-app
topic read ndp/#
topic read airgradient/#
topic read shared/#
topic write ndp/predictions/#

# Home Assistant user
user homeassistant
topic read homeassistant/#
topic write homeassistant/#
topic read home/#
topic write home/#
topic read shared/#
topic write shared/events/#

# AirGradient sensors (by client ID pattern)
pattern read $SYS/broker/#
pattern write ndp/sensors/air-quality/%c/#
```

### Authentication Setup

```bash
# Create password file
mosquitto_passwd -c /mosquitto/config/passwd ndp-app
mosquitto_passwd /mosquitto/config/passwd homeassistant

# Update mosquitto.conf
allow_anonymous false
password_file /mosquitto/config/passwd
acl_file /mosquitto/config/acl.conf
```

---

## Implementation Recommendations

### Phase 1: Namespace Migration (AIR-008)

1. Update NDP topic pattern from `airgradient/readings/+` to `ndp/sensors/air-quality/+/readings`
2. Configure AirGradient sensors to publish to new namespace
3. Add backward-compatible subscription to legacy topics during transition

### Phase 2: Home Assistant Integration

1. Configure HA MQTT integration to connect to shared broker
2. Set up HA sensors subscribing to `ndp/sensors/+/+/readings`
3. Configure HA to publish home events to `home/` namespace

### Phase 3: Matter/Thread Devices

1. Matter devices connect via HA Matter Server
2. HA publishes device states to `home/{location}/{device}/state`
3. NDP subscribes to relevant home state topics for analytics

### Phase 4: Production Hardening

1. Enable authentication with per-service credentials
2. Implement ACL rules for topic isolation
3. Add TLS/SSL encryption (port 8883)
4. Set up monitoring and alerting

---

## Configuration Examples

### Updated NDP Stream Config

```yaml
# config/base/streams/air-quality/config.yaml
stream_id: air-quality
version: "2.0.0"

sources:
  - type: mqtt
    enabled: true
    broker_url: mosquitto
    port: 1883
    client_id: ndp-air-quality-ingestion
    topic_patterns:
      - ndp/sensors/air-quality/+/readings    # New namespace
      - airgradient/readings/+                 # Legacy compatibility
    qos: 1
```

### Home Assistant MQTT Configuration

```yaml
# HA configuration.yaml
mqtt:
  broker: mosquitto
  port: 1883
  client_id: homeassistant
  username: homeassistant
  password: !secret mqtt_password

sensor:
  - platform: mqtt
    name: "Indoor Air Quality"
    state_topic: "ndp/sensors/air-quality/+/readings"
    value_template: "{{ value_json.pm02 }}"
    unit_of_measurement: "ug/m3"
    json_attributes_topic: "ndp/sensors/air-quality/+/readings"
```

### Mosquitto Production Config

```conf
# mosquitto.conf
listener 1883
listener 8883
cafile /mosquitto/certs/ca.crt
certfile /mosquitto/certs/server.crt
keyfile /mosquitto/certs/server.key

allow_anonymous false
password_file /mosquitto/config/passwd
acl_file /mosquitto/config/acl.conf

persistence true
persistence_location /mosquitto/data/

log_dest stdout
log_type all

# Performance tuning for Pi 5
max_connections 100
message_size_limit 65536
```

---

## Future Considerations

### Unified Namespace (UNS) Evolution

As NDP grows, consider adopting full UNS principles:

```
enterprise/
  {site}/
    {area}/
      {line}/
        {device}/
          {data_type}
```

This aligns with ISA-95 industrial automation standards and enables:
- Consistent cross-system data access
- Standardized topic discovery
- Simplified analytics and dashboards

### MQTT 5.0 Features

When Mosquitto 2.x is widely deployed:
- **Shared Subscriptions**: Load balancing for NDP workers
- **User Properties**: Metadata without payload modification
- **Topic Aliases**: Reduced bandwidth for high-frequency topics
- **Message Expiry**: Automatic cleanup of stale messages

---

## References

- [MQTT Topic Namespace Best Practices - i-flow](https://i-flow.io/en/ressources/mqtt-topic-namespace-best-practices-step-by-step-guide/)
- [MQTT Topics, Wildcards, & Best Practices - HiveMQ](https://www.hivemq.com/blog/mqtt-essentials-part-5-mqtt-topics-best-practices/)
- [MQTT Design Best Practices - AWS IoT Core](https://docs.aws.amazon.com/whitepapers/latest/designing-mqtt-topics-aws-iot-core/mqtt-design-best-practices.html)
- [Multi Tenant MQTT Broker - Ben's Place](https://blog.hardill.me.uk/2024/03/23/multi-tenant-mqtt-broker/)
- [Implementing UNS with MQTT Sparkplug - HiveMQ](https://www.hivemq.com/blog/implementing-unified-namespace-uns-mqtt-sparkplug/)
- [MQTT Topics and Wildcards Guide - EMQ](https://www.emqx.com/en/blog/advanced-features-of-mqtt-topics)
- [4 Reasons to Adopt MQTT in UNS - EMQ](https://www.emqx.com/en/blog/four-reasons-why-you-should-adopt-mqtt-in-unified-namespace)
- [MQTT Market Trends 2024 - Kai Waehner](https://www.kai-waehner.de/blog/2023/12/08/mqtt-market-trends-for-2024-cloud-unified-namespace-sparkplug-kafka-integration/)

---

## Summary

For the NDP + Home Assistant integration, we recommend:

1. **Shared Broker** (single Mosquitto instance) for resource efficiency
2. **Hierarchical Namespace** with `ndp/`, `home/`, and `shared/` prefixes
3. **Parallel Subscription** pattern for multi-subscriber scenarios
4. **Phased Migration** from legacy topics to new namespace
5. **ACL-based Security** for production isolation
