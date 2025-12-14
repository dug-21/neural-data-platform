# Research: Dashboards, Storage, Automation & Alerting for Raspberry Pi

**Research Date:** December 13, 2025
**Target Platform:** Raspberry Pi 4 (2-4GB RAM)
**Context:** Neural data platform with air quality monitoring

---

## 1. Dashboard Solutions

### 1.1 Grafana on Raspberry Pi

**Resource Requirements:**
- **Minimum:** 2GB RAM (4GB recommended for better performance)
- **Storage:** 16GB+ MicroSD or SSD (SSD strongly recommended for better performance and endurance)
- **Power:** Stable power supply essential for continuous operation
- **Optional:** Heatsink or fan for 24/7 operation

**Performance Characteristics:**
- Well-supported on ARM architecture (both 32-bit and 64-bit Raspberry Pi OS)
- Handles home monitoring use cases effectively (temperature, humidity, air quality, energy)
- Includes 15+ pre-configured Prometheus alerts
- 30+ essential metrics available out of the box
- Community reports good performance on Pi 4 with SSD storage

**Installation Options:**
1. Official packages from Grafana repository (apt)
2. Docker containers
3. Grafana Cloud integration available

**Best Practices:**
- Use SSD instead of SD card for better I/O performance
- Enable proper cooling for continuous operation
- Pair with InfluxDB, Telegraf, and MQTT for comprehensive IoT monitoring

