# Financial Intelligence for NDP: Master Research Synthesis

> **Research Conducted:** 2026-02-02
> **Swarm:** 8 specialized research agents (mesh topology)
> **Scope:** Long-term investing, regime detection, correlation discovery

---

## Executive Summary

This research investigated adding **financial intelligence capabilities** to NDP for long-term investing - specifically for identifying when to play defense vs offense, discovering non-obvious correlations, and integrating diverse data sources (economic, sentiment, events).

### Key Finding

**NDP can become a "Personal Economic Weather Station"** - a privacy-first, edge-deployed platform that:
- Detects market regimes (bull/bear, risk-on/risk-off)
- Discovers non-obvious correlations between diverse data
- Integrates sentiment, events, and economic indicators
- Runs entirely on Raspberry Pi with free data sources
- Reuses 80%+ of existing NDP infrastructure

---

## Research Documents Produced

| Document | Location | Focus |
|----------|----------|-------|
| Alternative Data | `alternative-data/SOURCES.md` | Free data sources, satellite myth, Baltic Dry |
| Sentiment Analysis | `sentiment-analysis/TECHNIQUES.md` | NLP techniques, contrarian signals, edge deployment |
| Regime Detection | `regime-detection/METHODS.md` | Hybrid HMM approach, defensive positioning |
| Correlation Discovery | `correlation-discovery/TECHNIQUES.md` | Granger causality, spurious avoidance |
| Event Signals | `event-signals/FRAMEWORK.md` | PEAD, event fusion, FinBERT |
| Economic Indicators | `economic-indicators/CATALOG.md` | FRED indicators, critical thresholds |
| Data Architecture | `data-sources/ARCHITECTURE.md` | Schemas, APIs, edge feasibility |
| Vision & Summary | `recommendations/VISION.md` | Platform vision, roadmap |

---

## Consolidated Findings

### 1. Data Sources (All Free)

| Need | Source | API | Cost |
|------|--------|-----|------|
| Stock prices | Alpaca | 10K req/min | $0 |
| Economic indicators | FRED | Unlimited | $0 |
| Sentiment | Finnhub | 60 req/min | $0 |
| News | RSS feeds | Unlimited | $0 |
| Yield curve | FRED/Treasury | Unlimited | $0 |

**Total cost: $0/month** for a 50-symbol portfolio

### 2. Regime Detection (Defense vs Offense)

**Hybrid approach recommended:**
- Simple thresholds (SMA, yield curve, spreads)
- HMM confirmation
- 68-75% out-of-sample accuracy
- 40-55% drawdown reduction

| Regime | Equity | Sectors | Fixed Income |
|--------|--------|---------|--------------|
| Offensive | 70-80% | Growth, Cyclicals | Short duration |
| Transition | 50-60% | Neutral | Barbell |
| Defensive | 40-50% | Defensive, Large Cap | Long duration |

### 3. Leading Indicators

| Indicator | Lead Time | FRED ID | Threshold |
|-----------|-----------|---------|-----------|
| Yield Curve | 6-24 mo | T10Y3M | Inverted >3mo |
| Building Permits | 6-12 mo | PERMIT | Down >20% YoY |
| Credit Spreads | Weeks-mo | BAMLH0A0HYM2 | >600 bps |
| Sahm Rule | Real-time | SAHMREALTIME | >= 0.50 |
| Initial Claims | 3-22 mo | ICSA | >400k 4-wk avg |

### 4. Sentiment Analysis

**Two-tier edge architecture:**
1. **VADER** (fast): <1ms, 58% accuracy, screens everything
2. **DistilFinBERT** (accurate): 100-200ms, 87% accuracy, ambiguous cases only

**Key insight:** Extreme sentiment is the signal (contrarian indicator)

### 5. Correlation Discovery

**Pipeline:**
1. Screen all pairs (Granger causality, mutual information)
2. Filter by economic intuition
3. Validate out-of-sample (walk-forward)
4. Monitor for breakdown
5. Decay when no longer significant

**Famous validated correlations:**
- Baltic Dry Index → Global growth (2-4 mo lead)
- Copper/Gold ratio → Interest rates (r > 0.7)
- High yield spreads → Equity (1-3 mo lead)

