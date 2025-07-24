"""
Adaptive Learning Mechanisms for DAA Trading System

This module implements individual agent learning, collective learning,
and meta-learning capabilities for the autonomous trading system.
"""

import numpy as np
from typing import Dict, List, Any, Optional, Tuple
from dataclasses import dataclass, field
from abc import ABC, abstractmethod
import json
import pickle
from collections import deque
from datetime import datetime, timedelta
import asyncio
from sklearn.ensemble import RandomForestRegressor, GradientBoostingRegressor
from sklearn.neural_network import MLPRegressor
import torch
import torch.nn as nn
import torch.optim as optim
from enum import Enum


class MarketRegime(Enum):
    """Market regime classifications"""
    BULL_TRENDING = "bull_trending"
    BEAR_TRENDING = "bear_trending"
    SIDEWAYS = "sideways"
    HIGH_VOLATILITY = "high_volatility"
    LOW_VOLATILITY = "low_volatility"
    CRISIS = "crisis"


@dataclass
class Experience:
    """Represents a single trading experience"""
    timestamp: datetime
    state: Dict[str, float]
    action: Dict[str, Any]
    reward: float
    next_state: Dict[str, float]
    done: bool
    metadata: Dict[str, Any] = field(default_factory=dict)


@dataclass
class Pattern:
    """Represents a discovered trading pattern"""
    id: str
    pattern_type: str
    conditions: Dict[str, Any]
    expected_outcome: Dict[str, float]
    confidence: float
    occurrences: int
    success_rate: float
    last_seen: datetime
    metadata: Dict[str, Any] = field(default_factory=dict)


class Memory:
    """Agent memory system with short-term and long-term components"""
    
    def __init__(self, short_term_capacity: int = 1000, long_term_capacity: int = 10000):
        self.short_term = deque(maxlen=short_term_capacity)
        self.long_term = deque(maxlen=long_term_capacity)
        self.patterns: Dict[str, Pattern] = {}
        self.consolidation_threshold = 0.7  # Confidence threshold for long-term storage
        
    def store_experience(self, experience: Experience) -> None:
        """Store an experience in short-term memory"""
        self.short_term.append(experience)
        
        # Consolidate to long-term if significant
        if experience.reward > self.consolidation_threshold:
            self.long_term.append(experience)
    
    def extract_patterns(self, min_occurrences: int = 5) -> List[Pattern]:
        """Extract patterns from experiences"""
        pattern_candidates = {}
        
        # Analyze short-term memory for patterns
        for i in range(len(self.short_term) - 1):
            exp = self.short_term[i]
            next_exp = self.short_term[i + 1]
            
            # Create pattern key from state features
            pattern_key = self._create_pattern_key(exp.state)
            
            if pattern_key not in pattern_candidates:
                pattern_candidates[pattern_key] = {
                    "occurrences": 0,
                    "successes": 0,
                    "conditions": exp.state,
                    "outcomes": []
                }
            
            pattern_candidates[pattern_key]["occurrences"] += 1
            if exp.reward > 0:
                pattern_candidates[pattern_key]["successes"] += 1
            pattern_candidates[pattern_key]["outcomes"].append(exp.reward)
        
        # Convert to Pattern objects
        patterns = []
        for key, data in pattern_candidates.items():
            if data["occurrences"] >= min_occurrences:
                pattern = Pattern(
                    id=key,
                    pattern_type="state_action",
                    conditions=data["conditions"],
                    expected_outcome={
                        "mean_reward": np.mean(data["outcomes"]),
                        "std_reward": np.std(data["outcomes"])
                    },
                    confidence=data["successes"] / data["occurrences"],
                    occurrences=data["occurrences"],
                    success_rate=data["successes"] / data["occurrences"],
                    last_seen=datetime.now()
                )
                patterns.append(pattern)
                self.patterns[key] = pattern
        
        return patterns
    
    def _create_pattern_key(self, state: Dict[str, float]) -> str:
        """Create a unique key for a pattern based on state"""
        # Discretize continuous values for pattern matching
        discretized = {}
        for key, value in state.items():
            if isinstance(value, (int, float)):
                discretized[key] = round(value, 2)
            else:
                discretized[key] = value
        
        return json.dumps(discretized, sort_keys=True)
    
    def recall_similar_experiences(self, current_state: Dict[str, float], k: int = 5) -> List[Experience]:
        """Recall k most similar experiences"""
        similarities = []
        
        for exp in self.long_term:
            similarity = self._calculate_similarity(current_state, exp.state)
            similarities.append((similarity, exp))
        
        # Sort by similarity and return top k
        similarities.sort(key=lambda x: x[0], reverse=True)
        return [exp for _, exp in similarities[:k]]
    
    def _calculate_similarity(self, state1: Dict[str, float], state2: Dict[str, float]) -> float:
        """Calculate similarity between two states"""
        common_keys = set(state1.keys()) & set(state2.keys())
        if not common_keys:
            return 0.0
        
        distances = []
        for key in common_keys:
            if isinstance(state1[key], (int, float)) and isinstance(state2[key], (int, float)):
                # Normalized distance for numeric values
                distances.append(1 - abs(state1[key] - state2[key]) / (abs(state1[key]) + abs(state2[key]) + 1e-6))
        
        return np.mean(distances) if distances else 0.0


