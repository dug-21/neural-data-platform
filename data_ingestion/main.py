"""Main entry point for the data ingestion service."""
import asyncio
import signal
import sys
import os
from typing import Optional, List
import click

from config import get_settings
from utils.logging import get_logger, setup_logging
from utils.metrics import start_metrics_server
from utils.metrics_integration import metrics_collector
from utils.health_check import HealthCheckHandler
from utils.health_tracker import health_tracker

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
        self.health_check_handler = HealthCheckHandler()
    
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
        
        # Enhanced provider configuration with circuit breakers
        self._setup_provider_enhancements()
        
        self._initialized = True
        logger.info("Lazy initialization of components completed")
    
    def _setup_provider_enhancements(self):
        """Setup enhanced features for providers including circuit breakers."""
        # Import circuit breaker
        from utils.circuit_breaker import CircuitBreaker, CircuitBreakerConfig
        
        # Default circuit breaker config for providers
        default_cb_config = CircuitBreakerConfig(
            failure_threshold=5,
            timeout=60.0,
            success_threshold=2
        )
        
        # Provider-specific configurations
        provider_configs = {
            'alpaca': {
                'circuit_breaker_config': CircuitBreakerConfig(
                    failure_threshold=3,  # More sensitive for real-time
                    timeout=30.0,
                    success_threshold=2
                ),
                'reconnect_attempts': 100,
                'buffer_size': 10000
            },
            'polygon': {
                'circuit_breaker_config': default_cb_config,
                'reconnect_attempts': 50,
                'buffer_size': 5000
            },
            'file_provider': {
                'circuit_breaker_config': CircuitBreakerConfig(
                    failure_threshold=10,  # More tolerant for files
                    timeout=10.0,
                    success_threshold=1
                ),
                'batch_size': 1000
            }
        }
        
        # Store configurations for providers to use
        self.provider_configs = provider_configs
        logger.info("Provider enhancement configurations setup complete")
    
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
        try:
            # Always try to start metrics server for Phase 4 integration
            metrics_port = 9090  # Default Prometheus metrics port
            if self.settings and hasattr(self.settings, 'prometheus_port'):
                metrics_port = self.settings.prometheus_port
            
            # Start metrics server (metrics endpoint is served by health check server)
            logger.info(f"Metrics will be available at health check server on /metrics endpoint")
            
            # Start metrics collector if available
            try:
                metrics_collector.start_collection()
                logger.info("Started metrics collection")
            except Exception:
                pass  # Metrics collector might not be available in all environments
        except Exception as e:
            logger.warning(f"Metrics initialization warning: {e}")
        
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
        
        # Set component references for health checks
        self.health_check_handler.realtime_coordinator = self.realtime_coordinator
        self.health_check_handler.stream_manager = self.stream_manager
        self.health_check_handler.batch_scheduler = self.batch_scheduler
        self.health_check_handler.timescale_db = self.realtime_coordinator.timescale if self.realtime_coordinator else None
        self.health_check_handler.redis_store = self.realtime_coordinator.redis if self.realtime_coordinator else None
        
        # Register health handler with tracker
        health_tracker.set_handler(self.health_check_handler)
        
        # Start health check server
        health_port = 8001  # Default port matching Dockerfile EXPOSE
        try:
            # Try to get port from settings if available
            if self.settings and hasattr(self.settings, 'health_check_port'):
                health_port = self.settings.health_check_port
        except Exception:
            pass  # Use default port
        
        await self.health_check_handler.start(port=health_port)
        logger.info(f"Health check server started on port {health_port}")
        
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
        
        # Stop health check server
        if self.health_check_handler:
            await self.health_check_handler.stop()
        
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
    default=None,
    help='Data providers to use (defaults to environment config)'
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
    # Initialize logging first
    global logger
    if logger is None:
        setup_logging()
        logger = get_logger(__name__)
    
    service = DataIngestionService()
    
    # Debug logging
    logger.info(f"CLI providers argument: {providers}")
    logger.info(f"Environment PRIMARY_PROVIDER: {os.environ.get('PRIMARY_PROVIDER', 'NOT SET')}")
    logger.info(f"Environment DEFAULT_PROVIDER: {os.environ.get('DEFAULT_PROVIDER', 'NOT SET')}")
    
    # If no providers specified, check environment variables
    if not providers:
        settings = get_settings()
        # PRIMARY_PROVIDER takes precedence, then DEFAULT_PROVIDER, then hardcoded default
        primary_provider = getattr(settings, 'primary_provider', None)
        default_provider = getattr(settings, 'default_provider', None)
        
        logger.info(f"Settings primary_provider: {primary_provider}")
        logger.info(f"Settings default_provider: {default_provider}")
        
        if primary_provider:
            providers = [primary_provider]
            logger.info(f"Using PRIMARY_PROVIDER from environment: {primary_provider}")
        elif default_provider:
            providers = [default_provider]
            logger.info(f"Using DEFAULT_PROVIDER from environment: {default_provider}")
        else:
            # Check for active providers list
            active_providers = getattr(settings, 'active_providers', None)
            if active_providers and isinstance(active_providers, list):
                providers = active_providers
                logger.info(f"Using ACTIVE_PROVIDERS from environment: {active_providers}")
            else:
                providers = ['alpaca']  # Hardcoded fallback
                logger.info("No provider configuration found, defaulting to: alpaca")
    
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
@click.option(
    '--path',
    '-p',
    required=True,
    type=click.Path(exists=True, readable=True),
    help='Path to the mounted file or directory containing historical data'
)
@click.option(
    '--symbols',
    '-s',
    multiple=True,
    help='Symbols to filter (if not specified, processes all symbols in file)'
)
@click.option(
    '--start-date',
    help='Start date for filtering (YYYY-MM-DD)'
)
@click.option(
    '--end-date',
    help='End date for filtering (YYYY-MM-DD)'
)
@click.option(
    '--format',
    type=click.Choice(['csv', 'json', 'parquet'], case_sensitive=False),
    default='csv',
    help='File format (default: csv)'
)
@click.option(
    '--checkpoint/--no-checkpoint',
    default=True,
    help='Use checkpoint system to track progress (default: enabled)'
)
@click.option(
    '--batch-size',
    type=int,
    default=1000,
    help='Number of records to process in each batch (default: 1000)'
)
@click.option(
    '--dry-run',
    is_flag=True,
    help='Preview what would be processed without writing to storage'
)
def backfill_file(path, symbols, start_date, end_date, format, checkpoint, batch_size, dry_run):
    """Backfill historical data from a mounted file or directory."""
    # Initialize logging
    global logger
    if logger is None:
        setup_logging()
        logger = get_logger(__name__)
    
    from pathlib import Path
    from datetime import datetime, timezone
    
    # Convert path to Path object
    data_path = Path(path)
    
    # Log operation details
    logger.info(f"Starting file-based backfill from: {data_path}")
    logger.info(f"Format: {format}, Batch size: {batch_size}, Checkpoint: {checkpoint}")
    if symbols:
        symbols_list = list(symbols)
        logger.info(f"Filtering symbols: {symbols_list} (type: {type(symbols)}, first symbol: '{symbols_list[0]}', length: {len(symbols_list[0])})")
    if start_date:
        logger.info(f"Start date filter: {start_date}")
    if end_date:
        logger.info(f"End date filter: {end_date}")
    if dry_run:
        logger.info("DRY RUN MODE - No data will be written")
    
    # Initialize service and run backfill
    service = DataIngestionService()
    
    async def run_file_backfill():
        """Run the file-based backfill process using the new FileProvider."""
        await service._lazy_initialize()
        
        # Initialize storage connections directly for backfill
        if service.realtime_coordinator:
            # Initialize TimescaleDB and Redis directly without providers
            from storage.timescale import TimescaleDB
            from storage.redis_store import RedisStore
            
            service.realtime_coordinator.timescale = TimescaleDB()
            service.realtime_coordinator.redis = RedisStore()
            
            await service.realtime_coordinator.timescale.connect()
            await service.realtime_coordinator.redis.connect()
            
            logger.info("Storage connections initialized for backfill")
        
        # Create FileProvider with configuration
        from providers.file_provider import FileProvider
        
        file_provider = FileProvider({
            'batch_size': batch_size,
            'use_checkpoint': checkpoint
        })
        
        # Connect the provider
        await file_provider.connect()
        
        try:
            # Process each file in the path
            files_to_process = []
            if data_path.is_file():
                files_to_process.append(data_path)
            else:
                # Get all files with the specified format, including compressed files
                if format == 'csv':
                    patterns = ["*.csv", "*.csv.gz"]
                else:
                    patterns = [f"*.{format}"]
                
                # Use rglob for recursive search
                for pattern in patterns:
                    files_to_process.extend(data_path.rglob(pattern))
                
                # Filter out system files and hidden files
                files_to_process = [
                    f for f in files_to_process 
                    if not f.name.startswith('.') and not f.name.startswith('._')
                ]
            
            total_processed = 0
            total_symbol_matches = 0
            
            logger.info(f"Starting backfill for {len(files_to_process)} files")
            
            for file_idx, file_path in enumerate(files_to_process, 1):
                file_matches = 0
                logger.info(f"[{file_idx}/{len(files_to_process)}] Starting file: {file_path.name}")
                
                # Get symbol from filename or use provided symbol
                file_symbol = None
                if symbols:
                    file_symbol = list(symbols)[0]  # Use first symbol if provided
                else:
                    # Try to extract symbol from filename (handle compressed files)
                    if file_path.suffix == '.gz' and file_path.stem.endswith('.csv'):
                        # For .csv.gz files, get the stem of the stem to get actual symbol
                        file_symbol = Path(file_path.stem).stem.upper()
                    else:
                        file_symbol = file_path.stem.upper()
                
                # Stream data from file
                first_record = True
                async for market_data in file_provider.load_from_file(
                    filepath=str(file_path),
                    format=format,
                    symbol=file_symbol,
                    data_type='market_data'
                ):
                    # Count total rows
                    total_processed += 1
                    
                    # Debug: Log first record from each file
                    if first_record:
                        logger.info(f"First record in file - Symbol: '{market_data.symbol}' Time: {market_data.time}")
                        first_record = False
                    
                    # Debug AAPL when found
                    if market_data.symbol == 'AAPL' and file_matches == 0:
                        logger.info(f"Found first AAPL! Time: {market_data.time}, Price: {market_data.close}")
                    # Apply date filters if provided (make both timezone-aware)
                    if start_date:
                        start_dt = datetime.fromisoformat(start_date)
                        if start_dt.tzinfo is None:
                            start_dt = start_dt.replace(tzinfo=timezone.utc)
                        if market_data.time.tzinfo is None:
                            market_data_time = market_data.time.replace(tzinfo=timezone.utc)
                        else:
                            market_data_time = market_data.time
                        if market_data_time < start_dt:
                            continue
                    
                    if end_date:
                        end_dt = datetime.fromisoformat(end_date)
                        if end_dt.tzinfo is None:
                            end_dt = end_dt.replace(tzinfo=timezone.utc)
                        if market_data.time.tzinfo is None:
                            market_data_time = market_data.time.replace(tzinfo=timezone.utc)
                        else:
                            market_data_time = market_data.time
                        if market_data_time > end_dt:
                            continue
                    
                    # Apply symbol filter if provided
                    if symbols and market_data.symbol not in symbols:
                        continue
                    
                    # Found a matching symbol
                    total_symbol_matches += 1
                    file_matches += 1
                    if file_matches == 1:
                        logger.info(f"Found first {market_data.symbol} record in {file_path.name}")
                    
                    if not dry_run:
                        # Store data using the service's storage components
                        if service.realtime_coordinator:
                            # Use the timescale and redis from realtime coordinator
                            await service.realtime_coordinator._process_market_data(market_data, 'file_provider')
                            
                            # Update health check timestamp
                            if service.health_check_handler:
                                service.health_check_handler.update_data_timestamp(
                                    'file_provider',
                                    market_data.symbol
                                )
                        else:
                            logger.warning("Realtime coordinator not initialized - data not stored!")
                
                # Log file completion
                logger.info(f"[{file_idx}/{len(files_to_process)}] Completed: {file_path.name} - Found {file_matches} {list(symbols)[0] if symbols else 'symbol'} records")
            
            logger.info(f"Backfill completed. Total {list(symbols)[0] if symbols else 'symbol'} records found: {total_symbol_matches}")
            
        finally:
            await file_provider.disconnect()
    
    # Run the async function
    asyncio.run(run_file_backfill())


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