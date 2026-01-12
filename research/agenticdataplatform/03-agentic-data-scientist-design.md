# Agentic Data Scientist Design

**Version**: 1.0.0
**Date**: 2026-01-03
**Status**: Design Proposal
**Author**: Hive-Mind Research Swarm (Analyst Agent)

---

## Executive Summary

This document defines the **Agentic Data Scientist** concept for the Neural Data Platform. These AI agents enable natural language data exploration, automated analytics, and intelligent decision support within the constraints of the NDP's edge-first architecture (dev container isolated from Pi production container).

The design introduces five specialized data science agents that collaborate to provide end-to-end analytical workflows, from raw data exploration to actionable insights.

---

## 1. Agent Types

### 1.1 Data Explorer Agent (`ndp-data-explorer`)

**Purpose**: Browse Bronze/Silver layers, generate exploratory data analysis (EDA), discover patterns.

**Scope**: Specialized - Data exploration and profiling

**Capabilities**:
- Schema discovery (Parquet files, TimescaleDB tables)
- Data profiling (distributions, cardinality, nulls)
- Time-series pattern detection
- Anomaly identification during exploration
- Sample data retrieval and visualization

**Key Interactions**:
```
┌────────────────────────────────────────────────────────────────────────┐
│                        DATA EXPLORER AGENT                              │
├────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  User: "What does the air quality data look like for the past week?"   │
│                                                                         │
│  ┌──────────────┐      ┌──────────────┐      ┌──────────────┐         │
│  │ Schema       │ ───► │ Sample       │ ───► │ Profile      │         │
│  │ Discovery    │      │ Retrieval    │      │ Generation   │         │
│  └──────────────┘      └──────────────┘      └──────────────┘         │
│         │                     │                     │                  │
│         ▼                     ▼                     ▼                  │
│  "Found: pm25, co2,     "7,344 rows,          "PM2.5: mean=12.3,      │
│   temp, humidity,        hourly granularity,   max=89.2, nulls=0.1%,  │
│   timestamp..."          3 sensors active"      distribution: normal" │
│                                                                         │
└────────────────────────────────────────────────────────────────────────┘
```

**Core Functions**:
```python
class DataExplorerAgent:
    async def discover_schemas(self, layer: str) -> SchemaInventory:
        """Enumerate available streams, tables, and their schemas."""

    async def profile_stream(self, stream_id: str, time_range: TimeRange) -> DataProfile:
        """Generate statistical profile for a data stream."""

    async def sample_data(self, stream_id: str, sample_size: int) -> DataFrame:
        """Retrieve representative sample for inspection."""

    async def detect_patterns(self, stream_id: str) -> List[Pattern]:
        """Identify seasonal, trend, or cyclical patterns."""

    async def summarize_for_user(self, exploration_results: ExplorationResults) -> str:
        """Generate human-readable summary of findings."""
```

---

### 1.2 SQL Synthesizer Agent (`ndp-sql-synthesizer`)

**Purpose**: Translate natural language questions into DuckDB SQL queries.

**Scope**: Specialized - Query generation and optimization

**Capabilities**:
- Natural language to SQL translation
- Schema-aware query generation
- Query optimization for DuckDB/TimescaleDB
- Query explanation and validation
- Result interpretation

**Key Interactions**:
```
┌────────────────────────────────────────────────────────────────────────┐
│                      SQL SYNTHESIZER AGENT                              │
├────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  User: "Compare indoor and outdoor PM2.5 when windows were likely      │
│         open (temp diff < 2 degrees)"                                   │
│                                                                         │
│  ┌──────────────┐      ┌──────────────┐      ┌──────────────┐         │
│  │ Intent       │ ───► │ Schema       │ ───► │ SQL          │         │
│  │ Parsing      │      │ Lookup       │      │ Generation   │         │
│  └──────────────┘      └──────────────┘      └──────────────┘         │
│         │                     │                     │                  │
│         ▼                     ▼                     ▼                  │
│  Intent:               Available tables:      Generated SQL:           │
│  - compare (action)    - silver_indoor_air   "SELECT time_bucket(...)" │
│  - pm25 (metric)       - silver_outdoor_...                            │
│  - temp_diff (filter)  - cross_stream_aligned                          │
│                                                                         │
│  ┌──────────────┐      ┌──────────────┐                               │
│  │ Query        │ ───► │ Result       │                               │
│  │ Execution    │      │ Interpretation│                               │
│  └──────────────┘      └──────────────┘                               │
│         │                     │                                        │
│         ▼                     ▼                                        │
│  Rows: 847             "When windows were likely open, indoor PM2.5    │
│  Columns: 5             averaged 8.2 ug/m3 vs 6.1 ug/m3 outdoors,     │
│                         suggesting effective ventilation."             │
│                                                                         │
└────────────────────────────────────────────────────────────────────────┘
```

**Core Functions**:
```python
class SQLSynthesizerAgent:
    async def parse_intent(self, query: str) -> QueryIntent:
        """Extract semantic intent from natural language."""

    async def lookup_schemas(self, intent: QueryIntent) -> RelevantSchemas:
        """Find tables/views relevant to the query."""

    async def generate_sql(self, intent: QueryIntent, schemas: RelevantSchemas) -> SQLQuery:
        """Synthesize executable SQL from intent and schema."""

    async def explain_query(self, sql: SQLQuery) -> str:
        """Explain what the SQL query does in plain language."""

    async def interpret_results(self, sql: SQLQuery, results: DataFrame) -> str:
        """Generate insight summary from query results."""
```

