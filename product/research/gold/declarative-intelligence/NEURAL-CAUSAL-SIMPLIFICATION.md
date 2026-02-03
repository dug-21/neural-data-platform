# Neural Simplification of Causal Validation

> **Created:** 2026-02-03
> **Context:** Simplifying correlation → causation transition via neural models
> **Goal:** Replace complex declarative validation with learned discrimination

---

## The Problem with Declarative Causal Validation

The declarative approach requires multiple hand-crafted checks:

```yaml
causal_validation:
  - temporal_precedence: "> 0.95"
  - counterfactual_baseline: "< 0.15"
  - dose_response: "> 0.5"
  - confounding_control: "> 0.6"
```

**Issues:**
1. Four thresholds to tune (why 0.95 and not 0.93?)
2. Boolean AND logic (all must pass - no nuance)
3. Equal implicit weighting (is precedence as important as dose-response?)
4. Brittle edge cases (precedence=0.94, dose_response=0.9 fails)
5. Domain-specific tuning needed

---

## The Neural Alternative: Causal Discriminator

**Core idea:** Train a small neural network to output P(causal | features)

```
┌─────────────────────────────────────────────────────────────┐
│                   CAUSAL DISCRIMINATOR                       │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  INPUT FEATURES (computed from observations)                 │
│  ────────────────────────────────────────                   │
│  Temporal:                                                   │
│    • precedence_rate (cause before effect %)                │
│    • reverse_precedence (effect before cause %)             │
│    • lag_consistency (std/mean of lag times)                │
│                                                              │
│  Response:                                                   │
│    • response_rate (% of causes with effects)               │
│    • response_magnitude_cv (coefficient of variation)       │
│    • baseline_effect_rate (effects without causes)          │
│                                                              │
│  Dose-Response:                                              │
│    • duration_magnitude_corr                                │
│    • intensity_magnitude_corr (if cause has intensity)      │
│                                                              │
│  Stability:                                                  │
│    • min_stratified_response (across conditions)            │
│    • temporal_stability (response rate over time)           │
│    • context_independence (does context change relationship)│
│                                                              │
│  NEURAL NETWORK                                              │
│  ──────────────                                              │
│  Input:  12 features                                         │
│  Hidden: 32 → 16 neurons (ReLU)                             │
│  Output: 1 (sigmoid → probability)                          │
│  Size:   ~5KB                                                │
│  Latency: <1ms                                               │
│                                                              │
│  OUTPUT                                                      │
│  ──────                                                      │
│  causal_probability: 0.0 - 1.0                              │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

---

## Training Strategy

### Phase 1: Bootstrap from Known Physics

Every home/environment has relationships we know are causal:

```yaml
known_causal_examples:
  # These are universally true
  - cause: light_switch.off→on
    effect: light_level
    label: causal
    reason: "direct electrical connection"

  - cause: hvac.off→heat
    effect: temperature
    label: causal
    reason: "thermodynamics"

  - cause: door.closed→open
    effect: room_pressure
    label: causal
    reason: "fluid dynamics"

  - cause: faucet.off→on
    effect: water_flow
    label: causal
    reason: "plumbing"

known_spurious_examples:
  # Classic examples of correlation ≠ causation
  - cause: morning_traffic
    effect: coffee_consumption
    label: spurious
    reason: "common cause: time of day"

  - cause: umbrella_sales
    effect: wet_sidewalks
    label: spurious
    reason: "common cause: rain"

  - cause: ice_cream_sales
    effect: drowning_rate
    label: spurious
    reason: "common cause: summer"

  - cause: shoe_size
    effect: reading_ability
    label: spurious
    reason: "common cause: age"
