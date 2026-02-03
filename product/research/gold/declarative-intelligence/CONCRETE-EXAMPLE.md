# Declarative Intelligence: Window/AQI Concrete Example

> **Created:** 2026-02-03
> **Context:** Validating declarative Gold layer architecture
> **Scenario:** Door/window sensor correlation with indoor air quality (CO2, humidity, PM2.5)

---

## The Challenge

Build a system that:
1. Starts with **zero knowledge** of relationships between streams
2. **Discovers** that window state affects indoor air quality
3. **Validates** the relationship is causal, not spurious
4. **Predicts** outcomes of actions
5. **Optimizes** across multiple objectives (CO2, humidity, PM2.5)
6. **Acts** when confidence is sufficient

All without LLM reasoning - pure declarative rules and statistical computation.

---

## Initial State

**Streams registered (no relationships declared):**

```yaml
streams:
  window_living_room:
    type: auto  # system will fingerprint
  indoor_co2:
    type: auto
  indoor_humidity:
    type: auto
  indoor_pm25:
    type: auto
  outdoor_pm25:
    type: auto
  outdoor_humidity:
    type: auto
```

**System knows nothing about causation.**

---

## Phase 1: Stream Classification

**Trigger:** New stream registered or weekly refresh

```yaml
classification_rules:
  state_stream:
    when:
      unique_values: "< 5"
      median_dwell_time: "> 10m"
    then:
      tag: state_stream
      enable: transition_tracking

  continuous_stream:
    when:
      unique_values: "> 20"
    then:
      tag: continuous_stream
      enable: response_detection
```

**Result after Day 1:**

| Stream | Classification | Reason |
|--------|---------------|--------|
| window_living_room | state_stream | binary, long dwell times |
| indoor_co2 | continuous_stream | continuous values |
| indoor_humidity | continuous_stream | continuous values |
| indoor_pm25 | continuous_stream | continuous values |
| outdoor_pm25 | continuous_stream | continuous values |
| outdoor_humidity | continuous_stream | continuous values |

---

## Phase 2: Correlation Discovery

**Trigger:** Any state_stream transition

```yaml
discovery:
  correlation_scan:
    on: state_stream_transition
    measure:
      - stream: all_continuous_streams
        window_before: 10m
        window_after: 60m
        metrics:
          - delta_mean      # change from baseline
          - delta_max       # peak change
          - response_lag    # time to peak response
    store_in: correlation_observations
```

**What happens on each transition:**

```
Day 3, 14:32: window_living_room transitions CLOSED→OPEN

System captures for each continuous stream:
┌─────────────────┬────────────┬───────────┬──────────────┐
│ stream          │ delta_mean │ delta_max │ response_lag │
├─────────────────┼────────────┼───────────┼──────────────┤
│ indoor_co2      │ -142 ppm   │ -185 ppm  │ 18 min       │
│ indoor_humidity │ +8%        │ +12%      │ 22 min       │
│ indoor_pm25     │ +6 µg/m³   │ +14 µg/m³ │ 12 min       │
│ outdoor_pm25    │ +0.2 µg/m³ │ +1 µg/m³  │ n/a          │
│ outdoor_humidity│ -0.1%      │ -0.3%     │ n/a          │
└─────────────────┴────────────┴───────────┴──────────────┘
```

After 30 days, 47 window transitions observed → 47 observation rows.

---

## Phase 3: Correlation Aggregation

**Trigger:** Weekly, or after N observations

```yaml
aggregation:
  correlation_summary:
    trigger:
      schedule: weekly
      min_observations: 20
    compute:
      for_each: [state_stream, transition_type, continuous_stream]
      metrics:
        response_rate: "count(abs(delta_mean) > noise_threshold) / count(*)"
        mean_delta: "avg(delta_mean)"
        std_delta: "stddev(delta_mean)"
        mean_lag: "avg(response_lag)"
        consistency: "1 - (std_delta / abs(mean_delta))"
    store_in: correlation_summary
```

**Result after 30 days:**

