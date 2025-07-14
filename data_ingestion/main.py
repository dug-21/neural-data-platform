"""Main entry point for the data ingestion service."""
import asyncio
import signal
import sys
from typing import Optional, List
import click

from config import get_settings
from utils.logging import get_logger, setup_logging
from utils.metrics import start_metrics_server
from utils.metrics_integration import metrics_collector

# Lazy imports to avoid import deadlock
# These modules will be imported only when needed
RealtimeCoordinator = None
BatchScheduler = None
StreamManager = None
PROVIDERS = None


# Defer logging setup to avoid blocking CLI output
# setup_logging() will be called when needed
logger = None


class DataIngestionService:
    """Main data ingestion service orchestrator."""
    
    def __init__(self):
        # Initialize logging if not already done
        global logger
        if logger is None:
            setup_logging()
            logger = get_logger(__name__)
        
        self.settings = get_settings()
        # Lazy initialization - these will be created when needed
        self.realtime_coordinator = None
        self.batch_scheduler = None
        self.stream_manager = None
        self._shutdown_event = asyncio.Event()
        self._initialized = False
    
    async def _lazy_initialize(self):
        """Lazy initialization of components to avoid import deadlock."""
        if self._initialized:
            return
        
        # Import modules only when needed
        global RealtimeCoordinator, BatchScheduler, StreamManager, PROVIDERS
        
        if RealtimeCoordinator is None:
            from schedulers import RealtimeCoordinator as RC
            RealtimeCoordinator = RC
        
        if BatchScheduler is None:
            from schedulers import BatchScheduler as BS
            BatchScheduler = BS
        
        if StreamManager is None:
            from schedulers import StreamManager as SM
            StreamManager = SM
        
        if PROVIDERS is None:
            from providers import PROVIDERS as P
            PROVIDERS = P
        
        # Initialize components
        self.realtime_coordinator = RealtimeCoordinator()
        self.batch_scheduler = BatchScheduler()
        self.stream_manager = StreamManager()
        
        self._initialized = True
        logger.info("Lazy initialization of components completed")
    
    async def start(
        self,
        providers: List[str],
        symbols: List[str],
        enable_realtime: bool = True,
        enable_batch: bool = True
    ):
        """Start the data ingestion service."""
        logger.info(
            f"Starting data ingestion service with providers: {providers}, "
            f"symbols: {symbols}"
        )
        
        # Start metrics server and collection
        if self.settings.prometheus_enabled:
            start_metrics_server(self.settings.prometheus_port)
            metrics_collector.start_collection()
            logger.info("Started clean metrics collection")
        
        # Initialize components with lazy loading
        await self._lazy_initialize()
        
        # Initialize coordinators with retry logic
        max_init_retries = 5
        for attempt in range(max_init_retries):
            try:
                await self.realtime_coordinator.initialize(providers)
                await self.batch_scheduler.initialize(providers)
                await self.stream_manager.start()
                logger.info("All components initialized successfully")
                break
            except Exception as e:
                logger.error(f"Initialization attempt {attempt + 1}/{max_init_retries} failed: {e}")
                if attempt == max_init_retries - 1:
                    logger.error("Max initialization attempts reached, service will exit")
                    raise
                else:
                    logger.info(f"Retrying initialization in 10 seconds...")
                    await asyncio.sleep(10)
        
        # Setup signal handlers
        def signal_handler(sig, frame):
            logger.info(f"Received signal {sig}, shutting down...")
            asyncio.create_task(self.shutdown())
        
        signal.signal(signal.SIGINT, signal_handler)
        signal.signal(signal.SIGTERM, signal_handler)
        
        # Start real-time streaming
        if enable_realtime:
            await self._start_realtime_streams(providers, symbols)
        
        # Schedule batch jobs
        if enable_batch:
            await self._schedule_batch_jobs(symbols)
        
        # Wait for shutdown
        await self._shutdown_event.wait()
    
    async def _start_realtime_streams(self, providers: List[str], symbols: List[str]):
        """Start real-time data streams."""
        logger.info("Starting real-time streams")
        
        # Subscribe to symbols
        await self.realtime_coordinator.subscribe(symbols)
        
        # Start coordinator
        asyncio.create_task(self.realtime_coordinator.start())
        
        # Register streams with stream manager
        for provider_name in providers:
            if provider_name in self.realtime_coordinator.providers:
                provider = self.realtime_coordinator.providers[provider_name]
                
                # Determine priority based on provider
                priority_map = {
                    'polygon': 1,
                    'alpaca': 2,
                    'iex_cloud': 3,
                    'finnhub': 4,
                    'alpha_vantage': 5,
                }
                priority = priority_map.get(provider_name, 10)
                
                stream_id = f"realtime_{provider_name}"
                await self.stream_manager.register_stream(
                    stream_id,
                    provider,
                    symbols,
                    'market_data',
                    priority
                )
    
    async def _schedule_batch_jobs(self, symbols: List[str]):
        """Schedule batch data collection jobs."""
        logger.info("Scheduling batch jobs")
        
        # Daily historical data update (run at 6 AM)
        await self.batch_scheduler.schedule_job(
            'daily_historical',
            '0 6 * * *',  # 6 AM daily
            {
                'symbols': symbols,
                'lookback_days': 2,  # Get last 2 days
                'interval': '1day',
                'aggregate_method': 'consensus'
            }
        )
        
        # Hourly intraday data update
        await self.batch_scheduler.schedule_job(
            'hourly_intraday',
            '5 * * * *',  # 5 minutes past every hour
            {
                'symbols': symbols,
                'lookback_days': 1,
                'interval': '1hour',
                'aggregate_method': 'priority'
            }
        )
        
        # Technical indicators calculation (every 15 minutes)
        await self.batch_scheduler.schedule_job(
            'technical_indicators',
            '*/15 * * * *',  # Every 15 minutes
            {
                'symbols': symbols,
                'lookback_days': 30,  # 30 days for indicators
                'interval': '1day',
                'aggregate_method': 'average'
            }
        )
    
    async def shutdown(self):
        """Gracefully shutdown the service."""
        logger.info("Shutting down data ingestion service")
        
        # Stop components if they were initialized
        if self.realtime_coordinator:
            await self.realtime_coordinator.stop()
        if self.batch_scheduler:
            await self.batch_scheduler.cleanup()
        if self.stream_manager:
            await self.stream_manager.stop()
        
        # Signal shutdown complete
        self._shutdown_event.set()
    
    async def backfill(
        self,
        symbols: List[str],
        start_date: str,
        end_date: str,
        providers: Optional[List[str]] = None
    ):
        """Run historical data backfill."""
        from datetime import datetime
        
        start = datetime.fromisoformat(start_date)
        end = datetime.fromisoformat(end_date)
        
        logger.info(
            f"Starting backfill for {len(symbols)} symbols "
            f"from {start_date} to {end_date}"
        )
        
        await self._lazy_initialize()
        await self.batch_scheduler.initialize(providers)
        await self.batch_scheduler.backfill_data(
            symbols, start, end, providers
        )
        await self.batch_scheduler.cleanup()


