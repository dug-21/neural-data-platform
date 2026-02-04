# Gold Layer Feature Roadmap: V1.1 → V2.0

> **Created:** 2026-02-03
> **Updated:** 2026-02-03 (V1.2 reframed as Pattern Detection, V2.0 Validation Test added)
> **Method:** Working Backwards from V2.0 Vision
> **Status:** Draft for Review

---

## Executive Summary

This document defines the feature roadmap from V1.1 (Gold Layer Foundation) through V2.0 (Multi-Stream Intelligence) using **backwards design**. Each version's features are derived from the requirements of the subsequent version, ensuring every capability has a clear purpose in the journey toward autonomous edge intelligence.

### Critical Architectural Principle

**V1.1's primary deliverable is an extensible architecture, not a fixed set of features.**

Following V1.0's declarative philosophy:

| V1.0 Pattern | V1.1 Applies Same Pattern |
|--------------|---------------------------|
| JSON config drives Bronze → Silver ETL | JSON config drives Silver → Gold transformation |
| Add new stream by editing config | Add new Gold aggregates/features by editing config |
| JSON Schema validation ensures correctness | JSON Schema validation for Gold config |
| No code changes to add streams | No code changes to add features or streams to Gold |

**The test of V1.1 success**: Can we add a new stream (e.g., `outdoor-air-quality`) to the Gold layer by *only editing JSON config*? If yes, the architecture works.

### The Core Insight

The platform's value emerges from a capability chain:

```
V1.0          V1.1           V1.2            V1.3              V2.0
─────         ────           ────            ────              ────
Ingest   →    Prepare    →   Detect      →   Predict/Act   →   New Domain
Data          for Detection  Candidates      on Candidates     via Config

Bronze→Silver Gold Layer     Pattern         Causal Models     Multi-Stream
Pipeline      Foundation     Detection       & Actions         Intelligence
```

**Each version exists to enable the next.** V1.1 is not valuable in isolation—it's valuable because it makes V1.2 possible. This document traces those dependencies rigorously.

---

## Foundational Concept: Streams and Domains

### The Mental Model

**Streams are domain-agnostic building blocks. Domains are declared at the Gold layer.**

This is a critical architectural insight that affects how we think about the entire platform:

```
BRONZE LAYER: Domain-Agnostic Ingestion
┌──────────────────────────────────────────────────────────────────────────────┐
│ air-quality │ weather │ state-events │ energy-prices │ solar-output │ ...   │
│                                                                              │
│ Streams are just data. No domain assignment. Any source can be added.       │
└──────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
SILVER LAYER: Domain-Agnostic Quality
┌──────────────────────────────────────────────────────────────────────────────┐
│ All streams: cleansed, validated, time-aligned, quality-scored              │
│                                                                              │
│ Still no domain assignment. Streams are reusable building blocks.           │
└──────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
GOLD LAYER: Domains Are DECLARED Here
┌──────────────────────────────────────────────────────────────────────────────┐
│                                                                              │
│  Domain: "Indoor Air Quality"         Domain: "Energy Management"           │
│  ┌──────────────────────────┐         ┌──────────────────────────┐         │
│  │ STREAMS:                 │         │ STREAMS:                 │         │
│  │   • air-quality          │         │   • energy-usage         │         │
│  │   • weather ─────────────┼─────────┼── weather (SHARED!)      │         │
│  │   • state-events         │         │   • solar-output         │         │
│  │                          │         │   • energy-prices        │         │
│  │ OBJECTIVES:              │         │                          │         │
│  │   • CO2 < 800 ppm        │         │ OBJECTIVES:              │         │
│  │   • PM2.5 < 12 µg/m³     │         │   • daily_cost < $5      │         │
│  └──────────────────────────┘         └──────────────────────────┘         │
│                                                                              │
│  DISCOVERY: "Weather correlates with BOTH domains in different ways"        │
│                                                                              │
└──────────────────────────────────────────────────────────────────────────────┘
```

### Why This Matters

| Principle | Implication |
|-----------|-------------|
| **Streams are reusable** | Weather data serves air quality, energy, agriculture—no duplication |
| **Domains are configuration** | Add a domain by declaring objectives + selecting streams in JSON |
| **No domain silos** | The correlation engine sees ALL streams, flags candidate relationships |
| **Cross-domain = unexpected correlation** | "The weather stream you use for comfort also predicts energy prices" |

### What "Cross-Stream Pattern Detection" Really Means

It's NOT about connecting separate systems. It's about identifying that:
- A stream you added for Domain A also correlates with Domain B
- Two streams you never expected to relate actually do
- The same underlying pattern (weather, time-of-day, etc.) affects multiple objectives

**Example**: You're tracking weather for indoor air quality decisions. The platform identifies that the same weather patterns correlate with your solar panel output AND your heating costs. You didn't configure this search—the system tested all stream pairs because streams aren't siloed.

### Scalability Beyond Edge

While the current focus is a $75 edge device, this architecture—**fully configuration-driven, domain-agnostic data lake**—scales:

| Deployment | Hardware | Use Case |
|------------|----------|----------|
| **Edge** (current focus) | Raspberry Pi 5 | Single home/facility, local intelligence |
| **Department** | Small server | Building portfolio, fleet of facilities |
| **Enterprise** | Cloud/on-prem cluster | Organization-wide correlation discovery |

The same declarative configuration drives all scales. Add streams, declare domains, discover correlations—whether on a Pi or a data center.

---

## Version Dependency Chain

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                          V2.0: MULTI-STREAM INTELLIGENCE                     │
│                                                                              │
│  "New domain via config → infrastructure → predictions (no code)"           │
│                                                                              │
│  REQUIRES FROM V1.3:                                                         │
│  • Stream-agnostic prediction system                                         │
│  • Generic model selection (not hardcoded for specific streams)             │
│  • Abstractable action framework                                             │
│  • Multi-stream objective support                                            │
└─────────────────────────────────────────────────────────────────────────────┘
                                      │
                                      ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                        V1.3: PREDICTION & ACTIONS                            │
│                                                                              │
│  "Predictions trigger correct actions >80% of time"                         │
│                                                                              │
│  REQUIRES FROM V1.2:                                                         │
│  • Candidate correlations with metadata (lag, strength, direction)          │
│  • Historical evidence that correlations held over time                     │
│  • Candidate ranking (which correlations to validate causally)              │
│  • Transition history for natural experiment analysis                       │
└─────────────────────────────────────────────────────────────────────────────┘
                                      │
                                      ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                       V1.2: PATTERN DETECTION ENGINE                         │
│                                                                              │
│  "System identifies 'window→CO2' candidates without being told"             │
│                                                                              │
│  REQUIRES FROM V1.1:                                                         │
│  • Classified streams (state vs continuous vs forecast)                     │
│  • Time-aligned data across all streams                                     │
│  • Consistent feature granularity (hourly buckets)                          │
│  • Objectives declaring which outcomes matter                               │
│  • State transition events (when did window open?)                          │
│  • Sufficient historical data for statistical significance                  │
└─────────────────────────────────────────────────────────────────────────────┘
                                      │
                                      ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                       V1.1: GOLD LAYER FOUNDATION                            │
│                                                                              │
│  "System computes ML-ready features, classifies streams, accepts targets"   │
│                                                                              │
│  REQUIRES FROM V1.0:                                                         │
│  • Working Bronze → Silver pipeline                                          │
│  • Multiple streams ingesting (air quality, weather, state events)          │
│  • TimescaleDB operational with hypertables                                 │
│  • Declarative stream configuration                                          │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## V2.0: Multi-Stream Intelligence

### Vision Statement

> The same $75 device that optimizes your indoor air quality can also track energy costs, solar output, and economic indicators. The platform identifies that the weather stream you added for comfort decisions also correlates with your heating costs—without you configuring that search. Streams are building blocks; intelligence emerges from their combination.

### Mental Model Reminder

**Streams are domain-agnostic. Domains are declared at Gold via objectives.**

V2.0 doesn't "add domains"—it adds **new stream sources** (financial feeds, energy data) that users can combine with existing streams to form new domains or extend existing ones.

### What V2.0 Delivers

| Capability | Description | User Experience |
|------------|-------------|-----------------|
| **Financial Stream Sources** | FRED, Alpaca, Finnhub integration | Add economic/market streams via config |
| **Cross-Stream Pattern Detection** | Identify candidate relationships across all streams systematically | "Weather patterns you track for air quality also correlate with energy prices" |
| **Multi-Objective Domains** | Declare domains with objectives spanning many streams | Single domain uses air quality + weather + energy streams |
| **Stream-Agnostic Learning** | Same algorithms work on any stream type | One correlation engine for all data |

### V2.0 Features (Derived from Vision)

| ID | Feature | Description | Depends On |
|----|---------|-------------|------------|
| **v2-001** | Financial Stream Sources | Source traits for FRED, Alpaca, Finnhub APIs | V1.0 source trait pattern |
| **v2-002** | Stream Source Registry | Runtime registration of new stream sources | V1.0 stream config pattern |
| **v2-003** | Full Correlation Scanner | Granger causality across ALL streams (no domain boundaries) | V1.2 correlation engine |
| **v2-004** | Multi-Stream Objectives | Objectives referencing any combination of streams | V1.1 objectives framework |
| **v2-005** | Seeded Financial Models | HMM regime detection, indicator composites | V1.3 model zoo |
| **v2-006** | Stream-Specific Feature Templates | Pre-built features per stream type (AQI calc, Sharpe ratio) | V1.1 feature registry |
| **v2-007** | Unified Dashboard | Single view across all streams and declared domains | V1.1 Grafana foundation |

