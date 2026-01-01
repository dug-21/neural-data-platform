#!/usr/bin/env python3
"""
Event Generator for Testing
Generates trading events and signals for testing
"""

import json
import random
import datetime
import uuid
from typing import List, Dict, Any
from dataclasses import dataclass, asdict
from enum import Enum


class SignalType(str, Enum):
    BUY = "BUY"
    SELL = "SELL"
    HOLD = "HOLD"


class OrderStatus(str, Enum):
    PENDING = "PENDING"
    SUBMITTED = "SUBMITTED"
    FILLED = "FILLED"
    PARTIAL = "PARTIAL"
    CANCELLED = "CANCELLED"
    REJECTED = "REJECTED"


@dataclass
class TradingSignal:
    """Trading signal event"""
    id: str
    timestamp: str
    symbol: str
    signal_type: str
    strength: float
    confidence: float
    strategy: str
    metadata: Dict[str, Any]
    
    def to_dict(self) -> Dict[str, Any]:
        return asdict(self)


@dataclass
class OrderEvent:
    """Order execution event"""
    id: str
    timestamp: str
    symbol: str
    order_type: str
    side: str
    quantity: int
    price: float
    status: str
    signal_id: str
    
    def to_dict(self) -> Dict[str, Any]:
        return asdict(self)