**SQL Generation Patterns**:
```sql
-- Pattern: Time-series comparison
-- Intent: "compare X and Y over time"
WITH base AS (
    SELECT time_bucket('1 hour', timestamp) as hour, ...
    FROM {table}
    WHERE timestamp BETWEEN {start} AND {end}
)
SELECT hour, metric_a, metric_b, metric_a - metric_b as diff
FROM base
ORDER BY hour;

-- Pattern: Conditional aggregation
-- Intent: "average X when Y condition"
SELECT
    AVG(CASE WHEN {condition} THEN {metric} END) as metric_when_true,
    AVG(CASE WHEN NOT {condition} THEN {metric} END) as metric_when_false
FROM {table}
WHERE timestamp BETWEEN {start} AND {end};

-- Pattern: Anomaly detection
-- Intent: "find unusual readings in X"
WITH stats AS (
    SELECT AVG({metric}) as mean, STDDEV({metric}) as std
    FROM {table}
    WHERE timestamp BETWEEN {start} AND {end}
)
SELECT t.*, (t.{metric} - s.mean) / s.std as z_score
FROM {table} t, stats s
WHERE ABS((t.{metric} - s.mean) / s.std) > 3;
```

---

### 1.3 Visualization Agent (`ndp-viz-agent`)

**Purpose**: Auto-generate Grafana dashboards and visualization recommendations.

**Scope**: Specialized - Dashboard generation and visual analytics

**Capabilities**:
- Dashboard JSON generation
- Panel type recommendation
- Time-series visualization
- Threshold and alert visualization
- Dashboard provisioning

**Key Interactions**:
```
┌────────────────────────────────────────────────────────────────────────┐
│                      VISUALIZATION AGENT                                │
├────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  User: "Create a dashboard showing forecast accuracy by lead time"     │
│                                                                         │
│  ┌──────────────┐      ┌──────────────┐      ┌──────────────┐         │
│  │ Requirements │ ───► │ Panel        │ ───► │ Dashboard    │         │
│  │ Analysis     │      │ Design       │      │ Generation   │         │
│  └──────────────┘      └──────────────┘      └──────────────┘         │
│         │                     │                     │                  │
│         ▼                     ▼                     ▼                  │
│  Metrics needed:       Panel types:           Generated JSON:          │
│  - temp_error          - Heatmap (lead vs     {                        │
│  - lead_time_hours       error intensity)     "panels": [...],        │
│  - time                - Line chart (error     "templating": {...}    │
│                          trend over time)     }                        │
│                        - Stat panel (current                           │
│                          trustworthy horizon)                          │
│                                                                         │
└────────────────────────────────────────────────────────────────────────┘
```

**Core Functions**:
```python
class VisualizationAgent:
    async def analyze_requirements(self, request: str) -> VizRequirements:
        """Determine what visualizations are needed."""

    async def recommend_panels(self, requirements: VizRequirements) -> List[PanelSpec]:
        """Suggest optimal panel types for the data."""

    async def generate_dashboard(self, panels: List[PanelSpec]) -> GrafanaDashboard:
        """Generate complete Grafana dashboard JSON."""

    async def provision_dashboard(self, dashboard: GrafanaDashboard) -> str:
        """Deploy dashboard to Grafana, return URL."""
```

**Panel Type Selection Logic**:
```
┌─────────────────────────────────────────────────────────────────┐
│                   PANEL TYPE DECISION TREE                       │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  Data Type?                                                      │
│  ├── Single metric over time ──────────────► Time Series Panel  │
│  ├── Multiple metrics over time ──────────► Multi-series Line  │
│  ├── Categorical comparison ───────────────► Bar Chart          │
│  ├── Two dimensions + value ───────────────► Heatmap            │
│  ├── Single current value ─────────────────► Stat Panel         │
│  ├── Value with thresholds ────────────────► Gauge              │
│  ├── Geographic data ──────────────────────► Geomap             │
│  └── Tabular exploration ──────────────────► Table              │
│                                                                  │
│  Time Granularity?                                               │
│  ├── Sub-minute ───────────────────────────► Streaming/Live     │
│  ├── Minutes to hours ─────────────────────► Standard refresh   │
│  └── Days to months ───────────────────────► Materialized views │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

---

### 1.4 Data Quality Agent (`ndp-dq-agent`)

**Purpose**: Detect anomalies, validate data quality, suggest fixes.

**Scope**: Specialized - Data quality monitoring and remediation

**Capabilities**:
- Anomaly detection (statistical, rule-based)
- Data quality scoring
- Root cause analysis
- Fix recommendations
- DQ transparency reporting

**Key Interactions**:
```
┌────────────────────────────────────────────────────────────────────────┐
│                      DATA QUALITY AGENT                                 │
├────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  Trigger: Continuous monitoring / User request                         │
│                                                                         │
│  ┌──────────────┐      ┌──────────────┐      ┌──────────────┐         │
│  │ Quality      │ ───► │ Anomaly      │ ───► │ Root Cause   │         │
│  │ Profiling    │      │ Detection    │      │ Analysis     │         │
│  └──────────────┘      └──────────────┘      └──────────────┘         │
│         │                     │                     │                  │
│         ▼                     ▼                     ▼                  │
│  DQ Scores:            Detected Issues:       Likely Causes:           │
│  - Completeness: 98%   - PM2.5 spike at      - Sensor recalibration  │
│  - Validity: 96%         03:42 (z=4.2)         detected at 03:40     │
│  - Freshness: 100%     - CO2 gap 14:00-      - Network outage        │
│                          14:30                  (etcd logs confirm)   │
│                                                                         │
│  ┌──────────────┐      ┌──────────────┐                               │
│  │ Fix          │ ───► │ Transparency │                               │
│  │ Recommendations│    │ Report       │                               │
│  └──────────────┘      └──────────────┘                               │
│         │                     │                                        │
│         ▼                     ▼                                        │
│  Suggestions:          DQ Report:                                      │
│  - Interpolate gap     "Data quality score: 94%                       │
│  - Flag spike in       2 anomalies detected, 1 gap identified.        │
│    transparency table  Recommended actions: [...]"                     │
│                                                                         │
└────────────────────────────────────────────────────────────────────────┘
```

**Core Functions**:
```python
class DataQualityAgent:
    async def profile_quality(self, stream_id: str) -> DQProfile:
        """Generate comprehensive data quality profile."""

    async def detect_anomalies(self, stream_id: str, window: TimeRange) -> List[Anomaly]:
        """Identify statistical and rule-based anomalies."""

    async def analyze_root_cause(self, anomaly: Anomaly) -> RootCauseAnalysis:
        """Correlate anomaly with system events, logs, configs."""

    async def recommend_fixes(self, anomalies: List[Anomaly]) -> List[Recommendation]:
        """Suggest remediation actions."""

    async def generate_report(self, profile: DQProfile, anomalies: List[Anomaly]) -> DQReport:
        """Create transparency report for audit trail."""