### V2.0 Validation Test

**This is the definitive test of the platform architecture.**

A new domain (e.g., "Energy Efficiency") is declared via:
1. Adding stream configs for energy-related sources (JSON)
2. Declaring objectives with targets (JSON)
3. Configuring Gold layer features and alignment (JSON)

**Without code changes**, the system:
- Materializes the necessary Gold infrastructure (aggregates, features, alignment views)
- Runs pattern detection against the new objectives
- Surfaces candidate relationships for validation
- Enables predictions and trigger configuration

**Success Criteria**:
| Metric | Target |
|--------|--------|
| Code changes required | Zero |
| Config-to-infrastructure time | < 15 minutes (refresh cycle) |
| Time to first candidate | < 7 days after data accumulation threshold met |
| New stream addition | Config edit only |
| New objective addition | Config edit only |

**Why This Matters**: If this test passes, we've built a **declarative intelligence platform**—not a hardcoded air quality application. The architecture scales to any domain that can be expressed as streams + objectives.

---

### What V2.0 Requires from V1.3

For multi-stream intelligence to work, V1.3 must have proven that:

1. **Prediction is stream-agnostic**: The model selection system works on generic time-series relationships, not hardcoded for specific streams
2. **Actions are abstractable**: The action framework can trigger alerts, webhooks, device control—anything a declared domain might need
3. **Learning generalizes**: EWC++ or similar prevents catastrophic forgetting when learning new stream relationships
4. **Objectives drive everything**: The system optimizes toward declared targets using whatever streams are relevant

---

## V1.3: Prediction & Actions

### Vision Statement

> The system predicts "CO2 will exceed 900 in 45 minutes" and suggests "open window now"—with >80% accuracy. When you accept, it learns. Over time, it acts autonomously within safety limits.

### What V1.3 Delivers

| Capability | Description | User Experience |
|------------|-------------|-----------------|
| **Causal Validation** | Confirms correlation → causation via PC algorithm | "Confirmed: window open *causes* CO2 drop (not just correlation)" |
| **Predictive Models** | TCN/ARIMA/Prophet trained on validated relationships | "CO2 forecast: 847 ppm in 30 minutes" |
| **Model Tournament** | Automatic selection of best model per relationship | System picks TCN for CO2, ARIMA for temperature |
| **Action Recommendations** | Suggests actions to achieve objectives | "Open window to maintain CO2 < 800" |
| **Outcome Tracking** | Measures if action achieved objective | "Window opened at 3pm → CO2 dropped 15% ✓" |
| **Graduated Autonomy** | Alert → Suggest → Auto-execute progression | User controls automation level per action |

### V1.3 Features (Derived from V2.0 Requirements)

| ID | Feature | Description | Depends On | Enables |
|----|---------|-------------|------------|---------|
| **v13-001** | Causal Validation Engine | PC algorithm + natural experiment detection | V1.2 correlation candidates | V2.0 multi-stream validation |
| **v13-002** | Model Zoo | TCN-Lite, ARIMA, Prophet, MLP implementations | V1.2 relationship metadata | V2.0 seeded models |
| **v13-003** | Tournament Selection | Compare models on holdout data, select winner | v13-002 model zoo | V2.0 domain-agnostic selection |
| **v13-004** | Prediction Service | Real-time forecasts for target metrics | v13-003 selected models | V2.0 multi-domain predictions |
| **v13-005** | Action Framework | Define actions with preconditions, effects, safety limits | V1.1 objectives | V2.0 domain actions |
| **v13-006** | Action Scoring | Rank actions by predicted objective impact | v13-004, v13-005 | V2.0 multi-stream actions |
| **v13-007** | Outcome Tracker | Record action → result pairs | v13-005 actions | v13-008 feedback |
| **v13-008** | Feedback Learning | Update models based on action outcomes (EWC++) | v13-007 outcomes | V2.0 continuous learning |
| **v13-009** | Autonomy Controller | Per-action automation level (alert/suggest/auto) | v13-005 actions | V2.0 safety controls |
| **v13-010** | Prediction Dashboard | Forecasts + confidence intervals + action suggestions | v13-004, v13-006 | V2.0 unified dashboard |

### What V1.3 Requires from V1.2

For prediction/actions to work, V1.2 must provide:

1. **Candidate Relationships**: Correlation candidates with:
   - Source stream (e.g., `state_events.window`)
   - Target stream (e.g., `air_quality.co2`)
   - Optimal lag (e.g., 17 minutes)
   - Correlation strength (e.g., -0.73)
   - Direction (negative = inverse relationship)
   - Sample size (statistical confidence)

2. **Historical Evidence**: Proof the correlation held over time:
   - N events where relationship was observed
   - Consistency score (did it hold every time?)
   - Seasonal variations (stronger in summer?)

3. **Transition History**: For causal analysis:
   - All state change events with timestamps
   - Context at time of change (was it user-initiated or system-suggested?)
   - Enables natural experiment identification

4. **Candidate Ranking**: Prioritized list of correlations to validate:
   - Ranked by relevance to objectives
   - Ranked by statistical strength
   - Filtered by minimum evidence threshold

---

## V1.2: Pattern Detection Engine

### Vision Statement

> The system identifies candidate relationships between events and metrics—like "window open → CO2 drop at 17-minute lag"—and surfaces them for validation. You didn't configure this search; the system scans all stream pairs systematically, testing relationships you didn't think to specify.

### What V1.2 Delivers

| Capability | Description | User Experience |
|------------|-------------|-----------------|
| **Correlation Scanning** | Granger causality on all stream pairs | Runs nightly, flags potential relationships |
| **Transition Detection** | Identifies state change moments | "Window opened at 3:47 PM" |
| **Response Measurement** | Measures effect on continuous streams | "CO2 dropped 12% in following 30 minutes" |
| **Lag Detection** | Finds optimal delay for each relationship | "Best correlation at 17-minute lag" |
| **Candidate Promotion** | Strong correlations become candidates for validation | "CANDIDATE: Window → CO2 relationship flagged" |
| **Pattern Candidates Dashboard** | Visualize flagged relationships | Graph showing candidate correlations |

### V1.2 Features (Derived from V1.3 Requirements)

| ID | Feature | Description | Depends On | Enables |
|----|---------|-------------|------------|---------|
| **v12-001** | Transition Event Materializer | Create explicit transition events from state streams | V1.1 state classification | v12-003 response detection |
| **v12-002** | Granger Causality Scanner | Pairwise Granger causality test on aligned streams | V1.1 aligned data | v12-005 correlation candidates |
| **v12-003** | Response Window Analyzer | Measure target stream response after source event | v12-001 transitions | v12-005 lag discovery |
| **v12-004** | Lag Optimizer | Find optimal lag (0-60 min) for each relationship | v12-003 responses | V1.3 model training |
| **v12-005** | Correlation Aggregator | Combine evidence across multiple events | v12-002, v12-003 | v12-006 ranking |
| **v12-006** | Candidate Ranker | Rank correlations by strength × relevance | v12-005, V1.1 objectives | V1.3 causal validation |
| **v12-007** | Candidate Promoter | Promote candidates exceeding threshold | v12-006 ranked | V1.3 model training |
| **v12-008** | Candidate Registry | Store candidate relationships with metadata | v12-007 promoted | V1.3 model zoo |
| **v12-009** | Pattern Candidates Dashboard | Visualize scanning progress and candidates | v12-008 registry | User feedback |
| **v12-010** | Correlation Strength Tracker | Monitor relationship stability over time | v12-008 registry | V1.3 confidence |

### What V1.2 Requires from V1.1

For pattern detection to work, V1.1 must provide:

1. **Stream Classification**: The scanner must know:
   - Which streams are **state** (potential causes): binary events like window open/close
   - Which streams are **continuous** (potential effects): numeric readings like CO2, PM2.5
   - Which streams are **forecasts** (context, not causes): weather predictions

2. **Time-Aligned Data**: Correlation requires:
   - All streams bucketed to same granularity (hourly)
   - Joined view with all streams in single query
   - NULL handling for sparse state events

3. **State Transition Events**: For response detection:
   - Explicit events: "window opened at 3:47 PM"
   - Not just current state, but *when it changed*
   - Derived from raw state stream

4. **Objectives**: To filter relevant correlations:
   - User declared targets (CO2 < 800)
   - Only correlations affecting targets are promoted
   - Prevents noise from irrelevant relationships

5. **Sufficient History**: For statistical significance:
   - At least 30 days of aligned data recommended
   - Multiple instances of each state transition
   - Seasonal coverage for robust correlations

---

## V1.1: Gold Layer Foundation

### Vision Statement

> The system prepares all data for pattern detection: streams are classified, timestamps aligned, features computed at consistent granularity, and user objectives are declared. This is the launchpad for intelligence.

### What V1.1 Delivers

| Capability | Description | User Experience |
|------------|-------------|-----------------|
| **Declarative Gold Architecture** | Config-driven Gold layer generation | Add streams/features by editing JSON |
| **Stream Classification** | Metadata distinguishing stream types | Config includes `"stream_type": "state_event"` |
| **Continuous Aggregates** | Hourly pre-computed statistics per stream | Fast dashboard queries |
| **Cross-Stream Alignment** | Single view joining all streams | Query all data in one place |
| **State Transition Tracking** | Explicit transition events derived | "Window opened" events in Gold |
| **Objectives Schema** | Declarative target specification | User declares `"targets": [{"metric": "co2", "threshold": 800}]` |
| **Feature Computation** | Rolling statistics (mean, std, trend) | ML-ready features available |