| state_stream | transition | response_stream | response_rate | mean_delta | mean_lag | consistency |
|--------------|------------|-----------------|---------------|------------|----------|-------------|
| window_living_room | CLOSED→OPEN | indoor_co2 | 0.94 | -138 ppm | 17 min | 0.82 |
| window_living_room | OPEN→CLOSED | indoor_co2 | 0.91 | +156 ppm | 23 min | 0.78 |
| window_living_room | CLOSED→OPEN | indoor_humidity | 0.72 | +7% | 21 min | 0.61 |
| window_living_room | CLOSED→OPEN | indoor_pm25 | 0.68 | varies* | 11 min | 0.34 |
| window_living_room | CLOSED→OPEN | outdoor_pm25 | 0.08 | ~0 | n/a | n/a |

*PM2.5 response varies - sometimes up, sometimes down (depends on outdoor level)*

---

## Phase 4: Correlation Promotion

**Trigger:** Correlation exceeds threshold, sustained

```yaml
promotion:
  correlation_to_candidate:
    when:
      metric: response_rate
      source: correlation_summary
      threshold: "> 0.7"
      consistency: "> 0.6"
      sustained: 4 weeks
    then:
      - create: candidate_relationship
        attributes:
          cause_stream: "{{state_stream}}"
          cause_event: "{{transition}}"
          effect_stream: "{{response_stream}}"
          direction: "sign({{mean_delta}})"
          magnitude: "{{mean_delta}}"
          lag: "{{mean_lag}}"
          status: candidate
      - enable: causal_validation
```

**Result - auto-generated candidates:**

```yaml
candidate_relationships:
  - id: cr_001
    cause: window_living_room.CLOSED→OPEN
    effect: indoor_co2
    direction: negative
    magnitude: -138 ppm
    lag: 17 min
    status: candidate

  - id: cr_002
    cause: window_living_room.OPEN→CLOSED
    effect: indoor_co2
    direction: positive
    magnitude: +156 ppm
    lag: 23 min
    status: candidate

  - id: cr_003
    cause: window_living_room.CLOSED→OPEN
    effect: indoor_humidity
    direction: positive
    magnitude: +7%
    lag: 21 min
    status: candidate

  # PM2.5 failed consistency check (0.34 < 0.6), not promoted
```

---

## Phase 5: Causal Validation (Declarative Approach)

**The hard part:** How do we go from correlation to causation declaratively?

```yaml
causal_validation:
  temporal_precedence:
    description: "Cause must always precede effect"
    for_each: candidate_relationship
    check:
      precedence_rate: |
        SELECT count(*) FILTER (WHERE cause_time < effect_peak_time)
        / count(*)
        FROM observations
        WHERE candidate_id = '{{id}}'
    threshold: "> 0.95"

  counterfactual_baseline:
    description: "Effect should NOT happen without cause"
    for_each: candidate_relationship
    check:
      false_positive_rate: |
        SELECT count(*) FILTER (WHERE no_cause AND effect_occurred)
        / count(*) FILTER (WHERE no_cause)
        FROM hourly_windows
        WHERE effect_stream = '{{effect_stream}}'
    threshold: "< 0.15"

  dose_response:
    description: "Longer cause duration = larger effect"
    for_each: candidate_relationship
    check:
      dose_response_corr: |
        SELECT corr(cause_duration_minutes, abs(effect_magnitude))
        FROM observations
        WHERE candidate_id = '{{id}}'
    threshold: "> 0.5"

  confounding_control:
    description: "Relationship holds across different conditions"
    for_each: candidate_relationship
    check:
      stratified_consistency: |
        SELECT min(response_rate)
        FROM correlation_summary
        WHERE candidate_id = '{{id}}'
        GROUP BY outdoor_condition_bucket
    threshold: "> 0.6"
```

**Promotion trigger:**

```yaml
promotion:
  candidate_to_causal:
    when:
      all_of:
        - temporal_precedence: "> 0.95"
        - counterfactual_baseline: "< 0.15"
        - dose_response: "> 0.5"
        - confounding_control: "> 0.6"
      sustained: 2 weeks
    then:
      - update: candidate_relationship
        set:
          status: causal
      - create: prediction_model
      - alert:
          level: info
          message: "Causal relationship validated: {{cause}} → {{effect}}"
```

