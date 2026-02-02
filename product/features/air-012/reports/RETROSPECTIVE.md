# air-012 Retrospective: What Went Wrong

**Feature:** Home Assistant Integration (Barebones)
**Date:** 2026-01-31
**Assessment:** Worst implementation to date - exposed systemic platform gaps

---

## Executive Summary

air-012 was a "simple" feature: add 3 binary sensors from Home Assistant via MQTT. It should have been straightforward given existing MQTT adapter code. Instead, it exposed **systemic issues** across configuration, automation, documentation, and agent coordination.

The feature is now complete, but the implementation process revealed that NDP lacks the infrastructure for **reproducible stream onboarding**.

---

## Issue Inventory

### Already Documented as Separate Features

| Issue | Feature | Summary |
|-------|---------|---------|
| Dual source of truth | **air-013** | Silver ETL reads YAML, but `list_streams()` reads etcd. If etcd sync fails, Silver ETL silently never runs. |
| Manual DDL required | **dp-015** | YAML config describes Silver schema, but nothing creates the table. Required manual `psql` execution. |

### Newly Identified Issues

| # | Category | Issue | Impact |
|---|----------|-------|--------|
| 1 | Config Validation | Invalid configs accepted silently | Hours debugging why stream wasn't appearing |
| 2 | Config Structure | Unclear componentization | Fields scattered across sections, easy to misconfigure |
| 3 | Onboarding | No documented procedure | Agents had to discover steps by trial and error |
| 4 | Onboarding | Manual data dictionary update | Had to manually edit CSV, no automation |
| 5 | Deployment | Manual Pi resyncs/restarts | Multiple SSH sessions, easy to miss steps |
| 6 | Silent Failures | No error surfacing | Logs showed nothing when Silver ETL didn't start |
| 7 | Agent Coordination | No runbook for stream addition | Swarm agents didn't know the workflow |
| 8 | Agent Coordination | Pattern not recorded beforehand | No AgentDB guidance for "add stream" workflow |

---

## Detailed Analysis

### 1. Config Validation (NEW - needs scope)

**Problem:** ConfigSyncService accepted syntactically valid YAML that was semantically invalid.

**Example:** `fields` section referenced field names that didn't match the MQTT payload structure. No validation caught this until runtime - and even then, the failure was silent.

**What should happen:**
- Schema validation at sync time
- Field name validation against known patterns
- Clear error messages with line numbers

**Potential scope:** `dp-016: Config Validation Framework`

---

### 2. Config Structure / Componentization (NEW - needs discussion)

**Problem:** Stream config YAML has grown complex with overlapping concerns:
- `fields` - Bronze schema
- `sources` - Data acquisition
- `silver_etl` - Silver transformation
- `entity_schemas` - Silver output schema
- Dimension entries (separate CSV)

**Observed confusion:**
- What's the difference between `fields` and `entity_schemas`?
- Why define field types in multiple places?
- Which section controls what?

**Questions:**
- Should config be split into Bronze config + Silver config?
- Should dimension entries be inline in stream config?
- Is the current `extra` section approach for silver_etl correct?

**Potential scope:** `dp-017: Config Architecture Review` or addressed in dp-015

---

### 3. No Documented Onboarding Procedure (NEW - needs docs)

**Problem:** Adding a new stream required discovering steps:

1. Create stream config YAML (where? what fields? what format?)
2. Add to data dictionary (which CSV? what columns?)
3. Create Silver DDL (what indexes? compression? retention?)
4. Sync config to etcd (`./deploy.sh sync`)
5. Deploy to Pi (`./deploy.sh` ... which subcommand?)
6. Restart services (which ones? what order?)
7. Verify data flow (how? what queries?)

**None of this was documented.** Agents pieced it together from code and existing examples.

**Needed:** `docs/procedures/add-new-stream.md` or similar runbook

---

### 4. Manual Data Dictionary Update (NEW - needs scope)

**Problem:** Dimension table entries (`entity_context.csv`) required manual CSV editing.

**Current workflow:**
1. Edit `data/dimensions/entity_context.csv`
2. Run `./deploy.sh sync-dimensions`

**Issues:**
- CSV format is fragile (easy to mess up columns)
- No validation of `correlates_with` references
- No relationship to stream config

**Question:** Should dimension entries be defined in stream config YAML?

**Potential scope:** Part of dp-015 or new `dp-018: Unified Stream Definition`

---

### 5. Manual Pi Resyncs/Restarts (Deployment Friction)

**Problem:** After creating config artifacts, deployment required multiple manual steps across SSH.

**What we had to do:**
```bash
# On dev machine
git push

# SSH to Pi
cd /opt/neural-data-platform
git pull
./deploy.sh sync
docker-compose restart air-quality-app
```