---

### V1.1 Architectural Focus: Declarative Gold Layer

**The primary goal of V1.1 is to create an extensible architecture for Gold layer generation.** The specific features we implement are secondary to ensuring the architecture supports easy addition of:

1. New streams to Gold aggregates
2. New feature computations
3. New objectives
4. New cross-stream alignments

#### Design Principle: Config-Driven Everything

Following V1.0's proven pattern, Gold layer capabilities should be **declared in JSON config** and **interpreted by the runtime**:

```json
{
  "gold_etl": {
    "enabled": true,
    "description": "Gold layer transformation for air-quality stream",

    "aggregates": {
      "granularities": ["1 hour", "1 day"],
      "default_metrics": ["mean", "std", "min", "max", "count"],
      "fields": {
        "pm25": { "metrics": ["mean", "std", "min", "max", "p95"] },
        "co2": { "metrics": ["mean", "std", "min", "max"] },
        "temperature_c": { "metrics": ["mean", "min", "max"] }
      }
    },

    "features": {
      "lag": {
        "enabled": true,
        "lags_hours": [1, 6, 24],
        "fields": ["pm25", "co2"]
      },
      "rolling": {
        "enabled": true,
        "windows": ["4 hours", "24 hours"],
        "stats": ["mean", "std"],
        "fields": ["pm25"]
      },
      "trend": {
        "enabled": true,
        "window": "4 hours",
        "fields": ["pm25", "co2"]
      }
    },

    "transitions": {
      "enabled": true,
      "description": "For state_event streams only - derive explicit transition events"
    }
  }
}
```

#### Design Principle: Cross-Stream Alignment as Config

```json
{
  "alignment": {
    "enabled": true,
    "view_name": "aligned_hourly",
    "granularity": "1 hour",
    "streams": [
      { "stream_id": "air-quality", "alias": "indoor" },
      { "stream_id": "outdoor-weather", "alias": "outdoor" },
      { "stream_id": "home-assistant-state", "alias": "state" }
    ],
    "join_strategy": "full_outer",
    "null_handling": "preserve"
  }
}
```

**Extensibility test**: To add `outdoor-air-quality` to alignment, edit config:
```json
"streams": [
  ...existing...,
  { "stream_id": "outdoor-air-quality", "alias": "outdoor_aqi" }
]
```

No code changes required.

#### Design Principle: Objectives as First-Class Config

```json
{
  "objectives": [
    {
      "id": "indoor_air_quality",
      "description": "Maintain healthy indoor air",
      "targets": [
        {
          "stream": "air-quality",
          "metric": "co2",
          "condition": "<",
          "threshold": 800,
          "unit": "ppm",
          "priority": "high"
        },
        {
          "stream": "air-quality",
          "metric": "pm25",
          "condition": "<",
          "threshold": 12,
          "unit": "µg/m³",
          "priority": "high"
        }
      ],
      "constraints": [
        {
          "description": "Don't open window if outdoor air is bad",
          "stream": "outdoor-air-quality",
          "metric": "pm25",
          "condition": "<",
          "threshold": 35
        }
      ]
    }
  ]
}
```

#### Architecture Components

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                     V1.1 DECLARATIVE GOLD ARCHITECTURE                       │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  CONFIG LAYER (JSON + JSON Schema Validation)                               │
│  ─────────────────────────────────────────────                               │
│  ┌────────────────┐  ┌────────────────┐  ┌────────────────┐                │
│  │ stream configs │  │ alignment.json │  │ objectives.json│                │
│  │ + gold_etl     │  │                │  │                │                │
│  └───────┬────────┘  └───────┬────────┘  └───────┬────────┘                │
│          │                   │                   │                          │
│          └───────────────────┼───────────────────┘                          │
│                              │                                              │
│                              ▼                                              │
│  INTERPRETER LAYER (Rust - reads config, generates SQL)                     │
│  ──────────────────────────────────────────────────────                      │
│  ┌────────────────────────────────────────────────────────────────────────┐│
│  │ GoldETLInterpreter                                                      ││
│  │ • Reads stream config + gold_etl section                               ││
│  │ • Generates CREATE MATERIALIZED VIEW statements                        ││
│  │ • Generates ADD_CONTINUOUS_AGGREGATE_POLICY statements                 ││
│  │ • Applies via tokio-postgres                                           ││
│  └────────────────────────────────────────────────────────────────────────┘│
│                              │                                              │
│                              ▼                                              │
│  TIMESCALEDB LAYER (Generated from config)                                  │
│  ─────────────────────────────────────────                                   │
│  ┌────────────────┐  ┌────────────────┐  ┌────────────────┐                │
│  │ gold.stream_   │  │ gold.aligned_  │  │ gold.state_    │                │
│  │ _hourly views  │  │ _hourly view   │  │ transitions    │                │
│  └────────────────┘  └────────────────┘  └────────────────┘                │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

### Unified Event Abstraction

**Critical Insight**: V1.2's correlation engine needs to detect relationships between **events** and **outcomes**. But events can come from multiple sources:

| Event Source | Example | How Detected |
|--------------|---------|--------------|
| State Transitions | Window opened | State field changes from `off` to `on` |
| Threshold Crossings | CO2 exceeded 800 ppm | Observation crosses objective threshold |
| Anomalies | Sudden CO2 spike (3σ) | Statistical deviation from recent pattern |
| Trend Changes | Temperature rising | Trend direction reversal |

**V1.1 Scope**: State Transitions + Threshold Crossings
**Future (V1.2/V1.3)**: Anomalies + Trend Changes

#### The Unified Event Model

All events share a common structure:

```
EVENT = {
    event_id,            // Unique identifier
    event_time,          // When the event occurred (exact timestamp)
    stream_id,           // Source stream
    entity_id,           // Which entity (ndp_id)
    event_type,          // "state_transition" | "threshold_crossing" | "anomaly" | "trend_change"
    details: {           // Type-specific payload
        // For state_transition:
        from_state, to_state, duration_in_previous

        // For threshold_crossing:
        metric, threshold, direction, value, objective_id

        // For anomaly (future):
        metric, z_score, expected_value, actual_value

        // For trend_change (future):
        metric, from_direction, to_direction, slope_change
    }
}
```

#### Why Unified Events Matter

**V1.2 Correlation Engine doesn't care about event type**. It asks:
> "When EVENT X occurs, does METRIC Y change?"

The event could be a window opening (state transition) OR CO2 crossing 800 ppm (threshold). Both are potential causes that V1.2 will analyze for correlation with other metrics.

**Example correlations V1.2 might discover:**
1. "When window opens (state), CO2 drops 17 min later" (state→metric)
2. "When CO2 crosses 800 (threshold), user opens window 5 min later" (threshold→state)
3. "When outdoor PM2.5 exceeds 35 (threshold), indoor PM2.5 rises 2 hours later" (threshold→metric)

#### V1.1 Event Types (Build Now)

##### 1. State Transition Events

**Source**: `state_event` streams (e.g., home-assistant-state)
**Detection**: State field value changes
**Already documented in**: v11-006 (Generic Transition Materializer)

```sql
-- From v11-006: Transition detection via LAG()
CASE
    WHEN LAG(state) OVER w IS DISTINCT FROM state THEN TRUE
    ELSE FALSE
END AS is_actual_transition
```

##### 2. Threshold Crossing Events

**Source**: Objectives config + observation streams
**Detection**: Metric value crosses declared threshold
**New feature**: v11-012 (Threshold Crossing Generator)

**Key Insight**: Objectives already define thresholds. Those same thresholds define meaningful events.

```json
{
  "objectives": [{
    "targets": [{
      "metric": "co2",
      "condition": "<",
      "threshold": 800
    }]
  }]
}
```

This objective implies an event: "CO2 crossed 800 ppm (rising)" and "CO2 crossed 800 ppm (falling)".

**Generated SQL for threshold crossings:**

```sql
-- Threshold crossing detection using LAG
CREATE VIEW gold.threshold_crossings AS
SELECT
    observation_time AS event_time,
    stream_id,
    ndp_id AS entity_id,
    'threshold_crossing' AS event_type,
    metric_name,
    threshold_value,
    CASE
        WHEN value >= threshold_value AND LAG(value) OVER w < threshold_value THEN 'rising'
        WHEN value < threshold_value AND LAG(value) OVER w >= threshold_value THEN 'falling'
    END AS crossing_direction,
    value AS current_value,
    LAG(value) OVER w AS previous_value,
    objective_id
FROM (
    -- Join observations with objectives thresholds
    SELECT
        o.observation_time,
        o.stream_id,
        o.ndp_id,
        t.metric AS metric_name,
        t.threshold AS threshold_value,
        CASE t.metric
            WHEN 'co2' THEN o.co2
            WHEN 'pm25' THEN o.pm25
            -- ... other metrics
        END AS value,
        obj.id AS objective_id
    FROM silver.air_quality_observations o
    CROSS JOIN LATERAL (
        SELECT * FROM unnest(objectives.targets)
    ) t
    JOIN objectives obj ON ...
) subq
WINDOW w AS (PARTITION BY stream_id, ndp_id, metric_name ORDER BY observation_time)
WHERE (value >= threshold_value AND LAG(value) OVER w < threshold_value)
   OR (value < threshold_value AND LAG(value) OVER w >= threshold_value);
```

#### V1.2+ Event Types (Architect Now, Build Later)

##### 3. Anomaly Events (V1.2 Scope)

**Source**: Observation streams with learned baseline
**Detection**: Z-score exceeds threshold (e.g., |z| > 3)
**Why deferred**: Requires baseline learning period

