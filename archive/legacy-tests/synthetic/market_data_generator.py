#!/usr/bin/env python3
"""
Synthetic Market Data Generator
Generates realistic market data for testing
"""

import json
import random
import datetime
from typing import List, Dict, Any
from dataclasses import dataclass, asdict


@dataclass
class MarketDataPoint:
    """Single market data point"""
    symbol: str
    timestamp: str
    open: float
    high: float
    low: float
    close: float
    volume: int
    vwap: float
    
    def to_dict(self) -> Dict[str, Any]:
        return asdict(self)


class MarketDataGenerator:
    """Generate synthetic market data for testing"""
    
    def __init__(self, seed: int = None):
        if seed:
            random.seed(seed)
        
        self.symbols = ["SPY", "QQQ", "IWM", "DIA", "VTI", "TEST"]
        self.base_prices = {
            "SPY": 450.0,
            "QQQ": 380.0,
            "IWM": 220.0,
            "DIA": 380.0,
            "VTI": 240.0,
            "TEST": 100.0
        }
        
    def generate_price_series(
        self,
        symbol: str,
        start_time: datetime.datetime,
        num_points: int,
        interval_minutes: int = 5
    ) -> List[MarketDataPoint]:
        """Generate a time series of market data"""
        
        data_points = []
        base_price = self.base_prices.get(symbol, 100.0)
        current_price = base_price
        current_time = start_time
        
        for _ in range(num_points):
            # Generate realistic price movement
            volatility = 0.002  # 0.2% volatility
            price_change = random.gauss(0, volatility * current_price)
            current_price = max(current_price + price_change, base_price * 0.5)
            
            # Generate OHLC data
            open_price = current_price
            close_price = current_price + random.gauss(0, volatility * current_price * 0.5)
            high_price = max(open_price, close_price) * random.uniform(1.0, 1.005)
            low_price = min(open_price, close_price) * random.uniform(0.995, 1.0)
            
            # Generate volume (higher during market hours)
            hour = current_time.hour
            if 9 <= hour <= 16:
                base_volume = 1000000
            else:
                base_volume = 100000
            volume = int(base_volume * random.uniform(0.5, 2.0))
            
            # Calculate VWAP
            vwap = (high_price + low_price + close_price) / 3
            
            data_point = MarketDataPoint(
                symbol=symbol,
                timestamp=current_time.isoformat(),
                open=round(open_price, 2),
                high=round(high_price, 2),
                low=round(low_price, 2),
                close=round(close_price, 2),
                volume=volume,
                vwap=round(vwap, 4)
            )
            
            data_points.append(data_point)
            current_price = close_price
            current_time += datetime.timedelta(minutes=interval_minutes)
        
        return data_points
    
    def generate_tick_data(
        self,
        symbol: str,
        start_time: datetime.datetime,
        num_ticks: int
    ) -> List[Dict[str, Any]]:
        """Generate tick-level data"""
        
        ticks = []
        base_price = self.base_prices.get(symbol, 100.0)
        current_price = base_price
        current_time = start_time
        
        for _ in range(num_ticks):
            # Generate tick
            price_change = random.gauss(0, 0.01)
            current_price = max(current_price + price_change, base_price * 0.5)
            
            size = random.choice([100, 200, 300, 400, 500, 1000])
            
            tick = {
                "symbol": symbol,
                "timestamp": current_time.isoformat(),
                "price": round(current_price, 4),
                "size": size,
                "conditions": random.choice(["", "I", "Q", "F"]),
                "exchange": random.choice(["NYSE", "NASDAQ", "ARCA", "BATS"])
            }
            
            ticks.append(tick)
            current_time += datetime.timedelta(milliseconds=random.randint(100, 5000))
        
        return ticks
    
    def generate_batch_data(
        self,
        symbols: List[str] = None,
        days: int = 7,
        points_per_day: int = 78  # 6.5 hours * 12 (5-min intervals)
    ) -> Dict[str, List[MarketDataPoint]]:
        """Generate batch data for multiple symbols"""
        
        if symbols is None:
            symbols = self.symbols[:3]  # Default to first 3 symbols
        
        start_time = datetime.datetime.now() - datetime.timedelta(days=days)
        all_data = {}
        
        for symbol in symbols:
            data_points = self.generate_price_series(
                symbol=symbol,
                start_time=start_time,
                num_points=days * points_per_day,
                interval_minutes=5
            )
            all_data[symbol] = data_points
        
        return all_data
    
    def save_to_file(self, data: Any, filename: str):
        """Save generated data to file"""
        
        if isinstance(data, dict):
            # Convert MarketDataPoint objects to dicts
            serializable_data = {}
            for key, value in data.items():
                if isinstance(value, list) and value and isinstance(value[0], MarketDataPoint):
                    serializable_data[key] = [point.to_dict() for point in value]
                else:
                    serializable_data[key] = value
            data = serializable_data
        elif isinstance(data, list) and data and isinstance(data[0], MarketDataPoint):
            data = [point.to_dict() for point in data]
        
        with open(filename, 'w') as f:
            json.dump(data, f, indent=2, default=str)
        
        print(f"Data saved to {filename}")


def main():
    """Generate sample data files"""
    
    generator = MarketDataGenerator(seed=42)
    
    # Generate different types of data
    print("Generating synthetic market data...")
    
    # 1. Single symbol time series
    spy_data = generator.generate_price_series(
        symbol="SPY",
        start_time=datetime.datetime.now() - datetime.timedelta(days=1),
        num_points=100
    )
    generator.save_to_file(spy_data, "/tmp/spy_data.json")
    
    # 2. Multi-symbol batch data
    batch_data = generator.generate_batch_data(
        symbols=["SPY", "QQQ", "IWM"],
        days=7
    )
    generator.save_to_file(batch_data, "/tmp/batch_market_data.json")
    
    # 3. Tick data
    tick_data = generator.generate_tick_data(
        symbol="SPY",
        start_time=datetime.datetime.now(),
        num_ticks=1000
    )
    generator.save_to_file(tick_data, "/tmp/tick_data.json")
    
    print("Synthetic data generation complete!")
    print("Files generated:")
    print("  - /tmp/spy_data.json")
    print("  - /tmp/batch_market_data.json")
    print("  - /tmp/tick_data.json")


if __name__ == "__main__":
    main()