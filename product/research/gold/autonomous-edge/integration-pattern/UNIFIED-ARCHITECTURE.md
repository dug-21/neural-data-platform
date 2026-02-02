# Unified Architecture for Autonomous Edge Intelligence

**Research Date:** 2026-02-02
**Platform Target:** Raspberry Pi 5 (16GB RAM, ARM Cortex-A76)
**Domain:** Cross-domain autonomous intelligence (Air Quality, Financial, General)
**Status:** Research Complete

---

## Executive Summary

This document defines the integration pattern for **Autonomous Edge Intelligence** - a unified system that combines correlation discovery, causal learning, and objective-driven action into a coherent architecture that runs entirely on edge devices.

### The Core Capability

```
OBSERVE --> DISCOVER --> HYPOTHESIZE --> TEST --> ACT --> LEARN
```

**What makes this autonomous:**
- Discovers correlations without human guidance
- Forms and tests causal hypotheses independently
- Takes actions based on learned relationships and objectives
- Continuously improves through feedback loops
- Operates entirely offline with optional cloud sync

### Key Architectural Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| **Memory Architecture** | 3-tier (Hot/Warm/Cold) | Balance speed vs persistence |
| **Scheduling Strategy** | Priority-based with budgets | Preserve real-time responsiveness |
| **Discovery Trigger** | Data-change + Time-based hybrid | Catch regime shifts, bound compute |
| **Causal Testing** | Shadow interventions + Natural experiments | Safe, observable |
| **Action Selection** | Thompson Sampling + Objective hierarchy | Balance exploration/exploitation |
| **Graceful Degradation** | 4-level fallback | Ensure always-on operation |

---

## 1. The Full Loop Architecture

### 1.1 System Overview

```
+=========================================================================+
|                    AUTONOMOUS EDGE INTELLIGENCE                          |
+=========================================================================+
|                                                                          |
|   OBSERVE                 DISCOVER                 HYPOTHESIZE           |
|   +-------------+        +---------------+        +---------------+      |
|   | Data Streams|        | Correlation   |        | Causal        |      |
|   | - Sensors   |------->| Discovery     |------->| Hypothesis    |      |
|   | - Events    |        | Engine        |        | Generator     |      |
|   | - External  |        |               |        |               |      |
|   +-------------+        +---------------+        +---------------+      |
|          |                      |                        |               |
|          |                      v                        v               |
|          |               +---------------+        +---------------+      |
|          |               | Pattern       |        | Hypothesis    |      |
|          |               | Memory        |        | Queue         |      |
|          |               | (Discovered   |        | (To Test)     |      |
|          |               |  Correlations)|        |               |      |
|          |               +---------------+        +---------------+      |
|          |                      |                        |               |
|          v                      v                        v               |
|   +-------------+        +---------------+        +---------------+      |
|   | Working     |<------>| Long-term     |<------>| Causal        |      |
|   | Memory      |        | Memory        |        | Testing       |      |
|   | (Recent)    |        | (Learned)     |        | Engine        |      |
|   +-------------+        +---------------+        +---------------+      |
|          |                      |                        |               |
|          v                      v                        v               |
|   +-------------+        +---------------+        +---------------+      |
|   | Feature     |        | Action        |        | Verified      |      |
|   | Engineering |        | Selection     |<-------| Causal        |      |
|   |             |        | Engine        |        | Relationships |      |
|   +-------------+        +---------------+        +---------------+      |
|          |                      |                        |               |
|          |                      v                        |               |
|          |               +---------------+               |               |
|          |               | OBJECTIVES    |               |               |
|          +-------------->| - User goals  |<--------------+               |
|                          | - Constraints |                               |
|                          | - Priorities  |                               |
|                          +-------+-------+                               |
|                                  |                                       |
|   ACT                            v                    LEARN              |
|   +-------------+        +---------------+        +---------------+      |
|   | Actuator    |<-------| Decision      |------->| Feedback      |      |
|   | Interface   |        | Engine        |        | Collector     |      |
|   | - Alerts    |        |               |        |               |      |
|   | - Logs      |        |               |        |               |      |
|   | - External  |        |               |        |               |      |
|   +-------------+        +---------------+        +---------------+      |
|          |                                               |               |
|          v                                               v               |
|   +-------------+                                +---------------+       |
|   | World       |------------------------------->| Outcome       |       |
|   | State       |                                | Evaluation    |       |
|   +-------------+                                +---------------+       |
|                                                         |                |
|                                                         v                |
|                                                  +---------------+       |
|                                                  | Model Update  |       |
|                                                  | (EWC++/LoRA)  |       |
|                                                  +---------------+       |
|                                                                          |
+=========================================================================+
```

### 1.2 Component Data Flow

```
TIMING DIAGRAM - Single Cycle (5 minutes typical)
================================================

T+0s     T+60s    T+120s   T+180s   T+240s   T+300s
|        |        |        |        |        |
[OBSERVE]------->|        |        |        |
         [FEATURE]------>|        |        |
                  [PREDICT]----->|        |
                           [ACT]-------->|
                                   [OUTCOME]-->
                                          [LEARN]

Background (when idle):
        [----DISCOVER----]        [--DISCOVER--]
                   [HYPOTHESIZE]        [TEST]
```

### 1.3 Stage Definitions

| Stage | Input | Output | Latency Budget | CPU Budget |
|-------|-------|--------|----------------|------------|
| **OBSERVE** | Raw sensor data | Validated readings | <100ms | 5% |
| **FEATURE** | Validated readings | Feature vectors | <50ms | 5% |
| **PREDICT** | Features + Context | Predictions | <100ms | 10% |
| **ACT** | Predictions + Objectives | Actions | <50ms | 5% |
| **LEARN** | Outcomes | Model updates | 1-5s (async) | 20% |
| **DISCOVER** | Historical data | Correlations | 10-60s (background) | 30% |
| **HYPOTHESIZE** | Correlations | Causal hypotheses | 5-30s (background) | 15% |
| **TEST** | Hypotheses + Data | Verified causations | Minutes-hours | 10% |

---

## 2. Memory Architecture

### 2.1 Three-Tier Memory Model

