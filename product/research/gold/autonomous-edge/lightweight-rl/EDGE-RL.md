# Lightweight Reinforcement Learning for Edge Devices

**Research Date:** 2026-02-02
**Platform:** Raspberry Pi 4/5 (ARM Cortex-A72/A76, 4-8GB RAM)
**Context:** Neural Data Platform - Autonomous air quality management
**Status:** Research Complete

---

## Executive Summary

This research evaluates reinforcement learning algorithms for deployment on resource-constrained edge devices, specifically targeting the Raspberry Pi for autonomous air quality management. The analysis covers algorithm feasibility, sample efficiency techniques, reward design patterns, and safe exploration strategies.

### Key Findings

| Algorithm Class | Pi Feasibility | Memory | Latency | Recommended Use Case |
|-----------------|----------------|--------|---------|---------------------|
| **Multi-Armed Bandits** | Excellent | <1MB | <1ms | Action selection (window/HVAC modes) |
| **Contextual Bandits** | Excellent | 1-10MB | <5ms | Context-aware decisions |
| **Tabular Q-Learning** | High | 1-50MB | <10ms | Discrete state-action spaces |
| **SARSA** | High | 1-50MB | <10ms | On-policy conservative learning |
| **Linear Function Approx** | High | 10-100MB | <20ms | Continuous states, discrete actions |
| **DQN (Quantized)** | Medium | 50-200MB | 20-100ms | Complex pattern recognition |
| **Actor-Critic (Lightweight)** | Medium | 100-500MB | 50-200ms | Continuous action spaces |
| **PPO** | Low-Medium | 200MB-1GB | 100-500ms | Complex policies (prefer offline training) |
| **Model-Based (Dyna-Q)** | Medium | 50-200MB | 10-50ms | Sample-efficient learning |

### Critical Architecture Decision

**Hierarchical RL with Bandit Gateway** is recommended:
- **Top Level**: Contextual bandit selects operating mode (comfort, eco, purge, sleep)
- **Mid Level**: Tabular Q-learning optimizes within-mode actions
- **Bottom Level**: Rule-based safety constraints (hard limits)

---

## 1. RL Algorithms by Complexity

### 1.1 Multi-Armed Bandits (MAB)

The simplest RL formulation, ideal for edge devices with minimal memory.

#### UCB1 (Upper Confidence Bound)

**Algorithm:**
```rust
/// UCB1 Bandit for action selection
pub struct UCB1Bandit {
    counts: Vec<u32>,      // Times each action taken
    values: Vec<f64>,      // Average reward per action
    total_count: u32,
}

impl UCB1Bandit {
    pub fn select_action(&self) -> usize {
        // UCB1: exploitation + exploration bonus
        self.values.iter()
            .enumerate()
            .map(|(i, &value)| {
                if self.counts[i] == 0 {
                    return f64::MAX;  // Explore untried actions
                }
                let exploration = (2.0 * (self.total_count as f64).ln()
                                   / self.counts[i] as f64).sqrt();
                value + exploration
            })
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
            .map(|(i, _)| i)
            .unwrap()
    }

    pub fn update(&mut self, action: usize, reward: f64) {
        self.counts[action] += 1;
        self.total_count += 1;
        // Incremental mean update
        self.values[action] += (reward - self.values[action])
                               / self.counts[action] as f64;
    }
}
```

**Memory:** O(k) where k = number of actions (typically 4-10)
**Latency:** O(k) - microseconds
**Pi Feasibility:** Excellent

**NDP Use Case:** Select HVAC mode (off, low, medium, high, purge)

#### Thompson Sampling

**Algorithm:**
```rust
/// Thompson Sampling with Beta prior (for Bernoulli rewards)
pub struct ThompsonSamplingBandit {
    successes: Vec<u32>,   // Alpha parameters
    failures: Vec<u32>,    // Beta parameters
}

impl ThompsonSamplingBandit {
    pub fn select_action(&self) -> usize {
        // Sample from Beta(alpha, beta) for each arm
        let samples: Vec<f64> = self.successes.iter()
            .zip(&self.failures)
            .map(|(&s, &f)| {
                sample_beta(s as f64 + 1.0, f as f64 + 1.0)
            })
            .collect();

        samples.iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
            .map(|(i, _)| i)
            .unwrap()
    }

    pub fn update(&mut self, action: usize, reward: bool) {
        if reward {
            self.successes[action] += 1;
        } else {
            self.failures[action] += 1;
        }
    }
}
```

**Advantages over UCB:**
- Better performance with delayed feedback (common in HVAC)
- Natural uncertainty quantification
- Handles non-stationary rewards better

