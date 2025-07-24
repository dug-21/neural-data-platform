"""
Enhanced Historical Data Backfill Coordinator.

Provides comprehensive historical data loading with:
- Parallel data fetching from multiple sources
- Data validation and cleaning pipeline  
- Gap filling and interpolation strategies
- Efficient storage in TimescaleDB
- Incremental loading capabilities
- Progress tracking and resumability
- Automatic retry and error recovery
- Data quality metrics and monitoring
"""

import asyncio
import json
import aiofiles
from datetime import datetime, timedelta
from typing import List, Dict, Any, AsyncIterator, Optional, Tuple, Set
import logging
from dataclasses import dataclass, field, asdict
from enum import Enum
from pathlib import Path
import pandas as pd
import numpy as np
from collections import defaultdict
import hashlib

from providers.base import BaseProvider, MarketData, TickData
from providers.yahoo_finance import YahooFinanceProvider
from providers.alpha_vantage import AlphaVantageProvider
from providers.alpaca import AlpacaProvider
from providers.fred import FREDProvider
from providers.polygon import PolygonProvider
from providers.iex_cloud import IEXCloudProvider
from storage.timescale import TimescaleDB
from utils.logging import get_logger
from utils.metrics import metrics
from utils.retry import with_retry


class BackfillPriority(Enum):
    """Priority levels for backfill operations"""
    CRITICAL = 1   # Tick data - last month
    HIGH = 2       # Minute data - last 6 months
    MEDIUM = 3     # Hourly data - last 2 years
    LOW = 4        # Daily data - last 5 years
    ARCHIVE = 5    # Beyond 5 years


class DataGranularity(Enum):
    """Data granularity levels"""
    TICK = "tick"        # Raw tick data
    MINUTE = "1min"      # 1 minute bars
    MINUTE_5 = "5min"    # 5 minute bars
    MINUTE_15 = "15min"  # 15 minute bars
    MINUTE_30 = "30min"  # 30 minute bars
    HOUR = "1hour"       # 1 hour bars
    HOUR_4 = "4hour"     # 4 hour bars
    DAY = "1day"         # Daily bars
    WEEK = "1week"       # Weekly bars
    MONTH = "1month"     # Monthly bars


class BackfillStatus(Enum):
    """Backfill job status"""
    PENDING = "pending"
    RUNNING = "running"
    COMPLETED = "completed"
    FAILED = "failed"
    PAUSED = "paused"
    CANCELLED = "cancelled"
    RETRYING = "retrying"


@dataclass
class BackfillJob:
    """Enhanced backfill job with comprehensive tracking"""
    # Core fields
    job_id: str
    symbol: str
    provider: str
    start_date: datetime
    end_date: datetime
    priority: BackfillPriority
    granularity: DataGranularity
    status: BackfillStatus = BackfillStatus.PENDING
    
    # Progress tracking
    progress: float = 0.0
    total_points_expected: int = 0
    points_loaded: int = 0
    points_validated: int = 0
    points_stored: int = 0
    
    # Quality metrics
    data_quality_score: float = 0.0
    gaps_found: int = 0
    gaps_filled: int = 0
    invalid_points: int = 0
    
    # Performance metrics
    start_time: Optional[datetime] = None
    end_time: Optional[datetime] = None
    fetch_duration_ms: int = 0
    validation_duration_ms: int = 0
    storage_duration_ms: int = 0
    
    # Error tracking
    error: Optional[str] = None
    error_count: int = 0
    last_error_time: Optional[datetime] = None
    retry_count: int = 0
    max_retries: int = 3
    
    # Checkpointing
    last_checkpoint: Optional[datetime] = None
    checkpoint_data: Dict[str, Any] = field(default_factory=dict)
    
    def __post_init__(self):
        if not self.job_id:
            # Generate unique job ID
            content = f"{self.symbol}-{self.provider}-{self.start_date}-{self.end_date}-{self.granularity.value}"
            self.job_id = hashlib.md5(content.encode()).hexdigest()[:12]
            
    def can_retry(self) -> bool:
        """Check if job can be retried"""
        return self.retry_count < self.max_retries
        
    def update_progress(self):
        """Update overall progress percentage"""
        if self.total_points_expected > 0:
            self.progress = (self.points_stored / self.total_points_expected) * 100
        else:
            # Time-based progress for unknown total
            if self.start_time and self.start_date and self.end_date:
                total_duration = (self.end_date - self.start_date).total_seconds()
                elapsed = (datetime.now() - self.start_time).total_seconds()
                self.progress = min((elapsed / total_duration) * 100, 99)
                
    def estimate_completion_time(self) -> Optional[datetime]:
        """Estimate job completion time"""
        if self.progress > 0 and self.start_time:
            elapsed = (datetime.now() - self.start_time).total_seconds()
            total_estimated = elapsed / (self.progress / 100)
            remaining_seconds = total_estimated - elapsed
            return datetime.now() + timedelta(seconds=remaining_seconds)
        return None


