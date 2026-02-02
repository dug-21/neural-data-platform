# Executive Summary: NDP Gold Layer Recommendations

**Document Version:** 1.0
**Date:** 2026-02-02
**Audience:** Technical Leadership, Product Stakeholders
**Decision Required:** Prioritization and resource allocation for Gold Layer development

---

## 1. Strategic Context

The Neural Data Platform has successfully established Bronze (Parquet + WAL) and Silver (TimescaleDB) layers. The Gold Layer represents the platform's intelligence tier - where raw observations become predictions, insights, and autonomous actions.

**Market Window:** The convergence of capable small language models, efficient edge hardware, and proven agentic patterns creates a unique opportunity to build a self-learning data platform that was technically impossible 24 months ago.

---

## 2. High-Level Recommendations

### Recommendation 1: Prioritize Foundation Over Innovation

**Summary:** Build on proven technology before experimenting with cutting-edge AI.

| Do First | Do Later |
|----------|----------|
| Time-series forecasting (augurs) | Local LLM integration |
| Anomaly detection (statistical + ML) | Agentic ETL orchestration |
| Self-healing pipelines (rules) | On-device model training |
| Semantic data catalog (sqlite-vec) | Hardware acceleration |

**Rationale:** Each "Do First" item has production implementations, known failure modes, and well-documented APIs. The "Do Later" items have higher value but also higher risk and require the foundation to be stable.

### Recommendation 2: Adopt Embedding-First Architecture

**Summary:** Make vector embeddings a core primitive, not an afterthought.

**Action Items:**
1. Extend AgentDB to store schema/column embeddings
2. Enable natural language queries via semantic similarity
3. Use embedding distance for anomaly detection
4. Track data drift via embedding space analysis

**Investment:** 4-6 weeks of development
**Return:** Unified semantic layer for search, anomaly detection, and NL queries

### Recommendation 3: Implement Incremental Intelligence

**Summary:** Add AI capabilities in layers, with clear fallbacks.

```
Layer 1: Statistical baselines (no ML)
    ↓ If insufficient
Layer 2: Classical ML (random forest, XGBoost)
    ↓ If insufficient
Layer 3: Deep learning (LSTM, transformers)
    ↓ If specialized need
Layer 4: Local LLM (Phi-3, Llama-3.2)
```

**Rationale:** 80% of use cases can be solved with Layers 1-2. Only escalate to more complex (and resource-intensive) approaches when simpler methods fail.

### Recommendation 4: Hardware Acceleration is Optional, Not Required

**Summary:** Design for CPU-only operation; treat accelerators as performance multipliers.

**Current State:** Raspberry Pi 5 can run:
- LSTM time-series forecasting
- Autoencoder anomaly detection
- Llama-3.2-1B at 5+ tokens/sec

**With Hailo-8L:** 3-5x throughput increase, enabling:
- Real-time multi-model inference
- Continuous anomaly monitoring
- Faster LLM response

**Recommendation:** Make Hailo-8L a Phase 2 addition. Do not block Phase 1 on hardware procurement.

### Recommendation 5: Federated Learning is a Strategic Differentiator

**Summary:** Multiple NDP instances should improve together without sharing raw data.

**Why Now:**
- NVIDIA/Meta partnership (April 2025) validates federated learning on edge
- TinyML + FedAvg protocols are mature
- Privacy regulations increasingly favor on-device processing

**Implementation Path:**
1. Phase 1: Single-instance learning (AgentDB patterns)
2. Phase 2: Model export/import between instances
3. Phase 3: Automated federated learning protocol

---

## 3. Priority Ranking

| Rank | Initiative | Value | Effort | Risk | Phase |
|------|------------|-------|--------|------|-------|
| 1 | Time-Series Forecasting | High | Medium | Low | 1 |
| 2 | Self-Healing Pipelines v1 | High | Medium | Low | 1 |
| 3 | Semantic Data Catalog | Medium | Low | Low | 1 |
| 4 | Statistical Anomaly Detection | High | Low | Low | 1 |
| 5 | ML Anomaly Detection | High | Medium | Medium | 1 |
| 6 | Natural Language Queries | Medium | Medium | Medium | 2 |
| 7 | Agentic ETL v1 | High | High | Medium | 2 |
| 8 | Hardware Acceleration | Medium | Medium | Low | 2 |
| 9 | On-Device Training | High | High | High | 2 |
| 10 | Federated Learning | High | High | Medium | 3 |
| 11 | Multi-Agent Orchestration | Medium | High | High | 3 |
| 12 | Neuromorphic Pilot | Low | Medium | High | 3+ |