```sql
-- Conceptual (V1.2)
CREATE VIEW gold.anomaly_events AS
SELECT
    observation_time AS event_time,
    'anomaly' AS event_type,
    metric,
    (value - rolling_mean) / rolling_std AS z_score,
    rolling_mean AS expected_value,
    value AS actual_value
FROM ...
WHERE ABS((value - rolling_mean) / rolling_std) > 3;
```

##### 4. Trend Change Events (V1.3 Scope)

**Source**: Computed trend features
**Detection**: Trend direction reversal
**Why deferred**: Requires trend computation from V1.1

```sql
-- Conceptual (V1.3)
CREATE VIEW gold.trend_change_events AS
SELECT
    bucket AS event_time,
    'trend_change' AS event_type,
    metric,
    LAG(trend_direction) OVER w AS from_direction,
    trend_direction AS to_direction
FROM gold.features_hourly
WHERE trend_direction != LAG(trend_direction) OVER w;
```

#### Unified Events View (Gold Layer)

V1.1 creates a unified view combining implemented event types:

```sql
-- V1.1: State transitions + threshold crossings
CREATE VIEW gold.events_unified AS

-- State transitions
SELECT
    transition_time AS event_time,
    stream_id,
    entity_id,
    'state_transition' AS event_type,
    jsonb_build_object(
        'from_state', from_state,
        'to_state', to_state,
        'duration_in_previous', duration_in_previous_state
    ) AS details
FROM gold.state_transitions
WHERE is_actual_transition = TRUE

UNION ALL

-- Threshold crossings
SELECT
    event_time,
    stream_id,
    entity_id,
    'threshold_crossing' AS event_type,
    jsonb_build_object(
        'metric', metric_name,
        'threshold', threshold_value,
        'direction', crossing_direction,
        'value', current_value,
        'objective_id', objective_id
    ) AS details
FROM gold.threshold_crossings;

-- V1.2 adds: UNION ALL SELECT ... FROM gold.anomaly_events
-- V1.3 adds: UNION ALL SELECT ... FROM gold.trend_change_events
```

#### Event Aggregation for Correlation (Hourly)

V1.2 correlation screening needs hourly event counts:

```sql
-- Added to gold.aligned_hourly
SELECT
    time_bucket('1 hour', event_time) AS bucket,

    -- State transition counts
    COUNT(*) FILTER (WHERE event_type = 'state_transition') AS transition_count,

    -- Threshold crossing counts
    COUNT(*) FILTER (WHERE event_type = 'threshold_crossing'
                       AND details->>'direction' = 'rising') AS threshold_rising_count,
    COUNT(*) FILTER (WHERE event_type = 'threshold_crossing'
                       AND details->>'direction' = 'falling') AS threshold_falling_count,

    -- By metric (for targeted correlation)
    COUNT(*) FILTER (WHERE event_type = 'threshold_crossing'
                       AND details->>'metric' = 'co2') AS co2_threshold_events

FROM gold.events_unified
GROUP BY bucket;
```

#### Architecture Impact

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                     V1.1 UNIFIED EVENT ARCHITECTURE                          │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  EVENT SOURCES                                                               │
│  ─────────────                                                               │
│  ┌────────────────┐  ┌────────────────┐  ┌────────────────┐                │
│  │ state_event    │  │ observation    │  │ objectives     │                │
│  │ streams        │  │ streams        │  │ (thresholds)   │                │
│  └───────┬────────┘  └───────┬────────┘  └───────┬────────┘                │
│          │                   │                   │                          │
│          ▼                   └────────┬──────────┘                          │
│  ┌────────────────┐          ┌───────▼─────────┐                           │
│  │ Transition     │          │ Threshold       │                           │
│  │ Materializer   │          │ Crossing Gen    │                           │
│  │ (v11-006)      │          │ (v11-012)       │                           │
│  └───────┬────────┘          └───────┬─────────┘                           │
│          │                           │                                      │
│          └───────────┬───────────────┘                                      │
│                      ▼                                                       │
│          ┌────────────────────────────────┐                                 │
│          │  gold.events_unified           │  ← Unified abstraction         │
│          │  (state + threshold in V1.1)   │                                 │
│          └───────────────┬────────────────┘                                 │
│                          │                                                   │
│          ┌───────────────┼───────────────┐                                  │
│          ▼               ▼               ▼                                  │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐                        │
│  │ Exact events │ │ Hourly agg   │ │ V1.2 Corr.   │                        │
│  │ (response    │ │ (screening)  │ │ Engine       │                        │
│  │  analysis)   │ │              │ │              │                        │
│  └──────────────┘ └──────────────┘ └──────────────┘                        │
│                                                                              │
│  FUTURE ADDITIONS (dotted lines in impl)                                    │
│  ┌────────────────┐  ┌────────────────┐                                    │
│  │ Anomaly Det.   │  │ Trend Change   │                                    │
│  │ (V1.2)         │  │ (V1.3)         │                                    │
│  └────────────────┘  └────────────────┘                                    │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

#### Extensibility Strategy: Plugin-Like Pattern

To add new feature types (beyond lag, rolling, trend), the architecture should support:

1. **Feature Type Registry**: Known feature types with SQL generation patterns
2. **New Type Addition**: Add new feature type by implementing a `FeatureGenerator` trait
3. **Config-Driven Activation**: Enable/disable features in JSON config

```rust
// Conceptual interface
trait FeatureGenerator {
    fn feature_type(&self) -> &str;  // "lag", "rolling", "trend", "custom"
    fn generate_sql(&self, config: &FeatureConfig, stream: &StreamConfig) -> String;
    fn validate_config(&self, config: &FeatureConfig) -> Result<(), ValidationError>;
}

// Registry of known feature generators
struct FeatureRegistry {
    generators: HashMap<String, Box<dyn FeatureGenerator>>,
}
```

This allows V1.2+ to add new feature types (e.g., `fourier`, `wavelet`) without modifying core architecture.

#### Deliberate Fast-Follower Test

**Strategy**: Implement V1.1 with 3 streams in alignment:
- `air-quality` (observation)
- `outdoor-weather` (observation)
- `home-assistant-state` (state_event)

**Deliberately exclude**: `outdoor-air-quality`

**Fast-follower test**: After V1.1 is complete, add `outdoor-air-quality` to Gold layer. Success criteria:
- Only JSON config changes required
- No Rust code changes
- New stream appears in aligned view within 15 minutes (refresh cycle)

If this test passes, V1.1 architecture is proven extensible.

---

### V1.1 Features (Architecture-First, Then Capabilities)

**Tier 1: Architecture (Must Have)**

| ID | Feature | Description | Depends On | Enables |
|----|---------|-------------|------------|---------|
| **v11-A01** | Gold ETL JSON Schema | JSON Schema for `gold_etl` config section | V1.0 schema validation | All Gold features |
| **v11-A02** | Gold ETL Interpreter | Rust module that reads config, generates SQL | v11-A01 schema | Declarative Gold layer |
| **v11-A03** | Alignment JSON Schema | JSON Schema for cross-stream alignment config | V1.0 schema validation | Cross-stream views |
| **v11-A04** | Alignment Interpreter | Rust module that generates aligned view SQL | v11-A03 schema | v11-005 aligned view |
| **v11-A05** | Objectives JSON Schema | JSON Schema for objectives config | V1.0 schema validation | V1.2 relevance filtering |
| **v11-A06** | Feature Type Registry | Extensible registry for feature generators | v11-A02 interpreter | Future feature types |

**Tier 2: Capabilities (Enabled by Architecture)**

| ID | Feature | Description | Depends On | Enables |
|----|---------|-------------|------------|---------|
| **v11-001** | Stream Type Classification | Add `stream_type` enum to stream config schema | V1.0 stream config | V1.2 scanner knows causes vs effects |
| **v11-002** | Classification Propagation | Stream type flows to Silver metadata, data dictionary | v11-001 schema | V1.2 query filtering |
| **v11-003** | Per-Stream Continuous Aggregates | Hourly aggregates for each Silver table | v11-A02 interpreter | V1.2 aligned queries |
| **v11-004** | Aggregate Refresh Policy | Auto-refresh every 15 min, 4-hour lookback | v11-003 aggregates | Real-time dashboards |
| **v11-005** | Cross-Stream Aligned View | Materialized view joining all streams hourly | v11-A04 interpreter | V1.2 correlation scanning |
| **v11-006** | State Transition Materializer | Derive "opened at X" events from state stream | V1.0 state_events table | V1.2 response detection |
| **v11-007** | Objectives Storage | Store objectives in etcd, expose via MCP | v11-A05 schema | V1.2, V1.3 objective queries |
| **v11-008** | Basic Feature Computation | Rolling mean, std, min, max per metric | v11-A06 registry | V1.2 enhanced correlations |
| **v11-009** | Lag Feature Computation | Metric values at t-1h, t-6h, t-24h | v11-A06 registry | V1.2 lag analysis |
| **v11-010** | Gold Layer Data Dictionary | Metadata for Gold tables and views | v11-003, v11-005 | Discoverability |
| **v11-011** | Correlation-Ready Dashboard | Grafana showing aligned streams + objectives | v11-005, v11-007 | V1.1 proof point |
| **v11-012** | Threshold Crossing Generator | Generate events when metrics cross objective thresholds | v11-007 objectives | V1.2 unified events |
| **v11-013** | Unified Events View | Combine state transitions + threshold crossings | v11-006, v11-012 | V1.2 correlation engine |

**Tier 3: Validation (Proves Architecture)**

