"""
Market Hours Implementation Module
Production-ready utilities for handling global market trading hours.

This module provides comprehensive functionality for:
- Checking if markets are open
- Converting between timezones
- Managing holiday calendars
- Calculating trading sessions
"""

from datetime import datetime, time, timedelta
from typing import Dict, List, Optional, Tuple, Union
from enum import Enum
import json
from pathlib import Path
from zoneinfo import ZoneInfo  # Python 3.9+ (preferred over pytz)
from dataclasses import dataclass
from functools import lru_cache


class Exchange(Enum):
    """Supported exchanges with their timezone identifiers."""
    NYSE = "America/New_York"
    NASDAQ = "America/New_York"
    LSE = "Europe/London"
    TSE = "Asia/Tokyo"
    JPX = "Asia/Tokyo"
    XETRA = "Europe/Berlin"
    FSE = "Europe/Berlin"
    HKEX = "Asia/Hong_Kong"


@dataclass
class TradingHours:
    """Trading hours for a specific exchange."""
    exchange: Exchange
    regular_open: time
    regular_close: time
    pre_market_open: Optional[time] = None
    after_hours_close: Optional[time] = None
    lunch_break_start: Optional[time] = None
    lunch_break_end: Optional[time] = None
    

@dataclass
class MarketHoliday:
    """Represents a market holiday."""
    date: str  # ISO format: YYYY-MM-DD
    name: str
    exchange: Exchange
    early_close_time: Optional[time] = None  # For half days