```
+=========================================================================+
|                         MEMORY ARCHITECTURE                              |
+=========================================================================+
|                                                                          |
|   TIER 1: HOT MEMORY (In-Process)                                       |
|   +-----------------------------------------------------------------+   |
|   |  Access: <1ms | Size: 50-200MB | TTL: Session/Minutes            |   |
|   |                                                                   |   |
|   |  +---------------+  +---------------+  +---------------+         |   |
|   |  | Working       |  | Feature       |  | Prediction    |         |   |
|   |  | Memory        |  | Cache         |  | Cache         |         |   |
|   |  | (VecDeque)    |  | (LRU)         |  | (LRU)         |         |   |
|   |  +---------------+  +---------------+  +---------------+         |   |
|   |                                                                   |   |
|   |  +---------------+  +---------------+  +---------------+         |   |
|   |  | Correlation   |  | Causal        |  | Action        |         |   |
|   |  | Hot Index     |  | Quick Lookup  |  | History       |         |   |
|   |  | (Top 100)     |  | (Top 50)      |  | (Last 1000)   |         |   |
|   |  +---------------+  +---------------+  +---------------+         |   |
|   +-----------------------------------------------------------------+   |
|                                      |                                   |
|                                      v (periodic flush)                  |
|   TIER 2: WARM MEMORY (SQLite + HNSW)                                   |
|   +-----------------------------------------------------------------+   |
|   |  Access: 1-10ms | Size: 500MB-2GB | TTL: Days-Weeks               |   |
|   |                                                                   |   |
|   |  +------------------------------+  +---------------------------+ |   |
|   |  | AgentDB Vector Store         |  | Pattern Memory            | |   |
|   |  | - Discovered correlations    |  | - Successful strategies   | |   |
|   |  | - Historical contexts        |  | - Failed approaches       | |   |
|   |  | - Semantic search enabled    |  | - Consolidated skills     | |   |
|   |  | - HNSW index (150x faster)   |  | - Causal relationships    | |   |
|   |  +------------------------------+  +---------------------------+ |   |
|   |                                                                   |   |
|   |  +------------------------------+  +---------------------------+ |   |
|   |  | Episode Store (Reflexion)    |  | Hypothesis Store          | |   |
|   |  | - Task executions            |  | - Pending tests           | |   |
|   |  | - Self-critiques             |  | - Test results            | |   |
|   |  | - Reward signals             |  | - Confidence scores       | |   |
|   |  +------------------------------+  +---------------------------+ |   |
|   +-----------------------------------------------------------------+   |
|                                      |                                   |
|                                      v (archive/sync)                    |
|   TIER 3: COLD MEMORY (TimescaleDB / Parquet)                           |
|   +-----------------------------------------------------------------+   |
|   |  Access: 10-100ms | Size: 10GB+ | TTL: Months-Years               |   |
|   |                                                                   |   |
|   |  +------------------------------+  +---------------------------+ |   |
|   |  | TimescaleDB                  |  | Parquet Archive           | |   |
|   |  | - Time-series data           |  | - Historical raw data     | |   |
|   |  | - Continuous aggregates      |  | - Compressed patterns     | |   |
|   |  | - Materialized features      |  | - Model checkpoints       | |   |
|   |  +------------------------------+  +---------------------------+ |   |
|   |                                                                   |   |
|   |  +------------------------------+  +---------------------------+ |   |
|   |  | Causal Graph (Persistent)    |  | Correlation Archive       | |   |
|   |  | - Verified relationships     |  | - Historical discoveries  | |   |
|   |  | - Confidence + sample size   |  | - Regime-tagged           | |   |
|   |  | - Domain-specific graphs     |  | - Validation history      | |   |
|   |  +------------------------------+  +---------------------------+ |   |
|   +-----------------------------------------------------------------+   |
|                                                                          |
+=========================================================================+
```

### 2.2 Memory Components Detail

#### 2.2.1 Working Memory (Hot)

```rust
/// Hot memory for real-time operations
pub struct WorkingMemory {
    /// Recent sensor readings (last N minutes)
    pub sensor_buffer: HashMap<StreamId, VecDeque<TimeSeriesPoint>>,

    /// Computed features cache (LRU)
    pub feature_cache: LruCache<FeatureKey, FeatureVector>,

    /// Recent predictions for feedback matching
    pub prediction_log: VecDeque<PredictionRecord>,

    /// Action history for outcome correlation
    pub action_history: VecDeque<ActionRecord>,

    /// Hot correlation index (most relevant)
    pub hot_correlations: Vec<CorrelationSummary>,

    /// Quick causal lookup (verified relationships)
    pub causal_shortcuts: HashMap<(Variable, Variable), CausalEffect>,
}

/// Configuration for working memory
pub struct WorkingMemoryConfig {
    pub sensor_buffer_minutes: u32,      // 60
    pub feature_cache_size: usize,       // 1000
    pub prediction_log_size: usize,      // 500
    pub action_history_size: usize,      // 1000
    pub hot_correlation_count: usize,    // 100
}
```

**Memory Budget (Hot):**
| Component | Size | Entries | Notes |
|-----------|------|---------|-------|
| Sensor buffers | 50MB | 60K points/stream | 6 streams x 10K |
| Feature cache | 40MB | 1000 vectors | 40KB per vector |
| Prediction log | 5MB | 500 records | With context |
| Action history | 10MB | 1000 records | With outcomes |
| Hot correlations | 5MB | 100 summaries | Indexed |
| Causal shortcuts | 10MB | 500 pairs | Quick lookup |
| **Total** | **~120MB** | | Fits in RAM |

#### 2.2.2 Pattern Memory (Warm)

```rust
/// Warm memory via AgentDB
pub struct PatternMemory {
    /// Vector database for semantic search
    pub agentdb: AgentDbClient,

    /// Episode store for reflexion-based learning
    pub episodes: ReflexionStore,

    /// Skill library for consolidated patterns
    pub skills: SkillLibrary,

    /// Hypothesis management
    pub hypotheses: HypothesisStore,
}

/// Memory namespaces
pub enum MemoryNamespace {
    /// Discovered correlations with embeddings
    Correlations,
    /// Causal relationships (verified)
    CausalEdges,
    /// Task execution episodes
    Episodes,
    /// Consolidated successful patterns
    Skills,
    /// Pending causal hypotheses
    Hypotheses,
    /// Domain-specific context
    DomainContext,
}
```

**Integration with AgentDB:**
```rust
// Store discovered correlation
agentdb.insert(CorrelationVector {
    text: "PM2.5 leads outdoor_aqi by 30min with r=0.72",
    embedding: embed(&correlation_description),
    metadata: CorrelationMetadata {
        variable_x: "indoor_pm25",
        variable_y: "outdoor_aqi",
        lag_minutes: 30,
        correlation: 0.72,
        p_value: 0.001,
        discovery_date: now(),
        regime: "winter_heating",
        validation_status: ValidationStatus::Pending,
    },
    tags: vec!["air_quality", "cross_stream", "lag_30m"],
});

// Semantic search for similar patterns
let similar = agentdb.search(
    query: "indoor air quality predicting outdoor conditions",
    k: 10,
    filters: Some(Filters {
        min_correlation: 0.5,
        tags: Some(vec!["air_quality"]),
    }),
);
```

#### 2.2.3 Causal Graph (Cold)