**Result after ~60 days:**

```yaml
causal_relationships:
  - id: cr_001
    cause: window_living_room.CLOSED→OPEN
    effect: indoor_co2
    direction: negative
    magnitude: -138 ppm (±22)
    lag: 17 min (±4)
    status: causal
    confidence: 0.89
    validation_scores:
      temporal_precedence: 0.98
      counterfactual_baseline: 0.09
      dose_response: 0.67
      confounding_control: 0.71
```

---

## Phase 5 Alternative: Neural Causal Validation

**The opportunity for neural simplification:**

Instead of four hand-crafted validation checks, train a neural model that learns to distinguish causal from spurious correlations.

```yaml
causal_validation_neural:
  model: causal_discriminator

  input_features:
    # Observation pattern features
    - response_rate
    - consistency
    - mean_lag
    - lag_variance

    # Temporal pattern features
    - precedence_rate
    - bidirectional_correlation  # does effect also "predict" cause?

    # Counterfactual features
    - effect_rate_without_cause
    - effect_rate_with_cause
    - rate_ratio

    # Dose-response features
    - duration_effect_correlation
    - magnitude_consistency

    # Context stability features
    - min_stratified_response
    - max_stratified_response
    - context_variance

  output: causal_probability (0-1)

  training:
    # Bootstrap from known physical relationships
    positive_examples:
      - hvac_on → temperature_change (known causal)
      - light_switch → light_level (known causal)
    negative_examples:
      - rooster_crow → sunrise (correlation, not causal)
      - ice_cream_sales → drowning (confounded by summer)

    # Self-supervised from intervention outcomes
    continuous_learning:
      when: action_taken
      label: did_predicted_effect_occur

  threshold: "> 0.75"
```

**Why neural helps here:**

| Declarative Approach | Neural Approach |
|---------------------|-----------------|
| 4 separate checks with 4 thresholds | Single model, single threshold |
| Hard boundaries (pass/fail) | Soft probability (nuanced) |
| Equal weighting | Learned weighting |
| Manual feature selection | Can discover interaction effects |
| Brittle to edge cases | Generalizes better |

**The neural model learns:**
- Which combinations of features indicate true causation
- How to weight temporal precedence vs dose-response vs confounding
- Edge cases where simple thresholds fail

---

## Phase 6: Prediction Model

**Trigger:** Relationship promoted to causal

```yaml
prediction:
  response_model:
    trigger:
      on: relationship_status_change
      to: causal
    create:
      model_type: linear_response  # simple, interpretable
      features:
        - cause_duration
        - baseline_value
        - outdoor_conditions
      target: effect_magnitude
      retrain_schedule: weekly

  state_forecast:
    for_each: causal_relationship
    compute:
      time_to_threshold: |
        extrapolate(current_value, current_trend, threshold)

      intervention_effect: |
        current_value + predicted_magnitude
```

**What the system can now answer:**

| Question | Answer |
|----------|--------|
| If window stays closed, when will CO2 exceed 1000 ppm? | At current rate (+45 ppm/hr), in 2.3 hours |
| If window opens now, what will CO2 be in 30 minutes? | Current 920 - 138 predicted drop = ~782 ppm |
| If window opens now, what will PM2.5 be? | Uncertain - relationship not validated (system knows its limits) |

---

## Phase 7: Multi-Objective Optimization

**Declare objectives and constraints:**

```yaml
objectives:
  indoor_air_quality:
    targets:
      - metric: indoor_co2
        goal: "< 800 ppm"
        weight: 0.4

      - metric: indoor_humidity
        goal: "40-60%"
        weight: 0.3

      - metric: indoor_pm25
        goal: "< 12 µg/m³"
        weight: 0.3

    constraints:
      - outdoor_pm25: "< 35 µg/m³"
      - outdoor_temp: "10-32°C"
```

**Action scoring:**

```yaml
optimization:
  action_scoring:
    for_each: [open_window, close_window, do_nothing]
    compute:
      predicted_state:
        co2: "current + predicted_delta(action, indoor_co2)"
        humidity: "current + predicted_delta(action, indoor_humidity)"
        pm25: "current + predicted_delta(action, indoor_pm25)"

      score: |
        sum(weight[i] * score_vs_target(predicted[i], target[i]))

      constraint_violations: "count of violated constraints"

    rank_by: "score WHERE constraint_violations = 0"
```