```

**DQ Rule Categories**:
```yaml
dq_rules:
  # Range validation
  range_checks:
    - field: pm25
      min: 0
      max: 500
      action: nullify_and_flag

    - field: temperature_c
      min: -40
      max: 60
      action: flag_for_review

  # Freshness checks
  freshness_checks:
    - stream: air-quality
      expected_interval: 120  # seconds
      alert_threshold: 300    # seconds

  # Completeness checks
  completeness_checks:
    - stream: outdoor-weather
      required_fields: [temperature, humidity, pressure]

  # Statistical checks
  statistical_checks:
    - field: pm25
      method: z_score
      threshold: 3.0
      window: 24h
```

---

### 1.5 Schema Designer Agent (`ndp-schema-designer`)

**Purpose**: Help design Silver-to-Gold transformations and new schemas.

**Scope**: Specialized - Schema evolution and transformation design

**Capabilities**:
- Schema inference from data
- Transformation logic design
- Migration script generation
- Backward compatibility analysis
- Documentation generation

**Key Interactions**:
```
┌────────────────────────────────────────────────────────────────────────┐
│                      SCHEMA DESIGNER AGENT                              │
├────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  User: "Design a Gold layer table for window management decisions"     │
│                                                                         │
│  ┌──────────────┐      ┌──────────────┐      ┌──────────────┐         │
│  │ Requirements │ ───► │ Source       │ ───► │ Schema       │         │
│  │ Analysis     │      │ Analysis     │      │ Design       │         │
│  └──────────────┘      └──────────────┘      └──────────────┘         │
│         │                     │                     │                  │
│         ▼                     ▼                     ▼                  │
│  Use case needs:       Source tables:         Proposed schema:         │
│  - Real-time decision  - silver_indoor_air   gold.window_decisions:   │
│  - Window open/close   - silver_outdoor...   - time TIMESTAMPTZ       │
│  - Based on AQ + temp  - weather_forecasts   - recommendation TEXT    │
│                                               - indoor_pm25 FLOAT     │
│                                               - outdoor_pm25 FLOAT    │
│                                               - confidence FLOAT      │
│                                                                         │
│  ┌──────────────┐      ┌──────────────┐                               │
│  │ Transform    │ ───► │ Migration    │                               │
│  │ Logic        │      │ Script       │                               │
│  └──────────────┘      └──────────────┘                               │
│         │                     │                                        │
│         ▼                     ▼                                        │
│  SQL/dbt model:        Migration file:                                 │
│  "SELECT ... CASE      "-- V001__create_window_decisions.sql          │
│   WHEN outdoor <        CREATE TABLE gold.window_decisions ..."       │
│   indoor * 0.8 ..."                                                    │
│                                                                         │
└────────────────────────────────────────────────────────────────────────┘
```

**Core Functions**:
```python
class SchemaDesignerAgent:
    async def analyze_requirements(self, use_case: str) -> SchemaRequirements:
        """Understand what the schema needs to support."""

    async def analyze_sources(self, requirements: SchemaRequirements) -> SourceAnalysis:
        """Identify source tables and their relevant fields."""

    async def design_schema(self, requirements: SchemaRequirements, sources: SourceAnalysis) -> SchemaDesign:
        """Create optimal schema design."""

    async def generate_transform(self, design: SchemaDesign) -> TransformLogic:
        """Generate SQL/dbt transformation logic."""

    async def generate_migration(self, design: SchemaDesign) -> MigrationScript:
        """Create versioned migration script."""

    async def check_compatibility(self, design: SchemaDesign) -> CompatibilityReport:
        """Verify backward compatibility with existing consumers."""
