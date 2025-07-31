"""
Data quality analysis and scoring for market data.

Provides comprehensive quality metrics including:
- Completeness scoring
- Accuracy assessment
- Timeliness evaluation
- Consistency checks
- Overall quality scoring
"""

import asyncio
from typing import List, Dict, Any, Optional, Tuple, Set
from datetime import datetime, timedelta
from dataclasses import dataclass, field
import asyncpg
import numpy as np
from scipy import stats
from collections import defaultdict

from ..utils.logging import get_logger
from ..utils.metrics import metrics


@dataclass
class QualityDimension:
    """Represents a single dimension of data quality."""
    name: str
    score: float  # 0-100
    weight: float  # Weight in overall score
    metrics: Dict[str, Any] = field(default_factory=dict)
    issues: List[str] = field(default_factory=list)
    recommendations: List[str] = field(default_factory=list)
    
    @property
    def weighted_score(self) -> float:
        """Calculate weighted contribution to overall score."""
        return self.score * self.weight


@dataclass
class DataQualityReport:
    """Comprehensive data quality assessment report."""
    symbol: str
    start_date: datetime
    end_date: datetime
    overall_quality_score: float
    quality_grade: str  # A, B, C, D, F
    dimensions: Dict[str, QualityDimension]
    summary_statistics: Dict[str, Any]
    critical_issues: List[str]
    recommendations: List[str]
    detailed_metrics: Dict[str, Any]
    timestamp: datetime = field(default_factory=datetime.now)
    
    def to_dict(self) -> Dict[str, Any]:
        """Convert report to dictionary format."""
        return {
            'symbol': self.symbol,
            'date_range': {
                'start': self.start_date.isoformat(),
                'end': self.end_date.isoformat()
            },
            'overall_quality_score': self.overall_quality_score,
            'quality_grade': self.quality_grade,
            'dimensions': {
                name: {
                    'score': dim.score,
                    'weight': dim.weight,
                    'weighted_score': dim.weighted_score,
                    'metrics': dim.metrics,
                    'issues': dim.issues
                }
                for name, dim in self.dimensions.items()
            },
            'critical_issues': self.critical_issues,
            'recommendations': self.recommendations,
            'timestamp': self.timestamp.isoformat()
        }


