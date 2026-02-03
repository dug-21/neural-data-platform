# Financial Domain Application: Long-Term Investing Intelligence

> **Created:** 2026-02-03
> **Context:** Applying Edge Intelligence Platform to financial/investing domain
> **Focus:** Regime detection, sector rotation, long-term positioning

---

## The Question

Can the same declarative intelligence framework that learns "window affects CO2" also learn "yield curve inversion precedes defensive positioning"?

**Answer: Yes, with domain-aware model seeding.**

---

## Key Insight: Seeded Model Selection

The tournament doesn't have to be random. For well-studied domains, we know which models work:

### Home Environment (Exploratory)

```
Model Selection: TOURNAMENT (discover what works)

Why: Physical relationships vary by home
- Your thermal mass ≠ my thermal mass
- Your ventilation ≠ my ventilation
- Learn what works for THIS environment
```

### Financial Markets (Established Knowledge)

```
Model Selection: SEEDED (apply known techniques)

Why: Decades of research on what works
- Yield curve → recession: well-documented
- Sentiment extremes → contrarian signals: proven
- Regime detection: HMM is standard approach
- Start with what academia/industry knows
```

---

## Domain-Seeded Architecture

```yaml
domain_adapters:
  home_environment:
    model_selection: tournament
    seed_models: [mlp, tcn, nlinear]  # generic, let tournament decide

  financial_intelligence:
    model_selection: seeded
    relationship_models:
      # Regime detection - research says HMM works
      regime_detection:
        primary: hmm_regime_classifier
        features: [yield_curve, credit_spreads, volatility, momentum]
        reference: "Hamilton 1989, Ang & Bekaert 2002"

      # Yield curve analysis - known leading indicator
      yield_curve_signal:
        primary: threshold_rules  # simple rules work here
        thresholds:
          inverted_3mo: "T10Y3M < 0 for 90 days"
          steep_warning: "T10Y3M > 2.5"
        reference: "Fed research, 87.5% recession accuracy"

      # Sentiment extremes - contrarian indicator
      sentiment_signal:
        primary: percentile_bands
        method: "flag when sentiment > 90th or < 10th percentile"
        reference: "AAII sentiment, CNN Fear/Greed"

      # Correlation discovery - let system learn
      cross_asset_correlations:
        primary: tournament  # here we explore
        seed_models: [granger_var, dcca, wavelet_coherence]
        reason: "known correlations documented, but new ones emerge"
```

---

## Financial Relationships: Known vs Discovered

### Category 1: Well-Established (Use Seeded Models)

These relationships are documented in academic literature:

| Relationship | Model | Why This Model | Reference |
|--------------|-------|----------------|-----------|
| Yield curve → recession | Threshold rules | Simple, proven 87.5% accuracy | Fed research |
| Credit spreads → equity risk | Linear regression | Direct relationship | Gilchrist & Zakrajsek |
| VIX → near-term volatility | Regime-switching | Volatility clusters | CBOE research |
| Sentiment extremes → reversal | Percentile bands | Contrarian proven | AAII 30-year data |
| Earnings surprise → drift | Event study | PEAD persists 60-90 days | Ball & Brown 1968 |

**For these: Skip tournament, apply known model.**

### Category 2: Emerging/Novel (Use Tournament)

These relationships exist but optimal model unclear:

| Relationship | Tournament Candidates | Why Tournament |
|--------------|----------------------|----------------|
| Baltic Dry → equity sectors | Granger, LSTM, TCN | Lag structure unclear |
| Social sentiment → specific stocks | VADER+rules, FinBERT, ensemble | Domain evolving |
| Cross-asset momentum | VAR, neural, simple | Depends on regime |
| Alternative data → alpha | Multiple | New data, unclear signal |

**For these: Run tournament, learn what works.**

### Category 3: Speculative/Discovery (Pure Exploration)

Let the system discover:
- Correlations between your air quality data and healthcare stocks?
- Local weather patterns and energy sector performance?
- Personal spending patterns and consumer sentiment?

**These are unique to having cross-domain data on one device.**

---

## Declarative Financial Configuration