```

---

## 2. Workflow Patterns

### 2.1 Question-Driven Exploration

The primary workflow where a user asks a question and agents collaborate to answer it.

```
┌────────────────────────────────────────────────────────────────────────┐
│              QUESTION-DRIVEN EXPLORATION WORKFLOW                       │
├────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  User Question                                                          │
│       │                                                                 │
│       ▼                                                                 │
│  ┌──────────────────────────────────────────────────────────────────┐  │
│  │ 1. ORCHESTRATOR (Claude/claude-flow)                              │  │
│  │    - Parse intent                                                 │  │
│  │    - Select agents                                                │  │
│  │    - Coordinate workflow                                          │  │
│  └──────────────────────────────┬───────────────────────────────────┘  │
│                                 │                                       │
│                   ┌─────────────┼─────────────┐                        │
│                   ▼             ▼             ▼                        │
│            ┌───────────┐ ┌───────────┐ ┌───────────┐                  │
│            │ Data      │ │ SQL       │ │ DQ        │                  │
│            │ Explorer  │ │Synthesizer│ │ Agent     │                  │
│            └─────┬─────┘ └─────┬─────┘ └─────┬─────┘                  │
│                  │             │             │                         │
│                  ▼             ▼             ▼                         │
│            Schema info   SQL query     DQ validation                   │
│                  │             │             │                         │
│                  └─────────────┴─────────────┘                         │
│                                │                                        │
│                                ▼                                        │
│                  ┌─────────────────────────┐                           │
│                  │ 2. EXECUTION LAYER      │                           │
│                  │    - Query DuckDB/TS    │                           │
│                  │    - Apply DQ checks    │                           │
│                  │    - Cache results      │                           │
│                  └────────────┬────────────┘                           │
│                               │                                         │
│                               ▼                                         │
│                  ┌─────────────────────────┐                           │
│                  │ 3. INTERPRETATION       │                           │
│                  │    - SQL Synthesizer:   │                           │
│                  │      interpret results  │                           │
│                  │    - Generate insight   │                           │
│                  └────────────┬────────────┘                           │
│                               │                                         │
│                               ▼                                         │
│                  ┌─────────────────────────┐                           │
│                  │ 4. OPTIONAL VIZ         │                           │
│                  │    - Viz Agent:         │                           │
│                  │      generate chart     │                           │
│                  └────────────┬────────────┘                           │
│                               │                                         │
│                               ▼                                         │
│                        User Response                                    │
│                        (Text + Chart)                                   │
│                                                                         │
└────────────────────────────────────────────────────────────────────────┘
```

### 2.2 Proactive Exploration

Agents suggest what to explore next based on discoveries.

```
┌────────────────────────────────────────────────────────────────────────┐
│                PROACTIVE EXPLORATION WORKFLOW                           │
├────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  Initial Analysis Complete                                              │
│       │                                                                 │
│       ▼                                                                 │
│  ┌──────────────────────────────────────────────────────────────────┐  │
│  │ PATTERN DETECTION (Data Explorer + DQ Agent)                      │  │
│  │                                                                   │  │
│  │  Findings:                                                        │  │
│  │  - PM2.5 correlates with outdoor temp (r=0.72)                   │  │
│  │  - CO2 spikes daily at 14:00-15:00                               │  │
│  │  - Gap in outdoor data on weekends                               │  │
│  └──────────────────────────────┬───────────────────────────────────┘  │
│                                 │                                       │
│                                 ▼                                       │
│  ┌──────────────────────────────────────────────────────────────────┐  │
│  │ SUGGESTION GENERATION (Orchestrator)                              │  │
│  │                                                                   │  │
│  │  "Based on my analysis, I noticed some interesting patterns:     │  │
│  │                                                                   │  │
│  │   1. PM2.5 strongly correlates with outdoor temperature.         │  │
│  │      Would you like me to explore if this is due to:             │  │
│  │      a) Window opening patterns?                                  │  │
│  │      b) HVAC cycles?                                              │  │
│  │      c) Outdoor pollution sources?                                │  │
│  │                                                                   │  │
│  │   2. There's a daily CO2 spike around 2-3 PM.                    │  │
│  │      This could indicate meeting room usage.                      │  │
│  │      Should I correlate with calendar data (if available)?        │  │
│  │                                                                   │  │
│  │   3. I noticed the outdoor API doesn't collect data on weekends. │  │
│  │      This seems like a configuration issue.                       │  │
│  │      Should I investigate the polling schedule?"                  │  │
│  │                                                                   │  │
│  └──────────────────────────────────────────────────────────────────┘  │
│                                                                         │
│  User selects: "Explore option 1a - window opening patterns"          │
│       │                                                                 │
│       ▼                                                                 │
│  [Return to Question-Driven Exploration with focused query]            │
│                                                                         │
└────────────────────────────────────────────────────────────────────────┘
```

### 2.3 Multi-Agent Collaboration

Complex analyses require multiple agents working together.

```
┌────────────────────────────────────────────────────────────────────────┐
│            MULTI-AGENT COLLABORATION PATTERN                            │
├────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  Request: "Build a forecast accuracy dashboard with data quality       │
│            monitoring and recommend schema improvements"                │
│                                                                         │
│  ┌──────────────────────────────────────────────────────────────────┐  │
│  │ PHASE 1: PARALLEL ANALYSIS                                        │  │
│  │                                                                   │  │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐           │  │
│  │  │ Data         │  │ DQ           │  │ Schema       │           │  │
│  │  │ Explorer     │  │ Agent        │  │ Designer     │           │  │
│  │  │              │  │              │  │              │           │  │
│  │  │ "Discover    │  │ "Assess      │  │ "Analyze     │           │  │
│  │  │  forecast    │  │  quality of  │  │  current     │           │  │
│  │  │  data        │  │  forecast    │  │  schema      │           │  │
│  │  │  structure"  │  │  data"       │  │  fitness"    │           │  │
│  │  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘           │  │
│  │         │                 │                 │                    │  │
│  └─────────┼─────────────────┼─────────────────┼────────────────────┘  │
│            │                 │                 │                       │
│            ▼                 ▼                 ▼                       │
│  ┌──────────────────────────────────────────────────────────────────┐  │
│  │ PHASE 2: SYNTHESIS (Shared Memory)                                │  │
│  │                                                                   │  │
│  │  Memory Store:                                                    │  │
│  │  ├── exploration_results: {schemas, samples, patterns}           │  │
│  │  ├── dq_assessment: {scores, anomalies, gaps}                    │  │
│  │  └── schema_analysis: {current, recommendations}                 │  │
│  │                                                                   │  │
│  └──────────────────────────────┬───────────────────────────────────┘  │
│                                 │                                       │
│                                 ▼                                       │
│  ┌──────────────────────────────────────────────────────────────────┐  │
│  │ PHASE 3: COORDINATED OUTPUT                                       │  │
│  │                                                                   │  │
│  │  ┌──────────────┐     ┌──────────────┐                           │  │
│  │  │ SQL          │────►│ Viz          │                           │  │
│  │  │ Synthesizer  │     │ Agent        │                           │  │
│  │  │              │     │              │                           │  │
│  │  │ "Generate    │     │ "Build       │                           │  │
│  │  │  accuracy    │     │  dashboard   │                           │  │
│  │  │  queries"    │     │  JSON"       │                           │  │
│  │  └──────────────┘     └──────────────┘                           │  │
│  │                                                                   │  │
│  │  ┌──────────────┐     ┌──────────────┐                           │  │
│  │  │ DQ           │────►│ Schema       │                           │  │
│  │  │ Agent        │     │ Designer     │                           │  │
│  │  │              │     │              │                           │  │
│  │  │ "Add DQ      │     │ "Generate    │                           │  │
│  │  │  panels"     │     │  migration"  │                           │  │
│  │  └──────────────┘     └──────────────┘                           │  │
│  │                                                                   │  │
│  └──────────────────────────────────────────────────────────────────┘  │
│                                                                         │
│  Output:                                                                │
│  - Grafana dashboard JSON (deployable)                                 │
│  - Schema migration script                                              │
│  - DQ monitoring configuration                                          │
│  - Analysis report                                                      │
│                                                                         │
└────────────────────────────────────────────────────────────────────────┘
```

---

## 3. Memory and Learning

### 3.1 Memory Architecture

Agents use the hybrid memory system defined in the AI Agent Memory Architecture.

```
┌────────────────────────────────────────────────────────────────────────┐
│                    AGENTIC DATA SCIENCE MEMORY                          │
├────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  L1: FAST CACHE (claude-flow)                                          │
│  ├── Session context (current conversation)                            │
│  ├── Recent query results (5 min TTL)                                  │
│  ├── Schema cache (1 hour TTL)                                         │
│  └── User preferences                                                   │
│                                                                         │
│  L2: SEMANTIC LEARNING (AgentDB)                                       │
│  ├── Exploration patterns (what questions lead to insights)            │
│  ├── Query templates (proven SQL patterns)                             │
│  ├── Domain knowledge (PM2.5 thresholds, weather correlations)        │
│  ├── Schema evolution history                                          │
│  └── Successful workflow trajectories                                   │
│                                                                         │
│  L3: HEAVY OPERATIONS (RuVector - if needed)                           │
│  ├── Embedding of exploration reports                                   │
│  ├── Clustering of similar questions                                    │
│  └── Semantic routing for query intent                                 │
│                                                                         │
└────────────────────────────────────────────────────────────────────────┘
```

### 3.2 Knowledge Accumulation

What agents learn over time:

```yaml
learned_patterns:
  # Domain knowledge
  domain:
    pm25_thresholds:
      good: [0, 9.0]
      moderate: [9.1, 35.4]
      unhealthy_sensitive: [35.5, 55.4]
      source: "EPA 2024 standards"

    weather_correlations:
      - "PM2.5 increases with temperature inversion"
      - "Indoor CO2 peaks during afternoon meetings"
      - "Humidity >80% correlates with sensor drift"

  # Query patterns
  sql_patterns:
    time_bucket_comparison: |
      WITH aligned AS (
        SELECT time_bucket('1 hour', timestamp) as hour, ...
      )
      SELECT hour, metric_a, metric_b
      FROM aligned

    conditional_aggregation: |
      SELECT
        AVG(CASE WHEN {condition} THEN {metric} END) as when_true,
        AVG(CASE WHEN NOT {condition} THEN {metric} END) as when_false
      FROM {table}

  # Exploration heuristics
  exploration:
    - "When PM2.5 spikes, check outdoor pollution API data"
    - "When CO2 elevated, check occupancy or HVAC status"
    - "When sensors disagree, check calibration dates"

  # Schema conventions
  schema:
    naming:
      - "Gold tables: gold.{domain}_{use_case}"
      - "Metrics: {metric}_{aggregation} (e.g., pm25_avg)"
      - "Time columns: {grain}_bucket (e.g., hour_bucket)"