### 6. Event Integration

**Post-Earnings Drift persists 60-90 days** - actionable signal

**FinBERT sentiment reduces forecasting error by 32.2%**

**Calendar anomalies have weakened** - don't rely on simple seasonal patterns

---

## Architecture Recommendation

### Financial Domain Adapter

```
┌─────────────────────────────────────────────────────────────┐
│                    NDP Financial Domain                      │
├─────────────────────────────────────────────────────────────┤
│  Bronze Layer                                                │
│  ├── price_observations (Alpaca)                            │
│  ├── economic_indicators (FRED)                             │
│  ├── sentiment_scores (Finnhub, RSS + VADER)                │
│  └── financial_events (SEC EDGAR, calendars)                │
├─────────────────────────────────────────────────────────────┤
│  Silver Layer                                                │
│  ├── daily_returns (log returns, volatility)                │
│  ├── indicator_signals (standardized, z-scores)             │
│  ├── sentiment_aggregates (rolling, percentiles)            │
│  └── event_features (days to/since, surprise history)       │
├─────────────────────────────────────────────────────────────┤
│  Gold Layer                                                  │
│  ├── regime_score (composite 5-indicator)                   │
│  ├── correlation_matrix (rolling, discovered pairs)         │
│  ├── sentiment_extremes (contrarian signals)                │
│  └── leading_indicator_dashboard                            │
└─────────────────────────────────────────────────────────────┘
```

### Shared Infrastructure with Air Quality

| Component | Air Quality | Financial | Shared? |
|-----------|-------------|-----------|---------|
| Bronze storage | Parquet + WAL | Parquet + WAL | Yes |
| Silver storage | TimescaleDB | TimescaleDB | Yes |
| Feature computation | Continuous aggregates | Continuous aggregates | Yes |
| ML inference | ruv-FANN, augurs | ruv-FANN, augurs | Yes |
| Anomaly detection | Isolation Forest | Isolation Forest | Yes |
| Dashboards | Grafana | Grafana | Yes |
| MCP tools | rmcp | rmcp | Yes |

**Infrastructure reuse: 80%+**

---

## Cross-Domain Opportunity

NDP uniquely enables correlation discovery ACROSS domains:

```
Air Quality Domain              Financial Domain
─────────────────               ─────────────────
Local AQI readings       →      Healthcare sector stocks?
Pollution events         →      Insurance company exposure?
Weather patterns         →      Energy sector positioning?
Respiratory health       →      Pharmaceutical demand?
```

**This cross-domain correlation discovery is unique to NDP** - no other platform combines personal environmental data with financial analysis.

---

## Edge Deployment Feasibility

### Resource Budget (Pi 5 16GB)

| Component | Memory | CPU |
|-----------|--------|-----|
| Existing NDP | 2.5GB | 40% |
| Financial data | 200MB | 10% |
| Sentiment (VADER + DistilBERT) | 1GB | 25% |
| Regime detection | 100MB | 5% |
| Correlation analysis | 200MB | 10% |
| **Total** | **4GB** | **90%** |
| **Headroom** | **12GB** | **10%** |

**Verdict: Fully feasible on Pi 5**

### Storage Estimate

| Data Type | Annual Storage |
|-----------|----------------|
| Prices (500 symbols) | 50-100 MB |
| Economic indicators | 10 MB |
| Sentiment | 50 MB |
| Events | 20 MB |
| Features | 100 MB |
| **Total** | **~300 MB/year** |

---

## Implementation Roadmap

### Phase 1: Data Foundation (6 weeks)

| Week | Milestone |
|------|-----------|
| 1-2 | FRED adapter (economic indicators) |
| 3-4 | Alpaca adapter (prices) |
| 5-6 | Bronze/Silver schemas, validation |

**Exit criteria:** Daily data ingestion working

### Phase 2: Core Analytics (8 weeks)

| Week | Milestone |
|------|-----------|
| 1-2 | Regime detection (5-indicator composite) |
| 3-4 | Correlation discovery pipeline |
| 5-6 | Sentiment ingestion (RSS + VADER) |
| 7-8 | Event feature engineering |

**Exit criteria:** Regime score and correlations computed

