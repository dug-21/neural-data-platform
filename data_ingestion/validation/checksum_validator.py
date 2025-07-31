"""
Checksum validation for ensuring data integrity during backfill operations.

Provides multiple validation methods:
- Row count verification
- Hash-based checksums for data blocks
- Statistical fingerprinting
- Cross-provider validation
"""

import asyncio
import hashlib
from typing import List, Dict, Any, Optional, Tuple, Set
from datetime import datetime, timedelta
from dataclasses import dataclass, field
import asyncpg
from decimal import Decimal
import json

from ..utils.logging import get_logger
from ..utils.metrics import metrics


@dataclass
class ChecksumResult:
    """Result of checksum validation."""
    is_valid: bool
    checksum_type: str
    expected_value: Optional[str]
    actual_value: Optional[str]
    match_percentage: float
    details: Dict[str, Any] = field(default_factory=dict)
    errors: List[str] = field(default_factory=list)
    
    @property
    def matches(self) -> bool:
        """Check if checksums match."""
        return self.expected_value == self.actual_value


@dataclass
class DataIntegrityReport:
    """Comprehensive data integrity validation report."""
    symbol: str
    start_date: datetime
    end_date: datetime
    validations_performed: List[str]
    overall_integrity_score: float
    checksum_results: Dict[str, ChecksumResult]
    row_count_validation: Dict[str, Any]
    statistical_validation: Dict[str, Any]
    cross_provider_validation: Optional[Dict[str, Any]]
    recommendations: List[str]
    timestamp: datetime = field(default_factory=datetime.now)