| ID | Feature | Description | Depends On | Enables |
|----|---------|-------------|------------|---------|
| **v11-V01** | Fast-Follower Stream Test | Add `outdoor-air-quality` via config only | v11-A02, v11-A04 | Architecture validation |
| **v11-V02** | New Feature Type Test | Add a new feature type via registry | v11-A06 registry | Extensibility validation |

### V1.1 Feature Details

#### Architecture Features (v11-A01 through v11-A06)

##### v11-A01: Gold ETL JSON Schema

**Purpose**: Define the JSON Schema for the `gold_etl` section in stream configs. This schema is the contract for how Gold layer capabilities are declared.

**Location**: `config/schemas/gold-etl.schema.json`

**Key Schema Elements**:
```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "$id": "gold-etl.schema.json",
  "type": "object",
  "properties": {
    "gold_etl": {
      "type": "object",
      "properties": {
        "enabled": { "type": "boolean", "default": true },
        "aggregates": { "$ref": "#/definitions/aggregates" },
        "features": { "$ref": "#/definitions/features" },
        "transitions": { "$ref": "#/definitions/transitions" }
      }
    }
  },
  "definitions": {
    "aggregates": {
      "type": "object",
      "properties": {
        "granularities": {
          "type": "array",
          "items": { "type": "string", "pattern": "^\\d+ (hour|day|minute)s?$" }
        },
        "fields": {
          "type": "object",
          "additionalProperties": {
            "type": "object",
            "properties": {
              "metrics": {
                "type": "array",
                "items": { "enum": ["mean", "std", "min", "max", "count", "p95", "p99"] }
              }
            }
          }
        }
      }
    },
    "features": {
      "type": "object",
      "properties": {
        "lag": { "$ref": "#/definitions/lagFeature" },
        "rolling": { "$ref": "#/definitions/rollingFeature" },
        "trend": { "$ref": "#/definitions/trendFeature" }
      }
    }
  }
}
```

**Acceptance Criteria**:
- [ ] Schema defined and documented
- [ ] Validates against example gold_etl configs
- [ ] Integrated with existing schema validation pipeline
- [ ] Error messages are helpful for config authors

##### v11-A02: Gold ETL Interpreter

**Purpose**: Rust module that reads `gold_etl` config and generates TimescaleDB SQL for continuous aggregates.

**Location**: `core/src/gold/interpreter.rs` (new module)

**Key Responsibilities**:
1. Parse `gold_etl` JSON config
2. Generate `CREATE MATERIALIZED VIEW ... WITH (timescaledb.continuous)` SQL
3. Generate `add_continuous_aggregate_policy()` SQL
4. Execute via tokio-postgres
5. Track generated objects in metadata

**Interface** (conceptual):
```rust
pub struct GoldETLInterpreter {
    db_pool: Pool<Postgres>,
    schema_validator: SchemaValidator,
}

impl GoldETLInterpreter {
    pub async fn apply_stream_config(&self, stream_config: &StreamConfig) -> Result<GoldETLResult>;
    pub async fn generate_sql(&self, stream_config: &StreamConfig) -> Result<Vec<String>>;
    pub async fn validate_config(&self, gold_etl: &GoldETLConfig) -> Result<()>;
}
```

**Acceptance Criteria**:
- [ ] Generates valid TimescaleDB SQL
- [ ] Idempotent (can re-run safely)
- [ ] Logs generated SQL for debugging
- [ ] Validates config before execution

##### v11-A05: Objectives JSON Schema

**Purpose**: Define the JSON Schema for objectives configuration.

**Location**: `config/schemas/objectives.schema.json`

**Schema**:
```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "$id": "objectives.schema.json",
  "type": "object",
  "properties": {
    "objectives": {
      "type": "array",
      "items": {
        "type": "object",
        "required": ["id", "targets"],
        "properties": {
          "id": { "type": "string", "pattern": "^[a-z_]+$" },
          "description": { "type": "string" },
          "targets": {
            "type": "array",
            "items": {
              "type": "object",
              "required": ["stream", "metric", "condition", "threshold"],
              "properties": {
                "stream": { "type": "string" },
                "metric": { "type": "string" },
                "condition": { "enum": ["<", ">", "<=", ">=", "==", "between"] },
                "threshold": {
                  "oneOf": [
                    { "type": "number" },
                    { "type": "array", "items": { "type": "number" }, "minItems": 2, "maxItems": 2 }
                  ]
                },
                "unit": { "type": "string" },
                "priority": { "enum": ["high", "medium", "low"], "default": "medium" }
              }
            }
          },
          "constraints": { "type": "array" }
        }
      }
    }
  }
}
```

**Acceptance Criteria**:
- [ ] Schema defined and documented
- [ ] Example objectives for air quality domain
- [ ] Validates against example configs
- [ ] Parseable by Rust (serde)

---

#### Capability Features (v11-001 through v11-011)

##### v11-001: Stream Type Classification

**Purpose**: V1.2 correlation scanner needs to know which streams are potential causes (state events) vs potential effects (continuous observations).

**Schema Addition** (in stream config JSON):
```json
{
  "stream_id": "home-assistant-state",
  "stream_type": "state_event",
  ...
}
```

**Type Definitions**:

| Type | Description | Correlation Role | Examples |
|------|-------------|------------------|----------|
| `observation` | Continuous numeric readings | **Effect** (target of correlation) | PM2.5, CO2, temperature |
| `state_event` | Binary/discrete state changes | **Cause** (source of correlation) | Window open/close, door |
| `forecast` | Future predictions from external source | **Context** (not cause/effect) | NWS weather forecast |
| `dimension` | Slowly changing reference data | **Metadata** (not correlated) | Entity context, locations |

**Acceptance Criteria**:
- [ ] JSON Schema updated with `stream_type` enum
- [ ] All existing streams classified
- [ ] Validation rejects unknown types
- [ ] Data dictionary shows stream type

---

#### v11-002: Classification Propagation

**Purpose**: Stream type must be queryable in Gold layer and visible in data dictionary.

**Implementation**:
```sql
-- Add to Silver metadata or Gold view
CREATE VIEW gold.stream_metadata AS
SELECT
    stream_id,
    stream_type,
    description,
    ...
FROM data_dictionary.streams;

-- Query to find all "cause" streams
SELECT stream_id FROM gold.stream_metadata WHERE stream_type = 'state_event';
```

**Acceptance Criteria**:
- [ ] Stream type in data dictionary
- [ ] MCP tool can query stream types
- [ ] Gold views include stream type column

---

#### v11-003: Per-Stream Continuous Aggregates

**Purpose**: Provide fast, pre-computed hourly statistics for each stream. Foundation for alignment and correlation.

**Implementation**:
```sql
-- Example for air_quality
CREATE MATERIALIZED VIEW gold.air_quality_hourly
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('1 hour', observation_time) AS bucket,
    ndp_id,

    -- Central tendency
    AVG(pm25) AS pm25_mean,
    AVG(co2) AS co2_mean,
    AVG(temperature_c) AS temp_mean,
    AVG(humidity_pct) AS humidity_mean,

    -- Variability
    STDDEV(pm25) AS pm25_std,
    STDDEV(co2) AS co2_std,

    -- Extremes
    MAX(pm25) AS pm25_max,
    MIN(pm25) AS pm25_min,
    MAX(co2) AS co2_max,
    MIN(co2) AS co2_min,

    -- Quality
    COUNT(*) AS sample_count,
    COUNT(*) FILTER (WHERE dq_flags IS NULL) AS clean_samples

FROM silver.air_quality_observations
GROUP BY bucket, ndp_id;
```

**Streams Requiring Aggregates**:

| Stream | Silver Table | Gold Aggregate |
|--------|--------------|----------------|
| air-quality | `silver.air_quality_observations` | `gold.air_quality_hourly` |
| outdoor-air-quality | `silver.outdoor_air_quality_observations` | `gold.outdoor_aqi_hourly` |
| outdoor-weather | `silver.outdoor_weather_observations` | `gold.outdoor_weather_hourly` |
| home-assistant-state | `silver.state_events` | `gold.state_events_hourly` (special handling) |
| nws-forecast-hourly | `silver.nws_forecast_hourly` | `gold.forecast_hourly` (passthrough) |

**Acceptance Criteria**:
- [ ] Continuous aggregate exists for each stream
- [ ] Refresh policy configured (15 min interval, 4 hour lookback)
- [ ] Compression policy for data > 7 days
- [ ] Query performance < 100ms for 30-day range

---

#### v11-004: Aggregate Refresh Policy

**Purpose**: Keep aggregates current without overwhelming Pi resources.

**Implementation**:
```sql
SELECT add_continuous_aggregate_policy('gold.air_quality_hourly',
    start_offset => INTERVAL '4 hours',    -- Re-compute last 4 hours
    end_offset => INTERVAL '15 minutes',   -- Leave 15-min buffer for late data
    schedule_interval => INTERVAL '15 minutes'  -- Run every 15 min
);
```

**Resource Budget**:
- Refresh should use < 100MB RAM peak
- < 5% CPU sustained
- Complete within 30 seconds

**Acceptance Criteria**:
- [ ] Policies configured for all aggregates
- [ ] Resource usage within budget
- [ ] Monitoring query shows refresh status

---

#### v11-005: Cross-Stream Aligned View

**Purpose**: Single materialized view joining all streams on hourly buckets. **This is the primary input to V1.2 correlation scanner.**