```

### Phase 2: Self-Supervised from Interventions

The key insight: **When we take action based on a relationship, we learn if it's truly causal.**

```yaml
self_supervised_learning:
  trigger: action_executed

  record:
    relationship_id: "{{relationship}}"
    features_at_decision: "{{causal_features}}"
    predicted_effect: "{{expected_magnitude}}"
    actual_effect: "{{observed_magnitude}}"
    prediction_error: "abs(predicted - actual) / predicted"

  label_generation:
    if: prediction_error < 0.3
    then: reinforce_as_causal

    if: prediction_error > 0.7
    then: demote_confidence

  training:
    schedule: weekly
    method: online_gradient_update
    learning_rate: 0.001  # slow, conservative
```

**Example learning sequence:**

```
Day 60: System believes window→CO2 is causal (probability=0.78)
Day 61: Action taken - window opened
        Predicted: CO2 drops 138ppm in 17min
        Actual: CO2 dropped 142ppm in 19min
        Error: 3% magnitude, 12% timing
        Label: CONFIRMED (prediction accurate)

Day 62: Model updated, probability→0.82

Day 90: Action taken - window opened
        Predicted: CO2 drops 138ppm
        Actual: CO2 dropped 45ppm (HVAC was also running)
        Error: 67%
        Label: UNCERTAIN (confounding detected)

Day 91: Model learns: need to check HVAC state as confounder
```

### Phase 3: Transfer Learning Across Domains

Once trained on physical relationships, the model learns general patterns of causation:

```
Physical domain (well understood)     Financial domain (less certain)
─────────────────────────────────     ────────────────────────────────
switch→light (train)            →     yield_curve→returns (apply)
hvac→temperature (train)        →     fed_decision→market (apply)
window→air_quality (train)      →     earnings→stock_price (apply)

The model learns:
• High precedence + high response rate + low baseline = likely causal
• Consistent lag times = mechanistic relationship
• Context independence = robust causation
```

---

## Comparison: Declarative vs Neural

| Aspect | Declarative | Neural |
|--------|-------------|--------|
| **Thresholds** | 4 manual thresholds | 1 (probability > 0.75) |
| **Logic** | Boolean AND (brittle) | Learned weighting (soft) |
| **Edge cases** | Fail on boundaries | Graceful degradation |
| **Tuning** | Manual per domain | Self-tuning from outcomes |
| **Interpretability** | Explicit rules | Feature importance analysis |
| **Adaptability** | Manual rule updates | Continuous learning |

### Concrete Example: Edge Case Handling

**Scenario:** Relationship with unusual pattern

```
Features:
  precedence_rate: 0.94      (just below 0.95 threshold)
  counterfactual_rate: 0.08  (good - below 0.15)
  dose_response: 0.72        (good - above 0.5)
  confounding_control: 0.81  (good - above 0.6)
```

**Declarative result:** REJECTED (precedence 0.94 < 0.95)

**Neural result:**
```
P(causal) = sigmoid(
  w1 * 0.94 +    # precedence (high weight learned)
  w2 * 0.08 +    # counterfactual (inverted, high weight)
  w3 * 0.72 +    # dose-response (medium weight)
  w4 * 0.81 +    # confounding (medium weight)
  bias
)
= 0.81  → ACCEPTED

Model learned: very low counterfactual rate (0.08) compensates
for slightly below-threshold precedence
```

---

## Architecture for NDP

```yaml
# gold-layer.manifest.yaml

causal_validation:
  method: neural  # or "declarative" for fallback

  neural_config:
    model_path: models/causal_discriminator.onnx
    threshold: 0.75

    features:
      - precedence_rate
      - reverse_precedence
      - lag_cv
      - response_rate
      - response_magnitude_cv
      - baseline_effect_rate
      - duration_magnitude_corr
      - min_stratified_response
      - temporal_stability
      - context_independence
      - observation_count  # confidence scaling
      - days_observed      # maturity

    training:
      bootstrap: models/causal_bootstrap.json
      self_supervised: true
      update_schedule: weekly
      min_examples: 50

    fallback:
      when: observation_count < 20
      use: declarative_rules  # not enough data for neural