```sql
-- Causal relationship storage
CREATE TABLE causal_edges (
    id SERIAL PRIMARY KEY,
    cause_variable VARCHAR(100) NOT NULL,
    effect_variable VARCHAR(100) NOT NULL,
    domain VARCHAR(50) NOT NULL,            -- 'air_quality', 'financial', etc.

    -- Causal strength and confidence
    causal_effect DOUBLE PRECISION NOT NULL,
    confidence DOUBLE PRECISION NOT NULL,    -- 0-1
    p_value DOUBLE PRECISION,

    -- Evidence
    sample_size INTEGER NOT NULL,
    discovery_method VARCHAR(50),            -- 'granger', 'transfer_entropy', 'intervention'
    test_method VARCHAR(50),                 -- 'natural_experiment', 'shadow_intervention'

    -- Temporal characteristics
    lag_minutes INTEGER,
    effect_duration_minutes INTEGER,

    -- Validity
    first_observed TIMESTAMPTZ NOT NULL,
    last_validated TIMESTAMPTZ NOT NULL,
    regime_context VARCHAR(100),             -- 'winter', 'high_volatility', etc.
    is_active BOOLEAN DEFAULT TRUE,

    -- Indexing
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),

    UNIQUE(cause_variable, effect_variable, domain)
);

-- Create hypertable for time-based queries
SELECT create_hypertable('causal_edges', 'last_validated');

-- Index for quick lookups
CREATE INDEX idx_causal_active ON causal_edges(cause_variable, is_active)
    WHERE is_active = TRUE;
CREATE INDEX idx_causal_domain ON causal_edges(domain, confidence DESC);
```

### 2.3 Memory Flow Patterns

```
DATA LIFETIME FLOW
==================

[New Data] --> [Hot Memory] --> [Warm Memory] --> [Cold Memory]
                  |                  |                 |
               <1min             <1 week           >1 week
                  |                  |                 |
            Real-time ops      Learning/Search    Archive/Audit

PROMOTION CRITERIA:
- Hot -> Warm: Validated correlations, successful patterns
- Warm -> Cold: Verified causal edges, archived episodes

DEMOTION CRITERIA:
- Cold -> Warm: Frequently accessed patterns (LRU promotion)
- Warm -> Hot: Actively used correlations (access count)

EXPIRATION:
- Hot: Session-based or time-based (5-60 min)
- Warm: Usage-based (unused 30+ days) + confidence decay
- Cold: Retention policy (1-2 years) or explicit archive
```

---

## 3. Scheduling and Orchestration

### 3.1 Scheduler Architecture

```
+=========================================================================+
|                      SCHEDULING ARCHITECTURE                             |
+=========================================================================+
|                                                                          |
|   PRIORITY LEVELS                                                        |
|   +-----------------------------------------------------------------+   |
|   | P0: CRITICAL    | Real-time data validation, alerts             |   |
|   | P1: HIGH        | Predictions, feature engineering               |   |
|   | P2: NORMAL      | Action selection, outcome logging              |   |
|   | P3: LOW         | Correlation discovery, hypothesis testing      |   |
|   | P4: BACKGROUND  | Model training, memory consolidation           |   |
|   +-----------------------------------------------------------------+   |
|                                                                          |
|   EXECUTION MODEL                                                        |
|   +-----------------------------------------------------------------+   |
|   |                                                                   |   |
|   |   +------------------+                                           |   |
|   |   | Foreground Loop  |  <-- Always runs, P0-P2 tasks             |   |
|   |   | (tokio runtime)  |                                           |   |
|   |   +------------------+                                           |   |
|   |            |                                                      |   |
|   |            v                                                      |   |
|   |   +------------------+     +------------------+                   |   |
|   |   | Background Pool  |---->| Worker Thread 1  | Discovery        |   |
|   |   | (rayon/tokio)    |---->| Worker Thread 2  | Training         |   |
|   |   |                  |---->| Worker Thread 3  | Consolidation    |   |
|   |   +------------------+     +------------------+                   |   |
|   |                                                                   |   |
|   +-----------------------------------------------------------------+   |
|                                                                          |
|   BUDGET ALLOCATION                                                      |
|   +-----------------------------------------------------------------+   |
|   |                                                                   |   |
|   |   CPU Budget by Priority:                                        |   |
|   |   +-------+-------+--------+----------+------------+             |   |
|   |   | P0:20%| P1:25%| P2:15% |  P3:25%  |  P4:15%    |             |   |
|   |   +-------+-------+--------+----------+------------+             |   |
|   |                                                                   |   |
|   |   Memory Budget:                                                  |   |
|   |   +---------------+---------------+---------------+              |   |
|   |   | Hot: 200MB    | Warm: 1.5GB   | Cold: 10GB+   |              |   |
|   |   +---------------+---------------+---------------+              |   |
|   |                                                                   |   |
|   +-----------------------------------------------------------------+   |
|                                                                          |
+=========================================================================+
```

### 3.2 Task Scheduling Rules

```rust
/// Task scheduling configuration
pub struct SchedulerConfig {
    /// Foreground tasks
    pub data_validation_interval: Duration,      // 1s
    pub feature_computation_interval: Duration,  // 5s
    pub prediction_interval: Duration,           // 60s
    pub action_evaluation_interval: Duration,    // 60s

    /// Background tasks
    pub correlation_discovery_interval: Duration, // 6h
    pub hypothesis_testing_interval: Duration,   // 1h
    pub model_training_interval: Duration,       // 24h
    pub memory_consolidation_interval: Duration, // 6h

    /// Budget enforcement
    pub max_background_cpu_percent: f32,         // 40%
    pub background_pause_threshold_cpu: f32,     // 80%
    pub background_pause_threshold_mem: f32,     // 85%
}

/// Task priority and characteristics
pub enum TaskPriority {
    Critical,   // Cannot be deferred, <100ms budget
    High,       // 1s budget, can be briefly deferred
    Normal,     // 5s budget, deferrable
    Low,        // No deadline, preemptible
    Background, // Runs only when idle
}

/// Scheduling decision logic
impl Scheduler {
    pub fn should_run(&self, task: &Task, system_state: &SystemState) -> Decision {
        match task.priority {
            TaskPriority::Critical => Decision::RunImmediately,

            TaskPriority::High if system_state.cpu_percent < 90.0 =>
                Decision::RunImmediately,

            TaskPriority::Normal if system_state.cpu_percent < 80.0 =>
                Decision::RunImmediately,

            TaskPriority::Low if system_state.cpu_percent < 60.0 &&
                                 system_state.memory_percent < 70.0 =>
                Decision::RunImmediately,

            TaskPriority::Background if system_state.cpu_percent < 40.0 &&
                                        system_state.memory_percent < 60.0 &&
                                        !system_state.has_pending_high_priority =>
                Decision::RunInBackground,

            _ => Decision::Defer(calculate_backoff(task, system_state)),
        }
    }
}
```

### 3.3 Discovery Scheduling

