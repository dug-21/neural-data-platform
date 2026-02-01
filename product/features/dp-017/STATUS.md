# dp-017: Integration Test Harness for Deployment Evolution

## Current Phase
architecture

## Progress
- [x] SCOPE.md created
- [x] SPARC Specification in progress
- [x] SPARC Pseudocode complete
- [x] SPARC Architecture in progress
- [ ] SPARC Refinement complete
- [ ] SPARC Completion complete
- [ ] All tests passing
- [ ] Documentation updated
- [ ] Deployed to production

## SPARC Phase Tracking

| Phase | Status | Deliverable | Location |
|-------|--------|-------------|----------|
| Specification | In Progress | Requirements, acceptance criteria | `specification/SPECIFICATION.md` |
| Pseudocode | **Complete** | Test harness algorithm | `pseudocode/PSEUDOCODE.md` |
| Architecture | In Progress | Component design, ADRs | `architecture/ARCHITECTURE.md` |
| Refinement | Pending | TDD implementation | `refinement/REFINEMENT.md` |
| Completion | Pending | Verification, docs | `completion/COMPLETION.md` |

## Active Work

**Current**: SPARC planning swarm executing - parallel agents working on S/P/A phases
**Next**: Command verification (sync, init-streams, sync-dictionary)
**Blocker**: dp-016 (Config Architecture Review)

---

## Task Progress

| ID | Task | Status | Notes |
|----|------|--------|-------|
| 1 | Update docker-compose.integration.yml | Complete | Services match production |
| 2 | Remove silver-etl-daemon | Complete | Removed obsolete service |
| 3 | Add ndp-mcp-server | Complete | Added to compose |
| 4 | Add grafana | Complete | Added with dashboards profile |
| 5 | Fix mosquitto config | Complete | Removed deprecated option |
| 6 | Fix init script paths | Complete | Points to deploy/pi/init-scripts |
| 7 | Verify infrastructure | Complete | etcd, timescaledb, mosquitto healthy |
| 8 | Test `./deploy.sh sync` | In Progress | SPARC swarm active |
| 9 | Test `./deploy.sh init-streams` | Pending | |
| 10 | Test `./deploy.sh sync-dictionary` | Pending | |
| 11 | Test data flow | Pending | MQTT -> Bronze -> Silver |
| 12 | Audit root compose files | Deferred | Out of scope for initial pass |

---

## Verification Checklist

### Infrastructure (Complete)
- [x] Infrastructure services start cleanly
- [x] etcd healthy and accepts writes
- [x] TimescaleDB schemas initialized (silver, gold, data_dictionary)
- [x] Mosquitto accepts connections (pub/sub tested)
- [x] No silver-etl-daemon references in compose

### Commands (In Progress)
- [ ] `DEPLOY_ENV=integration ./deploy.sh deploy` works
- [ ] `DEPLOY_ENV=integration ./deploy.sh status` works
- [ ] `DEPLOY_ENV=integration ./deploy.sh sync` works
- [ ] `DEPLOY_ENV=integration ./deploy.sh init-streams` works
- [ ] `DEPLOY_ENV=integration ./deploy.sh sync-dictionary` works
- [ ] `DEPLOY_ENV=integration ./deploy.sh stop` works

### Data Flow (Pending)
- [ ] Test data flows MQTT -> Bronze
- [ ] Test data flows Bronze -> Silver

### Test Harness (Pending)
- [ ] `scripts/integration-test.sh` created
- [ ] All tests pass
- [ ] CI integration documented

---

## Bugs

| ID | Status | Summary |
|----|--------|---------|
| - | - | No bugs tracked yet |

---

## Branch
`main` (no feature branch yet - changes are infrastructure alignment)

## Related Documents
- `SCOPE.md` - Feature scope and success criteria
- `SPARC-PLAN.md` - Detailed SPARC planning document

## Dependencies
| Dependency | Status | Notes |
|------------|--------|-------|
| dp-016 | Blocking | Config Architecture Review required |
| docker-compose.integration.yml | Complete | Aligned with production |
| deploy/pi/deploy.sh | Stable | Reference implementation |

---

## Last Updated
2026-02-01 21:15 by ndp-scrum-master