**Reference:** [Thompson Sampling - Wikipedia](https://en.wikipedia.org/wiki/Thompson_sampling)

### 1.2 Contextual Bandits

Extends MAB with context/state information without full MDP modeling.

#### LinUCB (Linear UCB)

**Algorithm:**
```rust
/// LinUCB for contextual decisions
pub struct LinUCB {
    // Per-action parameters
    a_matrices: Vec<DMatrix<f64>>,  // d x d matrices
    b_vectors: Vec<DVector<f64>>,   // d x 1 vectors
    alpha: f64,                      // Exploration parameter
}

impl LinUCB {
    pub fn select_action(&self, context: &DVector<f64>) -> usize {
        self.a_matrices.iter()
            .zip(&self.b_vectors)
            .enumerate()
            .map(|(i, (a, b))| {
                // theta = A^-1 * b
                let a_inv = a.clone().try_inverse().unwrap();
                let theta = &a_inv * b;

                // UCB: theta^T * x + alpha * sqrt(x^T * A^-1 * x)
                let exploitation = theta.dot(context);
                let exploration = self.alpha * (context.dot(&(&a_inv * context))).sqrt();

                (i, exploitation + exploration)
            })
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
            .map(|(i, _)| i)
            .unwrap()
    }

    pub fn update(&mut self, action: usize, context: &DVector<f64>, reward: f64) {
        // A = A + x * x^T
        self.a_matrices[action] += context * context.transpose();
        // b = b + r * x
        self.b_vectors[action] += reward * context;
    }
}
```

**Memory:** O(k * d^2) where d = context dimension
**Typical d:** 10-50 features (hour, day, temperature, humidity, occupancy, etc.)
**Pi Feasibility:** Excellent for d < 100

**NDP Use Case:** Select ventilation action based on current conditions
- Context: [hour, day_of_week, outdoor_temp, indoor_co2, pm25, occupancy, ...]
- Actions: [do_nothing, increase_vent, decrease_vent, open_window, close_window]

**Reference:** [Contextual Bandits Documentation](https://contextual-bandits.readthedocs.io/)

### 1.3 Tabular Q-Learning

Full MDP with discrete states and actions, stored in lookup table.

**Algorithm:**
```rust
/// Tabular Q-Learning agent
pub struct TabularQLearning {
    q_table: HashMap<(State, Action), f64>,
    learning_rate: f64,     // alpha: 0.1-0.3
    discount_factor: f64,   // gamma: 0.9-0.99
    epsilon: f64,           // exploration: 0.1-0.3
}

#[derive(Hash, Eq, PartialEq, Clone)]
pub struct State {
    // Discretized state representation
    co2_level: u8,        // 0-4 (good, moderate, poor, very_poor, hazardous)
    pm25_level: u8,       // 0-4
    temp_comfort: u8,     // 0-2 (cold, comfortable, hot)
    humidity_level: u8,   // 0-2 (dry, comfortable, humid)
    time_bucket: u8,      // 0-23 (hour of day)
    occupancy: u8,        // 0-2 (empty, light, full)
}

impl TabularQLearning {
    pub fn select_action(&self, state: &State) -> Action {
        if rand::random::<f64>() < self.epsilon {
            // Explore: random action
            Action::random()
        } else {
            // Exploit: best known action
            self.best_action(state)
        }
    }

    pub fn update(
        &mut self,
        state: State,
        action: Action,
        reward: f64,
        next_state: State
    ) {
        let current_q = *self.q_table.get(&(state.clone(), action.clone()))
                                     .unwrap_or(&0.0);
        let max_next_q = self.max_q_value(&next_state);

        // Q-learning update rule
        let new_q = current_q + self.learning_rate * (
            reward + self.discount_factor * max_next_q - current_q
        );

        self.q_table.insert((state, action), new_q);
    }

    fn max_q_value(&self, state: &State) -> f64 {
        Action::all().iter()
            .map(|a| *self.q_table.get(&(state.clone(), a.clone())).unwrap_or(&0.0))
            .fold(f64::NEG_INFINITY, f64::max)
    }
}
```

**Memory Analysis:**
- States: 5 * 5 * 3 * 3 * 24 * 3 = 16,200 states
- Actions: 5 actions
- Q-values: 16,200 * 5 = 81,000 entries
- Memory: 81,000 * 8 bytes = **648 KB**

**Pi Feasibility:** High - easily fits in memory

**When to Use:**
- Discrete, well-defined state space
- <100K state-action pairs
- Faster convergence than function approximation

### 1.4 SARSA (State-Action-Reward-State-Action)

On-policy variant of Q-learning - more conservative, follows the policy it's learning.

**Algorithm:**
```rust
impl SARSA {
    pub fn update(
        &mut self,
        state: State,
        action: Action,
        reward: f64,
        next_state: State,
        next_action: Action,  // Key difference from Q-learning
    ) {
        let current_q = *self.q_table.get(&(state.clone(), action.clone()))
                                     .unwrap_or(&0.0);
        // Uses Q(s', a') instead of max_a Q(s', a)
        let next_q = *self.q_table.get(&(next_state.clone(), next_action.clone()))
                                  .unwrap_or(&0.0);

        let new_q = current_q + self.learning_rate * (
            reward + self.discount_factor * next_q - current_q
        );

        self.q_table.insert((state, action), new_q);
    }
}
```

**SARSA vs Q-Learning:**

| Aspect | Q-Learning | SARSA |
|--------|------------|-------|
| Policy | Off-policy | On-policy |
| Optimism | Optimistic (max Q) | Realistic (actual next action) |
| Safety | Riskier exploration | Safer, accounts for exploration |
| Convergence | To optimal policy | To policy being followed |
| Best for | Simulation-based training | Real-world with safety concerns |

**NDP Recommendation:** SARSA for online learning (safer), Q-learning for offline training

### 1.5 Linear Function Approximation

When state space is continuous, use linear approximation instead of tables.

**Algorithm:**
```rust
/// Linear Q-function approximation
pub struct LinearQ {
    weights: HashMap<Action, DVector<f64>>,  // Weights per action
    learning_rate: f64,
    discount_factor: f64,
    epsilon: f64,
}

impl LinearQ {
    pub fn q_value(&self, features: &DVector<f64>, action: &Action) -> f64 {
        self.weights.get(action)
            .map(|w| w.dot(features))
            .unwrap_or(0.0)
    }

    pub fn update(
        &mut self,
        features: DVector<f64>,
        action: Action,
        reward: f64,
        next_features: DVector<f64>,
    ) {
        let current_q = self.q_value(&features, &action);
        let max_next_q = self.max_q_value(&next_features);

        let td_error = reward + self.discount_factor * max_next_q - current_q;

        // Gradient descent update
        let weights = self.weights.entry(action).or_insert_with(||
            DVector::zeros(features.len())
        );
        *weights += self.learning_rate * td_error * &features;
    }
}
```

**Feature Engineering for Air Quality:**
```rust
fn extract_features(state: &SensorState) -> DVector<f64> {
    DVector::from_vec(vec![
        // Raw normalized values
        state.co2_ppm / 2000.0,
        state.pm25_ugm3 / 100.0,
        state.temperature_c / 40.0,
        state.humidity_percent / 100.0,

        // Temporal features
        (state.hour as f64 / 24.0).sin(),  // Circular encoding
        (state.hour as f64 / 24.0).cos(),
        (state.day_of_week as f64 / 7.0).sin(),
        (state.day_of_week as f64 / 7.0).cos(),

        // Derived features
        state.co2_rate_of_change / 100.0,  // PPM/min
        state.pm25_rate_of_change / 10.0,

        // Context
        state.outdoor_temp_c / 40.0,
        state.outdoor_aqi / 200.0,
        state.occupancy_estimate / 10.0,

        // Interaction terms
        state.co2_ppm * state.occupancy_estimate / 20000.0,

        // Bias term
        1.0,
    ])
}
```

**Memory:** O(d * k) where d = features, k = actions
**Pi Feasibility:** High - 15 features * 5 actions = 75 weights = 600 bytes

### 1.6 Deep Q-Network (DQN) - Lightweight Variants

For complex pattern recognition, but requires careful optimization for edge.

**TinyDQN Architecture:**
```rust
/// Lightweight DQN for edge deployment
pub struct TinyDQN {
    // Small network: 15 -> 32 -> 16 -> 5
    layer1: Linear<32, 15>,   // Input features
    layer2: Linear<16, 32>,   // Hidden
    output: Linear<5, 16>,    // Action Q-values
    target_network: Option<Box<TinyDQN>>,  // For stable learning

    replay_buffer: ReplayBuffer,
    batch_size: usize,        // 16-32 for edge
    target_update_freq: u32,  // Update target every N steps
}

impl TinyDQN {
    pub fn forward(&self, state: &[f32; 15]) -> [f32; 5] {
        let x = self.layer1.forward(state);
        let x = relu(&x);
        let x = self.layer2.forward(&x);
        let x = relu(&x);
        self.output.forward(&x)
    }

    pub fn train_step(&mut self) {
        let batch = self.replay_buffer.sample(self.batch_size);

        for (state, action, reward, next_state, done) in batch {
            let current_q = self.forward(&state)[action];
            let target_q = if done {
                reward
            } else {
                let target = self.target_network.as_ref().unwrap();
                reward + 0.99 * target.forward(&next_state).iter().cloned()
                                      .fold(f32::NEG_INFINITY, f32::max)
            };

            // Backprop with TD error
            self.backprop(state, action, target_q - current_q);
        }
    }
}
```

**Model Size Analysis:**
- Layer 1: 15 * 32 + 32 = 512 params
- Layer 2: 32 * 16 + 16 = 528 params
- Output: 16 * 5 + 5 = 85 params
- Total: 1,125 params * 4 bytes = **4.5 KB**

**With INT8 Quantization:** 1.1 KB

**Pi Feasibility:** Medium - inference is fast, training requires care

**Reference:** [TinyRL: Towards Reinforcement Learning on Tiny Embedded Devices](https://dl.acm.org/doi/abs/10.1145/3511808.3557206)

### 1.7 Actor-Critic Methods

For continuous action spaces (e.g., set exact temperature setpoint).

**Lightweight Actor-Critic:**
```rust
/// Advantage Actor-Critic (A2C) for edge
pub struct LightweightA2C {
    // Actor: policy network
    actor: SmallMLP<15, 32, 1>,    // Outputs action mean
    actor_log_std: f32,            // Fixed log std for simplicity

    // Critic: value network
    critic: SmallMLP<15, 32, 1>,   // Outputs state value

    learning_rate: f32,
    discount_factor: f32,
}

impl LightweightA2C {
    pub fn select_action(&self, state: &[f32; 15]) -> f32 {
        let mean = self.actor.forward(state)[0];
        let std = self.actor_log_std.exp();

        // Sample from Gaussian
        let noise = rand_normal() * std;
        (mean + noise).clamp(-1.0, 1.0)  // Normalized action
    }

    pub fn update(
        &mut self,
        state: [f32; 15],
        action: f32,
        reward: f32,
        next_state: [f32; 15],
        done: bool,
    ) {
        let value = self.critic.forward(&state)[0];
        let next_value = if done {
            0.0
        } else {
            self.critic.forward(&next_state)[0]
        };

        // Advantage = TD error
        let advantage = reward + self.discount_factor * next_value - value;

        // Update critic (minimize TD error)
        self.critic.backprop_mse(&state, value + self.learning_rate * advantage);

        // Update actor (policy gradient)
        let action_mean = self.actor.forward(&state)[0];
        let log_prob = -0.5 * ((action - action_mean) / self.actor_log_std.exp()).powi(2);
        self.actor.backprop_policy_gradient(&state, advantage * log_prob);
    }
}
```

**Use Case:** Continuous setpoint control
- Action: Temperature setpoint adjustment (-2.0 to +2.0 degrees)
- Action: Ventilation rate (0.0 to 1.0 normalized)

**Memory:** ~10-50 KB for small networks
**Pi Feasibility:** Medium - requires more compute than bandits/Q-learning

### 1.8 PPO (Proximal Policy Optimization)

State-of-the-art for complex policies, but heavy for edge.

**Recommendation:** Train offline, deploy frozen policy

**Lightweight PPO Strategy:**
```rust
/// PPO with fixed policy for inference-only deployment
pub struct FrozenPPOPolicy {
    actor: QuantizedMLP,   // INT8 quantized
    // No critic needed for inference
}

impl FrozenPPOPolicy {
    pub fn load(path: &str) -> Self {
        // Load pre-trained, quantized weights
        let weights = load_safetensors(path);
        Self { actor: QuantizedMLP::from_weights(weights) }
    }

    pub fn select_action(&self, state: &[f32]) -> Action {
        let logits = self.actor.forward(state);
        // Deterministic: argmax for deployment
        // Or sample for exploration during fine-tuning
        Action::from_index(argmax(&logits))
    }
}
```

**Workflow:**
1. Train PPO in simulation/cloud (PyTorch/stable-baselines3)
2. Quantize model (INT8)
3. Export to ONNX or safetensors
4. Deploy inference-only on Pi
5. Optional: Periodic fine-tuning with bandit wrapper

**Reference:** [From REINFORCE to PPO: The Complete On-Policy RL Journey](https://taewoon.kim/2025-08-07-on-policy-rl/)

---

## 2. TinyRL and Edge RL State of the Art

### 2.1 TinyRL Framework (2022-2026)

[TinyRL](https://dl.acm.org/doi/abs/10.1145/3511808.3557206) demonstrates reinforcement learning on resource-limited devices by transferring RL algorithms knowledge to microcontrollers.

**Key Motivations:**
- Communication delays degrade system performance in IoT
- On-device learning eliminates round-trip latency
- Critical for real-time sensing-control scenarios

**TinyRL Approach:**
1. Train full RL agent in simulation
2. Distill policy to lightweight model
3. Deploy quantized policy to MCU
4. Optional: Continue learning on-device with simplified algorithm

### 2.2 DRL-TinyEdge (2026)

[DRL-TinyEdge](https://www.mdpi.com/1999-5903/18/1/31) is a latency- and energy-sensitive deep RL platform for adaptive TinyML at the 6G edge.

**Key Features:**
- Autonomous execution venue selection (local/partial/cloud)
- Dynamic model configuration (depth, quantization, frequency)
- Real-time accuracy/latency/power trade-offs

**Tested Platforms:**
- Raspberry Pi 4
- NVIDIA Jetson Nano
- ESP32 microcontroller

### 2.3 LExCI Framework (2024)

[LExCI](https://link.springer.com/article/10.1007/s10489-024-05573-0) bridges the gap between conventional RL libraries and embedded hardware deployment.

**Architecture:**
- Separates training from deployment
- Provides hardware abstraction layer
- Supports continuous learning updates

### 2.4 TinyML RL for Greenhouse Control (2024)

[TinyML Reinforcement Learning for Light Control](https://arxiv.org/html/2512.01167) demonstrates:
- Tabular Q-learning on ESP32
- On-device decision-making
- Adaptive lighting control
- Memory footprint < 50KB

---

## 3. Sample Efficiency Techniques

### 3.1 Model-Based RL: Dyna-Q

Learn a world model to generate synthetic experiences, reducing real-world interactions.

**Dyna-Q Algorithm:**
```rust
/// Dyna-Q: Model-based RL for sample efficiency
pub struct DynaQ {
    q_table: HashMap<(State, Action), f64>,

    // World model: predicts next state and reward
    model: HashMap<(State, Action), (State, f64)>,

    planning_steps: usize,  // Simulated experiences per real step
    learning_rate: f64,
    discount_factor: f64,
}

impl DynaQ {
    pub fn step(&mut self, state: State, action: Action,
                reward: f64, next_state: State) {
        // 1. Direct RL update from real experience
        self.q_update(&state, &action, reward, &next_state);

        // 2. Model learning: store transition
        self.model.insert((state.clone(), action.clone()),
                         (next_state.clone(), reward));

        // 3. Planning: simulate from model
        for _ in 0..self.planning_steps {
            // Sample random previously-seen state-action
            let (s, a) = self.model.keys().choose(&mut rand::thread_rng())
                                         .unwrap().clone();
            let (s_next, r) = self.model.get(&(s.clone(), a.clone())).unwrap();

            // Update Q from simulated experience
            self.q_update(&s, &a, *r, s_next);
        }
    }

    fn q_update(&mut self, state: &State, action: &Action,
                reward: f64, next_state: &State) {
        let current_q = *self.q_table.get(&(state.clone(), action.clone()))
                                     .unwrap_or(&0.0);
        let max_next_q = self.max_q_value(next_state);

        let new_q = current_q + self.learning_rate * (
            reward + self.discount_factor * max_next_q - current_q
        );

        self.q_table.insert((state.clone(), action.clone()), new_q);
    }
}
```

**Sample Efficiency Improvement:**
- 5-10x fewer real interactions needed
- Planning steps: 5-50 per real step
- Memory trade-off: stores model in addition to Q-table

**Reference:** [Model-Based Reinforcement Learning - DI-engine](https://opendilab.github.io/DI-engine/02_algo/model_based_rl.html)

### 3.2 Offline RL (Batch RL)

Learn from historical data without online interaction - perfect for safety-critical domains.

**Advantages for NDP:**
- Learn from logged HVAC data before deployment
- No dangerous exploration during learning
- Can incorporate human expert demonstrations

**Conservative Q-Learning (CQL):**
```rust
/// Simplified CQL for offline learning
pub struct OfflineQL {
    q_table: HashMap<(State, Action), f64>,
    dataset: Vec<(State, Action, f64, State)>,  // Historical data
    cql_alpha: f64,  // Conservatism coefficient
}

impl OfflineQL {
    pub fn train_offline(&mut self, epochs: usize) {
        for _ in 0..epochs {
            for (state, action, reward, next_state) in &self.dataset {
                // Standard Q update
                let max_next_q = self.max_q_value(next_state);
                let target = reward + 0.99 * max_next_q;

                // CQL penalty: reduce Q for actions not in dataset
                let q_current = *self.q_table.get(&(state.clone(), action.clone()))
                                            .unwrap_or(&0.0);

                // Penalize overestimation of unseen actions
                let all_q_mean = Action::all().iter()
                    .map(|a| *self.q_table.get(&(state.clone(), a.clone()))
                                         .unwrap_or(&0.0))
                    .sum::<f64>() / Action::count() as f64;

                let cql_penalty = self.cql_alpha * (all_q_mean - q_current);

                let new_q = q_current + 0.1 * (target - q_current - cql_penalty);
                self.q_table.insert((state.clone(), action.clone()), new_q);
            }
        }
    }
}
```

**Reference:** [Efficient Online Reinforcement Learning with Offline Data](https://arxiv.org/abs/2302.02948)

### 3.3 Transfer Learning from Simulation

Train in simulation, fine-tune on real device.

**Workflow:**
1. Build air quality simulator (model room dynamics, sensor noise)
2. Train RL agent extensively in simulation
3. Deploy to Pi with frozen policy
4. Optionally fine-tune with bandit layer

**Sim-to-Real Gap Mitigation:**
```rust
/// Domain randomization during simulation training
pub struct SimulatorConfig {
    // Randomize these during training
    room_volume_m3: Range<f64>,       // 20-100 m3
    ventilation_efficiency: Range<f64>, // 0.5-1.0
    co2_generation_rate: Range<f64>,   // 0.004-0.008 m3/hr/person
    sensor_noise_std: Range<f64>,      // 0-50 ppm
    response_delay_minutes: Range<u32>, // 0-10 minutes
}
```

### 3.4 Experience Replay Strategies

Memory-efficient replay for edge devices.

**Prioritized Experience Replay (Simplified):**
```rust
/// Memory-efficient prioritized replay
pub struct PrioritizedReplay<const CAPACITY: usize> {
    buffer: [(State, Action, f64, State, f64); CAPACITY],  // priority included
    priorities: [f64; CAPACITY],
    write_idx: usize,
    size: usize,
}

impl<const CAPACITY: usize> PrioritizedReplay<CAPACITY> {
    pub fn add(&mut self, transition: (State, Action, f64, State), td_error: f64) {
        let priority = (td_error.abs() + 0.01).powf(0.6);  // Prioritization
        self.buffer[self.write_idx] = (
            transition.0, transition.1, transition.2, transition.3, priority
        );
        self.priorities[self.write_idx] = priority;
        self.write_idx = (self.write_idx + 1) % CAPACITY;
        self.size = self.size.max(self.write_idx + 1).min(CAPACITY);
    }

    pub fn sample(&self, batch_size: usize) -> Vec<&(State, Action, f64, State, f64)> {
        // Proportional sampling based on priority
        // Simplified: use reservoir sampling with priority weights
        let total_priority: f64 = self.priorities[..self.size].iter().sum();

        (0..batch_size)
            .map(|_| {
                let target = rand::random::<f64>() * total_priority;
                let mut cumsum = 0.0;
                for i in 0..self.size {
                    cumsum += self.priorities[i];
                    if cumsum >= target {
                        return &self.buffer[i];
                    }
                }
                &self.buffer[self.size - 1]
            })
            .collect()
    }
}
```

**Memory Budget:**
- Buffer size: 1000-10000 transitions
- Transition size: ~100 bytes
- Total: 100KB - 1MB

---

## 4. Continuous vs Discrete Actions

### 4.1 Discrete Action Space Design

Most practical for HVAC/air quality control.

**Recommended Action Set:**
```rust
#[derive(Clone, Copy, Hash, Eq, PartialEq)]
pub enum AirQualityAction {
    // HVAC modes
    HvacOff,
    HvacLow,
    HvacMedium,
    HvacHigh,
    HvacPurge,      // Maximum ventilation for rapid air exchange

    // Window control (if automated)
    WindowClose,
    WindowPartial,
    WindowFull,

    // Air purifier
    PurifierOff,
    PurifierLow,
    PurifierHigh,

    // Combined presets
    EcoMode,        // Minimal energy, acceptable air quality
    ComfortMode,    // Optimal air quality, normal energy
    SleepMode,      // Low noise, adequate ventilation
    BoostMode,      // Maximum air quality improvement
}

impl AirQualityAction {
    pub fn all() -> &'static [Self] {
        &[
            Self::HvacOff, Self::HvacLow, Self::HvacMedium,
            Self::HvacHigh, Self::HvacPurge,
            Self::WindowClose, Self::WindowPartial, Self::WindowFull,
            Self::PurifierOff, Self::PurifierLow, Self::PurifierHigh,
        ]
    }

    pub fn to_setpoints(&self) -> Setpoints {
        match self {
            Self::HvacOff => Setpoints { fan_speed: 0.0, damper: 0.0, purifier: 0.0 },
            Self::HvacLow => Setpoints { fan_speed: 0.3, damper: 0.3, purifier: 0.0 },
            Self::HvacMedium => Setpoints { fan_speed: 0.5, damper: 0.5, purifier: 0.0 },
            Self::HvacHigh => Setpoints { fan_speed: 0.8, damper: 0.8, purifier: 0.0 },
            Self::HvacPurge => Setpoints { fan_speed: 1.0, damper: 1.0, purifier: 0.0 },
            // ...
        }
    }
}
```

### 4.2 Continuous Action Space

For precise setpoint control.

**Action Parameterization:**
```rust
/// Continuous action representation
pub struct ContinuousAction {
    pub ventilation_rate: f32,    // 0.0 - 1.0 (normalized CFM)
    pub temperature_setpoint: f32, // 18.0 - 26.0 C
    pub purifier_speed: f32,       // 0.0 - 1.0
}

impl ContinuousAction {
    pub fn from_actor_output(output: &[f32]) -> Self {
        Self {
            ventilation_rate: sigmoid(output[0]),      // 0-1
            temperature_setpoint: 18.0 + 8.0 * sigmoid(output[1]),  // 18-26
            purifier_speed: sigmoid(output[2]),        // 0-1
        }
    }
}
```

**When to Use Continuous:**
- Fine-grained temperature control
- Variable-speed equipment
- Energy optimization requiring precision

### 4.3 Hybrid Action Space

**Hierarchical Approach:**
```rust
/// Two-level action selection
pub struct HybridAgent {
    // Level 1: Mode selection (discrete - bandit)
    mode_selector: ContextualBandit,

    // Level 2: Within-mode optimization (continuous - actor-critic)
    mode_optimizers: HashMap<Mode, ActorCritic>,
}

impl HybridAgent {
    pub fn select_action(&self, state: &State) -> HybridAction {
        // 1. Select operating mode
        let mode = self.mode_selector.select(state);

        // 2. Optimize within mode
        let params = self.mode_optimizers.get(&mode)
                                         .map(|opt| opt.select_action(state))
                                         .unwrap_or_default();

        HybridAction { mode, params }
    }
}
```

---

## 5. Reward Design for Air Quality Objectives

### 5.1 Multi-Objective Reward Function

**Comprehensive Reward Design:**
```rust
/// Multi-objective reward calculator for air quality management
pub struct RewardCalculator {
    // Weights (configurable by user preference)
    pub w_air_quality: f64,    // 0.4 - primary objective
    pub w_energy: f64,         // 0.25 - secondary
    pub w_comfort: f64,        // 0.2
    pub w_stability: f64,      // 0.1 - penalize rapid changes
    pub w_safety: f64,         // 0.05 - hard constraint bonus
}

impl RewardCalculator {
    pub fn compute(&self,
                   prev_state: &State,
                   action: &Action,
                   curr_state: &State) -> f64 {

        // 1. Air Quality Component (shaped reward)
        let aq_reward = self.air_quality_reward(curr_state);

        // 2. Energy Efficiency Component
        let energy_penalty = self.energy_penalty(action, curr_state);

        // 3. Thermal Comfort Component
        let comfort_reward = self.comfort_reward(curr_state);

        // 4. Action Stability (penalize rapid switching)
        let stability_penalty = self.stability_penalty(prev_state, curr_state);

        // 5. Safety Bonus (for staying within constraints)
        let safety_bonus = self.safety_reward(curr_state);

        // Weighted sum
        self.w_air_quality * aq_reward
        + self.w_energy * energy_penalty
        + self.w_comfort * comfort_reward
        + self.w_stability * stability_penalty
        + self.w_safety * safety_bonus
    }

    fn air_quality_reward(&self, state: &State) -> f64 {
        // EPA AQI-based reward shaping
        let co2_score = match state.co2_ppm {
            x if x < 800.0 => 1.0,                      // Excellent
            x if x < 1000.0 => 0.8,                     // Good
            x if x < 1500.0 => 0.5,                     // Moderate
            x if x < 2000.0 => 0.0,                     // Poor
            x if x < 5000.0 => -0.5,                    // Very poor
            _ => -1.0,                                   // Hazardous
        };

        let pm25_score = match state.pm25_ugm3 {
            x if x < 12.0 => 1.0,                       // Good
            x if x < 35.5 => 0.6,                       // Moderate
            x if x < 55.4 => 0.2,                       // USG
            x if x < 150.4 => -0.3,                     // Unhealthy
            x if x < 250.4 => -0.7,                     // Very unhealthy
            _ => -1.0,                                   // Hazardous
        };

        // Combined score with VOC if available
        0.5 * co2_score + 0.5 * pm25_score
    }

    fn energy_penalty(&self, action: &Action, state: &State) -> f64 {
        // Normalized energy consumption
        let power_fraction = action.estimated_power_watts() / MAX_SYSTEM_POWER;

        // Consider outdoor conditions (ventilation costs more when hot/cold)
        let outdoor_penalty = if state.outdoor_temp_c < 15.0 || state.outdoor_temp_c > 28.0 {
            1.5  // Higher cost for conditioning
        } else {
            1.0
        };

        -power_fraction * outdoor_penalty
    }

    fn comfort_reward(&self, state: &State) -> f64 {
        // PMV/PPD-inspired comfort metric
        let temp_comfort = gaussian_reward(state.temperature_c, 22.0, 2.0);
        let humidity_comfort = gaussian_reward(state.humidity_percent, 50.0, 15.0);

        0.6 * temp_comfort + 0.4 * humidity_comfort
    }

    fn stability_penalty(&self, prev: &State, curr: &State) -> f64 {
        // Penalize rapid changes (uncomfortable for occupants)
        let temp_change = (curr.temperature_c - prev.temperature_c).abs();
        let vent_change = (curr.ventilation_rate - prev.ventilation_rate).abs();

        -0.1 * temp_change - 0.05 * vent_change
    }

    fn safety_reward(&self, state: &State) -> f64 {
        // Bonus for maintaining safe conditions
        if state.co2_ppm < 2000.0 && state.pm25_ugm3 < 55.0 {
            0.1
        } else {
            0.0
        }
    }
}

fn gaussian_reward(value: f64, target: f64, std: f64) -> f64 {
    (-0.5 * ((value - target) / std).powi(2)).exp()
}
```

### 5.2 Potential-Based Reward Shaping (PBRS)

Accelerate learning without changing optimal policy.

**Implementation:**
```rust
/// PBRS for air quality optimization
pub struct PotentialBasedShaping {
    discount_factor: f64,
}

impl PotentialBasedShaping {
    /// Potential function: estimate future air quality improvement potential
    pub fn potential(&self, state: &State) -> f64 {
        // Higher potential = better expected future
        let co2_potential = 1.0 - (state.co2_ppm / 2000.0).min(1.0);
        let pm25_potential = 1.0 - (state.pm25_ugm3 / 100.0).min(1.0);
        let ventilation_capacity = 1.0 - state.ventilation_rate;  // Room to increase

        // Occupancy-adjusted (more people = more CO2 generation expected)
        let occupancy_factor = 1.0 - 0.2 * state.occupancy as f64;

        (0.4 * co2_potential + 0.4 * pm25_potential
         + 0.1 * ventilation_capacity + 0.1 * occupancy_factor)
    }

    /// Shaping reward: F(s, a, s') = gamma * Phi(s') - Phi(s)
    pub fn shaping_reward(&self, state: &State, next_state: &State) -> f64 {
        self.discount_factor * self.potential(next_state) - self.potential(state)
    }

    /// Total reward = original + shaping
    pub fn shaped_reward(&self,
                         original_reward: f64,
                         state: &State,
                         next_state: &State) -> f64 {
        original_reward + self.shaping_reward(state, next_state)
    }
}
```

**PBRS Guarantees:**
- Does not change optimal policy
- Provides learning signal in sparse reward regions
- Encourages proactive action before problems occur

**Reference:** [HPRS: Hierarchical Potential-Based Reward Shaping](https://www.frontiersin.org/journals/robotics-and-ai/articles/10.3389/frobt.2024.1444188/full)

### 5.3 Avoiding Reward Hacking

**Common Hacks and Mitigations:**

| Potential Hack | Description | Mitigation |
|----------------|-------------|------------|
| Sensor manipulation | Agent learns sensor has delay, exploits timing | Include sensor lag in state |
| Oscillation | Rapid on/off to trigger bonus states | Stability penalty, action cooldown |
| Boundary gaming | Stay just above threshold | Continuous shaping, not binary |
| Energy gaming | Minimize energy by ignoring air quality | Hard safety constraints |

**Hard Safety Constraints:**
```rust
/// Safety layer that overrides RL agent
pub struct SafetyShield {
    max_co2_ppm: f64,       // 2500 ppm - hard limit
    max_pm25_ugm3: f64,     // 150 - unhealthy for sensitive groups
    min_ventilation: f64,   // Always maintain minimum airflow
}

impl SafetyShield {
    pub fn filter_action(&self,
                         proposed: Action,
                         state: &State) -> Action {
        // Override if safety violated
        if state.co2_ppm > self.max_co2_ppm {
            return Action::HvacPurge;  // Force ventilation
        }

        if state.pm25_ugm3 > self.max_pm25_ugm3 && state.outdoor_aqi < 50.0 {
            return Action::WindowFull;  // Fresh air if outdoor is clean
        }

        // Ensure minimum ventilation
        if proposed.ventilation_rate() < self.min_ventilation {
            return proposed.with_min_ventilation(self.min_ventilation);
        }

        proposed
    }
}
```

**Reference:** [Reward Hacking in RL](https://lilianweng.github.io/posts/2024-11-28-reward-hacking/)

### 5.4 Multi-Objective Pareto Optimization

For balancing conflicting objectives.

**Research Finding:** [Multi-Task Deep RL for Building Co-Optimization](https://www.cambridge.org/core/journals/data-centric-engineering/article/multitask-deep-reinforcement-learningbased-recommender-system-for-cooptimizing-energy-comfort-and-air-quality-in-commercial-buildings-with-humansintheloop/2165D51CABA9B5AF821A103571836F9E)
- 8% energy reduction in energy-focused mode
- 5-10% improvement in joint optimization
- 21% thermal comfort improvement in comfort mode

**Pareto Q-Learning Approach:**
```rust
/// Multi-objective Q-learning maintaining Pareto front
pub struct ParetoQLearning {
    // Q-values for each objective
    q_energy: HashMap<(State, Action), f64>,
    q_comfort: HashMap<(State, Action), f64>,
    q_air_quality: HashMap<(State, Action), f64>,

    // User preference (scalarization weights)
    preference: [f64; 3],
}

impl ParetoQLearning {
    pub fn select_action(&self, state: &State) -> Action {
        // Scalarize objectives based on user preference
        Action::all().iter()
            .map(|a| {
                let q_vec = [
                    *self.q_energy.get(&(state.clone(), a.clone())).unwrap_or(&0.0),
                    *self.q_comfort.get(&(state.clone(), a.clone())).unwrap_or(&0.0),
                    *self.q_air_quality.get(&(state.clone(), a.clone())).unwrap_or(&0.0),
                ];
                let score = q_vec.iter().zip(&self.preference)
                                        .map(|(q, w)| q * w)
                                        .sum::<f64>();
                (a.clone(), score)
            })
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
            .map(|(a, _)| a)
            .unwrap()
    }

    pub fn set_preference(&mut self, energy: f64, comfort: f64, air_quality: f64) {
        let sum = energy + comfort + air_quality;
        self.preference = [energy / sum, comfort / sum, air_quality / sum];
    }
}
```

---

## 6. Safe Exploration Strategies

### 6.1 Safe RL with Neural Barrier Certificates

**Research Finding:** [Safe RL for Buildings](https://www.mdpi.com/1996-1073/18/19/5313) demonstrates:
- Temperature constraints of 18-24C maintained across all zones
- Transforms constrained HVAC problem to unconstrained optimization
- Neural network learns barrier certificates from data

### 6.2 Expert-Guided Training

**Reference:** [Expert-Guided Training for HVAC RL](https://www.nature.com/articles/s41598-025-91326-z)
- 8.8x reduction in training convergence time
- Runtime shielding with expert model
- Maintains comfortable temperature range

**Implementation Pattern:**
```rust
/// Expert-guided safe exploration
pub struct ExpertGuidedAgent {
    rl_agent: TabularQLearning,
    expert_policy: RuleBasedController,
    exploration_rate: f64,
    expert_confidence_threshold: f64,
}

impl ExpertGuidedAgent {
    pub fn select_action(&self, state: &State) -> Action {
        let expert_action = self.expert_policy.recommend(state);
        let expert_confidence = self.expert_policy.confidence(state);

        if expert_confidence > self.expert_confidence_threshold {
            // Use expert when confident (safety-critical states)
            if rand::random::<f64>() < 0.1 {
                // Small exploration around expert
                self.perturb_action(expert_action)
            } else {
                expert_action
            }
        } else {
            // RL agent explores when expert uncertain
            self.rl_agent.select_action(state)
        }
    }
}

/// Rule-based expert for air quality
pub struct RuleBasedController {
    co2_threshold_high: f64,
    co2_threshold_low: f64,
    pm25_threshold: f64,
}

impl RuleBasedController {
    pub fn recommend(&self, state: &State) -> Action {
        // Simple rule-based logic
        if state.co2_ppm > self.co2_threshold_high {
            Action::HvacHigh
        } else if state.co2_ppm > self.co2_threshold_low {
            Action::HvacMedium
        } else if state.pm25_ugm3 > self.pm25_threshold {
            if state.outdoor_aqi < 50.0 {
                Action::WindowFull
            } else {
                Action::PurifierHigh
            }
        } else {
            Action::HvacLow
        }
    }

    pub fn confidence(&self, state: &State) -> f64 {
        // High confidence in extreme states
        if state.co2_ppm > 2000.0 || state.pm25_ugm3 > 100.0 {
            0.95
        } else if state.co2_ppm < 600.0 && state.pm25_ugm3 < 15.0 {
            0.9  // Very good conditions
        } else {
            0.5  // Moderate - let RL explore
        }
    }
}
```

### 6.3 Constrained Policy Optimization

**Lagrangian Relaxation:**
```rust
/// Constrained RL with Lagrange multipliers
pub struct ConstrainedRL {
    q_table: HashMap<(State, Action), f64>,

    // Constraint tracking
    co2_violations: u32,
    comfort_violations: u32,

    // Lagrange multipliers (learned)
    lambda_co2: f64,
    lambda_comfort: f64,

    constraint_thresholds: Constraints,
}

impl ConstrainedRL {
    pub fn compute_lagrangian_reward(
        &self,
        base_reward: f64,
        state: &State
    ) -> f64 {
        // Penalize constraint violations
        let co2_penalty = if state.co2_ppm > self.constraint_thresholds.max_co2 {
            self.lambda_co2 * (state.co2_ppm - self.constraint_thresholds.max_co2)
        } else {
            0.0
        };

        let comfort_penalty = if !self.is_comfortable(state) {
            self.lambda_comfort * self.comfort_distance(state)
        } else {
            0.0
        };

        base_reward - co2_penalty - comfort_penalty
    }

    pub fn update_multipliers(&mut self, episode_co2_violations: u32,
                              episode_comfort_violations: u32) {
        // Dual gradient ascent
        let lr = 0.01;

        if episode_co2_violations > 0 {
            self.lambda_co2 += lr * episode_co2_violations as f64;
        } else {
            self.lambda_co2 = (self.lambda_co2 - lr).max(0.0);
        }

        if episode_comfort_violations > 0 {
            self.lambda_comfort += lr * episode_comfort_violations as f64;
        } else {
            self.lambda_comfort = (self.lambda_comfort - lr).max(0.0);
        }
    }
}
```

### 6.4 Batch/Offline RL for Safety

**Reference:** [Safe HVAC Control via Batch RL](https://ieeexplore.ieee.org/document/9797532/)
- Learn from historical data without online exploration
- Policy improvement from day 1 of deployment
- No dangerous exploration phase

**Offline Learning Workflow:**
```rust
/// Offline RL training from historical HVAC logs
pub fn train_offline_policy(
    historical_data: &[(State, Action, f64, State)],
    epochs: usize,
) -> TabularQLearning {
    let mut agent = TabularQLearning::new();

    for _ in 0..epochs {
        for (state, action, reward, next_state) in historical_data {
            // Pessimistic update (conservative)
            let uncertainty_penalty = agent.action_uncertainty(state, action);
            let adjusted_reward = reward - 0.1 * uncertainty_penalty;

            agent.update(state.clone(), action.clone(),
                        adjusted_reward, next_state.clone());
        }
    }

    agent
}
```

---

## 7. Rust/Edge Implementations

### 7.1 Rust RL Crates

| Crate | Features | Edge Suitability |
|-------|----------|-----------------|
| `gym-rs` | OpenAI Gym interface | Training environment |
| `tch-rs` | PyTorch bindings | Heavy, for training |
| `burn` | Pure Rust deep learning | Good for inference |
| `ndarray` | N-dimensional arrays | Core math operations |
| `rand` | Random number generation | Exploration |

### 7.2 Minimal Dependencies Implementation

```toml
# Cargo.toml for edge RL
[dependencies]
rand = "0.8"
serde = { version = "1.0", features = ["derive"] }

# Optional for linear algebra
nalgebra = { version = "0.32", optional = true }

[features]
default = []
linear_fa = ["nalgebra"]  # Enable for linear function approximation
```

### 7.3 Memory-Efficient Data Structures

```rust
/// Compact state representation for edge deployment
#[repr(C, packed)]
pub struct CompactState {
    co2_level: u8,        // 0-255 (maps to 400-2500 ppm)
    pm25_level: u8,       // 0-255 (maps to 0-255 ug/m3)
    temp_comfort: i8,     // -128 to 127 (0.1C precision)
    humidity: u8,         // 0-100%
    hour: u8,             // 0-23
    occupancy: u8,        // 0-15
}

impl CompactState {
    pub fn from_sensors(readings: &SensorReadings) -> Self {
        Self {
            co2_level: ((readings.co2_ppm - 400.0) / 8.0).clamp(0.0, 255.0) as u8,
            pm25_level: readings.pm25_ugm3.clamp(0.0, 255.0) as u8,
            temp_comfort: ((readings.temperature_c - 20.0) * 10.0).clamp(-128.0, 127.0) as i8,
            humidity: readings.humidity_percent.clamp(0.0, 100.0) as u8,
            hour: readings.hour as u8,
            occupancy: readings.occupancy.clamp(0, 15) as u8,
        }
    }

    pub fn hash_key(&self) -> u64 {
        // Direct bit packing for hash table key
        ((self.co2_level as u64) << 40)
        | ((self.pm25_level as u64) << 32)
        | ((self.temp_comfort as u8 as u64) << 24)
        | ((self.humidity as u64) << 16)
        | ((self.hour as u64) << 8)
        | (self.occupancy as u64)
    }
}
// Total: 6 bytes per state
```

### 7.4 Quantized Neural Network for Edge

```rust
/// INT8 quantized inference for edge deployment
pub struct QuantizedDQN {
    weights_layer1: Vec<i8>,      // Quantized weights
    biases_layer1: Vec<i32>,      // Accumulated biases
    weights_layer2: Vec<i8>,
    biases_layer2: Vec<i32>,
    weights_output: Vec<i8>,
    biases_output: Vec<i32>,

    scale_input: f32,
    scale_layer1: f32,
    scale_layer2: f32,
    scale_output: f32,
}

impl QuantizedDQN {
    pub fn forward(&self, input: &[i8]) -> [i32; NUM_ACTIONS] {
        // Layer 1: INT8 matmul
        let mut hidden1 = self.int8_matmul(input, &self.weights_layer1, &self.biases_layer1);

        // ReLU: clamp to positive
        for h in hidden1.iter_mut() {
            *h = (*h).max(0);
        }

        // Quantize for next layer
        let hidden1_int8: Vec<i8> = hidden1.iter()
            .map(|&x| (x as f32 * self.scale_layer1).clamp(-128.0, 127.0) as i8)
            .collect();

        // Layer 2
        let mut hidden2 = self.int8_matmul(&hidden1_int8, &self.weights_layer2, &self.biases_layer2);
        for h in hidden2.iter_mut() {
            *h = (*h).max(0);
        }

        let hidden2_int8: Vec<i8> = hidden2.iter()
            .map(|&x| (x as f32 * self.scale_layer2).clamp(-128.0, 127.0) as i8)
            .collect();

        // Output layer
        let output = self.int8_matmul(&hidden2_int8, &self.weights_output, &self.biases_output);

        output.try_into().unwrap()
    }

    fn int8_matmul(&self, input: &[i8], weights: &[i8], biases: &[i32]) -> Vec<i32> {
        // Efficient INT8 matrix multiplication
        // Can use ARM NEON for acceleration
        biases.iter().enumerate()
            .map(|(out_idx, &bias)| {
                let mut sum: i32 = bias;
                for (in_idx, &inp) in input.iter().enumerate() {
                    let w = weights[out_idx * input.len() + in_idx];
                    sum += (inp as i32) * (w as i32);
                }
                sum
            })
            .collect()
    }
}
```

### 7.5 Integration with NDP Architecture

```rust
/// RL Controller integrated with NDP data flow
pub struct RLController {
    agent: Box<dyn RLAgent>,
    safety_shield: SafetyShield,
    reward_calculator: RewardCalculator,

    // State management
    last_state: Option<State>,
    last_action: Option<Action>,

    // Metrics
    episode_rewards: Vec<f64>,
    violations: u32,
}

#[async_trait]
impl Processor for RLController {
    async fn process(&self, point: &RawDataPoint) -> Result<ProcessorOutput, CoreError> {
        // 1. Extract state from sensor data
        let state = State::from_data_point(point);

        // 2. Calculate reward from previous action (if any)
        if let (Some(prev_state), Some(prev_action)) = (&self.last_state, &self.last_action) {
            let reward = self.reward_calculator.compute(prev_state, prev_action, &state);

            // 3. Update agent
            self.agent.update(prev_state.clone(), prev_action.clone(),
                             reward, state.clone());
        }

        // 4. Select action
        let proposed_action = self.agent.select_action(&state);

        // 5. Apply safety filter
        let safe_action = self.safety_shield.filter_action(proposed_action, &state);

        // 6. Store for next iteration
        self.last_state = Some(state.clone());
        self.last_action = Some(safe_action.clone());

        // 7. Return control commands
        Ok(ProcessorOutput {
            predictions: None,
            control_commands: Some(safe_action.to_commands()),
        })
    }

    fn name(&self) -> &str {
        "rl-air-quality-controller"
    }
}
```

---

## 8. Recommended RL Approach for NDP

### 8.1 Phased Implementation Roadmap

#### Phase 1: Baseline (Weeks 1-4)

**Goal:** Deploy simple, safe, interpretable controller

**Implementation:**
1. **Contextual Bandit** for mode selection
2. **Rule-based safety layer** (hard constraints)
3. **Logging infrastructure** for offline learning data

```rust
// Phase 1: Simple contextual bandit
pub struct Phase1Controller {
    mode_bandit: LinUCB,
    safety_shield: SafetyShield,
}

impl Phase1Controller {
    pub fn select_mode(&self, context: &Context) -> OperatingMode {
        let mode = self.mode_bandit.select_action(context);
        // Modes: Eco, Comfort, Sleep, Boost
        OperatingMode::from_index(mode)
    }
}
```

**Memory:** < 100 KB
**Latency:** < 5 ms
**Risk:** Low (safe defaults)

#### Phase 2: Model-Based Learning (Weeks 5-8)

**Goal:** Learn environment dynamics, improve sample efficiency

**Implementation:**
1. **Dyna-Q** with learned world model
2. **Offline training** from logged data
3. **ADWIN drift detection** for retraining triggers

```rust
// Phase 2: Dyna-Q with world model
pub struct Phase2Controller {
    dyna_agent: DynaQ,
    world_model: LearnedRoomModel,
    drift_detector: ADWIN,
}
```

**Memory:** 500 KB - 2 MB
**Latency:** < 20 ms
**Improvement:** 5-10x sample efficiency

#### Phase 3: Deep Learning (Weeks 9-16)

**Goal:** Handle complex patterns, multi-objective optimization

**Implementation:**
1. **Quantized DQN** trained offline
2. **Actor-Critic** for continuous setpoints (optional)
3. **Pareto multi-objective** optimization

```rust
// Phase 3: Deep RL with offline training
pub struct Phase3Controller {
    policy_network: QuantizedDQN,
    online_adapter: ContextualBandit,  // Fine-tuning layer
    multi_objective: ParetoWeights,
}
```

**Memory:** 1-5 MB
**Latency:** 20-100 ms
**Improvement:** Complex pattern recognition

### 8.2 Algorithm Selection Decision Tree

```
Start
  |
  v
Is state space continuous? ----Yes----> Linear Function Approx or DQN
  |
  No
  |
  v
Is state-action space < 100K? --Yes---> Tabular Q-Learning or SARSA
  |
  No
  |
  v
Can you run offline training? --Yes---> Train DQN offline, deploy quantized
  |
  No (online only)
  |
  v
Is action space discrete? -----Yes----> Contextual Bandit + Mode Selection
  |
  No (continuous actions)
  |
  v
Lightweight Actor-Critic (with safety layer)
```

### 8.3 Resource Budget Summary

| Component | Memory | CPU | Priority |
|-----------|--------|-----|----------|
| Base NDP services | 750 MB | 10-20% | Required |
| Contextual bandit | 10-50 KB | <1% | Phase 1 |
| Tabular Q-Learning | 100-500 KB | 1-2% | Phase 1 |
| World model (Dyna-Q) | 500 KB - 2 MB | 2-5% | Phase 2 |
| Replay buffer | 500 KB - 1 MB | <1% | Phase 2 |
| Quantized DQN | 1-5 MB | 2-5% | Phase 3 |
| Safety shield | 10 KB | <1% | Always |
| **Total RL Stack** | **2-10 MB** | **5-15%** | - |

**Remaining Headroom (Pi 5 16GB):** ~15 GB RAM, ~75% CPU

### 8.4 Key Success Metrics

| Metric | Target | Measurement |
|--------|--------|-------------|
| CO2 compliance | >95% time < 1000 ppm | Rolling 24h |
| PM2.5 compliance | >95% time < 35 ug/m3 | Rolling 24h |
| Energy efficiency | >15% reduction vs baseline | Monthly |
| Comfort violations | <5% of occupied hours | Weekly |
| Safety violations | 0 | Continuous |
| Learning convergence | <1000 episodes | Initial training |
| Adaptation time | <24 hours after drift | Post-drift |

---

## 9. References

### TinyRL and Edge RL
- [TinyRL: Towards Reinforcement Learning on Tiny Embedded Devices](https://dl.acm.org/doi/abs/10.1145/3511808.3557206)
- [DRL-TinyEdge: Energy- and Latency-Aware Deep RL for Adaptive TinyML](https://www.mdpi.com/1999-5903/18/1/31)
- [LExCI: A Framework for RL with Embedded Systems](https://link.springer.com/article/10.1007/s10489-024-05573-0)
- [TinyML RL for Greenhouse Light Control](https://arxiv.org/html/2512.01167)
- [TinyML in 2026: ML at the Edge](https://research.aimultiple.com/tinyml/)

### HVAC and Building Control
- [RL for HVAC Control: Technical and Conceptual Review](https://www.sciencedirect.com/science/article/pii/S235271022401653X)
- [Expert-Guided Training for Building HVAC Control](https://www.nature.com/articles/s41598-025-91326-z)
- [Safe RL for Buildings: Minimizing Energy While Maximizing Comfort](https://www.mdpi.com/1996-1073/18/19/5313)
- [Safe HVAC Control via Batch RL](https://ieeexplore.ieee.org/document/9797532/)
- [Comparative Field Deployment of RL and MPC for Residential HVAC](https://arxiv.org/abs/2510.01475)

### Multi-Objective RL
- [Multi-Task Deep RL for Co-Optimizing Energy, Comfort, and Air Quality](https://www.cambridge.org/core/journals/data-centric-engineering/article/multitask-deep-reinforcement-learningbased-recommender-system-for-cooptimizing-energy-comfort-and-air-quality-in-commercial-buildings-with-humansintheloop/2165D51CABA9B5AF821A103571836F9E)
- [Multi-Objective RL for Smart Buildings: Systematic Review](https://www.sciencedirect.com/science/article/abs/pii/S0378778825007753)
- [Hierarchical Deep RL for Year-Round HVAC Optimization](https://www.sciencedirect.com/science/article/abs/pii/S030626192500546X)
- [OCTOPUS: Holistic Smart Building Control with DRL](https://dl.acm.org/doi/10.1145/3656043)

### Sample Efficiency and Model-Based RL
- [Efficient Online RL with Offline Data](https://arxiv.org/abs/2302.02948)
- [Model-Based Reinforcement Learning - DI-engine](https://opendilab.github.io/DI-engine/02_algo/model_based_rl.html)
- [Federated RL for Edge Device Power Efficiency](https://ieeexplore.ieee.org/document/10992947/)

### Bandits and Contextual Bandits
- [Thompson Sampling - Wikipedia](https://en.wikipedia.org/wiki/Thompson_sampling)
- [Contextual Bandits Documentation](https://contextual-bandits.readthedocs.io/)
- [Scalable and Interpretable Contextual Bandits](https://arxiv.org/html/2505.16918v1)

### Reward Shaping and Safety
- [HPRS: Hierarchical Potential-Based Reward Shaping](https://www.frontiersin.org/journals/robotics-and-ai/articles/10.3389/frobt.2024.1444188/full)
- [Reward Hacking in RL](https://lilianweng.github.io/posts/2024-11-28-reward-hacking/)
- [Continuous RL via AVD Reward Shaping](https://www.sciencedirect.com/science/article/abs/pii/S0952197625006761)

### Policy Gradient and Actor-Critic
- [From REINFORCE to PPO: The Complete On-Policy RL Journey](https://taewoon.kim/2025-08-07-on-policy-rl/)
- [Policy Gradient Algorithms - Lil'Log](https://lilianweng.github.io/posts/2018-04-08-policy-gradient/)
- [PPO Explained - DigitalOcean](https://www.digitalocean.com/community/tutorials/proximal-policy-optimization-implementation-applications)

### NDP Project Context
- [Rust-Native ML Frameworks Research](/workspaces/neural-data-platform/product/research/03-rust-ml-frameworks.md)
- [Edge ML Deployment Strategies](/workspaces/neural-data-platform/product/research/gold/edge-ml/DEPLOYMENT-STRATEGIES.md)
- [Self-Learning and Adaptive Systems](/workspaces/neural-data-platform/product/research/gold/self-learning/ADAPTIVE-SYSTEMS.md)

---

## 10. Conclusion

Deploying reinforcement learning on Raspberry Pi for air quality management is **feasible and practical** with the right algorithm selection:

**Immediate Deployment (High Confidence):**
1. **Contextual Bandits (LinUCB/Thompson Sampling)** - Simple, safe, interpretable
2. **Tabular Q-Learning** - Full RL with discrete states
3. **Rule-based safety layer** - Non-negotiable for production

**Medium-Term (After Validation):**
4. **Dyna-Q** - Model-based for sample efficiency
5. **Offline RL** - Learn from historical data safely
6. **ADWIN** - Detect drift and trigger retraining

**Advanced (Careful Evaluation):**
7. **Quantized DQN** - Complex patterns, trained offline
8. **Multi-Objective Pareto** - Balance energy/comfort/air quality
9. **Actor-Critic** - Continuous action spaces

**Critical Success Factors:**
- Always include safety shield (hard constraints)
- Start simple (bandits), add complexity as needed
- Log everything for offline learning
- Monitor for reward hacking and drift
- Maintain interpretability for debugging

The recommended architecture is a **Hierarchical RL with Bandit Gateway**:
- Top level: Contextual bandit selects operating mode
- Mid level: Within-mode optimization (Q-learning or rule-based)
- Bottom level: Safety constraints (always enforced)

This approach provides immediate value with simple algorithms while enabling incremental sophistication as the system matures.

---

*Research conducted for Neural Data Platform Gold Layer Autonomous Edge capabilities*
*Document Version: 1.0*
*Status: Complete*
