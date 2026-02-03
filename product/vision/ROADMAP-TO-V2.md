# Roadmap: v1.0 to v2.0

> **Created:** 2026-02-03
> **Current State:** v1.0.0 - Bronze → Silver via declarative config
> **Target:** v2.0 - Cross-domain edge intelligence

---

## The Journey

```
v1.0.0      v1.1        v1.2         v1.3         v2.0
──────      ────        ────         ────         ────
Bronze  →   Gold    →   Discovery →  Prediction → Cross-Domain
Silver      Features    Correlation  Actions      Intelligence
Config      Objectives  Causation    Learning     Multi-Domain
```

---

## Version Summary

| Version | Focus | What It Enables |
|---------|-------|-----------------|
| **v1.0.0** | Data pipeline | Bronze → Silver working, declarative deployment ✅ |
| **v1.1** | Gold Layer | ML-ready features, stream classification, declared objectives |
| **v1.2** | Discovery | Automatic correlation detection, no manual relationship definition |
| **v1.3** | Intelligence | Causal validation, predictions, model selection, actions |
| **v2.0** | Multi-Domain | Financial adapter, cross-domain correlation discovery |

---

## v1.1: Gold Layer Foundation

**Build:** Feature computation, stream classification, objectives framework

**Outcome:** System computes ML-ready features automatically, knows which streams are state vs continuous, user can declare targets

---

## v1.2: Discovery Engine

**Build:** Transition tracking, response detection, correlation aggregation, candidate promotion

**Outcome:** System discovers "window open correlates with CO2 drop" without being told

**This is the proof point.**

---

## v1.3: Prediction & Actions

**Build:** Causal validation, model zoo, tournament/seeded selection, action scoring, outcome tracking

**Outcome:** System predicts outcomes, recommends/executes actions, learns from results

---

## v2.0: Cross-Domain Intelligence

**Build:** Financial domain adapter (FRED, Alpaca, Finnhub), seeded models (HMM regime, indicators), cross-domain scanner

**Outcome:** Same platform handles air quality + financial, discovers correlations across domains

---

## Effort Estimate

| Version | Weeks | Cumulative |
|---------|-------|------------|
| v1.1 | 5 | 5 |
| v1.2 | 6 | 11 |
| v1.3 | 11 | 22 |
| v2.0 | 11 | 33 |

**~8 months to full vision**

---

## Dependencies

```
v1.1 ──→ v1.2 ──→ v1.3
  │                 │
  └──→ v2.0 Financial (can parallel after v1.1 pattern established)
              │
              └──→ v2.0 Cross-Domain (needs both domains)
```

---

## Key Decision Points

| At Version | Decision |
|------------|----------|
| v1.1 | Gold layer architecture (continuous aggregates vs materialized views) |
| v1.2 | Correlation thresholds (tune based on real data) |
| v1.3 | Neural causal validation vs declarative rules |
| v1.3 | Model selection strategy (tournament vs seeded vs hybrid) |
| v2.0 | Additional domains to support |

---

## Success Criteria

| Version | Proof Point |
|---------|-------------|
| v1.1 | Features computed automatically for all streams |
| v1.2 | Window→CO2 discovered without manual configuration |
| v1.3 | Predictions trigger correct actions >80% of time |
| v2.0 | Financial regime detection + air quality on same device |

---

*From data pipeline to edge intelligence in 4 versions*