```rust
/// When to trigger correlation discovery
pub struct DiscoveryTriggers {
    /// Time-based (guaranteed discovery cadence)
    pub scheduled_interval: Duration,       // 6 hours

    /// Data-change triggers
    pub min_new_data_points: usize,         // 1000 points
    pub data_drift_threshold: f64,          // ADWIN detection

    /// Event triggers
    pub on_regime_change: bool,             // True
    pub on_new_data_source: bool,           // True
    pub on_user_request: bool,              // True

    /// Budget constraints
    pub max_daily_discoveries: usize,       // 4
    pub max_concurrent_discoveries: usize,  // 1
}

/// Discovery scheduling logic
impl DiscoveryScheduler {
    pub async fn check_triggers(&self) -> Option<DiscoveryTask> {
        // 1. Check scheduled interval
        if self.last_discovery.elapsed() > self.config.scheduled_interval {
            return Some(DiscoveryTask::Full);
        }

        // 2. Check data volume threshold
        if self.new_data_count() >= self.config.min_new_data_points {
            return Some(DiscoveryTask::Incremental);
        }

        // 3. Check drift detection
        if self.drift_detector.detect_drift() {
            return Some(DiscoveryTask::RegimeChange);
        }

        // 4. Check event triggers
        if let Some(event) = self.pending_events.pop() {
            match event {
                DiscoveryEvent::NewDataSource(id) =>
                    return Some(DiscoveryTask::NewSource(id)),
                DiscoveryEvent::UserRequest(params) =>
                    return Some(DiscoveryTask::Targeted(params)),
                _ => {}
            }
        }

        None
    }
}
```

### 3.4 Causal Testing Schedule

```rust
/// Hypothesis testing configuration
pub struct CausalTestingConfig {
    /// Testing cadence
    pub test_interval: Duration,              // 1 hour
    pub max_tests_per_interval: usize,        // 5

    /// Prioritization
    pub priority_by_confidence: bool,         // Test high-confidence first
    pub priority_by_impact: bool,             // Test high-impact first

    /// Resource limits
    pub max_test_duration: Duration,          // 10 minutes
    pub min_evidence_required: usize,         // 100 observations
}

/// Test scheduling
impl CausalTestScheduler {
    pub async fn select_next_test(&self) -> Option<HypothesisTest> {
        // Get pending hypotheses sorted by priority
        let candidates = self.hypothesis_store
            .get_pending()
            .sorted_by(|a, b| self.calculate_priority(b).cmp(&self.calculate_priority(a)))
            .take(10)
            .collect::<Vec<_>>();

        for hypothesis in candidates {
            // Check if we have enough evidence to test
            let evidence_count = self.count_evidence(&hypothesis);
            if evidence_count < self.config.min_evidence_required {
                continue;
            }

            // Check if natural experiment conditions are met
            if self.can_test_naturally(&hypothesis) {
                return Some(HypothesisTest {
                    hypothesis,
                    method: TestMethod::NaturalExperiment,
                    evidence_count,
                });
            }

            // Check if shadow intervention is possible
            if self.can_shadow_test(&hypothesis) {
                return Some(HypothesisTest {
                    hypothesis,
                    method: TestMethod::ShadowIntervention,
                    evidence_count,
                });
            }
        }

        None
    }

    fn calculate_priority(&self, hypothesis: &Hypothesis) -> f64 {
        let confidence_score = hypothesis.prior_confidence * 0.4;
        let impact_score = hypothesis.expected_impact * 0.4;
        let age_penalty = hypothesis.age_days() as f64 * 0.01;
        let urgency = if hypothesis.related_to_active_objective { 0.2 } else { 0.0 };

        confidence_score + impact_score - age_penalty + urgency
    }
}
```

---

## 4. Resource Budget

### 4.1 CPU Budget Allocation

```
+=========================================================================+
|                        CPU BUDGET (100% = 4 cores)                       |
+=========================================================================+
|                                                                          |
|   NORMAL OPERATION (Steady State)                                       |
|   +-----------------------------------------------------------------+   |
|   |                                                                   |   |
|   |   Foreground (60%)                Background (30%)    Reserve    |   |
|   |   +-------------------------+     +---------------+   +-------+  |   |
|   |   | P0 Critical    : 20%   |     | Discovery: 15%|   | 10%   |  |   |
|   |   | P1 High        : 25%   |     | Training : 10%|   | System|  |   |
|   |   | P2 Normal      : 15%   |     | Consolidate:5%|   |       |  |   |
|   |   +-------------------------+     +---------------+   +-------+  |   |
|   |                                                                   |   |
|   +-----------------------------------------------------------------+   |
|                                                                          |
|   HIGH LOAD (CPU > 80%)                                                 |
|   +-----------------------------------------------------------------+   |
|   |                                                                   |   |
|   |   Foreground (85%)                Background (5%)     Reserve    |   |
|   |   +-------------------------+     +---------------+   +-------+  |   |
|   |   | P0 Critical    : 30%   |     | Paused/       |   | 10%   |  |   |
|   |   | P1 High        : 35%   |     | Minimal       |   |       |  |   |
|   |   | P2 Normal      : 20%   |     |               |   |       |  |   |
|   |   +-------------------------+     +---------------+   +-------+  |   |
|   |                                                                   |   |
|   +-----------------------------------------------------------------+   |
|                                                                          |
|   DISCOVERY BURST (Scheduled Window)                                    |
|   +-----------------------------------------------------------------+   |
|   |                                                                   |   |
|   |   Foreground (40%)                Background (50%)    Reserve    |   |
|   |   +-------------------------+     +---------------+   +-------+  |   |
|   |   | P0 Critical    : 20%   |     | Discovery: 35%|   | 10%   |  |   |
|   |   | P1 High        : 15%   |     | Training : 10%|   |       |  |   |
|   |   | P2 Deferred    :  5%   |     | Other    :  5%|   |       |  |   |
|   |   +-------------------------+     +---------------+   +-------+  |   |
|   |                                                                   |   |
|   +-----------------------------------------------------------------+   |
|                                                                          |
+=========================================================================+
```

### 4.2 Memory Budget Allocation

```
+=========================================================================+
|                    MEMORY BUDGET (16GB Pi 5)                             |
+=========================================================================+
|                                                                          |
|   FIXED ALLOCATIONS (4GB)                                               |
|   +-----------------------------------------------------------------+   |
|   | OS + Services           : 1.0 GB                                 |   |
|   | TimescaleDB             : 1.5 GB                                 |   |
|   | Rust Application Base   : 0.5 GB                                 |   |
|   | AgentDB/SQLite          : 1.0 GB                                 |   |
|   +-----------------------------------------------------------------+   |
|                                                                          |
|   DYNAMIC ALLOCATIONS (6GB)                                             |
|   +-----------------------------------------------------------------+   |
|   | Hot Memory (Working)    : 0.2 GB  | Always allocated             |   |
|   | Feature Cache           : 0.3 GB  | LRU managed                  |   |
|   | Model Inference         : 0.5 GB  | ONNX Runtime                 |   |
|   | Discovery Buffer        : 1.0 GB  | Background, releasable       |   |
|   | Training Buffer         : 1.5 GB  | Background, releasable       |   |
|   | Correlation Analysis    : 1.0 GB  | Background, releasable       |   |
|   | HNSW Index              : 0.5 GB  | Pattern search               |   |
|   | Reserve                 : 1.0 GB  | Spike handling               |   |
|   +-----------------------------------------------------------------+   |
|                                                                          |
|   RESERVED (6GB)                                                        |
|   +-----------------------------------------------------------------+   |
|   | Future expansion, safety margin                                  |   |
|   +-----------------------------------------------------------------+   |
|                                                                          |
+=========================================================================+

MEMORY PRESSURE RESPONSE:
=========================

Level 1 (70% used): Release discovery buffers
Level 2 (80% used): Pause background tasks, release training buffers
Level 3 (85% used): Reduce feature cache, alert
Level 4 (90% used): Emergency mode - critical tasks only
```

