# Edge AI Frameworks Research

**Date**: 2026-01-03
**Research Type**: Edge Deployment Analysis
**Relevance to NDP**: Critical - Constraints define what's possible

---

## 1. Edge Computing Context for NDP

### Hardware Constraints

| Resource | Raspberry Pi 5 | NDP Current Usage | Available |
|----------|----------------|-------------------|-----------|
| RAM | 16GB | ~750MB | **~15GB** |
| CPU | Cortex-A76 (4 cores @ 2.4GHz) | ~10-20% | **80%+** |
| Storage | 256GB+ NVMe | ~5GB data | **250GB+** |
| GPU | VideoCore VII | Unused | Available |
| Network | 1Gbps Ethernet | ~1Mbps ingest | **999Mbps** |

### Key Insight
**NDP has substantial headroom for agentic capabilities** - only using ~5% of available RAM.

---

## 2. Agentic Framework for Edge AI (PRO-VE 2025)

**Source**: [Arxiv PRO-VE 2025](https://arxiv.org/abs/2510.25813)

### Key Concepts

```
┌─────────────────────────────────────────────────────────────────┐
│              INDUSTRY 5.0 AGENTIC FRAMEWORK                      │
│                                                                  │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐          │
│  │   Human      │  │ Computational│  │ Collaborative│          │
│  │   Agent      │  │    Agent     │  │    Agent     │          │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘          │
│         └────────────────┬────────────────┘                    │
│                          ▼                                      │
│              ┌───────────────────────┐                         │
│              │   Modular Components  │                         │
│              │   - Inference Engine  │                         │
│              │   - Sensor Interface  │                         │
│              │   - Decision Logic    │                         │
│              └───────────────────────┘                         │
│                          │                                      │
│                          ▼                                      │
│              ┌───────────────────────┐                         │
│              │   Edge Device         │                         │
│              │   (ESP32, Pi, Jetson) │                         │
│              └───────────────────────┘                         │
└─────────────────────────────────────────────────────────────────┘
```

### Relevance to NDP

| Framework Feature | NDP Application |
|-------------------|-----------------|
| Modular agent design | Data exploration agents |
| Local inference | On-Pi query understanding |
| Human-in-the-loop | Dashboard interaction |
| Real-time processing | Stream analysis |

---

## 3. Raspberry Pi AI Agent Host

**Source**: [GitHub quantiota/Raspberry-Pi-AI-Agent-Host](https://github.com/quantiota/Raspberry-Pi-AI-Agent-Host)

### Architecture Pattern

```
┌─────────────────────────────────────────────────────────────────┐
│                    HYBRID PI + CLOUD PATTERN                     │
│                                                                  │
│  ┌─────────────────────┐        ┌─────────────────────┐        │
│  │   RASPBERRY PI      │        │   REMOTE SERVER     │        │
│  │   (Edge Node)       │        │   (GPU Compute)     │        │
│  │                     │        │                      │        │
│  │  ┌───────────────┐ │  SSH   │  ┌───────────────┐  │        │
│  │  │  QuestDB      │ │ Tunnel │  │  JupyterHub   │  │        │
│  │  │  (timeseries) │ ├───────►│  │  (AI/ML)      │  │        │
│  │  └───────────────┘ │        │  └───────────────┘  │        │
│  │                     │        │                      │        │
│  │  ┌───────────────┐ │        │  ┌───────────────┐  │        │
│  │  │  Grafana      │ │◄───────┤  │  LLM APIs     │  │        │
│  │  │  (viz)        │ │ Query  │  │  (inference)  │  │        │
│  │  └───────────────┘ │        │  └───────────────┘  │        │
│  │                     │        │                      │        │
│  │  ┌───────────────┐ │        │                      │        │
│  │  │  Sensor Data  │ │        │                      │        │
│  │  │  Collection   │ │        │                      │        │
│  │  └───────────────┘ │        │                      │        │
│  └─────────────────────┘        └─────────────────────┘        │
└─────────────────────────────────────────────────────────────────┘
```

### Key Insights

1. **DietPi optimization**: Lightweight OS maximizes available resources
2. **Module-based**: Add/remove components based on requirements
3. **Tunnel pattern**: Heavy compute offloaded to cloud/server

### NDP Application

This pattern perfectly fits NDP's architecture:
- **Pi** runs data ingestion, storage, visualization (already implemented)
- **Dev container** runs AI agents via API calls (proposed)
- **Tunnel** connects dev to production data

---

## 4. Tiny AI Models for Raspberry Pi

**Source**: [KDnuggets: 7 Tiny AI Models](https://www.kdnuggets.com/7-tiny-ai-models-for-raspberry-pi)

### Viable Models for Pi 5

| Model | Size | RAM Needed | Capability | NDP Use Case |
|-------|------|------------|------------|--------------|
| **EXAONE 4.0 1.2B** | 1.2B params | ~2-3GB | Agentic tool use | Data exploration |
| **Yi 1.5B** | 1.5B params | ~3GB | General reasoning | Query planning |
| **Phi-3 Mini** | 3.8B params | ~4-6GB | Code generation | SQL synthesis |
| **Gemma 2B** | 2B params | ~3-4GB | Multilingual | Logs analysis |
| **TinyLlama** | 1.1B params | ~2GB | Chat | User interaction |

### EXAONE 4.0 1.2B Highlight

```
┌─────────────────────────────────────────────────────────────────┐
│                      EXAONE 4.0 1.2B                            │
│                                                                  │
│  ✅ Designed for on-device deployment                           │
│  ✅ Supports agentic tool use (function calling)               │
│  ✅ Hybrid reasoning: fast mode + deep thinking mode           │
│  ✅ ~2-3GB RAM footprint with quantization                     │
│                                                                  │
│  Perfect for NDP:                                               │
│  - Local query understanding                                    │
│  - Tool orchestration (DuckDB, Grafana)                        │
│  - Runs alongside existing services                             │
└─────────────────────────────────────────────────────────────────┘
```

---

## 5. Generative AI Performance on Edge

**Source**: [Arxiv: GenAI on Edge](https://arxiv.org/html/2411.17712v1)

### Benchmark Results

| Model Size | Pi 5 Throughput | Latency | Viable? |
|------------|-----------------|---------|---------|
| Large (7B+) | <1 tok/s | >10s | No |
| Mid (3-4B) | 2-5 tok/s | 3-8s | Marginal |
| Small (1-2B) | 5-15 tok/s | 1-3s | **Yes** |
| Tiny (<1B) | 15-30 tok/s | <1s | **Yes** |

### Optimization Techniques

```
┌─────────────────────────────────────────────────────────────────┐
│              EDGE OPTIMIZATION STACK                             │
│                                                                  │
│  1. QUANTIZATION                                                │
│     └─► INT8: 4x smaller, 2x faster                             │
│     └─► INT4: 8x smaller, 3x faster                             │
│     └─► GGUF format: optimized for CPU inference                │
│                                                                  │
│  2. SPECULATIVE DECODING                                        │
│     └─► Small model drafts, large model verifies                │
│     └─► 2-3x speedup on constrained hardware                    │
│                                                                  │
│  3. KV CACHE OPTIMIZATION                                       │
│     └─► Flash Attention: 50-75% memory reduction               │
│     └─► Sliding window: bounded memory usage                    │
│                                                                  │
│  4. BATCH PROCESSING                                            │
│     └─► Group queries for efficiency                            │
│     └─► Async inference pipeline                                │
└─────────────────────────────────────────────────────────────────┘
```

---

## 6. Agentic AI at the Edge

**Source**: [Xenonstack: Agentic AI Edge Computing](https://www.xenonstack.com/blog/agentic-ai-edge-computing)

### Key Principles

1. **Autonomy**: Agents make decisions without constant cloud connectivity
2. **Context-Aware**: Use local sensor data for decisions
3. **Continuous Learning**: Adapt to local patterns
4. **Resource Efficient**: Optimize for constrained environments

### Architecture Pattern for NDP

```
┌─────────────────────────────────────────────────────────────────┐
│                  AGENTIC EDGE PATTERN                            │
│                                                                  │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │                    AGENT LAYER                           │   │
│  │                                                          │   │
│  │  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐  │   │
│  │  │ Anomaly  │ │ Query    │ │ Schema   │ │ Alert    │  │   │
│  │  │ Detector │ │ Planner  │ │ Advisor  │ │ Manager  │  │   │
│  │  └────┬─────┘ └────┬─────┘ └────┬─────┘ └────┬─────┘  │   │
│  │       └────────────┴────────────┴────────────┘         │   │
│  │                         │                               │   │
│  │  ┌──────────────────────┼──────────────────────────┐   │   │
│  │  │         Local Inference Engine                  │   │   │
│  │  │      (EXAONE 1.2B / Phi-3 Mini / API)          │   │   │
│  │  └─────────────────────────────────────────────────┘   │   │
│  └─────────────────────────────────────────────────────────┘   │
│                              │                                  │
│                              ▼                                  │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │                    DATA LAYER                            │   │
│  │                                                          │   │
│  │   Bronze (Parquet) → Silver (DuckDB) → Gold (Features)  │   │
│  │                                                          │   │
│  └─────────────────────────────────────────────────────────┘   │
│                              │                                  │
│                              ▼                                  │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │                    SENSOR LAYER                          │   │
│  │                                                          │   │
│  │   AirGradient → MQTT → NDP                              │   │
│  │   Weather API → HTTP → NDP                              │   │
│  └─────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
```

---

## 7. Explainable AI at the Edge

**Source**: [ResearchGate: XAI for Edge Devices](https://www.researchgate.net/publication/393592059_Explainable_AI_in_Edge_Devices_A_Lightweight_Framework_for_Real-Time_Decision_Transparency)

### Three-Layer Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                    XAI FRAMEWORK                                 │
│                                                                  │
│  Layer 3: Visualization                                         │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │  Lightweight rendering of explanations                   │   │
│  │  - Attention heatmaps                                    │   │
│  │  - Feature importance bars                               │   │
│  │  - Decision trees                                        │   │
│  └─────────────────────────────────────────────────────────┘   │
│                              │                                  │
│  Layer 2: XAI Engine                                            │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │  Plug-in explainability modules                          │   │
│  │  - SHAP (simplified)                                     │   │
│  │  - Attention weights                                     │   │
│  │  - Rule extraction                                       │   │
│  └─────────────────────────────────────────────────────────┘   │
│                              │                                  │
│  Layer 1: Inference                                             │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │  Base model execution                                    │   │
│  │  - Quantized models                                      │   │
│  │  - Optimized runtime                                     │   │
│  └─────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
```

### NDP Application

| XAI Feature | NDP Use |
|-------------|---------|
| Feature importance | "Why did PM2.5 spike?" |
| Attention heatmaps | Time-series correlation viz |
| Decision rules | Alert threshold explanations |

---

## 8. Memory Budget Analysis

### Scenario A: API-Based Agents (Minimal Local)

| Component | Memory | Notes |
|-----------|--------|-------|
| Existing NDP | 750MB | Current services |
| smolagents | 100MB | Agent framework |
| DuckDB MCP | 50MB | Protocol bridge |
| rvLite patterns | 200MB | Pattern storage |
| **Total** | **1.1GB** | **7% of 16GB** |

✅ **Highly feasible** - leaves 15GB headroom

### Scenario B: Local Small LLM

| Component | Memory | Notes |
|-----------|--------|-------|
| Existing NDP | 750MB | Current services |
| smolagents | 100MB | Agent framework |
| DuckDB MCP | 50MB | Protocol bridge |
| rvLite patterns | 200MB | Pattern storage |
| EXAONE 1.2B (Q8) | 2.5GB | Local inference |
| **Total** | **3.6GB** | **22.5% of 16GB** |

✅ **Feasible** - leaves 12GB for data operations

### Scenario C: Full Agentic Stack

| Component | Memory | Notes |
|-----------|--------|-------|
| Existing NDP | 750MB | Current services |
| smolagents | 100MB | Agent framework |
| DuckDB MCP | 50MB | Protocol bridge |
| rvLite patterns | 500MB | Extended patterns |
| Phi-3 Mini (Q4) | 4GB | Capable local LLM |
| agentic-flow coordinator | 200MB | Multi-agent |
| **Total** | **5.6GB** | **35% of 16GB** |

⚠️ **Feasible but tight** - monitor carefully

---

## 9. Deployment Strategy

### Recommended Approach: Progressive Enhancement

```
┌─────────────────────────────────────────────────────────────────┐
│                 PROGRESSIVE DEPLOYMENT                           │
│                                                                  │
│  Phase 1: API-First (Week 1)                                    │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │  - smolagents with Claude API                            │   │
│  │  - DuckDB MCP server                                     │   │
│  │  - Basic SQL tools                                       │   │
│  │  Memory: ~1GB additional                                 │   │
│  └─────────────────────────────────────────────────────────┘   │
│                              │                                  │
│                              ▼                                  │
│  Phase 2: Pattern Memory (Week 2)                               │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │  - Add rvLite for SQL pattern storage                   │   │
│  │  - Semantic query retrieval                              │   │
│  │  Memory: ~200MB additional                               │   │
│  └─────────────────────────────────────────────────────────┘   │
│                              │                                  │
│                              ▼                                  │
│  Phase 3: Local Inference (Week 3-4)                            │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │  - Deploy EXAONE 1.2B or TinyLlama                      │   │
│  │  - Hybrid: local for simple, API for complex            │   │
│  │  Memory: ~2-3GB additional                               │   │
│  └─────────────────────────────────────────────────────────┘   │
│                              │                                  │
│                              ▼                                  │
│  Phase 4: Full Agentic (Month 2+)                               │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │  - Multi-agent coordination                              │   │
│  │  - Self-learning pipelines                               │   │
│  │  - Dashboard auto-generation                             │   │
│  │  Memory: Varies by ambition                              │   │
│  └─────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
```

---

## 10. References

- [Agentic Framework for Edge AI (PRO-VE 2025)](https://arxiv.org/abs/2510.25813)
- [Raspberry Pi AI Agent Host](https://github.com/quantiota/Raspberry-Pi-AI-Agent-Host)
- [Generative AI on Edge Performance](https://arxiv.org/html/2411.17712v1)
- [7 Tiny AI Models for Raspberry Pi](https://www.kdnuggets.com/7-tiny-ai-models-for-raspberry-pi)
- [Agentic AI in Edge Computing](https://www.xenonstack.com/blog/agentic-ai-edge-computing)
- [Explainable AI for Edge Devices](https://www.researchgate.net/publication/393592059_Explainable_AI_in_Edge_Devices_A_Lightweight_Framework_for_Real-Time_Decision_Transparency)
- [Edge AI LLMs 2025](https://www.lktechacademy.com/2025/09/edge-ai-llms-laptop-raspberrypi-2025.html)
- [Agentic Edge AI Trends](https://www.trendmicro.com/vinfo/us/security/news/cybercrime-and-digital-threats/agentic-edge-ai-autonomous-intelligence-on-the-edge)

---

*Research conducted as part of Hive Mind Research Swarm*
