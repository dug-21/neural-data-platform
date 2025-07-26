"""
Gap detection and analysis for time series market data.

Identifies and analyzes gaps in historical data including:
- Market hours gaps vs non-trading hours
- Weekend and holiday gaps
- Unexpected data gaps
- Gap severity classification
"""

import asyncio
from typing import List, Dict, Any, Optional, Tuple, Set
from datetime import datetime, timedelta, time
from dataclasses import dataclass, field
import asyncpg
import pandas as pd
import holidays
from enum import Enum

from ..utils.logging import get_logger
from ..utils.metrics import metrics


class GapSeverity(Enum):
    """Classification of gap severity."""
    EXPECTED = "expected"      # Weekend, holiday, after-hours
    MINOR = "minor"            # Small gaps during trading hours
    MODERATE = "moderate"      # Moderate gaps that may need attention
    SEVERE = "severe"          # Large unexpected gaps
    CRITICAL = "critical"      # Critical gaps affecting data integrity


@dataclass
class DataGap:
    """Represents a gap in time series data."""
    symbol: str
    start_time: datetime
    end_time: datetime
    duration: timedelta
    severity: GapSeverity
    gap_type: str  # 'weekend', 'holiday', 'after_hours', 'unexpected'
    records_missing: Optional[int] = None
    is_market_hours: bool = False
    notes: Optional[str] = None
    
    @property
    def duration_hours(self) -> float:
        """Gap duration in hours."""
        return self.duration.total_seconds() / 3600
        
    @property
    def duration_minutes(self) -> float:
        """Gap duration in minutes."""
        return self.duration.total_seconds() / 60


@dataclass
class GapAnalysisResult:
    """Result of gap analysis."""
    symbol: str
    start_date: datetime
    end_date: datetime
    total_gaps: int
    gaps_by_severity: Dict[GapSeverity, int]
    gaps_by_type: Dict[str, int]
    largest_gap: Optional[DataGap]
    total_missing_time: timedelta
    coverage_percentage: float
    gap_details: List[DataGap]
    summary_statistics: Dict[str, Any]
    recommendations: List[str]