### 4.3 Task Resource Requirements

| Task | CPU % | Memory | Duration | Frequency |
|------|-------|--------|----------|-----------|
| Data validation | 2% | 10MB | <10ms | Continuous |
| Feature engineering | 5% | 100MB | <50ms | Per reading |
| Prediction | 10% | 200MB | <100ms | Per minute |
| Action selection | 5% | 50MB | <50ms | Per minute |
| Outcome logging | 2% | 20MB | <10ms | Per action |
| Correlation discovery | 30% | 1GB | 10-60s | 4x daily |
| Hypothesis generation | 15% | 500MB | 5-30s | After discovery |
| Causal testing | 20% | 500MB | 1-10min | 5x daily |
| Model training | 40% | 1.5GB | 5-30min | Daily |
| Memory consolidation | 10% | 500MB | 1-5min | 4x daily |

---

## 5. Graceful Degradation

### 5.1 Degradation Levels

```
+=========================================================================+
|                    GRACEFUL DEGRADATION LEVELS                           |
+=========================================================================+
|                                                                          |
|   LEVEL 0: FULL CAPABILITY                                              |
|   +-----------------------------------------------------------------+   |
|   | - All discovery/learning active                                  |   |
|   | - Full prediction models                                         |   |
|   | - Proactive recommendations                                      |   |
|   | - Background optimization running                                |   |
|   | Trigger: Normal operation (CPU <60%, Mem <70%)                   |   |
|   +-----------------------------------------------------------------+   |
|                                                                          |
|   LEVEL 1: REDUCED LEARNING                                             |
|   +-----------------------------------------------------------------+   |
|   | - Discovery paused (use cached correlations)                     |   |
|   | - Training deferred (use existing models)                        |   |
|   | - Full predictions continue                                      |   |
|   | - Reactive recommendations only                                  |   |
|   | Trigger: High load (CPU 60-80% OR Mem 70-80%)                    |   |
|   +-----------------------------------------------------------------+   |
|                                                                          |
|   LEVEL 2: ESSENTIAL OPERATIONS                                         |
|   +-----------------------------------------------------------------+   |
|   | - Predictions from lightweight models only                       |   |
|   | - Statistical anomaly detection only                             |   |
|   | - No new learning                                                |   |
|   | - Critical alerts only                                           |   |
|   | Trigger: Stressed (CPU 80-90% OR Mem 80-85%)                     |   |
|   +-----------------------------------------------------------------+   |
|                                                                          |
|   LEVEL 3: RULE-BASED FALLBACK                                          |
|   +-----------------------------------------------------------------+   |
|   | - Hardcoded threshold rules only                                 |   |
|   | - No ML inference                                                |   |
|   | - Data logging continues                                         |   |
|   | - Emergency alerts only                                          |   |
|   | Trigger: Critical (CPU >90% OR Mem >85%)                         |   |
|   +-----------------------------------------------------------------+   |
|                                                                          |
|   LEVEL 4: SURVIVAL MODE                                                |
|   +-----------------------------------------------------------------+   |
|   | - Data buffering only (no processing)                            |   |
|   | - Write to disk when memory available                            |   |
|   | - System health alerts                                           |   |
|   | - Request human intervention                                     |   |
|   | Trigger: Emergency (CPU >95% OR Mem >90%)                        |   |
|   +-----------------------------------------------------------------+   |
|                                                                          |
+=========================================================================+
```

### 5.2 Component Essentiality

| Component | Level 0 | Level 1 | Level 2 | Level 3 | Level 4 |
|-----------|---------|---------|---------|---------|---------|
| Data ingestion | Full | Full | Full | Full | Buffer only |
| DQ validation | Full | Full | Statistical | Threshold | None |
| Feature engineering | Full | Full | Reduced | None | None |
| ML predictions | Full | Full | Lightweight | None | None |
| Rule-based checks | Active | Active | Active | Active | Active |
| Action selection | Optimized | Reactive | Essential | Emergency | None |
| Discovery | Active | Paused | Paused | Paused | Paused |
| Hypothesis testing | Active | Paused | Paused | Paused | Paused |
| Model training | Scheduled | Deferred | Deferred | Deferred | Deferred |
| Memory consolidation | Active | Reduced | Paused | Paused | Paused |

### 5.3 Degradation Implementation

```rust
/// System state monitoring
pub struct SystemMonitor {
    cpu_percent: AtomicU8,
    memory_percent: AtomicU8,
    current_level: AtomicU8,
}

impl SystemMonitor {
    pub fn get_degradation_level(&self) -> DegradationLevel {
        let cpu = self.cpu_percent.load(Ordering::Relaxed);
        let mem = self.memory_percent.load(Ordering::Relaxed);

        match (cpu, mem) {
            (c, m) if c > 95 || m > 90 => DegradationLevel::Survival,
            (c, m) if c > 90 || m > 85 => DegradationLevel::RuleBased,
            (c, m) if c > 80 || m > 80 => DegradationLevel::Essential,
            (c, m) if c > 60 || m > 70 => DegradationLevel::ReducedLearning,
            _ => DegradationLevel::Full,
        }
    }
}

/// Adaptive task execution
impl TaskRunner {
    pub async fn run_task(&self, task: Task) -> TaskResult {
        let level = self.monitor.get_degradation_level();

        match (&task.component, level) {
            // Always run data ingestion
            (Component::DataIngestion, _) => self.run_ingestion(task).await,

            // Predictions degrade gracefully
            (Component::Prediction, DegradationLevel::Full |
                                    DegradationLevel::ReducedLearning) =>
                self.run_full_prediction(task).await,
            (Component::Prediction, DegradationLevel::Essential) =>
                self.run_lightweight_prediction(task).await,
            (Component::Prediction, _) =>
                TaskResult::Skipped("degraded"),

            // Discovery only in full mode
            (Component::Discovery, DegradationLevel::Full) =>
                self.run_discovery(task).await,
            (Component::Discovery, _) =>
                TaskResult::Deferred,

            // Training only in full mode, scheduled
            (Component::Training, DegradationLevel::Full) if self.is_training_window() =>
                self.run_training(task).await,
            (Component::Training, _) =>
                TaskResult::Deferred,

            _ => self.default_handler(task, level).await,
        }
    }
}
```

### 5.4 Rule-Based Fallback Rules