class EventGenerator:
    """Generate synthetic trading events"""
    
    def __init__(self, seed: int = None):
        if seed:
            random.seed(seed)
        
        self.symbols = ["SPY", "QQQ", "IWM", "AAPL", "GOOGL", "MSFT"]
        self.strategies = ["momentum", "mean_reversion", "ml_ensemble", "technical", "fundamental"]
        
    def generate_signal(
        self,
        symbol: str = None,
        timestamp: datetime.datetime = None
    ) -> TradingSignal:
        """Generate a single trading signal"""
        
        if symbol is None:
            symbol = random.choice(self.symbols)
        if timestamp is None:
            timestamp = datetime.datetime.now()
        
        # Determine signal type based on random market conditions
        rand = random.random()
        if rand < 0.4:
            signal_type = SignalType.BUY
        elif rand < 0.8:
            signal_type = SignalType.SELL
        else:
            signal_type = SignalType.HOLD
        
        signal = TradingSignal(
            id=str(uuid.uuid4()),
            timestamp=timestamp.isoformat(),
            symbol=symbol,
            signal_type=signal_type.value,
            strength=random.uniform(0.5, 1.0),
            confidence=random.uniform(0.6, 0.95),
            strategy=random.choice(self.strategies),
            metadata={
                "rsi": random.uniform(20, 80),
                "macd_signal": random.uniform(-2, 2),
                "volume_ratio": random.uniform(0.5, 2.0),
                "price_change": random.uniform(-0.05, 0.05)
            }
        )
        
        return signal
    
    def generate_order_from_signal(
        self,
        signal: TradingSignal,
        base_price: float = 100.0
    ) -> OrderEvent:
        """Generate an order event from a signal"""
        
        timestamp = datetime.datetime.fromisoformat(signal.timestamp)
        timestamp += datetime.timedelta(seconds=random.uniform(0.1, 2.0))
        
        # Determine order parameters based on signal
        if signal.signal_type == SignalType.BUY.value:
            side = "BUY"
            order_type = random.choice(["MARKET", "LIMIT"])
            price = base_price * random.uniform(0.99, 1.01)
        elif signal.signal_type == SignalType.SELL.value:
            side = "SELL"
            order_type = random.choice(["MARKET", "LIMIT"])
            price = base_price * random.uniform(0.99, 1.01)
        else:
            return None  # No order for HOLD signals
        
        # Determine quantity based on signal strength
        base_quantity = 100
        quantity = int(base_quantity * signal.strength * random.uniform(0.8, 1.2))
        
        # Determine status (most orders should be filled in testing)
        status_rand = random.random()
        if status_rand < 0.7:
            status = OrderStatus.FILLED
        elif status_rand < 0.85:
            status = OrderStatus.PARTIAL
        elif status_rand < 0.95:
            status = OrderStatus.PENDING
        else:
            status = OrderStatus.REJECTED
        
        order = OrderEvent(
            id=str(uuid.uuid4()),
            timestamp=timestamp.isoformat(),
            symbol=signal.symbol,
            order_type=order_type,
            side=side,
            quantity=quantity,
            price=round(price, 2),
            status=status.value,
            signal_id=signal.id
        )
        
        return order
    
    def generate_signal_stream(
        self,
        duration_minutes: int = 60,
        signals_per_minute: int = 2
    ) -> List[TradingSignal]:
        """Generate a stream of signals over time"""
        
        signals = []
        start_time = datetime.datetime.now() - datetime.timedelta(minutes=duration_minutes)
        
        for minute in range(duration_minutes):
            current_time = start_time + datetime.timedelta(minutes=minute)
            
            for _ in range(random.randint(0, signals_per_minute * 2)):
                signal = self.generate_signal(timestamp=current_time)
                signals.append(signal)
                current_time += datetime.timedelta(
                    seconds=random.uniform(1, 30)
                )
        
        return signals
    
    def generate_order_stream(
        self,
        signals: List[TradingSignal]
    ) -> List[OrderEvent]:
        """Generate orders from signals"""
        
        orders = []
        base_prices = {symbol: random.uniform(50, 500) for symbol in self.symbols}
        
        for signal in signals:
            if signal.signal_type != SignalType.HOLD.value:
                # Not all signals result in orders
                if random.random() < 0.8:
                    order = self.generate_order_from_signal(
                        signal,
                        base_prices.get(signal.symbol, 100.0)
                    )
                    if order:
                        orders.append(order)
        
        return orders
    
    def generate_test_scenario(
        self,
        scenario_name: str = "default"
    ) -> Dict[str, Any]:
        """Generate a complete test scenario"""
        
        scenarios = {
            "bullish": {
                "duration": 30,
                "signals_per_minute": 3,
                "buy_bias": 0.7
            },
            "bearish": {
                "duration": 30,
                "signals_per_minute": 3,
                "sell_bias": 0.7
            },
            "volatile": {
                "duration": 60,
                "signals_per_minute": 5,
                "random_bias": True
            },
            "quiet": {
                "duration": 60,
                "signals_per_minute": 1,
                "hold_bias": 0.5
            },
            "default": {
                "duration": 45,
                "signals_per_minute": 2,
                "balanced": True
            }
        }
        
        config = scenarios.get(scenario_name, scenarios["default"])
        
        # Generate signals
        signals = self.generate_signal_stream(
            duration_minutes=config["duration"],
            signals_per_minute=config["signals_per_minute"]
        )
        
        # Generate orders
        orders = self.generate_order_stream(signals)
        
        # Calculate statistics
        stats = {
            "total_signals": len(signals),
            "total_orders": len(orders),
            "buy_signals": sum(1 for s in signals if s.signal_type == SignalType.BUY.value),
            "sell_signals": sum(1 for s in signals if s.signal_type == SignalType.SELL.value),
            "hold_signals": sum(1 for s in signals if s.signal_type == SignalType.HOLD.value),
            "filled_orders": sum(1 for o in orders if o.status == OrderStatus.FILLED.value),
            "rejected_orders": sum(1 for o in orders if o.status == OrderStatus.REJECTED.value)
        }
        
        return {
            "scenario": scenario_name,
            "config": config,
            "signals": [s.to_dict() for s in signals],
            "orders": [o.to_dict() for o in orders],
            "statistics": stats
        }
    
    def save_scenario(self, scenario: Dict[str, Any], filename: str):
        """Save scenario to file"""
        
        with open(filename, 'w') as f:
            json.dump(scenario, f, indent=2)
        
        print(f"Scenario saved to {filename}")


def main():
    """Generate sample event data"""
    
    generator = EventGenerator(seed=42)
    
    print("Generating synthetic trading events...")
    
    # Generate different scenarios
    scenarios = ["bullish", "bearish", "volatile", "quiet", "default"]
    
    for scenario_name in scenarios:
        print(f"Generating {scenario_name} scenario...")
        scenario = generator.generate_test_scenario(scenario_name)
        filename = f"/tmp/{scenario_name}_scenario.json"
        generator.save_scenario(scenario, filename)
        
        # Print statistics
        stats = scenario["statistics"]
        print(f"  - Signals: {stats['total_signals']}")
        print(f"  - Orders: {stats['total_orders']}")
        print(f"  - Fill rate: {stats['filled_orders']}/{stats['total_orders']}")
    
    print("\nEvent generation complete!")


if __name__ == "__main__":
    main()