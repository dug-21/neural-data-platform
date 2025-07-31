"""Market hours utility for checking trading hours."""
from datetime import datetime, time, timedelta
from typing import Dict, Tuple, Optional
import pytz
from enum import Enum


class MarketStatus(Enum):
    """Market status enumeration."""
    OPEN = "open"
    CLOSED = "closed"
    PRE_MARKET = "pre_market"
    AFTER_HOURS = "after_hours"
    WEEKEND = "weekend"
    HOLIDAY = "holiday"


class MarketHours:
    """Market hours checker for various exchanges."""
    
    # US Stock Market (NYSE/NASDAQ)
    US_STOCK_MARKET = {
        'timezone': 'US/Eastern',
        'pre_market_open': time(4, 0),      # 4:00 AM ET
        'regular_open': time(9, 30),        # 9:30 AM ET
        'regular_close': time(16, 0),       # 4:00 PM ET
        'after_hours_close': time(20, 0),   # 8:00 PM ET
        'trading_days': [0, 1, 2, 3, 4]     # Monday to Friday
    }
    
    # Cryptocurrency Market (24/7)
    CRYPTO_MARKET = {
        'timezone': 'UTC',
        'pre_market_open': time(0, 0),
        'regular_open': time(0, 0),
        'regular_close': time(23, 59, 59),
        'after_hours_close': time(23, 59, 59),
        'trading_days': [0, 1, 2, 3, 4, 5, 6]  # All days
    }
    
    # Forex Market (Sunday 5PM ET to Friday 5PM ET)
    FOREX_MARKET = {
        'timezone': 'US/Eastern',
        'pre_market_open': time(17, 0),     # Sunday 5:00 PM ET
        'regular_open': time(17, 0),        # Sunday 5:00 PM ET
        'regular_close': time(17, 0),       # Friday 5:00 PM ET
        'after_hours_close': time(17, 0),   # Friday 5:00 PM ET
        'trading_days': [0, 1, 2, 3, 4, 6]  # Monday to Friday + Sunday evening
    }
    
    # Major US holidays (simplified list)
    US_HOLIDAYS_2024 = [
        datetime(2024, 1, 1),    # New Year's Day
        datetime(2024, 1, 15),   # MLK Day
        datetime(2024, 2, 19),   # Presidents Day
        datetime(2024, 3, 29),   # Good Friday
        datetime(2024, 5, 27),   # Memorial Day
        datetime(2024, 6, 19),   # Juneteenth
        datetime(2024, 7, 4),    # Independence Day
        datetime(2024, 9, 2),    # Labor Day
        datetime(2024, 11, 28),  # Thanksgiving
        datetime(2024, 12, 25),  # Christmas
    ]
    
    US_HOLIDAYS_2025 = [
        datetime(2025, 1, 1),    # New Year's Day
        datetime(2025, 1, 20),   # MLK Day
        datetime(2025, 2, 17),   # Presidents Day
        datetime(2025, 4, 18),   # Good Friday
        datetime(2025, 5, 26),   # Memorial Day
        datetime(2025, 6, 19),   # Juneteenth
        datetime(2025, 7, 4),    # Independence Day
        datetime(2025, 9, 1),    # Labor Day
        datetime(2025, 11, 27),  # Thanksgiving
        datetime(2025, 12, 25),  # Christmas
    ]
    
    @classmethod
    def get_market_status(cls, 
                         market_type: str = 'US_STOCK', 
                         check_time: Optional[datetime] = None) -> Tuple[MarketStatus, str]:
        """
        Check if the market is open.
        
        Args:
            market_type: Type of market ('US_STOCK', 'CRYPTO', 'FOREX')
            check_time: Time to check (defaults to current time)
            
        Returns:
            Tuple of (MarketStatus, description message)
        """
        if check_time is None:
            check_time = datetime.now(pytz.UTC)
            
        # Get market configuration
        if market_type == 'US_STOCK':
            market_config = cls.US_STOCK_MARKET
        elif market_type == 'CRYPTO':
            market_config = cls.CRYPTO_MARKET
        elif market_type == 'FOREX':
            market_config = cls.FOREX_MARKET
        else:
            return MarketStatus.CLOSED, f"Unknown market type: {market_type}"
            
        # Convert to market timezone
        market_tz = pytz.timezone(market_config['timezone'])
        market_time = check_time.astimezone(market_tz)
        
        # Check if it's a weekend (for non-crypto markets)
        if market_type != 'CRYPTO':
            weekday = market_time.weekday()
            if weekday not in market_config['trading_days']:
                if weekday == 5:  # Saturday
                    return MarketStatus.WEEKEND, "Market closed - Saturday"
                elif weekday == 6:  # Sunday
                    if market_type == 'FOREX' and market_time.time() >= time(17, 0):
                        return MarketStatus.OPEN, "Forex market open - Sunday evening"
                    return MarketStatus.WEEKEND, "Market closed - Sunday"
                    
        # Check if it's a holiday (US markets only)
        if market_type == 'US_STOCK':
            date_only = market_time.date()
            year = date_only.year
            holidays = cls.US_HOLIDAYS_2024 if year == 2024 else cls.US_HOLIDAYS_2025
            
            for holiday in holidays:
                if holiday.date() == date_only:
                    return MarketStatus.HOLIDAY, f"Market closed - US holiday"
                    
        # Check time of day
        current_time = market_time.time()
        
        # Check pre-market
        if current_time >= market_config['pre_market_open'] and current_time < market_config['regular_open']:
            return MarketStatus.PRE_MARKET, "Pre-market hours"
            
        # Check regular hours
        if current_time >= market_config['regular_open'] and current_time < market_config['regular_close']:
            return MarketStatus.OPEN, "Market open - regular hours"
            
        # Check after-hours
        if current_time >= market_config['regular_close'] and current_time <= market_config['after_hours_close']:
            return MarketStatus.AFTER_HOURS, "After-hours trading"
            
        # Market is closed
        return MarketStatus.CLOSED, "Market closed - outside trading hours"
    
    @classmethod
    def is_market_open(cls, 
                      market_type: str = 'US_STOCK',
                      include_extended_hours: bool = True,
                      check_time: Optional[datetime] = None) -> bool:
        """
        Simple check if market is open.
        
        Args:
            market_type: Type of market ('US_STOCK', 'CRYPTO', 'FOREX')
            include_extended_hours: Include pre-market and after-hours
            check_time: Time to check (defaults to current time)
            
        Returns:
            True if market is open, False otherwise
        """
        status, _ = cls.get_market_status(market_type, check_time)
        
        if include_extended_hours:
            return status in [MarketStatus.OPEN, MarketStatus.PRE_MARKET, MarketStatus.AFTER_HOURS]
        else:
            return status == MarketStatus.OPEN
    
    @classmethod
    def get_next_market_open(cls, market_type: str = 'US_STOCK') -> Optional[datetime]:
        """Get the next market open time."""
        now = datetime.now(pytz.UTC)
        
        if market_type == 'CRYPTO':
            # Crypto is always open
            return now
            
        # For US Stock market
        if market_type == 'US_STOCK':
            market_tz = pytz.timezone('US/Eastern')
            current_et = now.astimezone(market_tz)
            
            # If it's a weekday and before market open
            if current_et.weekday() < 5:
                next_open = current_et.replace(
                    hour=9, minute=30, second=0, microsecond=0
                )
                if current_et < next_open:
                    return next_open.astimezone(pytz.UTC)
                    
            # Find next weekday
            days_ahead = 1
            while True:
                next_day = current_et + timedelta(days=days_ahead)
                if next_day.weekday() < 5:  # Monday to Friday
                    next_open = next_day.replace(
                        hour=9, minute=30, second=0, microsecond=0
                    )
                    return next_open.astimezone(pytz.UTC)
                days_ahead += 1
                if days_ahead > 7:  # Safety check
                    break
                    
        return None


def is_market_data_expected(provider: str) -> Tuple[bool, str]:
    """
    Check if market data is expected for a given provider.
    
    Args:
        provider: Provider name (e.g., 'alpaca', 'polygon', 'binance')
        
    Returns:
        Tuple of (should_expect_data, reason)
    """
    # Map providers to market types
    provider_market_map = {
        'alpaca': 'US_STOCK',
        'polygon': 'US_STOCK',
        'finnhub': 'US_STOCK',
        'binance': 'CRYPTO',
        'coinbase': 'CRYPTO',
    }
    
    market_type = provider_market_map.get(provider.lower(), 'US_STOCK')
    status, message = MarketHours.get_market_status(market_type)
    
    # For crypto, always expect data
    if market_type == 'CRYPTO':
        return True, "Crypto markets are 24/7"
        
    # For stock markets, check status
    if status in [MarketStatus.OPEN, MarketStatus.PRE_MARKET, MarketStatus.AFTER_HOURS]:
        return True, message
    else:
        return False, message