class MarketHoursManager:
    """
    Comprehensive market hours management system.
    
    Features:
    - Real-time market status checking
    - Holiday calendar management
    - Timezone-aware operations
    - Performance optimized with caching
    """
    
    def __init__(self, holidays_file: Optional[Path] = None):
        """
        Initialize the market hours manager.
        
        Args:
            holidays_file: Path to JSON file containing holiday calendars
        """
        self.trading_hours = self._initialize_trading_hours()
        self.holidays = self._load_holidays(holidays_file) if holidays_file else {}
        
    def _initialize_trading_hours(self) -> Dict[Exchange, TradingHours]:
        """Initialize standard trading hours for all exchanges."""
        return {
            Exchange.NYSE: TradingHours(
                exchange=Exchange.NYSE,
                regular_open=time(9, 30),
                regular_close=time(16, 0),
                pre_market_open=time(4, 0),
                after_hours_close=time(20, 0)
            ),
            Exchange.NASDAQ: TradingHours(
                exchange=Exchange.NASDAQ,
                regular_open=time(9, 30),
                regular_close=time(16, 0),
                pre_market_open=time(4, 0),
                after_hours_close=time(20, 0)
            ),
            Exchange.LSE: TradingHours(
                exchange=Exchange.LSE,
                regular_open=time(8, 0),
                regular_close=time(16, 30),
                pre_market_open=time(7, 0),
                after_hours_close=time(17, 0)
            ),
            Exchange.TSE: TradingHours(
                exchange=Exchange.TSE,
                regular_open=time(9, 0),
                regular_close=time(15, 0),
                lunch_break_start=time(11, 30),
                lunch_break_end=time(12, 30)
            ),
            Exchange.JPX: TradingHours(
                exchange=Exchange.JPX,
                regular_open=time(9, 0),
                regular_close=time(15, 25),
                lunch_break_start=time(11, 30),
                lunch_break_end=time(12, 30)
            ),
            Exchange.XETRA: TradingHours(
                exchange=Exchange.XETRA,
                regular_open=time(9, 0),
                regular_close=time(17, 30)
            ),
            Exchange.FSE: TradingHours(
                exchange=Exchange.FSE,
                regular_open=time(8, 0),
                regular_close=time(22, 0)
            ),
            Exchange.HKEX: TradingHours(
                exchange=Exchange.HKEX,
                regular_open=time(9, 30),
                regular_close=time(16, 0),
                lunch_break_start=time(12, 0),
                lunch_break_end=time(13, 0)
            )
        }
    
    def _load_holidays(self, holidays_file: Path) -> Dict[Exchange, List[MarketHoliday]]:
        """Load holiday calendar from JSON file."""
        try:
            with open(holidays_file, 'r') as f:
                data = json.load(f)
            
            holidays = {}
            for exchange_name, holiday_list in data.items():
                exchange = Exchange[exchange_name]
                holidays[exchange] = [
                    MarketHoliday(
                        date=h['date'],
                        name=h['name'],
                        exchange=exchange,
                        early_close_time=time.fromisoformat(h['early_close']) 
                            if h.get('early_close') else None
                    )
                    for h in holiday_list
                ]
            return holidays
        except Exception as e:
            print(f"Warning: Could not load holidays file: {e}")
            return {}
    
    @lru_cache(maxsize=128)
    def is_market_open(
        self, 
        exchange: Exchange, 
        dt: Optional[datetime] = None,
        include_extended_hours: bool = False
    ) -> bool:
        """
        Check if a market is currently open.
        
        Args:
            exchange: The exchange to check
            dt: Datetime to check (defaults to now)
            include_extended_hours: Include pre-market and after-hours
            
        Returns:
            True if market is open, False otherwise
        """
        if dt is None:
            dt = datetime.now(ZoneInfo('UTC'))
        
        # Convert to exchange timezone
        local_dt = self.convert_to_exchange_time(dt, exchange)
        
        # Check if it's a weekend
        if local_dt.weekday() >= 5:  # Saturday = 5, Sunday = 6
            return False
        
        # Check if it's a holiday
        if self.is_holiday(exchange, local_dt.date()):
            return False
        
        # Get trading hours
        hours = self.trading_hours[exchange]
        current_time = local_dt.time()
        
        # Check extended hours if requested
        if include_extended_hours:
            if hours.pre_market_open and current_time >= hours.pre_market_open:
                if hours.after_hours_close and current_time <= hours.after_hours_close:
                    return True
            return False
        
        # Check regular hours
        if hours.lunch_break_start and hours.lunch_break_end:
            # Handle markets with lunch breaks
            morning_session = (
                current_time >= hours.regular_open and 
                current_time < hours.lunch_break_start
            )
            afternoon_session = (
                current_time >= hours.lunch_break_end and 
                current_time <= hours.regular_close
            )
            return morning_session or afternoon_session
        else:
            # Markets without lunch breaks
            return current_time >= hours.regular_open and current_time <= hours.regular_close
    
    def is_holiday(self, exchange: Exchange, date) -> bool:
        """Check if a specific date is a holiday for the exchange."""
        if exchange not in self.holidays:
            return False
        
        date_str = date.isoformat() if hasattr(date, 'isoformat') else str(date)
        return any(h.date == date_str for h in self.holidays.get(exchange, []))
    
    @lru_cache(maxsize=256)
    def convert_to_exchange_time(self, dt: datetime, exchange: Exchange) -> datetime:
        """Convert a datetime to the exchange's local timezone."""
        if dt.tzinfo is None:
            # Assume UTC if no timezone
            dt = dt.replace(tzinfo=ZoneInfo('UTC'))
        
        exchange_tz = ZoneInfo(exchange.value)
        return dt.astimezone(exchange_tz)
    
    def get_next_market_open(self, exchange: Exchange, dt: Optional[datetime] = None) -> datetime:
        """Get the next market open time for an exchange."""
        if dt is None:
            dt = datetime.now(ZoneInfo('UTC'))
        
        local_dt = self.convert_to_exchange_time(dt, exchange)
        hours = self.trading_hours[exchange]
        
        # Start with current day
        next_open = local_dt.replace(
            hour=hours.regular_open.hour,
            minute=hours.regular_open.minute,
            second=0,
            microsecond=0
        )
        
        # If we're past today's open, move to next day
        if local_dt.time() >= hours.regular_open:
            next_open += timedelta(days=1)
        
        # Skip weekends and holidays
        while next_open.weekday() >= 5 or self.is_holiday(exchange, next_open.date()):
            next_open += timedelta(days=1)
        
        return next_open.astimezone(ZoneInfo('UTC'))
    
    def get_trading_sessions(
        self, 
        exchange: Exchange, 
        start_date: datetime,
        end_date: datetime
    ) -> List[Tuple[datetime, datetime]]:
        """
        Get all trading sessions between two dates.
        
        Returns:
            List of (open_time, close_time) tuples in UTC
        """
        sessions = []
        current = start_date.date()
        end = end_date.date()
        
        while current <= end:
            dt = datetime.combine(current, time(12, 0), tzinfo=ZoneInfo(exchange.value))
            
            if dt.weekday() < 5 and not self.is_holiday(exchange, current):
                hours = self.trading_hours[exchange]
                
                # Handle regular session
                open_time = dt.replace(
                    hour=hours.regular_open.hour,
                    minute=hours.regular_open.minute
                ).astimezone(ZoneInfo('UTC'))
                
                close_time = dt.replace(
                    hour=hours.regular_close.hour,
                    minute=hours.regular_close.minute
                ).astimezone(ZoneInfo('UTC'))
                
                # Check for early close
                holiday = next(
                    (h for h in self.holidays.get(exchange, []) 
                     if h.date == current.isoformat() and h.early_close_time),
                    None
                )
                if holiday:
                    close_time = dt.replace(
                        hour=holiday.early_close_time.hour,
                        minute=holiday.early_close_time.minute
                    ).astimezone(ZoneInfo('UTC'))
                
                sessions.append((open_time, close_time))
            
            current += timedelta(days=1)
        
        return sessions
    
    def get_current_trading_day(self, exchange: Exchange) -> Optional[datetime]:
        """Get the current trading day for an exchange (in local time)."""
        local_dt = self.convert_to_exchange_time(datetime.now(ZoneInfo('UTC')), exchange)
        
        if self.is_market_open(exchange):
            return local_dt.date()
        
        # If market is closed, check if we're before today's open
        hours = self.trading_hours[exchange]
        if local_dt.time() < hours.regular_open and local_dt.weekday() < 5:
            if not self.is_holiday(exchange, local_dt.date()):
                return local_dt.date()
        
        # Otherwise, return None (no current trading day)
        return None
    
    def minutes_until_open(self, exchange: Exchange) -> Optional[int]:
        """Get minutes until market opens (None if already open)."""
        if self.is_market_open(exchange):
            return None
        
        now = datetime.now(ZoneInfo('UTC'))
        next_open = self.get_next_market_open(exchange)
        
        delta = next_open - now
        return int(delta.total_seconds() / 60)
    
    def minutes_until_close(self, exchange: Exchange) -> Optional[int]:
        """Get minutes until market closes (None if already closed)."""
        if not self.is_market_open(exchange):
            return None
        
        now = datetime.now(ZoneInfo('UTC'))
        local_now = self.convert_to_exchange_time(now, exchange)
        hours = self.trading_hours[exchange]
        
        # Calculate close time
        close_dt = local_now.replace(
            hour=hours.regular_close.hour,
            minute=hours.regular_close.minute,
            second=0,
            microsecond=0
        )
        
        # Handle lunch break if applicable
        if hours.lunch_break_start and local_now.time() < hours.lunch_break_start:
            # We're in morning session, close is lunch start
            close_dt = local_now.replace(
                hour=hours.lunch_break_start.hour,
                minute=hours.lunch_break_start.minute
            )
        
        delta = close_dt - local_now
        return int(delta.total_seconds() / 60)
    
    def get_market_status_summary(self) -> Dict[str, Dict]:
        """Get current status for all markets."""
        summary = {}
        now = datetime.now(ZoneInfo('UTC'))
        
        for exchange in Exchange:
            local_time = self.convert_to_exchange_time(now, exchange)
            is_open = self.is_market_open(exchange)
            
            status = {
                'exchange': exchange.name,
                'local_time': local_time.strftime('%Y-%m-%d %H:%M:%S %Z'),
                'is_open': is_open,
                'timezone': exchange.value
            }
            
            if is_open:
                status['minutes_until_close'] = self.minutes_until_close(exchange)
            else:
                status['minutes_until_open'] = self.minutes_until_open(exchange)
                status['next_open'] = self.get_next_market_open(exchange).isoformat()
            
            summary[exchange.name] = status
        
        return summary