### Phase 3: Intelligence Layer (8 weeks)

| Week | Milestone |
|------|-----------|
| 1-2 | DistilFinBERT integration |
| 3-4 | MCP tools for Claude |
| 5-6 | Alert system (regime changes, extremes) |
| 7-8 | Grafana dashboards |

**Exit criteria:** End-to-end system with alerts

### Phase 4: Cross-Domain (6 weeks)

| Week | Milestone |
|------|-----------|
| 1-2 | Air quality ↔ financial correlation discovery |
| 3-4 | Cross-domain features |
| 5-6 | Portfolio integration |

**Exit criteria:** Cross-domain insights working

---

## MVP Feature Set

| Feature | Priority | Effort |
|---------|----------|--------|
| Economic regime dashboard | P0 | Medium |
| Risk sentiment indicator | P0 | Medium |
| Correlation monitor | P1 | Medium |
| Regime change alerts | P1 | Low |
| Sentiment extremes alerts | P1 | Low |
| Sector rotation view | P2 | Medium |

---

## Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| API discontinuation | Medium | High | Multiple backup sources |
| Regime model accuracy | Medium | Medium | Conservative thresholds, paper trade first |
| Spurious correlations | High | Medium | Economic intuition filter, walk-forward validation |
| Scope creep | High | Medium | Strict MVP definition |
| Backtest overfitting | Medium | High | Out-of-sample validation, vintage data |

---

## Key Insights from Research

### From Alternative Data
> "Most 'alternative data' is institutional-only ($50K+/year). The valuable signals for retail investors are often free (FRED, yield curve, Baltic Dry) and small - perfect for edge deployment."

### From Sentiment Analysis
> "Extreme sentiment is the most reliable signal. Focus on contrarian indicators, not momentum. DistilBERT achieves 97% of BERT accuracy at 40% smaller - edge deployment is viable."

### From Regime Detection
> "A hybrid approach combining simple threshold rules with HMM confirmation achieves 68-75% accuracy and reduces drawdowns by 40-55%."

### From Correlation Discovery
> "The best non-obvious correlations combine statistical significance with economic intuition. Pure data mining without fundamental reasoning leads to spurious relationships."

### From Event Signals
> "Post-earnings announcement drift persists for 60-90 days. Calendar anomalies have weakened significantly due to algorithmic trading."

### From Economic Indicators
> "Building permits is the single most critical leading indicator according to Moody's ML research. The yield curve has 87.5% accuracy for recession prediction."

---

## Synergy with Autonomous Correlation Discovery Vision

The financial domain validates the **autonomous correlation discovery + causal action** vision:

```
Financial Version                Air Quality Version
─────────────────               ────────────────────
OBSERVE: Ingest FRED,           OBSERVE: Ingest sensors,
prices, sentiment, events       weather, door/window state

DISCOVER: Find correlations     DISCOVER: Find correlations
(yield curve → regime)          (window open → AQI spike)

LEARN: Validate causality       LEARN: Validate causality
(economic intuition filter)     (physics intuition filter)

ACT: Alert on regime change     ACT: Close window when
or position for defense         outdoor AQI exceeds threshold

REFINE: Track if alert          REFINE: Track if action
was correct, adjust model       achieved objective
```

**Same architecture, different domains.**

---

## Recommendation

**Proceed with Phase 1 (6 weeks) as validation:**

1. Build FRED and Alpaca adapters
2. Implement Bronze/Silver schemas
3. Verify data quality
4. Paper trade regime signals

**Go/No-Go criteria for Phase 2:**
- Data ingestion reliable (>99% uptime)
- Regime signals match expectations
- Resource usage within budget

---

## Sources

All detailed research with full citations available in individual documents:
- `alternative-data/SOURCES.md`
- `sentiment-analysis/TECHNIQUES.md`
- `regime-detection/METHODS.md`
- `correlation-discovery/TECHNIQUES.md`
- `event-signals/FRAMEWORK.md`
- `economic-indicators/CATALOG.md`
- `data-sources/ARCHITECTURE.md`
- `recommendations/VISION.md`
- `recommendations/EXECUTIVE-SUMMARY.md`

---

*Research conducted by 8-agent mesh swarm using claude-flow coordination*