@dataclass 
class DataValidationResult:
    """Results from data validation"""
    is_valid: bool
    total_points: int
    valid_points: int
    invalid_points: int
    missing_points: int
    duplicate_points: int
    quality_score: float
    issues: List[str] = field(default_factory=list)
    statistics: Dict[str, Any] = field(default_factory=dict)
    

@dataclass
class StorageEstimate:
    """Storage requirements estimate"""
    symbol: str
    time_range_days: int
    data_points: Dict[DataGranularity, int]
    storage_bytes: Dict[DataGranularity, int]
    total_storage_gb: float
    estimated_load_time_hours: float
    
    def __str__(self):
        return (
            f"Storage Estimate for {self.symbol}:\n"
            f"  Time Range: {self.time_range_days} days\n"
            f"  Total Storage: {self.total_storage_gb:.2f} GB\n"
            f"  Estimated Load Time: {self.estimated_load_time_hours:.1f} hours\n"
            f"  Data Points by Granularity:\n" +
            "\n".join([f"    {g.value}: {self.data_points[g]:,}" for g in self.data_points])
        )


class HistoricalBackfillCoordinator:
    """Enhanced coordinator for parallel historical data backfill"""
    
    # Storage estimates per data point (bytes)
    STORAGE_PER_POINT = {
        DataGranularity.TICK: 64,      # timestamp + price + size + metadata
        DataGranularity.MINUTE: 96,    # OHLCV + metadata
        DataGranularity.MINUTE_5: 96,
        DataGranularity.MINUTE_15: 96,
        DataGranularity.MINUTE_30: 96,
        DataGranularity.HOUR: 96,
        DataGranularity.HOUR_4: 96,
        DataGranularity.DAY: 96,
        DataGranularity.WEEK: 96,
        DataGranularity.MONTH: 96
    }
    
    # Expected data points per day by granularity
    POINTS_PER_DAY = {
        DataGranularity.TICK: 50000,      # ~50k ticks per day average
        DataGranularity.MINUTE: 390,      # 6.5 hours * 60
        DataGranularity.MINUTE_5: 78,
        DataGranularity.MINUTE_15: 26,
        DataGranularity.MINUTE_30: 13,
        DataGranularity.HOUR: 6.5,
        DataGranularity.HOUR_4: 1.625,
        DataGranularity.DAY: 1,
        DataGranularity.WEEK: 0.2,
        DataGranularity.MONTH: 0.045
    }
    
    def __init__(self, checkpoint_dir: Optional[Path] = None):
        self.logger = get_logger(__name__)
        self.storage = TimescaleDB()
        self.jobs: List[BackfillJob] = []
        self.active_jobs: Dict[str, BackfillJob] = {}
        self.completed_jobs: List[BackfillJob] = []
        self.failed_jobs: List[BackfillJob] = []
        self.providers = {}
        self.checkpoint_dir = checkpoint_dir or Path(".backfill_checkpoints")
        self.checkpoint_dir.mkdir(exist_ok=True)
        self._semaphore = asyncio.Semaphore(10)  # Limit concurrent operations
        self._initialize_providers()
        self._load_checkpoints()
        
    def _initialize_providers(self):
        """Initialize data providers with capabilities mapping"""
        # Map providers to their capabilities
        self.provider_capabilities = {
            'yahoo': {
                'provider': YahooFinanceProvider(),
                'max_history_years': 20,
                'granularities': [DataGranularity.DAY, DataGranularity.WEEK, DataGranularity.MONTH],
                'rate_limit_per_min': 2000,
                'reliability_score': 0.95,
                'cost': 'free'
            },
            'alpaca': {
                'provider': AlpacaProvider(),
                'max_history_years': 5,
                'granularities': [DataGranularity.MINUTE, DataGranularity.MINUTE_5, 
                                DataGranularity.MINUTE_15, DataGranularity.HOUR, 
                                DataGranularity.DAY],
                'rate_limit_per_min': 200,
                'reliability_score': 0.98,
                'cost': 'free'
            },
            'polygon': {
                'provider': PolygonProvider() if hasattr(self, 'polygon_key') else None,
                'max_history_years': 10,
                'granularities': [DataGranularity.TICK, DataGranularity.MINUTE,
                                DataGranularity.HOUR, DataGranularity.DAY],
                'rate_limit_per_min': 100,
                'reliability_score': 0.99,
                'cost': 'paid'
            },
            'alpha_vantage': {
                'provider': AlphaVantageProvider(),
                'max_history_years': 20,
                'granularities': [DataGranularity.MINUTE, DataGranularity.DAY],
                'rate_limit_per_min': 5,  # Very limited on free tier
                'reliability_score': 0.85,
                'cost': 'free'
            },
            'iex': {
                'provider': IEXCloudProvider() if hasattr(self, 'iex_key') else None,
                'max_history_years': 5,
                'granularities': [DataGranularity.MINUTE, DataGranularity.DAY],
                'rate_limit_per_min': 100,
                'reliability_score': 0.97,
                'cost': 'paid'
            }
        }
        
        # Initialize only available providers
        self.providers = {
            name: info['provider'] 
            for name, info in self.provider_capabilities.items() 
            if info['provider'] is not None
        }
        
    def _load_checkpoints(self):
        """Load saved checkpoints from disk"""
        try:
            checkpoint_files = list(self.checkpoint_dir.glob("*.json"))
            for checkpoint_file in checkpoint_files:
                with open(checkpoint_file, 'r') as f:
                    job_data = json.load(f)
                    job = self._deserialize_job(job_data)
                    if job.status in [BackfillStatus.RUNNING, BackfillStatus.RETRYING]:
                        # Resume paused jobs
                        job.status = BackfillStatus.PENDING
                        self.jobs.append(job)
                    elif job.status == BackfillStatus.COMPLETED:
                        self.completed_jobs.append(job)
                    elif job.status == BackfillStatus.FAILED:
                        self.failed_jobs.append(job)
            self.logger.info(f"Loaded {len(self.jobs)} pending jobs from checkpoints")
        except Exception as e:
            self.logger.error(f"Failed to load checkpoints: {e}")
            
    async def _save_checkpoint(self, job: BackfillJob):
        """Save job checkpoint to disk"""
        try:
            checkpoint_file = self.checkpoint_dir / f"{job.job_id}.json"
            async with aiofiles.open(checkpoint_file, 'w') as f:
                await f.write(json.dumps(self._serialize_job(job), indent=2))
        except Exception as e:
            self.logger.error(f"Failed to save checkpoint for job {job.job_id}: {e}")
            
    def _serialize_job(self, job: BackfillJob) -> Dict[str, Any]:
        """Serialize job to JSON-compatible format"""
        data = asdict(job)
        # Convert datetime objects to ISO format
        for key in ['start_date', 'end_date', 'start_time', 'end_time', 
                   'last_error_time', 'last_checkpoint']:
            if data.get(key):
                data[key] = data[key].isoformat()
        # Convert enums to values
        data['priority'] = job.priority.value
        data['granularity'] = job.granularity.value
        data['status'] = job.status.value
        return data
        
    def _deserialize_job(self, data: Dict[str, Any]) -> BackfillJob:
        """Deserialize job from JSON data"""
        # Convert ISO strings back to datetime
        for key in ['start_date', 'end_date', 'start_time', 'end_time', 
                   'last_error_time', 'last_checkpoint']:
            if data.get(key):
                data[key] = datetime.fromisoformat(data[key])
        # Convert enum values back to enums
        data['priority'] = BackfillPriority(data['priority'])
        data['granularity'] = DataGranularity(data['granularity'])
        data['status'] = BackfillStatus(data['status'])
        return BackfillJob(**data)
        
    def estimate_storage_requirements(
        self, 
        symbols: List[str], 
        years: int = 5,
        granularities: Optional[List[DataGranularity]] = None
    ) -> Dict[str, StorageEstimate]:
        """Estimate storage requirements for backfill"""
        if not granularities:
            # Default backfill strategy
            granularities = [
                (DataGranularity.TICK, 30),      # 1 month of tick data
                (DataGranularity.MINUTE, 180),   # 6 months of minute data
                (DataGranularity.HOUR, 730),     # 2 years of hourly data
                (DataGranularity.DAY, 1825)      # 5 years of daily data
            ]
        
        estimates = {}
        for symbol in symbols:
            time_range_days = years * 365
            data_points = {}
            storage_bytes = {}
            
            for granularity, days in granularities:
                points = int(self.POINTS_PER_DAY[granularity] * days)
                bytes_required = points * self.STORAGE_PER_POINT[granularity]
                data_points[granularity] = points
                storage_bytes[granularity] = bytes_required
            
            total_bytes = sum(storage_bytes.values())
            total_gb = total_bytes / (1024 ** 3)
            
            # Estimate load time based on provider capabilities
            # Assume 1000 points/second for optimized parallel loading
            total_points = sum(data_points.values())
            estimated_hours = (total_points / 1000) / 3600
            
            estimates[symbol] = StorageEstimate(
                symbol=symbol,
                time_range_days=time_range_days,
                data_points=data_points,
                storage_bytes=storage_bytes,
                total_storage_gb=total_gb,
                estimated_load_time_hours=estimated_hours
            )
            
        return estimates
        
    async def validate_data(
        self, 
        data: List[MarketData],
        granularity: DataGranularity
    ) -> DataValidationResult:
        """Comprehensive data validation with quality scoring"""
        if not data:
            return DataValidationResult(
                is_valid=False,
                total_points=0,
                valid_points=0,
                invalid_points=0,
                missing_points=0,
                duplicate_points=0,
                quality_score=0.0,
                issues=["No data provided"]
            )
        
        issues = []
        invalid_points = 0
        duplicate_points = 0
        
        # Sort data by timestamp
        data.sort(key=lambda x: x.time)
        
        # Check for duplicates
        seen_timestamps = set()
        unique_data = []
        for point in data:
            timestamp_key = (point.time, point.symbol)
            if timestamp_key in seen_timestamps:
                duplicate_points += 1
                issues.append(f"Duplicate data point at {point.time}")
            else:
                seen_timestamps.add(timestamp_key)
                unique_data.append(point)
        
        # Validate each data point
        for point in unique_data:
            validation_issues = self._validate_single_point(point)
            if validation_issues:
                invalid_points += 1
                issues.extend(validation_issues)
        
        # Check for missing data gaps
        expected_interval = self._get_expected_interval(granularity)
        missing_points = 0
        
        for i in range(1, len(unique_data)):
            time_diff = (unique_data[i].time - unique_data[i-1].time).total_seconds()
            expected_diff = expected_interval.total_seconds()
            
            # Allow for some tolerance (weekends, holidays)
            if time_diff > expected_diff * 2:
                gaps = int(time_diff / expected_diff) - 1
                missing_points += gaps
                if gaps > 10:  # Only report significant gaps
                    issues.append(
                        f"Gap of {gaps} expected intervals between "
                        f"{unique_data[i-1].time} and {unique_data[i].time}"
                    )
        
        # Calculate quality score
        total_points = len(data)
        valid_points = total_points - invalid_points - duplicate_points
        quality_score = (valid_points / total_points) if total_points > 0 else 0.0
        
        # Adjust quality score based on missing data
        if missing_points > 0:
            completeness_ratio = total_points / (total_points + missing_points)
            quality_score *= completeness_ratio
        
        # Calculate statistics
        statistics = {}
        if valid_points > 0:
            prices = [p.close for p in unique_data if hasattr(p, 'close')]
            volumes = [p.volume for p in unique_data if hasattr(p, 'volume')]
            
            if prices:
                statistics['price_mean'] = np.mean(prices)
                statistics['price_std'] = np.std(prices)
                statistics['price_min'] = np.min(prices)
                statistics['price_max'] = np.max(prices)
                
            if volumes:
                statistics['volume_mean'] = np.mean(volumes)
                statistics['volume_total'] = np.sum(volumes)
        
        return DataValidationResult(
            is_valid=quality_score >= 0.8,  # 80% quality threshold
            total_points=total_points,
            valid_points=valid_points,
            invalid_points=invalid_points,
            missing_points=missing_points,
            duplicate_points=duplicate_points,
            quality_score=quality_score,
            issues=issues[:100],  # Limit issues to first 100
            statistics=statistics
        )
        
    def _validate_single_point(self, point: MarketData) -> List[str]:
        """Validate a single market data point"""
        issues = []
        
        # Check required fields
        if not point.time:
            issues.append("Missing timestamp")
        if not point.symbol:
            issues.append("Missing symbol")
            
        # Validate OHLC data if present
        if hasattr(point, 'open') and hasattr(point, 'close'):
            if point.open <= 0 or point.close <= 0:
                issues.append(f"Invalid price data: open={point.open}, close={point.close}")
                
            if hasattr(point, 'high') and hasattr(point, 'low'):
                # Check OHLC consistency
                if point.high < point.low:
                    issues.append(f"High ({point.high}) is less than Low ({point.low})")
                if point.high < point.open or point.high < point.close:
                    issues.append(f"High ({point.high}) is not the highest price")
                if point.low > point.open or point.low > point.close:
                    issues.append(f"Low ({point.low}) is not the lowest price")
                    
        # Validate volume
        if hasattr(point, 'volume') and point.volume < 0:
            issues.append(f"Negative volume: {point.volume}")
            
        return issues
        
    def _get_expected_interval(self, granularity: DataGranularity) -> timedelta:
        """Get expected time interval for granularity"""
        intervals = {
            DataGranularity.TICK: timedelta(seconds=1),
            DataGranularity.MINUTE: timedelta(minutes=1),
            DataGranularity.MINUTE_5: timedelta(minutes=5),
            DataGranularity.MINUTE_15: timedelta(minutes=15),
            DataGranularity.MINUTE_30: timedelta(minutes=30),
            DataGranularity.HOUR: timedelta(hours=1),
            DataGranularity.HOUR_4: timedelta(hours=4),
            DataGranularity.DAY: timedelta(days=1),
            DataGranularity.WEEK: timedelta(weeks=1),
            DataGranularity.MONTH: timedelta(days=30)  # Approximate
        }
        return intervals.get(granularity, timedelta(days=1))
        
    async def plan_backfill(self, symbols: List[str], years: int = 5) -> List[BackfillJob]:
        """Plan backfill jobs for given symbols"""
        jobs = []
        end_date = datetime.now()
        
        for symbol in symbols:
            # Check existing data coverage
            coverage = await self._check_data_coverage(symbol)
            
            # Plan jobs based on missing data
            if coverage['oldest_date'] is None:
                # No data exists, full backfill needed
                jobs.extend(self._create_backfill_jobs(
                    symbol, 
                    end_date - timedelta(days=years*365),
                    end_date
                ))
            else:
                # Fill gaps
                gaps = coverage['gaps']
                for gap_start, gap_end in gaps:
                    jobs.extend(self._create_backfill_jobs(
                        symbol,
                        gap_start,
                        gap_end
                    ))
                    
        self.jobs = sorted(jobs, key=lambda x: x.priority.value)
        return self.jobs
        
    def _create_backfill_jobs(self, symbol: str, start: datetime, end: datetime) -> List[BackfillJob]:
        """Create backfill jobs for a date range"""
        jobs = []
        
        # Determine time chunks and priorities
        chunks = self._split_date_range(start, end)
        
        for chunk_start, chunk_end, priority in chunks:
            # Yahoo Finance for long historical data (20+ years)
            if (end - start).days > 365:
                jobs.append(BackfillJob(
                    symbol=symbol,
                    provider='yahoo',
                    start_date=chunk_start,
                    end_date=chunk_end,
                    priority=priority,
                    interval='1day'
                ))
                
            # Alpaca for recent high-quality data (5 years)
            if (end - start).days <= 1825:  # 5 years
                jobs.append(BackfillJob(
                    symbol=symbol,
                    provider='alpaca',
                    start_date=chunk_start,
                    end_date=chunk_end,
                    priority=priority,
                    interval='1min' if priority == BackfillPriority.CRITICAL else '1day'
                ))
                
        return jobs
        
    def _split_date_range(self, start: datetime, end: datetime) -> List[tuple]:
        """Split date range into prioritized chunks"""
        chunks = []
        now = datetime.now()
        
        # Critical: Last month
        if end > now - timedelta(days=30):
            chunks.append((
                max(start, now - timedelta(days=30)),
                end,
                BackfillPriority.CRITICAL
            ))
            
        # High: Last year
        if end > now - timedelta(days=365) and start < now - timedelta(days=30):
            chunks.append((
                max(start, now - timedelta(days=365)),
                min(end, now - timedelta(days=30)),
                BackfillPriority.HIGH
            ))
            
        # Medium: Last 5 years
        if end > now - timedelta(days=1825) and start < now - timedelta(days=365):
            chunks.append((
                max(start, now - timedelta(days=1825)),
                min(end, now - timedelta(days=365)),
                BackfillPriority.MEDIUM
            ))
            
        # Low: Beyond 5 years
        if start < now - timedelta(days=1825):
            chunks.append((
                start,
                min(end, now - timedelta(days=1825)),
                BackfillPriority.LOW
            ))
            
        return chunks
        
    async def _check_data_coverage(self, symbol: str) -> Dict[str, Any]:
        """Check existing data coverage for a symbol"""
        # Query TimescaleDB for coverage info
        coverage_query = """
        SELECT 
            MIN(time) as oldest_date,
            MAX(time) as newest_date,
            COUNT(*) as total_points,
            COUNT(DISTINCT DATE(time)) as days_covered
        FROM market_data
        WHERE symbol = %s
        """
        
        # Find gaps in data
        gap_query = """
        WITH date_series AS (
            SELECT generate_series(
                MIN(DATE(time)),
                MAX(DATE(time)),
                '1 day'::interval
            )::date as date
            FROM market_data
            WHERE symbol = %s
        ),
        existing_dates AS (
            SELECT DISTINCT DATE(time) as date
            FROM market_data
            WHERE symbol = %s
        )
        SELECT 
            date as gap_date
        FROM date_series
        WHERE date NOT IN (SELECT date FROM existing_dates)
        ORDER BY date
        """
        
        # Execute queries and return coverage info
        # ... implementation ...
        
        return {
            'oldest_date': None,  # Placeholder
            'newest_date': None,
            'total_points': 0,
            'days_covered': 0,
            'gaps': []
        }
        
    async def execute_backfill(self, max_concurrent: int = 3):
        """Execute backfill jobs with concurrency control"""
        self.logger.info(f"Starting backfill of {len(self.jobs)} jobs")
        
        # Group jobs by priority
        priority_groups = {}
        for job in self.jobs:
            if job.priority not in priority_groups:
                priority_groups[job.priority] = []
            priority_groups[job.priority].append(job)
            
        # Execute by priority
        for priority in sorted(priority_groups.keys(), key=lambda x: x.value):
            jobs = priority_groups[priority]
            self.logger.info(f"Processing {len(jobs)} {priority.name} priority jobs")
            
            # Process in batches
            for i in range(0, len(jobs), max_concurrent):
                batch = jobs[i:i + max_concurrent]
                await asyncio.gather(*[
                    self._execute_single_job(job) for job in batch
                ])
                
    async def _execute_single_job(self, job: BackfillJob):
        """Execute a single backfill job"""
        try:
            job.status = "running"
            provider = self.providers[job.provider]
            
            self.logger.info(
                f"Starting backfill: {job.symbol} from {job.start_date} to {job.end_date} "
                f"using {job.provider}"
            )
            
            # Track metrics
            metrics.backfill_jobs_started.labels(
                provider=job.provider,
                priority=job.priority.name
            ).inc()
            
            start_time = asyncio.get_event_loop().time()
            data_count = 0
            
            # Fetch data
            async for data_point in provider.get_market_data(
                symbols=[job.symbol],
                start_time=job.start_date,
                end_time=job.end_date,
                interval=job.interval
            ):
                # Store in database
                await self.storage.store_market_data(data_point)
                data_count += 1
                
                # Update progress
                if data_count % 1000 == 0:
                    elapsed = (job.end_date - job.start_date).total_seconds()
                    progress = (data_point.time - job.start_date).total_seconds() / elapsed
                    job.progress = min(progress * 100, 100)
                    
            # Mark complete
            job.status = "completed"
            job.progress = 100.0
            
            duration = asyncio.get_event_loop().time() - start_time
            self.logger.info(
                f"Completed backfill: {job.symbol} - "
                f"{data_count} data points in {duration:.2f}s"
            )
            
            metrics.backfill_jobs_completed.labels(
                provider=job.provider,
                priority=job.priority.name
            ).inc()
            
            metrics.backfill_data_points.labels(
                provider=job.provider
            ).inc(data_count)
            
        except Exception as e:
            job.status = "failed"
            job.error = str(e)
            
            self.logger.error(
                f"Backfill failed for {job.symbol}: {e}"
            )
            
            metrics.backfill_jobs_failed.labels(
                provider=job.provider,
                priority=job.priority.name,
                error_type=type(e).__name__
            ).inc()
            
    async def get_backfill_status(self) -> Dict[str, Any]:
        """Get current backfill status"""
        total = len(self.jobs)
        completed = sum(1 for j in self.jobs if j.status == "completed")
        failed = sum(1 for j in self.jobs if j.status == "failed")
        running = sum(1 for j in self.jobs if j.status == "running")
        pending = sum(1 for j in self.jobs if j.status == "pending")
        
        avg_progress = sum(j.progress for j in self.jobs) / total if total > 0 else 0
        
        return {
            'total_jobs': total,
            'completed': completed,
            'failed': failed,
            'running': running,
            'pending': pending,
            'average_progress': avg_progress,
            'jobs_by_provider': self._group_jobs_by_provider(),
            'jobs_by_priority': self._group_jobs_by_priority()
        }
        
    def _group_jobs_by_provider(self) -> Dict[str, int]:
        """Group jobs by provider"""
        groups = {}
        for job in self.jobs:
            if job.provider not in groups:
                groups[job.provider] = 0
            groups[job.provider] += 1
        return groups
        
    def _group_jobs_by_priority(self) -> Dict[str, int]:
        """Group jobs by priority"""
        groups = {}
        for job in self.jobs:
            priority_name = job.priority.name
            if priority_name not in groups:
                groups[priority_name] = 0
            groups[priority_name] += 1
        return groups


# CLI interface for running backfill
async def main():
    """Main entry point for historical backfill"""
    import argparse
    
    parser = argparse.ArgumentParser(description='Historical data backfill')
    parser.add_argument('--symbols', nargs='+', default=['AAPL', 'GOOGL', 'MSFT', 'TSLA', 'NVDA'])
    parser.add_argument('--years', type=int, default=5)
    parser.add_argument('--concurrent', type=int, default=3)
    
    args = parser.parse_args()
    
    coordinator = HistoricalBackfillCoordinator()
    
    # Plan backfill
    jobs = await coordinator.plan_backfill(args.symbols, args.years)
    print(f"Planned {len(jobs)} backfill jobs")
    
    # Execute backfill
    await coordinator.execute_backfill(max_concurrent=args.concurrent)
    
    # Show final status
    status = await coordinator.get_backfill_status()
    print(f"\nBackfill complete:")
    print(f"  Total: {status['total_jobs']}")
    print(f"  Completed: {status['completed']}")
    print(f"  Failed: {status['failed']}")


if __name__ == "__main__":
    asyncio.run(main())