```yaml
# financial-intelligence.manifest.yaml

domain: financial_intelligence
data_sources:
  - source: fred
    series: [T10Y3M, BAMLH0A0HYM2, ICSA, PERMIT, UMCSENT]
    refresh: daily

  - source: alpaca
    symbols: [SPY, QQQ, IWM, TLT, GLD, sector_etfs...]
    refresh: daily

  - source: finnhub
    type: sentiment
    refresh: hourly

objectives:
  regime_awareness:
    goal: "Detect regime changes with >5 day lead time"
    method: seeded
    model: hmm_regime_classifier

  risk_positioning:
    goal: "Alert when risk indicators exceed historical norms"
    thresholds:
      credit_spreads: "> 600 bps"
      vix_term_structure: "inverted > 5 days"
      yield_curve: "inverted > 90 days"

  correlation_discovery:
    goal: "Find leading indicators for portfolio sectors"
    method: tournament
    candidates: [granger, mutual_info, dcca]
    min_lead_time: 5 days

actions:
  - type: alert
    triggers: [regime_change, threshold_breach, new_correlation]

  - type: dashboard
    views: [regime_state, leading_indicators, correlation_matrix]

  # Note: No automatic trading - alerts only
```

---

## Model Seeding by Relationship Type

```yaml
seeded_models:
  # Regime Detection
  regime_classification:
    model: hmm_gaussian_2state
    rationale: |
      Hidden Markov Models are standard for regime detection.
      Hamilton (1989) established this. Ang & Bekaert (2002) validated.
      Two states: risk-on, risk-off. Can extend to 3 (add transition).
    parameters:
      states: 2
      features: [sp500_return, yield_spread, credit_spread, volatility]
      lookback: 252 days

  # Leading Indicator Scoring
  leading_indicators:
    model: composite_threshold
    rationale: |
      Conference Board approach: multiple indicators combined.
      Each indicator contributes to composite score.
      Thresholds from historical recession precedents.
    indicators:
      yield_curve:
        weight: 0.25
        signal: "T10Y3M < 0"
        lead_time: "6-24 months"
      building_permits:
        weight: 0.20
        signal: "YoY change < -20%"
        lead_time: "6-12 months"
      initial_claims:
        weight: 0.15
        signal: "4-week avg > 400k"
        lead_time: "3-12 months"
      credit_spreads:
        weight: 0.20
        signal: "> 600 bps"
        lead_time: "weeks to months"
      sahm_rule:
        weight: 0.20
        signal: ">= 0.50"
        lead_time: "real-time confirmation"

  # Sentiment Analysis
  sentiment_signals:
    model: percentile_contrarian
    rationale: |
      Extreme sentiment is the signal, not direction.
      >90th percentile bullish = contrarian bearish
      <10th percentile bearish = contrarian bullish
    sources:
      - aaii_sentiment: weekly
      - put_call_ratio: daily
      - vix_percentile: daily
    threshold: 90th/10th percentile of 52-week range

  # Correlation Discovery (tournament for this)
  correlation_discovery:
    model: tournament
    rationale: |
      While some correlations are known (copper/gold → rates),
      new correlations emerge. Let system discover.
    candidates:
      - granger_causality  # traditional, interpretable
      - transfer_entropy   # non-linear extension
      - dcca              # detrended cross-correlation
      - wavelet_coherence # time-frequency analysis
    validation: walk_forward_oos
```

---

## Comparison: Air Quality vs Financial

| Aspect | Air Quality | Financial |
|--------|-------------|-----------|
| **Relationship discovery** | Exploratory (your home is unique) | Mixed (some known, some novel) |
| **Model selection** | Tournament (learn what works) | Seeded + tournament |
| **Causal validation** | Physical causation (door→air flow) | Statistical causation (leading indicators) |
| **Action type** | Physical (open window) | Informational (alert, suggest) |
| **Feedback loop** | Direct (did CO2 drop?) | Indirect (was regime call correct?) |
| **Time horizon** | Minutes to hours | Days to months |

### What Transfers

- Declarative objective framework ✅
- Correlation discovery engine ✅
- Threshold-based triggers ✅
- Confidence accumulation ✅
- Graduated autonomy (alert → suggest) ✅

### What's Domain-Specific

- Model seeds (HMM for regime, percentiles for sentiment)
- Validation criteria (walk-forward vs immediate outcome)
- Action types (no physical actuators)
- Time horizons (days not minutes)

---

## Edge Feasibility for Financial Domain

### Data Volume

| Source | Daily Volume | Annual Storage |
|--------|--------------|----------------|
| FRED (50 series) | ~5 KB | ~2 MB |
| Alpaca (100 symbols) | ~100 KB | ~35 MB |
| Finnhub sentiment | ~50 KB | ~18 MB |
| **Total** | ~155 KB/day | ~55 MB/year |

**Trivial for Pi.**

### Compute Requirements

