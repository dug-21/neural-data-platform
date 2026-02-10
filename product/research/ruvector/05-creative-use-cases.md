# RuVector Creative Use Cases for Neural Data Platform

**Research Date:** 2026-02-10
**Revised:** 2026-02-10 (actual Pi 5 specs: 16GB RAM, 1TB NVMe SSD)
**Author:** Research Agent (creative exploration)
**Status:** Complete
**Scope:** Non-obvious, speculative, but technically grounded applications of ruvector on NDP

> **REVISION NOTE (16GB + NVMe):** All 10 creative use cases were originally assessed for a 4GB Pi. With 16GB RAM + 1TB NVMe:
> - All feasibility ratings upgrade to **High** (previously Medium/Medium-Low items are now unconstrained)
> - **Living Causal Knowledge Graph (#7):** Promoted from "Explore Later" to **Phase 1 viable** — GNN module (~320MB) fits easily
> - **Cross-Domain Transfer (#8):** Feasibility upgrades from Medium-Low to **Medium** — hyperbolic embedding module fits
> - **Federated Learning (#4):** No change (bottleneck is multi-Pi fleet, not memory)
> - **Natural Language Query (#5):** all-MiniLM-L6-v2 (~200MB) now fits alongside everything else
> - f32 vectors with no compression simplifies all storage calculations
> - Implementation strategy and priority rankings are unchanged — complexity management, not resources, drives phasing
> - See `00-SYNTHESIS.md` for complete revised recommendations

---

## Context

This document explores creative applications of ruvector that go beyond the planned roadmap (Gold layer feature store, similarity search, pattern memory). Each idea is assessed for technical feasibility on Raspberry Pi 5, integration fit with the existing NDP architecture, and novelty in the edge AI space.

NDP currently ingests: indoor air quality (AirGradient: PM1/2.5/10, CO2, temperature, humidity, TVOC, NOx -- 22+ fields per reading at ~1 minute intervals), outdoor weather (NWS observations and forecasts), outdoor AQI, and home-assistant state events (door/window open/close). The Silver layer in TimescaleDB provides continuous aggregates. Gold layer DDL generation is operational. Forecasting (augurs baseline, ruv-FANN Phase 2) and anomaly detection are planned.

Ruvector capabilities referenced: HNSW index (61us p50 search), GNN path learning, SONA (Micro-LoRA + EWC++), ReasoningBank (K-means++ trajectory clustering), 40 attention mechanisms, hyperbolic embeddings, Cypher graph queries, rvLite (2MB edge), binary vector compression, MCP server, PostgreSQL extension (pgvector compatible).

---

## 1. Sensor Fingerprinting: Holistic Situation Recognition

### Concept

Embed the full state of ALL sensor readings at a given moment as a single high-dimensional vector -- a "fingerprint" of the home's current condition. Over weeks of operation, build a library of labeled fingerprints representing recognized situations (morning routine, cooking, sleeping, party, wildfire smoke event, HVAC failure, window left open overnight). New readings are embedded and compared against the library. The system recognizes "this feels like a Tuesday morning" without explicit rule programming.

### How It Works

```
Sensor snapshot at time T:
  [pm25=8.2, co2=620, temp=22.3, humidity=45, tvoc=85, nox=12,
   outdoor_temp=15, outdoor_aqi=32, wind_speed=8,
   window_living_room=closed, door_front=closed, hour=7, day_of_week=2]

              |
              v
    Embedding function (learned or hand-crafted)
              |
              v
    384-dimensional vector V(T)

              |
              v
    HNSW search: top-5 nearest fingerprints from library
              |
              v
    Results: [(morning_weekday, 0.94), (morning_weekend, 0.72), ...]
              |
              v
    Situation: "morning_weekday" with confidence 0.94
```

The embedding function starts simple (z-score normalization of each field concatenated into a vector) and evolves through SONA's Micro-LoRA to weight dimensions that distinguish situations most effectively. The GNN module tracks which situations frequently transition to which others (morning -> commute_departure -> empty_house), creating a temporal situation graph.

### NDP Integration Point

- **Silver layer** provides the input data (continuous aggregate at 5-minute resolution)
- **Gold layer** stores the fingerprint library as a ruvector collection
- **Intelligence layer** uses situation recognition as an input signal for anomaly detection and action selection
- The PostgreSQL extension variant (pgvector-compatible) could live directly inside TimescaleDB, eliminating the need for a separate service for this use case

### Value Proposition

Situation awareness without explicit programming. The homeowner does not need to write rules for "cooking mode" or "sleeping mode." The system discovers and labels these situations from the data. Over time, it learns YOUR specific patterns -- your cooking produces a distinctive fingerprint different from generic "cooking." This is the foundation for truly personalized intelligence.

### Feasibility on Pi (High)

- 384-dimensional vectors at 5-minute resolution = ~105,000 vectors/year = ~160MB storage
- HNSW search at 61us is negligible overhead
- Embedding computation is simple arithmetic (normalization + concatenation) or a tiny MLP (<1ms)
- Memory footprint: <200MB for a year of fingerprints

### Novelty Score: 4/5

Some smart home systems use simple multi-sensor "scenes" but none use learned vector embeddings with self-improving HNSW search on edge devices. The combination of automatic situation discovery + temporal situation graphs + self-improving search is novel for edge.

### Priority: Experiment Now

This is a foundation for many other ideas in this list. The infrastructure investment (embedding pipeline, fingerprint library, HNSW index) is reusable. Start with z-score normalization as embedding, validate that situations cluster naturally in vector space using the existing AirGradient + NWS + HA state data.

---

## 2. Temporal Attention for Multi-Scale Pattern Learning

### Concept

Different environmental phenomena operate on different timescales. CO2 buildup from occupancy happens in 15-30 minute cycles. Outdoor temperature follows a diurnal 24-hour cycle. Humidity has seasonal patterns spanning months. PM2.5 spikes from cooking happen in 3-5 minute bursts. Current approaches use fixed-window continuous aggregates (1-hour, 1-day). Instead, use ruvector's attention mechanisms to LEARN which timescales matter for which predictions, dynamically.

### How It Works

```
Multi-scale input representation:
  For each metric M at time T, compute features at multiple scales:
    M_1min  = raw value
    M_5min  = rolling mean(5min)
    M_15min = rolling mean(15min)
    M_1hr   = rolling mean(1hr)
    M_6hr   = rolling mean(6hr)
    M_24hr  = rolling mean(24hr)
    M_7day  = rolling mean(7day)

              |
              v
    Attention mechanism selects which scales matter:
      For predicting PM2.5 6 hours ahead:
        attention_weights = softmax([0.1, 0.2, 0.4, 0.7, 0.9, 0.3, 0.1])
                              1min  5min 15min 1hr  6hr  24hr  7day
                                                    ^^^
                              System learns 6hr scale matters most for 6hr forecast

    For predicting CO2 30 minutes ahead:
        attention_weights = softmax([0.3, 0.5, 0.9, 0.6, 0.1, 0.1, 0.1])
                              1min  5min 15min 1hr  6hr  24hr  7day
                                         ^^^^
                              System learns 15min scale matters most
```

Ruvector's attention variants enable this. Use **multi-head attention** so different heads can attend to different time scales simultaneously. Use **sparse attention** to avoid processing all historical data. Use **hyperbolic attention** to capture the hierarchical relationship between timescales (seconds contain minutes contain hours -- this is naturally hierarchical, and hyperbolic space represents hierarchies with less distortion than Euclidean space).

### NDP Integration Point

- **Silver continuous aggregates** already compute multi-resolution statistics (1-hour, 1-day). Extend to finer granularities (5-min, 15-min) or compute on-the-fly from Bronze
- **Gold feature engineering** uses learned attention weights to select which resolution's features to include for each prediction task
- **SONA Micro-LoRA** enables the attention weights to adapt as seasonality shifts (summer vs winter may need different timescale emphasis)

### Value Proposition

Eliminates the manual guesswork of "which time window do I aggregate over?" The system discovers that PM2.5 prediction needs recent high-frequency data (cooking events are sudden) while temperature prediction needs low-frequency trends (weather fronts move slowly). This learned knowledge is stored in ruvector as attention weight vectors, transferable across deployments.

### Feasibility on Pi (Medium)

- Multi-resolution feature computation is straightforward (TimescaleDB continuous aggregates handle most of it)
- Attention computation for a 7-scale x 7-metric input is tiny (49-element matrix, <0.1ms)
- Flash Attention optimization in ruvector (7.47x speedup via NAPI) handles the computation efficiently
- The challenge is training the attention weights -- requires backpropagation through the forecasting loss, which needs ONNX or ruv-FANN integration

### Novelty Score: 5/5

Multi-scale temporal attention on edge devices with self-optimizing weights is genuinely novel. Academic work on temporal attention exists (Temporal Fusion Transformer, Time2Vec) but deploying it as a self-improving system on a Raspberry Pi with learned scale selection is new territory. The use of hyperbolic attention for hierarchical timescale representation is particularly original.

### Priority: Explore Later (Phase 2/3)

Depends on ruv-FANN integration and forecasting pipeline completion. The prerequisite infrastructure (multi-resolution features, ONNX inference) is part of the planned roadmap. When those are ready, adding attention-based scale selection is a natural enhancement.

---

## 3. Experience Replay: Persistent Edge Memory

### Concept

The Pi accumulates months and years of "experience episodes" -- observed situation-action-outcome triples. When a new situation arises, the system searches its experience library for the most similar past situations, retrieves what happened and what actions (if any) were taken, and uses that historical experience to inform its current response. This is analogous to how humans draw on personal experience: "Last time I saw PM2.5 spike like this at 6pm, it was cooking and it resolved in 30 minutes without intervention."

### How It Works

```
Episode structure:
  {
    situation_fingerprint: V(T),        // Sensor fingerprint at detection time
    trigger: "pm25_spike",              // What caught attention
    context: {                          // Surrounding conditions
      outdoor_aqi: 32,
      window_state: "closed",
      hour: 18, day: "tuesday"
    },
    action_taken: "none",               // What was done (or nothing)
    outcome: {                          // What happened afterward
      resolved_in_minutes: 28,
      peak_pm25: 42,
      return_to_baseline: true
    },
    confidence: 0.85,                   // How reliable is this episode
    timestamp: "2026-01-15T18:23:00Z"
  }

New situation detected:
  1. Embed current situation as fingerprint V(now)
  2. Search ReasoningBank for top-10 similar episodes
  3. Weight by recency, confidence, and situation similarity
  4. Aggregate outcomes: "80% of similar episodes resolved in <30min"
  5. Recommend action: "Wait and monitor (high confidence in self-resolution)"
```

The ReasoningBank in ruvector stores these episodes with K-means++ clustering. Over time, episode clusters form "archetypes" -- canonical patterns that represent the system's learned understanding. EWC++ prevents new episodes from overwriting knowledge of rare but important events (like the one wildfire smoke incident from last summer).

### NDP Integration Point

- **Anomaly detection** generates the triggers (statistical and ML-based)
- **Sensor fingerprinting** (Idea #1) provides the situation vectors
- **ReasoningBank** in ruvector stores and clusters the episodes
- **Action framework** (Phase 3 roadmap) queries past episodes before acting
- **MCP server** allows Claude to query "what happened last time conditions looked like this?"

### Value Proposition

This is the core differentiator for edge vs cloud. Cloud services process millions of users' data but none of it is YOUR home. Your Pi has been watching YOUR environment for months. It knows that YOUR kitchen PM2.5 spikes at 6pm on weekdays because that is when YOU cook, that YOUR house takes 28 minutes to clear after cooking because of YOUR ventilation characteristics. This hyper-personalized experience is impossible to replicate in a cloud service that does not have continuous local observation.

### Feasibility on Pi (High)

- Episode storage: ~1KB per episode x ~10 episodes/day x 365 days = ~3.5MB/year (trivial)
- K-means++ clustering on <4,000 episodes is instant
- ReasoningBank lookup: <0.8ms per query
- EWC++ overhead: <50ms per adaptation cycle
- Total memory: <100MB for multi-year experience library

### Novelty Score: 5/5

Experience replay is well-known in reinforcement learning, but applying it as a persistent, self-curating episodic memory on an edge device for home intelligence is novel. The combination of ReasoningBank's trajectory clustering with EWC++ forgetting prevention creates something that genuinely learns from experience over months/years -- essentially, a personal AI assistant with long-term memory grounded in physical sensor observations.

### Priority: Experiment Now

This builds directly on Idea #1 (fingerprinting). Once fingerprints exist, recording episodes is straightforward. The anomaly detection pipeline (planned Phase 1) provides the triggers. Start with manual episode labeling (via MCP tools), then automate.

---

## 4. Federated Learning Seed: Privacy-Preserving Fleet Intelligence

### Concept

Multiple NDP instances (apartment buildings, school classrooms, office floors) each learn local patterns independently. Periodically, they share learned REPRESENTATIONS -- not raw sensor data -- with peers. Specifically, they exchange compressed binary vectors representing their learned situation archetypes and attention weights. An apartment building with 20 units collectively learns 20x faster than a single unit, while preserving complete privacy.

### How It Works

```
Pi-A (Apartment 3A):                    Pi-B (Apartment 3B):
  Learns: "cooking at 6pm              Learns: "cooking at 7pm
    causes PM2.5 spike,                  causes PM2.5 spike,
    resolves in 30min"                   resolves in 45min"
          |                                     |
          v                                     v
  Compress archetype to                 Compress archetype to
  binary vector (256 bits)              binary vector (256 bits)
          |                                     |
          +-----------> Mesh sync <-------------+
                            |
                            v
  Both Pis now know:
    "cooking PM2.5 spikes are common,
     resolution time varies 30-45min,
     here is a composite archetype"

No raw data shared. No sensor readings leave the device.
Only 256-bit vectors representing learned patterns.
```

rvLite (2MB) runs on each Pi. Ruvector's binary vector compression reduces 384-dimension float vectors to compact binary representations that can be compared with Hamming distance. The sync protocol uses a gossip mechanism over the local network.

### NDP Integration Point

- **rvLite** on each Pi stores local patterns and archetypes
- **Binary vector compression** enables efficient sync (256 bits per archetype vs 1,536 bytes uncompressed)
- **GNN module** on a coordinator node (optional) builds a fleet-wide causal graph from shared edge vectors
- **SONA EWC++** on each Pi integrates fleet knowledge without forgetting local patterns
- **Docker compose** in NDP's existing deployment model can orchestrate multi-Pi setups

### Value Proposition

A single Pi observes one environment. Twenty Pis collectively understand an entire building's air quality dynamics, microclimate patterns, and cross-unit effects (someone cooking in 3A affects the hallway, which affects 3B's intake). This knowledge network grows in value with each participant. The privacy guarantee (only vectors, never readings) makes opt-in reasonable. For commercial deployments (office buildings, schools), this is a compelling differentiator: "Deploy NDP on every floor. They teach each other."

### Feasibility on Pi (Medium)

- rvLite: 2MB footprint, well within Pi constraints
- Binary vector sync: 256 bits x 100 archetypes x 20 peers = ~64KB per sync cycle (trivial bandwidth)
- Gossip protocol: simple TCP/UDP, runs on cron or timer
- Challenge: conflict resolution when archetypes from different Pis represent similar but not identical patterns. K-means++ re-clustering can handle this but needs testing
- Challenge: network discovery and trust establishment between Pis

### Novelty Score: 5/5

Federated learning is an active research area, but federated vector-based experience sharing on edge microcontrollers is essentially unstudied. Google's Federated Learning works on phones with model gradient sharing. NDP's approach shares compressed archetypes (experience summaries), not gradients. This is simpler, more bandwidth-efficient, and requires no coordinated training -- each Pi learns independently and shares what it has learned.

### Priority: Park It (Phase 4)

This is a compelling long-term vision but requires the single-Pi experience pipeline to be mature first. The unified architecture roadmap already slots "Federation" as Phase 4 (2027+). The key preparatory step is designing archetype compression and comparison now (during Phase 2) so that federation is possible later.

---

## 5. Natural Language to Sensor Query: Bridging Human Experience and Data

### Concept

Embed sensor states AND natural language descriptions in the same vector space. "Show me times when the air felt stuffy" maps to a vector near historical readings where CO2 was high, humidity was elevated, and ventilation was low. Users interact with their sensor data using subjective human language, and the system translates to objective sensor queries. The MCP server makes this accessible to Claude directly.

### How It Works

```
Step 1: Build the dual embedding space

  Sensor fingerprint at T=1: [co2=1200, humidity=68, temp=24, tvoc=250, ...]
    -> V_sensor = embed_sensors(readings)

  Human label (from user or discovered): "stuffy"
    -> V_text = ruvector.embed("stuffy air, hard to breathe, need to open window")

  Training: Minimize distance between V_sensor and V_text for labeled pairs
  Over time: The embedding space aligns sensor readings with human descriptions

Step 2: Query with natural language

  User (via Claude): "When did the air feel stuffy last week?"

  Claude -> MCP tool -> ruvector:
    V_query = ruvector.embed("stuffy air last week")
    results = ruvector.search(V_query, top_k=20, filter={time: last_week})

  Returns: 5 episodes where CO2 > 1000, humidity > 60, TVOC > 200
    "Tuesday 8pm-10pm (2 hours, during movie night, CO2 peaked at 1400)"
    "Wednesday 6pm-7pm (1 hour, during cooking, resolved after opening window)"

Step 3: User feedback refines the space

  User: "The Tuesday episode was stuffy but the Wednesday one was more like smoky"
  System: Separates "stuffy" and "smoky" clusters in embedding space
  SONA Micro-LoRA: Adjusts embedding weights to better distinguish the two
```

### NDP Integration Point

- **MCP server** (already exists in `core/src/mcp/`) provides the Claude interface
- **ruvector MCP server** handles embedding and search operations
- **Silver layer** provides the historical sensor data for matching
- **ReasoningBank** stores successful query-to-reading mappings for SONA learning
- **Semantic router** classifies whether a query is about current state, historical search, or prediction

### Value Proposition

This bridges the gap between the engineer who thinks in PM2.5 micrograms and the homeowner who thinks in "stuffy" and "fresh." It makes the data platform accessible to non-technical users. More importantly, it creates a feedback loop: user descriptions improve the embedding space, which improves future queries. Over time, the system learns YOUR vocabulary for YOUR environment.

### Feasibility on Pi (Medium)

- Embedding generation: ruvector's built-in all-minilm-l6-v2 model runs on ARM, ~12ms per embedding
- Dual embedding alignment: requires a small amount of labeled data (50-100 pairs) to bootstrap
- Search: standard HNSW, negligible overhead
- The bottleneck is the LLM interaction (Claude via MCP), which requires network connectivity
- On-device alternative: use the embedding space itself for classification without LLM, falling back to nearest labeled examples

### Novelty Score: 4/5

Semantic search over sensor data exists in research papers. The novel aspects are: (a) learning on-device with user feedback via SONA, (b) the MCP integration that makes it conversational, (c) the dual embedding that aligns subjective human descriptions with objective sensor readings on the edge. Industrial IoT has nothing like this for consumer use.

### Priority: Explore Later (Phase 3)

Depends on MCP tooling being mature and a sufficient history of labeled episodes. The LLM integration (Llama-edge or Claude via MCP) is a Phase 3 roadmap item. However, the embedding infrastructure from Idea #1 is a prerequisite, so foundational work begins immediately.

---

## 6. Anomaly as Embedding Distance: Self-Calibrating Novelty Detection

### Concept

Replace statistical anomaly detection (z-score > 3, IQR fences) with embedding-based novelty detection. Normal sensor states cluster tightly in vector space. An anomaly is any reading whose embedding is far from all known clusters. This is fundamentally different from traditional approaches: instead of defining "abnormal" by static thresholds, "abnormal" means "I have never seen anything like this before." The GNN learns what is normal for THIS specific home, and self-calibrates as seasons change via SONA/EWC++.

### How It Works

```
Normal operation (learning phase):
  Every 5 minutes, embed sensor state as V(T)
  Insert V(T) into ruvector HNSW index
  Clusters form naturally: {morning, daytime, evening, cooking, sleeping, ...}

Anomaly detection:
  New reading arrives, embed as V(now)
  Search HNSW: distance to nearest cluster centroid = D
  If D > threshold_adaptive:
    ANOMALY: "This state is unlike anything I have seen before"
  Else:
    NORMAL: "This matches the {evening_summer} cluster"

Self-calibration:
  Summer -> Fall transition:
    New readings gradually diverge from summer clusters
    SONA detects distribution shift (ADWIN)
    EWC++ incorporates fall patterns WITHOUT forgetting summer
    Next summer: summer clusters still accessible, no relearning needed

  Contrast with z-score:
    Z-score on temperature: 15C in summer is anomalous but normal in fall
    Must manually adjust thresholds or use long history windows
    Embedding approach: fall readings form new cluster, summer cluster persists
```

### NDP Integration Point

- **Silver continuous aggregates** provide the input features
- **Sensor fingerprinting** (Idea #1) provides the embedding
- **HNSW index** in ruvector stores the normal-state library
- **ADWIN drift detection** (planned Phase 2) triggers SONA adaptation
- **EWC++** prevents catastrophic forgetting across seasons
- Replaces or augments the planned Isolation Forest anomaly detection

### Value Proposition

Self-calibrating anomaly detection that adapts to seasonal changes, lifestyle changes (new roommate, pet adoption, WFH vs office), and sensor drift without manual threshold tuning. The system defines "normal" as "what I have observed" rather than "what a static rule says." Rare events (wildfire smoke once a year) are preserved by EWC++ and immediately recognized if they recur. This is the most natural application of vector databases to time-series monitoring.

### Feasibility on Pi (High)

- HNSW search: 61us per query -- can run on every 5-minute reading with zero impact
- Storage: same as Idea #1 (~160MB/year)
- EWC++ adaptation: <50ms per cycle, runs asynchronously
- Threshold adaptation: simple running percentile of distances, no ML overhead
- This is simpler than Isolation Forest and may perform better for personalized environments

### Novelty Score: 4/5

Embedding-distance anomaly detection exists in the literature. The novelty is: (a) self-calibrating via SONA/EWC++ on edge devices, (b) seasonal memory preservation, (c) integration with situation fingerprinting so anomalies are contextualized ("this is unusual FOR a Tuesday morning" vs "this is unusual overall").

### Priority: Experiment Now

This is a natural companion to Idea #1 and requires minimal additional infrastructure. Once fingerprints are computed and stored in HNSW, anomaly detection is just a distance threshold on the search results. Can be prototyped in a single afternoon using existing Silver layer data.

---

## 7. Living Causal Knowledge Graph

### Concept

Every discovered correlation, validated causation, and failed prediction is stored as a weighted edge in ruvector's graph layer. Over months, this builds a living knowledge graph of "how this building works." Cooking -> PM2.5 spike (confidence: 0.89, lag: 5min). Window open + high outdoor AQI -> indoor AQI rise (confidence: 0.72, lag: 20min). HVAC cycle -> temperature oscillation (confidence: 0.95, lag: 2min). The graph self-prunes weak edges and strengthens validated ones.

### How It Works

```
Phase 1: Correlation Discovery (automated, nightly)
  Run Granger causality tests on all metric pairs
  Significant correlations -> create graph edges with initial weight 0.3

Phase 2: Edge Strengthening (continuous)
  Every time a predicted effect follows an observed cause:
    edge.weight += learning_rate * (1 - edge.weight)
  Every time a predicted effect does NOT follow:
    edge.weight -= decay_rate * edge.weight

Phase 3: Causal Validation (weekly/monthly)
  PC algorithm validates structure
  Validated edges promoted to weight > 0.7
  Refuted edges demoted below 0.3

Phase 4: Queryable Knowledge (always available)
  ruvector GNN Cypher query:
    "MATCH (cause)-[r:AFFECTS]->(effect)
     WHERE effect.metric = 'pm25'
     AND r.weight > 0.5
     RETURN cause, r.weight, r.avg_lag
     ORDER BY r.weight DESC"

  Result:
    1. cooking_event -> pm25 (weight: 0.89, lag: 5min)
    2. window_closed_with_high_outdoor_aqi -> pm25 (weight: 0.72, lag: 20min)
    3. hvac_off -> pm25 (weight: 0.45, lag: 60min)
```

GNN message passing enriches the graph: if A->B and B->C are strong edges, the system infers A may indirectly affect C and tests this hypothesis. Multi-hop queries reveal causal chains that no single correlation analysis would discover.

### NDP Integration Point

- **Granger causality** (planned Phase 2) feeds correlation discovery
- **PC algorithm** (planned Phase 3) provides causal validation
- **GNN module** in ruvector stores and queries the graph
- **ReasoningBank** stores successful and failed prediction episodes for edge weight updates
- **MCP tools** enable querying: "What causes PM2.5 to spike in this house?"
- **Action framework** queries the graph before taking action: "What are the expected effects of closing this window?"

### Value Proposition

This is the knowledge artifact that makes NDP truly intelligent. After a year of operation, the Pi has a personalized causal model of your environment that answers questions no generic model can: "Why does your bedroom get stuffy at 3am?" (Answer: HVAC cycles off at midnight, CO2 accumulates in sealed room, graph shows HVAC_off -> co2_rise with 3-hour lag in bedroom). This knowledge graph is transferable -- move to a new house, and it bootstraps from fleet patterns (Idea #4) while learning local specifics.

### Feasibility on Pi (Medium)

- Graph storage: ruvector GNN module handles this natively
- Edge count: ~20 metrics x 20 metrics = 400 possible edges, sparse in practice (~50-100 active edges)
- GNN query: 18ms for 2-hop traversal (acceptable for non-real-time queries)
- Granger causality computation: nightly batch, ~60 seconds for all pairs (Pi feasible)
- PC algorithm: weekly batch, ~minutes (Pi feasible with memory management)

### Novelty Score: 5/5

Self-maintaining causal knowledge graphs on edge devices are essentially non-existent. Academic causal discovery focuses on offline analysis of datasets. The concept of a continuously-updated, self-pruning causal graph running on a Pi that gets more accurate over months is genuinely novel. The integration with ruvector's GNN for graph learning (not just storage) adds another layer of novelty.

### Priority: Explore Later (Phase 2/3)

Depends on Granger causality implementation (Phase 2 roadmap). The graph infrastructure can be prototyped earlier using manually defined edges (known physics: cooking -> PM2.5), then automated as correlation discovery comes online. Start with a hardcoded seed graph and prove the query/update cycle works.

---

## 8. Cross-Domain Transfer Learning via Hyperbolic Embeddings

### Concept

NDP will add domains over time: air quality (V1), financial (V2), energy, health. Each domain has a hierarchical structure (domain -> stream -> metric -> feature). Hyperbolic embeddings naturally represent hierarchies with less distortion than Euclidean space. By embedding all domains in shared hyperbolic space, patterns learned in one domain can transfer to new domains. The "weather affects indoor air quality" pattern might transfer to "weather affects energy consumption" because both share the "outdoor conditions affect indoor environment" hyperbolic archetype.

### How It Works

```
Hyperbolic embedding space (Poincare disk):

                    [root]
                   /      \
          [environmental]  [financial]
          /       |          |       \
    [air_quality] [weather] [prices] [indicators]
    /    |    \       |       |
  [pm25] [co2] [temp] [wind] [sp500]

Correlation discovered: wind_speed -> pm25 (outdoor source transport)
Embedded as: edge in hyperbolic space linking weather.wind to air_quality.pm25

When energy domain added:
  energy.heating_demand is placed near air_quality in hyperbolic space
  (both respond to outdoor conditions)
  System hypothesizes: wind_speed -> heating_demand
  Tests hypothesis against data -> confirmed (wind chill effect)
  Transfer successful without explicit programming

The hyperbolic structure ensures:
  - Metrics within the same domain are close (pm25 near co2)
  - Related metrics across domains are connected (temp near heating)
  - The hierarchy is preserved (domain > stream > metric)
  - Distance in hyperbolic space = semantic relatedness
```

### NDP Integration Point

- **Domain adapter layer** defines the hierarchy (already designed)
- **Hyperbolic embeddings** in ruvector represent the cross-domain structure
- **GNN module** discovers cross-domain correlations via graph traversal
- **Stream config** (`config.json` files) provide the metadata for hierarchy construction
- **New domain onboarding** queries the hyperbolic space: "What existing patterns might apply to this new metric?"

### Value Proposition

When you add a new domain (energy monitoring), the system does not start from scratch. It transfers relevant knowledge from existing domains. "Weather affects air quality" transfers to "weather affects heating demand." "Time-of-day affects CO2" transfers to "time-of-day affects energy usage." This dramatically reduces the time to useful intelligence in new domains. Hyperbolic space makes this transfer geometrically natural -- hierarchically related concepts are close by construction.

### Feasibility on Pi (Medium-Low)

- Hyperbolic embedding computation is mathematically straightforward (Poincare ball model)
- ruvector supports hyperbolic embeddings natively
- The challenge is training the cross-domain embedding alignment, which requires data from multiple domains running simultaneously
- Multi-domain data collection (financial + air quality) is planned for Phase 2
- Hyperbolic distance computation is slightly more expensive than Euclidean (exp/log operations) but still <1ms

### Novelty Score: 5/5

Hyperbolic embeddings for cross-domain IoT transfer learning on edge devices is genuinely unexplored territory. Hyperbolic embeddings have been studied for NLP (Poincare embeddings for word hierarchies) and knowledge graphs, but their application to multi-domain sensor data transfer on edge hardware is novel. The NDP domain hierarchy is a natural fit for hyperbolic geometry.

### Priority: Explore Later (Phase 2/3)

Requires multiple active domains to be meaningful. Begin with the hierarchy embedding (domain -> stream -> metric) as an organizational tool during Phase 2 financial domain onboarding. Transfer learning experiments can start once two domains are producing data simultaneously.

---

## 9. Predictive Maintenance of the Platform Itself

### Concept

Embed NDP system metrics (query latency, memory usage, disk I/O, ingestion rate, DQ rejection rate, TimescaleDB chunk size) as vectors, just like sensor data. Learn the "healthy system" fingerprint. Detect when the system is drifting toward degradation (memory pressure building, query latency increasing, disk filling up) BEFORE it fails. The Pi monitors itself using the same intelligence it uses to monitor the environment.

### How It Works

```
System health fingerprint (computed every 5 minutes):
  [query_latency_p95=45ms, memory_used_pct=42, disk_used_pct=31,
   ingestion_rate=60/min, dq_rejection_rate=0.02, chunk_count=24,
   wal_size_bytes=1200, bronze_write_latency=3ms]

              |
              v
    Embed as system health vector V_sys(T)
    Insert into separate ruvector collection "system_health"

Healthy operation:
  V_sys vectors cluster tightly around a "healthy" centroid
  Distance to centroid < 0.3: normal
  Distance 0.3-0.6: warning (something is changing)
  Distance > 0.6: alert (significant deviation)

Degradation detection:
  Over 3 days, memory_used_pct gradually increases: 42, 43, 45, 48, 52...
  V_sys vectors drift steadily away from healthy centroid
  System alerts at 0.4 distance, 48 hours BEFORE OOM would occur
  Recommendation: "Memory usage trending upward. Consider running
    Bronze compaction or increasing swap. Estimated time to 80%: 72 hours."
```

### NDP Integration Point

- **core/src/coordinator/** already collects system metrics internally
- **Grafana dashboards** (pipeline-health.json) already visualize system state
- **Sensor fingerprinting** infrastructure (Idea #1) is reused for system vectors
- **ReasoningBank** stores past degradation episodes and their resolutions
- **MCP tools** expose system health to Claude: "Is the platform healthy?"

### Value Proposition

Meta-intelligence: the platform uses its own intelligence capabilities to monitor itself. This is particularly valuable for unattended edge deployments where the Pi runs for months without human attention. Known issues in NDP's history (BUG-004 OOM from Polars, acceptance test failures from partition mismatches) would have been detected earlier by embedding-distance drift detection. The historical degradation episodes become experience (Idea #3) for predicting future failures.

### Feasibility on Pi (High)

- System metric collection: already exists, near-zero overhead
- Embedding: same technique as sensor fingerprinting
- Storage: ~105,000 vectors/year at 5-minute resolution, <50MB (system vectors have fewer dimensions)
- Detection: HNSW distance check on each cycle, 61us
- This is simpler than environment monitoring because the feature space is smaller and more predictable

### Novelty Score: 3/5

Self-monitoring systems exist (Prometheus alerting, autoscaling). The novelty is using the SAME vector embedding infrastructure for both domain monitoring and self-monitoring, and the experience replay aspect (learning from past degradation episodes). On edge devices specifically, this level of self-monitoring intelligence is uncommon.

### Priority: Experiment Now

Minimal additional infrastructure required. The sensor fingerprinting pipeline from Idea #1 can be directly reused with system metrics instead of sensor readings. Can be prototyped alongside Idea #1 as a second ruvector collection.

---

## 10. Dream Mode: Offline Consolidation

### Concept

When no new sensor data is arriving (sensor disconnected, network down, or simply late at night when readings are stable), the system enters "dream mode." It replays past embeddings, tests hypothetical correlations, strengthens confident patterns, prunes weak ones, and re-clusters the experience library. Like neural consolidation during human sleep, this offline processing turns raw experience into refined knowledge. It uses idle CPU time that would otherwise be wasted.

### How It Works

```
Dream mode activation:
  Trigger: ingestion rate < 10% of normal for > 30 minutes
  OR: system time between 2am-5am AND ingestion stable

Dream activities (priority-ordered, time-budgeted):

  1. REPLAY (10 minutes):
     - Sample 100 random historical episodes
     - Re-embed with current SONA weights
     - Compare to original embeddings
     - If distance > threshold: significant representation shift
     - Update stored embeddings with current understanding

  2. CONSOLIDATE (5 minutes):
     - Re-cluster ReasoningBank episodes with K-means++
     - Merge clusters with <0.1 centroid distance (redundant)
     - Split clusters with high internal variance (heterogeneous)
     - Update archetype summaries

  3. HYPOTHESIZE (10 minutes):
     - Generate candidate correlations from causal graph gaps
     - "PM2.5 and NOx have never been tested for correlation"
     - Run lightweight correlation test on Silver data
     - If significant: create candidate edge in causal graph

  4. PRUNE (5 minutes):
     - Review causal graph edges with weight < 0.2 for > 30 days
     - Remove edges that have not been reinforced
     - Free vector storage for obsolete fingerprints (> 2 years old)

  5. COMPRESS (remaining time):
     - Convert old high-dimensional vectors to binary vectors
     - Archive detailed vectors to disk, keep binary in HNSW
     - Reclaim memory for active operations

Dream mode exits:
  - New data arrives (ingestion rate increases)
  - Any dream activity exceeds CPU budget (50% single core)
  - System resource pressure detected (memory > 70%)
```

### NDP Integration Point

- **Storage layer** (`core/src/storage/`) manages the sleep/wake schedule
- **ReasoningBank** is the primary target of consolidation
- **Causal graph** (Idea #7) is the target of hypothesis generation
- **Bronze compaction** (already exists: `compression_after_days: 7`) is a simpler version of this concept
- **System health** (Idea #9) monitors that dream mode does not degrade system performance

### Value Proposition

Turns idle time into intelligence improvement. The Pi is on 24/7 but new data arrives sporadically. Without dream mode, overnight hours are wasted. With dream mode, the system wakes up smarter each morning: clusters are tighter, weak correlations are pruned, new hypotheses are queued for daytime testing. The biological metaphor is apt -- sleep consolidation is how humans transfer short-term experiences into long-term knowledge. Resource-friendly by design: self-limiting CPU and memory usage.

### Feasibility on Pi (High)

- All dream activities are batch operations on existing data, no new I/O required
- CPU budget limiting ensures the Pi remains responsive to new data
- K-means++ re-clustering on <10,000 episodes: <1 second
- Correlation testing: leverages TimescaleDB (offloaded to DB engine)
- Binary vector compression: ruvector built-in capability
- Total dream cycle: ~30 minutes, runs during natural idle periods

### Novelty Score: 5/5

Offline consolidation inspired by neural sleep processes on edge AI devices is genuinely novel. Some systems have "maintenance windows" but none implement cognitive consolidation (replay, re-cluster, hypothesize, prune) as a deliberate intelligence-improvement strategy. The concept of an edge device that "dreams" to get smarter is compelling and technically grounded.

### Priority: Explore Later (Phase 2)

Depends on experience library (Idea #3) and causal graph (Idea #7) being populated. The infrastructure is simple (a cron-like scheduler with CPU limiting) but the value depends on having enough data and patterns to consolidate. Prototype the scheduler early; add dream activities as the underlying systems mature.

---

## Top 5 Most Promising Ideas

Ranked by **value x feasibility**, considering immediate buildability, infrastructure reuse, and contribution to the NDP autonomous intelligence vision.

### Rank 1: Sensor Fingerprinting (#1)

**Value: 9/10 | Feasibility: 9/10 | Product: 81**

Foundation for nearly every other idea. Simple to implement (z-score normalization as initial embedding). Uses existing Silver data. Creates the vector infrastructure that Ideas #3, #6, #9, and #10 build upon. Start here.

**Immediate next step:** Generate fingerprints from 30 days of existing Silver layer data. Visualize clustering with t-SNE to validate that natural situations emerge. Requires: ruvector deployed as Docker container, Silver layer query access.

### Rank 2: Anomaly as Embedding Distance (#6)

**Value: 8/10 | Feasibility: 9/10 | Product: 72**

Natural extension of fingerprinting. Once sensor states are embedded, anomaly detection is just a distance threshold. Self-calibrating, season-adaptive, and simpler than Isolation Forest. Can replace or augment planned statistical anomaly detection. The EWC++ integration preserves rare event knowledge across seasons.

**Immediate next step:** After fingerprints exist, compute distance to nearest cluster for historical data. Compare detection quality against simple z-score on the same data. Requires: fingerprint library from Rank 1.

### Rank 3: Experience Replay (#3)

**Value: 10/10 | Feasibility: 8/10 | Product: 80**

This is the core differentiator for NDP. Personal experience memory creates an AI assistant that genuinely knows YOUR home. The ReasoningBank in ruvector is purpose-built for this. Negligible storage overhead. The MCP integration makes this queryable by Claude. Technically straightforward once fingerprints and anomaly detection exist.

**Immediate next step:** Define episode schema. Start recording episodes manually (MCP tool that labels current anomaly as "cooking" or "wildfire"). Automate labeling later via situation recognition. Requires: fingerprints (#1), anomaly detection (#6).

### Rank 4: Platform Self-Monitoring (#9)

**Value: 7/10 | Feasibility: 9/10 | Product: 63**

Reuses the same infrastructure as #1 with zero additional architecture. Immediate practical value for unattended edge deployments. Would have caught BUG-004 (Polars OOM) before it caused data loss. Can be built as a second ruvector collection alongside sensor fingerprints.

**Immediate next step:** Define system health vector from existing coordinator metrics. Deploy alongside sensor fingerprinting. Alert thresholds can be calibrated against the known BUG-004 degradation timeline. Requires: ruvector deployed (same instance as #1).

### Rank 5: Dream Mode (#10)

**Value: 8/10 | Feasibility: 7/10 | Product: 56**

Transforms idle time into intelligence improvement. The biological metaphor is technically grounded: replay, consolidation, hypothesis generation, and pruning are all well-understood operations. Depends on other ideas being populated but the scheduler infrastructure is simple. The binary vector compression during dream mode directly addresses long-term storage growth.

**Immediate next step:** Implement a simple sleep-mode scheduler (cron-based, CPU-limited). First dream activity: re-cluster existing fingerprints. Add more activities as the experience library and causal graph grow. Requires: fingerprint library (#1), experience episodes (#3).

---

## Implementation Strategy

```
Phase 1 (Now, with Gold layer work):
  [1] Sensor Fingerprinting    <- FOUNDATION
  [6] Embedding Anomaly        <- BUILDS ON #1
  [9] Self-Monitoring           <- REUSES #1

Phase 2 (With forecasting/correlation work):
  [3] Experience Replay         <- BUILDS ON #1 + #6
  [10] Dream Mode               <- BUILDS ON #1 + #3
  [7] Causal Graph (seed)       <- BUILDS ON Granger causality

Phase 3 (With LLM/action framework):
  [5] Natural Language Query    <- BUILDS ON #1 + #3 + MCP
  [2] Temporal Attention        <- BUILDS ON forecasting + ONNX
  [7] Causal Graph (full)       <- BUILDS ON #7 seed + PC algorithm

Phase 4 (Federation):
  [4] Federated Learning        <- BUILDS ON #1 + #3 + binary compression
  [8] Cross-Domain Transfer     <- BUILDS ON #7 + multiple domains
```

All ideas share the sensor fingerprinting infrastructure (#1), making it the highest-leverage investment. The ruvector deployment (Docker container, HNSW index, ReasoningBank) is a one-time infrastructure cost that enables the entire creative roadmap.

---

## Document Control

| Field | Value |
|-------|-------|
| **Location** | `/workspaces/neural-data-platform/product/research/ruvector/05-creative-use-cases.md` |
| **Created** | 2026-02-10 |
| **Last Updated** | 2026-02-10 |
| **Status** | Complete |
| **Stakeholders** | NDP Architecture Team, Product |
