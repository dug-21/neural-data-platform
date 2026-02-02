# Autonomous Action Frameworks for Edge Devices

**Research Date:** 2026-02-02
**Research Focus:** Systems that take actions based on learned models on constrained hardware
**Target Platform:** Raspberry Pi 5 (16GB RAM, ARM Cortex-A76)
**Status:** Research Complete

---

## Executive Summary

This research explores autonomous action frameworks for edge devices, addressing how systems can make decisions and take actions based on learned models while running on resource-constrained hardware like Raspberry Pi. The goal is to enable NDP to evolve from a data platform into an intelligent platform that can autonomously act on insights.

### Key Findings

| Framework | Edge Feasibility | Memory | Latency | Best For |
|-----------|------------------|--------|---------|----------|
| **Rule-Based (If-Then)** | Excellent | <1MB | <1ms | Simple, safety-critical |
| **Finite State Machines** | Excellent | 1-10MB | <1ms | Sequential workflows |
| **Behavior Trees** | Very Good | 5-50MB | 1-5ms | Modular, reactive behaviors |
| **Hierarchical FSM** | Good | 10-100MB | 2-10ms | Complex state management |
| **GOAP** | Limited | 50-500MB | 10-100ms | Goal-driven planning |
| **Lightweight RL (Bandits)** | Very Good | 1-50MB | 1-10ms | Online optimization |
| **POMDP (Approximated)** | Limited | 100MB+ | 50-500ms | Uncertainty handling |

### Critical Architecture Decision

**Hybrid Rule-ML Architecture** is recommended:
- **Tier 1 (Always)**: Rule-based safety constraints (hard limits, invariants)
- **Tier 2 (Default)**: Behavior Trees for modular action coordination
- **Tier 3 (Enhanced)**: Lightweight RL for continuous optimization
- **Safety Layer**: Human-in-the-loop for high-stakes decisions

---

## 1. Action Framework Architectures

### 1.1 Rule-Based Systems (If-Then)

The simplest and most reliable action framework. Ideal for safety-critical edge deployment.

**Architecture:**

```
┌─────────────────────────────────────────────────────────────────┐
│                    RULE-BASED ACTION ENGINE                      │
│                                                                  │
│  Sensor Data ──► Rule Matcher ──► Action Selector ──► Executor  │
│                       │                │                        │
│                       ▼                ▼                        │
│               ┌──────────────┐  ┌──────────────┐                │
│               │    Rules     │  │   Actions    │                │
│               │  (YAML/JSON) │  │  (Handlers)  │                │
│               └──────────────┘  └──────────────┘                │
└─────────────────────────────────────────────────────────────────┘
```

**Rust Implementation:**

```rust
/// Simple rule engine for edge actions
pub struct RuleEngine {
    rules: Vec<Rule>,
    safety_constraints: Vec<SafetyConstraint>,
}

pub struct Rule {
    pub id: String,
    pub conditions: Vec<Condition>,
    pub actions: Vec<Action>,
    pub priority: u8,
    pub cooldown: Duration,
    pub last_fired: Option<Instant>,
}

pub enum Condition {
    Threshold { sensor: String, operator: Operator, value: f64 },
    TimeRange { start: NaiveTime, end: NaiveTime },
    StateEquals { entity: String, state: String },
    And(Vec<Condition>),
    Or(Vec<Condition>),
    Not(Box<Condition>),
}

pub enum Action {
    SetState { entity: String, state: String },
    SendAlert { level: AlertLevel, message: String },
    TriggerWebhook { url: String, payload: serde_json::Value },
    AdjustSetpoint { device: String, delta: f64 },
    DeferToHuman { context: String, timeout: Duration },
}

impl RuleEngine {
    pub fn evaluate(&mut self, context: &SensorContext) -> Vec<Action> {
        let mut triggered_actions = Vec::new();

        for rule in &mut self.rules {
            // Check cooldown
            if let Some(last) = rule.last_fired {
                if last.elapsed() < rule.cooldown {
                    continue;
                }
            }

            // Evaluate conditions
            if self.evaluate_conditions(&rule.conditions, context) {
                // Safety check before executing
                let safe_actions = self.filter_safe_actions(&rule.actions, context);
                triggered_actions.extend(safe_actions);
                rule.last_fired = Some(Instant::now());
            }
        }

        // Sort by priority
        triggered_actions.sort_by_key(|a| a.priority());
        triggered_actions
    }
}
```

**Edge Performance:**

| Metric | Value | Notes |
|--------|-------|-------|
| Memory | <1MB for 1000 rules | Rules stored as compact structs |
| Latency | <1ms per evaluation | Linear scan, hot path optimized |
| Complexity | O(n) rules, O(m) conditions | Cacheable condition results |

**NDP Rule Examples:**

```yaml
# config/rules/air-quality-actions.yaml
rules:
  - id: "pm25-high-alert"
    description: "Alert when PM2.5 exceeds safe threshold"
    conditions:
      - sensor: "pm25_current"
        operator: ">"
        value: 35.4  # AQI Unhealthy for Sensitive Groups
    actions:
      - type: "send_alert"
        level: "warning"
        message: "PM2.5 elevated: consider closing windows"
    priority: 1
    cooldown: "30m"

  - id: "window-recommendation"
    description: "Suggest opening windows when outdoor air is cleaner"
    conditions:
      - and:
          - sensor: "pm25_indoor"
            operator: ">"
            value: 20
          - sensor: "pm25_outdoor"
            operator: "<"
            value: 10
          - sensor: "outdoor_temp"
            operator: "between"
            value: [18, 26]
    actions:
      - type: "send_suggestion"
        message: "Outdoor air is cleaner. Consider opening windows."
    priority: 2
    cooldown: "2h"
```

### 1.2 Finite State Machines (FSM)

Classic approach for managing discrete system states. Well-suited for edge due to minimal memory and deterministic behavior.

**Architecture:**

```
┌─────────────────────────────────────────────────────────────────┐
│                    FSM-BASED ACTION CONTROLLER                   │
│                                                                  │
│                        ┌─────────────┐                          │
│                        │   NORMAL    │                          │
│                        │  (monitor)  │                          │
│                        └──────┬──────┘                          │
│                               │                                  │
│              pm25 > 35  ◄─────┴─────► pm25 < 20                 │
│                               │                                  │
│                        ┌──────▼──────┐                          │
│          ┌─────────────│  ELEVATED   │─────────────┐            │
│          │             │   (alert)   │             │            │
│          │             └─────────────┘             │            │
│          │                                         │            │
│          ▼ pm25 > 55                  pm25 < 35 ◄─┘            │
│   ┌──────────────┐                                              │
│   │ UNHEALTHY    │                                              │
│   │ (mitigate)   │                                              │
│   └──────────────┘                                              │
└─────────────────────────────────────────────────────────────────┘
```

**Rust Implementation:**

```rust
/// State machine for air quality management
#[derive(Debug, Clone, PartialEq)]
pub enum AQState {
    Normal,
    Elevated,
    Unhealthy,
    Hazardous,
    Emergency,
}

pub struct AQStateMachine {
    current_state: AQState,
    entry_time: Instant,
    transitions: HashMap<(AQState, Trigger), (AQState, Vec<Action>)>,
}

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub enum Trigger {
    PM25Above(u8),      // Threshold crossed
    PM25Below(u8),      // Recovery
    DurationExceeded,   // Time-based
    ManualOverride,     // Human intervention
}

impl AQStateMachine {
    pub fn new() -> Self {
        let mut transitions = HashMap::new();

        // Normal -> Elevated
        transitions.insert(
            (AQState::Normal, Trigger::PM25Above(35)),
            (AQState::Elevated, vec![
                Action::SendAlert {
                    level: AlertLevel::Info,
                    message: "Air quality degrading".into(),
                },
            ]),
        );

        // Elevated -> Unhealthy
        transitions.insert(
            (AQState::Elevated, Trigger::PM25Above(55)),
            (AQState::Unhealthy, vec![
                Action::SendAlert {
                    level: AlertLevel::Warning,
                    message: "Air quality unhealthy".into(),
                },
                Action::SetState {
                    entity: "hvac".into(),
                    state: "air_purification_mode".into(),
                },
            ]),
        );

        // Recovery transitions
        transitions.insert(
            (AQState::Elevated, Trigger::PM25Below(20)),
            (AQState::Normal, vec![
                Action::SendAlert {
                    level: AlertLevel::Info,
                    message: "Air quality recovered".into(),
                },
            ]),
        );

        Self {
            current_state: AQState::Normal,
            entry_time: Instant::now(),
            transitions,
        }
    }

    pub fn process(&mut self, trigger: Trigger) -> Option<Vec<Action>> {
        let key = (self.current_state.clone(), trigger);

        if let Some((next_state, actions)) = self.transitions.get(&key) {
            self.current_state = next_state.clone();
            self.entry_time = Instant::now();
            Some(actions.clone())
        } else {
            None
        }
    }
}
```

**Comparison with Behavior Trees:**

| Aspect | FSM | Behavior Trees |
|--------|-----|----------------|
| State explosion | Problem at scale | Modular composition |
| Reactivity | Explicit transitions | Built-in fallbacks |
| Debugging | State diagrams | Tree visualization |
| Edge suitability | Excellent | Very Good |
| Complexity handling | Limited | Strong |

