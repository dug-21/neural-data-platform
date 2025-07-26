"""Health check implementation for data ingestion service."""
import asyncio
from datetime import datetime, timedelta
from typing import Dict, Any, List, Optional, Tuple
from aiohttp import web
import json
import time
import random
from enum import Enum
from dataclasses import dataclass, field

from utils.logging import get_logger
from utils.metrics import metrics
from utils.market_hours import MarketHours, is_market_data_expected
from config import get_settings
from prometheus_client import generate_latest

logger = get_logger(__name__)


class CircuitBreakerState(Enum):
    """Circuit breaker states."""
    CLOSED = "closed"  # Normal operation
    OPEN = "open"      # Failures detected, blocking requests
    HALF_OPEN = "half_open"  # Testing if service recovered


@dataclass
class CircuitBreaker:
    """Circuit breaker for health check resilience."""
    failure_threshold: int = 5
    recovery_timeout: float = 60.0
    success_threshold: int = 2
    
    state: CircuitBreakerState = field(default=CircuitBreakerState.CLOSED)
    failure_count: int = field(default=0)
    success_count: int = field(default=0)
    last_failure_time: Optional[float] = field(default=None)
    
    def should_allow_request(self) -> bool:
        """Check if request should be allowed."""
        if self.state == CircuitBreakerState.CLOSED:
            return True
        
        if self.state == CircuitBreakerState.OPEN:
            # Check if recovery timeout has passed
            if self.last_failure_time and \
               time.time() - self.last_failure_time >= self.recovery_timeout:
                self.state = CircuitBreakerState.HALF_OPEN
                self.success_count = 0
                logger.info("Circuit breaker transitioned to HALF_OPEN")
                return True
            return False
        
        # HALF_OPEN state
        return True
    
    def record_success(self):
        """Record a successful operation."""
        if self.state == CircuitBreakerState.HALF_OPEN:
            self.success_count += 1
            if self.success_count >= self.success_threshold:
                self.state = CircuitBreakerState.CLOSED
                self.failure_count = 0
                logger.info("Circuit breaker transitioned to CLOSED")
        elif self.state == CircuitBreakerState.CLOSED:
            self.failure_count = 0
    
    def record_failure(self):
        """Record a failed operation."""
        self.failure_count += 1
        self.last_failure_time = time.time()
        
        if self.state == CircuitBreakerState.CLOSED and \
           self.failure_count >= self.failure_threshold:
            self.state = CircuitBreakerState.OPEN
            logger.warning(f"Circuit breaker opened after {self.failure_count} failures")
        elif self.state == CircuitBreakerState.HALF_OPEN:
            self.state = CircuitBreakerState.OPEN
            self.success_count = 0
            logger.warning("Circuit breaker reopened due to failure in HALF_OPEN state")