class GapDetector:
    """Detects and analyzes gaps in market data."""
    
    # Market hours (simplified - would need adjustment for different markets)
    MARKET_OPEN = time(9, 30)   # 9:30 AM
    MARKET_CLOSE = time(16, 0)  # 4:00 PM
    
    # Expected intervals by granularity (in minutes)
    EXPECTED_INTERVALS = {
        'tick': 0.0167,      # 1 second
        '1min': 1,
        '5min': 5,
        '15min': 15,
        '30min': 30,
        '1hour': 60,
        '4hour': 240,
        '1day': 1440
    }
    
    def __init__(self, db_connection_string: str, market: str = 'US'):
        self.logger = get_logger(__name__)
        self.db_connection_string = db_connection_string
        self.market = market
        self.holidays = holidays.US()  # US market holidays
        self._conn_pool: Optional[asyncpg.Pool] = None
        
    async def __aenter__(self):
        """Initialize database connection pool."""
        self._conn_pool = await asyncpg.create_pool(
            self.db_connection_string,
            min_size=2,
            max_size=10
        )
        return self
        
    async def __aexit__(self, exc_type, exc_val, exc_tb):
        """Close database connection pool."""
        if self._conn_pool:
            await self._conn_pool.close()
            
    async def analyze_gaps(
        self,
        symbol: str,
        start_date: datetime,
        end_date: datetime,
        granularity: str = '1min',
        detailed_analysis: bool = True
    ) -> GapAnalysisResult:
        """Perform comprehensive gap analysis on market data."""
        self.logger.info(
            f"Starting gap analysis for {symbol} from {start_date} to {end_date}"
        )
        
        start_time = asyncio.get_event_loop().time()
        
        # Fetch data points
        data_points = await self._fetch_data_points(symbol, start_date, end_date)
        
        if not data_points:
            return GapAnalysisResult(
                symbol=symbol,
                start_date=start_date,
                end_date=end_date,
                total_gaps=0,
                gaps_by_severity={},
                gaps_by_type={},
                largest_gap=None,
                total_missing_time=timedelta(0),
                coverage_percentage=0.0,
                gap_details=[],
                summary_statistics={},
                recommendations=["No data found for analysis"]
            )
        
        # Detect gaps
        gaps = await self._detect_gaps(data_points, granularity)
        
        # Classify gaps
        classified_gaps = await self._classify_gaps(gaps, symbol)
        
        # Calculate statistics
        stats = await self._calculate_gap_statistics(
            classified_gaps, data_points, start_date, end_date
        )
        
        # Generate recommendations
        recommendations = self._generate_recommendations(classified_gaps, stats)
        
        # Track metrics
        analysis_time = asyncio.get_event_loop().time() - start_time
        metrics.gap_analysis_duration.labels(
            symbol=symbol,
            granularity=granularity
        ).observe(analysis_time)
        
        self.logger.info(
            f"Gap analysis completed: found {len(classified_gaps)} gaps "
            f"in {analysis_time:.2f}s"
        )
        
        return GapAnalysisResult(
            symbol=symbol,
            start_date=start_date,
            end_date=end_date,
            total_gaps=len(classified_gaps),
            gaps_by_severity=stats['gaps_by_severity'],
            gaps_by_type=stats['gaps_by_type'],
            largest_gap=stats['largest_gap'],
            total_missing_time=stats['total_missing_time'],
            coverage_percentage=stats['coverage_percentage'],
            gap_details=classified_gaps if detailed_analysis else classified_gaps[:100],
            summary_statistics=stats,
            recommendations=recommendations
        )
        
    async def _fetch_data_points(
        self,
        symbol: str,
        start_date: datetime,
        end_date: datetime
    ) -> List[datetime]:
        """Fetch all timestamps for the symbol in date range."""
        async with self._conn_pool.acquire() as conn:
            query = """
                SELECT DISTINCT time
                FROM market_data
                WHERE symbol = $1
                AND time >= $2
                AND time <= $3
                ORDER BY time
            """
            
            rows = await conn.fetch(query, symbol, start_date, end_date)
            return [row['time'] for row in rows]
            
    async def _detect_gaps(
        self,
        data_points: List[datetime],
        granularity: str
    ) -> List[Tuple[datetime, datetime, timedelta]]:
        """Detect gaps in the time series."""
        gaps = []
        expected_interval = timedelta(minutes=self.EXPECTED_INTERVALS.get(granularity, 1))
        
        # Allow some tolerance for slight timing variations
        tolerance = expected_interval * 0.1
        max_allowed_gap = expected_interval + tolerance
        
        for i in range(1, len(data_points)):
            prev_time = data_points[i-1]
            curr_time = data_points[i]
            time_diff = curr_time - prev_time
            
            # Detect gap if interval is larger than expected
            if time_diff > max_allowed_gap:
                gaps.append((prev_time, curr_time, time_diff))
                
        return gaps
        
    async def _classify_gaps(
        self,
        gaps: List[Tuple[datetime, datetime, timedelta]],
        symbol: str
    ) -> List[DataGap]:
        """Classify gaps by type and severity."""
        classified_gaps = []
        
        for gap_start, gap_end, duration in gaps:
            gap_type, is_market_hours = self._determine_gap_type(gap_start, gap_end)
            severity = self._determine_gap_severity(
                duration, gap_type, is_market_hours
            )
            
            # Estimate missing records based on gap duration
            if gap_type == 'unexpected' and is_market_hours:
                # During market hours, expect data every minute
                records_missing = int(duration.total_seconds() / 60)
            else:
                records_missing = None
                
            gap = DataGap(
                symbol=symbol,
                start_time=gap_start,
                end_time=gap_end,
                duration=duration,
                severity=severity,
                gap_type=gap_type,
                records_missing=records_missing,
                is_market_hours=is_market_hours
            )
            
            classified_gaps.append(gap)
            
        return classified_gaps
        
    def _determine_gap_type(
        self,
        gap_start: datetime,
        gap_end: datetime
    ) -> Tuple[str, bool]:
        """Determine the type of gap and if it's during market hours."""
        # Check if gap spans weekend
        if gap_start.weekday() == 4 and gap_end.weekday() == 0:  # Friday to Monday
            return 'weekend', False
            
        # Check if gap includes holiday
        current = gap_start.date()
        while current <= gap_end.date():
            if current in self.holidays:
                return 'holiday', False
            current += timedelta(days=1)
            
        # Check if gap is entirely outside market hours
        gap_start_time = gap_start.time()
        gap_end_time = gap_end.time()
        
        if (gap_start_time >= self.MARKET_CLOSE or 
            gap_end_time <= self.MARKET_OPEN):
            return 'after_hours', False
            
        # Check if gap is during market hours
        is_market_hours = (
            gap_start.weekday() < 5 and  # Weekday
            gap_start_time >= self.MARKET_OPEN and
            gap_start_time <= self.MARKET_CLOSE
        )
        
        return 'unexpected', is_market_hours
        
    def _determine_gap_severity(
        self,
        duration: timedelta,
        gap_type: str,
        is_market_hours: bool
    ) -> GapSeverity:
        """Determine severity of the gap."""
        duration_minutes = duration.total_seconds() / 60
        
        # Expected gaps are always low severity
        if gap_type in ['weekend', 'holiday', 'after_hours']:
            return GapSeverity.EXPECTED
            
        # For unexpected gaps during market hours
        if is_market_hours:
            if duration_minutes < 5:
                return GapSeverity.MINOR
            elif duration_minutes < 30:
                return GapSeverity.MODERATE
            elif duration_minutes < 120:
                return GapSeverity.SEVERE
            else:
                return GapSeverity.CRITICAL
        else:
            # Unexpected gaps outside market hours
            if duration_minutes < 60:
                return GapSeverity.MINOR
            else:
                return GapSeverity.MODERATE
                
    async def _calculate_gap_statistics(
        self,
        gaps: List[DataGap],
        data_points: List[datetime],
        start_date: datetime,
        end_date: datetime
    ) -> Dict[str, Any]:
        """Calculate comprehensive gap statistics."""
        if not gaps:
            total_duration = end_date - start_date
            actual_duration = data_points[-1] - data_points[0] if data_points else timedelta(0)
            coverage_pct = (actual_duration.total_seconds() / total_duration.total_seconds() * 100) if total_duration.total_seconds() > 0 else 100
            
            return {
                'gaps_by_severity': {},
                'gaps_by_type': {},
                'largest_gap': None,
                'total_missing_time': timedelta(0),
                'coverage_percentage': coverage_pct,
                'avg_gap_duration': timedelta(0),
                'market_hours_gaps': 0,
                'critical_gaps': []
            }
            
        # Group gaps by severity and type
        gaps_by_severity = {}
        gaps_by_type = {}
        
        for gap in gaps:
            gaps_by_severity[gap.severity] = gaps_by_severity.get(gap.severity, 0) + 1
            gaps_by_type[gap.gap_type] = gaps_by_type.get(gap.gap_type, 0) + 1
            
        # Find largest gap
        largest_gap = max(gaps, key=lambda g: g.duration)
        
        # Calculate total missing time
        total_missing_time = sum((gap.duration for gap in gaps), timedelta(0))
        
        # Calculate coverage percentage
        total_duration = end_date - start_date
        missing_seconds = total_missing_time.total_seconds()
        total_seconds = total_duration.total_seconds()
        coverage_pct = ((total_seconds - missing_seconds) / total_seconds * 100) if total_seconds > 0 else 0
        
        # Market hours gaps
        market_hours_gaps = [g for g in gaps if g.is_market_hours]
        
        # Critical gaps
        critical_gaps = [g for g in gaps if g.severity == GapSeverity.CRITICAL]
        
        return {
            'gaps_by_severity': gaps_by_severity,
            'gaps_by_type': gaps_by_type,
            'largest_gap': largest_gap,
            'total_missing_time': total_missing_time,
            'coverage_percentage': coverage_pct,
            'avg_gap_duration': total_missing_time / len(gaps) if gaps else timedelta(0),
            'market_hours_gaps': len(market_hours_gaps),
            'critical_gaps': critical_gaps,
            'gap_duration_percentiles': self._calculate_duration_percentiles(gaps)
        }
        
    def _calculate_duration_percentiles(self, gaps: List[DataGap]) -> Dict[str, float]:
        """Calculate gap duration percentiles."""
        if not gaps:
            return {}
            
        durations = sorted([g.duration_minutes for g in gaps])
        n = len(durations)
        
        return {
            'p50': durations[n // 2],
            'p75': durations[int(n * 0.75)],
            'p90': durations[int(n * 0.90)],
            'p95': durations[int(n * 0.95)] if n > 20 else durations[-1],
            'p99': durations[int(n * 0.99)] if n > 100 else durations[-1]
        }
        
    def _generate_recommendations(
        self,
        gaps: List[DataGap],
        stats: Dict[str, Any]
    ) -> List[str]:
        """Generate recommendations based on gap analysis."""
        recommendations = []
        
        # Check coverage
        if stats['coverage_percentage'] < 90:
            recommendations.append(
                f"Data coverage is only {stats['coverage_percentage']:.1f}%. "
                "Consider alternative data sources for missing periods."
            )
            
        # Check for critical gaps
        critical_gaps = stats.get('critical_gaps', [])
        if critical_gaps:
            recommendations.append(
                f"Found {len(critical_gaps)} critical gaps. "
                "Investigate and backfill these periods urgently."
            )
            
        # Check market hours gaps
        market_hours_gaps = stats.get('market_hours_gaps', 0)
        if market_hours_gaps > 10:
            recommendations.append(
                f"Found {market_hours_gaps} gaps during market hours. "
                "Review data provider reliability and connection stability."
            )
            
        # Check for patterns
        gaps_by_type = stats.get('gaps_by_type', {})
        if gaps_by_type.get('unexpected', 0) > len(gaps) * 0.1:
            recommendations.append(
                "More than 10% of gaps are unexpected. "
                "Consider implementing real-time monitoring and alerts."
            )
            
        # Provide backfill suggestions
        if gaps:
            severe_gaps = [g for g in gaps if g.severity in [GapSeverity.SEVERE, GapSeverity.CRITICAL]]
            if severe_gaps:
                recommendations.append(
                    f"Prioritize backfilling {len(severe_gaps)} severe/critical gaps "
                    f"totaling {sum(g.duration for g in severe_gaps)} of missing data."
                )
                
        return recommendations
        
    async def generate_gap_report(
        self,
        result: GapAnalysisResult,
        output_format: str = 'text'
    ) -> str:
        """Generate a formatted gap analysis report."""
        if output_format == 'json':
            import json
            return json.dumps({
                'symbol': result.symbol,
                'date_range': {
                    'start': result.start_date.isoformat(),
                    'end': result.end_date.isoformat()
                },
                'summary': {
                    'total_gaps': result.total_gaps,
                    'coverage_percentage': result.coverage_percentage,
                    'total_missing_time': str(result.total_missing_time)
                },
                'gaps_by_severity': {k.value: v for k, v in result.gaps_by_severity.items()},
                'gaps_by_type': result.gaps_by_type,
                'recommendations': result.recommendations
            }, indent=2)
            
        # Text format report
        report = []
        report.append(f"Gap Analysis Report for {result.symbol}")
        report.append("=" * 50)
        report.append(f"Date Range: {result.start_date} to {result.end_date}")
        report.append(f"Total Gaps Found: {result.total_gaps}")
        report.append(f"Data Coverage: {result.coverage_percentage:.2f}%")
        report.append(f"Total Missing Time: {result.total_missing_time}")
        
        if result.largest_gap:
            report.append(f"\nLargest Gap: {result.largest_gap.duration} "
                         f"({result.largest_gap.start_time} to {result.largest_gap.end_time})")
            
        report.append("\nGaps by Severity:")
        for severity, count in sorted(result.gaps_by_severity.items(), key=lambda x: x[0].value):
            report.append(f"  {severity.value.capitalize()}: {count}")
            
        report.append("\nGaps by Type:")
        for gap_type, count in result.gaps_by_type.items():
            report.append(f"  {gap_type.capitalize()}: {count}")
            
        if result.recommendations:
            report.append("\nRecommendations:")
            for i, rec in enumerate(result.recommendations, 1):
                report.append(f"  {i}. {rec}")
                
        return "\n".join(report)