| Task | Frequency | Duration | Memory |
|------|-----------|----------|--------|
| Data ingestion | Daily | 30 sec | 50 MB |
| Feature computation | Daily | 60 sec | 100 MB |
| Regime classification | Daily | 5 sec | 50 MB |
| Correlation scan | Weekly | 5 min | 200 MB |
| Dashboard updates | Hourly | 10 sec | 50 MB |

**Easily within Pi budget.**

### Model Sizes

| Model | Size (INT8) | Inference |
|-------|-------------|-----------|
| HMM regime classifier | 100 KB | 5 ms |
| Sentiment percentiles | 10 KB | 1 ms |
| Composite indicator | 5 KB | 1 ms |
| Granger scanner | 50 KB | 60s for all pairs |

**All fit comfortably.**

---

## The Value Proposition

### What You Get

```
WEEK 1:
  Connect FRED, Alpaca, Finnhub (free APIs)
  Declare: "Alert me on regime changes, threshold breaches"

WEEK 2-4:
  System ingests data, computes features
  HMM trains on historical data
  Dashboard shows current regime state

WEEK 4-8:
  Leading indicator thresholds calibrated
  Sentiment percentiles established
  Correlation discovery runs

ONGOING:
  Daily: Regime state updated, thresholds checked
  Weekly: New correlations explored
  Alert: "Yield curve inverted 90 days, credit spreads widening -
         historical pattern suggests defensive posture"
```

### What's Different from Bloomberg/Refinitiv

| Capability | Bloomberg | This |
|------------|-----------|------|
| Cost | $24,000/year | $0/year |
| Runs offline | No | Yes |
| Learns your patterns | No | Yes |
| Cross-domain (+ air quality) | No | Yes |
| Customizable objectives | Limited | Fully |
| Open source | No | Yes |

### What's Different from DIY Python Scripts

| Capability | DIY Scripts | This |
|------------|-------------|------|
| Setup time | Weeks/months | Hours |
| Maintenance | Ongoing | Automatic |
| Model selection | Manual | Automatic/seeded |
| Correlation discovery | Manual | Automatic |
| Runs reliably | Maybe | Designed for it |

---

## Iterative Model Addition

The platform supports adding domain-specific models over time:

```yaml
# Version 1.0 - Launch models
models:
  regime: hmm_2state
  sentiment: percentile_bands
  indicators: composite_threshold
  discovery: granger_causality

# Version 1.1 - Add proven alternatives
models_added:
  regime: markov_switching_var  # more sophisticated
  sentiment: finbert_classifier  # if Pi can handle
  discovery: transfer_entropy    # non-linear

# Version 1.2 - Community contributions
models_added:
  sector_rotation: momentum_relative_strength
  earnings: post_earnings_drift_model
  volatility: garch_forecast

# Version 2.0 - Cross-domain models
models_added:
  air_quality_health_stocks: discovered_correlation_model
  weather_energy_sector: seasonal_adjustment_model
```

**Each model addition is declarative:**

```yaml
model_registry:
  - name: momentum_relative_strength
    domain: financial
    relationship_type: sector_rotation
    source: community
    validation: backtested_sharpe_0.8
    paper: "Jegadeesh & Titman 1993"
```

---

## Implementation Path

### Phase 1: Financial Data Foundation

- FRED adapter (economic indicators)
- Alpaca adapter (prices)
- Finnhub adapter (sentiment)
- Feature computation pipeline

### Phase 2: Seeded Models

- HMM regime classifier
- Composite leading indicator
- Sentiment percentile bands
- Threshold alerting

### Phase 3: Discovery Layer

- Granger causality scanner
- Correlation dashboard
- Novel relationship flagging

### Phase 4: Cross-Domain

- Air quality → health sector correlations
- Weather → energy sector correlations
- Personal data → market correlations

---

## Conclusion

**Does the declarative framework work for financial intelligence?**

Yes, with domain-aware seeding:

| Component | Approach |
|-----------|----------|
| Data ingestion | Same (adapters for FRED, Alpaca, Finnhub) |
| Feature computation | Same (continuous aggregates) |
| Correlation discovery | Same (Granger, with financial candidates) |
| Causal validation | Modified (longer validation periods) |
| Model selection | **Seeded** (use known models for known relationships) |
| Actions | Same framework, different action types (alerts not actuators) |

**The key insight:** Tournament selection makes sense for novel/personal relationships. Seeded selection makes sense for well-studied domains. Support both.

---

*Financial intelligence as a domain adapter on the Edge Intelligence Platform*