class DataQualityAnalyzer:
    """Analyzes and scores data quality across multiple dimensions."""
    
    # Quality dimension weights
    DIMENSION_WEIGHTS = {
        'completeness': 0.25,
        'accuracy': 0.25,
        'consistency': 0.20,
        'timeliness': 0.15,
        'validity': 0.15
    }
    
    # Quality grade thresholds
    GRADE_THRESHOLDS = {
        'A': 90,
        'B': 80,
        'C': 70,
        'D': 60,
        'F': 0
    }
    
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
            
    async def analyze_quality(
        self,
        symbol: str,
        start_date: datetime,
        end_date: datetime,
        expected_records: Optional[int] = None,
        granularity: Optional[str] = '1min'
    ) -> DataQualityReport:
        """Perform comprehensive data quality analysis."""
        self.logger.info(
            f"Starting quality analysis for {symbol} "
            f"from {start_date} to {end_date}"
        )
        
        start_time = asyncio.get_event_loop().time()
        
        # Analyze each quality dimension
        dimensions = {}
        
        # Completeness
        completeness = await self._analyze_completeness(
            symbol, start_date, end_date, expected_records, granularity
        )
        dimensions['completeness'] = completeness
        
        # Accuracy
        accuracy = await self._analyze_accuracy(
            symbol, start_date, end_date
        )
        dimensions['accuracy'] = accuracy
        
        # Consistency
        consistency = await self._analyze_consistency(
            symbol, start_date, end_date
        )
        dimensions['consistency'] = consistency
        
        # Timeliness
        timeliness = await self._analyze_timeliness(
            symbol, start_date, end_date
        )
        dimensions['timeliness'] = timeliness
        
        # Validity
        validity = await self._analyze_validity(
            symbol, start_date, end_date
        )
        dimensions['validity'] = validity
        
        # Calculate overall score
        overall_score = sum(dim.weighted_score for dim in dimensions.values())
        
        # Determine quality grade
        quality_grade = self._determine_grade(overall_score)
        
        # Gather summary statistics
        summary_stats = await self._gather_summary_statistics(
            symbol, start_date, end_date
        )
        
        # Identify critical issues
        critical_issues = self._identify_critical_issues(dimensions)
        
        # Generate recommendations
        recommendations = self._generate_recommendations(
            dimensions, overall_score, critical_issues
        )
        
        # Collect detailed metrics
        detailed_metrics = await self._collect_detailed_metrics(
            symbol, start_date, end_date
        )
        
        # Track metrics
        analysis_time = asyncio.get_event_loop().time() - start_time
        metrics.quality_analysis_duration.labels(
            symbol=symbol
        ).observe(analysis_time)
        
        metrics.data_quality_score.labels(
            symbol=symbol,
            grade=quality_grade
        ).set(overall_score)
        
        self.logger.info(
            f"Quality analysis completed: Score={overall_score:.1f}, "
            f"Grade={quality_grade}, Duration={analysis_time:.2f}s"
        )
        
        return DataQualityReport(
            symbol=symbol,
            start_date=start_date,
            end_date=end_date,
            overall_quality_score=overall_score,
            quality_grade=quality_grade,
            dimensions=dimensions,
            summary_statistics=summary_stats,
            critical_issues=critical_issues,
            recommendations=recommendations,
            detailed_metrics=detailed_metrics
        )
        
    async def _analyze_completeness(
        self,
        symbol: str,
        start_date: datetime,
        end_date: datetime,
        expected_records: Optional[int],
        granularity: str
    ) -> QualityDimension:
        """Analyze data completeness."""
        async with self._conn_pool.acquire() as conn:
            # Count total records
            count_query = """
                SELECT 
                    COUNT(*) as total_count,
                    COUNT(DISTINCT DATE(time)) as days_with_data,
                    MIN(time) as first_record,
                    MAX(time) as last_record
                FROM market_data
                WHERE symbol = $1
                AND time >= $2
                AND time <= $3
            """
            
            result = await conn.fetchrow(count_query, symbol, start_date, end_date)
            
            # Check for NULL values
            null_query = """
                SELECT 
                    COUNT(*) FILTER (WHERE open IS NULL) as null_open,
                    COUNT(*) FILTER (WHERE high IS NULL) as null_high,
                    COUNT(*) FILTER (WHERE low IS NULL) as null_low,
                    COUNT(*) FILTER (WHERE close IS NULL) as null_close,
                    COUNT(*) FILTER (WHERE volume IS NULL) as null_volume
                FROM market_data
                WHERE symbol = $1
                AND time >= $2
                AND time <= $3
            """
            
            null_result = await conn.fetchrow(null_query, symbol, start_date, end_date)
            
            # Calculate expected records based on granularity
            if not expected_records:
                trading_days = self._calculate_trading_days(start_date, end_date)
                if granularity == '1min':
                    expected_records = trading_days * 390  # 6.5 hours * 60 minutes
                elif granularity == '1hour':
                    expected_records = trading_days * 6.5
                elif granularity == '1day':
                    expected_records = trading_days
                    
            # Calculate completeness metrics
            actual_count = result['total_count']
            completeness_ratio = actual_count / expected_records if expected_records > 0 else 0
            
            # Check for gaps
            gap_query = """
                WITH time_series AS (
                    SELECT 
                        time,
                        LAG(time) OVER (ORDER BY time) as prev_time
                    FROM market_data
                    WHERE symbol = $1
                    AND time >= $2
                    AND time <= $3
                )
                SELECT COUNT(*) as gap_count
                FROM time_series
                WHERE time - prev_time > INTERVAL '2 minutes'  -- Adjust based on granularity
            """
            
            gap_result = await conn.fetchrow(gap_query, symbol, start_date, end_date)
            
            # Calculate score
            score = 100.0
            issues = []
            
            # Deduct for missing records
            if completeness_ratio < 1.0:
                score -= (1 - completeness_ratio) * 30
                issues.append(f"Missing {expected_records - actual_count} expected records")
                
            # Deduct for NULL values
            total_nulls = sum([
                null_result['null_open'],
                null_result['null_high'],
                null_result['null_low'],
                null_result['null_close'],
                null_result['null_volume']
            ])
            
            if total_nulls > 0:
                null_ratio = total_nulls / (actual_count * 5) if actual_count > 0 else 0
                score -= null_ratio * 20
                issues.append(f"Found {total_nulls} NULL values across fields")
                
            # Deduct for gaps
            if gap_result['gap_count'] > 0:
                gap_ratio = gap_result['gap_count'] / actual_count if actual_count > 0 else 0
                score -= gap_ratio * 10
                issues.append(f"Found {gap_result['gap_count']} time gaps")
                
            metrics_dict = {
                'total_records': actual_count,
                'expected_records': expected_records,
                'completeness_ratio': completeness_ratio,
                'days_with_data': result['days_with_data'],
                'null_counts': {
                    'open': null_result['null_open'],
                    'high': null_result['null_high'],
                    'low': null_result['null_low'],
                    'close': null_result['null_close'],
                    'volume': null_result['null_volume']
                },
                'gap_count': gap_result['gap_count']
            }
            
            recommendations = []
            if completeness_ratio < 0.9:
                recommendations.append(
                    "Data completeness is below 90%. Consider backfilling missing periods."
                )
                
            return QualityDimension(
                name='completeness',
                score=max(0, min(100, score)),
                weight=self.DIMENSION_WEIGHTS['completeness'],
                metrics=metrics_dict,
                issues=issues,
                recommendations=recommendations
            )
            
    async def _analyze_accuracy(
        self,
        symbol: str,
        start_date: datetime,
        end_date: datetime
    ) -> QualityDimension:
        """Analyze data accuracy."""
        async with self._conn_pool.acquire() as conn:
            # Check for price anomalies
            anomaly_query = """
                WITH price_stats AS (
                    SELECT 
                        AVG(close) as avg_price,
                        STDDEV(close) as std_price
                    FROM market_data
                    WHERE symbol = $1
                    AND time >= $2
                    AND time <= $3
                )
                SELECT 
                    COUNT(*) FILTER (WHERE close <= 0) as zero_prices,
                    COUNT(*) FILTER (WHERE ABS(close - ps.avg_price) > 5 * ps.std_price) as extreme_outliers,
                    COUNT(*) FILTER (WHERE high < low) as invalid_ohlc,
                    COUNT(*) FILTER (WHERE volume < 0) as negative_volume,
                    COUNT(*) as total_count
                FROM market_data, price_stats ps
                WHERE symbol = $1
                AND time >= $2
                AND time <= $3
            """
            
            result = await conn.fetchrow(anomaly_query, symbol, start_date, end_date)
            
            # Check for suspicious price movements
            movement_query = """
                WITH price_changes AS (
                    SELECT 
                        time,
                        close,
                        LAG(close) OVER (ORDER BY time) as prev_close,
                        ABS((close - LAG(close) OVER (ORDER BY time)) / NULLIF(LAG(close) OVER (ORDER BY time), 0) * 100) as pct_change
                    FROM market_data
                    WHERE symbol = $1
                    AND time >= $2
                    AND time <= $3
                )
                SELECT 
                    COUNT(*) FILTER (WHERE pct_change > 20) as large_moves,
                    MAX(pct_change) as max_move
                FROM price_changes
            """
            
            movement_result = await conn.fetchrow(movement_query, symbol, start_date, end_date)
            
            # Calculate accuracy score
            score = 100.0
            issues = []
            
            # Critical errors
            if result['zero_prices'] > 0:
                score -= 30
                issues.append(f"Found {result['zero_prices']} zero or negative prices")
                
            if result['invalid_ohlc'] > 0:
                score -= 20
                issues.append(f"Found {result['invalid_ohlc']} records with high < low")
                
            if result['negative_volume'] > 0:
                score -= 20
                issues.append(f"Found {result['negative_volume']} negative volume records")
                
            # Outliers
            outlier_ratio = result['extreme_outliers'] / result['total_count'] if result['total_count'] > 0 else 0
            if outlier_ratio > 0.001:  # More than 0.1%
                score -= min(10, outlier_ratio * 1000)
                issues.append(f"Found {result['extreme_outliers']} extreme price outliers")
                
            # Large movements
            if movement_result['large_moves'] > 0:
                score -= min(10, movement_result['large_moves'] * 0.5)
                issues.append(f"Found {movement_result['large_moves']} price moves > 20%")
                
            metrics_dict = {
                'zero_prices': result['zero_prices'],
                'invalid_ohlc': result['invalid_ohlc'],
                'negative_volume': result['negative_volume'],
                'extreme_outliers': result['extreme_outliers'],
                'outlier_ratio': outlier_ratio,
                'large_price_moves': movement_result['large_moves'],
                'max_price_move_pct': float(movement_result['max_move']) if movement_result['max_move'] else 0
            }
            
            recommendations = []
            if result['zero_prices'] > 0 or result['invalid_ohlc'] > 0:
                recommendations.append(
                    "Critical data accuracy issues detected. Review data validation pipeline."
                )
                
            return QualityDimension(
                name='accuracy',
                score=max(0, min(100, score)),
                weight=self.DIMENSION_WEIGHTS['accuracy'],
                metrics=metrics_dict,
                issues=issues,
                recommendations=recommendations
            )
            
    async def _analyze_consistency(
        self,
        symbol: str,
        start_date: datetime,
        end_date: datetime
    ) -> QualityDimension:
        """Analyze data consistency."""
        async with self._conn_pool.acquire() as conn:
            # Check OHLC consistency
            consistency_query = """
                SELECT 
                    COUNT(*) FILTER (WHERE NOT (high >= open AND high >= close)) as high_violations,
                    COUNT(*) FILTER (WHERE NOT (low <= open AND low <= close)) as low_violations,
                    COUNT(*) FILTER (WHERE close > high OR close < low) as close_out_of_range,
                    COUNT(*) FILTER (WHERE open > high OR open < low) as open_out_of_range,
                    COUNT(*) as total_count
                FROM market_data
                WHERE symbol = $1
                AND time >= $2
                AND time <= $3
            """
            
            result = await conn.fetchrow(consistency_query, symbol, start_date, end_date)
            
            # Check provider consistency
            provider_query = """
                WITH provider_comparison AS (
                    SELECT 
                        time,
                        COUNT(DISTINCT provider) as provider_count,
                        MAX(close) - MIN(close) as price_spread,
                        AVG(close) as avg_price
                    FROM market_data
                    WHERE symbol = $1
                    AND time >= $2
                    AND time <= $3
                    GROUP BY time
                    HAVING COUNT(DISTINCT provider) > 1
                )
                SELECT 
                    COUNT(*) as overlap_count,
                    AVG(price_spread / NULLIF(avg_price, 0) * 100) as avg_spread_pct,
                    MAX(price_spread / NULLIF(avg_price, 0) * 100) as max_spread_pct
                FROM provider_comparison
            """
            
            provider_result = await conn.fetchrow(provider_query, symbol, start_date, end_date)
            
            # Calculate consistency score
            score = 100.0
            issues = []
            
            # OHLC violations
            total_violations = (
                result['high_violations'] +
                result['low_violations'] +
                result['close_out_of_range'] +
                result['open_out_of_range']
            )
            
            if total_violations > 0:
                violation_ratio = total_violations / result['total_count'] if result['total_count'] > 0 else 0
                score -= min(30, violation_ratio * 3000)
                issues.append(f"Found {total_violations} OHLC consistency violations")
                
            # Provider consistency
            if provider_result['overlap_count'] > 0:
                avg_spread = provider_result['avg_spread_pct'] or 0
                if avg_spread > 0.1:  # More than 0.1% average spread
                    score -= min(20, avg_spread * 20)
                    issues.append(
                        f"Provider price discrepancies averaging {avg_spread:.2f}%"
                    )
                    
            metrics_dict = {
                'ohlc_violations': {
                    'high': result['high_violations'],
                    'low': result['low_violations'],
                    'close_out_of_range': result['close_out_of_range'],
                    'open_out_of_range': result['open_out_of_range']
                },
                'provider_consistency': {
                    'overlapping_records': provider_result['overlap_count'] or 0,
                    'avg_price_spread_pct': float(provider_result['avg_spread_pct']) if provider_result['avg_spread_pct'] else 0,
                    'max_price_spread_pct': float(provider_result['max_spread_pct']) if provider_result['max_spread_pct'] else 0
                }
            }
            
            recommendations = []
            if total_violations > result['total_count'] * 0.001:
                recommendations.append(
                    "OHLC consistency violations exceed threshold. Review data cleaning process."
                )
                
            return QualityDimension(
                name='consistency',
                score=max(0, min(100, score)),
                weight=self.DIMENSION_WEIGHTS['consistency'],
                metrics=metrics_dict,
                issues=issues,
                recommendations=recommendations
            )
            
    async def _analyze_timeliness(
        self,
        symbol: str,
        start_date: datetime,
        end_date: datetime
    ) -> QualityDimension:
        """Analyze data timeliness."""
        async with self._conn_pool.acquire() as conn:
            # Check data freshness
            freshness_query = """
                SELECT 
                    MAX(time) as latest_data,
                    NOW() - MAX(time) as data_age,
                    COUNT(DISTINCT DATE(time)) as days_updated
                FROM market_data
                WHERE symbol = $1
                AND time >= $2
                AND time <= $3
            """
            
            result = await conn.fetchrow(freshness_query, symbol, start_date, end_date)
            
            # Check for delayed data
            delay_query = """
                SELECT 
                    AVG(EXTRACT(EPOCH FROM (inserted_at - time))) as avg_delay_seconds,
                    MAX(EXTRACT(EPOCH FROM (inserted_at - time))) as max_delay_seconds,
                    COUNT(*) FILTER (WHERE EXTRACT(EPOCH FROM (inserted_at - time)) > 3600) as delayed_records
                FROM market_data
                WHERE symbol = $1
                AND time >= $2
                AND time <= $3
                AND inserted_at IS NOT NULL
            """
            
            delay_result = await conn.fetchrow(delay_query, symbol, start_date, end_date)
            
            # Calculate timeliness score
            score = 100.0
            issues = []
            
            # Check if data is current
            if result['latest_data']:
                data_age = result['data_age']
                if data_age and data_age.total_seconds() > 86400:  # More than 1 day old
                    days_old = data_age.total_seconds() / 86400
                    score -= min(30, days_old * 5)
                    issues.append(f"Latest data is {days_old:.1f} days old")
                    
            # Check for delays in data insertion
            if delay_result['avg_delay_seconds']:
                avg_delay = delay_result['avg_delay_seconds']
                if avg_delay > 300:  # More than 5 minutes average delay
                    score -= min(20, (avg_delay / 300) * 5)
                    issues.append(f"Average data insertion delay: {avg_delay:.0f} seconds")
                    
            if delay_result['delayed_records'] and delay_result['delayed_records'] > 0:
                score -= min(10, delay_result['delayed_records'] * 0.001)
                issues.append(f"Found {delay_result['delayed_records']} records with >1 hour delay")
                
            metrics_dict = {
                'latest_data_time': result['latest_data'].isoformat() if result['latest_data'] else None,
                'data_age_hours': result['data_age'].total_seconds() / 3600 if result['data_age'] else None,
                'avg_insertion_delay_seconds': float(delay_result['avg_delay_seconds']) if delay_result['avg_delay_seconds'] else 0,
                'max_insertion_delay_seconds': float(delay_result['max_delay_seconds']) if delay_result['max_delay_seconds'] else 0,
                'delayed_record_count': delay_result['delayed_records'] or 0
            }
            
            recommendations = []
            if result['data_age'] and result['data_age'].total_seconds() > 86400:
                recommendations.append(
                    "Data is not current. Ensure real-time data feeds are working properly."
                )
                
            return QualityDimension(
                name='timeliness',
                score=max(0, min(100, score)),
                weight=self.DIMENSION_WEIGHTS['timeliness'],
                metrics=metrics_dict,
                issues=issues,
                recommendations=recommendations
            )
            
    async def _analyze_validity(
        self,
        symbol: str,
        start_date: datetime,
        end_date: datetime
    ) -> QualityDimension:
        """Analyze data validity."""
        async with self._conn_pool.acquire() as conn:
            # Check business rules
            validity_query = """
                SELECT 
                    COUNT(*) FILTER (WHERE close <= 0) as invalid_prices,
                    COUNT(*) FILTER (WHERE volume < 0) as invalid_volumes,
                    COUNT(*) FILTER (WHERE time > NOW()) as future_timestamps,
                    COUNT(*) FILTER (WHERE provider IS NULL OR provider = '') as missing_provider,
                    COUNT(*) as total_count
                FROM market_data
                WHERE symbol = $1
                AND time >= $2
                AND time <= $3
            """
            
            result = await conn.fetchrow(validity_query, symbol, start_date, end_date)
            
            # Check data patterns
            pattern_query = """
                WITH daily_stats AS (
                    SELECT 
                        DATE(time) as date,
                        COUNT(*) as record_count,
                        MIN(close) as min_price,
                        MAX(close) as max_price
                    FROM market_data
                    WHERE symbol = $1
                    AND time >= $2
                    AND time <= $3
                    GROUP BY DATE(time)
                )
                SELECT 
                    COUNT(*) FILTER (WHERE record_count = 0) as empty_days,
                    COUNT(*) FILTER (WHERE min_price = max_price AND record_count > 1) as static_days,
                    STDDEV(record_count) as count_stddev
                FROM daily_stats
            """
            
            pattern_result = await conn.fetchrow(pattern_query, symbol, start_date, end_date)
            
            # Calculate validity score
            score = 100.0
            issues = []
            
            # Critical validity issues
            if result['invalid_prices'] > 0:
                score -= 30
                issues.append(f"Found {result['invalid_prices']} invalid prices")
                
            if result['invalid_volumes'] > 0:
                score -= 20
                issues.append(f"Found {result['invalid_volumes']} invalid volumes")
                
            if result['future_timestamps'] > 0:
                score -= 20
                issues.append(f"Found {result['future_timestamps']} future timestamps")
                
            if result['missing_provider'] > 0:
                score -= 10
                issues.append(f"Found {result['missing_provider']} records without provider")
                
            # Pattern issues
            if pattern_result['static_days'] and pattern_result['static_days'] > 0:
                score -= min(10, pattern_result['static_days'] * 2)
                issues.append(f"Found {pattern_result['static_days']} days with no price movement")
                
            metrics_dict = {
                'invalid_prices': result['invalid_prices'],
                'invalid_volumes': result['invalid_volumes'],
                'future_timestamps': result['future_timestamps'],
                'missing_provider': result['missing_provider'],
                'static_price_days': pattern_result['static_days'] or 0,
                'empty_days': pattern_result['empty_days'] or 0
            }
            
            recommendations = []
            if result['invalid_prices'] > 0 or result['invalid_volumes'] > 0:
                recommendations.append(
                    "Critical validity issues found. Implement stricter validation rules."
                )
                
            return QualityDimension(
                name='validity',
                score=max(0, min(100, score)),
                weight=self.DIMENSION_WEIGHTS['validity'],
                metrics=metrics_dict,
                issues=issues,
                recommendations=recommendations
            )
            
    async def _gather_summary_statistics(
        self,
        symbol: str,
        start_date: datetime,
        end_date: datetime
    ) -> Dict[str, Any]:
        """Gather summary statistics for the data."""
        async with self._conn_pool.acquire() as conn:
            summary_query = """
                SELECT 
                    COUNT(*) as total_records,
                    COUNT(DISTINCT provider) as provider_count,
                    COUNT(DISTINCT DATE(time)) as trading_days,
                    MIN(time) as first_timestamp,
                    MAX(time) as last_timestamp,
                    AVG(close) as avg_price,
                    STDDEV(close) as price_volatility,
                    SUM(volume) as total_volume,
                    AVG(volume) as avg_volume
                FROM market_data
                WHERE symbol = $1
                AND time >= $2
                AND time <= $3
            """
            
            result = await conn.fetchrow(summary_query, symbol, start_date, end_date)
            
            return {
                'total_records': result['total_records'],
                'provider_count': result['provider_count'],
                'trading_days': result['trading_days'],
                'date_range': {
                    'first': result['first_timestamp'].isoformat() if result['first_timestamp'] else None,
                    'last': result['last_timestamp'].isoformat() if result['last_timestamp'] else None
                },
                'price_statistics': {
                    'average': float(result['avg_price']) if result['avg_price'] else 0,
                    'volatility': float(result['price_volatility']) if result['price_volatility'] else 0
                },
                'volume_statistics': {
                    'total': result['total_volume'] or 0,
                    'average': float(result['avg_volume']) if result['avg_volume'] else 0
                }
            }
            
    async def _collect_detailed_metrics(
        self,
        symbol: str,
        start_date: datetime,
        end_date: datetime
    ) -> Dict[str, Any]:
        """Collect detailed quality metrics."""
        async with self._conn_pool.acquire() as conn:
            # Provider quality comparison
            provider_query = """
                SELECT 
                    provider,
                    COUNT(*) as record_count,
                    COUNT(*) FILTER (WHERE close <= 0) as invalid_records,
                    AVG(close) as avg_price,
                    STDDEV(close) as price_stddev
                FROM market_data
                WHERE symbol = $1
                AND time >= $2
                AND time <= $3
                GROUP BY provider
            """
            
            provider_results = await conn.fetch(provider_query, symbol, start_date, end_date)
            
            return {
                'provider_quality': [
                    {
                        'provider': row['provider'],
                        'record_count': row['record_count'],
                        'invalid_records': row['invalid_records'],
                        'quality_score': (
                            100 * (1 - row['invalid_records'] / row['record_count'])
                            if row['record_count'] > 0 else 0
                        )
                    }
                    for row in provider_results
                ]
            }
            
    def _calculate_trading_days(self, start_date: datetime, end_date: datetime) -> int:
        """Calculate number of trading days between dates."""
        # Simplified calculation - would need market calendar for accuracy
        total_days = (end_date - start_date).days
        weeks = total_days // 7
        remaining_days = total_days % 7
        
        # Rough estimate: 5 trading days per week
        trading_days = weeks * 5
        
        # Add remaining days (simplified)
        if remaining_days > 0:
            trading_days += min(5, remaining_days)
            
        return max(1, trading_days)
        
    def _determine_grade(self, score: float) -> str:
        """Determine quality grade based on score."""
        for grade, threshold in sorted(
            self.GRADE_THRESHOLDS.items(),
            key=lambda x: x[1],
            reverse=True
        ):
            if score >= threshold:
                return grade
        return 'F'
        
    def _identify_critical_issues(self, dimensions: Dict[str, QualityDimension]) -> List[str]:
        """Identify critical issues from dimension analysis."""
        critical_issues = []
        
        for name, dimension in dimensions.items():
            if dimension.score < 60:  # Below D grade
                critical_issues.append(
                    f"{name.capitalize()} score critically low: {dimension.score:.1f}"
                )
                
            # Add specific critical issues from each dimension
            for issue in dimension.issues:
                if any(keyword in issue.lower() for keyword in ['zero', 'negative', 'invalid', 'critical']):
                    critical_issues.append(f"[{name}] {issue}")
                    
        return critical_issues
        
    def _generate_recommendations(
        self,
        dimensions: Dict[str, QualityDimension],
        overall_score: float,
        critical_issues: List[str]
    ) -> List[str]:
        """Generate actionable recommendations."""
        recommendations = []
        
        # Overall recommendations
        if overall_score < 70:
            recommendations.append(
                "Overall data quality is below acceptable threshold. "
                "Prioritize addressing critical issues before using data in production."
            )
            
        # Dimension-specific recommendations
        for dimension in dimensions.values():
            recommendations.extend(dimension.recommendations)
            
        # Critical issue recommendations
        if critical_issues:
            recommendations.append(
                f"Address {len(critical_issues)} critical issues immediately. "
                "These may cause system failures or incorrect analysis."
            )
            
        # Add general best practices
        if overall_score < 90:
            if 'completeness' in dimensions and dimensions['completeness'].score < 80:
                recommendations.append(
                    "Improve data collection reliability and implement gap detection."
                )
            if 'accuracy' in dimensions and dimensions['accuracy'].score < 80:
                recommendations.append(
                    "Enhance data validation rules and outlier detection."
                )
                
        return list(set(recommendations))  # Remove duplicates