```

### 3.3 Learning Workflows

```
┌────────────────────────────────────────────────────────────────────────┐
│                    LEARNING FEEDBACK LOOP                               │
├────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  1. EXPLORATION PHASE                                                   │
│     Agent performs analysis                                             │
│     └── Records trajectory in AgentDB                                  │
│                                                                         │
│  2. USER FEEDBACK                                                       │
│     User indicates usefulness                                           │
│     ├── Explicit: "This was helpful" / "Not what I needed"            │
│     └── Implicit: Follow-up questions, dashboard usage                 │
│                                                                         │
│  3. VERDICT JUDGMENT (ReasoningBank)                                   │
│     Evaluate exploration success                                        │
│     ├── Did user get actionable insight?                               │
│     ├── Was query efficient?                                            │
│     └── Was visualization appropriate?                                  │
│                                                                         │
│  4. PATTERN EXTRACTION                                                  │
│     Distill successful approaches                                       │
│     ├── Store proven SQL patterns                                       │
│     ├── Record effective visualizations                                 │
│     └── Note domain correlations discovered                            │
│                                                                         │
│  5. REINFORCEMENT                                                       │
│     Update agent decision-making                                        │
│     ├── Increase weight on successful patterns                         │
│     ├── Decrease weight on unhelpful approaches                        │
│     └── Update exploration heuristics                                  │
│                                                                         │
└────────────────────────────────────────────────────────────────────────┘
```

---

## 4. Edge Constraints

### 4.1 Container Isolation Challenge

The development container (where agents run) is isolated from the Pi production container (where data lives).

```
┌────────────────────────────────────────────────────────────────────────┐
│                    DEPLOYMENT TOPOLOGY                                  │
├────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  ┌──────────────────────────────┐   ┌──────────────────────────────┐  │
│  │      DEV CONTAINER           │   │      PI PRODUCTION           │  │
│  │      (Codespace/Local)       │   │      (Raspberry Pi 5)        │  │
│  │                              │   │                              │  │
│  │  ┌────────────────────────┐  │   │  ┌────────────────────────┐  │  │
│  │  │ Claude Code / Agents   │  │   │  │ air-quality-app        │  │  │
│  │  │ - Data Explorer        │  │   │  │ - Ingestion            │  │  │
│  │  │ - SQL Synthesizer      │  │   │  │ - Storage              │  │  │
│  │  │ - Viz Agent            │  │   │  └────────────────────────┘  │  │
│  │  │ - DQ Agent             │  │   │                              │  │
│  │  │ - Schema Designer      │  │   │  ┌────────────────────────┐  │  │
│  │  └────────────────────────┘  │   │  │ DuckDB HTTP API        │  │  │
│  │                              │   │  │ - Bronze Parquet       │  │  │
│  │  ┌────────────────────────┐  │   │  │ - Silver Views         │  │  │
│  │  │ AgentDB (Local)        │  │   │  └────────────────────────┘  │  │
│  │  │ - Pattern Memory       │  │   │                              │  │
│  │  └────────────────────────┘  │   │  ┌────────────────────────┐  │  │
│  │                              │   │  │ TimescaleDB            │  │  │
│  │                              │   │  │ - Data Dictionary      │  │  │
│  │                              │   │  │ - Silver Tables        │  │  │
│  │                              │   │  └────────────────────────┘  │  │
│  │                              │   │                              │  │
│  │                              │   │  ┌────────────────────────┐  │  │
│  │                              │   │  │ Grafana                │  │  │
│  │                              │   │  │ - Dashboards           │  │  │
│  │                              │   │  └────────────────────────┘  │  │
│  │                              │   │                              │  │
│  └──────────────────────────────┘   └──────────────────────────────┘  │
│                                                                         │
│                       NETWORK BOUNDARY                                  │
│                            │                                            │
│         ┌──────────────────┴──────────────────┐                        │
│         │                                      │                        │
│         ▼                                      ▼                        │
│    SSH TUNNEL                             HTTPS API                     │
│    (Port Forwarding)                      (DuckDB/Grafana)             │
│                                                                         │
└────────────────────────────────────────────────────────────────────────┘
```

### 4.2 Access Patterns

#### Option A: API-Based Access (Recommended)

```
┌────────────────────────────────────────────────────────────────────────┐
│                    API ACCESS PATTERN                                   │
├────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  Data Science Agents                                                    │
│       │                                                                 │
│       ▼                                                                 │
│  ┌──────────────────────────────────────────────────────────────────┐  │
│  │ NDP DATA CLIENT (Python/Rust SDK)                                 │  │
│  │                                                                   │  │
│  │  Endpoints:                                                       │  │
│  │  ├── DuckDB HTTP: http://pi:9090/query                           │  │
│  │  ├── Grafana API: http://pi:3000/api/                            │  │
│  │  ├── TimescaleDB: postgres://pi:5432/                            │  │
│  │  └── etcd (config): http://pi:2379/                              │  │
│  │                                                                   │  │
│  │  Features:                                                        │  │
│  │  ├── Connection pooling                                           │  │
│  │  ├── Retry with backoff                                           │  │
│  │  ├── Result caching                                               │  │
│  │  └── Query timeout management                                     │  │
│  │                                                                   │  │
│  └──────────────────────────────────────────────────────────────────┘  │
│                                                                         │
│  Benefits:                                                              │
│  + No filesystem access needed                                          │
│  + Works over any network (local, VPN, internet)                       │
│  + APIs already exist (DuckDB HTTP, Grafana API)                       │
│  + Easier authentication and access control                             │
│                                                                         │
│  Challenges:                                                            │
│  - Network latency adds to query time                                   │
│  - Need to expose ports securely                                        │
│  - Large result sets may be slow                                        │
│                                                                         │
└────────────────────────────────────────────────────────────────────────┘
```

#### Option B: SSH Tunnel + Direct Access

```
┌────────────────────────────────────────────────────────────────────────┐
│                    SSH TUNNEL PATTERN                                   │
├────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  Setup:                                                                 │
│  $ ssh -L 9090:localhost:9090 -L 5432:localhost:5432 pi@raspberrypi   │
│                                                                         │
│  Then agents connect as if local:                                       │
│  ├── DuckDB: localhost:9090                                            │
│  └── TimescaleDB: localhost:5432                                       │
│                                                                         │
│  Benefits:                                                              │
│  + Appears local to agents                                              │
│  + Encrypted transport (SSH)                                            │
│  + Lower latency than VPN                                               │
│                                                                         │
│  Challenges:                                                            │
│  - Requires SSH access setup                                            │
│  - Tunnel management complexity                                         │
│  - Single point of failure                                              │
│                                                                         │
└────────────────────────────────────────────────────────────────────────┘
```

#### Option C: Data Sync/Snapshot (For Offline Analysis)

```
┌────────────────────────────────────────────────────────────────────────┐
│                    DATA SYNC PATTERN                                    │
├────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  Nightly/Hourly Sync:                                                   │
│  Pi ──rsync/scp──► Dev Container                                       │
│                                                                         │
│  Synced Data:                                                           │
│  ├── /data/bronze/*.parquet (last 7 days)                              │
│  ├── Data dictionary export                                             │
│  └── Schema snapshots                                                   │
│                                                                         │
│  Benefits:                                                              │
│  + Full offline access                                                  │
│  + Fast local queries                                                   │
│  + Works without network                                                │
│                                                                         │
│  Challenges:                                                            │
│  - Data staleness                                                       │
│  - Storage requirements                                                 │
│  - Sync complexity                                                      │
│  - NOT suitable for real-time analysis                                  │
│                                                                         │
└────────────────────────────────────────────────────────────────────────┘
```

### 4.3 Recommended Hybrid Approach

```yaml
access_strategy:
  # Default: API-based for most operations
  default: api

  api_config:
    duckdb:
      endpoint: "${NDP_PI_HOST}:9090"
      timeout: 30s
      max_rows: 100000

    timescaledb:
      endpoint: "${NDP_PI_HOST}:5432"
      database: ndp
      user: readonly_user

    grafana:
      endpoint: "${NDP_PI_HOST}:3000"
      api_key: "${GRAFANA_API_KEY}"

  # Fallback: Local DuckDB with synced Parquet
  fallback: local_sync

  local_sync_config:
    sync_schedule: "0 */6 * * *"  # Every 6 hours
    sync_days: 7                   # Last 7 days
    parquet_path: "./data/sync/"

  # Caching layer
  cache:
    provider: redis  # or in-memory
    schema_ttl: 1h
    query_result_ttl: 5m
    max_cached_rows: 10000
