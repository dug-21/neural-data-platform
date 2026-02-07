# ops-003 Analysis: Unified CLI Consolidation

> **Date**: 2026-02-06
> **Author**: Research swarm (5 parallel agents)
> **Problem**: Agent confusion from 3 deployment binaries with overlapping concerns

---

## TL;DR

After ops-001/ops-002, we have **3 binaries** (`ndp`, `ndp-validate`, `ndp-gold-ddl`) that agents confuse. The 18-week migration plan (doc 09) is too slow. Instead, use a **facade pattern**: keep existing crate internals intact but route everything through the single `ndp` binary. This solves agent confusion in ~3-4 weeks with minimal test risk.

## Analysis Documents

| Document | Contents |
|----------|----------|
| [CURRENT-STATE.md](CURRENT-STATE.md) | Binary inventory, dependency graph, test distribution |
| [DUPLICATION-AUDIT.md](DUPLICATION-AUDIT.md) | 9 categories of code duplication across crates |
| [MIGRATION-PLAN-CRITIQUE.md](MIGRATION-PLAN-CRITIQUE.md) | What doc 09 got right/wrong, why phases were skipped |
| [RECOMMENDATIONS.md](RECOMMENDATIONS.md) | 3-phase delivery plan with facade pattern |
| [DEPLOY-SH-AUDIT.md](DEPLOY-SH-AUDIT.md) | Every binary dispatch site and flag mapping |
| [RISK-ASSESSMENT.md](RISK-ASSESSMENT.md) | Risk matrix, rollback strategy, edge cases |

## Key Numbers

| Metric | Current | After ops-003 |
|--------|---------|---------------|
| Deployment binaries | 3 | 1 |
| `command -v` sites in deploy.sh | 7 (across 3 binaries) | 7 (single binary) |
| Tests at risk | 0 | 0 (facade, no logic moves) |
| Duplicated DbClient traits | 2 | 1 |
| Duplicated VALID_METRICS lists | 2 | 1 |
| NoOpDbClient copies | 3 | 1 |
| Weeks of work | - | ~3-4 |

## Recommended Next Step

Write SCOPE.md for ops-003 based on the RECOMMENDATIONS.md analysis, then execute via SPARC swarm.