class ChecksumValidator:
    """Validates data integrity using various checksum methods."""
    
    # Checksum block size for efficient processing
    BLOCK_SIZE = 10000
    
    # Statistical tolerance for floating point comparisons
    STATISTICAL_TOLERANCE = 0.0001
    
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
            command_timeout=120
        )
        return self
        
    async def __aexit__(self, exc_type, exc_val, exc_tb):
        """Close database connection pool."""
        if self._conn_pool:
            await self._conn_pool.close()
            
    async def validate_integrity(
        self,
        symbol: str,
        start_date: datetime,
        end_date: datetime,
        expected_checksums: Optional[Dict[str, str]] = None,
        expected_row_count: Optional[int] = None,
        validate_cross_provider: bool = False
    ) -> DataIntegrityReport:
        """Perform comprehensive data integrity validation."""
        self.logger.info(
            f"Starting integrity validation for {symbol} "
            f"from {start_date} to {end_date}"
        )
        
        start_time = asyncio.get_event_loop().time()
        validations_performed = []
        checksum_results = {}
        
        # Row count validation
        row_count_result = await self._validate_row_count(
            symbol, start_date, end_date, expected_row_count
        )
        validations_performed.append('row_count')
        
        # Data checksums
        if expected_checksums:
            for checksum_type, expected_value in expected_checksums.items():
                if checksum_type == 'md5':
                    result = await self._validate_md5_checksum(
                        symbol, start_date, end_date, expected_value
                    )
                elif checksum_type == 'statistical':
                    result = await self._validate_statistical_checksum(
                        symbol, start_date, end_date, expected_value
                    )
                else:
                    continue
                    
                checksum_results[checksum_type] = result
                validations_performed.append(f'checksum_{checksum_type}')
        else:
            # Generate checksums for future validation
            md5_result = await self._calculate_md5_checksum(symbol, start_date, end_date)
            stat_result = await self._calculate_statistical_checksum(symbol, start_date, end_date)
            
            checksum_results['md5'] = ChecksumResult(
                is_valid=True,
                checksum_type='md5',
                expected_value=None,
                actual_value=md5_result,
                match_percentage=100.0,
                details={'generated': True}
            )
            
            checksum_results['statistical'] = ChecksumResult(
                is_valid=True,
                checksum_type='statistical',
                expected_value=None,
                actual_value=json.dumps(stat_result),
                match_percentage=100.0,
                details=stat_result
            )
            
            validations_performed.extend(['checksum_md5_generated', 'checksum_statistical_generated'])
            
        # Statistical validation
        statistical_result = await self._validate_statistical_properties(
            symbol, start_date, end_date
        )
        validations_performed.append('statistical_properties')
        
        # Cross-provider validation if requested
        cross_provider_result = None
        if validate_cross_provider:
            cross_provider_result = await self._validate_cross_provider(
                symbol, start_date, end_date
            )
            validations_performed.append('cross_provider')
            
        # Calculate overall integrity score
        integrity_score = self._calculate_integrity_score(
            row_count_result,
            checksum_results,
            statistical_result,
            cross_provider_result
        )
        
        # Generate recommendations
        recommendations = self._generate_recommendations(
            integrity_score,
            row_count_result,
            checksum_results,
            statistical_result
        )
        
        # Track metrics
        validation_time = asyncio.get_event_loop().time() - start_time
        metrics.validation_duration.labels(
            stage='checksum',
            data_type='market_data'
        ).observe(validation_time)
        
        self.logger.info(
            f"Integrity validation completed with score: {integrity_score:.2f}, "
            f"duration: {validation_time:.2f}s"
        )
        
        return DataIntegrityReport(
            symbol=symbol,
            start_date=start_date,
            end_date=end_date,
            validations_performed=validations_performed,
            overall_integrity_score=integrity_score,
            checksum_results=checksum_results,
            row_count_validation=row_count_result,
            statistical_validation=statistical_result,
            cross_provider_validation=cross_provider_result,
            recommendations=recommendations
        )
        
    async def _validate_row_count(
        self,
        symbol: str,
        start_date: datetime,
        end_date: datetime,
        expected_count: Optional[int]
    ) -> Dict[str, Any]:
        """Validate row count against expectations."""
        async with self._conn_pool.acquire() as conn:
            # Get actual count
            count_query = """
                SELECT COUNT(*) as count
                FROM market_data
                WHERE symbol = $1
                AND time >= $2
                AND time <= $3
            """
            
            result = await conn.fetchrow(count_query, symbol, start_date, end_date)
            actual_count = result['count']
            
            # Get count by provider
            provider_query = """
                SELECT provider, COUNT(*) as count
                FROM market_data
                WHERE symbol = $1
                AND time >= $2
                AND time <= $3
                GROUP BY provider
                ORDER BY count DESC
            """
            
            provider_counts = await conn.fetch(provider_query, symbol, start_date, end_date)
            
            validation_result = {
                'actual_count': actual_count,
                'expected_count': expected_count,
                'is_valid': True,
                'match_percentage': 100.0,
                'provider_breakdown': {row['provider']: row['count'] for row in provider_counts}
            }
            
            if expected_count:
                diff = abs(actual_count - expected_count)
                match_pct = (1 - diff / expected_count) * 100 if expected_count > 0 else 0
                validation_result['match_percentage'] = match_pct
                validation_result['is_valid'] = match_pct >= 95.0  # 95% threshold
                validation_result['difference'] = actual_count - expected_count
                
            return validation_result
            
    async def _calculate_md5_checksum(
        self,
        symbol: str,
        start_date: datetime,
        end_date: datetime
    ) -> str:
        """Calculate MD5 checksum for data block."""
        async with self._conn_pool.acquire() as conn:
            # Create deterministic string representation of data
            checksum_query = """
                SELECT 
                    time::text || '|' ||
                    open::text || '|' ||
                    high::text || '|' ||
                    low::text || '|' ||
                    close::text || '|' ||
                    volume::text as data_string
                FROM market_data
                WHERE symbol = $1
                AND time >= $2
                AND time <= $3
                ORDER BY time, provider
            """
            
            md5_hash = hashlib.md5()
            
            # Process in blocks to handle large datasets
            async for row in conn.cursor(checksum_query, symbol, start_date, end_date):
                md5_hash.update(row['data_string'].encode('utf-8'))
                
            return md5_hash.hexdigest()
            
    async def _validate_md5_checksum(
        self,
        symbol: str,
        start_date: datetime,
        end_date: datetime,
        expected_checksum: str
    ) -> ChecksumResult:
        """Validate MD5 checksum against expected value."""
        actual_checksum = await self._calculate_md5_checksum(symbol, start_date, end_date)
        
        is_valid = actual_checksum == expected_checksum
        match_percentage = 100.0 if is_valid else 0.0
        
        return ChecksumResult(
            is_valid=is_valid,
            checksum_type='md5',
            expected_value=expected_checksum,
            actual_value=actual_checksum,
            match_percentage=match_percentage,
            details={
                'algorithm': 'MD5',
                'block_size': self.BLOCK_SIZE
            },
            errors=[] if is_valid else ['MD5 checksum mismatch']
        )
        
    async def _calculate_statistical_checksum(
        self,
        symbol: str,
        start_date: datetime,
        end_date: datetime
    ) -> Dict[str, Any]:
        """Calculate statistical fingerprint of the data."""
        async with self._conn_pool.acquire() as conn:
            stats_query = """
                SELECT 
                    COUNT(*) as count,
                    AVG(close) as mean_close,
                    STDDEV(close) as stddev_close,
                    MIN(close) as min_close,
                    MAX(close) as max_close,
                    SUM(volume) as total_volume,
                    AVG(high - low) as avg_range,
                    AVG((close - open) / open * 100) as avg_return_pct
                FROM market_data
                WHERE symbol = $1
                AND time >= $2
                AND time <= $3
            """
            
            result = await conn.fetchrow(stats_query, symbol, start_date, end_date)
            
            return {
                'count': result['count'],
                'mean_close': float(result['mean_close']) if result['mean_close'] else 0,
                'stddev_close': float(result['stddev_close']) if result['stddev_close'] else 0,
                'min_close': float(result['min_close']) if result['min_close'] else 0,
                'max_close': float(result['max_close']) if result['max_close'] else 0,
                'total_volume': result['total_volume'],
                'avg_range': float(result['avg_range']) if result['avg_range'] else 0,
                'avg_return_pct': float(result['avg_return_pct']) if result['avg_return_pct'] else 0
            }
            
    async def _validate_statistical_checksum(
        self,
        symbol: str,
        start_date: datetime,
        end_date: datetime,
        expected_stats_json: str
    ) -> ChecksumResult:
        """Validate statistical properties against expected values."""
        expected_stats = json.loads(expected_stats_json)
        actual_stats = await self._calculate_statistical_checksum(symbol, start_date, end_date)
        
        # Compare each statistical measure
        mismatches = []
        total_checks = 0
        passed_checks = 0
        
        for key in expected_stats:
            if key not in actual_stats:
                continue
                
            total_checks += 1
            expected = expected_stats[key]
            actual = actual_stats[key]
            
            # Use tolerance for floating point comparisons
            if isinstance(expected, float):
                if abs(expected - actual) <= self.STATISTICAL_TOLERANCE * abs(expected):
                    passed_checks += 1
                else:
                    mismatches.append(f"{key}: expected {expected}, got {actual}")
            else:
                if expected == actual:
                    passed_checks += 1
                else:
                    mismatches.append(f"{key}: expected {expected}, got {actual}")
                    
        match_percentage = (passed_checks / total_checks * 100) if total_checks > 0 else 0
        is_valid = match_percentage >= 95.0  # 95% threshold
        
        return ChecksumResult(
            is_valid=is_valid,
            checksum_type='statistical',
            expected_value=expected_stats_json,
            actual_value=json.dumps(actual_stats),
            match_percentage=match_percentage,
            details={
                'expected_stats': expected_stats,
                'actual_stats': actual_stats,
                'mismatches': mismatches
            },
            errors=mismatches
        )
        
    async def _validate_statistical_properties(
        self,
        symbol: str,
        start_date: datetime,
        end_date: datetime
    ) -> Dict[str, Any]:
        """Validate statistical properties for anomalies."""
        async with self._conn_pool.acquire() as conn:
            # Check for statistical anomalies
            anomaly_query = """
                WITH stats AS (
                    SELECT 
                        AVG(close) as mean_close,
                        STDDEV(close) as stddev_close,
                        AVG(volume) as mean_volume,
                        STDDEV(volume) as stddev_volume
                    FROM market_data
                    WHERE symbol = $1
                    AND time >= $2
                    AND time <= $3
                )
                SELECT 
                    COUNT(*) FILTER (WHERE ABS(close - stats.mean_close) > 3 * stats.stddev_close) as price_outliers,
                    COUNT(*) FILTER (WHERE volume > stats.mean_volume + 3 * stats.stddev_volume) as volume_outliers,
                    COUNT(*) FILTER (WHERE close <= 0) as zero_prices,
                    COUNT(*) FILTER (WHERE volume < 0) as negative_volumes,
                    COUNT(*) as total_count
                FROM market_data, stats
                WHERE symbol = $1
                AND time >= $2
                AND time <= $3
            """
            
            result = await conn.fetchrow(anomaly_query, symbol, start_date, end_date)
            
            # Distribution analysis
            distribution_query = """
                SELECT 
                    PERCENTILE_CONT(0.01) WITHIN GROUP (ORDER BY close) as p1,
                    PERCENTILE_CONT(0.25) WITHIN GROUP (ORDER BY close) as q1,
                    PERCENTILE_CONT(0.50) WITHIN GROUP (ORDER BY close) as median,
                    PERCENTILE_CONT(0.75) WITHIN GROUP (ORDER BY close) as q3,
                    PERCENTILE_CONT(0.99) WITHIN GROUP (ORDER BY close) as p99,
                    SKEW(close) as skewness,
                    KURTOSIS(close) as kurtosis
                FROM market_data
                WHERE symbol = $1
                AND time >= $2
                AND time <= $3
            """
            
            dist_result = await conn.fetchrow(distribution_query, symbol, start_date, end_date)
            
            return {
                'anomalies': {
                    'price_outliers': result['price_outliers'],
                    'volume_outliers': result['volume_outliers'],
                    'zero_prices': result['zero_prices'],
                    'negative_volumes': result['negative_volumes'],
                    'total_records': result['total_count']
                },
                'distribution': {
                    'p1': float(dist_result['p1']) if dist_result['p1'] else None,
                    'q1': float(dist_result['q1']) if dist_result['q1'] else None,
                    'median': float(dist_result['median']) if dist_result['median'] else None,
                    'q3': float(dist_result['q3']) if dist_result['q3'] else None,
                    'p99': float(dist_result['p99']) if dist_result['p99'] else None,
                    'skewness': float(dist_result['skewness']) if dist_result['skewness'] else None,
                    'kurtosis': float(dist_result['kurtosis']) if dist_result['kurtosis'] else None
                },
                'is_valid': result['zero_prices'] == 0 and result['negative_volumes'] == 0
            }
            
    async def _validate_cross_provider(
        self,
        symbol: str,
        start_date: datetime,
        end_date: datetime
    ) -> Dict[str, Any]:
        """Validate data consistency across multiple providers."""
        async with self._conn_pool.acquire() as conn:
            # Compare data from different providers
            comparison_query = """
                WITH provider_stats AS (
                    SELECT 
                        provider,
                        COUNT(*) as count,
                        AVG(close) as avg_close,
                        STDDEV(close) as stddev_close,
                        MIN(close) as min_close,
                        MAX(close) as max_close
                    FROM market_data
                    WHERE symbol = $1
                    AND time >= $2
                    AND time <= $3
                    GROUP BY provider
                )
                SELECT * FROM provider_stats
                ORDER BY count DESC
            """
            
            provider_stats = await conn.fetch(comparison_query, symbol, start_date, end_date)
            
            if len(provider_stats) < 2:
                return {
                    'is_valid': True,
                    'providers_compared': len(provider_stats),
                    'message': 'Insufficient providers for cross-validation'
                }
                
            # Compare overlapping data points
            overlap_query = """
                WITH overlapping_times AS (
                    SELECT time
                    FROM market_data
                    WHERE symbol = $1
                    AND time >= $2
                    AND time <= $3
                    GROUP BY time
                    HAVING COUNT(DISTINCT provider) > 1
                )
                SELECT 
                    t.time,
                    ARRAY_AGG(DISTINCT m.provider) as providers,
                    MAX(m.close) - MIN(m.close) as price_diff,
                    (MAX(m.close) - MIN(m.close)) / AVG(m.close) * 100 as price_diff_pct
                FROM overlapping_times t
                JOIN market_data m ON m.time = t.time AND m.symbol = $1
                GROUP BY t.time
                HAVING MAX(m.close) - MIN(m.close) > AVG(m.close) * 0.01  -- > 1% difference
                ORDER BY price_diff_pct DESC
                LIMIT 100
            """
            
            discrepancies = await conn.fetch(overlap_query, symbol, start_date, end_date)
            
            return {
                'is_valid': len(discrepancies) == 0,
                'providers_compared': len(provider_stats),
                'provider_statistics': [dict(row) for row in provider_stats],
                'discrepancy_count': len(discrepancies),
                'max_price_diff_pct': float(discrepancies[0]['price_diff_pct']) if discrepancies else 0,
                'sample_discrepancies': [dict(row) for row in discrepancies[:10]]
            }
            
    def _calculate_integrity_score(
        self,
        row_count_result: Dict[str, Any],
        checksum_results: Dict[str, ChecksumResult],
        statistical_result: Dict[str, Any],
        cross_provider_result: Optional[Dict[str, Any]]
    ) -> float:
        """Calculate overall data integrity score (0-100)."""
        scores = []
        weights = []
        
        # Row count score (weight: 20%)
        if 'match_percentage' in row_count_result:
            scores.append(row_count_result['match_percentage'])
            weights.append(0.2)
        elif row_count_result.get('is_valid'):
            scores.append(100.0)
            weights.append(0.2)
            
        # Checksum scores (weight: 30%)
        for checksum_type, result in checksum_results.items():
            scores.append(result.match_percentage)
            weights.append(0.15)  # Split 30% among checksums
            
        # Statistical validation score (weight: 30%)
        if statistical_result.get('is_valid'):
            anomaly_score = 100.0
            if 'anomalies' in statistical_result:
                total = statistical_result['anomalies']['total_records']
                anomalies = sum([
                    statistical_result['anomalies']['price_outliers'],
                    statistical_result['anomalies']['volume_outliers'],
                    statistical_result['anomalies']['zero_prices'],
                    statistical_result['anomalies']['negative_volumes']
                ])
                if total > 0:
                    anomaly_score = (1 - anomalies / total) * 100
            scores.append(anomaly_score)
            weights.append(0.3)
        else:
            scores.append(0.0)
            weights.append(0.3)
            
        # Cross-provider score (weight: 20%)
        if cross_provider_result:
            if cross_provider_result.get('is_valid'):
                scores.append(100.0)
            else:
                # Score based on discrepancy count
                discrepancy_count = cross_provider_result.get('discrepancy_count', 0)
                score = max(0, 100 - discrepancy_count * 2)  # -2 points per discrepancy
                scores.append(score)
            weights.append(0.2)
            
        # Normalize weights if needed
        total_weight = sum(weights)
        if total_weight > 0:
            weights = [w / total_weight for w in weights]
            
        # Calculate weighted average
        integrity_score = sum(s * w for s, w in zip(scores, weights))
        
        return min(100.0, max(0.0, integrity_score))
        
    def _generate_recommendations(
        self,
        integrity_score: float,
        row_count_result: Dict[str, Any],
        checksum_results: Dict[str, ChecksumResult],
        statistical_result: Dict[str, Any]
    ) -> List[str]:
        """Generate recommendations based on validation results."""
        recommendations = []
        
        # Overall integrity
        if integrity_score < 90:
            recommendations.append(
                f"Data integrity score is {integrity_score:.1f}%. "
                "Review and address validation failures."
            )
            
        # Row count issues
        if row_count_result.get('match_percentage', 100) < 95:
            diff = row_count_result.get('difference', 0)
            recommendations.append(
                f"Row count mismatch detected ({diff:+d} records). "
                "Verify data completeness and check for missing batches."
            )
            
        # Checksum failures
        for checksum_type, result in checksum_results.items():
            if not result.is_valid:
                recommendations.append(
                    f"{checksum_type.upper()} checksum validation failed. "
                    "Data may have been modified or corrupted during transfer."
                )
                
        # Statistical anomalies
        if statistical_result.get('anomalies'):
            anomalies = statistical_result['anomalies']
            if anomalies['zero_prices'] > 0:
                recommendations.append(
                    f"Found {anomalies['zero_prices']} zero price records. "
                    "Clean or remove invalid price data."
                )
            if anomalies['negative_volumes'] > 0:
                recommendations.append(
                    f"Found {anomalies['negative_volumes']} negative volume records. "
                    "Fix volume data validation."
                )
            if anomalies['price_outliers'] > anomalies['total_records'] * 0.01:
                recommendations.append(
                    "More than 1% of prices are statistical outliers. "
                    "Review data source quality and filtering."
                )
                
        # Add positive feedback if all good
        if not recommendations:
            recommendations.append(
                "Data integrity validation passed all checks. "
                "Data is ready for production use."
            )
            
        return recommendations