class NeuralAdaptiveNetwork(nn.Module):
    """Neural network for adaptive learning"""
    
    def __init__(self, input_size: int, hidden_sizes: List[int], output_size: int):
        super().__init__()
        
        layers = []
        prev_size = input_size
        
        for hidden_size in hidden_sizes:
            layers.append(nn.Linear(prev_size, hidden_size))
            layers.append(nn.ReLU())
            layers.append(nn.Dropout(0.2))
            prev_size = hidden_size
        
        layers.append(nn.Linear(prev_size, output_size))
        
        self.network = nn.Sequential(*layers)
        self.optimizer = optim.Adam(self.parameters(), lr=0.001)
        
    def forward(self, x):
        return self.network(x)
    
    def adapt(self, experiences: List[Experience], epochs: int = 10) -> float:
        """Adapt network based on experiences"""
        if not experiences:
            return 0.0
        
        # Prepare training data
        states = []
        targets = []
        
        for exp in experiences:
            state_vector = self._state_to_vector(exp.state)
            target = exp.reward
            
            states.append(state_vector)
            targets.append(target)
        
        states = torch.FloatTensor(states)
        targets = torch.FloatTensor(targets).reshape(-1, 1)
        
        # Training loop
        total_loss = 0.0
        for epoch in range(epochs):
            self.optimizer.zero_grad()
            
            predictions = self.forward(states)
            loss = nn.MSELoss()(predictions, targets)
            
            loss.backward()
            self.optimizer.step()
            
            total_loss += loss.item()
        
        return total_loss / epochs
    
    def _state_to_vector(self, state: Dict[str, float]) -> List[float]:
        """Convert state dictionary to vector"""
        # Assuming fixed order of state features
        return list(state.values())


