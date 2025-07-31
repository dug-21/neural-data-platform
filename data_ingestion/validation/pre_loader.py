"""
Pre-load validation for historical market data.

Validates data format, consistency, and completeness before database insertion.
"""

import asyncio
from typing import List, Dict, Any, Optional, Tuple, Set
from datetime import datetime, timedelta
from dataclasses import dataclass, field
import numpy as np
from decimal import Decimal
import pandas as pd
from collections import defaultdict

from ..providers.base import MarketData, TickData, DataGranularity
from ..utils.logging import get_logger
from ..utils.metrics import metrics


@dataclass
class PreValidationResult:
    """Result of pre-load validation."""
    is_valid: bool
    total_records: int
    valid_records: int
    invalid_records: int
    validation_errors: List[Dict[str, Any]] = field(default_factory=list)
    warnings: List[str] = field(default_factory=list)
    statistics: Dict[str, Any] = field(default_factory=dict)
    
    @property
    def validation_score(self) -> float:
        """Calculate validation score (0-1)."""
        if self.total_records == 0:
            return 0.0
        return self.valid_records / self.total_records
    
    @property
    def error_rate(self) -> float:
        """Calculate error rate percentage."""
        if self.total_records == 0:
            return 0.0
        return (self.invalid_records / self.total_records) * 100


