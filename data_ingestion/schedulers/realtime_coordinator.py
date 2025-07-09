"""Real-time data streaming coordinator."""
import asyncio
from typing import List, Dict, Any, Set, Optional, Callable
from datetime import datetime
import json

from providers import PROVIDERS, BaseProvider
from processors import DataValidator, DataCleaner, DataTransformer
from storage import TimescaleDB, RedisStore
from config import get_settings
from utils.logging import get_logger
from utils.metrics import metrics


logger = get_logger(__name__)


class RealtimeCoordinator:
    """Coordinate real-time data streaming from multiple providers."""
    
    def __init__(self):
        self.settings = get_settings()
        self.logger = logger
        self.providers: Dict[str, BaseProvider] = {}
        self.active_streams: Dict[str, asyncio.Task] = {}
        self.subscribed_symbols: Set[str] = set()
        
        # Data processors
        self.validator = DataValidator()
        self.cleaner = DataCleaner()
        self.transformer = DataTransformer()
        
        # Storage backends
        self.timescale = TimescaleDB()
        self.redis = RedisStore()
        
        # Callbacks
        self.data_callbacks: List[Callable] = []
        self.error_callbacks: List[Callable] = []
        
        # Control
        self._running = False
        self._shutdown_event = asyncio.Event()
    
    async def initialize(self, provider_names: List[str]):
        """Initialize specified providers."""
        # Connect to storage
        await self.timescale.connect()
        await self.redis.connect()
        
        # Initialize providers
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
        
        if not self.providers:
            raise RuntimeError("No providers initialized successfully")
    
    async def subscribe(self, symbols: List[str]):
        """Subscribe to real-time data for symbols."""
        # Validate and normalize symbols
        valid_symbols = []
        for symbol in symbols:
            clean_symbol = symbol.upper().strip()
            if clean_symbol and clean_symbol not in self.subscribed_symbols:
                valid_symbols.append(clean_symbol)
                self.subscribed_symbols.add(clean_symbol)
        
        if not valid_symbols:
            self.logger.warning("No new valid symbols to subscribe")
            return
        
        self.logger.info(f"Subscribing to {len(valid_symbols)} symbols: {valid_symbols}")
        
        # Start streaming for each provider
        for provider_name, provider in self.providers.items():
            if provider_name not in self.active_streams:
                task = asyncio.create_task(
                    self._stream_provider(provider_name, provider, list(self.subscribed_symbols))
                )
                self.active_streams[provider_name] = task
    
    async def unsubscribe(self, symbols: List[str]):
        """Unsubscribe from symbols."""
        for symbol in symbols:
            clean_symbol = symbol.upper().strip()
            self.subscribed_symbols.discard(clean_symbol)
        
        # If no more symbols, stop all streams
        if not self.subscribed_symbols:
            await self.stop_all_streams()
    
    async def start(self):
        """Start the coordinator."""
        self._running = True
        self.logger.info("Real-time coordinator started")
        
        # Start monitoring task
        asyncio.create_task(self._monitor_streams())
        
        # Wait for shutdown
        await self._shutdown_event.wait()
    
    async def stop(self):
        """Stop the coordinator."""
        self._running = False
        
        # Stop all streams
        await self.stop_all_streams()
        
        # Disconnect storage
        await self.timescale.disconnect()
        await self.redis.disconnect()
        
        # Disconnect providers
        for provider in self.providers.values():
            await provider.disconnect()
        
        self._shutdown_event.set()
        self.logger.info("Real-time coordinator stopped")
    
    async def stop_all_streams(self):
        """Stop all active streams."""
        for task in self.active_streams.values():
            task.cancel()
        
        # Wait for cancellations
        await asyncio.gather(*self.active_streams.values(), return_exceptions=True)
        self.active_streams.clear()
    
    async def _stream_provider(self, provider_name: str, provider: BaseProvider, symbols: List[str]):
        """Stream data from a single provider."""
        self.logger.info(f"Starting stream for {provider_name} with symbols: {symbols}")
        
        try:
            async for market_data in provider.stream_market_data(symbols):
                if not self._running:
                    break
                
                # Process the data
                await self._process_market_data(market_data, provider_name)
                
        except asyncio.CancelledError:
            self.logger.info(f"Stream cancelled for {provider_name}")
            raise
        except Exception as e:
            self.logger.error(f"Stream error for {provider_name}: {e}")
            metrics.streaming_errors.labels(provider=provider_name).inc()
            
            # Notify error callbacks
            for callback in self.error_callbacks:
                try:
                    await callback(provider_name, e)
                except:
                    pass
            
            # Retry after delay
            if self._running:
                await asyncio.sleep(30)
                if provider_name in self.active_streams:
                    # Restart the stream
                    self.active_streams[provider_name] = asyncio.create_task(
                        self._stream_provider(provider_name, provider, symbols)
                    )
    
    async def _process_market_data(self, data: Any, provider_name: str):
        """Process incoming market data."""
        try:
            # Convert to dict if needed
            if hasattr(data, '__dict__'):
                data_dict = data.__dict__
            else:
                data_dict = data
            
            # Add provider info
            data_dict['provider'] = provider_name
            
            # Validate
            validation = self.validator.validate_realtime_data(data_dict)
            if not validation['is_valid']:
                self.logger.warning(
                    f"Invalid data from {provider_name}: {validation['errors']}"
                )
                metrics.validation_failures.labels(provider=provider_name).inc()
                return
            
            # Clean
            cleaned = self.cleaner._clean_record(data_dict)
            if not cleaned:
                return
            
            # Store in TimescaleDB
            await self.timescale.insert_market_data([cleaned])
            
            # Cache in Redis
            cache_key = f"realtime:{cleaned['symbol']}:latest"
            await self.redis.set(
                cache_key,
                json.dumps(cleaned, default=str),
                ttl=300  # 5 minutes
            )
            
            # Publish to Redis channel
            channel = f"market_data:{cleaned['symbol']}"
            await self.redis.publish(channel, json.dumps(cleaned, default=str))
            
            # Update metrics
            metrics.data_points_processed.labels(
                provider=provider_name,
                data_type='realtime'
            ).inc()
            
            # Notify callbacks
            for callback in self.data_callbacks:
                try:
                    await callback(cleaned)
                except Exception as e:
                    self.logger.error(f"Callback error: {e}")
                    
        except Exception as e:
            import traceback
            self.logger.error(f"Failed to process market data: {e}")
            self.logger.error(f"Traceback: {traceback.format_exc()}")
            self.logger.error(f"Data dict: {data_dict}")
            metrics.processing_errors.labels(provider=provider_name, error_type='processing').inc()
    
    async def _monitor_streams(self):
        """Monitor stream health and restart if needed."""
        while self._running:
            try:
                # Check each stream
                for provider_name, task in list(self.active_streams.items()):
                    if task.done():
                        # Stream ended, check if it was an error
                        try:
                            await task
                        except asyncio.CancelledError:
                            # Normal cancellation
                            pass
                        except Exception as e:
                            self.logger.error(f"Stream {provider_name} failed: {e}")
                            
                            # Restart if we're still running
                            if self._running and provider_name in self.providers:
                                provider = self.providers[provider_name]
                                self.active_streams[provider_name] = asyncio.create_task(
                                    self._stream_provider(
                                        provider_name,
                                        provider,
                                        list(self.subscribed_symbols)
                                    )
                                )
                
                # Log status
                active_count = len([t for t in self.active_streams.values() if not t.done()])
                metrics.active_streams.set(active_count)
                
                await asyncio.sleep(30)  # Check every 30 seconds
                
            except Exception as e:
                self.logger.error(f"Monitor error: {e}")
                await asyncio.sleep(30)
    
    def add_data_callback(self, callback: Callable):
        """Add callback for processed data."""
        self.data_callbacks.append(callback)
    
    def add_error_callback(self, callback: Callable):
        """Add callback for errors."""
        self.error_callbacks.append(callback)
    
    async def get_latest_prices(self, symbols: Optional[List[str]] = None) -> Dict[str, Any]:
        """Get latest cached prices for symbols."""
        if symbols is None:
            symbols = list(self.subscribed_symbols)
        
        prices = {}
        for symbol in symbols:
            cache_key = f"realtime:{symbol}:latest"
            data = await self.redis.get(cache_key)
            if data:
                prices[symbol] = json.loads(data)
        
        return prices
    
    async def get_stream_status(self) -> Dict[str, Any]:
        """Get status of all streams."""
        status = {
            'running': self._running,
            'subscribed_symbols': list(self.subscribed_symbols),
            'active_providers': list(self.providers.keys()),
            'streams': {}
        }
        
        for provider_name, task in self.active_streams.items():
            status['streams'][provider_name] = {
                'active': not task.done(),
                'cancelled': task.cancelled() if task.done() else False
            }
        
        return status