class AdaptiveAgent:
    """Individual agent with adaptive learning capabilities"""
    
    def __init__(self, agent_id: str, input_size: int, output_size: int):
        self.agent_id = agent_id
        self.memory = Memory()
        self.neural_net = NeuralAdaptiveNetwork(
            input_size=input_size,
            hidden_sizes=[128, 64, 32],
            output_size=output_size
        )
        self.performance_history = deque(maxlen=100)
        self.learning_rate = 0.001
        self.exploration_rate = 0.1
        
    def learn_from_experience(self, experience: Experience) -> None:
        """Learn from a single experience"""
        # Store in memory
        self.memory.store_experience(experience)
        
        # Update performance history
        self.performance_history.append(experience.reward)
        
        # Adapt neural network if enough experiences
        if len(self.memory.short_term) >= 10:
            recent_experiences = list(self.memory.short_term)[-10:]
            loss = self.neural_net.adapt(recent_experiences, epochs=5)
            
            # Extract patterns periodically
            if len(self.memory.short_term) % 50 == 0:
                patterns = self.memory.extract_patterns()
                self._integrate_patterns(patterns)
    
    def _integrate_patterns(self, patterns: List[Pattern]) -> None:
        """Integrate discovered patterns into decision making"""
        for pattern in patterns:
            if pattern.confidence > 0.7:
                # High confidence patterns influence exploration
                self.exploration_rate *= 0.95
            
            # Store pattern for future use
            self.memory.patterns[pattern.id] = pattern
    
    def make_decision(self, state: Dict[str, float]) -> Dict[str, Any]:
        """Make a decision based on current state"""
        # Recall similar experiences
        similar_experiences = self.memory.recall_similar_experiences(state, k=5)
        
        # Check for matching patterns
        pattern_key = self.memory._create_pattern_key(state)
        if pattern_key in self.memory.patterns:
            pattern = self.memory.patterns[pattern_key]
            if pattern.confidence > 0.8:
                # Use pattern-based decision
                return self._pattern_based_decision(pattern, state)
        
        # Neural network prediction
        state_vector = torch.FloatTensor(list(state.values()))
        with torch.no_grad():
            prediction = self.neural_net(state_vector).numpy()
        
        # Exploration vs exploitation
        if np.random.random() < self.exploration_rate:
            # Explore
            return self._explore_action(state)
        else:
            # Exploit
            return self._exploit_action(prediction, state)
    
    def _pattern_based_decision(self, pattern: Pattern, state: Dict[str, float]) -> Dict[str, Any]:
        """Make decision based on pattern"""
        return {
            "action": "pattern_based",
            "pattern_id": pattern.id,
            "confidence": pattern.confidence,
            "expected_reward": pattern.expected_outcome["mean_reward"]
        }
    
    def _explore_action(self, state: Dict[str, float]) -> Dict[str, Any]:
        """Generate exploratory action"""
        return {
            "action": "explore",
            "random_factor": np.random.random(),
            "state_hash": hash(str(state))
        }
    
    def _exploit_action(self, prediction: np.ndarray, state: Dict[str, float]) -> Dict[str, Any]:
        """Generate exploitation action based on prediction"""
        return {
            "action": "exploit",
            "prediction": prediction.tolist(),
            "confidence": self._calculate_confidence(prediction)
        }
    
    def _calculate_confidence(self, prediction: np.ndarray) -> float:
        """Calculate confidence in prediction"""
        # Based on recent performance
        if len(self.performance_history) < 10:
            return 0.5
        
        recent_performance = np.mean(list(self.performance_history)[-10:])
        return min(0.95, max(0.1, recent_performance))