```

### 4.4 Caching Strategy

```
┌────────────────────────────────────────────────────────────────────────┐
│                    CACHING FOR REMOTE EXPLORATION                       │
├────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  CACHE LAYERS:                                                          │
│                                                                         │
│  1. Schema Cache (AgentDB L1)                                          │
│     ├── Table/view definitions                                          │
│     ├── Column types and descriptions                                   │
│     ├── Data dictionary entries                                         │
│     └── TTL: 1 hour (schemas change infrequently)                      │
│                                                                         │
│  2. Query Result Cache (Redis/Memory)                                  │
│     ├── Hash of SQL query → result DataFrame                           │
│     ├── TTL: 5 minutes (balance freshness vs. performance)             │
│     ├── Max rows cached: 10,000                                         │
│     └── Eviction: LRU                                                   │
│                                                                         │
│  3. Aggregation Cache (Materialized in AgentDB)                        │
│     ├── Daily/weekly rollups                                            │
│     ├── Frequently-used aggregations                                    │
│     └── Updated on schedule                                             │
│                                                                         │
│  CACHE INVALIDATION:                                                    │
│                                                                         │
│  Schema changes:                                                        │
│  └── etcd watch triggers cache clear                                   │
│                                                                         │
│  Query results:                                                         │
│  └── TTL-based expiry (no active invalidation)                         │
│                                                                         │
│  Aggregations:                                                          │
│  └── Scheduled refresh (e.g., every 6 hours)                           │
│                                                                         │
└────────────────────────────────────────────────────────────────────────┘
```

---

## 5. Agent Interaction Patterns

### 5.1 Agent Communication Protocol

```yaml
# Message format between agents
agent_message:
  from: "ndp-data-explorer"
  to: "ndp-sql-synthesizer"
  type: "request"  # request | response | notification
  correlation_id: "abc123"
  payload:
    intent: "generate_sql"
    context:
      schema_info:
        - table: silver_indoor_air
          columns: [timestamp, pm25, co2, temperature]
      user_question: "Average PM2.5 by hour of day"
    metadata:
      priority: "normal"
      timeout_ms: 5000