**Implementation**:
```sql
CREATE MATERIALIZED VIEW gold.aligned_hourly
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('1 hour', COALESCE(
        aq.bucket,
        oaq.bucket,
        ow.bucket,
        se.bucket
    )) AS bucket,

    -- Indoor Air Quality (continuous - potential effects)
    aq.pm25_mean AS indoor_pm25,
    aq.co2_mean AS indoor_co2,
    aq.temp_mean AS indoor_temp,
    aq.humidity_mean AS indoor_humidity,

    -- Outdoor Air Quality (continuous - context)
    oaq.pm25_mean AS outdoor_pm25,
    oaq.aqi_mean AS outdoor_aqi,

    -- Outdoor Weather (continuous - context/causes)
    ow.temp_mean AS outdoor_temp,
    ow.humidity_mean AS outdoor_humidity,
    ow.wind_speed_mean AS wind_speed,
    ow.pressure_mean AS pressure,

    -- State Events (discrete - potential causes)
    -- Aggregated as: count of transitions, last known state
    se.window_opens,      -- Count of open events this hour
    se.window_closes,     -- Count of close events this hour
    se.last_window_state, -- State at end of hour

    -- Forecast (context - not cause/effect)
    fc.forecast_temp,
    fc.forecast_precip_prob

FROM gold.air_quality_hourly aq
FULL OUTER JOIN gold.outdoor_aqi_hourly oaq ON aq.bucket = oaq.bucket
FULL OUTER JOIN gold.outdoor_weather_hourly ow ON aq.bucket = ow.bucket
FULL OUTER JOIN gold.state_events_hourly se ON aq.bucket = se.bucket
FULL OUTER JOIN gold.forecast_hourly fc ON aq.bucket = fc.bucket;
```

**Key Design Decisions**:

1. **FULL OUTER JOIN**: Preserves rows even when some streams have no data
2. **Hourly granularity**: Balances detail vs noise for correlation
3. **State aggregation**: Counts transitions, not just current state
4. **NULL handling**: V1.2 must handle sparse data gracefully

**Acceptance Criteria**:
- [ ] View created with all current streams
- [ ] Query returns data for last 30 days
- [ ] NULLs handled correctly (no dropped rows)
- [ ] Documentation of column meanings

---

#### v11-006: Generic Transition Materializer

**Purpose**: Convert raw state events into explicit "transition" records. This is a **generic building block** that works for any `state_event` stream—no domain knowledge required.

**Design Principle**: The system doesn't know about "windows" or "doors." It understands **transitions**:

```
TRANSITION = {
    entity_id,           // Which thing changed (ndp_id)
    transition_time,     // When it changed (exact timestamp)
    from_state,          // Previous state value
    to_state,            // New state value
    is_actual_change,    // Did state actually change? (filters noise)
    duration_in_prev     // How long was it in previous state
}
```

**What V1.2 Pattern Detection Engine Needs**:

| V1.2 Process | Input Required | Granularity |
|--------------|----------------|-------------|
| Correlation Screening | Transition counts per hour | Hourly (in aligned view) |
| Response Analysis | Exact transition events | Exact timestamps |
| Lag Discovery | Precise timing | Sub-hourly precision |

**V1.1 Provides Both**:
1. **Exact transition events** → `gold.{stream_id}_transitions` view
2. **Hourly aggregates** → transition counts in `gold.aligned_hourly`

**Config-Driven Generation**:

In stream config JSON:
```json
{
  "stream_id": "home-assistant-state",
  "stream_type": "state_event",

  "gold_etl": {
    "transitions": {
      "enabled": true,
      "state_field": "state",
      "entity_field": "ndp_id",
      "track_duration": true,
      "include_in_alignment": true
    }
  }
}
```

**Generated Artifact 1: Transition Events View**:

```sql
-- Generated by Gold ETL Interpreter for ANY state_event stream
-- No domain knowledge - pure state machine logic
CREATE VIEW gold.{stream_id}_transitions AS
SELECT
    event_time AS transition_time,
    ndp_id AS entity_id,
    LAG(state) OVER w AS from_state,
    state AS to_state,

    -- Only TRUE when state actually changed (filters repeated events)
    CASE
        WHEN LAG(state) OVER w IS DISTINCT FROM state THEN TRUE
        WHEN LAG(state) OVER w IS NULL THEN TRUE  -- First event for entity
        ELSE FALSE
    END AS is_actual_transition,

    -- Duration in previous state (NULL for first event)
    event_time - LAG(event_time) OVER w AS duration_in_previous_state

FROM silver.state_events
WHERE stream_id = '{stream_id}'
WINDOW w AS (PARTITION BY ndp_id ORDER BY event_time);

-- Indexes for V1.2 queries
CREATE INDEX idx_{stream_id}_trans_time
    ON gold.{stream_id}_transitions (transition_time);
CREATE INDEX idx_{stream_id}_trans_entity_time
    ON gold.{stream_id}_transitions (entity_id, transition_time);
```

**Generated Artifact 2: Hourly Transition Aggregates** (for aligned view):

```sql
-- Included in gold.aligned_hourly via alignment config
SELECT
    time_bucket('1 hour', transition_time) AS bucket,

    -- Transition counts by resulting state
    COUNT(*) FILTER (WHERE to_state = 'on' AND is_actual_transition)
        AS {stream_alias}_to_on_count,
    COUNT(*) FILTER (WHERE to_state = 'off' AND is_actual_transition)
        AS {stream_alias}_to_off_count,
    COUNT(*) FILTER (WHERE is_actual_transition)
        AS {stream_alias}_transition_count,

    -- State at end of hour (for point-in-time queries)
    LAST(to_state, transition_time) AS {stream_alias}_state_eoh

FROM gold.{stream_id}_transitions
GROUP BY bucket;
```

**Clean Boundaries**:

| V1.1 Knows | V1.1 Does NOT Know |
|------------|-------------------|
| State changed from X to Y | What X and Y mean |
| Transition happened at time T | Which transitions are "interesting" |
| Entity was in state X for N minutes | What to correlate with |
| How to aggregate into hourly counts | What lag to expect |

**V1.2 Identifies**: "When entity `door_backslider` transitions to state `on`, metric `co2` decreases with 17-minute lag."

**Acceptance Criteria**:
- [ ] Transition view generated from config (no hardcoded stream names)
- [ ] Works for ANY state_event stream
- [ ] `is_actual_transition` correctly filters noise
- [ ] Duration computed accurately
- [ ] Hourly aggregates included in aligned view
- [ ] Indexes support V1.2 query patterns

---

#### v11-007: Objectives Storage

**Purpose**: Store objectives in etcd and expose via MCP. Uses the schema defined in v11-A05.

**Note**: Schema is defined in **v11-A05 (Objectives JSON Schema)**. This feature handles storage and retrieval.

**Example Objectives Config** (JSON format per V1.0 standards):

```json
{
  "objectives": [
    {
      "id": "indoor_air_quality",
      "description": "Maintain healthy indoor air",
      "targets": [
        {
          "metric": "co2",
          "stream": "air-quality",
          "condition": "<",
          "threshold": 800,
          "unit": "ppm",
          "priority": "high"
        },
        {
          "metric": "pm25",
          "stream": "air-quality",
          "condition": "<",
          "threshold": 12,
          "unit": "µg/m³",
          "priority": "high"
        },
        {
          "metric": "humidity_pct",
          "stream": "air-quality",
          "condition": "between",
          "threshold": [40, 60],
          "unit": "%",
          "priority": "medium"
        }
      ],
      "constraints": [
        {
          "description": "Don't open window if outdoor air is bad",
          "stream": "outdoor-air-quality",
          "metric": "pm25",
          "condition": "<",
          "threshold": 35
        }
      ]
    }
  ]
}
```

**etcd Storage**:
```
/platform/objectives -> { full JSON blob }
```

**MCP Tools** (for Claude Code access):
- `mcp__ndp__list_objectives()` → All objective IDs
- `mcp__ndp__get_objective(id)` → Single objective details
- `mcp__ndp__list_target_metrics()` → All metrics with targets (for V1.2 filtering)

**Acceptance Criteria**:
- [ ] Objectives stored in etcd as JSON blob
- [ ] Sync script updates etcd from config file
- [ ] MCP tools expose objectives
- [ ] V1.2 can query target metrics for correlation filtering

**JSON Schema** (defined in v11-A05, shown here for reference):
```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "type": "object",
  "properties": {
    "objectives": {
      "type": "array",
      "items": {
        "type": "object",
        "required": ["id", "targets"],
        "properties": {
          "id": { "type": "string" },
          "description": { "type": "string" },
          "targets": {
            "type": "array",
            "items": {
              "type": "object",
              "required": ["metric", "stream", "condition", "threshold"],
              "properties": {
                "metric": { "type": "string" },
                "stream": { "type": "string" },
                "condition": { "enum": ["<", ">", "<=", ">=", "==", "between"] },
                "threshold": { "oneOf": [{"type": "number"}, {"type": "array"}] },
                "unit": { "type": "string" },
                "priority": { "enum": ["high", "medium", "low"] }
              }
            }
          },
          "constraints": { "type": "array" }
        }
      }
    }
  }
}
```

**Acceptance Criteria**:
- [ ] Schema defined and documented
- [ ] Example objectives for air quality domain
- [ ] Validation against schema
- [ ] Parseable by Rust (serde)

---

#### v11-008: Objectives Storage

**Purpose**: Store objectives in etcd for runtime access. Expose via MCP for Claude Code queries.

**etcd Structure**:
```
/platform/objectives/indoor_air_quality -> { JSON blob }
/platform/objectives/energy_efficiency -> { JSON blob }
```

