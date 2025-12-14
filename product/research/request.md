Objective
Build a generic, domain-agnostic time-series intelligence platform with agentic self-learning capabilities. The initial domain focus is indoor air quality monitoring using an AirGradient ONE sensor on Raspberry Pi, but the architecture must support extensibility to other domains (IoT, energy, finance, etc.).
Core Requirements:

Ingestion: Poll AirGradient ONE sensor data (CO2, PM2.5, VOC/NOx, temperature, humidity)
Processing: Feature engineering, anomaly detection, ML forecasting (NHITS, NBEATSx, Transformers)
Actions: Rule-based alerts, ML-driven predictions, agentic self-tuning (reflection loops that adjust thresholds/models based on outcomes)
Interface: MCP server for Claude integration, dashboards for visualization, HomeKit/MQTT export for home automation
Learning: Online learning with concept drift adaptation, model hot-swapping, performance-based parameter tuning

Hardware Context: Raspberry Pi for sensor ingestion, M4 Mac (64GB) for heavier ML inference via Ollama/local models.
Starting Point: Evaluate the existing neural-data-platform codebase (v2-phase5 branch) as a potential foundation, determining what can be reused vs. what requires refactoring for domain agnosticism.

Research Request
Analyze the following to inform architecture and implementation decisions for an agentic air quality intelligence platform, using the current repository as a potential starting point:

1. Current Codebase Analysis (neural-data-platform v2-phase5)
Objective: Determine how much of the existing trading-focused platform can be repurposed for a generic time-series intelligence engine.
Examine:

Workspace Structure: Cargo.toml — workspace members, dependencies, feature flags. Which crates exist and what are their responsibilities?
Core Abstractions: neural-core/ — Are there generic traits (TimeSeriesEvent, DataSource, Action, Agent)? Or are they trading-specific?
Data Ingestion: data_ingestion/ — What data source patterns exist? How pluggable are they for non-trading sources?
MCP Integration: mcp-trading-server/ — What tools are exposed? How tightly coupled to trading logic?
Event System: proto/ — Protobuf definitions for events. Are they generic or trading-specific?
ML/Forecasting: neural-ml-ops/, models/ — What forecasting models are implemented? NHITS, NBEATSx, ensemble patterns?
Interfaces: interfaces/ — REST API, WebSocket, output adapters?
Storage: What databases are used (TimescaleDB, Redis, SQLite)?
Agentic Capabilities: Is there a reflection loop, feedback mechanism, or self-tuning pattern?
ruv-FANN Integration: vendor/ruv-fann/ — What's vendored? How is it used?

Evaluate:

Domain Agnosticism Score: How much trading logic is embedded in core vs. isolated in domain-specific crates?
Trait Compatibility: Do existing traits align with our target design (TimeSeriesEvent, DataSource, Action, Agent)?
Refactoring Effort: Estimated work to extract a generic core and add an air-quality domain adapter
Reusable Components: Which modules can be used as-is for air quality monitoring?


2. Air Quality Analysis & Analytics (Latest Research)
Objective: Identify best practices, health thresholds, derived metrics, and ML approaches for indoor air quality monitoring.
Research:

Current standards for indoor AQI calculation from multi-sensor data (EPA, WHO guidelines)
Health-based thresholds: CO2 cognitive impact (>1000 ppm), PM2.5 cardiovascular effects (>12 µg/m³), VOC exposure limits
Derived metrics: ventilation adequacy (ACH calculation from CO2 decay curves), mold risk indices (temp/humidity correlation), thermal comfort indices
Event detection patterns: cooking detection (PM2.5 spike signatures), wildfire smoke infiltration (indoor/outdoor PM2.5 ratio), occupancy inference from CO2 rise rates
Time-series forecasting for environmental data: seasonal decomposition, HVAC cycle detection, diurnal patterns
Open datasets or benchmarks for air quality ML model training/validation
AirGradient ONE sensor specifications: SenseAir S8 (CO2), Plantower PMS5003 (PM2.5), Sensirion SGP41 (VOC/NOx) — accuracy, calibration requirements


