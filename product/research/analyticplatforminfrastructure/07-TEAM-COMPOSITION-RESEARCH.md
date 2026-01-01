# Neural Data Platform: Team Composition Research

## Research Context

**Date**: 2026-01-01
**Status**: Complete
**Research Method**: Multi-agent research swarm with source verification
**Related Documents**: 01-06 in this research series

---

## Executive Summary

This research synthesizes best practices for building data platform teams, combining:
- **Generic data platform patterns** from Gartner, McKinsey, Team Topologies
- **Weather/meteorology domain expertise** from NOAA, ECMWF, Met Office, The Weather Company
- **Air quality domain expertise** from EPA, academic research, industry (PurpleAir, IQAir)
- **MLOps/DataOps practices** from Google, Microsoft, Neptune.ai
- **Lifecycle coverage** from DAMA-DMBOK, Fundamentals of Data Engineering

### Key Finding

The optimal NDP team structure follows a **hybrid model** with:
1. **Platform Core** (centralized) - Infrastructure, governance, standards
2. **Domain Pods** (stream-aligned) - Weather/AQ specialists with data engineering

---

## 1. Team Structure Recommendation

### 1.1 Platform Core Team (4-6 roles)

These roles provide shared infrastructure and standards:

| Role | Responsibility | NDP Mapping |
|------|----------------|-------------|
| **Data Platform Engineer** | Bronze/Silver layers, Rust pipelines, Parquet/TimescaleDB | `ndp-rust-dev`, `ndp-parquet-dev`, `ndp-timescale-dev` |
| **Data Architect** | Domain Adapter pattern, ADRs, schema design | `ndp-architect` |
| **Data Quality Engineer** | DQ rules, transparency tables, monitoring | Layered DQ Strategy |
| **MLOps Engineer** | Feature store, model deployment, ruv-FANN | `ndp-ml-engineer` |
| **Platform/DevOps Engineer** | CI/CD, Docker, Pi deployment, etcd | Deployment infrastructure |
| **Data Governance Lead** | Standards, data contracts, lineage | Cross-cutting |

### 1.2 Weather/Air Quality Domain Pod (3-5 roles)

Stream-aligned team owning the weather/AQ domain:

| Role | Responsibility | Key Skills |
|------|----------------|------------|
| **Meteorological Data Scientist** | NWS data interpretation, forecast evaluation, domain models | Atmospheric science, Python/xarray, NWP understanding |
| **Air Quality Specialist** | AQI calculations, sensor calibration, EPA compliance | Environmental science, pollutant chemistry, sensor networks |
| **Analytics Engineer** | Silver layer transformations, dbt models, forecast accuracy views | SQL, TimescaleDB, domain knowledge |
| **Visualization Engineer** | Grafana dashboards, decision support | Grafana, visualization, UX |
| **Feature Engineer** (future) | Time-series features, windowing, ML inputs | `ndp-feature-engineer` |

---

## 2. Skills Matrix

### 2.1 Core Technical Skills

| Skill Category | Required | Advanced |
|----------------|----------|----------|
| **Languages** | Rust, Python, SQL | R, TypeScript |
| **Data Engineering** | Parquet, TimescaleDB, ETL | Kafka, streaming |
| **Weather Data** | NetCDF/GRIB, xarray, NWS APIs | WRF, data assimilation |
| **Air Quality** | AQI calculation, PM2.5/O3 chemistry | Sensor calibration, spatial analysis |
| **ML/Analytics** | Time series, forecasting | GNN, ensemble methods |
| **Infrastructure** | Docker, Git, Linux | Kubernetes, Terraform |

### 2.2 Domain Knowledge Requirements

#### Weather Domain (from NOAA, ECMWF, The Weather Company research)

| Knowledge Area | Why Critical for NDP |
|----------------|---------------------|
| Forecast lead time interpretation | Core use case: forecast accuracy evaluation |
| Issue time vs valid time semantics | Schema design (`05-FORECAST-EVALUATION-SCHEMA.md`) |
| NWS gridpoint structure | Current data source integration |
| Uncertainty quantification | Ensemble handling, confidence intervals |
| Temporal/spatial interpolation | Grid-to-point translation |