**MCP Tool**:
```typescript
// Get objectives
mcp__ndp__get_objectives() -> ObjectivesConfig

// Get specific objective
mcp__ndp__get_objective(id: "indoor_air_quality") -> Objective

// List target metrics (for V1.2 correlation filtering)
mcp__ndp__list_target_metrics() -> ["air-quality.co2", "air-quality.pm25", ...]
```

**Acceptance Criteria**:
- [ ] Objectives stored in etcd
- [ ] Sync script updates etcd from YAML
- [ ] MCP tool exposes objectives
- [ ] Grafana can query objectives for threshold lines

---

#### v11-009: Basic Feature Computation

**Purpose**: Compute rolling statistics that enhance correlation detection in V1.2.

**Features to Compute**:

| Feature | Formula | Purpose |
|---------|---------|---------|
| `{metric}_mean_4h` | AVG over last 4 hours | Short-term level |
| `{metric}_mean_24h` | AVG over last 24 hours | Daily level |
| `{metric}_std_4h` | STDDEV over last 4 hours | Volatility |
| `{metric}_trend_4h` | Linear regression slope | Direction |
| `{metric}_diff_1h` | Current - 1 hour ago | Rate of change |

**Implementation**: Add to continuous aggregates using window functions.

**Acceptance Criteria**:
- [ ] Features computed for key metrics (pm25, co2, temp)
- [ ] Available in aligned view
- [ ] Query performance acceptable

---

#### v11-010: Lag Feature Computation

**Purpose**: Pre-compute lagged values for V1.2 lag analysis.

**Implementation**:
```sql
-- In aligned view or separate feature view
SELECT
    bucket,
    indoor_co2,
    LAG(indoor_co2, 1) OVER (ORDER BY bucket) AS co2_lag_1h,
    LAG(indoor_co2, 6) OVER (ORDER BY bucket) AS co2_lag_6h,
    LAG(indoor_co2, 24) OVER (ORDER BY bucket) AS co2_lag_24h,
    ...
FROM gold.aligned_hourly;
```

**Acceptance Criteria**:
- [ ] Lag features for 1h, 6h, 24h
- [ ] Computed for target metrics
- [ ] NULL handling for edge cases

---

#### v11-011: Gold Layer Data Dictionary

**Purpose**: Metadata for Gold tables/views for discoverability.

**Contents**:
- Table/view name
- Description
- Column definitions
- Stream type (if applicable)
- Refresh policy
- Dependencies

**Acceptance Criteria**:
- [ ] All Gold objects documented
- [ ] Queryable via SQL or MCP
- [ ] Consistent with Silver data dictionary pattern

---

#### v11-012: Correlation-Ready Dashboard

**Purpose**: Grafana dashboard demonstrating V1.1 capabilities. Sets up visual foundation for V1.2 pattern candidates.

**Panels**:

1. **Time-Aligned Multi-Stream View**
   - Indoor CO2 line
   - Outdoor AQI line
   - Window state markers (vertical lines on open/close)
   - Temperature overlay

2. **Objective Status**
   - CO2 gauge with 800 ppm threshold
   - PM2.5 gauge with 12 µg/m³ threshold
   - Time below/above threshold

3. **State Transition Log**
   - Recent window open/close events
   - Duration in each state

4. **Feature Trends**
   - 4-hour rolling mean for key metrics
   - Trend direction indicators

**Acceptance Criteria**:
- [ ] Dashboard created in Grafana
- [ ] Uses Gold layer views (not Silver)
- [ ] Shows all stream types aligned
- [ ] Objective thresholds visible
- [ ] Performant (< 2s load time)

---

#### v11-012: Threshold Crossing Event Generator

**Purpose**: Generate explicit events when observation metrics cross objective thresholds. These events join with state transitions in the unified event abstraction, enabling V1.2 to discover correlations involving threshold crossings.

**Key Insight**: Objectives already define meaningful thresholds. A metric crossing that threshold IS an event.

**Example**: Objective says `co2 < 800`. When CO2 rises from 780 to 820, that's a `threshold_crossing` event with `direction: rising`. When it falls back to 790, that's another event with `direction: falling`.

**Config-Driven**:
```json
{
  "gold_etl": {
    "threshold_crossings": {
      "enabled": true,
      "source": "objectives",
      "include_in_unified": true,
      "include_hysteresis": false
    }
  }
}
```

**Generated SQL**:
```sql
CREATE VIEW gold.threshold_crossings AS
WITH observation_with_thresholds AS (
    SELECT
        o.observation_time,
        o.stream_id,
        o.ndp_id,
        t.metric AS metric_name,
        t.threshold AS threshold_value,
        t.condition,
        obj.id AS objective_id,
        CASE t.metric
            WHEN 'co2' THEN o.co2
            WHEN 'pm25' THEN o.pm25
            WHEN 'temperature_c' THEN o.temperature_c
            WHEN 'humidity_pct' THEN o.humidity_pct
        END AS metric_value
    FROM silver.air_quality_observations o
    CROSS JOIN LATERAL (
        SELECT obj.id, tgt.*
        FROM objectives obj,
        LATERAL jsonb_to_recordset(obj.targets) AS tgt(
            metric text, threshold numeric, condition text, stream text
        )
        WHERE tgt.stream = 'air-quality'
    ) t(id, metric, threshold, condition, stream)
),
with_lag AS (
    SELECT
        *,
        LAG(metric_value) OVER (
            PARTITION BY stream_id, ndp_id, metric_name
            ORDER BY observation_time
        ) AS prev_value
    FROM observation_with_thresholds
)
SELECT
    observation_time AS event_time,
    stream_id,
    ndp_id AS entity_id,
    'threshold_crossing' AS event_type,
    metric_name,
    threshold_value,
    objective_id,
    metric_value AS current_value,
    prev_value AS previous_value,
    CASE
        -- For < condition (target is to stay below)
        WHEN condition = '<' AND metric_value >= threshold_value
             AND prev_value < threshold_value THEN 'rising'
        WHEN condition = '<' AND metric_value < threshold_value
             AND prev_value >= threshold_value THEN 'falling'
        -- For > condition (target is to stay above)
        WHEN condition = '>' AND metric_value <= threshold_value
             AND prev_value > threshold_value THEN 'falling'
        WHEN condition = '>' AND metric_value > threshold_value
             AND prev_value <= threshold_value THEN 'rising'
    END AS crossing_direction
FROM with_lag
WHERE (
    -- Detect crossing for < condition
    (condition = '<' AND metric_value >= threshold_value AND prev_value < threshold_value)
    OR (condition = '<' AND metric_value < threshold_value AND prev_value >= threshold_value)
    -- Detect crossing for > condition
    OR (condition = '>' AND metric_value <= threshold_value AND prev_value > threshold_value)
    OR (condition = '>' AND metric_value > threshold_value AND prev_value <= threshold_value)
);

-- Index for efficient querying
CREATE INDEX idx_threshold_crossings_time
    ON gold.threshold_crossings (event_time);
CREATE INDEX idx_threshold_crossings_metric
    ON gold.threshold_crossings (metric_name, event_time);
```

**What This Enables for V1.2**:
- "When CO2 exceeds 800 (threshold event), does user open window within 30 min?"
- "When PM2.5 falls below 12 (threshold event), does HVAC turn off?"
- Correlate threshold breaches across streams (outdoor PM2.5 spike → indoor PM2.5 rise)

**Acceptance Criteria**:
- [ ] Generates threshold crossing events from objectives
- [ ] Detects both rising and falling crossings
- [ ] Works for all condition types (<, >, <=, >=)
- [ ] Includes in unified events view
- [ ] Config-driven (enabled per stream via gold_etl)
- [ ] Indexed for V1.2 query patterns

---

#### v11-013: Unified Events View

**Purpose**: Combine state transitions (v11-006) and threshold crossings (v11-013) into a single queryable view. This is the primary input for V1.2 correlation engine.

**Design Principle**: V1.2 doesn't care HOW an event was generated (state change vs threshold cross). It asks: "When EVENT happens, what changes?"

**Implementation**:
```sql
CREATE VIEW gold.events_unified AS

-- State transition events (from v11-006)
SELECT
    transition_time AS event_time,
    stream_id,
    entity_id,
    'state_transition'::text AS event_type,
    jsonb_build_object(
        'from_state', from_state,
        'to_state', to_state,
        'duration_in_previous_ms',
            EXTRACT(EPOCH FROM duration_in_previous_state) * 1000
    ) AS details
FROM gold.state_transitions
WHERE is_actual_transition = TRUE

UNION ALL

-- Threshold crossing events (from v11-013)
SELECT
    event_time,
    stream_id,
    entity_id,
    'threshold_crossing'::text AS event_type,
    jsonb_build_object(
        'metric', metric_name,
        'threshold', threshold_value,
        'direction', crossing_direction,
        'value', current_value,
        'previous_value', previous_value,
        'objective_id', objective_id
    ) AS details
FROM gold.threshold_crossings;

-- Future: UNION ALL from gold.anomaly_events (V1.2)
-- Future: UNION ALL from gold.trend_change_events (V1.3)
```