### Decision Matrix Explanation

**Value Criteria:**
- Impact on prediction accuracy
- Reduction in manual intervention
- Enable new use cases
- Strategic differentiation

**Effort Criteria:**
- Development time
- Integration complexity
- Testing requirements
- Operational overhead

**Risk Criteria:**
- Technology maturity
- Dependency stability
- Failure mode complexity
- Recovery difficulty

---

## 4. Implementation Phases

### Phase 1: Foundation (Q1-Q2 2026)

**Objective:** Establish core Gold Layer capabilities with proven technology.

**Deliverables:**
| Deliverable | Description | Success Criteria |
|-------------|-------------|------------------|
| Feature Store | TimescaleDB continuous aggregates + metadata | 50+ pre-computed features |
| Forecasting Engine | augurs library integration | <15% MAPE on 24hr forecasts |
| Anomaly Detection | Statistical + autoencoder hybrid | >0.85 F1 score |
| Semantic Catalog | sqlite-vec embeddings for schema | NL query support |
| Self-Healing v1 | Rule-based pipeline recovery | 50% auto-recovery rate |

**Resource Estimate:** 2 engineers, 6 months
**Budget:** Development only (no hardware procurement)

### Phase 2: Intelligence (Q3-Q4 2026)

**Objective:** Add AI-powered capabilities and optional hardware acceleration.

**Deliverables:**
| Deliverable | Description | Success Criteria |
|-------------|-------------|------------------|
| Local LLM | Llama-3.2-1B for NL queries | 70% query accuracy |
| Neural DQ | Autoencoder-based quality rules | 20% fewer false positives |
| Agentic ETL v1 | Single-agent pipeline coordination | 80% auto-recovery |
| Hailo-8L Integration | Hardware inference offload | 3x throughput increase |

**Resource Estimate:** 2-3 engineers, 6 months
**Budget:** $200-500 for Hailo-8L + AI Kit

### Phase 3: Autonomy (2027)

**Objective:** Full autonomous operation and cross-instance learning.

**Deliverables:**
| Deliverable | Description | Success Criteria |
|-------------|-------------|------------------|
| Multi-Agent ETL | Full agentic orchestration | 90% auto-recovery |
| Federated Learning | Cross-instance model improvement | Model quality parity |
| Self-Architecting | LLM-suggested schema evolution | Human approval rate >80% |
| Predictive Alerting | Proactive anomaly prediction | 2hr advance warning |

**Resource Estimate:** 3 engineers, 12 months
**Budget:** Network infrastructure for federation

---

## 5. Risk Assessment

### High Priority Risks

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| **LLM reliability for data tasks** | Medium | High | Fallback to traditional SQL; human-in-loop for critical decisions |
| **Scope creep from "cool tech"** | High | Medium | Strict phase gates; MVP-first approach |
| **Performance on constrained hardware** | Medium | High | Aggressive quantization; tiered model selection |

### Medium Priority Risks

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| **AgentDB pattern quality** | Medium | Medium | Curation process; automated validation |
| **Training data insufficient** | Low | Medium | Synthetic augmentation; transfer learning |
| **Integration complexity** | Medium | Medium | Strong abstraction layers; comprehensive testing |

### Monitoring Required

| Risk | Indicator | Action Trigger |
|------|-----------|----------------|
| Model drift | Accuracy degradation >10% | Retrain or fallback |
| Latency creep | p95 >500ms | Profile and optimize |
| Memory pressure | Usage >80% sustained | Model swapping or pruning |

---

## 6. Resource Requirements

### Personnel

| Role | Phase 1 | Phase 2 | Phase 3 |
|------|---------|---------|---------|
| Rust Developer | 1 FTE | 1 FTE | 1 FTE |
| ML Engineer | 0.5 FTE | 1 FTE | 1.5 FTE |
| Data Engineer | 0.5 FTE | 1 FTE | 0.5 FTE |
| **Total** | **2 FTE** | **3 FTE** | **3 FTE** |

### Hardware