```rust
/// Hardcoded rules for Level 3 degradation
pub struct FallbackRules {
    pub rules: Vec<FallbackRule>,
}

pub struct FallbackRule {
    pub name: String,
    pub condition: Condition,
    pub action: Action,
    pub priority: u8,
}

/// Example fallback rules
impl FallbackRules {
    pub fn default_air_quality_rules() -> Self {
        Self {
            rules: vec![
                // PM2.5 threshold alert
                FallbackRule {
                    name: "pm25_unhealthy".into(),
                    condition: Condition::GreaterThan("pm25", 55.4),
                    action: Action::Alert {
                        level: AlertLevel::Warning,
                        message: "PM2.5 exceeds EPA Unhealthy for Sensitive Groups".into(),
                    },
                    priority: 1,
                },
                // CO2 threshold alert
                FallbackRule {
                    name: "co2_high".into(),
                    condition: Condition::GreaterThan("co2", 1500.0),
                    action: Action::Alert {
                        level: AlertLevel::Warning,
                        message: "CO2 levels indicate poor ventilation".into(),
                    },
                    priority: 1,
                },
                // Rate of change alert
                FallbackRule {
                    name: "pm25_spike".into(),
                    condition: Condition::RateOfChange("pm25", 10.0, Duration::from_secs(300)),
                    action: Action::Alert {
                        level: AlertLevel::Info,
                        message: "Rapid PM2.5 increase detected".into(),
                    },
                    priority: 2,
                },
                // Sensor malfunction
                FallbackRule {
                    name: "sensor_stuck".into(),
                    condition: Condition::NoChange("pm25", Duration::from_secs(3600)),
                    action: Action::Alert {
                        level: AlertLevel::Warning,
                        message: "PM2.5 sensor may be malfunctioning".into(),
                    },
                    priority: 1,
                },
            ],
        }
    }
}
```

---

## 6. Minimum Viable Autonomous System (MVP)

### 6.1 MVP Definition

The MVP provides **observable, useful autonomous behavior** with minimal complexity.

```
+=========================================================================+
|                       MVP AUTONOMOUS SYSTEM                              |
+=========================================================================+
|                                                                          |
|   INCLUDED IN MVP                                                        |
|   +-----------------------------------------------------------------+   |
|   |                                                                   |   |
|   |   1. OBSERVE: Statistical data validation                        |   |
|   |      - Z-score anomaly detection per sensor                      |   |
|   |      - Range validation                                          |   |
|   |      - Rate-of-change limits                                     |   |
|   |                                                                   |   |
|   |   2. DISCOVER: Pairwise Granger causality                        |   |
|   |      - Test all sensor pairs                                     |   |
|   |      - Run nightly (low complexity)                              |   |
|   |      - Store top correlations                                    |   |
|   |                                                                   |   |
|   |   3. HYPOTHESIZE: Simple pattern matching                        |   |
|   |      - "If X rises, Y typically follows in N minutes"            |   |
|   |      - No complex causal graphs                                  |   |
|   |                                                                   |   |
|   |   4. TEST: Natural experiment observation                        |   |
|   |      - Wait for conditions to occur naturally                    |   |
|   |      - Track prediction vs outcome                               |   |
|   |      - Update confidence scores                                  |   |
|   |                                                                   |   |
|   |   5. ACT: Single-objective alerts                                |   |
|   |      - "Alert when PM2.5 likely to exceed threshold"             |   |
|   |      - Based on discovered correlations                          |   |
|   |                                                                   |   |
|   |   6. LEARN: Simple reward tracking                               |   |
|   |      - Did prediction match outcome?                             |   |
|   |      - Adjust confidence up/down                                 |   |
|   |                                                                   |   |
|   +-----------------------------------------------------------------+   |
|                                                                          |
|   EXCLUDED FROM MVP (Future Increments)                                 |
|   +-----------------------------------------------------------------+   |
|   | - Transfer entropy / mutual information                          |   |
|   | - Multi-hop causal chains                                        |   |
|   | - Active interventions                                           |   |
|   | - Multi-objective optimization                                   |   |
|   | - Neural network predictions                                     |   |
|   | - Federated learning                                             |   |
|   | - Complex action planning                                        |   |
|   +-----------------------------------------------------------------+   |
|                                                                          |
+=========================================================================+
```

### 6.2 MVP Architecture

```rust
/// MVP Autonomous System
pub struct MVPAutonomousSystem {
    // Data layer
    working_memory: WorkingMemory,
    pattern_store: SimplePatternStore,  // SQLite, not AgentDB

    // Discovery (simplified)
    granger_tester: GrangerCausalityTester,
    discovered_correlations: Vec<PairwiseCorrelation>,

    // Testing
    hypothesis_tracker: SimpleHypothesisTracker,

    // Action
    alert_engine: ThresholdAlertEngine,

    // Learning
    confidence_tracker: ConfidenceTracker,
}

/// MVP correlation structure
pub struct PairwiseCorrelation {
    pub variable_x: String,
    pub variable_y: String,
    pub lag_minutes: i32,
    pub correlation: f64,
    pub p_value: f64,
    pub confidence: f64,      // Updated by learning
    pub predictions_made: u32,
    pub predictions_correct: u32,
}

/// MVP implementation
impl MVPAutonomousSystem {
    /// Nightly discovery routine
    pub async fn run_discovery(&mut self) {
        // Get last 7 days of data
        let data = self.working_memory.get_historical(Duration::from_days(7));

        // Test all pairs
        let pairs = self.get_variable_pairs(&data);
        for (x, y) in pairs {
            if let Some(result) = self.granger_tester.test(&data, &x, &y) {
                if result.p_value < 0.05 && result.correlation.abs() > 0.3 {
                    self.discovered_correlations.push(PairwiseCorrelation {
                        variable_x: x,
                        variable_y: y,
                        lag_minutes: result.optimal_lag,
                        correlation: result.correlation,
                        p_value: result.p_value,
                        confidence: 0.5,  // Start neutral
                        predictions_made: 0,
                        predictions_correct: 0,
                    });
                }
            }
        }

        // Keep top 20 by confidence * |correlation|
        self.prune_correlations(20);
    }

    /// Real-time prediction check
    pub fn check_predictions(&mut self, current_data: &SensorReading) -> Vec<Alert> {
        let mut alerts = vec![];

        for corr in &self.discovered_correlations {
            // Check if X changed significantly
            let x_change = self.working_memory.get_change(&corr.variable_x, 5);

            if x_change.abs() > 2.0 {  // 2 std dev change
                // Predict Y change based on correlation
                let predicted_y_change = x_change * corr.correlation;
                let predicted_y = self.working_memory.current(&corr.variable_y)
                    + predicted_y_change;

                // Check if prediction crosses threshold
                if let Some(threshold) = self.get_threshold(&corr.variable_y) {
                    if predicted_y > threshold && self.working_memory.current(&corr.variable_y) < threshold {
                        alerts.push(Alert {
                            message: format!(
                                "{} likely to exceed {} in {} minutes (confidence: {:.0}%)",
                                corr.variable_y, threshold, corr.lag_minutes, corr.confidence * 100.0
                            ),
                            confidence: corr.confidence,
                            basis: format!("{} changed by {:.1}", corr.variable_x, x_change),
                        });

                        // Track prediction for learning
                        self.hypothesis_tracker.add_prediction(Prediction {
                            correlation_id: corr.id(),
                            predicted_value: predicted_y,
                            prediction_time: now(),
                            expected_outcome_time: now() + Duration::from_mins(corr.lag_minutes),
                        });
                    }
                }
            }
        }

        alerts
    }

    /// Learning from outcomes
    pub fn update_from_outcome(&mut self, outcome: &Outcome) {
        for prediction in self.hypothesis_tracker.get_pending_for(outcome) {
            let was_correct = self.evaluate_prediction(&prediction, outcome);

            if let Some(corr) = self.discovered_correlations
                .iter_mut()
                .find(|c| c.id() == prediction.correlation_id)
            {
                corr.predictions_made += 1;
                if was_correct {
                    corr.predictions_correct += 1;
                }

                // Update confidence (simple exponential moving average)
                let accuracy = corr.predictions_correct as f64 / corr.predictions_made as f64;
                corr.confidence = corr.confidence * 0.9 + accuracy * 0.1;
            }
        }
    }
}
```