**Hourly Aggregation for Correlation Screening**:
```sql
CREATE MATERIALIZED VIEW gold.events_hourly
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('1 hour', event_time) AS bucket,

    -- Total event counts
    COUNT(*) AS total_events,
    COUNT(*) FILTER (WHERE event_type = 'state_transition') AS state_transition_count,
    COUNT(*) FILTER (WHERE event_type = 'threshold_crossing') AS threshold_crossing_count,

    -- State transitions by result state
    COUNT(*) FILTER (WHERE event_type = 'state_transition'
        AND details->>'to_state' = 'on') AS transitions_to_on,
    COUNT(*) FILTER (WHERE event_type = 'state_transition'
        AND details->>'to_state' = 'off') AS transitions_to_off,

    -- Threshold crossings by direction
    COUNT(*) FILTER (WHERE event_type = 'threshold_crossing'
        AND details->>'direction' = 'rising') AS threshold_rising_count,
    COUNT(*) FILTER (WHERE event_type = 'threshold_crossing'
        AND details->>'direction' = 'falling') AS threshold_falling_count,

    -- Threshold crossings by metric (for targeted correlation)
    COUNT(*) FILTER (WHERE event_type = 'threshold_crossing'
        AND details->>'metric' = 'co2') AS co2_threshold_events,
    COUNT(*) FILTER (WHERE event_type = 'threshold_crossing'
        AND details->>'metric' = 'pm25') AS pm25_threshold_events

FROM gold.events_unified
GROUP BY bucket;

-- Refresh policy
SELECT add_continuous_aggregate_policy('gold.events_hourly',
    start_offset => INTERVAL '4 hours',
    end_offset => INTERVAL '15 minutes',
    schedule_interval => INTERVAL '15 minutes'
);
```

**Integration with Aligned View**:
```sql
-- Update gold.aligned_hourly to include event counts
ALTER VIEW gold.aligned_hourly AS
SELECT
    -- ... existing stream columns ...

    -- Event counts from unified events
    e.total_events,
    e.state_transition_count,
    e.threshold_crossing_count,
    e.threshold_rising_count,
    e.threshold_falling_count

FROM gold.air_quality_hourly aq
-- ... existing joins ...
LEFT JOIN gold.events_hourly e ON aq.bucket = e.bucket;
```

**Extensibility for Future Event Types**:

| Version | Event Type | Source | Added Via |
|---------|------------|--------|-----------|
| V1.1 | state_transition | v11-006 | Initial |
| V1.1 | threshold_crossing | v11-012 | Initial |
| V1.2 | anomaly | gold.anomaly_events | UNION ALL |
| V1.3 | trend_change | gold.trend_change_events | UNION ALL |

**Acceptance Criteria**:
- [ ] View combines state transitions and threshold crossings
- [ ] Consistent event schema (event_time, stream_id, entity_id, event_type, details)
- [ ] Details JSONB contains type-specific payload
- [ ] Hourly aggregation continuous aggregate created
- [ ] Refresh policy configured
- [ ] Integrated into aligned view
- [ ] Query performance < 100ms for 30-day range
- [ ] Documented extension points for V1.2/V1.3 event types

---

## V1.1 Implementation Phases

### Phase A: Architecture Foundation (Week 1-2)

**Focus**: Build the extensible architecture before implementing specific capabilities.

| Feature | Description | Priority |
|---------|-------------|----------|
| v11-A01 | Gold ETL JSON Schema | Critical |
| v11-A02 | Gold ETL Interpreter (basic) | Critical |
| v11-A03 | Alignment JSON Schema | Critical |
| v11-A05 | Objectives JSON Schema | High |
| v11-001 | Stream Type Classification | High |

**Exit Criteria**:
- JSON Schemas defined and validated
- Basic interpreter can generate SQL from config
- Stream types added to existing configs
- Architecture review completed

### Phase B: First Stream (Week 3)

**Focus**: Apply architecture to `air-quality` stream as reference implementation.

| Feature | Description | Priority |
|---------|-------------|----------|
| v11-002 | Classification Propagation | High |
| v11-003 | Per-Stream Continuous Aggregates (air-quality) | Critical |
| v11-004 | Aggregate Refresh Policy | High |
| v11-A06 | Feature Type Registry (basic) | High |
| v11-008 | Basic Feature Computation (air-quality) | Medium |

**Exit Criteria**:
- `gold.air_quality_hourly` generated from config
- Refresh policy operational
- At least one feature type (lag or rolling) working
- **Config-only change can modify aggregate fields**

### Phase C: Cross-Stream + Alignment (Week 4)

**Focus**: Extend to remaining streams, build alignment view.

| Feature | Description | Priority |
|---------|-------------|----------|
| v11-003 | Per-Stream Continuous Aggregates (outdoor-weather, state-events) | Critical |
| v11-A04 | Alignment Interpreter | Critical |
| v11-005 | Cross-Stream Aligned View (3 streams) | Critical |
| v11-006 | State Transition Materializer | High |
| v11-007 | Objectives Storage | Medium |

**Exit Criteria**:
- 3 streams in Gold layer (air-quality, outdoor-weather, home-assistant-state)
- Aligned view operational
- State transitions extractable
- Objectives stored in etcd

**Deliberately excluded**: `outdoor-air-quality` (saved for fast-follower test)

### Phase D: Validation + Fast-Follower (Week 5)

**Focus**: Prove the architecture works by adding a new stream via config only.

| Feature | Description | Priority |
|---------|-------------|----------|
| v11-V01 | **Fast-Follower Test**: Add `outdoor-air-quality` | Critical |
| v11-010 | Gold Layer Data Dictionary | Medium |
| v11-011 | Correlation-Ready Dashboard | High |
| - | Integration testing | High |
| - | Documentation | Medium |

**Exit Criteria**:
- `outdoor-air-quality` added to Gold layer via **config change only**
- No Rust code changes required for fast-follower
- Dashboard demonstrates all capabilities
- Architecture validated for V1.2

### Phase E: Unified Event Abstraction (Extended)

**Focus**: Implement the unified event abstraction with state transitions + threshold crossings.

**Note**: V1.1 delivery extends until complete. This phase ensures V1.2 has a consistent event interface.

| Feature | Description | Priority |
|---------|-------------|----------|
| v11-012 | Threshold Crossing Generator | Critical |
| v11-013 | Unified Events View | Critical |
| - | Events hourly continuous aggregate | High |
| - | Integration with aligned view | High |
| - | Event type extensibility documentation | Medium |

**Exit Criteria**:
- Threshold crossing events generated from objectives config
- Unified events view combines state + threshold events
- Hourly event aggregates in aligned view
- V1.2 correlation engine can query unified events
- Documentation shows how to add future event types (anomaly, trend)

**V1.2 Handoff Verification**:
- [ ] V1.2 can query `gold.events_unified` for all event types
- [ ] V1.2 can correlate events with observation metrics
- [ ] Event schema is stable (documented contract)
- [ ] Future event types (anomaly, trend) have documented extension points

---

## V1.1 Timeline Note

**V1.1 delivery extends until complete.** The unified event abstraction (Phase E) is critical for V1.2's correlation engine. Rather than time-box and ship incomplete, V1.1 delivers when:

1. ✅ All architecture foundations operational (Phases A-C)
2. ✅ Fast-follower test passes (Phase D)
3. ✅ Unified event abstraction complete (Phase E)

This ensures V1.2 has a solid foundation to build upon.

---

## Success Metrics by Version

| Version | Primary Metric | Target |
|---------|---------------|--------|
| **V1.1** | **Architecture extensibility** | Add new stream via config only (no code) |
| **V1.1** | Gold layer query performance | < 100ms for 30-day aligned query |
| **V1.1** | Stream classification coverage | 100% of streams classified |
| **V1.1** | Objective declarability | User can declare targets in config |
| **V1.1** | Fast-follower time | < 1 hour to add new stream to Gold |
| **V1.1** | Unified event coverage | State + threshold events in single view |
| **V1.2** | Candidates identified | > 3 statistically significant candidate relationships |
| **V1.2** | False positive rate | < 30% of candidates are spurious |
| **V1.2** | Time to first candidate | < 7 days from deployment (with existing data) |
| **V1.3** | Prediction accuracy | > 80% for 1-hour forecasts |
| **V1.3** | Action success rate | > 80% of suggested actions achieve objective |
| **V1.3** | User override rate | < 20% for mature system |
| **V2.0** | New domain via config | Zero code changes to add domain + objectives |
| **V2.0** | Config-to-infrastructure | < 15 minutes from config to materialized Gold layer |
| **V2.0** | Cross-stream candidates | > 1 candidate found across streams in new domain |

---

## Risk Assessment

| Risk | Version | Likelihood | Impact | Mitigation |
|------|---------|------------|--------|------------|
| Insufficient historical data for correlation | V1.2 | Low | High | Existing Bronze data available; CSV backfill for new streams |
| Spurious correlations overwhelm real ones | V1.2 | High | Medium | Strict thresholds, objective filtering, user validation |
| Cross-stream alignment gaps (sparse data) | V1.1 | Medium | Medium | NULL handling, interpolation strategy |
| Prediction accuracy below 80% | V1.3 | Medium | High | Model ensemble, conservative confidence |
| Resource exhaustion on Pi | All | Low | High | Memory budgets, batch processing |
| Scope creep in feature computation | V1.1 | High | Medium | Strict 20-feature limit initially |

---

## Appendix: Feature Naming Convention

```
v{major}{minor}-{sequence}

v11-001  = V1.1, feature #1
v12-005  = V1.2, feature #5
v13-010  = V1.3, feature #10
v2-003   = V2.0, feature #3
```

---

## Next Steps

1. **Review this document** - Validate feature definitions and dependencies
2. **Prioritize V1.1 features** - Identify any that can be deferred
3. **Create feature specs** - SPARC documentation for each V1.1 feature
4. **Estimate effort** - Refine 5-week estimate based on feature details
5. **Begin Phase A** - Stream classification and first aggregates

---

*Document created using backwards design from V2.0 vision*
*Each feature exists because the next version requires it*