```

### 5.2 Orchestration Pattern

```
┌────────────────────────────────────────────────────────────────────────┐
│                    AGENT ORCHESTRATION                                  │
├────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  ORCHESTRATOR (claude-flow or Claude Code)                             │
│       │                                                                 │
│       │ 1. Receive user request                                        │
│       │ 2. Classify request type                                       │
│       │ 3. Select agent(s)                                             │
│       │ 4. Dispatch tasks                                               │
│       │ 5. Aggregate responses                                          │
│       │ 6. Return to user                                               │
│       │                                                                 │
│       ▼                                                                 │
│  ┌──────────────────────────────────────────────────────────────────┐  │
│  │ Request Classification                                            │  │
│  │                                                                   │  │
│  │ "What's in my data?"          → Data Explorer (solo)             │  │
│  │ "Show me X over time"         → SQL Synth + Viz (sequence)       │  │
│  │ "Is my data quality OK?"      → DQ Agent (solo)                  │  │
│  │ "Build a dashboard for X"     → Explorer + SQL + Viz (parallel)  │  │
│  │ "Help me design a new table"  → Schema Designer (solo)           │  │
│  │ "Explore and recommend"       → All agents (full workflow)       │  │
│  │                                                                   │  │
│  └──────────────────────────────────────────────────────────────────┘  │
│                                                                         │
│  TASK DISPATCH:                                                         │
│                                                                         │
│  Solo tasks:                                                            │
│  └── Direct invocation, single agent handles entirely                  │
│                                                                         │
│  Sequential tasks:                                                      │
│  └── Chain: Agent A output → Agent B input                             │
│                                                                         │
│  Parallel tasks:                                                        │
│  └── Swarm: All agents work simultaneously, orchestrator aggregates   │
│                                                                         │
└────────────────────────────────────────────────────────────────────────┘
```

### 5.3 Error Handling

```yaml
error_handling:
  # Network errors (API unreachable)
  network_error:
    retry: 3
    backoff: exponential
    fallback: local_cache_or_fail

  # Query errors (SQL syntax, timeout)
  query_error:
    action: explain_and_suggest_fix
    example: |
      "The query timed out. This might be because the time range
       is too large. Would you like me to try with a smaller range
       (last 7 days instead of 30)?"

  # Data quality issues
  dq_issue:
    action: warn_and_proceed
    example: |
      "Note: 15% of rows in this time range have missing PM2.5 values.
       The following analysis excludes those rows."

  # Agent failure
  agent_failure:
    action: escalate_to_orchestrator
    fallback: simpler_agent_or_manual
