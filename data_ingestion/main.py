"""Main entry point for the data ingestion service."""
import asyncio
import signal
import sys
from typing import Optional, List
import click

from config import get_settings
from utils.logging import get_logger, setup_logging
from utils.metrics import start_metrics_server
from schedulers import RealtimeCoordinator, BatchScheduler, StreamManager
from providers import PROVIDERS


# Setup logging
setup_logging()
logger = get_logger(__name__)


class DataIngestionService:
    """Main data ingestion service orchestrator."""
    
    def __init__(self):
        self.settings = get_settings()
        self.realtime_coordinator = RealtimeCoordinator()
        self.batch_scheduler = BatchScheduler()
        self.stream_manager = StreamManager()
        self._shutdown_event = asyncio.Event()
    
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
        
        # Start metrics server
        if self.settings.prometheus_enabled:
            start_metrics_server(self.settings.prometheus_port)
        
        # Initialize components
        await self.realtime_coordinator.initialize(providers)
        await self.batch_scheduler.initialize(providers)
        await self.stream_manager.start()
        
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
                    'iex_cloud': 2,
                    'finnhub': 3,
                    'alpha_vantage': 4,
                    'yahoo_finance': 5
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
        
        # Stop components
        await self.realtime_coordinator.stop()
        await self.batch_scheduler.cleanup()
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
    default=['yahoo_finance', 'finnhub'],
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
    click.echo("Available data providers:")
    for name, provider_class in PROVIDERS.items():
        click.echo(f"  - {name}: {provider_class.__doc__.strip()}")


if __name__ == "__main__":
    cli()