### 6.3 MVP Resource Requirements

| Component | Memory | CPU (Avg) | Storage |
|-----------|--------|-----------|---------|
| Working memory | 50MB | 2% | - |
| Pattern store | 10MB | 1% | 50MB |
| Granger testing | 200MB (burst) | 20% (burst) | - |
| Hypothesis tracking | 10MB | 1% | 10MB |
| Alert engine | 5MB | 1% | - |
| Confidence tracking | 5MB | 0.5% | 5MB |
| **Total MVP** | **~280MB** | **~25%** | **~65MB** |

### 6.4 Incremental Capability Addition

```
MVP CAPABILITY LADDER
=====================

Level 1: MVP (As defined above)
- Pairwise Granger causality
- Simple predictions
- Confidence tracking
- Est. Effort: 2-3 weeks

Level 2: +Enhanced Discovery
- Add transfer entropy
- Add mutual information
- Multi-lag analysis
- Est. Effort: +2 weeks

Level 3: +Isolation Forest Anomalies
- Multivariate anomaly detection
- Anomaly as discovery trigger
- Est. Effort: +1 week

Level 4: +Sophisticated Testing
- Shadow interventions
- A/B testing framework
- Improved confidence intervals
- Est. Effort: +2 weeks

Level 5: +Neural Predictions
- ONNX model inference
- Ensemble predictions
- Est. Effort: +3 weeks

Level 6: +Multi-Objective Actions
- Thompson Sampling
- Objective hierarchy
- Action planning
- Est. Effort: +3 weeks

Level 7: +Full Causal Graph
- NOTEARS/PC algorithm
- Multi-hop reasoning
- Counterfactual analysis
- Est. Effort: +4 weeks

Level 8: +Federated Learning
- Multi-device coordination
- Privacy-preserving updates
- Est. Effort: +4 weeks
```

---

## 7. Cross-Domain Application

### 7.1 Domain-Agnostic Core

The architecture separates domain-specific components from the core autonomous loop:

```
+=========================================================================+
|                      DOMAIN-AGNOSTIC CORE                                |
+=========================================================================+
|                                                                          |
|   DOMAIN-AGNOSTIC (Reusable Across All Domains)                         |
|   +-----------------------------------------------------------------+   |
|   |                                                                   |   |
|   |   Discovery Engine                                               |   |
|   |   +---------------------------+                                  |   |
|   |   | - Granger causality       |                                  |   |
|   |   | - Transfer entropy        |                                  |   |
|   |   | - Mutual information      |                                  |   |
|   |   | - DTW correlation         |                                  |   |
|   |   +---------------------------+                                  |   |
|   |                                                                   |   |
|   |   Causal Testing                                                 |   |
|   |   +---------------------------+                                  |   |
|   |   | - Natural experiments     |                                  |   |
|   |   | - Shadow interventions    |                                  |   |
|   |   | - Confidence updating     |                                  |   |
|   |   +---------------------------+                                  |   |
|   |                                                                   |   |
|   |   Memory Management                                              |   |
|   |   +---------------------------+                                  |   |
|   |   | - Hot/Warm/Cold tiers     |                                  |   |
|   |   | - Vector search (HNSW)    |                                  |   |
|   |   | - Episode storage         |                                  |   |
|   |   +---------------------------+                                  |   |
|   |                                                                   |   |
|   |   Scheduling & Resources                                         |   |
|   |   +---------------------------+                                  |   |
|   |   | - Priority queues         |                                  |   |
|   |   | - Budget enforcement      |                                  |   |
|   |   | - Degradation handling    |                                  |   |
|   |   +---------------------------+                                  |   |
|   |                                                                   |   |
|   +-----------------------------------------------------------------+   |
|                                                                          |
|   DOMAIN-SPECIFIC (Pluggable per Domain)                                |
|   +-----------------------------------------------------------------+   |
|   |                                                                   |   |
|   |   Data Adapters              Objectives                          |   |
|   |   +----------------+         +------------------+                 |   |
|   |   | - Source trait |         | - Health goals   | (Air Quality)  |   |
|   |   | - Parser trait |         | - Cost goals     | (Financial)    |   |
|   |   | - Schema def   |         | - Custom KPIs    | (General)      |   |
|   |   +----------------+         +------------------+                 |   |
|   |                                                                   |   |
|   |   Domain Knowledge           Action Space                        |   |
|   |   +----------------+         +------------------+                 |   |
|   |   | - Valid ranges |         | - Alert types    |                |   |
|   |   | - Relationships|         | - Actuators      |                |   |
|   |   | - Constraints  |         | - Recommendations|                |   |
|   |   +----------------+         +------------------+                 |   |
|   |                                                                   |   |
|   +-----------------------------------------------------------------+   |
|                                                                          |
+=========================================================================+
```

### 7.2 Air Quality Domain Mapping