class CollectiveLearning:
    """Manages collective learning across multiple agents"""
    
    def __init__(self):
        self.agents: Dict[str, AdaptiveAgent] = {}
        self.shared_patterns: Dict[str, Pattern] = {}
        self.collective_memory = Memory(short_term_capacity=5000, long_term_capacity=50000)
        self.knowledge_graph = {}  # Relationships between patterns
        
    def register_agent(self, agent: AdaptiveAgent) -> None:
        """Register an agent for collective learning"""
        self.agents[agent.agent_id] = agent
    
    async def share_knowledge(self) -> None:
        """Share knowledge between agents"""
        # Aggregate patterns from all agents
        all_patterns = {}
        for agent in self.agents.values():
            for pattern_id, pattern in agent.memory.patterns.items():
                if pattern_id not in all_patterns:
                    all_patterns[pattern_id] = []
                all_patterns[pattern_id].append(pattern)
        
        # Consolidate patterns
        for pattern_id, pattern_list in all_patterns.items():
            consolidated_pattern = self._consolidate_patterns(pattern_list)
            if consolidated_pattern.confidence > 0.6:
                self.shared_patterns[pattern_id] = consolidated_pattern
        
        # Distribute consolidated patterns
        for agent in self.agents.values():
            await self._distribute_patterns(agent, self.shared_patterns)
    
    def _consolidate_patterns(self, patterns: List[Pattern]) -> Pattern:
        """Consolidate multiple instances of the same pattern"""
        if not patterns:
            return None
        
        # Average metrics across all instances
        total_occurrences = sum(p.occurrences for p in patterns)
        weighted_confidence = sum(p.confidence * p.occurrences for p in patterns) / total_occurrences
        weighted_success_rate = sum(p.success_rate * p.occurrences for p in patterns) / total_occurrences
        
        # Use the most recent pattern as template
        latest_pattern = max(patterns, key=lambda p: p.last_seen)
        
        return Pattern(
            id=latest_pattern.id,
            pattern_type=latest_pattern.pattern_type,
            conditions=latest_pattern.conditions,
            expected_outcome=self._merge_outcomes([p.expected_outcome for p in patterns]),
            confidence=weighted_confidence,
            occurrences=total_occurrences,
            success_rate=weighted_success_rate,
            last_seen=latest_pattern.last_seen,
            metadata={"source": "collective", "agent_count": len(patterns)}
        )
    
    def _merge_outcomes(self, outcomes: List[Dict[str, float]]) -> Dict[str, float]:
        """Merge expected outcomes from multiple patterns"""
        merged = {}
        
        for outcome in outcomes:
            for key, value in outcome.items():
                if key not in merged:
                    merged[key] = []
                merged[key].append(value)
        
        # Average the values
        return {key: np.mean(values) for key, values in merged.items()}
    
    async def _distribute_patterns(self, agent: AdaptiveAgent, patterns: Dict[str, Pattern]) -> None:
        """Distribute patterns to an agent"""
        for pattern_id, pattern in patterns.items():
            # Only share if agent doesn't have it or if collective version is better
            if (pattern_id not in agent.memory.patterns or 
                pattern.confidence > agent.memory.patterns[pattern_id].confidence):
                agent.memory.patterns[pattern_id] = pattern
    
    def evolve_strategies(self, performance_threshold: float = 0.7) -> None:
        """Evolve strategies using genetic algorithm principles"""
        # Rank agents by performance
        agent_performances = []
        for agent in self.agents.values():
            if agent.performance_history:
                avg_performance = np.mean(list(agent.performance_history))
                agent_performances.append((agent, avg_performance))
        
        agent_performances.sort(key=lambda x: x[1], reverse=True)
        
        # Select top performers
        top_performers = [agent for agent, perf in agent_performances if perf > performance_threshold]
        
        if len(top_performers) < 2:
            return
        
        # Cross-pollinate successful strategies
        for i in range(0, len(top_performers) - 1, 2):
            agent1, agent2 = top_performers[i], top_performers[i + 1]
            self._crossover_strategies(agent1, agent2)
    
    def _crossover_strategies(self, agent1: AdaptiveAgent, agent2: AdaptiveAgent) -> None:
        """Crossover strategies between two successful agents"""
        # Exchange neural network weights partially
        with torch.no_grad():
            for param1, param2 in zip(agent1.neural_net.parameters(), agent2.neural_net.parameters()):
                # Uniform crossover
                mask = torch.rand_like(param1) > 0.5
                temp = param1.data.clone()
                param1.data[mask] = param2.data[mask]
                param2.data[mask] = temp[mask]
        
        # Exchange best patterns
        best_patterns1 = sorted(
            agent1.memory.patterns.values(),
            key=lambda p: p.confidence,
            reverse=True
        )[:5]
        
        best_patterns2 = sorted(
            agent2.memory.patterns.values(),
            key=lambda p: p.confidence,
            reverse=True
        )[:5]
        
        # Share patterns
        for pattern in best_patterns1:
            agent2.memory.patterns[pattern.id] = pattern
        
        for pattern in best_patterns2:
            agent1.memory.patterns[pattern.id] = pattern


