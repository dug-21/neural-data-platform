"""Real-time data streaming coordinator."""
import asyncio
from typing import List, Dict, Any, Set, Optional, Callable
import json

from providers import PROVIDERS, BaseProvider
from processors import DataValidator, DataCleaner, DataTransformer
from storage import TimescaleDB, RedisStore
from config import get_settings
from utils.logging import get_logger
from utils.metrics import metrics
from utils.health_tracker import health_tracker
from utils.channel_validator import ChannelValidator, CircuitBreaker, CircuitBreakerOpenError
from utils.enhanced_retry import CircuitBreakerRetryIntegration, RetryConfig, RedisConnectionError, RedisTimeoutError


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
        
        # Channel management
        self.channel_validator = ChannelValidator()
        self.circuit_breaker = CircuitBreaker(
            failure_threshold=self.settings.redis_max_connections // 10,  # Dynamic threshold
            recovery_timeout=30
        )
        
        # Enhanced retry integration
        retry_config = RetryConfig(
            max_attempts=3,
            base_delay_ms=100,
            max_delay_ms=5000,
            backoff_multiplier=2.0
        )
        self.retry_integration = CircuitBreakerRetryIntegration(self.circuit_breaker, retry_config)
        
        # Control
        self._running = False
        self._shutdown_event = asyncio.Event()
    
    async def initialize(self, provider_names: List[str]):
        """Initialize specified providers."""
        # Connect to storage with error handling
        try:
            await self.timescale.connect()
        except Exception as e:
            self.logger.error(f"Failed to connect to TimescaleDB after retries: {e}")
            raise
        
        try:
            await self.redis.connect()
        except Exception as e:
            self.logger.error(f"Failed to connect to Redis: {e}")
            raise
        
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
        
        # Don't create streaming tasks here - wait for start() to be called
        # The tasks will be created when _running is True
    
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
        
        # Now create streaming tasks for each provider with subscribed symbols
        if self.subscribed_symbols:
            self.logger.info(f"Starting streams for {len(self.subscribed_symbols)} symbols")
            for provider_name, provider in self.providers.items():
                if provider_name not in self.active_streams:
                    task = asyncio.create_task(
                        self._stream_provider(provider_name, provider, list(self.subscribed_symbols))
                    )
                    self.active_streams[provider_name] = task
                    self.logger.info(f"Started stream for provider: {provider_name}")
        
        # Start monitoring task
        asyncio.create_task(self._monitor_streams())
        
        # Don't wait here - let the main service handle shutdown
    
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
    
    @metrics.track_pipeline_stage("realtime", "stream_provider")
    async def _stream_provider(self, provider_name: str, provider: BaseProvider, symbols: List[str]):
        """Stream data from a single provider."""
        self.logger.info(f"Starting stream for {provider_name} with symbols: {symbols}")
        
        try:
            async for market_data in provider.stream_market_data_ws(symbols):
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
    
    @metrics.track_pipeline_stage("realtime", "process_data")
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
            
            # Update health tracker
            health_tracker.update_data_timestamp(provider_name, cleaned.get('symbol', 'UNKNOWN'))
            
            # Cache in Redis
            cache_key = f"realtime:{cleaned['symbol']}:latest"
            await self.redis.set(
                cache_key,
                json.dumps(cleaned, default=str),
                ttl=300  # 5 minutes
            )
            
            # Convert time to timestamp for neural-trader compatibility
            market_data = cleaned.copy()
            if 'time' in market_data:
                # Check if time is already a datetime object
                if hasattr(market_data['time'], 'timestamp'):
                    # It's already a datetime object
                    market_data['timestamp'] = int(market_data['time'].timestamp())
                else:
                    # It's a string, parse it
                    from dateutil import parser
                    dt = parser.parse(market_data['time'])
                    market_data['timestamp'] = int(dt.timestamp())
                # Keep time field for backward compatibility
            
            # Publish to Redis channels
            # Symbol-specific channel (existing - keeping unchanged)
            channel = f"market_data:{cleaned['symbol']}"
            await self.redis.publish(channel, json.dumps(cleaned, default=str))
            
            # Phase 2A: Dual publishing implementation per INTERFACE_CONTRACT
            symbol = cleaned['symbol'].upper()  # Ensure uppercase per contract
            
            # 1. PRIMARY: Per-symbol channel (NEW - INTERFACE_CONTRACT format)
            symbol_channel = f"market:{symbol}"
            if self.channel_validator.validate_channel_name(symbol_channel):
                try:
                    await self._publish_with_retry(symbol_channel, market_data, provider_name)
                except Exception as e:
                    self.logger.error(f"Failed to publish to {symbol_channel} after all retries: {e}")
                    # Log error but don't call non-existent metric
            else:
                self.logger.warning(f"Invalid channel format: {symbol_channel} for symbol: {symbol}")
                # Log warning but don't call non-existent metric
            
            # 2. SECONDARY: Legacy unified channel (BACKWARD COMPATIBILITY)
            if self.settings.enable_legacy_channel:
                try:
                    await self._publish_with_retry("market:updates", market_data, provider_name)
                except Exception as e:
                    self.logger.error(f"Failed to publish to market:updates after all retries: {e}")
                    # Log error but don't call non-existent metric
            
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
    
    async def _publish_with_retry(self, channel: str, data: Dict[str, Any], provider: str):
        """
        Publish message with enhanced retry logic and circuit breaker protection.
        
        Args:
            channel: Redis channel name
            data: Message data to publish
            provider: Provider name for metrics
        """
        async def _do_publish():
            # Convert data to JSON with proper error handling
            try:
                message = json.dumps(data, default=str)
            except (TypeError, ValueError) as e:
                raise ValueError(f"Failed to serialize data: {e}")
            
            # Attempt Redis publish
            try:
                result = await self.redis.publish(channel, message)
                return result
            except Exception as e:
                # Classify the error for retry logic
                if "connection" in str(e).lower() or "timeout" in str(e).lower():
                    raise RedisConnectionError(f"Redis connection/timeout error: {e}")
                else:
                    # Other errors are also retryable for now
                    raise RedisConnectionError(f"Redis error: {e}")
        
        try:
            result = await self.retry_integration.execute_with_retry_and_circuit_breaker(
                _do_publish, channel
            )
            
            # Success - use correct metric name with sanitized channel
            # Replace colon with underscore for Prometheus label compatibility
            sanitized_channel = channel.replace(':', '_')
            metrics.redis_publish_total.labels(channel=sanitized_channel, provider=provider).inc()
            return result
            
        except Exception as e:
            # Final failure after all retries
            self.logger.error(f"Publishing to {channel} failed after all retry attempts: {e}")
            # Log but don't call non-existent metric
            raise
    
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