3. Agentic Engineering & Neural Capabilities
Objective: Identify the best Rust-native ML and agentic frameworks for self-learning time-series systems.
Research:

ruvnet/ruv-FANN Ecosystem:

ruv-swarm-ml: 27+ forecasting models (NHITS, NBEATSx, LSTM, Transformers) — API patterns, Rust integration
ruvector-sona: Online learning with EWC++ (Elastic Weight Consolidation) — how it prevents catastrophic forgetting
ruv-swarm-mcp: MCP server implementation — tool definitions, Claude integration patterns
ruv-swarm-agents: Cognitive patterns (Researcher, Analyst, Optimizer) — applicable to environmental monitoring?
ruv-swarm-core: Agent orchestration topologies (mesh, ring, hierarchical)
Current maturity, documentation quality, production readiness


Alternative Rust ML Crates:

burn + burn-tch: Deep learning with PyTorch interop
augurs (Grafana): Time-series forecasting, anomaly detection, seasonality — specifically designed for monitoring
linfa: scikit-learn equivalent for Rust
smartcore: Classical ML algorithms


Agentic Patterns:

Reflection loop architectures: observe outcomes → analyze performance → decide adjustments → apply changes
Andrew Ng's agentic patterns: Reflection, Tool Use, Planning, Multi-Agent coordination
Reward shaping for environmental optimization (minimize alerts while maintaining safety)


Online Learning & Concept Drift:

ADWIN (Adaptive Windowing) for drift detection
Incremental model updates without full retraining
Model hot-swapping strategies
Performance-based threshold auto-tuning


MCP Implementation:

rmcp (official Rust SDK): stdio/SSE transports, tool registration patterns
Tool design for air quality: get_current_readings, forecast_air_quality, analyze_ventilation, get_health_recommendations




4. Dashboards & Action Delivery
Objective: Identify lightweight, Pi-compatible solutions for visualization, alerting, and home automation integration.
Research:

Dashboards:

Grafana on Raspberry Pi: resource requirements, performance
Lightweight alternatives: Streamlit, custom React/Svelte dashboards
Remote dashboard serving from M4 Mac with Pi as data collector


Storage (Pi-Friendly):

SQLite vs. QuestDB (embedded) vs. InfluxDB (note: 2.6GB crash bug on 32-bit)
Time-series data retention strategies for constrained storage
Feature caching for ML inference


Home Automation Integration:

HomeKit/Homebridge: Air quality accessory types, characteristic mappings
MQTT publishing patterns for Home Assistant, Node-RED
Apple Home air quality tile requirements


Alerting:

Push notification options (Pushover, ntfy, Home app notifications)
Alert fatigue mitigation: rate limiting, severity escalation, smart grouping


Prometheus/Metrics:

Lightweight metrics export for external monitoring
Integration with existing observability stacks




5. Reference Architectures & Prior Art
Objective: Learn from existing implementations to avoid reinventing the wheel.
Research:

Open-source air quality monitoring platforms: architecture patterns, lessons learned
AirGradient's own software stack: data formats, API patterns
Generic time-series intelligence platforms applicable beyond trading
Event-driven architectures in Rust: tokio channels, actor patterns (actix), message bus designs
Hexagonal/ports-and-adapters patterns for domain isolation


Deliverables

Codebase Assessment: Detailed analysis of neural-data-platform(this codebase) with domain-agnosticism score and refactoring roadmap
Architecture Recommendation: Which components to reuse, which to replace, and what gaps require new implementation
Technology Selection: Recommended crates/tools for ML, storage, dashboards, and home automation
Air Quality Domain Specification: Measurements, derived metrics, health thresholds, event patterns, and action definitions for the initial air quality adapter
Implementation Roadmap: Phased approach from MVP (ingestion + rules + basic dashboard) to full agentic system (ML forecasting + reflection loops + MCP integration)

Store all research and analysis in organized fashion in product/research