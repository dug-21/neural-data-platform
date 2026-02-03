# Roadmap: Current State to V2.0

> **Created:** 2026-02-03
> **Current State:** Bronze → Silver via config
> **Target:** Cross-domain edge intelligence

---

## The Journey

```
CURRENT     V0.5        V1.0         V1.5         V2.0
───────     ────        ────         ────         ────
Bronze  →   Gold    →   Discovery →  Prediction → Cross-Domain
Silver      Features    Correlation  Actions      Intelligence
Config      Objectives  Causation    Learning     Multi-Domain
```

---

## Version Summary

| Version | Focus | What It Enables |
|---------|-------|-----------------|
| **Current** | Data pipeline | Bronze → Silver working, config-driven |
| **V0.5** | Gold Layer | ML-ready features, stream classification, declared objectives |
| **V1.0** | Discovery | Automatic correlation detection, no manual relationship definition |
| **V1.5** | Intelligence | Causal validation, predictions, model selection, actions |
| **V2.0** | Multi-Domain | Financial adapter, cross-domain correlation discovery |

---

## V0.5: Gold Layer Foundation

**Build:** Feature computation, stream classification, objectives framework

**Outcome:** System computes ML-ready features automatically, knows which streams are state vs continuous, user can declare targets

---

## V1.0: Discovery Engine

**Build:** Transition tracking, response detection, correlation aggregation, candidate promotion

**Outcome:** System discovers "window open correlates with CO2 drop" without being told

---

## V1.5: Prediction & Actions

**Build:** Causal validation, model zoo, tournament/seeded selection, action scoring, outcome tracking

**Outcome:** System predicts outcomes, recommends/executes actions, learns from results

---

## V2.0: Cross-Domain Intelligence

**Build:** Financial domain adapter (FRED, Alpaca, Finnhub), seeded models (HMM regime, indicators), cross-domain scanner

**Outcome:** Same platform handles air quality + financial, discovers correlations across domains

---

## Effort Estimate

| Version | Weeks | Cumulative |
|---------|-------|------------|
| V0.5 | 5 | 5 |
| V1.0 | 6 | 11 |
| V1.5 | 11 | 22 |
| V2.0 | 11 | 33 |

**~8 months to full vision**

---

## Dependencies

```
V0.5 ──→ V1.0 ──→ V1.5
  │                 │
  └──→ V2.0 Financial (can parallel after V0.5 pattern established)
              │
              └──→ V2.0 Cross-Domain (needs both domains)
```

---

## Key Decision Points

| At Version | Decision |
|------------|----------|
| V0.5 | Gold layer architecture (continuous aggregates vs materialized views) |
| V1.0 | Correlation thresholds (tune based on real data) |
| V1.5 | Neural causal validation vs declarative rules |
| V1.5 | Model selection strategy (tournament vs seeded vs hybrid) |
| V2.0 | Additional domains to support |

---

## Success Criteria

| Version | Proof Point |
|---------|-------------|
| V0.5 | Features computed automatically for all streams |
| V1.0 | Window→CO2 discovered without manual configuration |
| V1.5 | Predictions trigger correct actions >80% of time |
| V2.0 | Financial regime detection + air quality on same device |

---

*From data pipeline to edge intelligence in 4 versions*