class PreLoadValidator:
    """Validates market data before loading into database."""
    
    # Validation thresholds
    MAX_PRICE_CHANGE_PCT = 50.0  # Maximum allowed price change %
    MIN_VOLUME = 0
    MAX_VOLUME = 10_000_000_000  # 10 billion max volume
    MIN_PRICE = 0.0001
    MAX_PRICE = 1_000_000.0
    
    # Time validation
    MAX_FUTURE_TIME = timedelta(minutes=5)  # Allow 5 min future timestamps
    MAX_PAST_TIME = timedelta(days=365 * 50)  # 50 years of historical data
    
    def __init__(self):
        self.logger = get_logger(__name__)
        self._symbol_stats: Dict[str, Dict[str, Any]] = defaultdict(dict)
        
    async def validate_batch(
        self,
        data_batch: List[MarketData],
        granularity: DataGranularity,
        check_continuity: bool = True
    ) -> PreValidationResult:
        """Validate a batch of market data records."""
        if not data_batch:
            return PreValidationResult(
                is_valid=False,
                total_records=0,
                valid_records=0,
                invalid_records=0,
                validation_errors=[{"error": "Empty data batch"}]
            )
            
        start_time = asyncio.get_event_loop().time()
        
        # Sort by timestamp for continuity checks
        data_batch.sort(key=lambda x: x.time)
        
        # Run validation checks
        validation_tasks = [
            self._validate_format(data_batch),
            self._validate_timestamps(data_batch, granularity),
            self._validate_ohlc_consistency(data_batch),
            self._validate_volume(data_batch),
            self._validate_price_ranges(data_batch),
        ]
        
        if check_continuity:
            validation_tasks.append(self._validate_continuity(data_batch, granularity))
            
        results = await asyncio.gather(*validation_tasks)
        
        # Aggregate results
        total_errors = []
        warnings = []
        
        for errors, warns in results:
            total_errors.extend(errors)
            warnings.extend(warns)
            
        # Mark invalid records
        invalid_indices = set()
        for error in total_errors:
            if 'index' in error:
                invalid_indices.add(error['index'])
                
        valid_records = len(data_batch) - len(invalid_indices)
        
        # Calculate statistics
        statistics = await self._calculate_statistics(data_batch, invalid_indices)
        
        # Track metrics
        validation_time = asyncio.get_event_loop().time() - start_time
        metrics.validation_duration.labels(
            stage='pre_load',
            data_type='market_data'
        ).observe(validation_time)
        
        metrics.validation_errors.labels(
            stage='pre_load',
            error_type='total'
        ).inc(len(total_errors))
        
        result = PreValidationResult(
            is_valid=len(invalid_indices) < len(data_batch) * 0.05,  # 95% threshold
            total_records=len(data_batch),
            valid_records=valid_records,
            invalid_records=len(invalid_indices),
            validation_errors=total_errors[:1000],  # Limit to 1000 errors
            warnings=warnings[:100],  # Limit to 100 warnings
            statistics=statistics
        )
        
        self.logger.info(
            f"Pre-validation completed: {result.valid_records}/{result.total_records} "
            f"valid ({result.validation_score:.2%}), {validation_time:.2f}s"
        )
        
        return result
        
    async def _validate_format(
        self, 
        data_batch: List[MarketData]
    ) -> Tuple[List[Dict], List[str]]:
        """Validate data format and required fields."""
        errors = []
        warnings = []
        
        for idx, record in enumerate(data_batch):
            # Check required fields
            if not record.symbol:
                errors.append({
                    'index': idx,
                    'error': 'Missing symbol',
                    'record': str(record)
                })
                
            if not record.time:
                errors.append({
                    'index': idx,
                    'error': 'Missing timestamp',
                    'record': str(record)
                })
                
            # Check numeric fields
            for field in ['open', 'high', 'low', 'close']:
                value = getattr(record, field, None)
                if value is None:
                    errors.append({
                        'index': idx,
                        'error': f'Missing {field} price',
                        'symbol': record.symbol
                    })
                elif not isinstance(value, (int, float, Decimal)):
                    errors.append({
                        'index': idx,
                        'error': f'Invalid {field} price type: {type(value)}',
                        'symbol': record.symbol
                    })
                    
            # Volume validation
            if record.volume is None:
                warnings.append(f"Missing volume for {record.symbol} at {record.time}")
            elif not isinstance(record.volume, (int, float)):
                errors.append({
                    'index': idx,
                    'error': f'Invalid volume type: {type(record.volume)}',
                    'symbol': record.symbol
                })
                
        return errors, warnings
        
    async def _validate_timestamps(
        self,
        data_batch: List[MarketData],
        granularity: DataGranularity
    ) -> Tuple[List[Dict], List[str]]:
        """Validate timestamps for consistency and range."""
        errors = []
        warnings = []
        now = datetime.now()
        
        # Expected interval for granularity
        expected_interval = self._get_expected_interval(granularity)
        
        for idx, record in enumerate(data_batch):
            # Check timestamp range
            if record.time > now + self.MAX_FUTURE_TIME:
                errors.append({
                    'index': idx,
                    'error': f'Timestamp too far in future: {record.time}',
                    'symbol': record.symbol
                })
            elif record.time < now - self.MAX_PAST_TIME:
                errors.append({
                    'index': idx,
                    'error': f'Timestamp too far in past: {record.time}',
                    'symbol': record.symbol
                })
                
            # Check for weekend data (if not crypto)
            if not self._is_crypto(record.symbol):
                if record.time.weekday() >= 5:  # Saturday or Sunday
                    warnings.append(
                        f"Weekend data for non-crypto {record.symbol} at {record.time}"
                    )
                    
            # Check timestamp alignment for minute/hour data
            if granularity in [DataGranularity.MINUTE, DataGranularity.HOUR]:
                if record.time.second != 0 or record.time.microsecond != 0:
                    warnings.append(
                        f"Unaligned timestamp for {granularity.value} data: {record.time}"
                    )
                    
        # Check for duplicate timestamps
        timestamps_by_symbol = defaultdict(list)
        for idx, record in enumerate(data_batch):
            timestamps_by_symbol[record.symbol].append((idx, record.time))
            
        for symbol, timestamp_list in timestamps_by_symbol.items():
            seen = set()
            for idx, ts in timestamp_list:
                if ts in seen:
                    errors.append({
                        'index': idx,
                        'error': f'Duplicate timestamp: {ts}',
                        'symbol': symbol
                    })
                seen.add(ts)
                
        return errors, warnings
        
    async def _validate_ohlc_consistency(
        self,
        data_batch: List[MarketData]
    ) -> Tuple[List[Dict], List[str]]:
        """Validate OHLC price consistency."""
        errors = []
        warnings = []
        
        for idx, record in enumerate(data_batch):
            try:
                # Basic OHLC rules
                if record.high < record.low:
                    errors.append({
                        'index': idx,
                        'error': f'High ({record.high}) < Low ({record.low})',
                        'symbol': record.symbol,
                        'time': record.time
                    })
                    
                if record.high < record.open or record.high < record.close:
                    errors.append({
                        'index': idx,
                        'error': f'High ({record.high}) not highest price',
                        'symbol': record.symbol,
                        'time': record.time
                    })
                    
                if record.low > record.open or record.low > record.close:
                    errors.append({
                        'index': idx,
                        'error': f'Low ({record.low}) not lowest price',
                        'symbol': record.symbol,
                        'time': record.time
                    })
                    
                # Check for zero or negative prices
                for field in ['open', 'high', 'low', 'close']:
                    value = getattr(record, field)
                    if value <= 0:
                        errors.append({
                            'index': idx,
                            'error': f'Invalid {field} price: {value}',
                            'symbol': record.symbol,
                            'time': record.time
                        })
                        
                # Check for unusual price spreads
                if record.high > 0 and record.low > 0:
                    spread_pct = ((record.high - record.low) / record.low) * 100
                    if spread_pct > self.MAX_PRICE_CHANGE_PCT:
                        warnings.append(
                            f"Large intraday spread {spread_pct:.1f}% for "
                            f"{record.symbol} at {record.time}"
                        )
                        
            except Exception as e:
                errors.append({
                    'index': idx,
                    'error': f'OHLC validation error: {str(e)}',
                    'symbol': record.symbol
                })
                
        return errors, warnings
        
    async def _validate_volume(
        self,
        data_batch: List[MarketData]
    ) -> Tuple[List[Dict], List[str]]:
        """Validate volume data."""
        errors = []
        warnings = []
        
        # Group by symbol for volume analysis
        volume_by_symbol = defaultdict(list)
        
        for idx, record in enumerate(data_batch):
            # Check volume bounds
            if record.volume < self.MIN_VOLUME:
                errors.append({
                    'index': idx,
                    'error': f'Negative volume: {record.volume}',
                    'symbol': record.symbol,
                    'time': record.time
                })
            elif record.volume > self.MAX_VOLUME:
                errors.append({
                    'index': idx,
                    'error': f'Volume exceeds maximum: {record.volume}',
                    'symbol': record.symbol,
                    'time': record.time
                })
                
            volume_by_symbol[record.symbol].append((idx, record.volume))
            
        # Check for volume anomalies by symbol
        for symbol, volumes in volume_by_symbol.items():
            if len(volumes) > 10:  # Need sufficient data
                volume_values = [v[1] for v in volumes if v[1] > 0]
                if volume_values:
                    mean_vol = np.mean(volume_values)
                    std_vol = np.std(volume_values)
                    
                    # Check for outliers (> 5 std dev)
                    for idx, vol in volumes:
                        if vol > 0 and std_vol > 0:
                            z_score = abs((vol - mean_vol) / std_vol)
                            if z_score > 5:
                                warnings.append(
                                    f"Volume outlier detected for {symbol}: "
                                    f"{vol:,} (z-score: {z_score:.1f})"
                                )
                                
        return errors, warnings
        
    async def _validate_price_ranges(
        self,
        data_batch: List[MarketData]
    ) -> Tuple[List[Dict], List[str]]:
        """Validate price ranges and detect anomalies."""
        errors = []
        warnings = []
        
        # Group by symbol
        prices_by_symbol = defaultdict(list)
        
        for idx, record in enumerate(data_batch):
            # Check absolute price bounds
            for field in ['open', 'high', 'low', 'close']:
                price = getattr(record, field)
                if price < self.MIN_PRICE:
                    errors.append({
                        'index': idx,
                        'error': f'{field} price below minimum: {price}',
                        'symbol': record.symbol,
                        'time': record.time
                    })
                elif price > self.MAX_PRICE:
                    errors.append({
                        'index': idx,
                        'error': f'{field} price above maximum: {price}',
                        'symbol': record.symbol,
                        'time': record.time
                    })
                    
            prices_by_symbol[record.symbol].append((idx, record))
            
        # Check for price jumps between consecutive records
        for symbol, records in prices_by_symbol.items():
            records.sort(key=lambda x: x[1].time)
            
            for i in range(1, len(records)):
                prev_record = records[i-1][1]
                curr_record = records[i][1]
                
                # Calculate price change
                price_change_pct = abs(
                    (curr_record.close - prev_record.close) / prev_record.close * 100
                )
                
                if price_change_pct > self.MAX_PRICE_CHANGE_PCT:
                    warnings.append(
                        f"Large price jump {price_change_pct:.1f}% for {symbol} "
                        f"between {prev_record.time} and {curr_record.time}"
                    )
                    
        return errors, warnings
        
    async def _validate_continuity(
        self,
        data_batch: List[MarketData],
        granularity: DataGranularity
    ) -> Tuple[List[Dict], List[str]]:
        """Check for gaps in time series data."""
        errors = []
        warnings = []
        
        # Expected interval
        expected_interval = self._get_expected_interval(granularity)
        max_gap = expected_interval * 10  # Allow up to 10x expected interval
        
        # Group by symbol
        data_by_symbol = defaultdict(list)
        for record in data_batch:
            data_by_symbol[record.symbol].append(record)
            
        for symbol, records in data_by_symbol.items():
            records.sort(key=lambda x: x.time)
            
            gaps = []
            for i in range(1, len(records)):
                time_diff = records[i].time - records[i-1].time
                
                # Account for market hours for non-crypto
                if not self._is_crypto(symbol):
                    # Skip gap check over weekends
                    if records[i-1].time.weekday() == 4 and records[i].time.weekday() == 0:
                        continue
                        
                    # Skip gap check over market close (simplified)
                    if records[i-1].time.hour >= 16 and records[i].time.hour < 9:
                        continue
                        
                if time_diff > max_gap:
                    gaps.append({
                        'start': records[i-1].time,
                        'end': records[i].time,
                        'duration': time_diff
                    })
                    
            if gaps:
                warnings.append(
                    f"Found {len(gaps)} time gaps for {symbol}, "
                    f"largest: {max(g['duration'] for g in gaps)}"
                )
                
        return errors, warnings
        
    async def _calculate_statistics(
        self,
        data_batch: List[MarketData],
        invalid_indices: Set[int]
    ) -> Dict[str, Any]:
        """Calculate validation statistics."""
        valid_data = [
            record for idx, record in enumerate(data_batch)
            if idx not in invalid_indices
        ]
        
        if not valid_data:
            return {}
            
        stats = {
            'total_records': len(data_batch),
            'valid_records': len(valid_data),
            'invalid_records': len(invalid_indices),
            'symbols': list(set(r.symbol for r in valid_data)),
            'time_range': {
                'start': min(r.time for r in valid_data),
                'end': max(r.time for r in valid_data)
            }
        }
        
        # Price statistics
        all_prices = []
        for record in valid_data:
            all_prices.extend([record.open, record.high, record.low, record.close])
            
        if all_prices:
            stats['price_stats'] = {
                'min': float(min(all_prices)),
                'max': float(max(all_prices)),
                'mean': float(np.mean(all_prices)),
                'median': float(np.median(all_prices)),
                'std': float(np.std(all_prices))
            }
            
        # Volume statistics
        volumes = [r.volume for r in valid_data if r.volume > 0]
        if volumes:
            stats['volume_stats'] = {
                'min': int(min(volumes)),
                'max': int(max(volumes)),
                'mean': float(np.mean(volumes)),
                'median': float(np.median(volumes)),
                'total': int(sum(volumes))
            }
            
        return stats
        
    def _get_expected_interval(self, granularity: DataGranularity) -> timedelta:
        """Get expected time interval for granularity."""
        intervals = {
            DataGranularity.TICK: timedelta(seconds=1),
            DataGranularity.MINUTE: timedelta(minutes=1),
            DataGranularity.MINUTE_5: timedelta(minutes=5),
            DataGranularity.MINUTE_15: timedelta(minutes=15),
            DataGranularity.MINUTE_30: timedelta(minutes=30),
            DataGranularity.HOUR: timedelta(hours=1),
            DataGranularity.HOUR_4: timedelta(hours=4),
            DataGranularity.DAY: timedelta(days=1),
            DataGranularity.WEEK: timedelta(days=7),
            DataGranularity.MONTH: timedelta(days=30)
        }
        return intervals.get(granularity, timedelta(minutes=1))
        
    def _is_crypto(self, symbol: str) -> bool:
        """Check if symbol is cryptocurrency."""
        crypto_suffixes = ['USD', 'USDT', 'BTC', 'ETH', 'BUSD']
        return any(symbol.endswith(suffix) for suffix in crypto_suffixes)