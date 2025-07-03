"""Batch data processing scheduler."""
import asyncio
from typing import List, Dict, Any, Optional, Callable
from datetime import datetime, timedelta
import aiocron
from croniter import croniter

from ..providers import PROVIDERS, BaseProvider
from ..processors import DataValidator, DataCleaner, DataTransformer, DataAggregator
from ..storage import TimescaleDB, RedisStore
from ..config import get_settings
from ..utils.logging import get_logger
from ..utils.metrics import metrics


logger = get_logger(__name__)


class BatchScheduler:
    """Schedule and execute batch data ingestion jobs."""
    
    def __init__(self):
        self.settings = get_settings()
        self.logger = logger
        self.providers: Dict[str, BaseProvider] = {}
        self.scheduled_jobs: Dict[str, aiocron.Cron] = {}
        
        # Data processors
        self.validator = DataValidator()
        self.cleaner = DataCleaner()
        self.transformer = DataTransformer()
        self.aggregator = DataAggregator()
        
        # Storage backends
        self.timescale = TimescaleDB()
        self.redis = RedisStore()
        
        # Job callbacks
        self.job_callbacks: List[Callable] = []
        
        self._running = False
    
    async def initialize(self, provider_names: Optional[List[str]] = None):
        """Initialize scheduler with specified providers."""
        # Connect to storage
        await self.timescale.connect()
        await self.redis.connect()
        
        # Initialize providers
        if provider_names is None:
            provider_names = list(PROVIDERS.keys())
        
        for name in provider_names:
            if name in PROVIDERS:
                try:
                    provider_class = PROVIDERS[name]
                    provider = provider_class()
                    await provider.connect()
                    self.providers[name] = provider
                    self.logger.info(f"Initialized provider: {name}")
                except Exception as e:
                    self.logger.error(f"Failed to initialize provider {name}: {e}")
    
    async def schedule_job(
        self,
        job_id: str,
        cron_expression: str,
        job_config: Dict[str, Any]
    ):
        """Schedule a batch job with cron expression."""
        # Validate cron expression
        if not croniter.is_valid(cron_expression):
            raise ValueError(f"Invalid cron expression: {cron_expression}")
        
        # Cancel existing job if any
        if job_id in self.scheduled_jobs:
            self.scheduled_jobs[job_id].stop()
        
        # Create new job
        job = aiocron.crontab(
            cron_expression,
            func=self._execute_job,
            args=[job_id, job_config],
            start=True
        )
        
        self.scheduled_jobs[job_id] = job
        self.logger.info(f"Scheduled job {job_id} with cron: {cron_expression}")
    
    async def _execute_job(self, job_id: str, config: Dict[str, Any]):
        """Execute a scheduled batch job."""
        start_time = datetime.now()
        self.logger.info(f"Starting batch job: {job_id}")
        
        try:
            # Extract job parameters
            symbols = config.get('symbols', [])
            providers = config.get('providers', list(self.providers.keys()))
            lookback_days = config.get('lookback_days', 1)
            interval = config.get('interval', '1day')
            aggregate_method = config.get('aggregate_method', 'consensus')
            
            # Calculate time range
            end_time = datetime.now()
            start_time = end_time - timedelta(days=lookback_days)
            
            # Fetch data from each provider
            provider_data = {}
            
            for provider_name in providers:
                if provider_name not in self.providers:
                    continue
                
                try:
                    provider = self.providers[provider_name]
                    data = []
                    
                    async for market_data in provider.get_market_data(
                        symbols, start_time, end_time, interval
                    ):
                        data.append(market_data.__dict__ if hasattr(market_data, '__dict__') else market_data)
                    
                    if data:
                        # Clean and validate
                        cleaned = self.cleaner.clean_market_data(data)
                        valid, invalid = self.validator.validate_batch(cleaned)
                        
                        if valid:
                            provider_data[provider_name] = pd.DataFrame(valid)
                            self.logger.info(
                                f"Fetched {len(valid)} records from {provider_name}"
                            )
                        
                        if invalid:
                            self.logger.warning(
                                f"Rejected {len(invalid)} invalid records from {provider_name}"
                            )
                            
                except Exception as e:
                    self.logger.error(f"Failed to fetch from {provider_name}: {e}")
                    metrics.batch_job_errors.labels(job_id=job_id, provider=provider_name).inc()
            
            # Aggregate data if multiple providers
            if len(provider_data) > 1:
                aggregated = self.aggregator.merge_market_data(
                    provider_data,
                    method=aggregate_method
                )
                
                # Add technical indicators
                aggregated = self.transformer.add_technical_indicators(aggregated)
                
                # Store aggregated data
                records = self.transformer.prepare_for_storage(aggregated)
                await self.timescale.insert_market_data(records)
                
                self.logger.info(f"Stored {len(records)} aggregated records")
            
            elif len(provider_data) == 1:
                # Single provider, store directly
                provider_name, df = list(provider_data.items())[0]
                
                # Add technical indicators
                df = self.transformer.add_technical_indicators(df)
                
                records = self.transformer.prepare_for_storage(df)
                await self.timescale.insert_market_data(records)
                
                self.logger.info(f"Stored {len(records)} records from {provider_name}")
            
            else:
                self.logger.warning(f"No data fetched for job {job_id}")
            
            # Update job metrics
            duration = (datetime.now() - start_time).total_seconds()
            metrics.batch_job_duration.labels(job_id=job_id).observe(duration)
            metrics.batch_job_success.labels(job_id=job_id).inc()
            
            # Notify callbacks
            for callback in self.job_callbacks:
                try:
                    await callback(job_id, 'success', {
                        'duration': duration,
                        'records_processed': sum(len(df) for df in provider_data.values())
                    })
                except Exception as e:
                    self.logger.error(f"Callback error: {e}")
                    
        except Exception as e:
            self.logger.error(f"Batch job {job_id} failed: {e}")
            metrics.batch_job_errors.labels(job_id=job_id, provider='all').inc()
            
            # Notify callbacks
            for callback in self.job_callbacks:
                try:
                    await callback(job_id, 'error', {'error': str(e)})
                except:
                    pass
    
    async def run_job_once(self, job_id: str, config: Dict[str, Any]):
        """Run a job immediately without scheduling."""
        await self._execute_job(job_id, config)
    
    def cancel_job(self, job_id: str):
        """Cancel a scheduled job."""
        if job_id in self.scheduled_jobs:
            self.scheduled_jobs[job_id].stop()
            del self.scheduled_jobs[job_id]
            self.logger.info(f"Cancelled job: {job_id}")
    
    def list_jobs(self) -> List[Dict[str, Any]]:
        """List all scheduled jobs."""
        jobs = []
        for job_id, cron in self.scheduled_jobs.items():
            jobs.append({
                'job_id': job_id,
                'cron': str(cron),
                'next_run': cron.next() if hasattr(cron, 'next') else None
            })
        return jobs
    
    async def backfill_data(
        self,
        symbols: List[str],
        start_date: datetime,
        end_date: datetime,
        providers: Optional[List[str]] = None,
        interval: str = '1day',
        batch_size: int = 100
    ):
        """Backfill historical data for symbols."""
        if providers is None:
            providers = list(self.providers.keys())
        
        self.logger.info(
            f"Starting backfill for {len(symbols)} symbols "
            f"from {start_date} to {end_date}"
        )
        
        # Process in batches to avoid overwhelming providers
        for i in range(0, len(symbols), batch_size):
            batch_symbols = symbols[i:i + batch_size]
            
            config = {
                'symbols': batch_symbols,
                'providers': providers,
                'interval': interval,
                'aggregate_method': 'consensus'
            }
            
            # Override time range for backfill
            for provider_name in providers:
                if provider_name not in self.providers:
                    continue
                
                provider = self.providers[provider_name]
                data = []
                
                try:
                    async for market_data in provider.get_market_data(
                        batch_symbols, start_date, end_date, interval
                    ):
                        data.append(
                            market_data.__dict__ if hasattr(market_data, '__dict__')
                            else market_data
                        )
                    
                    if data:
                        # Process and store
                        cleaned = self.cleaner.clean_market_data(data)
                        valid, _ = self.validator.validate_batch(cleaned)
                        
                        if valid:
                            await self.timescale.insert_market_data(valid)
                            self.logger.info(
                                f"Backfilled {len(valid)} records "
                                f"for batch {i//batch_size + 1}"
                            )
                            
                except Exception as e:
                    self.logger.error(
                        f"Backfill error for {provider_name} "
                        f"batch {i//batch_size + 1}: {e}"
                    )
            
            # Rate limit between batches
            await asyncio.sleep(5)
        
        self.logger.info("Backfill completed")
    
    def add_job_callback(self, callback: Callable):
        """Add callback for job completion."""
        self.job_callbacks.append(callback)
    
    async def cleanup(self):
        """Clean up resources."""
        # Cancel all jobs
        for job_id in list(self.scheduled_jobs.keys()):
            self.cancel_job(job_id)
        
        # Disconnect providers
        for provider in self.providers.values():
            await provider.disconnect()
        
        # Disconnect storage
        await self.timescale.disconnect()
        await self.redis.disconnect()
        
        self.logger.info("Batch scheduler cleaned up")


# Import pandas here to avoid circular imports
import pandas as pd