*Source: [Polymath Robotics - State Machines vs Behavior Trees](https://www.polymathrobotics.com/blog/state-machines-vs-behavior-trees)*

### 1.3 Behavior Trees (BTs)

Behavior Trees have become the standard for robotics and game AI, replacing FSMs in systems like ROS2's Nav2. They offer modularity, reactivity, and composability.

*Source: [IEEE TSE - Behavior Trees and State Machines in Robotics](https://dl.acm.org/doi/abs/10.1109/TSE.2023.3269081)*

**Architecture:**

```
┌─────────────────────────────────────────────────────────────────┐
│                    BEHAVIOR TREE STRUCTURE                       │
│                                                                  │
│                       ┌─────────────┐                           │
│                       │  Selector   │ (Try until success)       │
│                       │     (?)     │                           │
│                       └──────┬──────┘                           │
│               ┌──────────────┼──────────────┐                   │
│               ▼              ▼              ▼                   │
│        ┌──────────┐   ┌──────────┐   ┌──────────┐              │
│        │ Sequence │   │ Sequence │   │ Fallback │              │
│        │  (→)     │   │  (→)     │   │  Action  │              │
│        └────┬─────┘   └────┬─────┘   └──────────┘              │
│             │              │                                     │
│       ┌─────┼─────┐   ┌────┼────┐                               │
│       ▼     ▼     ▼   ▼    ▼    ▼                               │
│    [Check] [Act] [Log] [Check][Act][Notify]                     │
│                                                                  │
│  Node Types:                                                     │
│  ─────────────                                                  │
│  (?) Selector: Try children until one succeeds                  │
│  (→) Sequence: Execute children in order, stop on failure       │
│  [X] Action: Leaf node that performs an action                  │
│  {?} Condition: Leaf node that checks a condition               │
└─────────────────────────────────────────────────────────────────┘
```

**Rust Implementation:**

```rust
/// Lightweight Behavior Tree for edge devices
pub enum BTNode {
    // Composite nodes
    Selector(Vec<BTNode>),    // OR - try until success
    Sequence(Vec<BTNode>),    // AND - all must succeed
    Parallel(Vec<BTNode>),    // Run all, custom success policy

    // Decorator nodes
    Inverter(Box<BTNode>),    // NOT
    Repeater { child: Box<BTNode>, count: u32 },
    Timeout { child: Box<BTNode>, duration: Duration },

    // Leaf nodes
    Condition(Box<dyn Fn(&Context) -> bool + Send + Sync>),
    Action(Box<dyn Fn(&mut Context) -> BTResult + Send + Sync>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum BTResult {
    Success,
    Failure,
    Running,
}

impl BTNode {
    pub fn tick(&self, context: &mut Context) -> BTResult {
        match self {
            BTNode::Selector(children) => {
                for child in children {
                    match child.tick(context) {
                        BTResult::Success => return BTResult::Success,
                        BTResult::Running => return BTResult::Running,
                        BTResult::Failure => continue,
                    }
                }
                BTResult::Failure
            }

            BTNode::Sequence(children) => {
                for child in children {
                    match child.tick(context) {
                        BTResult::Failure => return BTResult::Failure,
                        BTResult::Running => return BTResult::Running,
                        BTResult::Success => continue,
                    }
                }
                BTResult::Success
            }

            BTNode::Parallel(children) => {
                let results: Vec<BTResult> = children
                    .iter()
                    .map(|c| c.tick(context))
                    .collect();

                // Success if majority succeed
                let successes = results.iter().filter(|r| **r == BTResult::Success).count();
                if successes > results.len() / 2 {
                    BTResult::Success
                } else if results.iter().any(|r| *r == BTResult::Running) {
                    BTResult::Running
                } else {
                    BTResult::Failure
                }
            }

            BTNode::Condition(check) => {
                if check(context) { BTResult::Success } else { BTResult::Failure }
            }

            BTNode::Action(action) => action(context),

            BTNode::Inverter(child) => {
                match child.tick(context) {
                    BTResult::Success => BTResult::Failure,
                    BTResult::Failure => BTResult::Success,
                    BTResult::Running => BTResult::Running,
                }
            }

            BTNode::Timeout { child, duration } => {
                let start = Instant::now();
                let result = child.tick(context);
                if start.elapsed() > *duration {
                    BTResult::Failure
                } else {
                    result
                }
            }

            BTNode::Repeater { child, count } => {
                for _ in 0..*count {
                    child.tick(context);
                }
                BTResult::Success
            }
        }
    }
}
```

**NDP Behavior Tree Example:**

```rust
/// Air quality management behavior tree
fn build_aq_behavior_tree() -> BTNode {
    BTNode::Selector(vec![
        // Priority 1: Emergency response
        BTNode::Sequence(vec![
            BTNode::Condition(Box::new(|ctx| ctx.pm25 > 150.0)),
            BTNode::Action(Box::new(|ctx| {
                ctx.send_alert(AlertLevel::Critical, "Hazardous air quality!");
                ctx.activate_emergency_hvac();
                ctx.notify_human_required("Emergency air quality situation");
                BTResult::Success
            })),
        ]),

        // Priority 2: Active mitigation
        BTNode::Sequence(vec![
            BTNode::Condition(Box::new(|ctx| ctx.pm25 > 55.0)),
            BTNode::Selector(vec![
                // Try HVAC first
                BTNode::Sequence(vec![
                    BTNode::Condition(Box::new(|ctx| ctx.hvac_available())),
                    BTNode::Action(Box::new(|ctx| {
                        ctx.set_hvac_mode("purification");
                        BTResult::Success
                    })),
                ]),
                // Fallback to window management
                BTNode::Sequence(vec![
                    BTNode::Condition(Box::new(|ctx| ctx.outdoor_pm25 < ctx.pm25)),
                    BTNode::Action(Box::new(|ctx| {
                        ctx.suggest_open_windows();
                        BTResult::Success
                    })),
                ]),
                // Last resort: alert only
                BTNode::Action(Box::new(|ctx| {
                    ctx.send_alert(AlertLevel::Warning, "Air quality degraded");
                    BTResult::Success
                })),
            ]),
        ]),

        // Priority 3: Normal monitoring
        BTNode::Action(Box::new(|ctx| {
            ctx.log_status("Air quality normal");
            BTResult::Success
        })),
    ])
}
```

**Edge Performance:**

| Metric | Value | Notes |
|--------|-------|-------|
| Memory | 5-50MB | Depends on tree depth and closures |
| Latency | 1-5ms | Single tree traversal |
| Scalability | O(n) nodes | Efficient with pruning |

**Available Rust Libraries:**

| Library | Status | Notes |
|---------|--------|-------|
| [bonsai-bt](https://crates.io/crates/bonsai-bt) | Active | Lightweight, no_std support |
| [behavior_tree_lite](https://crates.io/crates/behavior_tree_lite) | Maintained | Minimal dependencies |
| Custom implementation | Recommended | Full control, minimal overhead |

### 1.4 Goal-Oriented Action Planning (GOAP)

GOAP separates goals from actions, allowing dynamic plan generation. More computationally expensive but highly flexible.

*Source: [Nez Framework - AI Documentation](https://anshuman-kumar.gitbook.io/nez-doc/ai-fsm-behavior-tree-goap-utility-ai)*

**Architecture:**

```
┌─────────────────────────────────────────────────────────────────┐
│                    GOAP PLANNER ARCHITECTURE                     │
│                                                                  │
│  Current State ──► Planner ──► Action Sequence ──► Executor    │
│        │              │                                         │
│        │              ▼                                         │
│        │       ┌──────────────┐                                 │
│        │       │   A* Search  │                                 │
│        │       │  on Action   │                                 │
│        │       │    Graph     │                                 │
│        │       └──────────────┘                                 │
│        │              │                                         │
│        ▼              ▼                                         │
│  ┌──────────┐  ┌──────────────┐  ┌──────────────┐              │
│  │  World   │  │   Actions    │  │    Goals     │              │
│  │  State   │  │ (with costs) │  │ (priorities) │              │
│  └──────────┘  └──────────────┘  └──────────────┘              │
│                                                                  │
│  Example World State:                                           │
│  { pm25: 45, window_open: false, hvac_on: false, temp: 24 }    │
│                                                                  │
│  Example Goal:                                                   │
│  { pm25 < 25 }                                                   │
│                                                                  │
│  Example Actions:                                                │
│  - open_window:  pre: {outdoor_pm25 < indoor_pm25}              │
│                  eff: {pm25 -= 10}                              │
│                  cost: 1                                         │
│  - run_hvac:     pre: {hvac_available}                          │
│                  eff: {pm25 -= 20}                              │
│                  cost: 5 (energy)                                │
└─────────────────────────────────────────────────────────────────┘
```

**Rust Implementation (Lightweight):**

```rust
/// Lightweight GOAP for edge devices
pub struct GOAPPlanner {
    actions: Vec<GOAPAction>,
    max_plan_depth: usize,
    planning_timeout: Duration,
}

pub struct GOAPAction {
    pub name: String,
    pub preconditions: Vec<(String, Predicate)>,
    pub effects: Vec<(String, StateChange)>,
    pub cost: f32,
}

pub enum Predicate {
    LessThan(f64),
    GreaterThan(f64),
    Equals(String),
    IsTrue,
    IsFalse,
}

pub enum StateChange {
    Set(f64),
    Add(f64),
    Subtract(f64),
    SetString(String),
    SetBool(bool),
}

impl GOAPPlanner {
    pub fn plan(
        &self,
        current_state: &HashMap<String, StateValue>,
        goal: &HashMap<String, Predicate>,
    ) -> Option<Vec<String>> {
        // A* search with state as nodes
        let mut open_set = BinaryHeap::new();
        let mut came_from: HashMap<StateHash, (StateHash, String)> = HashMap::new();
        let mut g_score: HashMap<StateHash, f32> = HashMap::new();

        let start_hash = state_hash(current_state);
        g_score.insert(start_hash.clone(), 0.0);

        open_set.push(PlanNode {
            state: current_state.clone(),
            cost: self.heuristic(current_state, goal),
            hash: start_hash.clone(),
        });

        let start_time = Instant::now();

        while let Some(current) = open_set.pop() {
            // Timeout check for edge safety
            if start_time.elapsed() > self.planning_timeout {
                warn!("GOAP planning timeout - returning partial plan");
                break;
            }

            // Goal reached?
            if self.satisfies_goal(&current.state, goal) {
                return Some(self.reconstruct_plan(&came_from, &current.hash));
            }

            // Try each action
            for action in &self.actions {
                if self.preconditions_met(&current.state, &action.preconditions) {
                    let new_state = self.apply_effects(&current.state, &action.effects);
                    let new_hash = state_hash(&new_state);
                    let tentative_g = g_score.get(&current.hash).unwrap_or(&f32::MAX) + action.cost;

                    if tentative_g < *g_score.get(&new_hash).unwrap_or(&f32::MAX) {
                        came_from.insert(new_hash.clone(), (current.hash.clone(), action.name.clone()));
                        g_score.insert(new_hash.clone(), tentative_g);

                        open_set.push(PlanNode {
                            state: new_state,
                            cost: tentative_g + self.heuristic(&new_state, goal),
                            hash: new_hash,
                        });
                    }
                }
            }
        }

        None // No plan found
    }

    fn heuristic(&self, state: &HashMap<String, StateValue>, goal: &HashMap<String, Predicate>) -> f32 {
        // Count unsatisfied goal conditions
        goal.iter()
            .filter(|(key, pred)| !self.satisfies_predicate(state.get(*key), pred))
            .count() as f32
    }
}
```

**Edge Feasibility Assessment:**

| Aspect | Assessment | Mitigation |
|--------|------------|------------|
| Memory | High (100-500MB for complex domains) | Limit action set, cache plans |
| Latency | High (10-100ms) | Timeout, background planning |
| Complexity | High | Pre-compute common plans |

**Recommendation:** GOAP is NOT recommended as primary framework for NDP edge. Use for offline planning or as a fallback for complex scenarios that require human-approved plans.

### 1.5 Framework Comparison Matrix

| Criteria | Rules | FSM | Behavior Trees | GOAP |
|----------|-------|-----|----------------|------|
| **Memory (1K decisions)** | <1MB | 1-10MB | 5-50MB | 100-500MB |
| **Latency** | <1ms | <1ms | 1-5ms | 10-100ms |
| **Modularity** | Low | Low | High | Very High |
| **Reactivity** | High | Medium | Very High | Low |
| **Safety guarantees** | Excellent | Good | Good | Limited |
| **Learning integration** | Easy | Medium | Medium | Hard |
| **Edge deployment** | Excellent | Excellent | Very Good | Limited |
| **Debugging** | Simple | Visual | Tree-based | Complex |

---

## 2. Decision Making Under Uncertainty

### 2.1 Bayesian Decision Networks (Lightweight)

For edge devices, full Bayesian networks are often too expensive. Lightweight approximations are viable.

**Naive Bayes for Action Selection:**

```rust
/// Lightweight Bayesian action selector
pub struct BayesianActionSelector {
    // P(success | features) for each action
    likelihood_tables: HashMap<String, ConditionalProbTable>,
    action_priors: HashMap<String, f64>,
}

pub struct ConditionalProbTable {
    // Discretized feature bins -> success probability
    bins: Vec<f64>,
    bin_edges: Vec<f64>,
    success_counts: Vec<u32>,
    total_counts: Vec<u32>,
}

impl BayesianActionSelector {
    pub fn select_action(&self, features: &[f64]) -> (String, f64) {
        let mut best_action = String::new();
        let mut best_prob = 0.0;

        for (action, likelihood_table) in &self.likelihood_tables {
            // P(success | features) * P(action)
            let likelihood = likelihood_table.compute_likelihood(features);
            let prior = self.action_priors.get(action).unwrap_or(&0.1);
            let posterior = likelihood * prior;

            if posterior > best_prob {
                best_prob = posterior;
                best_action = action.clone();
            }
        }

        (best_action, best_prob)
    }

    pub fn update(&mut self, action: &str, features: &[f64], success: bool) {
        if let Some(table) = self.likelihood_tables.get_mut(action) {
            let bin = table.find_bin(features);
            table.total_counts[bin] += 1;
            if success {
                table.success_counts[bin] += 1;
            }
        }
    }
}
```

**Memory:** ~10-50KB per action with 10-20 feature bins

### 2.2 Partially Observable MDPs (POMDPs) - Simplified

Full POMDP solvers are computationally intractable for edge. Use approximations.

*Source: [Annual Reviews - POMDPs and Robotics](https://www.annualreviews.org/content/journals/10.1146/annurev-control-042920-092451)*

**QMDP Approximation (Simplest):**

```rust
/// QMDP approximation for edge POMDPs
/// Assumes full observability at next step (optimistic)
pub struct QMDPSolver {
    // Q-values: Q[state][action]
    q_values: Vec<Vec<f64>>,
    // Belief state (probability distribution over states)
    belief: Vec<f64>,
    n_states: usize,
    n_actions: usize,
}

impl QMDPSolver {
    pub fn select_action(&self) -> usize {
        // Compute expected Q-value for each action under current belief
        let expected_q: Vec<f64> = (0..self.n_actions)
            .map(|a| {
                self.belief.iter()
                    .enumerate()
                    .map(|(s, b)| b * self.q_values[s][a])
                    .sum()
            })
            .collect();

        // Select action with highest expected value
        expected_q.iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
            .map(|(idx, _)| idx)
            .unwrap_or(0)
    }

    pub fn update_belief(&mut self, action: usize, observation: usize) {
        // Bayesian belief update
        // P(s' | o, a, b) = P(o | s', a) * sum_s P(s' | s, a) * b(s) / P(o | b, a)
        // Simplified for edge: use particle filter approximation
        let mut new_belief = vec![0.0; self.n_states];

        for (s_prime, b_new) in new_belief.iter_mut().enumerate() {
            // Observation likelihood * transition * prior belief
            *b_new = self.observation_prob(observation, s_prime, action)
                * self.transition_probability_sum(s_prime, action);
        }

        // Normalize
        let sum: f64 = new_belief.iter().sum();
        if sum > 0.0 {
            for b in &mut new_belief {
                *b /= sum;
            }
        }

        self.belief = new_belief;
    }
}
```

**Point-Based Value Iteration (PBVI) - More Accurate:**

```rust
/// Point-based POMDP solver for edge
/// Pre-computes value function for sampled belief points
pub struct PBVISolver {
    belief_points: Vec<Vec<f64>>,      // Sampled belief states
    alpha_vectors: Vec<AlphaVector>,   // Value function representation
    n_states: usize,
    n_actions: usize,
}

pub struct AlphaVector {
    action: usize,
    values: Vec<f64>,  // Value at each state
}

impl PBVISolver {
    /// Offline: compute optimal alpha vectors
    pub fn solve_offline(&mut self, iterations: usize) {
        for _ in 0..iterations {
            self.backup_all_beliefs();
        }
    }

    /// Online: select best action for current belief
    pub fn select_action(&self, belief: &[f64]) -> (usize, f64) {
        let mut best_action = 0;
        let mut best_value = f64::NEG_INFINITY;

        for alpha in &self.alpha_vectors {
            let value: f64 = belief.iter()
                .zip(&alpha.values)
                .map(|(b, v)| b * v)
                .sum();

            if value > best_value {
                best_value = value;
                best_action = alpha.action;
            }
        }

        (best_action, best_value)
    }
}
```

**Edge Feasibility:**

| Method | States | Actions | Memory | Online Latency |
|--------|--------|---------|--------|----------------|
| QMDP | <100 | <10 | ~100KB | <1ms |
| PBVI | <500 | <20 | ~5MB | <5ms |
| Full POMDP | <50 | <5 | 50MB+ | 10-100ms |

### 2.3 Monte Carlo Tree Search (Simplified)

Useful for planning with uncertainty. Limit iterations for edge.

```rust
/// Lightweight MCTS for edge planning
pub struct EdgeMCTS {
    root: MCTSNode,
    exploration_constant: f64,
    max_iterations: usize,
    max_depth: usize,
    simulation_timeout: Duration,
}

struct MCTSNode {
    state: State,
    action: Option<Action>,
    visits: u32,
    value: f64,
    children: Vec<MCTSNode>,
    untried_actions: Vec<Action>,
}

impl EdgeMCTS {
    pub fn search(&mut self, current_state: State) -> Action {
        self.root = MCTSNode::new(current_state);

        for _ in 0..self.max_iterations {
            // Selection
            let mut node = &mut self.root;
            let mut path = vec![];

            while node.is_fully_expanded() && !node.is_terminal() {
                path.push(node as *mut MCTSNode);
                node = node.select_child(self.exploration_constant);
            }

            // Expansion
            if !node.is_terminal() && !node.untried_actions.is_empty() {
                let action = node.untried_actions.pop().unwrap();
                let child_state = self.simulate_action(&node.state, &action);
                node.children.push(MCTSNode::new_with_action(child_state, action));
                node = node.children.last_mut().unwrap();
            }

            // Simulation (rollout) - limited depth for edge
            let reward = self.rollout(&node.state, self.max_depth);

            // Backpropagation
            node.visits += 1;
            node.value += reward;
            for node_ptr in path.into_iter().rev() {
                unsafe {
                    let n = &mut *node_ptr;
                    n.visits += 1;
                    n.value += reward;
                }
            }
        }

        // Return best action from root
        self.root.best_child().action.clone().unwrap()
    }

    fn rollout(&self, state: &State, depth: usize) -> f64 {
        // Random simulation with depth limit
        let mut current = state.clone();
        let mut total_reward = 0.0;
        let mut discount = 1.0;

        for _ in 0..depth {
            if current.is_terminal() {
                break;
            }
            let action = current.random_action();
            let (next_state, reward) = self.step(&current, &action);
            total_reward += discount * reward;
            discount *= 0.95;
            current = next_state;
        }

        total_reward
    }
}
```

**Edge Configuration:**

| Parameter | Edge Setting | Rationale |
|-----------|--------------|-----------|
| max_iterations | 100-500 | Balance quality vs latency |
| max_depth | 5-10 | Prevent explosion |
| exploration_constant | 1.41 (sqrt(2)) | UCB1 standard |
| simulation_timeout | 50ms | Hard latency limit |

---

## 3. Safety Constraints

### 3.1 Constraint Architecture

Safety is paramount for autonomous edge systems. Defense-in-depth approach.

*Source: [CNCF - Autonomous Enterprise and Platform Control](https://www.cncf.io/blog/2026/01/23/the-autonomous-enterprise-and-the-four-pillars-of-platform-control-2026-forecast/)*

**Safety Layer Architecture:**

```
┌─────────────────────────────────────────────────────────────────┐
│                    SAFETY CONSTRAINT ARCHITECTURE                │
│                                                                  │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │  LAYER 1: HARD CONSTRAINTS (Always Enforced)            │    │
│  │  - Never exceed physical limits                         │    │
│  │  - Never bypass safety sensors                          │    │
│  │  - Always maintain minimum response time                │    │
│  │  - Never act on stale data (>30s)                       │    │
│  └─────────────────────────────────────────────────────────┘    │
│                              │                                   │
│                              ▼                                   │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │  LAYER 2: SOFT CONSTRAINTS (Enforceable)                │    │
│  │  - Rate limiting (max N actions per hour)               │    │
│  │  - Budget limits (energy, API calls)                    │    │
│  │  - Confidence thresholds (>0.8 for actions)             │    │
│  │  - Reversibility requirements                           │    │
│  └─────────────────────────────────────────────────────────┘    │
│                              │                                   │
│                              ▼                                   │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │  LAYER 3: ADVISORY CONSTRAINTS (Logged)                 │    │
│  │  - Preference violations                                │    │
│  │  - Efficiency recommendations                           │    │
│  │  - Learning opportunities                               │    │
│  └─────────────────────────────────────────────────────────┘    │
│                              │                                   │
│                              ▼                                   │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │  LAYER 4: HUMAN-IN-THE-LOOP (Approval Required)         │    │
│  │  - High-stakes decisions                                │    │
│  │  - Novel situations (low confidence)                    │    │
│  │  - Irreversible actions                                 │    │
│  │  - Policy changes                                       │    │
│  └─────────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────────┘
```

**Rust Implementation:**

```rust
/// Defense-in-depth safety system
pub struct SafetyManager {
    hard_constraints: Vec<HardConstraint>,
    soft_constraints: Vec<SoftConstraint>,
    rate_limiters: HashMap<String, RateLimiter>,
    human_approval_queue: mpsc::Sender<HumanApprovalRequest>,
    audit_log: AuditLogger,
}

#[derive(Debug)]
pub struct HardConstraint {
    pub name: String,
    pub check: Box<dyn Fn(&Action, &Context) -> bool + Send + Sync>,
    pub violation_action: ViolationAction,
}

#[derive(Debug)]
pub enum ViolationAction {
    Block,
    BlockAndAlert,
    EmergencyStop,
}

pub struct SoftConstraint {
    pub name: String,
    pub check: Box<dyn Fn(&Action, &Context) -> ConstraintResult + Send + Sync>,
    pub on_violation: SoftViolationPolicy,
}

pub enum SoftViolationPolicy {
    Warn,
    RateLimited(Duration),
    RequireConfirmation,
}

pub enum ConstraintResult {
    Pass,
    Fail(String),
    PassWithWarning(String),
}

impl SafetyManager {
    pub async fn validate_action(&self, action: &Action, context: &Context) -> SafetyResult {
        // Layer 1: Hard constraints (always block on failure)
        for constraint in &self.hard_constraints {
            if !(constraint.check)(action, context) {
                self.audit_log.log_violation(&constraint.name, action, "HARD");
                return SafetyResult::Blocked {
                    reason: format!("Hard constraint violated: {}", constraint.name),
                    constraint_type: "hard",
                };
            }
        }

        // Layer 2: Soft constraints (may proceed with warnings)
        let mut warnings = Vec::new();
        for constraint in &self.soft_constraints {
            match (constraint.check)(action, context) {
                ConstraintResult::Fail(reason) => {
                    self.audit_log.log_violation(&constraint.name, action, "SOFT");
                    match &constraint.on_violation {
                        SoftViolationPolicy::Warn => warnings.push(reason),
                        SoftViolationPolicy::RateLimited(duration) => {
                            if !self.check_rate_limit(&constraint.name, *duration) {
                                return SafetyResult::RateLimited {
                                    retry_after: *duration,
                                };
                            }
                        }
                        SoftViolationPolicy::RequireConfirmation => {
                            return SafetyResult::RequiresApproval { reason };
                        }
                    }
                }
                ConstraintResult::PassWithWarning(warning) => warnings.push(warning),
                ConstraintResult::Pass => {}
            }
        }

        // Layer 3: Check if human approval needed
        if action.requires_human_approval() {
            let approval = self.request_human_approval(action, context).await;
            if !approval.approved {
                return SafetyResult::HumanRejected { reason: approval.reason };
            }
        }

        // All checks passed
        self.audit_log.log_approved(action, &warnings);
        SafetyResult::Approved { warnings }
    }
}
```

### 3.2 Human-in-the-Loop Patterns

*Source: [Frontier Enterprise - AI Agent Autonomy](https://www.frontier-enterprise.com/ai-agent-autonomy-needs-human-control-and-guardrails/)*

**Approval Request Flow:**

```rust
/// Human-in-the-loop approval system
pub struct HumanApprovalSystem {
    pending_requests: HashMap<Uuid, ApprovalRequest>,
    timeout: Duration,
    fallback_policy: FallbackPolicy,
    notification_channels: Vec<Box<dyn NotificationChannel>>,
}

pub struct ApprovalRequest {
    pub id: Uuid,
    pub action: Action,
    pub context: Context,
    pub reason: String,
    pub created_at: Instant,
    pub urgency: Urgency,
    pub auto_expire: Option<Duration>,
}

pub enum Urgency {
    Low,      // Can wait hours
    Medium,   // Should respond within 30 min
    High,     // Should respond within 5 min
    Critical, // Immediate attention required
}

pub enum FallbackPolicy {
    RejectOnTimeout,
    ApproveOnTimeout { max_risk: RiskLevel },
    DeferToNextAction,
    ExecuteWithLogging,
}

impl HumanApprovalSystem {
    pub async fn request_approval(&mut self, request: ApprovalRequest) -> ApprovalResult {
        let id = request.id;
        let timeout = request.auto_expire.unwrap_or(self.timeout);

        // Notify via configured channels
        for channel in &self.notification_channels {
            channel.notify(&request).await;
        }

        self.pending_requests.insert(id, request);

        // Wait for approval with timeout
        let result = tokio::select! {
            approval = self.wait_for_approval(id) => approval,
            _ = tokio::time::sleep(timeout) => {
                self.handle_timeout(id)
            }
        };

        self.pending_requests.remove(&id);
        result
    }

    fn handle_timeout(&self, id: Uuid) -> ApprovalResult {
        match &self.fallback_policy {
            FallbackPolicy::RejectOnTimeout => {
                ApprovalResult::Rejected {
                    reason: "Approval timeout - action rejected for safety".into(),
                }
            }
            FallbackPolicy::ApproveOnTimeout { max_risk } => {
                let request = self.pending_requests.get(&id).unwrap();
                if request.action.risk_level() <= *max_risk {
                    ApprovalResult::Approved {
                        auto_approved: true,
                        reason: "Timeout auto-approval (low risk)".into(),
                    }
                } else {
                    ApprovalResult::Rejected {
                        reason: "Risk too high for auto-approval".into(),
                    }
                }
            }
            FallbackPolicy::DeferToNextAction => {
                ApprovalResult::Deferred {
                    retry_at: Instant::now() + Duration::from_secs(300),
                }
            }
            FallbackPolicy::ExecuteWithLogging => {
                ApprovalResult::Approved {
                    auto_approved: true,
                    reason: "Executed with enhanced logging".into(),
                }
            }
        }
    }
}
```

**NDP Human-in-the-Loop Examples:**

| Action Type | Approval Required | Timeout Policy |
|-------------|-------------------|----------------|
| Send alert | No | - |
| Suggest action | No | - |
| Adjust HVAC setpoint +/-2C | No | - |
| Adjust HVAC setpoint >5C | Yes | Reject on timeout |
| Close windows automatically | Yes (first time) | Ask again later |
| Emergency ventilation | No (safety override) | - |
| Financial transaction suggestion | Yes | Never auto-approve |
| Change automation rules | Yes | Reject on timeout |

### 3.3 Kill-Switch and Graceful Degradation

*Source: [ClearanceJobs - AI Risk Forecast](https://news.clearancejobs.com/2026/01/02/from-ai-hype-to-ai-risk-cybersecurity-experts-share-2026-forecast-and-predictions/)*

```rust
/// Emergency stop and degradation system
pub struct GracefulDegradation {
    system_state: Arc<AtomicU8>,
    degradation_levels: Vec<DegradationLevel>,
}

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum SystemMode {
    Normal = 0,
    Degraded = 1,
    SafeMode = 2,
    EmergencyStop = 3,
    ManualOnly = 4,
}

pub struct DegradationLevel {
    pub mode: SystemMode,
    pub allowed_actions: HashSet<ActionType>,
    pub notification_required: bool,
    pub recovery_policy: RecoveryPolicy,
}

impl GracefulDegradation {
    pub fn trigger_degradation(&self, new_mode: SystemMode, reason: &str) {
        let current = SystemMode::from(self.system_state.load(Ordering::SeqCst));

        // Only allow escalation, not de-escalation without explicit recovery
        if new_mode as u8 > current as u8 {
            self.system_state.store(new_mode as u8, Ordering::SeqCst);
            warn!("System degraded to {:?}: {}", new_mode, reason);

            // Notify appropriate channels
            let level = &self.degradation_levels[new_mode as usize];
            if level.notification_required {
                self.notify_degradation(new_mode, reason);
            }
        }
    }

    pub fn check_action_allowed(&self, action: &Action) -> bool {
        let mode = SystemMode::from(self.system_state.load(Ordering::SeqCst));
        let level = &self.degradation_levels[mode as usize];
        level.allowed_actions.contains(&action.action_type())
    }

    pub fn emergency_stop(&self) {
        self.system_state.store(SystemMode::EmergencyStop as u8, Ordering::SeqCst);
        // Execute all emergency procedures
        self.execute_emergency_procedures();
    }
}
```

### 3.4 Bounded Autonomy Architecture

*Source: [Machine Learning Mastery - Agentic AI Trends 2026](https://machinelearningmastery.com/7-agentic-ai-trends-to-watch-in-2026/)*

```rust
/// Bounded autonomy with clear operational limits
pub struct BoundedAutonomy {
    operational_envelope: OperationalEnvelope,
    escalation_paths: Vec<EscalationPath>,
    audit_trail: AuditTrail,
}

pub struct OperationalEnvelope {
    // What the system CAN do without approval
    pub allowed_actions: HashSet<ActionType>,
    pub allowed_targets: HashSet<String>,
    pub max_impact_level: ImpactLevel,
    pub max_actions_per_hour: u32,
    pub max_cumulative_impact: f64,

    // Time-based restrictions
    pub active_hours: Option<(NaiveTime, NaiveTime)>,
    pub blackout_periods: Vec<(DateTime<Utc>, DateTime<Utc>)>,
}

pub struct EscalationPath {
    pub trigger: EscalationTrigger,
    pub handler: EscalationHandler,
    pub timeout: Duration,
}

pub enum EscalationTrigger {
    ActionOutsideEnvelope,
    CumulativeImpactExceeded,
    UncertaintyAboveThreshold(f64),
    AnomalyDetected,
    SafetyConstraintViolation,
    NovelSituation,
}

pub enum EscalationHandler {
    Human { channel: NotificationChannel },
    SupervisorAgent { endpoint: String },
    FallbackPolicy { policy: FallbackPolicy },
    EmergencyStop,
}

impl BoundedAutonomy {
    pub fn within_envelope(&self, action: &Action, context: &Context) -> EnvelopeCheck {
        // Check action type
        if !self.operational_envelope.allowed_actions.contains(&action.action_type()) {
            return EnvelopeCheck::Outside {
                reason: "Action type not in allowed set".into(),
                escalation: EscalationTrigger::ActionOutsideEnvelope,
            };
        }

        // Check target
        if !self.operational_envelope.allowed_targets.contains(&action.target()) {
            return EnvelopeCheck::Outside {
                reason: "Target not in allowed set".into(),
                escalation: EscalationTrigger::ActionOutsideEnvelope,
            };
        }

        // Check impact level
        if action.estimated_impact() > self.operational_envelope.max_impact_level {
            return EnvelopeCheck::Outside {
                reason: "Impact level too high".into(),
                escalation: EscalationTrigger::ActionOutsideEnvelope,
            };
        }

        // Check confidence
        if context.decision_confidence < 0.7 {
            return EnvelopeCheck::Outside {
                reason: "Confidence too low for autonomous action".into(),
                escalation: EscalationTrigger::UncertaintyAboveThreshold(0.7),
            };
        }

        // Check time restrictions
        if let Some((start, end)) = &self.operational_envelope.active_hours {
            let now = Local::now().time();
            if now < *start || now > *end {
                return EnvelopeCheck::Outside {
                    reason: "Outside active hours".into(),
                    escalation: EscalationTrigger::ActionOutsideEnvelope,
                };
            }
        }

        EnvelopeCheck::Within
    }
}
```

---

## 4. Feedback Loops

### 4.1 Outcome-Based Learning Architecture

The system must learn from the outcomes of its actions to improve over time.

**Architecture:**

```
┌─────────────────────────────────────────────────────────────────┐
│                    FEEDBACK LOOP ARCHITECTURE                    │
│                                                                  │
│  Decision ──► Action ──► Wait ──► Measure ──► Evaluate ──► Learn│
│      ▲                                                     │    │
│      │                                                     │    │
│      └─────────────────────────────────────────────────────┘    │
│                                                                  │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │                 OUTCOME MEASUREMENT                      │    │
│  │                                                          │    │
│  │  1. Immediate: Did action execute? (1s)                 │    │
│  │  2. Short-term: Did metrics improve? (5-30min)          │    │
│  │  3. Medium-term: Was problem solved? (1-6h)             │    │
│  │  4. Long-term: Did pattern help generally? (days+)      │    │
│  │                                                          │    │
│  └─────────────────────────────────────────────────────────┘    │
│                                                                  │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │                  REWARD CALCULATION                      │    │
│  │                                                          │    │
│  │  reward = w1 * goal_achievement                         │    │
│  │         + w2 * efficiency_score                         │    │
│  │         - w3 * safety_violations                        │    │
│  │         - w4 * human_intervention_needed                │    │
│  │         + w5 * positive_user_feedback                   │    │
│  │                                                          │    │
│  └─────────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────────┘
```

**Rust Implementation:**

```rust
/// Outcome-based learning system
pub struct OutcomeLearner {
    // Track action outcomes
    outcome_tracker: OutcomeTracker,
    // Policy update mechanism
    policy_updater: PolicyUpdater,
    // Reward signal calculator
    reward_calculator: RewardCalculator,
}

pub struct OutcomeTracker {
    pending_outcomes: HashMap<Uuid, PendingOutcome>,
    completed_outcomes: VecDeque<CompletedOutcome>,
    max_history: usize,
}

pub struct PendingOutcome {
    action_id: Uuid,
    action: Action,
    context_snapshot: Context,
    initiated_at: Instant,
    expected_outcome: ExpectedOutcome,
    measurement_schedule: Vec<MeasurementPoint>,
}

pub struct MeasurementPoint {
    delay: Duration,
    metric: String,
    baseline: f64,
    target: f64,
    measured: Option<f64>,
}

pub struct RewardCalculator {
    weights: RewardWeights,
}

pub struct RewardWeights {
    pub goal_achievement: f64,   // 0.4
    pub efficiency: f64,         // 0.2
    pub safety_penalty: f64,     // -0.3
    pub human_intervention: f64, // -0.1
    pub user_feedback: f64,      // 0.2
}

impl OutcomeLearner {
    pub async fn track_action(&mut self, action: Action, context: Context) -> Uuid {
        let id = Uuid::new_v4();

        let pending = PendingOutcome {
            action_id: id,
            action: action.clone(),
            context_snapshot: context.clone(),
            initiated_at: Instant::now(),
            expected_outcome: self.predict_outcome(&action, &context),
            measurement_schedule: self.create_measurement_schedule(&action),
        };

        self.outcome_tracker.pending_outcomes.insert(id, pending);

        // Schedule measurements
        self.schedule_measurements(id).await;

        id
    }

    async fn schedule_measurements(&self, action_id: Uuid) {
        let pending = self.outcome_tracker.pending_outcomes.get(&action_id).unwrap();

        for point in &pending.measurement_schedule {
            let delay = point.delay;
            let metric = point.metric.clone();
            let action_id = action_id;

            tokio::spawn(async move {
                tokio::time::sleep(delay).await;
                // Trigger measurement
                self.measure_outcome(action_id, &metric).await;
            });
        }
    }

    pub fn calculate_reward(&self, outcome: &CompletedOutcome) -> f64 {
        let w = &self.reward_calculator.weights;

        let goal_score = outcome.goal_achievement_ratio();
        let efficiency_score = outcome.efficiency_score();
        let safety_violations = outcome.safety_violations.len() as f64;
        let human_needed = if outcome.required_human_intervention { 1.0 } else { 0.0 };
        let user_feedback = outcome.user_feedback.unwrap_or(0.0);

        w.goal_achievement * goal_score
            + w.efficiency * efficiency_score
            + w.safety_penalty * safety_violations
            + w.human_intervention * human_needed
            + w.user_feedback * user_feedback
    }

    pub fn update_policy(&mut self, outcomes: &[CompletedOutcome]) {
        for outcome in outcomes {
            let reward = self.calculate_reward(outcome);

            // Update action preference
            self.policy_updater.update(
                &outcome.action,
                &outcome.context_snapshot,
                reward,
            );

            // If negative reward, potentially add constraint
            if reward < -0.5 {
                self.consider_new_constraint(&outcome);
            }
        }
    }
}
```

### 4.2 Online Policy Improvement

Lightweight policy updates suitable for edge.

```rust
/// Lightweight online policy improvement
pub struct OnlinePolicyLearner {
    // Simple tabular policy (state -> action preferences)
    policy_table: HashMap<StateKey, ActionPreferences>,
    // Learning parameters
    learning_rate: f64,
    discount_factor: f64,
    exploration_rate: f64,
}

pub struct ActionPreferences {
    preferences: HashMap<ActionKey, f64>,
    visit_counts: HashMap<ActionKey, u32>,
}

impl OnlinePolicyLearner {
    /// Select action using softmax over preferences
    pub fn select_action(&mut self, state: &StateKey) -> ActionKey {
        let prefs = self.policy_table.entry(state.clone()).or_default();

        if rand::random::<f64>() < self.exploration_rate {
            // Exploration: random action
            prefs.preferences.keys().choose(&mut rand::thread_rng()).cloned().unwrap()
        } else {
            // Exploitation: softmax selection
            let temp = 1.0;
            let exp_prefs: Vec<(ActionKey, f64)> = prefs.preferences
                .iter()
                .map(|(a, p)| (a.clone(), (p / temp).exp()))
                .collect();
            let sum: f64 = exp_prefs.iter().map(|(_, e)| e).sum();
            let probs: Vec<(ActionKey, f64)> = exp_prefs
                .into_iter()
                .map(|(a, e)| (a, e / sum))
                .collect();

            // Sample according to probabilities
            let mut cumsum = 0.0;
            let r = rand::random::<f64>();
            for (action, prob) in probs {
                cumsum += prob;
                if r < cumsum {
                    return action;
                }
            }
            prefs.preferences.keys().next().cloned().unwrap()
        }
    }

    /// Update policy based on observed reward
    pub fn update(&mut self, state: &StateKey, action: &ActionKey, reward: f64) {
        let prefs = self.policy_table.entry(state.clone()).or_default();

        // Update visit count
        *prefs.visit_counts.entry(action.clone()).or_insert(0) += 1;
        let n = prefs.visit_counts[action] as f64;

        // Running average update
        let current = *prefs.preferences.entry(action.clone()).or_insert(0.0);
        let new_value = current + (reward - current) / n;
        prefs.preferences.insert(action.clone(), new_value);

        // Decay exploration rate
        self.exploration_rate *= 0.9995;
        self.exploration_rate = self.exploration_rate.max(0.05);
    }

    /// Batch update with experience replay
    pub fn batch_update(&mut self, experiences: &[(StateKey, ActionKey, f64)]) {
        for (state, action, reward) in experiences {
            self.update(state, action, *reward);
        }
    }
}
```

### 4.3 Counterfactual Reasoning (Lightweight)

*Source: [arXiv - Compressed Causal Reasoning](https://arxiv.org/abs/2512.13725)*

Understanding what would have happened if a different action was taken.

```rust
/// Lightweight counterfactual analysis for edge
pub struct CounterfactualAnalyzer {
    action_effect_models: HashMap<ActionType, EffectModel>,
    baseline_models: HashMap<String, TimeSeriesBaseline>,
}

pub struct EffectModel {
    // Expected effect on each metric
    expected_effects: HashMap<String, ExpectedEffect>,
    // Variance of effect
    effect_variance: HashMap<String, f64>,
    // Sample count
    n_observations: u32,
}

pub struct ExpectedEffect {
    pub metric: String,
    pub direction: EffectDirection,
    pub magnitude: f64,
    pub confidence: f64,
    pub time_to_effect: Duration,
}

pub enum EffectDirection {
    Increase,
    Decrease,
    NoChange,
}

impl CounterfactualAnalyzer {
    /// Estimate what would have happened without the action
    pub fn estimate_counterfactual(
        &self,
        action: &Action,
        actual_outcome: &Outcome,
    ) -> CounterfactualEstimate {
        // Get baseline prediction (what would have happened without intervention)
        let baseline = self.baseline_models
            .get(&action.target())
            .map(|m| m.predict(actual_outcome.measurement_time))
            .unwrap_or(actual_outcome.pre_action_value);

        // Estimate action effect
        let effect_model = self.action_effect_models.get(&action.action_type());
        let estimated_effect = effect_model
            .and_then(|m| m.expected_effects.get(&actual_outcome.metric))
            .map(|e| e.magnitude)
            .unwrap_or(0.0);

        // Counterfactual: what if we hadn't acted?
        let counterfactual_value = actual_outcome.post_action_value - estimated_effect;

        // Actual lift from action
        let actual_lift = actual_outcome.post_action_value - baseline;

        // Attributed lift (causal effect estimate)
        let attributed_lift = actual_outcome.post_action_value - counterfactual_value;

        CounterfactualEstimate {
            baseline_prediction: baseline,
            counterfactual_value,
            actual_value: actual_outcome.post_action_value,
            actual_lift,
            attributed_lift,
            confidence: effect_model
                .and_then(|m| m.expected_effects.get(&actual_outcome.metric))
                .map(|e| e.confidence)
                .unwrap_or(0.5),
        }
    }

    /// Learn action effects from observations
    pub fn update_effect_model(
        &mut self,
        action: &Action,
        pre_value: f64,
        post_value: f64,
        metric: &str,
        baseline_prediction: f64,
    ) {
        let effect = post_value - baseline_prediction;

        let model = self.action_effect_models
            .entry(action.action_type())
            .or_insert_with(EffectModel::default);

        // Online update of expected effect
        let expected = model.expected_effects
            .entry(metric.to_string())
            .or_insert_with(|| ExpectedEffect {
                metric: metric.to_string(),
                direction: EffectDirection::NoChange,
                magnitude: 0.0,
                confidence: 0.5,
                time_to_effect: Duration::from_secs(300),
            });

        model.n_observations += 1;
        let n = model.n_observations as f64;

        // Running average
        expected.magnitude = expected.magnitude + (effect - expected.magnitude) / n;

        // Update direction
        expected.direction = if expected.magnitude > 0.1 {
            EffectDirection::Increase
        } else if expected.magnitude < -0.1 {
            EffectDirection::Decrease
        } else {
            EffectDirection::NoChange
        };

        // Update confidence based on variance
        let variance = model.effect_variance
            .entry(metric.to_string())
            .or_insert(1.0);
        *variance = *variance + ((effect - expected.magnitude).powi(2) - *variance) / n;

        // Higher sample count + lower variance = higher confidence
        expected.confidence = (1.0 - (*variance / (expected.magnitude.abs() + 1.0)))
            .clamp(0.1, 0.95)
            * (1.0 - 1.0 / n.sqrt());
    }
}
```

### 4.4 A/B Testing on Edge

*Source: [Adaptive Systems Research](/workspaces/neural-data-platform/product/research/gold/self-learning/ADAPTIVE-SYSTEMS.md)*

```rust
/// Edge-local A/B testing for action variants
pub struct EdgeABTesting {
    experiments: HashMap<String, Experiment>,
    results_store: ResultsStore,
}

pub struct Experiment {
    pub id: String,
    pub variants: Vec<ActionVariant>,
    pub allocation_strategy: AllocationStrategy,
    pub min_samples_per_variant: u32,
    pub statistical_significance: f64,
    pub created_at: DateTime<Utc>,
}

pub struct ActionVariant {
    pub id: String,
    pub action_modifier: ActionModifier,
    pub samples: u32,
    pub successes: u32,
    pub total_reward: f64,
}

pub enum AllocationStrategy {
    EqualSplit,
    Thompson,       // Thompson Sampling (Bayesian)
    UCB1,          // Upper Confidence Bound
    EpsilonGreedy { epsilon: f64 },
}

impl EdgeABTesting {
    /// Select variant using configured strategy
    pub fn select_variant(&mut self, experiment_id: &str) -> Option<&ActionVariant> {
        let exp = self.experiments.get(experiment_id)?;

        match &exp.allocation_strategy {
            AllocationStrategy::Thompson => self.thompson_select(experiment_id),
            AllocationStrategy::UCB1 => self.ucb1_select(experiment_id),
            AllocationStrategy::EpsilonGreedy { epsilon } => {
                if rand::random::<f64>() < *epsilon {
                    // Explore: random variant
                    exp.variants.choose(&mut rand::thread_rng())
                } else {
                    // Exploit: best variant
                    exp.variants.iter()
                        .max_by(|a, b| {
                            let a_rate = a.successes as f64 / a.samples.max(1) as f64;
                            let b_rate = b.successes as f64 / b.samples.max(1) as f64;
                            a_rate.partial_cmp(&b_rate).unwrap()
                        })
                }
            }
            AllocationStrategy::EqualSplit => {
                // Select variant with fewest samples
                exp.variants.iter().min_by_key(|v| v.samples)
            }
        }
    }

    fn thompson_select(&self, experiment_id: &str) -> Option<&ActionVariant> {
        let exp = self.experiments.get(experiment_id)?;

        // Sample from Beta distribution for each variant
        let samples: Vec<(usize, f64)> = exp.variants.iter()
            .enumerate()
            .map(|(i, v)| {
                let alpha = v.successes as f64 + 1.0;
                let beta = (v.samples - v.successes) as f64 + 1.0;
                (i, sample_beta(alpha, beta))
            })
            .collect();

        let best_idx = samples.iter()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
            .map(|(i, _)| *i)?;

        Some(&exp.variants[best_idx])
    }

    /// Record experiment outcome
    pub fn record_outcome(
        &mut self,
        experiment_id: &str,
        variant_id: &str,
        success: bool,
        reward: f64,
    ) {
        if let Some(exp) = self.experiments.get_mut(experiment_id) {
            if let Some(variant) = exp.variants.iter_mut().find(|v| v.id == variant_id) {
                variant.samples += 1;
                if success {
                    variant.successes += 1;
                }
                variant.total_reward += reward;
            }
        }

        // Check if experiment has reached significance
        self.check_significance(experiment_id);
    }

    fn check_significance(&self, experiment_id: &str) -> Option<ExperimentResult> {
        let exp = self.experiments.get(experiment_id)?;

        // Check minimum samples
        if exp.variants.iter().any(|v| v.samples < exp.min_samples_per_variant) {
            return None;
        }

        // Chi-squared test for significance
        let chi_squared = self.calculate_chi_squared(&exp.variants);
        let df = exp.variants.len() - 1;
        let p_value = chi_squared_p_value(chi_squared, df);

        if p_value < (1.0 - exp.statistical_significance) {
            // Find winner
            let winner = exp.variants.iter()
                .max_by(|a, b| {
                    let a_rate = a.total_reward / a.samples.max(1) as f64;
                    let b_rate = b.total_reward / b.samples.max(1) as f64;
                    a_rate.partial_cmp(&b_rate).unwrap()
                })?;

            Some(ExperimentResult {
                experiment_id: experiment_id.to_string(),
                winner_id: winner.id.clone(),
                p_value,
                lift: self.calculate_lift(&exp.variants, &winner.id),
            })
        } else {
            None
        }
    }
}
```

---

## 5. Lightweight Implementations

### 5.1 Memory-Efficient State Representations

**Compact State Encoding:**

```rust
/// Memory-efficient state representation
pub struct CompactState {
    // Use bit-packed representation for discrete states
    discrete_flags: u64,  // 64 boolean states
    // Quantized continuous values
    continuous_values: Vec<i16>,  // 16-bit fixed-point
    // Timestamp
    timestamp: u32,  // Seconds since epoch (32-bit enough for ~136 years)
}

impl CompactState {
    pub fn from_context(context: &Context) -> Self {
        let mut flags = 0u64;

        // Encode boolean states as bits
        if context.window_open { flags |= 1 << 0; }
        if context.hvac_on { flags |= 1 << 1; }
        if context.home_occupied { flags |= 1 << 2; }
        // ... up to 64 boolean states

        // Quantize continuous values to 16-bit fixed-point
        let continuous_values = vec![
            quantize_f64_to_i16(context.pm25, 0.0, 500.0),
            quantize_f64_to_i16(context.temperature, -20.0, 50.0),
            quantize_f64_to_i16(context.humidity, 0.0, 100.0),
            // ... more values
        ];

        Self {
            discrete_flags: flags,
            continuous_values,
            timestamp: context.timestamp.timestamp() as u32,
        }
    }

    pub fn memory_size(&self) -> usize {
        8 + // discrete_flags
        self.continuous_values.len() * 2 + // 2 bytes per i16
        4 // timestamp
    }
}

fn quantize_f64_to_i16(value: f64, min: f64, max: f64) -> i16 {
    let normalized = (value - min) / (max - min);
    let clamped = normalized.clamp(0.0, 1.0);
    (clamped * i16::MAX as f64) as i16
}

fn dequantize_i16_to_f64(value: i16, min: f64, max: f64) -> f64 {
    let normalized = value as f64 / i16::MAX as f64;
    min + normalized * (max - min)
}
```

**Memory Budget Analysis:**

| Component | Bytes per State | 1K States | 10K States |
|-----------|-----------------|-----------|------------|
| Boolean flags | 8 | 8KB | 80KB |
| 10 continuous values | 20 | 20KB | 200KB |
| Timestamp | 4 | 4KB | 40KB |
| **Total** | **32** | **32KB** | **320KB** |

### 5.2 Incremental Policy Updates

```rust
/// Incremental policy update with bounded memory
pub struct IncrementalPolicy {
    // Use hash-based bucketing to limit memory
    state_buckets: Vec<Option<BucketEntry>>,
    n_buckets: usize,
    // Maximum entries before eviction
    max_entries: usize,
    current_entries: usize,
    // LRU tracking for eviction
    lru_list: LinkedList<usize>,
}

struct BucketEntry {
    state_key: StateKey,
    action_values: [f64; MAX_ACTIONS],
    visit_count: u32,
    last_access: Instant,
}

impl IncrementalPolicy {
    pub fn new(n_buckets: usize, max_entries: usize) -> Self {
        Self {
            state_buckets: vec![None; n_buckets],
            n_buckets,
            max_entries,
            current_entries: 0,
            lru_list: LinkedList::new(),
        }
    }

    pub fn get_action_values(&mut self, state: &StateKey) -> &[f64; MAX_ACTIONS] {
        let bucket = self.hash_state(state);

        if let Some(entry) = &mut self.state_buckets[bucket] {
            if entry.state_key == *state {
                entry.last_access = Instant::now();
                return &entry.action_values;
            }
        }

        // Miss - create new entry
        self.ensure_capacity();

        let entry = BucketEntry {
            state_key: state.clone(),
            action_values: [0.0; MAX_ACTIONS],
            visit_count: 0,
            last_access: Instant::now(),
        };

        self.state_buckets[bucket] = Some(entry);
        self.current_entries += 1;

        &self.state_buckets[bucket].as_ref().unwrap().action_values
    }

    pub fn update(&mut self, state: &StateKey, action: usize, delta: f64, alpha: f64) {
        let bucket = self.hash_state(state);

        if let Some(entry) = &mut self.state_buckets[bucket] {
            if entry.state_key == *state {
                // Incremental mean update
                entry.visit_count += 1;
                let n = entry.visit_count as f64;
                entry.action_values[action] += alpha * (delta - entry.action_values[action]);
                entry.last_access = Instant::now();
            }
        }
    }

    fn ensure_capacity(&mut self) {
        if self.current_entries >= self.max_entries {
            // Evict least recently used entry
            let lru_bucket = self.find_lru_bucket();
            self.state_buckets[lru_bucket] = None;
            self.current_entries -= 1;
        }
    }
}
```

### 5.3 When to Use Rules vs Learned Policies

**Decision Matrix:**

| Scenario | Use Rules | Use Learning | Rationale |
|----------|-----------|--------------|-----------|
| Safety-critical actions | Yes | No | Deterministic, auditable |
| Well-understood domain | Yes | Optional | Rules capture expert knowledge |
| Dynamic environment | Partial | Yes | Learning adapts to change |
| Novel situations | Fallback | Yes | Learning generalizes |
| Regulatory compliance | Yes | No | Must be explainable |
| Optimization problems | No | Yes | Learning finds optima |
| Resource-constrained | Yes | Limited | Rules are cheaper |

**Hybrid Architecture:**

```rust
/// Hybrid rule + learning action system
pub struct HybridActionSystem {
    rule_engine: RuleEngine,
    learned_policy: OnlinePolicyLearner,
    arbitrator: Arbitrator,
}

pub struct Arbitrator {
    rule_priority_threshold: f64,
    confidence_threshold: f64,
}

impl HybridActionSystem {
    pub fn select_action(&mut self, context: &Context) -> (Action, ActionSource) {
        // First check rules
        let rule_actions = self.rule_engine.evaluate(context);

        // Check if any high-priority rule fires
        if let Some(action) = rule_actions.iter()
            .find(|a| a.priority() >= self.arbitrator.rule_priority_threshold)
        {
            return (action.clone(), ActionSource::Rule);
        }

        // Check learned policy
        let (learned_action, confidence) = self.learned_policy.select_with_confidence(context);

        if confidence > self.arbitrator.confidence_threshold {
            // Use learned action if confident
            return (learned_action, ActionSource::Learned);
        }

        // Fall back to default rule or no-op
        (
            rule_actions.first().cloned().unwrap_or(Action::NoOp),
            ActionSource::DefaultFallback,
        )
    }
}
```

---

## 6. Home Automation Context

### 6.1 Existing Smart Home Frameworks

**Home Assistant Climate Control:**

*Source: [Home Assistant Climate Integration](https://www.home-assistant.io/integrations/climate/)*

Home Assistant provides a mature automation framework that NDP can learn from:

| Feature | Home Assistant Approach | NDP Adaptation |
|---------|------------------------|----------------|
| Triggers | State changes, time, templates | Event-driven with TimescaleDB |
| Conditions | Boolean logic, templates | Rule engine conditions |
| Actions | Service calls, scripts | Action handlers |
| Learning | Limited (external add-ons) | Built-in with feedback loops |

**Home Assistant Automation Example:**

```yaml
# Home Assistant automation (for reference)
automation:
  - alias: "Ventilate on high CO2"
    trigger:
      - platform: numeric_state
        entity_id: sensor.co2
        above: 1000
        for: "00:05:00"
    condition:
      - condition: state
        entity_id: binary_sensor.window_open
        state: "off"
    action:
      - service: notify.mobile_app
        data:
          message: "CO2 is high. Consider opening windows."
      - service: climate.set_fan_mode
        target:
          entity_id: climate.hvac
        data:
          fan_mode: "high"
```

**NDP Equivalent with Learning:**

```rust
// NDP automation with learning capability
let automation = AutomationRule::builder()
    .name("ventilate_high_co2")
    .trigger(Trigger::NumericState {
        sensor: "co2".into(),
        operator: Operator::Above,
        threshold: 1000.0,
        for_duration: Duration::from_secs(300),
    })
    .condition(Condition::StateEquals {
        entity: "window_open".into(),
        state: "false".into(),
    })
    .action(vec![
        Action::SendNotification {
            message: "CO2 is high. Consider opening windows.".into(),
        },
        Action::SetClimateMode {
            entity: "hvac".into(),
            mode: "high_fan".into(),
        },
    ])
    // NDP enhancement: track outcome
    .track_outcome(OutcomeMetric {
        sensor: "co2".into(),
        expected_direction: Decrease,
        measurement_delay: Duration::from_secs(900),
    })
    // NDP enhancement: learn optimal threshold
    .enable_learning(LearningConfig {
        parameter: "threshold",
        bounds: (800.0, 1200.0),
        learning_rate: 0.1,
    })
    .build();
```

### 6.2 HVAC Control Systems

*Source: [Home Assistant Community - Context-Aware Heating](https://community.home-assistant.io/t/building-context-aware-heating-automation-from-rule-based-to-fully-autonomous-llm-control-using-ai-assisted-design/961850)*

**HVAC Action Types:**

| Action | Reversibility | Latency to Effect | Safety Level |
|--------|---------------|-------------------|--------------|
| Adjust setpoint +/- 1C | High | 5-15 min | Low risk |
| Adjust setpoint +/- 5C | High | 5-15 min | Medium risk |
| Change mode (heat/cool/auto) | High | 1-5 min | Low risk |
| Emergency off | High | Immediate | Safety action |
| Schedule modification | Medium | Hours | Low risk |

**Learning-Enabled HVAC Controller:**

```rust
/// HVAC controller with thermal model learning
pub struct SmartHVACController {
    // Thermal model of the building
    thermal_model: ThermalModel,
    // Occupancy predictor
    occupancy_model: OccupancyModel,
    // Energy cost optimizer
    energy_optimizer: EnergyOptimizer,
    // Comfort preference learner
    comfort_learner: ComfortLearner,
}

pub struct ThermalModel {
    // Heat loss coefficient (learned)
    heat_loss_coefficient: f64,
    // Thermal mass (learned)
    thermal_mass: f64,
    // HVAC heating/cooling capacity (learned)
    hvac_capacity: f64,
}

impl SmartHVACController {
    /// Predict temperature evolution
    pub fn predict_temperature(
        &self,
        current_temp: f64,
        outdoor_temp: f64,
        hvac_power: f64,
        hours_ahead: f64,
    ) -> f64 {
        // Newton's law of cooling + HVAC input
        // dT/dt = -k * (T - T_outdoor) + Q/C
        let k = self.thermal_model.heat_loss_coefficient;
        let c = self.thermal_model.thermal_mass;
        let q = hvac_power * self.thermal_model.hvac_capacity;

        // Analytical solution for constant conditions
        let t_eq = outdoor_temp + q / (k * c);
        let delta = current_temp - t_eq;
        let predicted = t_eq + delta * (-k * hours_ahead).exp();

        predicted
    }

    /// Learn thermal parameters from observations
    pub fn update_thermal_model(
        &mut self,
        observations: &[(f64, f64, f64, f64)], // (temp, outdoor_temp, hvac_power, time_delta)
    ) {
        // Least squares fit for thermal parameters
        // This is simplified - real implementation would use Kalman filter or gradient descent
        let (k, c, capacity) = fit_thermal_parameters(observations);

        // Update with exponential smoothing
        let alpha = 0.1;
        self.thermal_model.heat_loss_coefficient =
            alpha * k + (1.0 - alpha) * self.thermal_model.heat_loss_coefficient;
        self.thermal_model.thermal_mass =
            alpha * c + (1.0 - alpha) * self.thermal_model.thermal_mass;
        self.thermal_model.hvac_capacity =
            alpha * capacity + (1.0 - alpha) * self.thermal_model.hvac_capacity;
    }

    /// Optimize HVAC schedule for comfort and energy
    pub fn optimize_schedule(
        &self,
        current_temp: f64,
        outdoor_forecast: &[(DateTime<Utc>, f64)],
        occupancy_forecast: &[(DateTime<Utc>, bool)],
        energy_prices: &[(DateTime<Utc>, f64)],
        target_comfort: f64,
    ) -> Vec<(DateTime<Utc>, HVACSetpoint)> {
        // Simple greedy optimization for edge
        let mut schedule = Vec::new();

        for (time, outdoor_temp) in outdoor_forecast {
            let occupied = occupancy_forecast.iter()
                .find(|(t, _)| t == time)
                .map(|(_, o)| *o)
                .unwrap_or(false);

            let energy_price = energy_prices.iter()
                .find(|(t, _)| t == time)
                .map(|(_, p)| *p)
                .unwrap_or(1.0);

            let setpoint = if occupied {
                // Optimize for comfort
                target_comfort
            } else {
                // Allow wider band when unoccupied
                if energy_price > 1.5 {
                    // High price: aggressive setback
                    target_comfort - 4.0
                } else {
                    // Normal price: moderate setback
                    target_comfort - 2.0
                }
            };

            schedule.push((*time, HVACSetpoint { temperature: setpoint }));
        }

        schedule
    }
}
```

### 6.3 Window Management Actions

**Window State Actions:**

| Action | Prerequisites | Effect Delay | Reversal |
|--------|---------------|--------------|----------|
| Suggest open window | outdoor_aq < indoor_aq, temp OK | N/A | N/A |
| Suggest close window | outdoor_aq > indoor_aq or rain | N/A | N/A |
| Auto-close (with actuator) | rain detected, window open | Immediate | Auto or manual |

### 6.4 Alert and Notification Actions

**Alert Severity Framework:**

```rust
pub enum AlertLevel {
    Info,       // FYI, no action needed
    Suggestion, // Recommended action
    Warning,    // Should address soon
    Critical,   // Immediate attention
    Emergency,  // Safety issue
}

pub struct AlertConfig {
    pub level: AlertLevel,
    pub channels: Vec<NotificationChannel>,
    pub cooldown: Duration,
    pub escalation: Option<EscalationPolicy>,
}

pub enum NotificationChannel {
    PushNotification { app: String },
    Email { address: String },
    Webhook { url: String },
    LocalDisplay,
    HomeAssistant { service: String },
}
```

---

## 7. Integration with Causal Models

### 7.1 Causal-Informed Actions

Integrating causal models (from parallel research) with action frameworks.

```rust
/// Action selection informed by causal models
pub struct CausalActionSelector {
    causal_graph: CausalGraph,
    action_effect_estimates: HashMap<ActionType, CausalEffect>,
    intervention_history: Vec<Intervention>,
}

pub struct CausalEffect {
    pub treatment: ActionType,
    pub outcome: String,
    pub ate: f64,  // Average Treatment Effect
    pub confidence_interval: (f64, f64),
    pub n_observations: u32,
}

impl CausalActionSelector {
    /// Select action based on estimated causal effects
    pub fn select_action(
        &self,
        context: &Context,
        goal: &Goal,
    ) -> Option<(ActionType, f64)> {
        let candidate_actions = self.get_candidate_actions(context);

        // Score each action by estimated causal effect on goal
        let scored: Vec<(ActionType, f64)> = candidate_actions.iter()
            .filter_map(|action| {
                let effect = self.action_effect_estimates.get(action)?;
                if effect.outcome == goal.target_metric {
                    // Adjust for confidence
                    let adjusted_effect = effect.ate * effect.confidence();
                    Some((action.clone(), adjusted_effect))
                } else {
                    None
                }
            })
            .collect();

        // Return action with highest positive effect
        scored.into_iter()
            .filter(|(_, effect)| *effect > 0.0)
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
    }

    /// Update causal effect estimates from observation
    pub fn update_from_observation(
        &mut self,
        action: &ActionType,
        context_before: &Context,
        context_after: &Context,
        outcome_metric: &str,
    ) {
        let observed_effect = context_after.get_metric(outcome_metric)
            - context_before.get_metric(outcome_metric);

        // Adjust for confounders using causal graph
        let adjusted_effect = self.adjust_for_confounders(
            action,
            observed_effect,
            context_before,
        );

        // Update effect estimate
        let effect = self.action_effect_estimates
            .entry(action.clone())
            .or_insert_with(|| CausalEffect::default());

        // Incremental update
        effect.n_observations += 1;
        let n = effect.n_observations as f64;
        effect.ate = effect.ate + (adjusted_effect - effect.ate) / n;

        // Update confidence interval (simplified)
        let se = effect.ate.abs() / n.sqrt();
        effect.confidence_interval = (effect.ate - 1.96 * se, effect.ate + 1.96 * se);
    }

    /// Apply backdoor adjustment for confounders
    fn adjust_for_confounders(
        &self,
        action: &ActionType,
        observed_effect: f64,
        context: &Context,
    ) -> f64 {
        // Identify confounders from causal graph
        let confounders = self.causal_graph.get_confounders(action);

        if confounders.is_empty() {
            return observed_effect;
        }

        // Simple adjustment: stratify by confounder values and average
        // (In practice, would use more sophisticated methods like IPW)
        observed_effect  // Placeholder - full implementation would adjust
    }
}
```

### 7.2 Intervention Planning with Causal Reasoning

```rust
/// Plan interventions using causal reasoning
pub struct CausalInterventionPlanner {
    causal_graph: CausalGraph,
    effect_estimates: HashMap<(String, String), f64>,
}

impl CausalInterventionPlanner {
    /// Find best intervention to achieve goal
    pub fn plan_intervention(
        &self,
        current_state: &HashMap<String, f64>,
        goal: &Goal,
    ) -> Option<Intervention> {
        // Find all variables that causally affect goal target
        let parents = self.causal_graph.get_ancestors(&goal.target_metric);

        // For each potential intervention point
        let mut best_intervention = None;
        let mut best_expected_effect = 0.0;

        for parent in parents {
            // Get estimated effect of intervening on parent
            let effect = self.effect_estimates
                .get(&(parent.clone(), goal.target_metric.clone()))
                .copied()
                .unwrap_or(0.0);

            // Calculate intervention magnitude needed
            let current_value = current_state.get(&goal.target_metric).copied().unwrap_or(0.0);
            let gap = goal.target_value - current_value;

            if effect != 0.0 {
                let intervention_magnitude = gap / effect;

                // Check if intervention is feasible
                if self.is_feasible_intervention(&parent, intervention_magnitude) {
                    let expected_effect = effect * intervention_magnitude;
                    if expected_effect.abs() > best_expected_effect.abs() {
                        best_expected_effect = expected_effect;
                        best_intervention = Some(Intervention {
                            target: parent.clone(),
                            magnitude: intervention_magnitude,
                            expected_effect,
                        });
                    }
                }
            }
        }

        best_intervention
    }
}
```

---

## 8. Recommended Approach for NDP

### 8.1 Architecture Recommendation

Based on the research, NDP should implement a **Tiered Hybrid Architecture**:

```
┌─────────────────────────────────────────────────────────────────┐
│                NDP ACTION FRAMEWORK ARCHITECTURE                 │
│                                                                  │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │  TIER 0: SAFETY LAYER (Always Active)                   │    │
│  │  - Hard constraints (physical limits, data freshness)   │    │
│  │  - Kill switch                                          │    │
│  │  - Audit logging                                        │    │
│  │  Memory: <1MB | Latency: <0.1ms                         │    │
│  └─────────────────────────────────────────────────────────┘    │
│                              │                                   │
│                              ▼                                   │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │  TIER 1: RULE ENGINE (Default Handler)                  │    │
│  │  - YAML-defined rules                                   │    │
│  │  - Threshold-based triggers                             │    │
│  │  - Cooldown management                                  │    │
│  │  Memory: 1-5MB | Latency: <1ms                          │    │
│  └─────────────────────────────────────────────────────────┘    │
│                              │                                   │
│                              ▼                                   │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │  TIER 2: BEHAVIOR TREE (Complex Actions)                │    │
│  │  - Modular behavior composition                         │    │
│  │  - Fallback handling                                    │    │
│  │  - Reactive response                                    │    │
│  │  Memory: 5-20MB | Latency: 1-5ms                        │    │
│  └─────────────────────────────────────────────────────────┘    │
│                              │                                   │
│                              ▼                                   │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │  TIER 3: LEARNING LAYER (Optimization)                  │    │
│  │  - Online policy learning                               │    │
│  │  - A/B testing                                          │    │
│  │  - Causal effect estimation                             │    │
│  │  Memory: 50-200MB | Latency: 10-50ms                    │    │
│  └─────────────────────────────────────────────────────────┘    │
│                              │                                   │
│                              ▼                                   │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │  TIER 4: HUMAN-IN-THE-LOOP (Approval)                   │    │
│  │  - High-stakes decisions                                │    │
│  │  - Novel situations                                     │    │
│  │  - Policy changes                                       │    │
│  │  Memory: <1MB | Latency: Human-dependent                │    │
│  └─────────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────────┘
```

### 8.2 Implementation Phases

#### Phase 1: Foundation (Weeks 1-4)

| Component | Priority | Effort | Deliverable |
|-----------|----------|--------|-------------|
| Safety constraint layer | Critical | Medium | `SafetyManager` with hard/soft constraints |
| Rule engine | Critical | Low | YAML-based rules with cooldown |
| Action executor | Critical | Medium | Action handlers with audit logging |
| Basic alerts | High | Low | Notification system |

**Exit Criteria:**
- Safety constraints operational
- 10+ rules for air quality scenarios
- Full audit logging
- <5ms total decision latency

#### Phase 2: Behavior Trees (Weeks 5-8)

| Component | Priority | Effort | Deliverable |
|-----------|----------|--------|-------------|
| BT runtime | High | Medium | Lightweight behavior tree engine |
| Air quality BT | High | Medium | Modular AQ management tree |
| HVAC BT | Medium | Medium | Smart HVAC control tree |
| BT visualization | Low | Low | Debug/monitor interface |

**Exit Criteria:**
- Behavior trees for main scenarios
- Fallback handling operational
- <10ms decision latency

#### Phase 3: Learning (Weeks 9-12)

| Component | Priority | Effort | Deliverable |
|-----------|----------|--------|-------------|
| Outcome tracking | High | Medium | Action outcome measurement |
| Policy learner | High | Medium | Online policy improvement |
| A/B testing | Medium | Medium | Thompson sampling experiments |
| Counterfactual | Medium | High | Effect estimation |

**Exit Criteria:**
- Actions track outcomes
- Policy improves over time
- A/B tests for action variants

#### Phase 4: Causal Integration (Weeks 13-16)

| Component | Priority | Effort | Deliverable |
|-----------|----------|--------|-------------|
| Causal action selector | Medium | High | Effect-aware action selection |
| Intervention planner | Medium | High | Goal-directed planning |
| Human approval system | High | Medium | Approval workflows |

**Exit Criteria:**
- Causal reasoning influences action selection
- Human-in-the-loop operational
- Full feedback loop closed

### 8.3 Resource Budget

| Component | Memory | CPU | Notes |
|-----------|--------|-----|-------|
| Safety layer | 1MB | <1% | Always active |
| Rule engine | 5MB | <1% | 1000 rules |
| Behavior trees | 20MB | 2% | Complex scenarios |
| Learning layer | 200MB | 10% | Policy + history |
| Outcome tracking | 50MB | 5% | Measurement queue |
| **Total** | **~280MB** | **~18%** | Within Pi 5 budget |

**Remaining from NDP Gold Layer budget (~2.5GB):** ~2.2GB

### 8.4 Configuration Schema

```yaml
# config/actions/action-framework.yaml
action_framework:
  enabled: true

  safety:
    hard_constraints:
      - name: "data_freshness"
        max_age_seconds: 30
        violation_action: "block"
      - name: "hvac_temp_limit"
        min_setpoint: 15
        max_setpoint: 30
        violation_action: "block_and_alert"

    soft_constraints:
      - name: "action_rate_limit"
        max_actions_per_hour: 10
        violation_policy: "rate_limited"
      - name: "confidence_threshold"
        min_confidence: 0.7
        violation_policy: "require_confirmation"

  rule_engine:
    rules_path: "config/rules/"
    default_cooldown: "30m"
    evaluation_interval: "1s"

  behavior_trees:
    enabled: true
    trees_path: "config/behavior_trees/"
    tick_rate_ms: 100

  learning:
    enabled: true
    policy_store_path: "/data/policies"
    learning_rate: 0.1
    exploration_rate: 0.1
    min_exploration: 0.05
    outcome_measurement_delays:
      - 5m
      - 30m
      - 2h

  ab_testing:
    enabled: true
    allocation_strategy: "thompson"
    min_samples_per_variant: 50
    significance_threshold: 0.95

  human_in_the_loop:
    enabled: true
    timeout: "30m"
    fallback_policy: "reject_on_timeout"
    notification_channels:
      - type: "webhook"
        url: "${NOTIFICATION_WEBHOOK_URL}"
      - type: "home_assistant"
        service: "notify.mobile_app"

  audit:
    enabled: true
    log_path: "/data/audit/actions.log"
    retention_days: 90
```

---

## 9. Summary and Conclusions

### 9.1 Key Takeaways

1. **Hybrid Architecture is Optimal:** Combining rules (safety, simplicity) with behavior trees (modularity) and learning (optimization) provides the best balance for edge deployment.

2. **Safety First:** Multi-layered safety constraints with human-in-the-loop for high-stakes decisions is essential for autonomous action systems.

3. **Lightweight Learning Works:** Online policy improvement with bandits and simple gradient-free methods is feasible on Raspberry Pi with <200MB memory overhead.

4. **Feedback Loops are Critical:** Tracking action outcomes and learning from them transforms a reactive system into an adaptive one.

5. **Behavior Trees Over GOAP:** For edge devices, behavior trees provide the right balance of flexibility and efficiency. GOAP is too computationally expensive for real-time edge use.

6. **Causal Integration Adds Value:** Informing actions with causal effect estimates improves decision quality beyond simple correlation-based approaches.

### 9.2 Comparison with Existing Systems

| System | Approach | Learning | Edge Suitable |
|--------|----------|----------|---------------|
| Home Assistant | Rule-based | No | Yes |
| Node-RED | Flow-based | No | Yes |
| OpenHAB | Rule + state | No | Yes |
| **NDP (Proposed)** | **Hybrid + Learning** | **Yes** | **Yes** |

### 9.3 Next Steps

1. **Implement Safety Layer:** Start with hard constraints and audit logging
2. **Build Rule Engine:** YAML-based rules for common scenarios
3. **Prototype Behavior Trees:** Air quality management as first use case
4. **Add Outcome Tracking:** Measure action effects
5. **Enable Learning:** Policy improvement with Thompson sampling
6. **Integrate Causal Models:** Use causal research for effect-aware selection

---

## 10. References

### Action Framework Architectures
- [Polymath Robotics - State Machines vs Behavior Trees](https://www.polymathrobotics.com/blog/state-machines-vs-behavior-trees)
- [IEEE TSE - Behavior Trees and State Machines in Robotics](https://dl.acm.org/doi/abs/10.1109/TSE.2023.3269081)
- [ScienceDirect - Survey of Behavior Trees in Robotics and AI](https://www.sciencedirect.com/science/article/pii/S0921889022000513)
- [Nez Framework - AI Documentation (FSM, BT, GOAP, Utility AI)](https://anshuman-kumar.gitbook.io/nez-doc/ai-fsm-behavior-tree-goap-utility-ai)

### Edge AI and Agentic Systems
- [Dell - Edge AI Predictions for 2026](https://www.dell.com/en-us/blog/the-power-of-small-edge-ai-predictions-for-2026/)
- [IEEE - Toward Edge General Intelligence with Agentic AI](https://ieeexplore.ieee.org/iel8/9739/11321210/11339915.pdf)
- [ScienceDirect - Edge-enabled Smart Agriculture Framework](https://www.sciencedirect.com/science/article/pii/S2590123025033973)
- [Instaclustr - Agentic AI Frameworks 2026](https://www.instaclustr.com/education/agentic-ai/agentic-ai-frameworks-top-8-options-in-2026/)

### Safety and Guardrails
- [CNCF - Autonomous Enterprise and Platform Control](https://www.cncf.io/blog/2026/01/23/the-autonomous-enterprise-and-the-four-pillars-of-platform-control-2026-forecast/)
- [BigID - AEGIS Guardrails for Autonomous AI](https://bigid.com/blog/what-is-aegis/)
- [Medium - Defense-in-Depth Guardrails for Agentic AI](https://ssahuupgrad-93226.medium.com/building-production-ready-guardrails-for-agentic-ai-a-defense-in-depth-framework-4ab7151be1fe)
- [Frontier Enterprise - AI Agent Autonomy Needs Human Control](https://www.frontier-enterprise.com/ai-agent-autonomy-needs-human-control-and-guardrails/)
- [Machine Learning Mastery - Agentic AI Trends 2026](https://machinelearningmastery.com/7-agentic-ai-trends-to-watch-in-2026/)

### Reinforcement Learning and Policy Optimization
- [Nature - DRL Task Offloading in MEC](https://www.nature.com/articles/s41598-024-84038-3)
- [Springer - RL-based Edge Orchestration](https://link.springer.com/article/10.1007/s11227-025-07830-6)
- [Nature - Federated RL for Edge IoT Security](https://www.nature.com/articles/s41598-025-34879-3)

### POMDPs and Decision Under Uncertainty
- [Annual Reviews - POMDPs and Robotics](https://www.annualreviews.org/content/journals/10.1146/annurev-control-042920-092451)
- [arXiv - POMDPs in Robotics](https://arxiv.org/pdf/2209.10342)
- [arXiv - MEMBOT: Memory-Based Robot in Intermittent POMDP](https://arxiv.org/html/2509.11225)

### Causal Reasoning
- [arXiv - Compressed Causal Reasoning](https://arxiv.org/abs/2512.13725)
- [Wiley - Causal Learning Through Graph Neural Networks](https://wires.onlinelibrary.wiley.com/doi/10.1002/widm.70024)
- [MDPI - Causal Intervention and Counterfactual Reasoning](https://www.mdpi.com/2313-433X/11/11/379)

### Home Automation
- [Home Assistant - Climate Integration](https://www.home-assistant.io/integrations/climate/)
- [Home Assistant - Roadmap 2025](https://www.home-assistant.io/blog/2025/05/09/roadmap-2025h1/)
- [Home Assistant Community - Context-Aware Heating Automation](https://community.home-assistant.io/t/building-context-aware-heating-automation-from-rule-based-to-fully-autonomous-llm-control-using-ai-assisted-design/961850)

### NDP Internal Research
- [Self-Learning and Adaptive Systems](/workspaces/neural-data-platform/product/research/gold/self-learning/ADAPTIVE-SYSTEMS.md)
- [Home Assistant Integration](/workspaces/neural-data-platform/product/research/homeassistant/README.md)
- [Edge ML Deployment Strategies](/workspaces/neural-data-platform/product/research/gold/edge-ml/DEPLOYMENT-STRATEGIES.md)
- [Gold Layer Master Synthesis](/workspaces/neural-data-platform/product/research/gold/MASTER-SYNTHESIS.md)

---

*Research conducted for Neural Data Platform Autonomous Edge Action Capabilities*