| Item | Phase | Cost | Purpose |
|------|-------|------|---------|
| Raspberry Pi 5 8GB | Existing | $0 | Primary deployment |
| Hailo-8L AI Kit | Phase 2 | ~$70 | Inference acceleration |
| Additional RAM (if available) | Phase 2 | TBD | Larger model support |
| Dev Raspberry Pi | Phase 1 | ~$80 | Development/testing |

### Software/Services

| Item | Phase | Cost | Purpose |
|------|-------|------|---------|
| Hugging Face Hub | Ongoing | Free tier | Model downloads |
| augurs crate | Phase 1 | Open source | Time-series forecasting |
| llama.cpp | Phase 2 | Open source | LLM inference |
| sqlite-vec | Phase 1 | Open source | Vector search |

**Total Budget Estimate:**
- Phase 1: ~$80 (dev Pi only)
- Phase 2: ~$150-200 (Hailo-8L kit)
- Phase 3: ~$0-500 (depending on federation infrastructure)

---

## 7. Success Metrics

### Phase 1 Exit Criteria

| Metric | Target | Measurement |
|--------|--------|-------------|
| Forecast MAPE | <15% | 24-hour AQI predictions |
| Anomaly F1 | >0.85 | Labeled test dataset |
| Self-healing rate | 50% | Pipeline failures auto-resolved |
| Query latency p95 | <500ms | Feature store queries |

### Phase 2 Exit Criteria

| Metric | Target | Measurement |
|--------|--------|-------------|
| NL query accuracy | 70% | Curated query test set |
| DQ false positive rate | <20% | Manual review sample |
| Auto-recovery rate | 80% | Pipeline failure tracking |
| Inference throughput | 3x baseline | With Hailo-8L vs CPU |

### Phase 3 Exit Criteria

| Metric | Target | Measurement |
|--------|--------|-------------|
| Auto-recovery rate | 90% | All pipeline failures |
| Federated model quality | Parity | Cross-instance validation |
| Proactive alert accuracy | 80% | Predictions vs actuals |
| Manual intervention | -90% | Operations log analysis |

---

## 8. Recommended Next Steps

### Immediate (Next 2 Weeks)

1. **Approve Phase 1 scope and resource allocation**
2. **Set up development environment** with augurs and sqlite-vec
3. **Define feature store schema** based on air quality domain needs
4. **Establish baseline metrics** for current system performance

### Short-Term (Next 4 Weeks)

1. **Implement continuous aggregates** for core features
2. **Prototype augurs forecasting** on existing Silver data
3. **Create AgentDB patterns** for pipeline operations
4. **Document self-healing rules** for known failure modes

### Medium-Term (Next Quarter)

1. **Complete Phase 1 deliverables**
2. **Evaluate Hailo-8L** for Phase 2 acceleration
3. **Prototype local LLM** query interface
4. **Plan federated learning** architecture

---

## 9. Conclusion

The NDP Gold Layer has a clear path to becoming a self-learning, self-healing edge data platform. The key to success is **disciplined execution of proven technology first**, followed by **measured experimentation with emerging AI capabilities**.

**Core Message:** The foundation must be solid before the intelligence can be transformative. Phase 1 establishes that foundation; Phases 2-3 build the differentiated capabilities that will define NDP's long-term value.

**Recommended Decision:** Approve Phase 1 with 2 FTE allocation and $80 hardware budget. Gate Phase 2 on successful Phase 1 exit criteria.

---

## Appendix A: Technology Comparison Matrix

| Capability | Option A | Option B | Recommendation |
|------------|----------|----------|----------------|
| **Forecasting** | augurs (Rust) | statsforecast (Python) | augurs - native Rust, proven |
| **Anomaly Detection** | Autoencoder | Isolation Forest | Hybrid - both have strengths |
| **Vector Search** | sqlite-vec | pgvector | sqlite-vec - AgentDB alignment |
| **Local LLM** | Llama-3.2-1B | Phi-3-mini | Llama-3.2 - better edge optimization |
| **Acceleration** | Hailo-8L | Coral USB | Hailo-8L - higher throughput, Pi5 native |

## Appendix B: Related Documentation

- `/product/research/gold/art-of-possible/VISION.md` - Full vision document
- `/product/research/07-technology-selection.md` - Technology evaluation
- `/product/research/06-architecture-recommendation.md` - Architecture patterns
- `/docs/adrs/` - Architecture Decision Records
