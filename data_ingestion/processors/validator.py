"""Data validation utilities for market data."""
import pandas as pd
from typing import List, Dict, Any, Optional, Tuple
from datetime import datetime, time
import pytz

from ..utils.logging import get_logger
from ..utils.metrics import metrics


logger = get_logger(__name__)


class DataValidator:
    """Validate market data quality and consistency."""
    
    def __init__(self):
        self.market_timezone = pytz.timezone('US/Eastern')
        self.regular_market_open = time(9, 30)  # 9:30 AM ET
        self.regular_market_close = time(16, 0)  # 4:00 PM ET
        self.premarket_open = time(4, 0)  # 4:00 AM ET
        self.afterhours_close = time(20, 0)  # 8:00 PM ET
    
    def validate_batch(self, data: List[Dict[str, Any]]) -> Tuple[List[Dict[str, Any]], List[Dict[str, Any]]]:
        """
        Validate a batch of data records.
        Returns (valid_records, invalid_records)
        """
        valid = []
        invalid = []
        
        for record in data:
            validation_result = self.validate_record(record)
            if validation_result['is_valid']:
                valid.append(record)
            else:
                invalid.append({
                    'record': record,
                    'errors': validation_result['errors']
                })
        
        if invalid:
            logger.warning(f"Validation failed for {len(invalid)} records")
            metrics.validation_failures.labels(batch_size=len(data)).inc(len(invalid))
        
        return valid, invalid
    
    def validate_record(self, record: Dict[str, Any]) -> Dict[str, Any]:
        """
        Validate a single record.
        Returns validation result with is_valid flag and error details.
        """
        errors = []
        warnings = []
        
        # Check required fields
        required_fields = ['time', 'symbol', 'close']
        for field in required_fields:
            if field not in record or record[field] is None:
                errors.append(f"Missing required field: {field}")
        
        if errors:
            return {
                'is_valid': False,
                'errors': errors,
                'warnings': warnings
            }
        
        # Validate timestamp
        time_validation = self._validate_timestamp(record['time'])
        if not time_validation['is_valid']:
            errors.extend(time_validation['errors'])
        warnings.extend(time_validation.get('warnings', []))
        
        # Validate symbol
        symbol_validation = self._validate_symbol(record['symbol'])
        if not symbol_validation['is_valid']:
            errors.extend(symbol_validation['errors'])
        
        # Validate prices
        price_validation = self._validate_prices(record)
        if not price_validation['is_valid']:
            errors.extend(price_validation['errors'])
        warnings.extend(price_validation.get('warnings', []))
        
        # Validate volume
        if 'volume' in record:
            volume_validation = self._validate_volume(record['volume'])
            if not volume_validation['is_valid']:
                errors.extend(volume_validation['errors'])
        
        return {
            'is_valid': len(errors) == 0,
            'errors': errors,
            'warnings': warnings
        }
    
    def _validate_timestamp(self, timestamp: Any) -> Dict[str, Any]:
        """Validate timestamp field."""
        errors = []
        warnings = []
        
        # Convert to datetime if needed
        try:
            if isinstance(timestamp, str):
                dt = pd.to_datetime(timestamp)
            elif isinstance(timestamp, (datetime, pd.Timestamp)):
                dt = pd.to_datetime(timestamp)
            else:
                errors.append(f"Invalid timestamp type: {type(timestamp)}")
                return {'is_valid': False, 'errors': errors}
        except Exception as e:
            errors.append(f"Failed to parse timestamp: {str(e)}")
            return {'is_valid': False, 'errors': errors}
        
        # Check if timestamp is in the future
        if dt > datetime.now(pytz.UTC):
            errors.append("Timestamp is in the future")
        
        # Check if timestamp is too old (more than 20 years)
        if dt < datetime.now(pytz.UTC) - pd.Timedelta(days=365*20):
            warnings.append("Timestamp is more than 20 years old")
        
        # Check if timestamp is during market hours (warning only)
        if hasattr(dt, 'tz_localize'):
            try:
                et_time = dt.tz_localize('UTC').tz_convert(self.market_timezone)
            except:
                et_time = dt
        else:
            et_time = dt
        
        if hasattr(et_time, 'time'):
            market_time = et_time.time()
            if not (self.premarket_open <= market_time <= self.afterhours_close):
                warnings.append("Timestamp is outside extended market hours")
        
        return {
            'is_valid': len(errors) == 0,
            'errors': errors,
            'warnings': warnings
        }
    
    def _validate_symbol(self, symbol: Any) -> Dict[str, Any]:
        """Validate symbol field."""
        errors = []
        
        if not isinstance(symbol, str):
            errors.append(f"Symbol must be string, got {type(symbol)}")
            return {'is_valid': False, 'errors': errors}
        
        # Check length
        if len(symbol) == 0:
            errors.append("Symbol cannot be empty")
        elif len(symbol) > 10:
            errors.append("Symbol too long (max 10 characters)")
        
        # Check characters (alphanumeric, dots, and dashes allowed)
        if not all(c.isalnum() or c in '.-' for c in symbol):
            errors.append("Symbol contains invalid characters")
        
        return {'is_valid': len(errors) == 0, 'errors': errors}
    
    def _validate_prices(self, record: Dict[str, Any]) -> Dict[str, Any]:
        """Validate price fields."""
        errors = []
        warnings = []
        
        price_fields = ['open', 'high', 'low', 'close']
        prices = {}
        
        # Validate individual prices
        for field in price_fields:
            if field in record and record[field] is not None:
                try:
                    price = float(record[field])
                    prices[field] = price
                    
                    if price <= 0:
                        errors.append(f"{field} price must be positive")
                    elif price > 1000000:
                        warnings.append(f"{field} price unusually high: {price}")
                    elif price < 0.01:
                        warnings.append(f"{field} price unusually low: {price}")
                        
                except (ValueError, TypeError):
                    errors.append(f"Invalid {field} price: {record[field]}")
        
        # Validate OHLC relationships if all prices present
        if all(field in prices for field in price_fields):
            if prices['high'] < prices['low']:
                errors.append("High price is less than low price")
            if prices['high'] < prices['open'] or prices['high'] < prices['close']:
                errors.append("High price is not the highest")
            if prices['low'] > prices['open'] or prices['low'] > prices['close']:
                errors.append("Low price is not the lowest")
            
            # Check for unusual spreads
            spread = (prices['high'] - prices['low']) / prices['low'] * 100
            if spread > 50:  # More than 50% spread
                warnings.append(f"Unusually large high-low spread: {spread:.1f}%")
        
        return {
            'is_valid': len(errors) == 0,
            'errors': errors,
            'warnings': warnings
        }
    
    def _validate_volume(self, volume: Any) -> Dict[str, Any]:
        """Validate volume field."""
        errors = []
        
        try:
            vol = int(volume)
            if vol < 0:
                errors.append("Volume cannot be negative")
            elif vol > 1e12:  # 1 trillion
                errors.append("Volume unrealistically high")
        except (ValueError, TypeError):
            errors.append(f"Invalid volume value: {volume}")
        
        return {'is_valid': len(errors) == 0, 'errors': errors}
    
    def validate_dataframe(self, df: pd.DataFrame) -> Dict[str, Any]:
        """Validate entire DataFrame."""
        issues = {
            'missing_data': [],
            'duplicates': [],
            'gaps': [],
            'anomalies': []
        }
        
        if df.empty:
            return {
                'is_valid': False,
                'issues': issues,
                'summary': "DataFrame is empty"
            }
        
        # Check for required columns
        required_columns = ['time', 'symbol', 'close']
        missing_columns = [col for col in required_columns if col not in df.columns]
        if missing_columns:
            issues['missing_data'].append(f"Missing columns: {missing_columns}")
        
        # Check for duplicates
        duplicate_mask = df.duplicated(subset=['time', 'symbol'], keep=False)
        if duplicate_mask.any():
            duplicate_count = duplicate_mask.sum()
            issues['duplicates'].append(f"Found {duplicate_count} duplicate records")
        
        # Check for time gaps (per symbol)
        for symbol in df['symbol'].unique():
            symbol_data = df[df['symbol'] == symbol].sort_values('time')
            if len(symbol_data) > 1:
                time_diffs = symbol_data['time'].diff()
                median_diff = time_diffs.median()
                
                # Detect gaps larger than 10x median interval
                if median_diff > pd.Timedelta(0):
                    large_gaps = time_diffs > median_diff * 10
                    if large_gaps.any():
                        gap_count = large_gaps.sum()
                        issues['gaps'].append(f"{symbol}: {gap_count} time gaps detected")
        
        # Check for price anomalies
        if 'close' in df.columns:
            # Sudden price changes (more than 20% in one interval)
            for symbol in df['symbol'].unique():
                symbol_data = df[df['symbol'] == symbol].sort_values('time')
                if len(symbol_data) > 1:
                    returns = symbol_data['close'].pct_change().abs()
                    anomalies = returns > 0.2
                    if anomalies.any():
                        anomaly_count = anomalies.sum()
                        issues['anomalies'].append(
                            f"{symbol}: {anomaly_count} price jumps >20%"
                        )
        
        # Calculate overall validity
        total_issues = sum(len(v) for v in issues.values())
        
        return {
            'is_valid': total_issues == 0,
            'issues': issues,
            'summary': f"Found {total_issues} validation issues",
            'record_count': len(df),
            'symbol_count': df['symbol'].nunique() if 'symbol' in df.columns else 0,
            'time_range': {
                'start': df['time'].min() if 'time' in df.columns else None,
                'end': df['time'].max() if 'time' in df.columns else None
            }
        }
    
    def validate_realtime_data(self, data: Dict[str, Any]) -> Dict[str, Any]:
        """Validate real-time streaming data."""
        validation = self.validate_record(data)
        
        # Additional real-time specific checks
        if validation['is_valid'] and 'time' in data:
            # Check if data is stale (more than 1 minute old)
            try:
                data_time = pd.to_datetime(data['time'])
                if isinstance(data_time, pd.Timestamp):
                    data_time = data_time.to_pydatetime()
                
                age = datetime.now(pytz.UTC) - data_time.replace(tzinfo=pytz.UTC)
                if age.total_seconds() > 60:
                    validation['warnings'].append(f"Data is {age.total_seconds():.0f} seconds old")
            except:
                pass
        
        return validation