**Example decision:**

```
Current state:
  indoor_co2: 950 ppm      (goal: <800, BAD)
  indoor_humidity: 52%     (goal: 40-60%, OK)
  indoor_pm25: 8 µg/m³     (goal: <12, OK)
  outdoor_pm25: 28 µg/m³   (constraint: <35, OK)

Action scoring:
┌──────────────┬──────────────┬──────────────┬──────────────┬───────┐
│ action       │ pred_co2     │ pred_humid   │ pred_pm25    │ score │
├──────────────┼──────────────┼──────────────┼──────────────┼───────┤
│ open_window  │ 812 ppm (+)  │ 58% (ok)     │ 14 µg/m³ (-) │ 0.72  │
│ do_nothing   │ 995 ppm (--) │ 52% (ok)     │ 8 µg/m³ (++) │ 0.45  │
└──────────────┴──────────────┴──────────────┴──────────────┴───────┘

Winner: open_window (score 0.72)
Tradeoff: CO2 improves significantly, PM2.5 slightly worse but in range
```

---

## Phase 8: Action Execution

```yaml
actions:
  recommendation_mode:
    when:
      relationship_confidence: "< 0.85"
      OR optimization_confidence: "< 0.80"
    then:
      mode: alert_only
      message: "Consider {{best_action}}: {{reasoning}}"

  automation_mode:
    when:
      relationship_confidence: "> 0.85"
      AND optimization_confidence: "> 0.80"
      AND action_success_rate: "> 0.75"
      sustained: 30 days
    then:
      mode: automatic
      execute: "{{best_action}}"
      log: action_outcomes

  safety_constraints:
    always:
      - max_actions_per_hour: 4
      - min_dwell_time: 15m
      - user_override: immediate
      - revert_on_failure: true
```

---

## Full Timeline

| Day | Phase | What Happens |
|-----|-------|--------------|
| 1 | Classification | Streams auto-tagged (state vs continuous) |
| 1-30 | Discovery | Every window transition → capture responses |
| 30 | Aggregation | First correlation summary computed |
| 30-60 | Validation | Causal checks running |
| ~60 | Promotion | window→CO2 promoted to causal |
| 60-90 | Prediction | Response model trained |
| ~90 | Optimization | Multi-objective scoring online |
| 90-120 | Alert mode | System recommends actions |
| ~120+ | Automation | If success rate high, auto-execute |

---

## What's Declarative vs Computed

| Component | Declarative | Computed |
|-----------|-------------|----------|
| Stream registration | ✓ | |
| Classification rules | ✓ | |
| Correlation thresholds | ✓ | |
| Causal validation criteria | ✓ | |
| Objectives & weights | ✓ | |
| Constraints | ✓ | |
| Safety limits | ✓ | |
| Actual correlations | | ✓ |
| Causal relationships | | ✓ |
| Prediction models | | ✓ |
| Optimal actions | | ✓ |

---

## Where Neural Models Simplify

### Current Complexity: Causal Validation

The declarative approach requires:
- 4 separate validation checks
- 4 separate thresholds to tune
- Boolean logic (all must pass)
- No nuance for borderline cases

### Neural Simplification

```
┌─────────────────────────────────────────────────────────────┐
│           CAUSAL VALIDATION: DECLARATIVE vs NEURAL           │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  DECLARATIVE (current)                                       │
│  ─────────────────────                                       │
│  temporal_precedence > 0.95    ─┐                           │
│  counterfactual_rate < 0.15    ─┼─→ ALL must pass → causal  │
│  dose_response > 0.5           ─┤                           │
│  confounding_control > 0.6     ─┘                           │
│                                                              │
│  Problems:                                                   │
│  • What if precedence=0.94, but dose_response=0.9?          │
│  • Hard boundaries miss nuance                               │
│  • 4 thresholds to tune                                      │
│                                                              │
│  NEURAL (proposed)                                           │
│  ─────────────────                                           │
│  [all features] ─→ neural_model ─→ causal_probability       │
│                                                              │
│  Benefits:                                                   │
│  • Single threshold (probability > 0.75)                    │
│  • Learns feature interactions                               │
│  • Soft boundaries, graceful degradation                    │
│  • Self-improves from action outcomes                        │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

### Training the Causal Discriminator

**Bootstrap from physics:**
```yaml
known_causal:
  - hvac_on → temperature_change
  - light_switch → light_level
  - door_open → room_pressure_change
  - humidifier_on → humidity_increase