class MetaLearner:
    """Meta-learning system for adapting to different market regimes"""
    
    def __init__(self):
        self.regime_models: Dict[MarketRegime, Any] = {}
        self.regime_history = deque(maxlen=1000)
        self.regime_transition_matrix = np.zeros((len(MarketRegime), len(MarketRegime)))
        self.current_regime = MarketRegime.SIDEWAYS
        self.regime_strategies: Dict[MarketRegime, List[str]] = {
            MarketRegime.BULL_TRENDING: ["momentum", "breakout", "trend_following"],
            MarketRegime.BEAR_TRENDING: ["short_selling", "put_options", "defensive"],
            MarketRegime.SIDEWAYS: ["mean_reversion", "range_trading", "arbitrage"],
            MarketRegime.HIGH_VOLATILITY: ["volatility_arbitrage", "options", "hedging"],
            MarketRegime.LOW_VOLATILITY: ["carry_trade", "yield_enhancement", "leverage"],
            MarketRegime.CRISIS: ["risk_off", "safe_haven", "liquidity_preservation"]
        }
        
        # Initialize regime detection models
        self._initialize_regime_models()
    
    def _initialize_regime_models(self) -> None:
        """Initialize models for each regime"""
        for regime in MarketRegime:
            self.regime_models[regime] = GradientBoostingRegressor(
                n_estimators=100,
                learning_rate=0.1,
                max_depth=3
            )
    
    def detect_regime(self, market_data: Dict[str, Any]) -> MarketRegime:
        """Detect current market regime"""
        features = self._extract_regime_features(market_data)
        
        # Calculate probability for each regime
        regime_probabilities = {}
        for regime, model in self.regime_models.items():
            if hasattr(model, 'predict_proba'):
                prob = model.predict_proba([features])[0][1]
            else:
                # For regressors, use sigmoid of prediction
                pred = model.predict([features])[0]
                prob = 1 / (1 + np.exp(-pred))
            
            regime_probabilities[regime] = prob
        
        # Select regime with highest probability
        detected_regime = max(regime_probabilities, key=regime_probabilities.get)
        
        # Update history and transition matrix
        if self.regime_history:
            prev_regime = self.regime_history[-1]
            prev_idx = list(MarketRegime).index(prev_regime)
            curr_idx = list(MarketRegime).index(detected_regime)
            self.regime_transition_matrix[prev_idx, curr_idx] += 1
        
        self.regime_history.append(detected_regime)
        self.current_regime = detected_regime
        
        return detected_regime
    
    def _extract_regime_features(self, market_data: Dict[str, Any]) -> List[float]:
        """Extract features for regime detection"""
        features = []
        
        # Price trend features
        if 'prices' in market_data:
            prices = market_data['prices']
            features.append(self._calculate_trend(prices))
            features.append(self._calculate_volatility(prices))
            features.append(self._calculate_momentum(prices))
        
        # Volume features
        if 'volumes' in market_data:
            volumes = market_data['volumes']
            features.append(np.mean(volumes))
            features.append(np.std(volumes))
        
        # Market breadth
        if 'breadth' in market_data:
            features.append(market_data['breadth'])
        
        # Sentiment
        if 'sentiment' in market_data:
            features.append(market_data['sentiment'])
        
        return features
    
    def _calculate_trend(self, prices: List[float]) -> float:
        """Calculate trend strength"""
        if len(prices) < 2:
            return 0.0
        
        # Linear regression slope
        x = np.arange(len(prices))
        slope, _ = np.polyfit(x, prices, 1)
        
        # Normalize by price level
        return slope / (np.mean(prices) + 1e-6)
    
    def _calculate_volatility(self, prices: List[float]) -> float:
        """Calculate price volatility"""
        if len(prices) < 2:
            return 0.0
        
        returns = np.diff(prices) / prices[:-1]
        return np.std(returns)
    
    def _calculate_momentum(self, prices: List[float]) -> float:
        """Calculate price momentum"""
        if len(prices) < 10:
            return 0.0
        
        # Rate of change
        return (prices[-1] - prices[-10]) / (prices[-10] + 1e-6)
    
    def adapt_to_regime(self, agents: List[AdaptiveAgent]) -> None:
        """Adapt agent strategies to current regime"""
        recommended_strategies = self.regime_strategies[self.current_regime]
        
        for agent in agents:
            # Adjust exploration rate based on regime
            if self.current_regime in [MarketRegime.HIGH_VOLATILITY, MarketRegime.CRISIS]:
                agent.exploration_rate = min(0.3, agent.exploration_rate * 1.5)
            else:
                agent.exploration_rate = max(0.05, agent.exploration_rate * 0.9)
            
            # Update agent metadata with regime info
            agent.current_regime = self.current_regime
            agent.recommended_strategies = recommended_strategies
    
    def learn_regime_patterns(self, regime: MarketRegime, market_data: Dict[str, Any], outcome: float) -> None:
        """Learn patterns specific to a regime"""
        features = self._extract_regime_features(market_data)
        
        # Update regime model
        if hasattr(self.regime_models[regime], 'partial_fit'):
            self.regime_models[regime].partial_fit([features], [outcome])
        else:
            # Store for batch training
            if not hasattr(self, '_training_data'):
                self._training_data = {r: {'X': [], 'y': []} for r in MarketRegime}
            
            self._training_data[regime]['X'].append(features)
            self._training_data[regime]['y'].append(outcome)
            
            # Retrain periodically
            if len(self._training_data[regime]['X']) >= 100:
                X = np.array(self._training_data[regime]['X'])
                y = np.array(self._training_data[regime]['y'])
                self.regime_models[regime].fit(X, y)
                
                # Keep only recent data
                self._training_data[regime]['X'] = self._training_data[regime]['X'][-500:]
                self._training_data[regime]['y'] = self._training_data[regime]['y'][-500:]
    
    def predict_regime_transition(self) -> Dict[MarketRegime, float]:
        """Predict probability of transitioning to each regime"""
        if not self.regime_history:
            # Equal probability if no history
            return {regime: 1/len(MarketRegime) for regime in MarketRegime}
        
        current_idx = list(MarketRegime).index(self.current_regime)
        
        # Normalize transition matrix row
        transitions = self.regime_transition_matrix[current_idx]
        total_transitions = np.sum(transitions)
        
        if total_transitions == 0:
            # No transitions observed yet
            return {regime: 1/len(MarketRegime) for regime in MarketRegime}
        
        probabilities = transitions / total_transitions
        
        return {
            regime: prob 
            for regime, prob in zip(MarketRegime, probabilities)
        }


