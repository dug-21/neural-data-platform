# fe-010: Action Framework

## Vision

Complete the intelligence loop: observe -> embed -> search -> predict -> learn -> **act**. The action framework defines what the system CAN do (open window, send alert, adjust threshold), under what conditions (preconditions, safety limits), and uses Q-Learning to recommend actions with graduated autonomy — from "suggest only" through "act with confirmation" to "act autonomously within safety bounds."

This is the capstone of V1.3. The Pi goes from "telling you what it thinks will happen" to "recommending what to do about it."

## Tracking

- Feature: fe-010
- GitHub Issue: TBD
- Parent roadmap: `product/features/gold-001/FEATURE-ROADMAPv1.2.md` (v13-006, v13-007)
- Predecessors: fe-009 (SONA learning for informed recommendations)
- Version target: v1.3.x

## Deliverables

| ID | Task | Description |
|----|------|-------------|
| AC-01 | Action definitions | Config-driven action schema: id, preconditions, effects, safety limits, autonomy level |
| AC-02 | Action registry | Store available actions per domain in data_dictionary or domain config |
| AC-03 | Precondition evaluator | Check action preconditions against current state before recommending |
| AC-04 | Safety limits | Hard constraints that cannot be overridden (e.g., never close window if CO2 > 1000) |
| AC-05 | Q-Learning advisory | Q-table mapping (state, action) -> expected reward, trained from SONA trajectories |
| AC-06 | Autonomy levels | Per-action autonomy: suggest (log only), confirm (notify + wait), auto (execute within bounds) |
| AC-07 | Action logging | All actions (suggested, confirmed, executed) logged with context for SONA feedback |
| AC-08 | Action outcomes | Track what happened after an action — feeds back into Q-Learning and SONA |

## Constraints

- Initial deployment: ALL actions at "suggest" level only — no autonomous execution without explicit user opt-in
- Safety limits are non-negotiable — Q-Learning cannot override them
- Actions are domain-config-driven, not hardcoded
- Q-Learning tables are small (~6MB per domain per the roadmap memory budget)
- Action execution (for "auto" level) requires integration with Home Assistant or similar — out of scope for fe-010, which delivers the framework and advisory
- Must degrade gracefully without SONA (fe-009) — Q-Learning can train from raw prediction outcomes

## Acceptance Criteria

| Criterion | Target |
|-----------|--------|
| Actions defined in config | Air quality domain has at least 3 actions (e.g., open window, close window, alert) |
| Preconditions checked | Action only recommended when preconditions met |
| Safety limits enforced | Unsafe actions never recommended regardless of Q-values |
| Q-Learning produces recommendations | Given current state, system recommends top action |
| Autonomy levels respected | Suggest-only actions logged but not executed |
| Action outcomes tracked | Feedback loop from action -> outcome -> Q-table update |
| Pi resource budget | <10MB additional memory for Q-tables |

## Out of Scope

- Actual Home Assistant integration / action execution (future)
- MCP query interface for actions (future)
- Cross-domain action coordination (future)
- Sysops domain actions (future major release)

## Release

v1.3.x — Action framework. System recommends what to do. Completes V1.3.