**Sources:**
- [Grafana on Raspberry Pi Tutorial](https://grafana.com/tutorials/install-grafana-on-raspberry-pi/)
- [Raspberry Pi Monitoring Solution](https://grafana.com/solutions/raspberry-pi/monitor/)
- [Installation Guide - RaspberryTips](https://raspberrytips.com/install-grafana-raspberry-pi/)

---

### 1.2 Lightweight Dashboard Alternatives

#### **Dash by Plotly**
- **Advantage:** Only reruns called functions (not entire script like Streamlit)
- **Performance:** Significantly faster than Streamlit for interactive applications
- **Features:** Advanced visualization, flexible UI components, enterprise integration
- **Python-based:** Full Python framework for scalable data apps
- **Best for:** Complex dashboards with frequent user interactions

#### **Panel**
- **Flexibility:** More layout and widget options than Streamlit
- **Integration:** Works with entire PyData ecosystem
- **Scalability:** Better suited for complex web apps with intricate UIs
- **Best for:** Sophisticated dashboards requiring extensive customization

#### **Gradio**
- **Focus:** Machine learning model demos and interfaces
- **Ease of Use:** Add web UI with just a few lines of code
- **Installation:** Simple pip install
- **Best for:** ML model demonstrations, less suited for general dashboards

#### **Flask**
- **Footprint:** Most lightweight option (minimal WSGI framework)
- **Control:** Maximum flexibility and customization
- **Tradeoff:** Requires more manual implementation work
- **Best for:** Resource-constrained environments, custom solutions

#### **Svelte for Embedded Dashboards**
- **Bundle Size:** Runtime only 1.6KB vs React's 44KB
- **Performance:** 30% faster load times on average vs React (2025 benchmarks)
- **Compilation:** Compiles to vanilla JS at build time (no virtual DOM)
- **Mobile Performance:** Excellent for low-power devices and slow connections
- **Use Cases:** IoT dashboards, real-time applications, embedded widgets
- **Ecosystem:** Growing component library for robust dashboard development

#### **React Alternatives**
- **Preact:** 4KB runtime, React-compatible API
- **Solid.js:** Raw performance focus, ideal for real-time dashboards
- **React:** Better for data-heavy ecosystems with concurrent rendering

**Performance Note:** Streamlit reruns entire Python script on every interaction, causing performance issues on resource-constrained devices like Raspberry Pi.

**Recommendation Hierarchy (Pi-optimized):**
1. **Flask** - Absolute minimal footprint, full control
2. **Dash** - Best balance of Python ecosystem and performance
3. **Svelte** - For modern web-based dashboards with minimal overhead
4. **Panel** - For Python-heavy workflows requiring flexibility

**Sources:**
- [Streamlit Alternatives 2025](https://uibakery.io/blog/best-streamlit-alternatives)
- [Streamlit vs Dash Comparison](https://docs.kanaries.net/topics/Streamlit/streamlit-vs-dash)
- [Svelte vs React 2025](https://dev.to/paulthedev/svelte-vs-react-in-2025-the-ultimate-showdown-for-future-proof-frontend-development-5694)
- [React Performance Analysis](https://darktechinsights.com/react-vs-svelte-2025-enterprise-bloat/)

---

### 1.3 Remote Dashboard Architecture

**M4 Mac as Dashboard Server + Pi as Data Collector**

**Advantages:**
- Offload computational dashboard rendering to more powerful M4 Mac
- Pi focuses solely on data collection and forwarding
- Reduces Pi resource consumption significantly
- Centralized dashboard access for multiple Pi sensors

**Architecture Pattern:**
```
[Pi Sensors] → [MQTT/HTTP] → [M4 Mac: Grafana/Dashboard] ← [Users]
     ↓
[Local Storage: SQLite/QuestDB]
```

**Implementation Options:**
1. **MQTT Bridge:** Pi publishes to local MQTT, Mac subscribes and stores
2. **HTTP API:** Pi exposes lightweight API, Mac polls periodically
3. **Prometheus Remote Write:** Pi runs exporter, Mac scrapes metrics
4. **Direct Database:** Mac reads from Pi's database over network (less recommended)

**Best Practice:** Use MQTT for real-time streaming with Mac as primary dashboard host and Pi as edge collector.

---

## 2. Time-Series Storage (Pi-Optimized)

### 2.1 Database Comparison

#### **SQLite**
**Pros:**
- Minimal footprint and dependencies
- Perfect for single-program access
- Zero configuration required
- Excellent for simple use cases

**Cons:**
- Very slow for time-series: Only 8 inserts/second in testing
- Poor performance for concurrent writes
- Not optimized for time-series queries
- High I/O overhead (10x more reads than InfluxDB in tests)

**Best for:** Simple applications with low write rates, embedded caching

#### **QuestDB**
**Pros:**
- **Extremely high performance:** 2.94M rows/sec ingestion (5x faster than InfluxDB)
- **Minimal footprint:** Less than 10MB without Java runtime
- **High cardinality support:** 2.3M rows/sec with 10M unique series
- **Multi-protocol:** InfluxDB Line Protocol, PostgreSQL wire, HTTP, REST
- **SQL support:** Standard SQL queries (easier learning curve)
- **Low resource:** Designed for minimal hardware (works on Pi)
- **Embedded mode:** Can run without external dependencies

**Cons:**
- Requires 64-bit OS (won't work on 32-bit Raspberry Pi OS)
- Smaller community compared to InfluxDB
- Less mature ecosystem

**Best for:** High-performance time-series on 64-bit Pi OS, real-time analytics

**Performance Metrics:**
- 3-10x faster data ingestion than InfluxDB
- 581K rows/sec (InfluxDB) vs 2.94M rows/sec (QuestDB)
- Handles 10M unique series efficiently

#### **InfluxDB**
**Pros:**
- Mature, widely adopted time-series database
- Large community and ecosystem
- Good overall performance
- Native time-series optimizations
- Excellent documentation

**Cons:**
- **CRITICAL BUG:** InfluxDB 2.x has known crash issue on 32-bit ARM (2.6GB memory)
- Higher memory footprint than QuestDB
- Steeper learning curve (Flux query language)
- High cardinality performance issues (TSM tree per series)
- Higher I/O overhead than competitors

**Best for:** 64-bit Pi systems, established ecosystems requiring InfluxDB compatibility

**Note:** Use InfluxDB 1.x on 32-bit Pi to avoid crash bug, or upgrade to 64-bit OS for 2.x

**Sources:**
- [QuestDB Raspberry Pi 5 Benchmark](https://questdb.com/blog/raspberry-pi-5-benchmark/)
- [QuestDB vs InfluxDB Comparison](https://questdb.com/blog/2024/02/26/questdb-versus-influxdb/)
- [Time-Series DB Performance Study](https://link.springer.com/chapter/10.1007/978-3-030-50426-7_28)
- [Raspberry Pi Forum Discussion](https://forums.raspberrypi.com/viewtopic.php?t=258475)

---

### 2.2 Storage Optimization Strategies

#### **Time-Series Specific Techniques**

**Compression:**
- **Gorilla Compression:** 100GB → 1.37GB (less than 2 bytes per 16-byte point)
- **Column-store storage:** Partition techniques for efficient compression
- **Delta encoding:** Store differences instead of absolute values
- **Downsampling:** Reduce resolution for older data

**Write Optimization:**
- **LSM Tree Architecture:** Write-optimized strategy for high-throughput
  - In-memory write buffer (Memtable)
  - Multiple levels of persistent disk files
  - Batch writes to minimize I/O
- **Sequential Writes:** No in-place overwrites (flash-friendly)
- **Write-ahead logging:** Only needed for systems that overwrite

**Retention Policies:**
1. **Hot data (recent):** Full resolution, in-memory if possible
2. **Warm data (days-weeks):** Compressed, on SSD
3. **Cold data (months+):** Downsampled, archived or deleted

#### **Pi-Specific Optimizations**

**Flash/SD Card Protection:**
- Use databases that write sequentially (EmbedDB, QuestDB)
- Avoid write-ahead logging when possible
- Implement write batching (reduces write amplification)
- Consider hybrid storage: Memory + persistent

**Storage Architecture:**
```
[RAM Buffer] → [Batch Write] → [SSD/Flash]
     ↓              ↓
[Recent Data]  [Compressed Archive]
```

#### **Embedded Database Solutions**

**EmbedDB:**
- 10x faster insertion than SQLite
- Sequential page writes (no overwrites)
- No write-ahead logging needed
- Optimized for embedded systems

**FlashDB:**
- Ultra-lightweight KV and time-series support
- Designed for flash storage
- High insertion and query performance

**Hybrid Storage Research:**
- **TS-NSM:** 243.6x write IOPS improvement over InfluxDB
- **TS-PMEM:** 18.6% write throughput increase, 8.3% latency reduction
- Memory-first, storage-second architecture (GridDB approach)

**Sources:**
- [EmbedDB Research Paper](https://www.scitepress.org/Papers/2024/125581/125581.pdf)
- [FlashDB GitHub](https://github.com/armink/FlashDB)
- [Embedded IoT Time Series DB](https://onlinelibrary.wiley.com/doi/10.1155/2021/9948533)
- [Edge Database Challenges](https://greptime.com/blogs/2025-02-12-build-edge-database)

---

### 2.3 Feature Caching for ML Inference

**Strategy:** Pre-compute and cache ML features to reduce inference latency

**Implementation Options:**

1. **SQLite with TTL:**
   ```python
   # Cache computed features with expiration
   CREATE TABLE feature_cache (
     sensor_id TEXT,
     timestamp INTEGER,
     features BLOB,
     expires_at INTEGER
   )
   ```

2. **Redis (if available):**
   - In-memory cache for ultra-fast access
   - Built-in TTL support
   - May be too resource-heavy for Pi

3. **QuestDB/InfluxDB:**
   - Store computed features alongside raw data
   - Use retention policies for automatic cleanup
   - Query pre-computed features for inference

**Caching Patterns:**
- **Sliding window features:** Cache last N minutes of aggregations
- **Model embeddings:** Cache neural network intermediate layers
- **Prediction results:** Cache recent predictions to avoid recomputation

**Memory Management:**
- Limit cache size based on available RAM
- LRU eviction for memory-constrained environments
- Prioritize frequently accessed features

---

## 3. Home Automation Integration

### 3.1 HomeKit/Homebridge

#### **Air Quality Accessory in HomeKit**

**HomeKit Air Quality Characteristics:**
- **HMCharacteristicValueAirQuality:** Standard Apple API for air quality values
- **Display Values:** "Excellent", "Good", "Fair", "Poor", "Inferior"
- **Supported Metrics:**
  - PM2.5 and PM10 (particulate matter)
  - CO2 (carbon dioxide)
  - VOCs (volatile organic compounds)
  - Temperature
  - Humidity

**Home App Display:**
- Most sensors show current measurements on accessory tiles
- CO2 requires tapping tile for PPM reading
- Sensors appear under "Climate" category
- Battery level available for automation triggers (not in Home app UI)

**Notification Support:**
- iPhone push notifications for poor air quality
- CO2 sensor can send alerts for abnormal levels
- Standard push notifications (not iOS critical alerts)
- Automation triggers based on thresholds

**Sources:**
- [Apple Developer HMCharacteristicValueAirQuality](https://developer.apple.com/documentation/homekit/hmcharacteristicvalueairquality)
- [Best HomeKit Air Quality Sensors 2025](https://www.imore.com/best-homekit-air-quality-sensors)
- [Qingping Monitor Review](https://linkdhome.com/articles/qingping-air-monitor-lite-review)

#### **Homebridge Air Quality Plugins**

**homebridge-purpleair-sensor:**
- Creates virtual HomeKit sensors from PurpleAir data
- Supports both API access and local network sensors
- Enables home automation based on air quality changes
- Monitors PM2.5, temperature, humidity

**homebridge-airthings:**
- Integrates Airthings monitors via Consumer API
- Radon support via Leak Sensor workaround
- Radon contributes to Air Quality Sensor calculation

**homebridge-airnow:**
- USA-only, requires ZIP code
- Monitors outdoor air quality from AirNow API

**AirGradient Custom Firmware:**
- Direct HomeKit integration possible with custom firmware
- Requires coding knowledge
- Sends data directly to HomeKit without bridge

**homebridge-z2m:**
- Exposes Zigbee devices to HomeKit
- Integrates Zigbee2MQTT with Homebridge
- Supports various Zigbee air quality sensors

**Setup Process:**
1. Install Homebridge on Pi or separate server
2. Add air quality plugin via Homebridge UI
3. Configure sensor credentials/endpoints
4. Scan QR code in iOS Home app
5. Sensors auto-populate in Home app

**Sources:**
- [Homebridge PurpleAir Plugin](https://github.com/jmkk/homebridge-purpleair-sensor)
- [Homebridge Airthings Plugin](https://github.com/michaelahern/homebridge-airthings)
- [AirGradient HomeKit Guide](https://www.addtohomekit.com/blog/airgradient-homekit/)
- [Air Quality Monitors in HomeKit](https://seetheair.org/2021/01/26/aq-monitors-meet-apple-homekit/)

#### **Popular Native HomeKit Sensors**

**Eve Room:**
- Beautiful E Ink display with customization
- Built-in rechargeable battery (6 weeks runtime)
- Thread connectivity
- Monitors VOCs, temperature, humidity

**Aqara TVOC Air Quality Monitor:**
- Requires Aqara hub
- Compact design
- Coin battery (1 year runtime)
- VOCs, temperature, humidity

**Qingping Air Monitor Lite:**
- Dual function: air quality + digital clock
- OLED display
- Measures CO2, temperature, humidity, PM2.5, PM10
- Rechargeable battery for portability

**Netatmo Smart Weather Station:**
- Only HomeKit sensor with indoor + outdoor monitoring
- Indoor: CO2, temperature, humidity
- Outdoor: Weatherproof module

---

### 3.2 MQTT Integration

#### **MQTT Publishing Patterns**

**Topic Structure:**
```
homeassistant/sensor/neural_platform/air_quality/state
homeassistant/sensor/neural_platform/air_quality/config
```

**Home Assistant Auto-Discovery:**
```json
{
  "name": "Living Room Air Quality",
  "state_topic": "neural/livingroom/air_quality",
  "unit_of_measurement": "AQI",
  "device_class": "aqi",
  "unique_id": "neural_lr_aq_001"
}
```

**Publishing Strategies:**
1. **Periodic Updates:** Every 30-60 seconds for normal monitoring
2. **Event-Driven:** Publish immediately on threshold crossings
3. **Batch Updates:** Group multiple sensor readings in single message
4. **Retain Flag:** Use for state persistence across restarts

**Quality of Service (QoS):**
- **QoS 0:** Fire and forget (lowest overhead)
- **QoS 1:** At least once delivery (recommended for sensor data)
- **QoS 2:** Exactly once (highest overhead, rarely needed)

#### **Home Assistant Integration**

**Native MQTT Support:**
- Built-in MQTT broker option
- Auto-discovery protocol
- Dashboard cards for air quality
- Automation triggers and conditions

**Air Quality Devices:**
- **Air Lab:** $250, CO2/NOx/VOCs/temp/humidity/pressure, WiFi MQTT
- **AirGradient One:** $230, auto-configure in Home Assistant
- **Atmocube:** CO2/VOCs/PM, custom automations for threshold alerts
- **M5Stack Air Quality Kit:** ESP32S3, CO2/VOCs/PM1.0/PM2.5/PM4/PM10, e-ink display
- **Qingping Monitor:** Self-hosted MQTT support, auto-discovery

**Automation Examples:**
```yaml
# Alert on high CO2
automation:
  - trigger:
      platform: numeric_state
      entity_id: sensor.living_room_co2
      above: 1000
    action:
      - service: notify.mobile_app
        data:
          message: "CO2 exceeds 1000 ppm in living room"
      - service: switch.turn_on
        entity_id: switch.ventilation_fan
```

**Sources:**
- [Atmocube Home Assistant Integration](https://atmotube.com/blog/integrating-aqm-with-home-assistant-via-mqtt-and-setting-up-automations)
- [Air Lab Review](https://www.jeffgeerling.com/blog/2025/air-lab-flipper-zero-air-quality-monitors)
- [M5Stack Air Quality Kit](https://shop.m5stack.com/blogs/news/integrate-the-m5stack-air-quality-kit-airq-to-home-assistant)
- [Best Air Quality Monitors for Home Assistant](https://www.mightydeals.com/blog/best-air-quality-monitor-home-assistant/)

#### **DIY MQTT Solutions**

**AirQuality V3 (Wesley Elfring):**
- **Hardware:** ESP32-S3 microcontroller
- **Sensors:** PMS5003 ($17), SenseAir S8 ($18), SGP41 ($7)
- **Total Cost:** $53-57
- **Features:** Auto-discovery, periodic polling, MQTT publishing
- **Protocol:** MQTT with Home Assistant integration

**BME680 + ESPHome:**
- **Platform:** ESP8266/ESP32
- **Integration:** ESPHome framework
- **Setup Time:** ~2 hours including soldering
- **Metrics:** Barometric pressure, humidity, temperature
- **Configuration:** YAML-based, auto-discovery

**Custom Air Monitor:**
- WiFi connection
- Configurable refresh rate
- Home Assistant discovery messages
- MQTT publishing

**Sources:**
- [AirQuality V3 Project](https://www.hackster.io/news/wesley-elfring-s-airquality-v3-is-a-low-cost-broad-function-do-it-yourself-mqtt-air-quality-monitor-403b9d537bfe)
- [BME680 ESPHome Integration](https://admantium.medium.com/home-assistant-measuring-air-quality-humidity-and-temperature-with-a-bme680-sensor-012a579d6c27)
- [Custom Air Monitor GitHub](https://github.com/g8keeperzuul/AirMonitor)

#### **Node-RED Integration**

**Use Cases:**
- Visual flow-based automation
- Complex data transformations
- Multi-system orchestration
- Dashboard creation

**Pattern:**
```
[MQTT Input] → [Function: Parse & Transform] → [Multiple Outputs]
                                                ├→ Database
                                                ├→ Dashboard
                                                ├→ Alert System
                                                └→ Cloud API
```

**Advantages:**
- No coding required for basic flows
- Rich ecosystem of nodes
- Real-time data processing
- Integration with 100+ services

---

### 3.3 Apple Home Integration Requirements

**Air Quality Tile Display:**
- Primary metric: Overall air quality rating (1-5 scale)
- Tap to reveal: Individual sensor readings (CO2, PM2.5, VOCs)
- Color coding: Green (good) → Red (poor)
- Historical graphs available in detail view

**Characteristic Mappings:**
- **Air Quality:** Overall index (required)
- **PM2.5:** Particulate matter 2.5µm (optional)
- **PM10:** Particulate matter 10µm (optional)
- **VOC Density:** Volatile organic compounds (optional)
- **Carbon Dioxide:** CO2 level in ppm (optional)
- **Temperature:** Celsius/Fahrenheit (optional)
- **Humidity:** Relative humidity percentage (optional)

**Automation Triggers:**
- Air quality drops below threshold
- Specific pollutant exceeds limit
- Time-based with air quality condition
- Combination with other sensors

**Siri Integration:**
- "Hey Siri, what's the air quality?"
- "Hey Siri, what's the air quality in the [room name]?"
- "Hey Siri, what's the CO2 level in the bedroom?"

**Connectivity:**
- **Thread:** Low power, mesh networking (preferred)
- **Bluetooth:** Direct connection, no hub required
- **WiFi:** Standard connectivity
- **Bridge Required:** For non-native devices (Homebridge)

---

## 4. Alerting Systems

### 4.1 Push Notification Options

#### **ntfy - Self-Hosted Solution**

**Overview:**
- HTTP-based publish/subscribe notification service
- Completely self-hostable and open-source
- Minimal resource requirements (perfect for Pi)
- No reliance on third-party services

**Key Features:**
- Priority levels (min, low, default, high, urgent)
- File attachments support
- Action buttons in notifications
- Tags and emojis
- Authentication support
- Web app + mobile apps (iOS/Android)

**Resource Requirements:**
- Extremely lightweight (<50MB RAM typical)
- Runs well on Raspberry Pi (even older models)
- Docker support for armv6, armv7, arm64
- Can run 24/7 on low-power Pi

**Integration:**
```bash
# Simple notification
curl -d "CO2 exceeded 1000ppm" ntfy.sh/neural-alerts

# With priority and tags
curl \
  -H "Priority: high" \
  -H "Tags: warning,air-quality" \
  -d "High CO2 detected: 1200ppm" \
  ntfy.sh/neural-alerts
```

**Pros:**
- Complete data ownership
- No API costs
- Unlimited notifications
- Open source and well-documented
- Integrates with Home Assistant, Uptime Kuma, custom scripts

**Cons:**
- Requires self-hosting infrastructure
- Less polished than commercial services
- Manual setup needed
- Command-line focused (may need scripting)

**Sources:**
- [ntfy Installation Guide for Pi](https://pimylifeup.com/raspberry-pi-ntfy/)
- [ntfy Raspberry Pi Alerts](https://www.0xmm.in/posts/ntfy/)
- [Self-Hosted Notifications Guide](https://www.xda-developers.com/set-up-self-hosted-notification-service/)
- [ntfy Official Site](https://ntfy.sh/)

#### **Pushover - Commercial Solution**

**Overview:**
- Mature, reliable push notification service
- One-time payment ($5 per platform)
- Polished mobile apps
- Simple API

**Features:**
- Priority levels (silent, quiet, normal, high, emergency)
- Custom sounds
- Expiration and retry logic
- Device-specific targeting
- Attachment support (images)
- 10,000 messages/month on free tier

**Integration:**
```python
import requests

requests.post("https://api.pushover.net/1/messages.json", data={
    "token": "APP_TOKEN",
    "user": "USER_KEY",
    "message": "CO2 level critical: 1500ppm"
})
```

**Pros:**
- Most user-friendly experience
- Reliable delivery
- Beautiful mobile apps
- No infrastructure needed
- Well-documented API

**Cons:**
- Costs money ($5/platform, one-time)
- Depends on third-party service
- Privacy concerns (data sent externally)
- Message limits on free tier

**ntfy Configuration:**
- Can use Pushover as backend for ntfy
- Configured in ntfy config file
- Requires Pushover user_key

**Sources:**
- [Gotify vs Pushover vs ntfy](https://debian.ninja/post/2025/09/23/gotify-pushover-and-ntfy-real-time-notifications/)

#### **Home App Notifications (HomeKit)**

**Overview:**
- Native iOS notifications via HomeKit accessories
- Integrated with Apple ecosystem
- No additional apps required

**Features:**
- Trigger-based automations
- Critical alerts (bypass Do Not Disturb)
- Rich notifications with sensor readings
- Geofencing integration

**Notification Types:**
1. **Standard:** Regular notification banner
2. **Critical:** Bypasses silent mode (requires special entitlement)
3. **Actionable:** Quick response options

**Configuration:**
```
Home App → Automation → Create
  Trigger: CO2 > 1000ppm
  Action: Send notification "Ventilation needed"
```

**Pros:**
- Native iOS integration
- No additional apps
- Geofencing support
- Free

**Cons:**
- Apple ecosystem only
- Requires HomeKit/Homebridge
- Limited customization
- Can't bypass DND without critical alert entitlement

#### **Comparison Matrix**

| Feature | ntfy | Pushover | HomeKit |
|---------|------|----------|---------|
| Cost | Free (self-hosted) | $5 one-time | Free |
| Platforms | iOS/Android/Web | iOS/Android | iOS only |
| Self-hosted | Yes | No | N/A |
| Reliability | Good | Excellent | Excellent |
| Setup Complexity | Medium | Easy | Easy |
| Privacy | Excellent | Good | Excellent |
| Customization | High | Medium | Low |
| Pi-Friendly | Excellent | Excellent | Good |

**Recommendation:**
- **Privacy-focused, self-hosted:** ntfy
- **Ease of use, reliability:** Pushover
- **Apple ecosystem only:** HomeKit notifications
- **Hybrid:** ntfy for system alerts, Pushover for critical user notifications

---

### 4.2 Alert Fatigue Mitigation

#### **Rate Limiting Strategies**

**Time-Based Throttling:**
```python
# Don't send same alert more than once per hour
last_alert_time = {}

def send_alert(alert_type, message):
    now = time.time()
    if now - last_alert_time.get(alert_type, 0) > 3600:  # 1 hour
        notify(message)
        last_alert_time[alert_type] = now
```

**Exponential Backoff:**
```python
# Increase delay between repeated alerts
alert_count = {}

def get_backoff_delay(alert_type):
    count = alert_count.get(alert_type, 0)
    return min(300, 60 * (2 ** count))  # Max 5 minutes
```

**Burst Protection:**
```python
# Max 3 alerts per 15 minutes per type
from collections import deque

alert_timestamps = {}

def can_send_alert(alert_type, max_burst=3, window=900):
    if alert_type not in alert_timestamps:
        alert_timestamps[alert_type] = deque()

    now = time.time()
    timestamps = alert_timestamps[alert_type]

    # Remove old timestamps outside window
    while timestamps and timestamps[0] < now - window:
        timestamps.popleft()

    if len(timestamps) < max_burst:
        timestamps.append(now)
        return True
    return False
```

#### **Severity Escalation**

**Multi-Level Approach:**
1. **Info:** Log only, no notification
2. **Warning:** Desktop/web notification
3. **Alert:** Push notification
4. **Critical:** Push + SMS/call (if available)

**Threshold-Based Escalation:**
```python
def get_severity(co2_level):
    if co2_level < 800:
        return "info"
    elif co2_level < 1000:
        return "warning"
    elif co2_level < 1500:
        return "alert"
    else:
        return "critical"

def escalate_alert(sensor, value):
    severity = get_severity(value)

    if severity == "info":
        log_metric(sensor, value)
    elif severity == "warning":
        desktop_notify(f"{sensor}: {value}")
    elif severity == "alert":
        push_notify(f"⚠️ {sensor}: {value}")
    elif severity == "critical":
        push_notify(f"🚨 CRITICAL: {sensor}: {value}", priority="high")
        # Could also trigger automation (ventilation, etc.)
```

**Duration-Based Escalation:**
```python
# Only escalate if condition persists
def check_persistent_condition(condition, duration_threshold=300):
    if condition:
        condition_start = persistent_conditions.get(condition.id)
        if condition_start:
            if time.time() - condition_start > duration_threshold:
                return "escalate"
        else:
            persistent_conditions[condition.id] = time.time()
        return "monitor"
    else:
        persistent_conditions.pop(condition.id, None)
        return "clear"
```

#### **Smart Grouping**

**Temporal Grouping:**
```python
# Group alerts from same 5-minute window
alert_buffer = []

def buffer_alert(alert):
    alert_buffer.append(alert)

def flush_alerts_every_5_min():
    if alert_buffer:
        summary = f"{len(alert_buffer)} alerts: "
        summary += ", ".join(set(a.type for a in alert_buffer))
        notify(summary)
        alert_buffer.clear()
```

**Spatial Grouping:**
```python
# Group alerts from same room/zone
def group_by_location(alerts):
    by_location = {}
    for alert in alerts:
        location = alert.sensor.location
        if location not in by_location:
            by_location[location] = []
        by_location[location].append(alert)

    for location, alerts in by_location.items():
        summary = f"{location}: {len(alerts)} sensors require attention"
        notify(summary)
```

**Correlation Analysis:**
```python
# Don't alert on correlated conditions separately
def check_correlation(alert):
    # If CO2 is high and temperature is high, only alert once
    if alert.type == "high_temp" and recent_alert("high_co2"):
        return "grouped"  # Don't send separate alert
    return "send"
```

#### **User-Configurable Preferences**

**Quiet Hours:**
```python
def is_quiet_hours():
    current_hour = datetime.now().hour
    return 22 <= current_hour or current_hour < 7

def respect_quiet_hours(alert):
    if is_quiet_hours() and alert.severity < "critical":
        queue_for_morning(alert)
    else:
        send_immediately(alert)
```

**Alert Channels by Severity:**
```python
user_preferences = {
    "info": [],  # No notifications
    "warning": ["log", "dashboard"],
    "alert": ["log", "dashboard", "push"],
    "critical": ["log", "dashboard", "push", "sms"]
}

def send_via_channels(alert):
    channels = user_preferences[alert.severity]
    for channel in channels:
        send_to_channel(channel, alert)
```

**Digest Mode:**
```python
# Send single daily summary instead of individual alerts
def daily_digest():
    summary = {
        "warnings": count_by_type(severity="warning"),
        "alerts": count_by_type(severity="alert"),
        "peak_co2": max_value("co2"),
        "avg_temp": avg_value("temperature")
    }
    send_digest_email(summary)
```

#### **Machine Learning for Smart Alerts**

**Anomaly Detection:**
- Train model on normal sensor patterns
- Alert only on statistically significant deviations
- Reduces false positives from expected variations

**Pattern Recognition:**
- Learn user response patterns
- Suppress alerts user consistently ignores
- Escalate alerts user always responds to

**Predictive Alerts:**
- Forecast threshold crossings
- Alert before problem occurs
- "CO2 trending up, will exceed 1000ppm in 15 minutes"

**Implementation Considerations:**
- Start with simple rule-based system
- Add ML only after gathering sufficient data
- Keep human override controls
- Monitor alert response rates to tune thresholds

---

## 5. Prometheus & Observability

### 5.1 Lightweight Metrics Export

#### **Raspberry Pi-Specific Exporters**

**rpi_exporter (cavaliercoder):**
- Written in Go (no dependencies)
- Doesn't rely on vcgencmd
- Exposes VideoCore SoC metrics
- Minimal resource footprint

**rpi_exporter (neilmunday):**
- Python-based with Prometheus module
- Exports CPU and GPU temperatures
- Default port: 9111
- Includes Arch Linux support

**raspberrypi_exporter (fahlke):**
- Exports to node_exporter textfile collector
- Metrics: CPU temp, BCM2835 temp, ARM frequency
- Passive collection approach

**RPInfo:**
- Go-based RESTful API server
- Uses vcgencmd for hardware info
- JSON format via HTTP endpoints
- Optional /metrics endpoint for Prometheus
- Exposes: clock frequencies, CPU temp, voltage

**Node Exporter:**
- Standard system metrics exporter
- Pre-packaged in Raspberry Pi OS Bookworm
- Port 9100, /metrics endpoint
- Covers: CPU load, network, disk I/O, memory
- Official Prometheus project
- Compiled for armv7 and arm64

**Sources:**
- [rpi_export GitHub](https://github.com/cavaliercoder/rpi_export)
- [neilmunday rpi_exporter](https://github.com/neilmunday/rpi_exporter)
- [Raspberry Pi Metrics Visualization](https://blog.tschaefer.org/posts/2025/06/18/visualize-raspberry-pi-hardware-metrics/)
- [Monitor Pi with Prometheus](https://theawesomegarage.com/blog/monitor-your-raspberry-pi-with-prometheus-and-grafana)

#### **Custom Metrics for Air Quality**

**Prometheus Client Library Example:**
```python
from prometheus_client import Gauge, start_http_server

# Define metrics
air_quality_index = Gauge('air_quality_index', 'Overall air quality index')
co2_ppm = Gauge('co2_ppm', 'CO2 level in parts per million')
pm25 = Gauge('pm25_ug_m3', 'PM2.5 in micrograms per cubic meter')
temperature = Gauge('temperature_celsius', 'Temperature in Celsius')
humidity = Gauge('humidity_percent', 'Relative humidity percentage')

# Start metrics server on port 9100
start_http_server(9100)

# Update metrics
def update_metrics(sensor_data):
    air_quality_index.set(sensor_data['aqi'])
    co2_ppm.set(sensor_data['co2'])
    pm25.set(sensor_data['pm25'])
    temperature.set(sensor_data['temp'])
    humidity.set(sensor_data['humidity'])
```

**Textfile Collector Pattern:**
```bash
# More efficient for batch updates
cat > /var/lib/node_exporter/air_quality.prom <<EOF
# HELP air_quality_index Overall air quality index
# TYPE air_quality_index gauge
air_quality_index{location="living_room"} 42

# HELP co2_ppm CO2 level in parts per million
# TYPE co2_ppm gauge
co2_ppm{location="living_room"} 850
EOF
```

**Exporter Design Considerations:**
- Keep scrape interval reasonable (30-60s for air quality)
- Use labels for multi-room deployments
- Implement `/health` endpoint
- Cache sensor readings between scrapes
- Handle sensor failures gracefully

#### **Resource Optimization**

**Scrape Configuration:**
```yaml
# prometheus.yml
scrape_configs:
  - job_name: 'raspberry-pi'
    scrape_interval: 60s  # Don't scrape too frequently
    static_configs:
      - targets: ['localhost:9100']  # node_exporter
      - targets: ['localhost:9111']  # rpi_exporter
      - targets: ['localhost:9200']  # custom air quality exporter
```

**Metric Cardinality Management:**
- Avoid high-cardinality labels (e.g., timestamps)
- Use finite label values (room names, sensor IDs)
- Limit number of unique time series
- Drop unnecessary metrics

**Memory-Efficient Retention:**
```yaml
# Limit local Prometheus storage
storage:
  tsdb:
    retention.time: 7d  # Keep only 1 week locally
    retention.size: 1GB  # Or max 1GB
```

---

### 5.2 Integration with External Observability Stacks

#### **Remote Write to Central Prometheus**

**Configuration:**
```yaml
# On Raspberry Pi Prometheus
remote_write:
  - url: "https://prometheus.example.com/api/v1/write"
    basic_auth:
      username: "pi-sensors"
      password: "secret"
    queue_config:
      capacity: 10000
      max_shards: 5
      min_shards: 1
      max_samples_per_send: 1000
      batch_send_deadline: 5s
```

**Advantages:**
- Centralized metrics storage
- Offload retention to more capable server
- Reduce Pi storage requirements
- Enable cross-device queries

**Considerations:**
- Network dependency (buffer if offline)
- Upload bandwidth usage
- Central server storage capacity

#### **Grafana Cloud Integration**

**Free Tier:**
- 10,000 series
- 50GB logs
- 50GB traces
- 14-day retention

**Setup:**
```bash
# Add Grafana Cloud remote_write endpoint
remote_write:
  - url: https://prometheus-prod-XX.grafana.net/api/prom/push
    basic_auth:
      username: YOUR_USERNAME
      password: YOUR_API_KEY
```

**Pi-Specific Dashboards:**
- Pre-built Raspberry Pi monitoring dashboard
- 30+ essential metrics
- 15 pre-configured alerts
- CPU, memory, disk, network

**Sources:**
- [Raspberry Pi Grafana Cloud Integration](https://grafana.com/docs/grafana-cloud/monitor-infrastructure/integrations/integration-reference/integration-raspberry-pi-node/)

#### **VictoriaMetrics as Lightweight Alternative**

**Why VictoriaMetrics:**
- More resource-efficient than Prometheus
- Better compression (10x less disk space)
- Faster queries on historical data
- Drop-in Prometheus replacement

**Resource Comparison:**
- 7x less RAM than Prometheus
- 7x less CPU usage
- Storage: 0.3 bytes per datapoint vs 3.3 bytes (Prometheus)

**Configuration:**
```bash
# Run VictoriaMetrics instead of Prometheus on Pi
./victoria-metrics-prod \
  -storageDataPath=/var/lib/victoria-metrics \
  -retentionPeriod=30d \
  -memory.allowedPercent=50
```

#### **InfluxDB as Alternative Backend**

**Prometheus → InfluxDB:**
```yaml
# Use Telegraf to bridge
[[inputs.prometheus]]
  urls = ["http://localhost:9100/metrics"]

[[outputs.influxdb_v2]]
  urls = ["http://influxdb.local:8086"]
  token = "$INFLUX_TOKEN"
  organization = "home"
  bucket = "sensors"
```

**Advantages:**
- Native time-series optimizations
- Better long-term retention
- SQL-like query language (InfluxQL)
- Continuous queries for downsampling

#### **Hybrid Architecture Example**

```
┌─────────────────┐
│  Raspberry Pi   │
├─────────────────┤
│ Sensors         │
│ rpi_exporter    │──┐
│ node_exporter   │  │
│ custom_exporter │  │
└─────────────────┘  │
                     │
                     ├──► Local Prometheus
                     │    (7-day retention)
                     │         │
                     │         ├──► Grafana (local)
                     │         └──► Local alerts
                     │
                     └──► Remote Write
                              │
                    ┌─────────┴──────────┐
                    │                    │
            ┌───────▼──────┐    ┌───────▼─────────┐
            │   Grafana    │    │  VictoriaMetrics │
            │    Cloud     │    │   (on M4 Mac)    │
            └──────────────┘    └──────────────────┘
            (visualization)      (long-term storage)
```

**Benefits:**
- Local monitoring continues if network fails
- Long-term data on more capable hardware
- Reduced Pi storage requirements
- Centralized cross-device visibility

---

## 6. Recommendations Summary

### 6.1 Optimal Pi Configuration

**Dashboard:**
- **Primary:** Grafana on M4 Mac (remote dashboard server)
- **Fallback:** Lightweight Flask/Dash dashboard on Pi for local access
- **Mobile:** Svelte-based web dashboard for minimal footprint

**Storage:**
- **64-bit Pi OS:** QuestDB (best performance, 2.94M rows/sec)
- **32-bit Pi OS:** SQLite for simple cases, InfluxDB 1.x for time-series
- **Hybrid:** QuestDB for hot data + compressed archives for cold data

**Home Automation:**
- **Primary:** MQTT → Home Assistant (most flexible)
- **Secondary:** Homebridge for HomeKit integration
- **Native:** Thread-based HomeKit sensors where available

**Alerting:**
- **Self-hosted:** ntfy for system alerts and privacy
- **Critical:** Pushover for important user notifications
- **iOS:** HomeKit automations for in-ecosystem alerts

**Metrics:**
- **Local:** Prometheus + node_exporter + custom air quality exporter
- **Remote:** Remote write to VictoriaMetrics on M4 Mac
- **Visualization:** Grafana on M4 Mac with pre-built Pi dashboards

### 6.2 Resource Budget (Raspberry Pi 4, 4GB RAM)

**Memory Allocation:**
- OS + Base: 500MB
- QuestDB: 512MB
- Prometheus: 256MB
- Node exporter + custom exporters: 100MB
- ntfy: 50MB
- MQTT client: 50MB
- ML inference: 1GB
- Headroom: 1.5GB

**Storage Strategy:**
- Use 128GB+ SSD (not SD card)
- QuestDB: 30-day full retention
- Prometheus: 7-day local retention
- Logs: 14-day rotation
- Remote archive: Unlimited on M4 Mac

**Network Bandwidth:**
- MQTT: ~1KB/message × 60 msgs/hour = 60KB/hour
- Prometheus remote write: ~10KB/min = 600KB/hour
- Dashboard access: Variable (local only when needed)
- Total: <1MB/hour typical

### 6.3 Implementation Priority

**Phase 1: Foundation**
1. Install 64-bit Raspberry Pi OS
2. Set up QuestDB for time-series storage
3. Install node_exporter for system metrics
4. Deploy MQTT broker (Mosquitto)

**Phase 2: Monitoring**
1. Create custom air quality Prometheus exporter
2. Configure Prometheus with remote write to M4 Mac
3. Set up Grafana on M4 Mac
4. Import Raspberry Pi dashboard templates

**Phase 3: Automation**
1. Install Home Assistant (on separate Pi or M4 Mac)
2. Configure MQTT auto-discovery
3. Create basic automations (ventilation triggers)
4. Optional: Add Homebridge for HomeKit

**Phase 4: Alerting**
1. Deploy ntfy for self-hosted notifications
2. Configure alert rules in Prometheus
3. Set up Pushover for critical alerts
4. Implement alert fatigue mitigation

**Phase 5: Optimization**
1. Tune QuestDB retention and compression
2. Implement feature caching for ML
3. Set up automated backups
4. Performance profiling and optimization

---

## 7. Additional Resources

### Documentation
- [QuestDB Official Docs](https://questdb.io/docs/)
- [Prometheus Raspberry Pi Guide](https://prometheus.io/docs/guides/node-exporter/)
- [Home Assistant MQTT Discovery](https://www.home-assistant.io/integrations/mqtt/)
- [ntfy Documentation](https://docs.ntfy.sh/)
- [Grafana Raspberry Pi Tutorials](https://grafana.com/tutorials/install-grafana-on-raspberry-pi/)

### Community Resources
- Raspberry Pi Forums: Hardware optimization tips
- Home Assistant Community: Automation examples
- r/homelab: Self-hosting best practices
- Grafana Community: Dashboard sharing

### Reference Architectures
- [IoT Data Pipeline on Pi](https://github.com/bhemar/raspberry-metrics)
- [Home Air Quality Monitor](https://github.com/g8keeperzuul/AirMonitor)
- [Prometheus + Grafana Setup](https://theawesomegarage.com/blog/monitor-your-raspberry-pi-with-prometheus-and-grafana)

---

## Appendix: Full Source List

### Dashboards
- [Install Grafana on Raspberry Pi | Grafana Labs](https://grafana.com/tutorials/install-grafana-on-raspberry-pi/)
- [Raspberry Pi monitoring made easy | Grafana Labs](https://grafana.com/solutions/raspberry-pi/monitor/)
- [How To Install & Use Grafana On Raspberry Pi – RaspberryTips](https://raspberrytips.com/install-grafana-raspberry-pi/)
- [5 Best Streamlit Alternatives in 2025 | UI Bakery Blog](https://uibakery.io/blog/best-streamlit-alternatives)
- [Streamlit vs Dash: Which Framework is Right for You?](https://docs.kanaries.net/topics/Streamlit/streamlit-vs-dash)
- [Svelte vs React in 2025: The Ultimate Showdown](https://dev.to/paulthedev/svelte-vs-react-in-2025-the-ultimate-showdown-for-future-front end-development-5694)
- [React Is Undead in 2025? Why Developers Are Leaving for Svelte](https://darktechinsights.com/react-vs-svelte-2025-enterprise-bloat/)

### Storage
- [QuestDB and Raspberry Pi 5 benchmark](https://questdb.com/blog/raspberry-pi-5-benchmark/)
- [QuestDB - Lightweight Open Source Time-Series DB](https://forums.raspberrypi.com/viewtopic.php?t=258475)
- [Benchmark and comparison: QuestDB vs. InfluxDB](https://questdb.com/blog/2024/02/26/questdb-versus-influxdb/)
- [Comparing InfluxDB, TimescaleDB, and QuestDB](https://questdb.com/blog/comparing-influxdb-timescaledb-questdb-time-series-databases/)
- [Comparative Analysis of Time Series Databases](https://link.springer.com/chapter/10.1007/978-3-030-50426-7_28)
- [EmbedDB: A High-Performance Time Series Database](https://www.scitepress.org/Papers/2024/125581/125581.pdf)
- [FlashDB GitHub](https://github.com/armink/FlashDB)
- [Embedded IoT Time Series Database](https://onlinelibrary.wiley.com/doi/10.1155/2021/9948533)
- [Challenges and Solutions - Building Edge Databases](https://greptime.com/blogs/2025-02-12-build-edge-database)

### Home Automation
- [Best HomeKit air quality sensors in 2025](https://www.imore.com/best-homekit-air-quality-sensors)
- [GitHub - homebridge-purpleair-sensor](https://github.com/jmkk/homebridge-purpleair-sensor)
- [GitHub - homebridge-airthings](https://github.com/michaelahern/homebridge-airthings)
- [How to Add AirGradient to Apple HomeKit](https://www.addtohomekit.com/blog/airgradient-homekit/)
- [AQ Monitors meet Apple HomeKit](https://seetheair.org/2021/01/26/aq-monitors-meet-apple-homekit/)
- [HMCharacteristicValueAirQuality | Apple Developer](https://developer.apple.com/documentation/homekit/hmcharacteristicvalueairquality)
- [Integrating AQM with Home Assistant via MQTT](https://atmotube.com/blog/integrating-aqm-with-home-assistant-via-mqtt-and-setting-up-automations)
- [Air Lab Review | Jeff Geerling](https://www.jeffgeerling.com/blog/2025/air-lab-flipper-zero-air-quality-monitors)
- [Top 3 Best Air Quality Monitor Home Assistant](https://www.mightydeals.com/blog/best-air-quality-monitor-home-assistant/)
- [AirQuality V3 DIY Monitor | Hackster.io](https://www.hackster.io/news/wesley-elfring-s-airquality-v3-is-a-low-cost-broad-function-do-it-yourself-mqtt-air-quality-monitor-403b9d537bfe)
- [M5Stack Air Quality Kit Integration](https://shop.m5stack.com/blogs/news/integrate-the-m5stack-air-quality-kit-airq-to-home-assistant)

### Alerting
- [I set up a self-hosted notification service](https://www.xda-developers.com/set-up-self-hosted-notification-service/)
- [Installing the NTFY Server on Raspberry Pi](https://pimylifeup.com/raspberry-pi-ntfy/)
- [Raspberry Pi Notifications with ntfy](https://www.0xmm.in/posts/ntfy/)
- [Gotify, Pushover and ntfy comparison](https://debian.ninja/post/2025/09/23/gotify-pushover-and-ntfy-real-time-notifications/)
- [ntfy.sh Official Site](https://ntfy.sh/)

### Prometheus
- [rpi_export GitHub](https://github.com/cavaliercoder/rpi_export)
- [Monitor Pi with Prometheus and Grafana](https://theawesomegarage.com/blog/monitor-your-raspberry-pi-with-prometheus-and-grafana)
- [raspberrypi_exporter GitHub](https://github.com/fahlke/raspberrypi_exporter)
- [raspberry-metrics GitHub](https://github.com/bhemar/raspberry-metrics)
- [Installing Prometheus on the Raspberry Pi](https://pimylifeup.com/raspberry-pi-prometheus/)
- [neilmunday rpi_exporter](https://github.com/neilmunday/rpi_exporter)
- [Visualize Raspberry Pi Hardware Metrics](https://blog.tschaefer.org/posts/2025/06/18/visualize-raspberry-pi-hardware-metrics/)
- [Raspberry Pi Grafana Cloud Integration](https://grafana.com/docs/grafana-cloud/monitor-infrastructure/integrations/integration-reference/integration-raspberry-pi-node/)

---

**Document Version:** 1.0
**Last Updated:** December 13, 2025
**Next Review:** January 2026