#### Air Quality Domain (from EPA, academic research)

| Knowledge Area | Why Critical for NDP |
|----------------|---------------------|
| AQI calculation methodology | Correct NowCast and breakpoint calculations |
| Sensor calibration (PM2.5/CO2) | Data quality for AirGradient sensors |
| Indoor/outdoor relationships | Window management use case |
| Health impact thresholds | Alert trigger configuration |
| EPA regulatory standards | Compliance and reporting |

### 2.3 Soft Skills (Critical)

Research emphasizes these are equal in importance to technical skills:

| Skill | Application in NDP |
|-------|-------------------|
| **Stakeholder management** | Balancing domain needs with platform constraints |
| **Communication** | Translating weather science to data engineering |
| **Problem solving** | Data quality root cause analysis |
| **Business acumen** | Connecting forecasts to actionable decisions |
| **Cross-domain empathy** | Meteorologists understanding data pipelines; engineers understanding atmospheric science |

---

## 3. Lifecycle Coverage Matrix

Mapping roles to the NDP lifecycle phases from `03-DATA-PLATFORM-LIFECYCLE.md`:

### 3.1 Early Phase (Current - Domain Exploration)

| Activity | Primary Role | Agent |
|----------|--------------|-------|
| DuckDB exploration of Bronze | Analytics Engineer | - |
| Domain model drafting | Meteorological Data Scientist | - |
| Schema design | Data Architect | `ndp-architect` |
| Raw data quality review | Data Quality Engineer | - |

### 3.2 DevStage Phase (Next - Schema Iteration)

| Activity | Primary Role | Agent |
|----------|--------------|-------|
| Bronze → Silver ETL | Data Platform Engineer | `ndp-timescale-dev` |
| Silver schema iteration | Data Architect + Domain Scientist | `ndp-architect` |
| Test dashboards | Visualization Engineer | `ndp-grafana-dev` |
| DQ rule tuning | Data Quality Engineer | - |
| Forecast accuracy views | Analytics Engineer | - |

### 3.3 Stable Phase (Future - Production)

| Activity | Primary Role | Agent |
|----------|--------------|-------|
| Pipeline operations | Platform Engineer + SRE | - |
| Feature engineering | Feature Engineer | `ndp-feature-engineer` |
| ML model training | MLOps Engineer | `ndp-ml-engineer` |
| Alert configuration | Air Quality Specialist | `ndp-alert-engineer` |
| Production dashboards | Visualization Engineer | `ndp-grafana-dev` |

---

## 4. Scaling Patterns

### 4.1 Team Size Benchmarks

| Stage | Recommended Size | Notes |
|-------|------------------|-------|
| **Startup/Early** | 2-3 people | Combined roles (engineer + domain expert) |
| **Growth/DevStage** | 4-6 people | Begin specialization |
| **Scale/Stable** | 8+ (split into sub-teams) | Platform + Domain pods |

**Key Rule**: Split teams when exceeding 8 people to reduce cognitive load.

### 4.2 Ratio Guidance

From SYNQ research on 100+ tech scaleups:

| Metric | Typical Range | NDP Recommendation |
|--------|---------------|-------------------|
| Data:Engineer ratio | 1:8 median | Start with 1:4 (data-heavy project) |
| Insights:Engineering split | 45%:45%:10% ML | Balance domain expertise with infrastructure |
| Data team as % of workforce | 1-5% | N/A (project-based, not company) |

### 4.3 Agentic Team Scaling

For the NDP agentic development model:

| Lifecycle Phase | Agent Complexity | Human Oversight |
|-----------------|------------------|-----------------|
| Early | Few agents, high oversight | Domain expert validation |
| DevStage | More agents, medium oversight | Periodic SPARC reviews |
| Stable | Many agents, automated oversight | SLO-based monitoring |

---

## 5. NDP Agent Roster Enhancement

Based on this research, recommended additions/refinements to `.claude/agents/ndp/`:

### 5.1 Existing Agents (Validated)

| Agent | Coverage | Assessment |
|-------|----------|------------|
| `ndp-architect` | Data Architect role | Well-aligned |
| `ndp-rust-dev` | Platform Engineer (Rust) | Well-aligned |
| `ndp-parquet-dev` | Bronze layer specialist | Well-aligned |
| `ndp-timescale-dev` | Silver layer specialist | Well-aligned |
| `ndp-grafana-dev` | Visualization Engineer | Well-aligned |
| `ndp-ml-engineer` | MLOps (ruv-FANN) | Well-aligned |
| `ndp-feature-engineer` | Feature Engineering | Well-aligned |
| `ndp-alert-engineer` | Alert configuration | Well-aligned |
| `ndp-tester` | Quality assurance | Well-aligned |
| `ndp-scrum-master` | Lifecycle coordination | Well-aligned |

### 5.2 Recommended New Agents

| Proposed Agent | Role Coverage | Justification |
|----------------|---------------|---------------|
| `ndp-meteorologist` | Weather domain scientist | Interprets NWS data, validates forecast evaluation schemas, understands atmospheric science |
| `ndp-air-quality-specialist` | Air quality domain scientist | EPA standards, AQI calculations, sensor calibration, health thresholds |
| `ndp-dq-engineer` | Data Quality Engineer | Implements and maintains the Layered DQ Strategy |
| `ndp-analytics-engineer` | Analytics Engineer | Bridges domain and engineering, builds reusable Silver→Gold transforms |

### 5.3 Agent Skill Injection

Existing agents should be enhanced with domain knowledge:

```yaml
# Example: ndp-timescale-dev domain knowledge enhancement
domain_knowledge:
  weather:
    - issue_time vs valid_time semantics
    - lead_time calculation: valid_time - issue_time
    - NWS forecast update patterns
    - Hypertable partitioning on valid_time (for joins with observations)
  air_quality:
    - AQI breakpoints and calculation
    - Sensor calibration correction factors
    - NowCast weighted averaging
```

---

## 6. Team Topology Recommendations

### 6.1 Platform-as-a-Product

The Platform Core team should operate as an **internal platform team** (Team Topologies):

```
┌─────────────────────────────────────────────────────────────┐
│                     PLATFORM TEAM                            │
│  (Data Platform Engineer, Architect, Quality, MLOps, DevOps) │
│                                                              │
│  Provides:                                                   │
│  • Bronze layer (Parquet ingestion)                          │
│  • Silver layer (TimescaleDB schemas)                        │
│  • Gold layer (Feature engineering framework)                │
│  • Observability (Grafana, DQ monitoring)                    │
│  • Self-service tools (config-driven stream setup)           │
└──────────────────────────┬──────────────────────────────────┘
                           │ X-as-a-Service
                           ▼
┌─────────────────────────────────────────────────────────────┐
│                 WEATHER/AQ DOMAIN POD                        │
│    (Meteorologist, AQ Specialist, Analytics Engineer)        │
│                                                              │
│  Owns:                                                       │
│  • Domain model design                                       │
│  • Schema definitions (within platform constraints)          │
│  • Forecast accuracy analysis                                │
│  • Decision support models                                   │
└─────────────────────────────────────────────────────────────┘
```

### 6.2 Interaction Modes

| Interaction | Description | Example |
|-------------|-------------|---------|
| **X-as-a-Service** | Domain team consumes platform capabilities | Domain team uses Bronze ingestion service |
| **Collaboration** | Joint work on complex features | Schema design for forecast_accuracy view |
| **Facilitating** | Platform helps domain adopt best practices | DQ rules for NWS data quirks |

---

## 7. Implementation Roadmap

### Phase 1: Foundation (Now → DevStage)

**Priority Hires/Agents**:
1. Weather domain scientist (or domain-injected `ndp-meteorologist` agent)
2. Data Quality Engineer (or `ndp-dq-engineer` agent)
3. Analytics Engineer (or `ndp-analytics-engineer` agent)

