"""Data normalization utilities for consistent data formats across providers."""
from datetime import datetime, timezone
from typing import Union, Optional
import pandas as pd
from decimal import Decimal, ROUND_HALF_UP


class DataNormalizer:
    """Centralized data normalization rules aligned with database and neural network expectations."""
    
    # Price precision for database DECIMAL(10,4)
    PRICE_PRECISION = Decimal('0.01')  # 2 decimal places for standard prices
    VOLUME_PRECISION = 0  # Integer volume
    
    @staticmethod
    def normalize_timestamp(ts: Union[str, datetime, pd.Timestamp, int, float], 
                          interval: str = "1min") -> datetime:
        """
        Normalize timestamps to TIMESTAMPTZ format expected by database.
        
        Args:
            ts: Timestamp in various formats
            interval: Data interval for appropriate rounding
            
        Returns:
            datetime: Normalized timestamp with timezone (UTC)
        """
        # Convert to pandas timestamp for consistent handling
        if isinstance(ts, (int, float)):
            # Assume milliseconds if large number
            if ts > 1e10:
                dt = pd.to_datetime(ts, unit='ms')
            else:
                dt = pd.to_datetime(ts, unit='s')
        else:
            dt = pd.to_datetime(ts)
        
        # Ensure UTC timezone
        if dt.tz is None:
            dt = dt.tz_localize('UTC')
        else:
            dt = dt.tz_convert('UTC')
        
        # Convert to Python datetime
        dt = dt.to_pydatetime()
        
        # Round based on interval
        if interval in ["1min", "5min", "15min", "30min"]:
            # Round to nearest minute for intraday
            dt = dt.replace(second=0, microsecond=0)
        elif interval in ["1hour", "4hour"]:
            # Round to hour
            dt = dt.replace(minute=0, second=0, microsecond=0)
        elif interval in ["1day", "1week", "1month"]:
            # Normalize to midnight UTC for daily data
            dt = dt.replace(hour=0, minute=0, second=0, microsecond=0)
        
        return dt
    
    @staticmethod
    def normalize_price(price: Union[float, str, Decimal]) -> float:
        """
        Normalize price to 2 decimal places for consistency.
        Database uses DECIMAL(10,4) but we standardize to 2 decimals.
        
        Args:
            price: Price value in various formats
            
        Returns:
            float: Normalized price with 2 decimal places
        """
        if price is None or (isinstance(price, str) and price.strip() == ''):
            return 0.0
        
        # Convert to Decimal for precise rounding
        dec_price = Decimal(str(price))
        
        # Round to 2 decimal places
        rounded = dec_price.quantize(DataNormalizer.PRICE_PRECISION, rounding=ROUND_HALF_UP)
        
        return float(rounded)
    
    @staticmethod
    def normalize_volume(volume: Union[int, float, str]) -> int:
        """
        Normalize volume to integer as expected by database and neural networks.
        
        Args:
            volume: Volume value in various formats
            
        Returns:
            int: Normalized volume as integer
        """
        if volume is None or (isinstance(volume, str) and volume.strip() == ''):
            return 0
        
        # Convert and round to integer
        return int(float(volume))
    
    @staticmethod
    def normalize_symbol(symbol: str) -> str:
        """
        Normalize symbol to uppercase and strip whitespace.
        Consistent with database VARCHAR(10) constraint.
        
        Args:
            symbol: Trading symbol
            
        Returns:
            str: Normalized symbol
        """
        if not symbol:
            raise ValueError("Symbol cannot be empty")
        
        # Strip whitespace and uppercase
        normalized = symbol.strip().upper()
        
        # Validate length for database constraint
        if len(normalized) > 10:
            raise ValueError(f"Symbol '{normalized}' exceeds 10 character limit")
        
        # Basic validation - alphanumeric with some special chars
        valid_chars = set('ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789.-/')
        if not all(c in valid_chars for c in normalized):
            raise ValueError(f"Symbol '{normalized}' contains invalid characters")
        
        return normalized
    
    @staticmethod
    def validate_ohlc_consistency(open_price: float, high: float, 
                                 low: float, close: float) -> bool:
        """
        Validate OHLC data consistency.
        
        Args:
            open_price: Opening price
            high: High price
            low: Low price
            close: Close price
            
        Returns:
            bool: True if valid, False otherwise
        """
        # All prices must be positive
        if any(p <= 0 for p in [open_price, high, low, close]):
            return False
        
        # High must be highest
        if high < max(open_price, low, close):
            return False
        
        # Low must be lowest
        if low > min(open_price, high, close):
            return False
        
        # High must be >= low
        if high < low:
            return False
        
        return True
    
    @staticmethod
    def normalize_provider_name(provider: str) -> str:
        """
        Normalize provider name for consistency.
        
        Args:
            provider: Provider name
            
        Returns:
            str: Normalized provider name
        """
        return provider.strip().lower().replace(" ", "_")