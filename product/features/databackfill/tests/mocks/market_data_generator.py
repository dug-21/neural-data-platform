"""
Market data generator for testing purposes
"""
import random
import numpy as np
from datetime import datetime, timedelta
from typing import List, Optional, Tuple
from dataclasses import dataclass

from data_ingestion.providers.base import MarketData, TickData


class MarketDataGenerator:
    """Generate realistic market data for testing"""
    
    # Market session times (EST)
    MARKET_OPEN = (9, 30)  # 9:30 AM
    MARKET_CLOSE = (16, 0)  # 4:00 PM
    
    # Volatility parameters by symbol
    VOLATILITY_PARAMS = {
        'AAPL': {'base_price': 150, 'volatility': 0.02, 'avg_volume': 80000000},
        'GOOGL': {'base_price': 130, 'volatility': 0.025, 'avg_volume': 25000000},
        'TSLA': {'base_price': 200, 'volatility': 0.04, 'avg_volume': 100000000},
        'MSFT': {'base_price': 350, 'volatility': 0.018, 'avg_volume': 30000000},
        'NVDA': {'base_price': 450, 'volatility': 0.035, 'avg_volume': 50000000},
        'DEFAULT': {'base_price': 100, 'volatility': 0.02, 'avg_volume': 10000000}
    }
    
    @staticmethod
    def generate_ohlcv_data(
        symbol: str,
        start_date: datetime,
        end_date: datetime,
        interval: str,
        realistic: bool = True
    ) -> List[MarketData]:
        """
        Generate realistic OHLCV data with proper patterns
        
        Args:
            symbol: Stock symbol
            start_date: Start date for data generation
            end_date: End date for data generation
            interval: Time interval ('1min', '5min', '1hour', '1day')
            realistic: Whether to include realistic patterns (trends, volatility clustering)
        
        Returns:
            List of MarketData objects
        """
        data = []
        params = MarketDataGenerator.VOLATILITY_PARAMS.get(
            symbol, 
            MarketDataGenerator.VOLATILITY_PARAMS['DEFAULT']
        )
        
        # Parse interval
        interval_minutes = MarketDataGenerator._parse_interval(interval)
        
        # Initialize price
        current_price = params['base_price']
        trend = 0  # Current trend component
        
        # Generate time series
        current_time = start_date
        
        while current_time <= end_date:
            # Skip non-trading hours for intraday data
            if interval_minutes < 1440:  # Less than daily
                if not MarketDataGenerator._is_market_hours(current_time):
                    current_time += timedelta(minutes=interval_minutes)
                    continue
                
                # Skip weekends
                if current_time.weekday() >= 5:
                    current_time += timedelta(days=1)
                    current_time = current_time.replace(
                        hour=MarketDataGenerator.MARKET_OPEN[0],
                        minute=MarketDataGenerator.MARKET_OPEN[1],
                        second=0,
                        microsecond=0
                    )
                    continue
            
            # Generate OHLCV data
            if realistic:
                ohlcv = MarketDataGenerator._generate_realistic_ohlcv(
                    current_price, params, trend, interval_minutes
                )
                
                # Update trend
                trend = 0.9 * trend + 0.1 * np.random.normal(0, 0.001)
                current_price = ohlcv['close']
            else:
                # Simple random walk
                ohlcv = MarketDataGenerator._generate_simple_ohlcv(
                    current_price, params
                )
                current_price = ohlcv['close']
            
            # Create MarketData object
            data.append(MarketData(
                time=current_time,
                symbol=symbol,
                open=round(ohlcv['open'], 2),
                high=round(ohlcv['high'], 2),
                low=round(ohlcv['low'], 2),
                close=round(ohlcv['close'], 2),
                volume=ohlcv['volume']
            ))
            
            # Move to next time period
            current_time += timedelta(minutes=interval_minutes)
        
        return data
    
    @staticmethod
    def generate_tick_data(
        symbol: str,
        date: datetime,
        tick_count: int = 50000
    ) -> List[TickData]:
        """
        Generate realistic tick data
        
        Args:
            symbol: Stock symbol
            date: Date for tick data
            tick_count: Number of ticks to generate
        
        Returns:
            List of TickData objects
        """
        ticks = []
        params = MarketDataGenerator.VOLATILITY_PARAMS.get(
            symbol,
            MarketDataGenerator.VOLATILITY_PARAMS['DEFAULT']
        )
        
        # Start at market open
        current_time = date.replace(
            hour=MarketDataGenerator.MARKET_OPEN[0],
            minute=MarketDataGenerator.MARKET_OPEN[1],
            second=0,
            microsecond=0
        )
        
        # Market hours in seconds
        market_seconds = 6.5 * 3600  # 6.5 hours
        
        # Generate ticks
        current_price = params['base_price']
        
        for i in range(tick_count):
            # Time progression (non-uniform)
            time_increment = np.random.exponential(market_seconds / tick_count)
            current_time += timedelta(seconds=time_increment)
            
            # Price movement
            price_change = np.random.normal(0, params['volatility'] * 0.001)
            current_price *= (1 + price_change)
            
            # Size (lot size distribution)
            size = int(np.random.lognormal(5, 1.5))  # Log-normal distribution
            size = max(1, min(size, 10000))  # Clip to reasonable range
            
            # Bid/Ask spread
            spread = 0.01 * (1 + np.random.exponential(0.5))  # Variable spread
            bid = round(current_price - spread/2, 2)
            ask = round(current_price + spread/2, 2)
            
            ticks.append(TickData(
                time=current_time,
                symbol=symbol,
                price=round(current_price, 2),
                size=size,
                bid=bid,
                ask=ask,
                conditions=[]  # Can add trade conditions if needed
            ))
        
        return ticks
    
    @staticmethod
    def inject_anomalies(
        data: List[MarketData],
        gap_probability: float = 0.1,
        duplicate_probability: float = 0.05,
        invalid_probability: float = 0.02
    ) -> List[MarketData]:
        """
        Inject realistic data anomalies for testing
        
        Args:
            data: Original market data
            gap_probability: Probability of creating gaps
            duplicate_probability: Probability of creating duplicates
            invalid_probability: Probability of invalid data
        
        Returns:
            Data with injected anomalies
        """
        if not data:
            return data
        
        anomalous_data = data.copy()
        
        # Inject gaps
        if random.random() < gap_probability:
            # Remove some consecutive points
            gap_start = random.randint(len(data)//4, 3*len(data)//4)
            gap_size = random.randint(5, 20)
            anomalous_data = anomalous_data[:gap_start] + anomalous_data[gap_start+gap_size:]
        
        # Inject duplicates
        for i in range(len(anomalous_data)):
            if random.random() < duplicate_probability:
                # Duplicate this point
                anomalous_data.insert(i, anomalous_data[i])
        
        # Inject invalid data
        for i in range(len(anomalous_data)):
            if random.random() < invalid_probability:
                anomaly_type = random.choice(['negative_price', 'wrong_ohlc', 'zero_volume'])
                
                if anomaly_type == 'negative_price':
                    anomalous_data[i].close = -abs(anomalous_data[i].close)
                elif anomaly_type == 'wrong_ohlc':
                    # Make high lower than low
                    anomalous_data[i].high = anomalous_data[i].low - 1
                elif anomaly_type == 'zero_volume':
                    anomalous_data[i].volume = -1000  # Negative volume
        
        return anomalous_data
    
    @staticmethod
    def generate_market_event(
        data: List[MarketData],
        event_type: str,
        event_time: Optional[datetime] = None
    ) -> List[MarketData]:
        """
        Simulate market events like halts, gaps, splits
        
        Args:
            data: Original market data
            event_type: Type of event ('halt', 'gap', 'split')
            event_time: When the event occurs
        
        Returns:
            Data with simulated event
        """
        if not data or not event_time:
            return data
        
        event_data = data.copy()
        
        # Find the index closest to event time
        event_idx = min(
            range(len(event_data)),
            key=lambda i: abs(event_data[i].time - event_time)
        )
        
        if event_type == 'halt':
            # Trading halt - no data for 30 minutes
            halt_duration = 30  # minutes
            points_to_remove = []
            
            for i in range(event_idx, len(event_data)):
                if (event_data[i].time - event_time).total_seconds() < halt_duration * 60:
                    points_to_remove.append(i)
            
            # Remove points during halt
            for i in reversed(points_to_remove):
                event_data.pop(i)
                
        elif event_type == 'gap':
            # Price gap
            if event_idx < len(event_data) - 1:
                gap_size = event_data[event_idx].close * 0.05  # 5% gap
                
                for i in range(event_idx + 1, len(event_data)):
                    event_data[i].open += gap_size
                    event_data[i].high += gap_size
                    event_data[i].low += gap_size
                    event_data[i].close += gap_size
                    
        elif event_type == 'split':
            # Stock split (2:1)
            split_ratio = 2
            
            for i in range(event_idx, len(event_data)):
                event_data[i].open /= split_ratio
                event_data[i].high /= split_ratio
                event_data[i].low /= split_ratio
                event_data[i].close /= split_ratio
                event_data[i].volume *= split_ratio
        
        return event_data
    
    @staticmethod
    def _parse_interval(interval: str) -> int:
        """Parse interval string to minutes"""
        if interval == '1min':
            return 1
        elif interval == '5min':
            return 5
        elif interval == '15min':
            return 15
        elif interval == '30min':
            return 30
        elif interval == '1hour':
            return 60
        elif interval == '4hour':
            return 240
        elif interval == '1day':
            return 1440
        else:
            return 60  # Default to hourly
    
    @staticmethod
    def _is_market_hours(dt: datetime) -> bool:
        """Check if datetime is during market hours"""
        if dt.weekday() >= 5:  # Weekend
            return False
        
        market_open = dt.replace(
            hour=MarketDataGenerator.MARKET_OPEN[0],
            minute=MarketDataGenerator.MARKET_OPEN[1],
            second=0
        )
        market_close = dt.replace(
            hour=MarketDataGenerator.MARKET_CLOSE[0],
            minute=MarketDataGenerator.MARKET_CLOSE[1],
            second=0
        )
        
        return market_open <= dt <= market_close
    
    @staticmethod
    def _generate_realistic_ohlcv(
        current_price: float,
        params: dict,
        trend: float,
        interval_minutes: int
    ) -> dict:
        """Generate realistic OHLCV with patterns"""
        # Price movement with trend and mean reversion
        volatility = params['volatility'] * np.sqrt(interval_minutes / 390)  # Scale by time
        
        # Generate multiple ticks within the interval
        num_ticks = max(1, interval_minutes)
        prices = [current_price]
        
        for _ in range(num_ticks):
            change = np.random.normal(trend, volatility)
            mean_reversion = -0.01 * (prices[-1] / params['base_price'] - 1)
            new_price = prices[-1] * (1 + change + mean_reversion)
            prices.append(max(new_price, current_price * 0.5))  # Prevent extreme drops
        
        # OHLCV from generated prices
        open_price = prices[0]
        close_price = prices[-1]
        high_price = max(prices)
        low_price = min(prices)
        
        # Volume with intraday pattern
        base_volume = params['avg_volume'] / (390 / interval_minutes)  # Daily volume distributed
        volume_multiplier = 1 + 0.5 * np.sin(np.pi * np.random.random())  # U-shaped volume
        volume = int(base_volume * volume_multiplier * (0.5 + random.random()))
        
        return {
            'open': open_price,
            'high': high_price,
            'low': low_price,
            'close': close_price,
            'volume': volume
        }
    
    @staticmethod
    def _generate_simple_ohlcv(current_price: float, params: dict) -> dict:
        """Generate simple random OHLCV"""
        volatility = params['volatility']
        
        # Random price movements
        open_price = current_price
        close_price = current_price * (1 + np.random.normal(0, volatility))
        
        # Ensure OHLC consistency
        high_price = max(open_price, close_price) * (1 + abs(np.random.normal(0, volatility/2)))
        low_price = min(open_price, close_price) * (1 - abs(np.random.normal(0, volatility/2)))
        
        # Random volume
        volume = int(params['avg_volume'] / 390 * (0.5 + random.random()))
        
        return {
            'open': open_price,
            'high': high_price,
            'low': low_price,
            'close': close_price,
            'volume': volume
        }