```

---

## 6. Implementation Roadmap

### Phase 1: Foundation (Week 1-2)

```yaml
phase_1:
  goals:
    - Establish API client for Pi data access
    - Implement Data Explorer agent (basic)
    - Implement SQL Synthesizer agent (basic)

  deliverables:
    - NDP Data Client SDK
    - Data Explorer: schema discovery, basic profiling
    - SQL Synthesizer: simple query generation

  success_criteria:
    - Can connect to DuckDB HTTP API from dev container
    - Can execute natural language → SQL → results workflow
```

### Phase 2: Enhancement (Week 3-4)

```yaml
phase_2:
  goals:
    - Add Visualization agent
    - Add Data Quality agent
    - Implement basic memory (L1 cache)

  deliverables:
    - Viz Agent: Grafana dashboard generation
    - DQ Agent: anomaly detection, quality scoring
    - Redis/memory cache for query results

  success_criteria:
    - Can generate and deploy Grafana dashboards
    - Can detect and report data quality issues
```

### Phase 3: Intelligence (Week 5-6)

```yaml
phase_3:
  goals:
    - Add Schema Designer agent
    - Implement AgentDB integration (L2)
    - Enable proactive suggestions

  deliverables:
    - Schema Designer: transformation logic generation
    - Pattern memory (successful queries, domain knowledge)
    - Suggestion engine for next steps

  success_criteria:
    - Agents learn from successful interactions
    - Can suggest exploration directions proactively
```

### Phase 4: Production (Week 7-8)

```yaml
phase_4:
  goals:
    - Multi-agent coordination
    - Production hardening
    - Documentation and training

  deliverables:
    - Orchestration layer for complex workflows
    - Error handling and fallbacks
    - User documentation

  success_criteria:
    - Complex multi-agent workflows work reliably
    - System handles network/data issues gracefully
```

---

## 7. Related Documents

- `docs/architecture/PLATFORM_ARCHITECTURE_OVERVIEW.md` - NDP architecture
- `docs/architecture/ai-agent-memory-integration.md` - Memory system design
- `docs/architecture/CONSOLIDATED_ARCHITECTURE_DECISIONS.md` - ADR summary
- `.claude/agents/ndp/README.md` - Existing NDP agents
- `.claude/agents/ndp/ndp-analytics-engineer.md` - Related analytics patterns

---

## 8. Open Questions

1. **Authentication**: How do agents authenticate to Pi services from dev container?
2. **Rate Limiting**: Should we implement rate limiting for API calls to Pi?
3. **Audit Trail**: How do we log agent actions for compliance/debugging?
4. **User Preferences**: Where do we store user-specific preferences (time zones, units)?
5. **Multi-User**: Do we need to support multiple users with different access levels?

---

## Document History

| Version | Date | Changes |
|---------|------|---------|
| 1.0.0 | 2026-01-03 | Initial design document |

---

*End of Agentic Data Scientist Design Document*