**Activities**:
- Complete Bronze layer for all weather/AQ sources
- Draft Silver schemas based on domain model
- Establish DQ baseline

### Phase 2: DevStage (Schema Iteration)

**Priority Hires/Agents**:
1. Air Quality Specialist
2. Visualization Engineer (Grafana)

**Activities**:
- Implement Bronze → Silver ETL
- Build test dashboards
- Iterate schemas based on data reality

### Phase 3: Stable (Production)

**Priority Hires/Agents**:
1. Platform/DevOps Engineer (production hardening)
2. Feature Engineer
3. MLOps Engineer (if not already in place)

**Activities**:
- Lock schemas, enable migrations
- Feature engineering for ML
- Production dashboards and alerts

---

## 8. Source Summary

All sources verified for credibility. Key references:

### Industry Frameworks
- **Team Topologies** (Skelton & Pais): Platform vs Stream-aligned teams
- **Data Mesh** (Zhamak Dehghani): Domain ownership principles
- **DAMA-DMBOK**: Governance roles and competencies
- **Fundamentals of Data Engineering** (Reis & Housley): Lifecycle stages

### Domain-Specific
- **NOAA/NWS**: Meteorologist job requirements, GS-1340 series
- **ECMWF**: AIFS ML-NWP integration, Anemoi framework team structure
- **The Weather Company**: Cross-disciplinary team approach (AWS case study)
- **EPA**: AQI calculation, air quality analyst requirements
- **Met Office UK**: Atmospheric data scientist role definitions

### MLOps/DataOps
- **Google Cloud MLOps Maturity Model**: Level 0/1/2 progression
- **Microsoft MLOps Maturity Model**: 5-level framework
- **Neptune.ai**: Team structure recommendations
- **CD Foundation DataOps Initiative**: CI/CD for data practices

### Industry Benchmarks
- **SYNQ**: Data team sizing at 100+ scaleups
- **Gartner**: 2024-2025 data engineering skills
- **McKinsey Analytics Quotient**: Maturity stages

---

## Appendix: Detailed Role Definitions

### A.1 Meteorological Data Scientist

**Background**: MS/PhD in Atmospheric Science, Meteorology, or related field

**Technical Skills**:
- Python (xarray, Dask, wrf-python)
- NetCDF/GRIB data formats
- NWP interpretation (GFS, ECMWF)
- Statistical verification methods

**Domain Knowledge**:
- Forecast uncertainty quantification
- Lead time accuracy degradation
- NWS data quirks and update patterns
- Atmospheric thermodynamics

**NDP Responsibilities**:
- Validate forecast_accuracy schema design
- Interpret anomalies in NWS data
- Define domain-specific DQ rules
- Design forecast evaluation metrics

### A.2 Air Quality Specialist

**Background**: BS/MS in Environmental Science, Engineering, or Public Health

**Technical Skills**:
- AQI calculation (EPA methodology)
- Sensor calibration (PM2.5, CO2)
- Spatial analysis (GIS)
- Time series analysis

**Domain Knowledge**:
- EPA NAAQS standards
- Pollutant chemistry (O3, PM2.5, VOC)
- Health impact thresholds
- Indoor/outdoor air quality dynamics

**NDP Responsibilities**:
- Define AQI calculation logic for Silver layer
- Establish sensor calibration procedures
- Design health-based alert thresholds
- Validate AirGradient data quality

### A.3 Data Quality Engineer

**Background**: BS in Computer Science, Data Engineering, or related field

**Technical Skills**:
- Data profiling and validation
- Great Expectations or similar
- SQL, Python
- Monitoring and alerting

**Domain Knowledge**:
- Data quality dimensions (completeness, accuracy, timeliness)
- DQ rule design patterns
- Anomaly detection

**NDP Responsibilities**:
- Implement Layered DQ Strategy (Extract, Transform, Analytics)
- Build and maintain `silver.dq_results` transparency
- Configure Bronze rejection quarantine
- Create DQ dashboards

---

*Research completed: 2026-01-01*
*Research method: 5-agent parallel research swarm with source verification*