**What we need:** Single command from dev machine, or GitOps-style auto-deploy.

**Potential scope:** `ops-001: Deployment Automation` or address in existing ops features

---

### 6. Silent Failures (Partially addressed by air-013)

**Problem:** When things went wrong, there was no indication:

| Failure Mode | What Happened | What Should Happen |
|--------------|---------------|-------------------|
| etcd sync failed | No stream in `list_streams()` | Error log, deployment failure |
| Silver table missing | SilverSubscriber not created | Error log with table name |
| Config field mismatch | ETL produced nulls | Validation error at sync time |

**Root cause:** Error handling assumes "happy path". No defensive checks for missing prerequisites.

**Addressed by:** air-013 (unified config source), dp-015 (auto table creation)
**Still needed:** Comprehensive startup validation with clear error messages

---

### 7. Agent Coordination: No Runbook

**Problem:** Swarm agents spawned to work on air-012 had no guidance on "how to add a stream to NDP."

**What happened:**
- Agents explored codebase to find patterns
- Made assumptions that turned out wrong
- Created configs that looked right but didn't work
- Multiple iterations of trial-and-error

**What should happen:**
- AgentDB pattern: "add-new-stream-to-ndp"
- Step-by-step checklist for agents
- Validation checkpoints at each step

**Needed:** Record pattern in AgentDB + create procedure doc

---

### 8. Agent Coordination: No Pre-recorded Pattern

**Problem:** AgentDB had no pattern for "adding a stream." The pattern was recorded *after* completion (ID: 95), but should have existed *before*.

**Root cause:** We haven't been proactively recording patterns for common workflows.

**What should happen:**
- Before tackling new stream types, record expected workflow
- `/get-pattern` returns guidance before agents start
- `/reflexion` feedback improves pattern over time

---

## Proposed Action Items

### Immediate Documentation (No New Feature Scope)

| Action | Owner | Location |
|--------|-------|----------|
| Create "Add New Stream" runbook | Documentation | `docs/procedures/add-new-stream.md` |
| Record AgentDB pattern | Agent | `/save-pattern` with stream workflow |
| Update CLAUDE.md with stream guidance | Documentation | Project CLAUDE.md |

### New Feature Scopes Needed

| Feature ID | Title | Priority | Addresses |
|------------|-------|----------|-----------|
| **dp-016** | Config Validation Framework | High | Issue #1 (silent config errors) |
| **dp-017** | Config Architecture Review | Medium | Issue #2 (componentization) |
| **ops-001** | Deployment Automation | Medium | Issue #5 (manual Pi steps) |

### Already Scoped

| Feature ID | Title | Status |
|------------|-------|--------|
| **air-013** | Unified Config Source for Silver ETL | Scoped |
| **dp-015** | Config-Driven Silver Table Creation | Scoped |

---

## Priority Recommendation

**Highest impact, lowest effort first:**

1. **Documentation** - Create runbook (prevents repeat of confusion)
2. **dp-015** - Auto table creation (removes biggest manual step)
3. **air-013** - Unified config (eliminates silent failure mode)
4. **dp-016** - Config validation (catches errors early)
5. **dp-017** - Config architecture (longer-term improvement)
6. **ops-001** - Deployment automation (quality-of-life improvement)

---

## Lessons Learned

1. **"Simple" features expose platform gaps** - air-012 was simple in concept but revealed that our platform doesn't support easy stream onboarding.

2. **Silent failures are the worst failures** - Every failure mode must produce a visible error.

3. **Agents need runbooks** - Before assigning swarm agents to infrastructure tasks, ensure patterns exist.

4. **Config-driven promises must be kept** - If we claim to be config-driven, manual DDL is a broken promise.

5. **Test the onboarding path** - We tested the code but not the "new user/agent" experience of adding a stream.

---

## Appendix: Debugging Timeline

| Day | Event | Issue Discovered |
|-----|-------|------------------|
| Day 1 | Created YAML config | None (looked correct) |
| Day 1 | Ran deploy, checked logs | Bronze working, Silver silent |
| Day 2 | Investigated Silver ETL | Table didn't exist (#dp-015) |
| Day 2 | Created table manually | ETL still silent |
| Day 2 | Found `list_streams()` issue | etcd didn't have stream (#air-013) |
| Day 2 | Debugged ConfigSyncService | Validation error in `fields` (#1) |
| Day 2 | Fixed fields, re-synced | Missing dimension entries (#4) |
| Day 3 | Added dimensions, restarted | Finally working |

**Total debugging time:** ~8 hours across 3 days for what should have been 1-2 hours.

---

*Retrospective authored: 2026-01-31*