class HealthCheckHandler:
    """Handles health check requests and monitors service health."""
    
    def __init__(self, port: int = 8080):
        # Code-first configuration - works without env vars
        self.port = port
        self.logger = logger
        
        # Try to get settings, but work without them
        try:
            self.settings = get_settings()
        except Exception:
            self.settings = None
            logger.warning("Running health checks without environment configuration")
        
        # Health check thresholds (code-first defaults)
        self.max_data_age_seconds = 300  # 5 minutes
        self.min_success_rate = 0.8  # 80% success rate
        self.min_active_streams = 1  # At least one active stream
        
        # Circuit breakers for each component
        self.circuit_breakers = {
            'database': CircuitBreaker(),
            'redis': CircuitBreaker(),
            'websocket': CircuitBreaker(),
            'data_flow': CircuitBreaker()
        }
        
        # Component references (set by main service)
        self.realtime_coordinator = None
        self.stream_manager = None
        self.batch_scheduler = None
        self.timescale_db = None
        self.redis_store = None
        
        # Track last successful data timestamps
        self.last_data_timestamps: Dict[str, datetime] = {}
        
        # Health check web app
        self.app = web.Application()
        self._setup_routes()
        self.runner = None
        
    def _setup_routes(self):
        """Setup health check routes."""
        self.app.router.add_get('/health', self.health_check)
        self.app.router.add_get('/health/detailed', self.detailed_health_check)
        self.app.router.add_get('/health/live', self.liveness_probe)
        self.app.router.add_get('/health/ready', self.readiness_probe)
        self.app.router.add_get('/metrics', self.metrics_endpoint)
        
    async def start(self, port: Optional[int] = None):
        """Start health check HTTP server."""
        # Use provided port or instance default
        actual_port = port or self.port
        
        self.runner = web.AppRunner(self.app)
        await self.runner.setup()
        site = web.TCPSite(self.runner, '0.0.0.0', actual_port)
        await site.start()
        self.logger.info(f"Health check server started on port {actual_port}")
        
    async def stop(self):
        """Stop health check server."""
        if self.runner:
            await self.runner.cleanup()
            self.logger.info("Health check server stopped")
    
    def update_data_timestamp(self, provider: str, symbol: str):
        """Update timestamp for last received data."""
        key = f"{provider}:{symbol}"
        self.last_data_timestamps[key] = datetime.now()
        
    async def check_database_health(self) -> Tuple[bool, str]:
        """Check database connectivity with circuit breaker."""
        breaker = self.circuit_breakers['database']
        
        if not breaker.should_allow_request():
            return False, f"Circuit breaker OPEN (failures: {breaker.failure_count})"
        
        try:
            if self.timescale_db and hasattr(self.timescale_db, 'pool'):
                # Execute simple query to check connection with timeout
                async with asyncio.timeout(5.0):  # 5 second timeout
                    async with self.timescale_db.pool.acquire() as conn:
                        result = await conn.fetchval("SELECT 1")
                        if result == 1:
                            breaker.record_success()
                            return True, "Connected"
            
            breaker.record_failure()
            return False, "No database connection"
        except asyncio.TimeoutError:
            breaker.record_failure()
            return False, "Database query timeout"
        except Exception as e:
            breaker.record_failure()
            return False, f"Database error: {str(e)}"
    
    async def check_redis_health(self) -> Tuple[bool, str]:
        """Check Redis connectivity with circuit breaker."""
        breaker = self.circuit_breakers['redis']
        
        if not breaker.should_allow_request():
            return False, f"Circuit breaker OPEN (failures: {breaker.failure_count})"
        
        try:
            if self.redis_store and hasattr(self.redis_store, 'redis'):
                # Ping Redis with timeout
                async with asyncio.timeout(3.0):  # 3 second timeout
                    pong = await self.redis_store.redis.ping()
                    if pong:
                        breaker.record_success()
                        return True, "Connected"
            
            breaker.record_failure()
            return False, "No Redis connection"
        except asyncio.TimeoutError:
            breaker.record_failure()
            return False, "Redis ping timeout"
        except Exception as e:
            breaker.record_failure()
            return False, f"Redis error: {str(e)}"
    
    def check_websocket_health(self) -> Dict[str, Any]:
        """Check WebSocket connection status with circuit breaker."""
        breaker = self.circuit_breakers['websocket']
        
        ws_status = {
            'total_providers': 0,
            'active_connections': 0,
            'providers': {},
            'circuit_breaker': breaker.state.value,
            'healthy': True,
            'market_status': {}
        }
        
        if self.realtime_coordinator:
            ws_status['total_providers'] = len(self.realtime_coordinator.providers)
            
            for provider_name, provider in self.realtime_coordinator.providers.items():
                provider_status = {
                    'connected': False,
                    'subscribed_symbols': 0,
                    'last_error': None,
                    'market_open': False,
                    'market_message': ''
                }
                
                # Check market status for this provider
                market_open, market_message = is_market_data_expected(provider_name)
                provider_status['market_open'] = market_open
                provider_status['market_message'] = market_message
                
                # Check if provider has WebSocket connection
                if hasattr(provider, 'ws') and provider.ws:
                    provider_status['connected'] = not provider.ws.closed
                    if provider_status['connected']:
                        ws_status['active_connections'] += 1
                        
                # Check subscribed symbols
                if hasattr(provider, 'subscribed_symbols'):
                    provider_status['subscribed_symbols'] = len(provider.subscribed_symbols)
                    
                ws_status['providers'][provider_name] = provider_status
        
        # Check if any provider should have data right now
        any_market_open = any(
            p.get('market_open', False) 
            for p in ws_status['providers'].values()
        )
        
        # Update circuit breaker and health status based on connection status and market hours
        if ws_status['active_connections'] > 0:
            breaker.record_success()
            ws_status['healthy'] = True
        elif not any_market_open:
            # Markets are closed, so no connections is expected
            breaker.record_success()  # Don't penalize for closed markets
            ws_status['healthy'] = True
            ws_status['market_status'] = {
                'all_markets_closed': True,
                'message': 'All markets are closed - no data expected'
            }
        else:
            # Markets are open but no connections - this is a problem
            breaker.record_failure()
            ws_status['healthy'] = False
            
        return ws_status
    
    def check_data_flow_health(self) -> Dict[str, Any]:
        """Check data flow recency with circuit breaker."""
        breaker = self.circuit_breakers['data_flow']
        now = datetime.now()
        
        flow_status = {
            'total_flows': len(self.last_data_timestamps),
            'active_flows': 0,
            'stale_flows': 0,
            'oldest_data_age_seconds': None,
            'details': [],
            'circuit_breaker': breaker.state.value,
            'healthy': True,
            'market_considerations': {}
        }
        
        # Check which markets are open
        markets_open = {}
        for provider in ['alpaca', 'polygon', 'finnhub', 'binance']:
            market_open, message = is_market_data_expected(provider)
            markets_open[provider] = {'open': market_open, 'message': message}
        
        flow_status['market_considerations'] = markets_open
        any_market_open = any(m['open'] for m in markets_open.values())
        
        for key, timestamp in self.last_data_timestamps.items():
            age_seconds = (now - timestamp).total_seconds()
            provider_name = key.split(':')[0] if ':' in key else 'unknown'
            
            # Check if this provider's market is open
            provider_market_open = markets_open.get(provider_name, {}).get('open', True)
            
            # If market is closed, be more lenient with staleness
            if not provider_market_open:
                # Allow up to 24 hours for closed markets
                is_stale = age_seconds > 86400  # 24 hours
            else:
                is_stale = age_seconds > self.max_data_age_seconds
            
            flow_status['details'].append({
                'flow': key,
                'last_update': timestamp.isoformat(),
                'age_seconds': age_seconds,
                'is_stale': is_stale,
                'market_open': provider_market_open
            })
            
            if is_stale:
                flow_status['stale_flows'] += 1
            else:
                flow_status['active_flows'] += 1
                
            if flow_status['oldest_data_age_seconds'] is None or age_seconds > flow_status['oldest_data_age_seconds']:
                flow_status['oldest_data_age_seconds'] = age_seconds
        
        # Update circuit breaker based on data flow health
        if not any_market_open:
            # If no markets are open, don't require active flows
            breaker.record_success()
            flow_status['healthy'] = True
            flow_status['message'] = 'Markets closed - data staleness is expected'
        elif flow_status['total_flows'] == 0 and not any_market_open:
            # No flows when markets are closed is fine
            breaker.record_success()
            flow_status['healthy'] = True
            flow_status['message'] = 'No data flows - markets are closed'
        elif flow_status['total_flows'] == 0:
            # No flows when markets are open might be startup
            breaker.record_success()
            flow_status['healthy'] = True
            flow_status['message'] = 'No data flows yet - service may be starting up'
        elif flow_status['active_flows'] >= self.min_active_streams and flow_status['stale_flows'] == 0:
            breaker.record_success()
            flow_status['healthy'] = True
        else:
            breaker.record_failure()
            flow_status['healthy'] = False
                
        return flow_status
    
    def check_stream_health(self) -> Dict[str, Any]:
        """Check stream manager health."""
        stream_status = {
            'total_streams': 0,
            'active_streams': 0,
            'error_streams': 0,
            'success_rate': 0.0,
            'streams': {}
        }
        
        if self.stream_manager:
            stream_status['total_streams'] = len(self.stream_manager.active_streams)
            
            total_operations = 0
            successful_operations = 0
            
            for stream_id, stream_info in self.stream_manager.active_streams.items():
                is_active = stream_info.get('status') == 'running'
                error_count = stream_info.get('error_count', 0)
                success_count = stream_info.get('success_count', 0)
                
                if is_active:
                    stream_status['active_streams'] += 1
                if error_count > success_count:
                    stream_status['error_streams'] += 1
                    
                total_operations += error_count + success_count
                successful_operations += success_count
                
                stream_status['streams'][stream_id] = {
                    'status': stream_info.get('status'),
                    'error_count': error_count,
                    'success_count': success_count,
                    'last_data': stream_info.get('last_data')
                }
            
            if total_operations > 0:
                stream_status['success_rate'] = successful_operations / total_operations
                
        return stream_status
    
    async def get_health_status(self) -> Dict[str, Any]:
        """Get comprehensive health status."""
        # Database health
        db_healthy, db_message = await self.check_database_health()
        
        # Redis health
        redis_healthy, redis_message = await self.check_redis_health()
        
        # WebSocket health
        ws_status = self.check_websocket_health()
        ws_healthy = ws_status['healthy']  # Now uses market-aware logic
        
        # Data flow health
        flow_status = self.check_data_flow_health()
        flow_healthy = flow_status['healthy']  # Now uses market-aware logic
        
        # Stream health
        stream_status = self.check_stream_health()
        stream_healthy = (
            stream_status['active_streams'] >= self.min_active_streams and
            stream_status['success_rate'] >= self.min_success_rate
        )
        
        # Overall health
        is_healthy = all([db_healthy, redis_healthy, ws_healthy or stream_healthy, flow_healthy])
        
        # Update Prometheus metrics
        metrics.health_check_status.labels(component='overall').set(1 if is_healthy else 0)
        metrics.health_check_component_status.labels(component='database').set(1 if db_healthy else 0)
        metrics.health_check_component_status.labels(component='redis').set(1 if redis_healthy else 0)
        metrics.health_check_component_status.labels(component='websockets').set(1 if ws_healthy else 0)
        metrics.health_check_component_status.labels(component='data_flow').set(1 if flow_healthy else 0)
        metrics.health_check_component_status.labels(component='streams').set(1 if stream_healthy else 0)
        
        # Update data flow age metrics
        for key, timestamp in self.last_data_timestamps.items():
            provider, symbol = key.split(':', 1)
            age_seconds = (datetime.now() - timestamp).total_seconds()
            metrics.data_flow_age.labels(provider=provider, symbol=symbol).set(age_seconds)
        
        return {
            'status': 'healthy' if is_healthy else 'unhealthy',
            'timestamp': datetime.now().isoformat(),
            'checks': {
                'database': {
                    'healthy': db_healthy,
                    'message': db_message
                },
                'redis': {
                    'healthy': redis_healthy,
                    'message': redis_message
                },
                'websockets': {
                    'healthy': ws_healthy,
                    'details': ws_status
                },
                'data_flow': {
                    'healthy': flow_healthy,
                    'details': flow_status
                },
                'streams': {
                    'healthy': stream_healthy,
                    'details': stream_status
                }
            },
            'metrics': {
                'active_streams': metrics.active_streams._value.get() if hasattr(metrics.active_streams, '_value') else 0,
                'total_data_points': metrics.data_points_processed._value.sum() if hasattr(metrics.data_points_processed, '_value') else 0,
                'total_errors': metrics.processing_errors._value.sum() if hasattr(metrics.processing_errors, '_value') else 0
            }
        }
    
    async def health_check(self, request: web.Request) -> web.Response:
        """Simple health check endpoint that works without env vars."""
        try:
            status = await self.get_health_status()
            is_healthy = status['status'] == 'healthy'
            
            # Add circuit breaker status
            breaker_status = {
                breaker_name: {
                    'state': breaker.state.value,
                    'failures': breaker.failure_count
                }
                for breaker_name, breaker in self.circuit_breakers.items()
            }
            
            response_data = {
                'status': status['status'],
                'timestamp': status['timestamp'],
                'circuit_breakers': breaker_status
            }
            
            # Update Prometheus metrics
            for name, breaker in self.circuit_breakers.items():
                metrics.health_check_component_status.labels(
                    component=f'circuit_breaker_{name}'
                ).set(1 if breaker.state == CircuitBreakerState.CLOSED else 0)
            
            return web.Response(
                text=json.dumps(response_data),
                status=200 if is_healthy else 503,
                content_type='application/json'
            )
        except Exception as e:
            self.logger.error(f"Health check error: {e}")
            return web.Response(
                text=json.dumps({
                    'status': 'error',
                    'message': str(e),
                    'timestamp': datetime.now().isoformat()
                }),
                status=503,
                content_type='application/json'
            )
    
    async def detailed_health_check(self, request: web.Request) -> web.Response:
        """Detailed health check endpoint."""
        try:
            status = await self.get_health_status()
            is_healthy = status['status'] == 'healthy'
            
            return web.Response(
                text=json.dumps(status, indent=2),
                status=200 if is_healthy else 503,
                content_type='application/json'
            )
        except Exception as e:
            self.logger.error(f"Detailed health check error: {e}")
            return web.Response(
                text=json.dumps({
                    'status': 'error',
                    'message': str(e),
                    'timestamp': datetime.now().isoformat()
                }),
                status=503,
                content_type='application/json'
            )
    
    async def liveness_probe(self, request: web.Request) -> web.Response:
        """Kubernetes liveness probe - checks if service is alive."""
        return web.Response(text='OK', status=200)
    
    async def readiness_probe(self, request: web.Request) -> web.Response:
        """Kubernetes readiness probe - checks if service is ready to accept traffic."""
        try:
            # Quick checks for readiness
            db_healthy, _ = await self.check_database_health()
            redis_healthy, _ = await self.check_redis_health()
            
            if db_healthy and redis_healthy:
                return web.Response(text='OK', status=200)
            else:
                return web.Response(text='Not Ready', status=503)
        except Exception as e:
            return web.Response(text=f'Error: {str(e)}', status=503)
    
    async def metrics_endpoint(self, request: web.Request) -> web.Response:
        """Prometheus metrics endpoint."""
        try:
            # Update health metrics before serving
            status = await self.get_health_status()
            
            # Update health status metrics
            for component, check in status.get('checks', {}).items():
                health_value = 1 if check.get('healthy', False) else 0
                metrics.health_check_status.labels(component=component).set(health_value)
            
            # Update circuit breaker metrics
            circuit_state_map = {
                CircuitBreakerState.CLOSED: 0,
                CircuitBreakerState.OPEN: 1,
                CircuitBreakerState.HALF_OPEN: 2
            }
            
            for name, breaker in self.circuit_breakers.items():
                state_value = circuit_state_map.get(breaker.state, -1)
                metrics.circuit_breaker_state.labels(component=name).set(state_value)
                
            # Generate Prometheus metrics
            metrics_data = generate_latest()
            
            return web.Response(
                body=metrics_data,
                content_type='text/plain; version=0.0.4; charset=utf-8'
            )
        except Exception as e:
            self.logger.error(f"Metrics endpoint error: {e}")
            return web.Response(
                text=f"Error generating metrics: {str(e)}",
                status=500,
                content_type='text/plain'
            )