```rust
/// Air Quality domain configuration
pub struct AirQualityDomain {
    /// Variables to analyze
    pub variables: Vec<Variable> = vec![
        Variable { name: "indoor_pm25", unit: "ug/m3", range: (0.0, 500.0) },
        Variable { name: "indoor_co2", unit: "ppm", range: (400.0, 5000.0) },
        Variable { name: "indoor_temp", unit: "C", range: (10.0, 35.0) },
        Variable { name: "indoor_humidity", unit: "%", range: (0.0, 100.0) },
        Variable { name: "outdoor_aqi", unit: "AQI", range: (0.0, 500.0) },
        Variable { name: "outdoor_temp", unit: "C", range: (-20.0, 45.0) },
    ],

    /// Objectives
    pub objectives: Vec<Objective> = vec![
        Objective::Minimize {
            variable: "indoor_pm25",
            threshold: 12.0,  // EPA "Good"
            weight: 1.0,
        },
        Objective::Range {
            variable: "indoor_co2",
            target_min: 400.0,
            target_max: 1000.0,
            weight: 0.8,
        },
    ],

    /// Domain knowledge (prior relationships)
    pub prior_relationships: Vec<PriorRelationship> = vec![
        PriorRelationship {
            cause: "outdoor_aqi",
            effect: "indoor_pm25",
            expected_sign: Positive,
            expected_lag: Duration::from_mins(30),
            confidence: 0.7,
        },
        PriorRelationship {
            cause: "indoor_temp",
            effect: "indoor_co2",
            expected_sign: Positive,  // Warmer -> more occupant activity
            expected_lag: Duration::from_mins(0),
            confidence: 0.5,
        },
    ],

    /// Action space
    pub actions: Vec<Action> = vec![
        Action::Alert { name: "air_quality_warning", channels: vec!["push", "log"] },
        Action::Alert { name: "ventilation_recommendation", channels: vec!["push"] },
        Action::Log { name: "event_log" },
    ],
}
```

### 7.3 Financial Domain Mapping

```rust
/// Financial domain configuration
pub struct FinancialDomain {
    /// Variables to analyze
    pub variables: Vec<Variable> = vec![
        Variable { name: "spy_return", unit: "%", range: (-10.0, 10.0) },
        Variable { name: "vix", unit: "index", range: (9.0, 80.0) },
        Variable { name: "yield_10y", unit: "%", range: (0.0, 10.0) },
        Variable { name: "dxy", unit: "index", range: (80.0, 120.0) },
        Variable { name: "copper_gold_ratio", unit: "ratio", range: (0.001, 0.01) },
        Variable { name: "hy_spread", unit: "bps", range: (200.0, 1000.0) },
    ],

    /// Objectives
    pub objectives: Vec<Objective> = vec![
        Objective::Maximize {
            variable: "risk_adjusted_return",
            threshold: 0.0,
            weight: 1.0,
        },
        Objective::Minimize {
            variable: "drawdown",
            threshold: -0.10,  // -10%
            weight: 0.8,
        },
    ],

    /// Domain knowledge
    pub prior_relationships: Vec<PriorRelationship> = vec![
        PriorRelationship {
            cause: "vix",
            effect: "spy_return",
            expected_sign: Negative,
            expected_lag: Duration::from_hours(0),
            confidence: 0.8,
        },
        PriorRelationship {
            cause: "copper_gold_ratio",
            effect: "yield_10y",
            expected_sign: Positive,
            expected_lag: Duration::from_days(5),
            confidence: 0.6,
        },
    ],

    /// Action space
    pub actions: Vec<Action> = vec![
        Action::Alert { name: "regime_change", channels: vec!["email", "log"] },
        Action::Alert { name: "correlation_breakdown", channels: vec!["push"] },
        Action::Log { name: "signal_log" },
    ],
}
```

---

## 8. Implementation Roadmap

### Phase 1: MVP Foundation (Weeks 1-4)

**Deliverables:**
- [ ] Working memory implementation (sensor buffers)
- [ ] Simple pattern store (SQLite)
- [ ] Granger causality testing
- [ ] Basic confidence tracking
- [ ] Threshold alert engine
- [ ] Nightly discovery cron

**Exit Criteria:**
- System discovers at least 3 statistically significant correlations
- Predictions logged with outcomes tracked
- Confidence scores update based on accuracy

### Phase 2: Enhanced Discovery (Weeks 5-8)

**Deliverables:**
- [ ] Transfer entropy implementation
- [ ] Mutual information analysis
- [ ] ADWIN drift detection
- [ ] Discovery trigger system
- [ ] Warm memory (AgentDB integration)

**Exit Criteria:**
- Non-linear correlations detected
- Discovery triggers on regime change
- Semantic search for similar patterns working

### Phase 3: Causal Testing (Weeks 9-12)

**Deliverables:**
- [ ] Hypothesis queue implementation
- [ ] Natural experiment detector
- [ ] Shadow intervention framework
- [ ] Confidence interval calculation
- [ ] Causal graph storage (TimescaleDB)

**Exit Criteria:**
- At least 10 hypotheses tested
- Verified causal edges stored with evidence
- False positives identified and removed

### Phase 4: Intelligent Actions (Weeks 13-16)

**Deliverables:**
- [ ] Objective hierarchy definition
- [ ] Thompson Sampling action selection
- [ ] Multi-objective trade-offs
- [ ] Action planning (simple)
- [ ] Feedback loop completion

**Exit Criteria:**
- System selects actions based on objectives
- Exploration/exploitation balance observable
- Full OBSERVE->LEARN loop completing autonomously

### Phase 5: Production Hardening (Weeks 17-20)

**Deliverables:**
- [ ] Graceful degradation implementation
- [ ] Resource monitoring and budgets
- [ ] Cross-domain configuration
- [ ] Performance benchmarks
- [ ] Documentation and testing

**Exit Criteria:**
- System survives resource pressure tests
- Degradation levels activate correctly
- Second domain (financial) configured and running

---

## 9. References

### Research Foundation
- [Correlation Discovery Techniques](/workspaces/neural-data-platform/product/research/gold/financial-intelligence/correlation-discovery/TECHNIQUES.md)
- [Self-Learning Adaptive Systems](/workspaces/neural-data-platform/product/research/gold/self-learning/ADAPTIVE-SYSTEMS.md)
- [RuVector Deep Dive](/workspaces/neural-data-platform/product/research/gold/ruvector-analysis/RUVECTOR-DEEP-DIVE.md)
- [Edge Unsupervised Learning](/workspaces/neural-data-platform/product/research/gold/unsupervised-learning/EDGE-UNSUPERVISED.md)
- [Memory Intelligence Architecture](/workspaces/neural-data-platform/product/research/memory-intelligence-architecture.md)

### Platform Architecture
- [NDP Platform Overview](/workspaces/neural-data-platform/docs/architecture/PLATFORM_ARCHITECTURE_OVERVIEW.md)
- [Gold Layer Master Synthesis](/workspaces/neural-data-platform/product/research/gold/MASTER-SYNTHESIS.md)
- [Art of Possible Vision](/workspaces/neural-data-platform/product/research/gold/art-of-possible/VISION.md)

### Technologies
- AgentDB: Vector storage with RL algorithms
- RuVector/rvLite: HNSW indexing, pattern memory
- TimescaleDB: Time-series storage, continuous aggregates
- ONNX Runtime: Edge ML inference
- linfa/smartcore: Rust ML libraries

---

## Document Control

| Field | Value |
|-------|-------|
| **Location** | `/workspaces/neural-data-platform/product/research/gold/autonomous-edge/integration-pattern/UNIFIED-ARCHITECTURE.md` |
| **Created** | 2026-02-02 |
| **Last Updated** | 2026-02-02 |
| **Status** | Complete |
| **Author** | Research Agent |
| **Next Review** | 2026-03-02 |
| **Stakeholders** | NDP Architecture Team, ML Engineering |