# Example usage
if __name__ == "__main__":
    # Create adaptive agent
    agent = AdaptiveAgent("agent_1", input_size=10, output_size=3)
    
    # Create some sample experiences
    for i in range(100):
        state = {
            "price": 100 + np.random.randn(),
            "volume": 1000 + np.random.randn() * 100,
            "rsi": 50 + np.random.randn() * 10,
            "macd": np.random.randn()
        }
        
        action = agent.make_decision(state)
        
        # Simulate outcome
        reward = np.random.randn() * 0.1
        
        experience = Experience(
            timestamp=datetime.now(),
            state=state,
            action=action,
            reward=reward,
            next_state=state,  # Simplified
            done=False
        )
        
        agent.learn_from_experience(experience)
    
    # Extract patterns
    patterns = agent.memory.extract_patterns()
    print(f"Discovered {len(patterns)} patterns")
    
    # Test collective learning
    collective = CollectiveLearning()
    
    # Create multiple agents
    agents = []
    for i in range(5):
        agent = AdaptiveAgent(f"agent_{i}", input_size=10, output_size=3)
        agents.append(agent)
        collective.register_agent(agent)
    
    # Share knowledge
    asyncio.run(collective.share_knowledge())
    
    # Test meta-learning
    meta_learner = MetaLearner()
    
    # Simulate market data
    market_data = {
        "prices": [100 + i + np.random.randn() for i in range(50)],
        "volumes": [1000 + np.random.randn() * 100 for _ in range(50)],
        "breadth": 0.6,
        "sentiment": 0.7
    }
    
    regime = meta_learner.detect_regime(market_data)
    print(f"Detected market regime: {regime.value}")
    
    # Adapt agents to regime
    meta_learner.adapt_to_regime(agents)
    
    print("Adaptive learning system initialized successfully!")