@click.group()
def cli():
    """Data ingestion service CLI."""
    pass


@cli.command()
@click.option(
    '--providers',
    '-p',
    multiple=True,
    default=['alpaca'],
    help='Data providers to use'
)
@click.option(
    '--symbols',
    '-s',
    multiple=True,
    required=True,
    help='Symbols to track'
)
@click.option(
    '--realtime/--no-realtime',
    default=True,
    help='Enable real-time streaming'
)
@click.option(
    '--batch/--no-batch',
    default=True,
    help='Enable batch processing'
)
def start(providers, symbols, realtime, batch):
    """Start the data ingestion service."""
    service = DataIngestionService()
    
    asyncio.run(
        service.start(
            list(providers),
            list(symbols),
            enable_realtime=realtime,
            enable_batch=batch
        )
    )


@cli.command()
@click.option(
    '--symbols',
    '-s',
    multiple=True,
    required=True,
    help='Symbols to backfill'
)
@click.option(
    '--start-date',
    required=True,
    help='Start date (YYYY-MM-DD)'
)
@click.option(
    '--end-date',
    required=True,
    help='End date (YYYY-MM-DD)'
)
@click.option(
    '--providers',
    '-p',
    multiple=True,
    help='Data providers to use'
)
def backfill(symbols, start_date, end_date, providers):
    """Backfill historical data."""
    service = DataIngestionService()
    
    asyncio.run(
        service.backfill(
            list(symbols),
            start_date,
            end_date,
            list(providers) if providers else None
        )
    )


@cli.command()
def list_providers():
    """List available data providers."""
    # Don't setup logging for simple list command
    # Import providers only when needed
    from providers import PROVIDERS as P
    
    click.echo("Available data providers:")
    for name, provider_class in P.items():
        doc = provider_class.__doc__ or "No description available"
        click.echo(f"  - {name}: {doc.strip()}")


if __name__ == "__main__":
    cli()