known_spurious:
  - temperature → ice_cream_sales → drowning  # confounded
  - barometer → storm  # predictive but not causal
  - rooster → sunrise  # correlation only
```

**Self-supervised learning:**
```yaml
continuous_training:
  on_action:
    input: relationship_features
    label: did_predicted_effect_occur

  # If we predicted "open window → CO2 drops" and it did,
  # that's evidence the relationship is truly causal
  # If it didn't, maybe it was spurious
```

### Model Architecture (Edge-Friendly)

```
Input: 12 features (response_rate, consistency, lag, precedence, etc.)
Hidden: 2 layers × 32 neurons
Output: 1 probability

Size: ~5KB
Inference: <1ms on Pi
```

---

## Cross-Domain Applicability

The same declarative structure works for other domains:

### Financial Domain

```yaml
streams:
  yield_curve_slope:
    type: auto  # will classify as continuous
  credit_spreads:
    type: auto
  equity_returns:
    type: auto
  fed_rate_decision:
    type: auto  # will classify as state (discrete events)

# Same discovery/validation/prediction pipeline
# Different objectives:
objectives:
  portfolio_regime:
    targets:
      - metric: drawdown_risk
        goal: "< 10%"
        weight: 0.5
      - metric: opportunity_cost
        goal: "< 5%"
        weight: 0.5
```

### Energy Domain

```yaml
streams:
  solar_panel_output:
    type: auto
  battery_level:
    type: auto
  grid_price:
    type: auto
  hvac_mode:
    type: auto

objectives:
  energy_optimization:
    targets:
      - metric: energy_cost
        goal: minimize
        weight: 0.4
      - metric: comfort
        goal: "temperature 68-72°F"
        weight: 0.4
      - metric: battery_reserve
        goal: "> 20%"
        weight: 0.2
```

---

## Summary

### What Works Declaratively

| Phase | Declarative Viability | Notes |
|-------|----------------------|-------|
| Stream classification | ✅ Excellent | Simple rules, works universally |
| Correlation discovery | ✅ Excellent | Pure statistics, no tuning needed |
| Correlation aggregation | ✅ Excellent | SQL aggregates |
| Causal validation | ⚠️ Complex | 4 checks, 4 thresholds, brittle |
| Prediction | ✅ Good | Linear models work for physics |
| Optimization | ✅ Good | Weighted scoring straightforward |
| Action execution | ✅ Excellent | Simple gating rules |

### Where Neural Models Add Value

| Phase | Neural Benefit |
|-------|---------------|
| Causal validation | Replace 4 checks with 1 learned model |
| Prediction | Non-linear relationships when needed |
| Optimization | Learn weights from outcomes |

### The Hybrid Approach

```
DECLARATIVE (rules)          NEURAL (learned)
────────────────────         ────────────────
Stream classification    →
Correlation discovery    →
Correlation aggregation  →
                             Causal validation (discriminator)
Prediction (linear)      →   Prediction (non-linear backup)
Optimization scoring     →   Weight learning from outcomes
Action execution         →
```

---

## Open Questions

1. **Causal discriminator training:** How much bootstrap data is needed? Can it generalize across domains?

2. **Threshold sensitivity:** How sensitive is the system to the declared thresholds? Should there be auto-tuning?

3. **Multi-step causation:** What if A → B → C? How do we discover causal chains?

4. **Feedback loops:** What if opening the window affects outdoor sensors? How do we handle circular relationships?

5. **Concept drift:** If the physical system changes (new HVAC, different season), how quickly does the system adapt?

---

*Document created to preserve concrete example of declarative intelligence architecture*
