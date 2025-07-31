"""
Post-load validation for verifying database integrity after data insertion.

Performs comprehensive checks on loaded data including:
- Data completeness verification
- Consistency checks across tables
- Aggregation validation
- Index integrity verification
"""

import asyncio
from typing import List, Dict, Any, Optional, Tuple, Set
from datetime import datetime, timedelta
from dataclasses import dataclass, field
import asyncpg
from decimal import Decimal

from ..utils.logging import get_logger
from ..utils.metrics import metrics


@dataclass
class PostValidationResult:
    """Result of post-load validation."""
    is_valid: bool
    checks_performed: int
    checks_passed: int
    checks_failed: int
    validation_errors: List[Dict[str, Any]] = field(default_factory=list)
    warnings: List[str] = field(default_factory=list)
    statistics: Dict[str, Any] = field(default_factory=dict)
    query_results: Dict[str, Any] = field(default_factory=dict)
    
    @property
    def success_rate(self) -> float:
        """Calculate validation success rate."""
        if self.checks_performed == 0:
            return 0.0
        return self.checks_passed / self.checks_performed


class PostLoadValidator:
    """Validates data integrity after database insertion."""
    
    def __init__(self, db_connection_string: str):
        self.logger = get_logger(__name__)
        self.db_connection_string = db_connection_string
        self._conn_pool: Optional[asyncpg.Pool] = None
        
    async def __aenter__(self):
        """Initialize database connection pool."""
        self._conn_pool = await asyncpg.create_pool(
            self.db_connection_string,
            min_size=2,
            max_size=10,
            command_timeout=60
        )
        return self
        
    async def __aexit__(self, exc_type, exc_val, exc_tb):
        """Close database connection pool."""
        if self._conn_pool:
            await self._conn_pool.close()
            
    async def validate_load(
        self,
        symbol: str,
        start_date: datetime,
        end_date: datetime,
        expected_records: Optional[int] = None,
        granularity: Optional[str] = None
    ) -> PostValidationResult:
        """Perform comprehensive post-load validation."""
        self.logger.info(
            f"Starting post-load validation for {symbol} "
            f"from {start_date} to {end_date}"
        )
        
        start_time = asyncio.get_event_loop().time()
        
        # Run validation checks in parallel
        validation_tasks = [
            self._check_record_count(symbol, start_date, end_date, expected_records),
            self._check_data_completeness(symbol, start_date, end_date),
            self._check_ohlc_consistency(symbol, start_date, end_date),
            self._check_duplicate_records(symbol, start_date, end_date),
            self._check_time_gaps(symbol, start_date, end_date, granularity),
            self._check_aggregation_consistency(symbol, start_date, end_date),
            self._check_index_integrity(symbol),
            self._check_data_distribution(symbol, start_date, end_date)
        ]
        
        results = await asyncio.gather(*validation_tasks, return_exceptions=True)
        
        # Aggregate results
        all_errors = []
        all_warnings = []
        all_statistics = {}
        query_results = {}
        checks_passed = 0
        checks_failed = 0
        
        for i, result in enumerate(results):
            if isinstance(result, Exception):
                all_errors.append({
                    'check': validation_tasks[i].__name__,
                    'error': str(result)
                })
                checks_failed += 1
            else:
                passed, errors, warnings, stats, queries = result
                if passed:
                    checks_passed += 1
                else:
                    checks_failed += 1
                    
                all_errors.extend(errors)
                all_warnings.extend(warnings)
                all_statistics.update(stats)
                query_results.update(queries)
                
        # Calculate overall statistics
        validation_time = asyncio.get_event_loop().time() - start_time
        
        # Track metrics
        metrics.validation_duration.labels(
            stage='post_load',
            data_type='market_data'
        ).observe(validation_time)
        
        validation_result = PostValidationResult(
            is_valid=checks_failed == 0,
            checks_performed=len(validation_tasks),
            checks_passed=checks_passed,
            checks_failed=checks_failed,
            validation_errors=all_errors,
            warnings=all_warnings,
            statistics=all_statistics,
            query_results=query_results
        )
        
        self.logger.info(
            f"Post-load validation completed: {checks_passed}/{len(validation_tasks)} "
            f"checks passed ({validation_result.success_rate:.2%}), "
            f"duration: {validation_time:.2f}s"
        )
        
        return validation_result
        
    async def _check_record_count(
        self,
        symbol: str,
        start_date: datetime,
        end_date: datetime,
        expected_records: Optional[int]
    ) -> Tuple[bool, List[Dict], List[str], Dict, Dict]:
        """Verify record count matches expectations."""
        errors = []
        warnings = []
        stats = {}
        queries = {}
        
        async with self._conn_pool.acquire() as conn:
            # Count total records
            count_query = """
                SELECT COUNT(*) as total_count
                FROM market_data
                WHERE symbol = $1
                AND time >= $2
                AND time <= $3
            """
            
            result = await conn.fetchrow(count_query, symbol, start_date, end_date)
            actual_count = result['total_count']
            
            stats['actual_record_count'] = actual_count
            queries['record_count_query'] = count_query
            
            # Check against expected count if provided
            if expected_records:
                stats['expected_record_count'] = expected_records
                diff_pct = abs(actual_count - expected_records) / expected_records * 100
                
                if diff_pct > 5:  # More than 5% difference
                    errors.append({
                        'check': 'record_count',
                        'error': f'Record count mismatch: expected {expected_records}, got {actual_count} ({diff_pct:.1f}% diff)',
                        'symbol': symbol
                    })
                    return False, errors, warnings, stats, queries
                elif diff_pct > 1:  # Between 1-5% difference
                    warnings.append(
                        f"Minor record count difference: {diff_pct:.1f}% "
                        f"(expected: {expected_records}, actual: {actual_count})"
                    )
                    
            # Check daily distribution
            daily_query = """
                SELECT 
                    DATE(time) as date,
                    COUNT(*) as count,
                    MIN(time) as first_time,
                    MAX(time) as last_time
                FROM market_data
                WHERE symbol = $1
                AND time >= $2
                AND time <= $3
                GROUP BY DATE(time)
                ORDER BY date
            """
            
            daily_results = await conn.fetch(daily_query, symbol, start_date, end_date)
            
            stats['days_with_data'] = len(daily_results)
            stats['avg_records_per_day'] = actual_count / len(daily_results) if daily_results else 0
            
            # Check for days with unusually low data
            if daily_results:
                counts = [row['count'] for row in daily_results]
                avg_count = sum(counts) / len(counts)
                
                for row in daily_results:
                    if row['count'] < avg_count * 0.5:  # Less than 50% of average
                        warnings.append(
                            f"Low data count on {row['date']}: {row['count']} records "
                            f"(avg: {avg_count:.0f})"
                        )
                        
        return True, errors, warnings, stats, queries
        
    async def _check_data_completeness(
        self,
        symbol: str,
        start_date: datetime,
        end_date: datetime
    ) -> Tuple[bool, List[Dict], List[str], Dict, Dict]:
        """Check for NULL values and data completeness."""
        errors = []
        warnings = []
        stats = {}
        queries = {}
        
        async with self._conn_pool.acquire() as conn:
            # Check for NULL values in critical fields
            null_check_query = """
                SELECT 
                    COUNT(*) FILTER (WHERE open IS NULL) as null_open,
                    COUNT(*) FILTER (WHERE high IS NULL) as null_high,
                    COUNT(*) FILTER (WHERE low IS NULL) as null_low,
                    COUNT(*) FILTER (WHERE close IS NULL) as null_close,
                    COUNT(*) FILTER (WHERE volume IS NULL) as null_volume,
                    COUNT(*) as total_count
                FROM market_data
                WHERE symbol = $1
                AND time >= $2
                AND time <= $3
            """
            
            result = await conn.fetchrow(null_check_query, symbol, start_date, end_date)
            
            # Check for any NULL values
            null_fields = []
            for field in ['open', 'high', 'low', 'close', 'volume']:
                null_count = result[f'null_{field}']
                if null_count > 0:
                    null_fields.append(f"{field}: {null_count}")
                    
            if null_fields:
                errors.append({
                    'check': 'data_completeness',
                    'error': f'NULL values found: {", ".join(null_fields)}',
                    'symbol': symbol
                })
                return False, errors, warnings, stats, queries
                
            # Check for zero prices (which shouldn't happen)
            zero_check_query = """
                SELECT 
                    COUNT(*) FILTER (WHERE open = 0) as zero_open,
                    COUNT(*) FILTER (WHERE high = 0) as zero_high,
                    COUNT(*) FILTER (WHERE low = 0) as zero_low,
                    COUNT(*) FILTER (WHERE close = 0) as zero_close
                FROM market_data
                WHERE symbol = $1
                AND time >= $2
                AND time <= $3
            """
            
            zero_result = await conn.fetchrow(zero_check_query, symbol, start_date, end_date)
            
            zero_fields = []
            for field in ['open', 'high', 'low', 'close']:
                zero_count = zero_result[f'zero_{field}']
                if zero_count > 0:
                    zero_fields.append(f"{field}: {zero_count}")
                    
            if zero_fields:
                errors.append({
                    'check': 'data_completeness',
                    'error': f'Zero prices found: {", ".join(zero_fields)}',
                    'symbol': symbol
                })
                
            stats['completeness'] = {
                'total_records': result['total_count'],
                'null_counts': {field: result[f'null_{field}'] for field in ['open', 'high', 'low', 'close', 'volume']},
                'zero_counts': {field: zero_result[f'zero_{field}'] for field in ['open', 'high', 'low', 'close']}
            }
            
        return len(errors) == 0, errors, warnings, stats, queries
        
    async def _check_ohlc_consistency(
        self,
        symbol: str,
        start_date: datetime,
        end_date: datetime
    ) -> Tuple[bool, List[Dict], List[str], Dict, Dict]:
        """Verify OHLC data consistency in database."""
        errors = []
        warnings = []
        stats = {}
        queries = {}
        
        async with self._conn_pool.acquire() as conn:
            # Check OHLC consistency rules
            consistency_query = """
                SELECT 
                    COUNT(*) FILTER (WHERE high < low) as high_less_than_low,
                    COUNT(*) FILTER (WHERE high < open) as high_less_than_open,
                    COUNT(*) FILTER (WHERE high < close) as high_less_than_close,
                    COUNT(*) FILTER (WHERE low > open) as low_greater_than_open,
                    COUNT(*) FILTER (WHERE low > close) as low_greater_than_close,
                    COUNT(*) as total_count
                FROM market_data
                WHERE symbol = $1
                AND time >= $2
                AND time <= $3
            """
            
            result = await conn.fetchrow(consistency_query, symbol, start_date, end_date)
            
            inconsistencies = []
            for check, count in [
                ('high < low', result['high_less_than_low']),
                ('high < open', result['high_less_than_open']),
                ('high < close', result['high_less_than_close']),
                ('low > open', result['low_greater_than_open']),
                ('low > close', result['low_greater_than_close'])
            ]:
                if count > 0:
                    inconsistencies.append(f"{check}: {count}")
                    
            if inconsistencies:
                errors.append({
                    'check': 'ohlc_consistency',
                    'error': f'OHLC inconsistencies found: {", ".join(inconsistencies)}',
                    'symbol': symbol
                })
                
            # Get samples of inconsistent records for debugging
            if inconsistencies:
                sample_query = """
                    SELECT time, open, high, low, close
                    FROM market_data
                    WHERE symbol = $1
                    AND time >= $2
                    AND time <= $3
                    AND (high < low OR high < open OR high < close OR low > open OR low > close)
                    LIMIT 10
                """
                
                samples = await conn.fetch(sample_query, symbol, start_date, end_date)
                stats['inconsistent_samples'] = [dict(row) for row in samples]
                
            stats['ohlc_consistency'] = {
                'total_records': result['total_count'],
                'inconsistencies': {k: v for k, v in result.items() if k != 'total_count'}
            }
            
        return len(errors) == 0, errors, warnings, stats, queries
        
    async def _check_duplicate_records(
        self,
        symbol: str,
        start_date: datetime,
        end_date: datetime
    ) -> Tuple[bool, List[Dict], List[str], Dict, Dict]:
        """Check for duplicate records."""
        errors = []
        warnings = []
        stats = {}
        queries = {}
        
        async with self._conn_pool.acquire() as conn:
            # Check for exact duplicates (same symbol, time, provider)
            duplicate_query = """
                SELECT 
                    time,
                    provider,
                    COUNT(*) as count
                FROM market_data
                WHERE symbol = $1
                AND time >= $2
                AND time <= $3
                GROUP BY time, provider
                HAVING COUNT(*) > 1
                ORDER BY count DESC
                LIMIT 100
            """
            
            duplicates = await conn.fetch(duplicate_query, symbol, start_date, end_date)
            
            if duplicates:
                errors.append({
                    'check': 'duplicate_records',
                    'error': f'Found {len(duplicates)} duplicate timestamp entries',
                    'symbol': symbol,
                    'samples': [dict(row) for row in duplicates[:10]]
                })
                
                stats['duplicate_count'] = len(duplicates)
                stats['max_duplicates_per_timestamp'] = duplicates[0]['count'] if duplicates else 0
                
            # Check for near-duplicates (same timestamp, different providers)
            provider_overlap_query = """
                SELECT 
                    time,
                    COUNT(DISTINCT provider) as provider_count,
                    ARRAY_AGG(DISTINCT provider) as providers
                FROM market_data
                WHERE symbol = $1
                AND time >= $2
                AND time <= $3
                GROUP BY time
                HAVING COUNT(DISTINCT provider) > 1
                LIMIT 100
            """
            
            overlaps = await conn.fetch(provider_overlap_query, symbol, start_date, end_date)
            
            if overlaps:
                warnings.append(
                    f"Found {len(overlaps)} timestamps with data from multiple providers"
                )
                stats['multi_provider_timestamps'] = len(overlaps)
                
        return len(errors) == 0, errors, warnings, stats, queries
        
    async def _check_time_gaps(
        self,
        symbol: str,
        start_date: datetime,
        end_date: datetime,
        granularity: Optional[str]
    ) -> Tuple[bool, List[Dict], List[str], Dict, Dict]:
        """Check for time gaps in the data."""
        errors = []
        warnings = []
        stats = {}
        queries = {}
        
        async with self._conn_pool.acquire() as conn:
            # Find gaps using window functions
            gap_query = """
                WITH time_diffs AS (
                    SELECT 
                        time,
                        LAG(time) OVER (ORDER BY time) as prev_time,
                        time - LAG(time) OVER (ORDER BY time) as time_diff
                    FROM market_data
                    WHERE symbol = $1
                    AND time >= $2
                    AND time <= $3
                    ORDER BY time
                )
                SELECT 
                    prev_time as gap_start,
                    time as gap_end,
                    time_diff,
                    EXTRACT(EPOCH FROM time_diff) as gap_seconds
                FROM time_diffs
                WHERE time_diff > INTERVAL '1 hour'  -- Adjust based on granularity
                ORDER BY time_diff DESC
                LIMIT 100
            """
            
            gaps = await conn.fetch(gap_query, symbol, start_date, end_date)
            
            if gaps:
                # Analyze gap patterns
                gap_sizes = [row['gap_seconds'] for row in gaps]
                
                stats['gap_analysis'] = {
                    'total_gaps': len(gaps),
                    'largest_gap_hours': max(gap_sizes) / 3600 if gap_sizes else 0,
                    'avg_gap_hours': sum(gap_sizes) / len(gap_sizes) / 3600 if gap_sizes else 0
                }
                
                # Check if gaps are during market hours
                market_hour_gaps = []
                for gap in gaps:
                    gap_start = gap['gap_start']
                    gap_end = gap['gap_end']
                    
                    # Simple market hours check (9:30 AM - 4:00 PM ET, weekdays)
                    if (gap_start.weekday() < 5 and 
                        gap_start.hour >= 9 and gap_start.hour < 16 and
                        gap_end.weekday() < 5 and 
                        gap_end.hour >= 9 and gap_end.hour < 16):
                        market_hour_gaps.append(gap)
                        
                if market_hour_gaps:
                    warnings.append(
                        f"Found {len(market_hour_gaps)} gaps during market hours, "
                        f"largest: {max(g['gap_seconds'] for g in market_hour_gaps) / 3600:.1f} hours"
                    )
                    
            # Check coverage percentage
            total_seconds = (end_date - start_date).total_seconds()
            covered_seconds = await conn.fetchval("""
                SELECT 
                    EXTRACT(EPOCH FROM (MAX(time) - MIN(time))) 
                FROM market_data
                WHERE symbol = $1
                AND time >= $2
                AND time <= $3
            """, symbol, start_date, end_date)
            
            coverage_pct = (covered_seconds / total_seconds * 100) if total_seconds > 0 else 0
            stats['time_coverage_pct'] = coverage_pct
            
            if coverage_pct < 80:  # Less than 80% coverage
                warnings.append(
                    f"Low time coverage: {coverage_pct:.1f}% of expected time range"
                )
                
        return True, errors, warnings, stats, queries
        
    async def _check_aggregation_consistency(
        self,
        symbol: str,
        start_date: datetime,
        end_date: datetime
    ) -> Tuple[bool, List[Dict], List[str], Dict, Dict]:
        """Verify consistency between raw data and aggregated views."""
        errors = []
        warnings = []
        stats = {}
        queries = {}
        
        async with self._conn_pool.acquire() as conn:
            # Check if continuous aggregates exist
            check_aggregates_query = """
                SELECT 
                    view_name,
                    materialization_hypertable_name
                FROM timescaledb_information.continuous_aggregates
                WHERE view_name IN ('market_data_1h', 'market_data_1d')
            """
            
            aggregates = await conn.fetch(check_aggregates_query)
            
            if not aggregates:
                warnings.append("No continuous aggregates found for validation")
                return True, errors, warnings, stats, queries
                
            # Compare raw data with 1-hour aggregate
            if any(agg['view_name'] == 'market_data_1h' for agg in aggregates):
                comparison_query = """
                    WITH raw_hourly AS (
                        SELECT 
                            time_bucket('1 hour', time) as hour,
                            FIRST(open, time) as open,
                            MAX(high) as high,
                            MIN(low) as low,
                            LAST(close, time) as close,
                            SUM(volume) as volume
                        FROM market_data
                        WHERE symbol = $1
                        AND time >= $2
                        AND time <= $3
                        GROUP BY hour
                    ),
                    aggregate_hourly AS (
                        SELECT 
                            bucket as hour,
                            open, high, low, close, volume
                        FROM market_data_1h
                        WHERE symbol = $1
                        AND bucket >= $2
                        AND bucket <= $3
                    )
                    SELECT 
                        r.hour,
                        ABS(r.open - a.open) as open_diff,
                        ABS(r.high - a.high) as high_diff,
                        ABS(r.low - a.low) as low_diff,
                        ABS(r.close - a.close) as close_diff,
                        ABS(r.volume - a.volume) as volume_diff
                    FROM raw_hourly r
                    JOIN aggregate_hourly a ON r.hour = a.hour
                    WHERE 
                        ABS(r.open - a.open) > 0.01 OR
                        ABS(r.high - a.high) > 0.01 OR
                        ABS(r.low - a.low) > 0.01 OR
                        ABS(r.close - a.close) > 0.01 OR
                        ABS(r.volume - a.volume) > 1
                    LIMIT 10
                """
                
                discrepancies = await conn.fetch(comparison_query, symbol, start_date, end_date)
                
                if discrepancies:
                    errors.append({
                        'check': 'aggregation_consistency',
                        'error': f'Found {len(discrepancies)} hourly aggregation discrepancies',
                        'symbol': symbol,
                        'samples': [dict(row) for row in discrepancies]
                    })
                    
            stats['aggregation_check'] = {
                'aggregates_found': [agg['view_name'] for agg in aggregates],
                'discrepancy_count': len(discrepancies) if 'discrepancies' in locals() else 0
            }
            
        return len(errors) == 0, errors, warnings, stats, queries
        
    async def _check_index_integrity(
        self,
        symbol: str
    ) -> Tuple[bool, List[Dict], List[str], Dict, Dict]:
        """Check database index health and usage."""
        errors = []
        warnings = []
        stats = {}
        queries = {}
        
        async with self._conn_pool.acquire() as conn:
            # Check index health
            index_health_query = """
                SELECT 
                    schemaname,
                    tablename,
                    indexname,
                    idx_scan,
                    idx_tup_read,
                    idx_tup_fetch,
                    pg_size_pretty(pg_relation_size(indexrelid)) as index_size
                FROM pg_stat_user_indexes
                WHERE schemaname = 'public'
                AND tablename = 'market_data'
                ORDER BY idx_scan DESC
            """
            
            index_stats = await conn.fetch(index_health_query)
            
            # Check for unused indexes
            unused_indexes = [
                idx for idx in index_stats 
                if idx['idx_scan'] == 0 and 'primary' not in idx['indexname'].lower()
            ]
            
            if unused_indexes:
                warnings.append(
                    f"Found {len(unused_indexes)} unused indexes: "
                    f"{', '.join(idx['indexname'] for idx in unused_indexes)}"
                )
                
            # Check for missing indexes on symbol column
            symbol_index_exists = any(
                'symbol' in idx['indexname'].lower() 
                for idx in index_stats
            )
            
            if not symbol_index_exists:
                warnings.append("No index found on symbol column")
                
            stats['index_health'] = {
                'total_indexes': len(index_stats),
                'unused_indexes': len(unused_indexes),
                'most_used_index': index_stats[0]['indexname'] if index_stats else None,
                'total_index_scans': sum(idx['idx_scan'] for idx in index_stats)
            }
            
        return True, errors, warnings, stats, queries
        
    async def _check_data_distribution(
        self,
        symbol: str,
        start_date: datetime,
        end_date: datetime
    ) -> Tuple[bool, List[Dict], List[str], Dict, Dict]:
        """Analyze data distribution and detect anomalies."""
        errors = []
        warnings = []
        stats = {}
        queries = {}
        
        async with self._conn_pool.acquire() as conn:
            # Get price distribution statistics
            distribution_query = """
                SELECT 
                    MIN(close) as min_price,
                    MAX(close) as max_price,
                    AVG(close) as avg_price,
                    STDDEV(close) as stddev_price,
                    PERCENTILE_CONT(0.25) WITHIN GROUP (ORDER BY close) as q1_price,
                    PERCENTILE_CONT(0.50) WITHIN GROUP (ORDER BY close) as median_price,
                    PERCENTILE_CONT(0.75) WITHIN GROUP (ORDER BY close) as q3_price,
                    MIN(volume) as min_volume,
                    MAX(volume) as max_volume,
                    AVG(volume) as avg_volume
                FROM market_data
                WHERE symbol = $1
                AND time >= $2
                AND time <= $3
            """
            
            dist_stats = await conn.fetchrow(distribution_query, symbol, start_date, end_date)
            
            # Check for outliers using IQR method
            if dist_stats['q3_price'] and dist_stats['q1_price']:
                iqr = float(dist_stats['q3_price'] - dist_stats['q1_price'])
                lower_bound = float(dist_stats['q1_price']) - 1.5 * iqr
                upper_bound = float(dist_stats['q3_price']) + 1.5 * iqr
                
                outlier_query = """
                    SELECT COUNT(*) as outlier_count
                    FROM market_data
                    WHERE symbol = $1
                    AND time >= $2
                    AND time <= $3
                    AND (close < $4 OR close > $5)
                """
                
                outlier_result = await conn.fetchrow(
                    outlier_query, 
                    symbol, start_date, end_date, 
                    lower_bound, upper_bound
                )
                
                outlier_count = outlier_result['outlier_count']
                if outlier_count > 0:
                    warnings.append(
                        f"Found {outlier_count} price outliers outside IQR bounds "
                        f"[{lower_bound:.2f}, {upper_bound:.2f}]"
                    )
                    
            stats['distribution'] = {
                'price': {
                    'min': float(dist_stats['min_price']) if dist_stats['min_price'] else None,
                    'max': float(dist_stats['max_price']) if dist_stats['max_price'] else None,
                    'avg': float(dist_stats['avg_price']) if dist_stats['avg_price'] else None,
                    'stddev': float(dist_stats['stddev_price']) if dist_stats['stddev_price'] else None,
                    'q1': float(dist_stats['q1_price']) if dist_stats['q1_price'] else None,
                    'median': float(dist_stats['median_price']) if dist_stats['median_price'] else None,
                    'q3': float(dist_stats['q3_price']) if dist_stats['q3_price'] else None
                },
                'volume': {
                    'min': int(dist_stats['min_volume']) if dist_stats['min_volume'] else None,
                    'max': int(dist_stats['max_volume']) if dist_stats['max_volume'] else None,
                    'avg': float(dist_stats['avg_volume']) if dist_stats['avg_volume'] else None
                },
                'outlier_count': outlier_count if 'outlier_count' in locals() else 0
            }
            
        return True, errors, warnings, stats, queries