```

---

## Training Data Generation

The system automatically generates training data:

```sql
-- For each candidate relationship, compute training features
CREATE VIEW causal_training_data AS
SELECT
  candidate_id,

  -- Temporal features
  precedence_rate,
  reverse_precedence,
  stddev(lag_minutes) / avg(lag_minutes) as lag_cv,

  -- Response features
  response_rate,
  stddev(effect_magnitude) / avg(effect_magnitude) as magnitude_cv,
  baseline_effect_rate,

  -- Dose-response
  corr(cause_duration, effect_magnitude) as duration_corr,

  -- Stability
  min(response_rate) OVER (PARTITION BY context_bucket) as min_stratified,
  -- ... more features ...

  -- Label (initially NULL, filled by outcomes)
  action_prediction_accuracy as label

FROM candidate_relationships
JOIN correlation_observations USING (candidate_id)
GROUP BY candidate_id;
```

---

## Edge Deployment Considerations

**Model size:** ~5KB (12 inputs × 32 hidden × 16 hidden × 1 output)

**Inference time:** <1ms on Pi 5

**Training:**
- Bootstrap: offline, done once
- Self-supervised updates: weekly batch, ~10 seconds

**Memory:**
- Model weights: 5KB
- Training buffer: ~1MB (stores recent action outcomes)
- Feature computation: negligible (SQL)

---

## What the Neural Model Learns

After training, feature importance analysis reveals what matters:

```
Feature                    Importance
─────────────────────────  ──────────
baseline_effect_rate       0.23  ← "Does effect happen without cause?"
precedence_rate            0.19  ← "Does cause come before effect?"
response_rate              0.15  ← "Does effect follow cause?"
lag_cv                     0.12  ← "Is timing consistent?"
duration_corr              0.11  ← "Does longer cause = bigger effect?"
temporal_stability         0.08  ← "Does pattern hold over time?"
context_independence       0.07  ← "Does it work in all conditions?"
reverse_precedence         0.05  ← "Sanity check: not backwards?"
```

The model discovered that **baseline_effect_rate** (counterfactual) is the most important - if the effect happens frequently without the cause, it's probably not causal.

---

## Declarative Wrapper

Even with neural validation, the trigger remains declarative:

```yaml
promotion:
  candidate_to_causal:
    when:
      method: neural
      causal_probability: "> 0.75"
      observation_count: "> 30"
      sustained: 2 weeks
    then:
      - update: candidate_relationship
        set:
          status: causal
          confidence: "{{causal_probability}}"
      - create: prediction_model
```

**The neural model is a computation within the declarative framework, not a replacement for it.**

---

## Progression Path

| Stage | Causal Validation | When |
|-------|-------------------|------|
| MVP | Declarative rules (4 checks) | Day 1 |
| V1 | Neural discriminator (bootstrapped) | After 30 days |
| V2 | Neural + self-supervised | After 100 actions |
| V3 | Transfer learning to new domains | After 1000 actions |

---

## Summary

**Correlation discovery:** Stays declarative (works well)

**Causal validation:** Neural simplification
- Replace 4 thresholds with 1 probability
- Learn feature interactions automatically
- Self-improve from action outcomes
- Transfer across domains

**Everything else:** Stays declarative
- Prediction models
- Multi-objective optimization
- Action execution
- Safety constraints

**The neural model is a focused intervention** - it replaces the most complex and brittle part of the declarative pipeline with a learned component that gets better over time.

---

## Open Research Questions

1. **Minimum bootstrap size:** How many known-causal examples are needed before the model generalizes?

2. **Negative examples:** How do we generate good spurious correlation examples for training?

3. **Confidence calibration:** Is P(causal)=0.8 actually 80% likely to be causal?

4. **Domain transfer:** Does a model trained on physical systems work for financial relationships?

5. **Adversarial robustness:** Can sensor noise create false causal signals that fool the model?

---

*Document created to explore neural simplification of causal validation*