# Example usage and helper functions
def create_sample_holidays_file(filepath: Path):
    """Create a sample holidays JSON file for 2025."""
    holidays_2025 = {
        "NYSE": [
            {"date": "2025-01-01", "name": "New Year's Day"},
            {"date": "2025-01-20", "name": "Martin Luther King Jr. Day"},
            {"date": "2025-02-17", "name": "Presidents Day"},
            {"date": "2025-04-18", "name": "Good Friday"},
            {"date": "2025-05-26", "name": "Memorial Day"},
            {"date": "2025-06-19", "name": "Juneteenth"},
            {"date": "2025-07-03", "name": "Independence Day (Early Close)", "early_close": "13:00"},
            {"date": "2025-07-04", "name": "Independence Day"},
            {"date": "2025-09-01", "name": "Labor Day"},
            {"date": "2025-11-27", "name": "Thanksgiving Day"},
            {"date": "2025-11-28", "name": "Day After Thanksgiving (Early Close)", "early_close": "13:00"},
            {"date": "2025-12-24", "name": "Christmas Eve (Early Close)", "early_close": "13:00"},
            {"date": "2025-12-25", "name": "Christmas Day"}
        ],
        "LSE": [
            {"date": "2025-01-01", "name": "New Year's Day"},
            {"date": "2025-04-18", "name": "Good Friday"},
            {"date": "2025-04-21", "name": "Easter Monday"},
            {"date": "2025-05-05", "name": "Early May Bank Holiday"},
            {"date": "2025-05-26", "name": "Spring Bank Holiday"},
            {"date": "2025-08-25", "name": "Summer Bank Holiday"},
            {"date": "2025-12-25", "name": "Christmas Day"},
            {"date": "2025-12-26", "name": "Boxing Day"}
        ],
        "TSE": [
            {"date": "2025-01-01", "name": "New Year's Day"},
            {"date": "2025-01-02", "name": "Market Holiday"},
            {"date": "2025-01-03", "name": "Market Holiday"},
            {"date": "2025-01-13", "name": "Coming of Age Day"},
            {"date": "2025-02-11", "name": "National Foundation Day"},
            {"date": "2025-02-24", "name": "Emperor's Birthday (Substitute)"},
            {"date": "2025-03-20", "name": "Vernal Equinox"},
            {"date": "2025-04-29", "name": "Showa Day"},
            {"date": "2025-05-05", "name": "Children's Day"},
            {"date": "2025-05-06", "name": "Greenery Day (Substitute)"},
            {"date": "2025-07-21", "name": "Marine Day"},
            {"date": "2025-08-11", "name": "Mountain Day"},
            {"date": "2025-09-15", "name": "Respect for the Aged Day"},
            {"date": "2025-09-23", "name": "Autumnal Equinox"},
            {"date": "2025-10-13", "name": "Sports Day"},
            {"date": "2025-11-03", "name": "Culture Day"},
            {"date": "2025-11-24", "name": "Labor Thanksgiving Day (Substitute)"},
            {"date": "2025-12-31", "name": "Market Holiday"}
        ]
    }
    
    with open(filepath, 'w') as f:
        json.dump(holidays_2025, f, indent=2)


if __name__ == "__main__":
    # Example usage
    manager = MarketHoursManager()
    
    # Check current market status
    print("Current Market Status:")
    print("-" * 50)
    for exchange in Exchange:
        is_open = manager.is_market_open(exchange)
        status = "OPEN" if is_open else "CLOSED"
        print(f"{exchange.name}: {status}")
    
    # Get detailed summary
    print("\nDetailed Market Summary:")
    print("-" * 50)
    summary = manager.get_market_status_summary()
    for exchange_name, details in summary.items():
        print(f"\n{exchange_name}:")
        for key, value in details.items():
            print(f"  {key}: {value}")