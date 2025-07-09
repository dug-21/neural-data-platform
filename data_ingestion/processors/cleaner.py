"""Data cleaning utilities for market data."""
import pandas as pd
import numpy as np
from typing import List, Dict, Any, Optional, Union
from datetime import datetime, timezone

from utils.logging import get_logger
from utils.metrics import metrics


logger = get_logger(__name__)


class DataCleaner:
    """Clean and preprocess market data."""
    
    def __init__(self):
        self.outlier_threshold = 3  # Standard deviations for outlier detection
        self.min_price = 0.01  # Minimum valid price
        self.max_price = 1000000  # Maximum valid price
        self.min_volume = 0  # Minimum valid volume
    
    def clean_market_data(self, data: List[Dict[str, Any]]) -> List[Dict[str, Any]]:
        """Clean market data records."""
        if not data:
            return []
        
        cleaned = []
        removed_count = 0
        
        for record in data:
            # Basic validation
            if not self._validate_record(record):
                removed_count += 1
                continue
            
            # Clean the record
            cleaned_record = self._clean_record(record)
            
            # Skip if cleaning failed
            if cleaned_record:
                cleaned.append(cleaned_record)
            else:
                removed_count += 1
        
        if removed_count > 0:
            logger.warning(f"Removed {removed_count} invalid records during cleaning")
            metrics.data_quality_issues.labels(issue_type="invalid_records").inc(removed_count)
        
        return cleaned
    
    def clean_dataframe(self, df: pd.DataFrame) -> pd.DataFrame:
        """Clean market data DataFrame."""
        if df.empty:
            return df
        
        original_size = len(df)
        
        # Remove duplicates
        df = df.drop_duplicates(subset=['time', 'symbol'], keep='last')
        
        # Handle missing values
        df = self._handle_missing_values(df)
        
        # Remove outliers
        df = self._remove_outliers(df)
        
        # Validate price relationships
        df = self._validate_price_relationships(df)
        
        # Sort by time
        df = df.sort_values('time')
        
        # Reset index
        df = df.reset_index(drop=True)
        
        cleaned_size = len(df)
        if cleaned_size < original_size:
            removed = original_size - cleaned_size
            logger.info(f"Cleaned DataFrame: removed {removed} rows ({removed/original_size*100:.2f}%)")
            metrics.data_quality_issues.labels(issue_type="dataframe_cleaning").inc(removed)
        
        return df
    
    def _validate_record(self, record: Dict[str, Any]) -> bool:
        """Validate a single record."""
        # Check required fields
        required_fields = ['time', 'symbol', 'close']
        for field in required_fields:
            if field not in record or record[field] is None:
                logger.debug(f"Missing required field: {field}")
                return False
        
        # Validate time
        if not isinstance(record['time'], (datetime, pd.Timestamp)):
            try:
                record['time'] = pd.to_datetime(record['time'], utc=True)
            except:
                logger.debug(f"Invalid time format: {record.get('time')}")
                return False
        
        # Validate symbol
        if not isinstance(record['symbol'], str) or not record['symbol']:
            logger.debug(f"Invalid symbol: {record.get('symbol')}")
            return False
        
        # Validate prices
        price_fields = ['open', 'high', 'low', 'close']
        for field in price_fields:
            if field in record and record[field] is not None:
                try:
                    price = float(record[field])
                    if not (self.min_price <= price <= self.max_price):
                        logger.debug(f"Price out of range: {field}={price}")
                        return False
                except (ValueError, TypeError):
                    logger.debug(f"Invalid price value: {field}={record.get(field)}")
                    return False
        
        # Validate volume
        if 'volume' in record and record['volume'] is not None:
            try:
                # Handle string volumes with commas or decimals
                volume_str = str(record['volume']).replace(',', '')
                volume = int(float(volume_str))
                if volume < self.min_volume:
                    logger.debug(f"Invalid volume: {volume}")
                    return False
            except (ValueError, TypeError):
                logger.debug(f"Invalid volume value: {record.get('volume')}")
                return False
        
        return True
    
    def _clean_record(self, record: Dict[str, Any]) -> Optional[Dict[str, Any]]:
        """Clean a single record."""
        cleaned = record.copy()
        
        try:
            # Ensure proper types
            cleaned['time'] = pd.to_datetime(cleaned['time'], utc=True)
            cleaned['symbol'] = str(cleaned['symbol']).upper().strip()
            
            # Clean price fields
            for field in ['open', 'high', 'low', 'close']:
                if field in cleaned and cleaned[field] is not None:
                    try:
                        # Handle string prices with commas
                        price_str = str(cleaned[field]).replace(',', '')
                        cleaned[field] = float(price_str)
                    except (ValueError, TypeError):
                        logger.debug(f"Could not convert {field} to float: {cleaned.get(field)}")
                        # Use close price as fallback
                        if field != 'close' and 'close' in cleaned:
                            cleaned[field] = cleaned.get('close', 0.0)
                        else:
                            cleaned[field] = 0.0
                else:
                    # Use close price as fallback
                    if field != 'close' and 'close' in cleaned:
                        cleaned[field] = cleaned.get('close', 0.0)
            
            # Clean volume
            if 'volume' in cleaned and cleaned['volume'] is not None:
                try:
                    cleaned['volume'] = int(float(str(cleaned['volume']).replace(',', '')))
                except (ValueError, TypeError):
                    logger.debug(f"Could not convert volume to int: {cleaned.get('volume')}")
                    cleaned['volume'] = 0
            else:
                cleaned['volume'] = 0
            
            # Validate OHLC relationships
            if all(field in cleaned for field in ['open', 'high', 'low', 'close']):
                # High should be >= all other prices
                cleaned['high'] = max(cleaned['open'], cleaned['high'], cleaned['low'], cleaned['close'])
                # Low should be <= all other prices
                cleaned['low'] = min(cleaned['open'], cleaned['low'], cleaned['close'])
            
            return cleaned
            
        except Exception as e:
            logger.debug(f"Failed to clean record: {e}")
            return None
    
    def _handle_missing_values(self, df: pd.DataFrame) -> pd.DataFrame:
        """Handle missing values in DataFrame."""
        # Forward fill for price data (carry forward last known price)
        price_columns = ['open', 'high', 'low', 'close']
        for col in price_columns:
            if col in df.columns:
                df[col] = df.groupby('symbol')[col].fillna(method='ffill')
        
        # Fill remaining NaN prices with close price
        for col in price_columns:
            if col in df.columns and 'close' in df.columns:
                df[col] = df[col].fillna(df['close'])
        
        # Fill volume with 0
        if 'volume' in df.columns:
            df['volume'] = df['volume'].fillna(0)
        
        # Drop rows where we still have NaN in critical columns
        critical_columns = ['time', 'symbol', 'close']
        df = df.dropna(subset=critical_columns)
        
        return df
    
    def _remove_outliers(self, df: pd.DataFrame) -> pd.DataFrame:
        """Remove statistical outliers from DataFrame."""
        if len(df) < 10:  # Not enough data for outlier detection
            return df
        
        # Group by symbol for outlier detection
        cleaned_dfs = []
        
        for symbol, group in df.groupby('symbol'):
            if len(group) < 10:
                cleaned_dfs.append(group)
                continue
            
            # Calculate returns for outlier detection
            group = group.sort_values('time')
            returns = group['close'].pct_change()
            
            # Remove extreme returns (likely data errors)
            mean_return = returns.mean()
            std_return = returns.std()
            
            if std_return > 0:
                z_scores = np.abs((returns - mean_return) / std_return)
                # Keep only returns within threshold standard deviations
                mask = z_scores <= self.outlier_threshold
                mask[0] = True  # Always keep first row (no return calculated)
                group = group[mask]
            
            cleaned_dfs.append(group)
        
        return pd.concat(cleaned_dfs, ignore_index=True) if cleaned_dfs else pd.DataFrame()
    
    def _validate_price_relationships(self, df: pd.DataFrame) -> pd.DataFrame:
        """Validate OHLC price relationships."""
        if not all(col in df.columns for col in ['open', 'high', 'low', 'close']):
            return df
        
        # Create mask for valid price relationships
        valid_mask = (
            (df['high'] >= df['open']) &
            (df['high'] >= df['close']) &
            (df['high'] >= df['low']) &
            (df['low'] <= df['open']) &
            (df['low'] <= df['close']) &
            (df['low'] <= df['high']) &
            (df['high'] > 0) &
            (df['low'] > 0)
        )
        
        invalid_count = (~valid_mask).sum()
        if invalid_count > 0:
            logger.warning(f"Found {invalid_count} records with invalid price relationships")
            metrics.data_quality_issues.labels(issue_type="price_relationships").inc(invalid_count)
        
        return df[valid_mask]
    
    def detect_gaps(self, df: pd.DataFrame, expected_interval: str = '1min') -> List[Dict[str, Any]]:
        """Detect time gaps in data."""
        gaps = []
        
        # Convert interval to timedelta
        interval_map = {
            '1min': pd.Timedelta(minutes=1),
            '5min': pd.Timedelta(minutes=5),
            '15min': pd.Timedelta(minutes=15),
            '30min': pd.Timedelta(minutes=30),
            '1hour': pd.Timedelta(hours=1),
            '1day': pd.Timedelta(days=1)
        }
        
        expected_delta = interval_map.get(expected_interval, pd.Timedelta(minutes=1))
        
        for symbol, group in df.groupby('symbol'):
            group = group.sort_values('time')
            time_diff = group['time'].diff()
            
            # Find gaps larger than expected interval
            gap_mask = time_diff > expected_delta * 1.5  # Allow 50% tolerance
            
            if gap_mask.any():
                gap_indices = group.index[gap_mask]
                for idx in gap_indices:
                    gaps.append({
                        'symbol': symbol,
                        'start': group.loc[idx - 1, 'time'] if idx > 0 else None,
                        'end': group.loc[idx, 'time'],
                        'gap_duration': time_diff.loc[idx]
                    })
        
        if gaps:
            logger.info(f"Detected {len(gaps)} time gaps in data")
            metrics.data_quality_issues.labels(issue_type="time_gaps").inc(len(gaps))
        
        return gaps