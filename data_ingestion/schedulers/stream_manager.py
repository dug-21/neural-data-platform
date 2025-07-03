"""Unified stream manager for coordinating multiple data streams."""
import asyncio
from typing import List, Dict, Any, Optional, Set, Callable
from datetime import datetime
import json
from collections import defaultdict

from ..providers import BaseProvider
from ..config import get_settings
from ..utils.logging import get_logger
from ..utils.metrics import metrics


logger = get_logger(__name__)


class StreamManager:
    """Manage and coordinate multiple data streams with failover and load balancing."""
    
    def __init__(self):
        self.settings = get_settings()
        self.logger = logger
        
        # Stream tracking
        self.active_streams: Dict[str, Dict[str, Any]] = {}
        self.stream_health: Dict[str, Dict[str, Any]] = defaultdict(dict)
        self.symbol_assignments: Dict[str, Set[str]] = defaultdict(set)  # symbol -> providers
        
        # Configuration
        self.max_retries = 3
        self.health_check_interval = 30
        self.failover_threshold = 0.5  # 50% error rate triggers failover
        
        # Control
        self._running = False
        self._health_monitor_task: Optional[asyncio.Task] = None
    
    async def start(self):
        """Start the stream manager."""
        self._running = True
        self._health_monitor_task = asyncio.create_task(self._monitor_health())
        self.logger.info("Stream manager started")
    
    async def stop(self):
        """Stop the stream manager."""
        self._running = False
        
        if self._health_monitor_task:
            self._health_monitor_task.cancel()
            try:
                await self._health_monitor_task
            except asyncio.CancelledError:
                pass
        
        self.logger.info("Stream manager stopped")
    
    async def register_stream(
        self,
        stream_id: str,
        provider: BaseProvider,
        symbols: List[str],
        stream_type: str = "market_data",
        priority: int = 1
    ):
        """Register a new stream with the manager."""
        self.active_streams[stream_id] = {
            'provider': provider,
            'symbols': set(symbols),
            'stream_type': stream_type,
            'priority': priority,
            'status': 'pending',
            'task': None,
            'created_at': datetime.now(),
            'last_data': None,
            'error_count': 0,
            'success_count': 0
        }
        
        # Update symbol assignments
        for symbol in symbols:
            self.symbol_assignments[symbol].add(stream_id)
        
        self.logger.info(
            f"Registered stream {stream_id} for {len(symbols)} symbols "
            f"with priority {priority}"
        )
    
    async def start_stream(self, stream_id: str, data_handler: Callable):
        """Start a registered stream."""
        if stream_id not in self.active_streams:
            raise ValueError(f"Stream {stream_id} not registered")
        
        stream_info = self.active_streams[stream_id]
        
        if stream_info['task'] and not stream_info['task'].done():
            self.logger.warning(f"Stream {stream_id} already running")
            return
        
        # Create stream task
        stream_info['task'] = asyncio.create_task(
            self._run_stream(stream_id, data_handler)
        )
        stream_info['status'] = 'running'
        
        self.logger.info(f"Started stream {stream_id}")
    
    async def stop_stream(self, stream_id: str):
        """Stop a specific stream."""
        if stream_id not in self.active_streams:
            return
        
        stream_info = self.active_streams[stream_id]
        
        if stream_info['task']:
            stream_info['task'].cancel()
            try:
                await stream_info['task']
            except asyncio.CancelledError:
                pass
        
        stream_info['status'] = 'stopped'
        self.logger.info(f"Stopped stream {stream_id}")
    
    async def _run_stream(self, stream_id: str, data_handler: Callable):
        """Run a single stream with error handling and retries."""
        stream_info = self.active_streams[stream_id]
        provider = stream_info['provider']
        symbols = list(stream_info['symbols'])
        stream_type = stream_info['stream_type']
        
        retry_count = 0
        backoff = 1
        
        while self._running and retry_count < self.max_retries:
            try:
                self.logger.info(f"Stream {stream_id} connecting...")
                
                # Choose appropriate streaming method
                if stream_type == 'market_data':
                    stream_method = provider.stream_market_data
                elif stream_type == 'tick_data':
                    stream_method = provider.stream_tick_data
                else:
                    raise ValueError(f"Unknown stream type: {stream_type}")
                
                # Start streaming
                async for data in stream_method(symbols):
                    if not self._running:
                        break
                    
                    # Update statistics
                    stream_info['last_data'] = datetime.now()
                    stream_info['success_count'] += 1
                    
                    # Process data
                    try:
                        await data_handler(data, stream_id)
                    except Exception as e:
                        self.logger.error(f"Data handler error: {e}")
                    
                    # Reset retry count on successful data
                    retry_count = 0
                    backoff = 1
                    
            except asyncio.CancelledError:
                self.logger.info(f"Stream {stream_id} cancelled")
                break
                
            except Exception as e:
                retry_count += 1
                stream_info['error_count'] += 1
                
                self.logger.error(
                    f"Stream {stream_id} error (retry {retry_count}/{self.max_retries}): {e}"
                )
                
                if retry_count < self.max_retries:
                    # Exponential backoff
                    await asyncio.sleep(backoff)
                    backoff = min(backoff * 2, 60)  # Max 60 seconds
                else:
                    # Max retries reached
                    stream_info['status'] = 'failed'
                    await self._handle_stream_failure(stream_id)
                    break
        
        stream_info['status'] = 'stopped'
    
    async def _handle_stream_failure(self, stream_id: str):
        """Handle stream failure with failover logic."""
        stream_info = self.active_streams[stream_id]
        failed_symbols = stream_info['symbols']
        
        self.logger.warning(
            f"Stream {stream_id} failed, attempting failover for "
            f"{len(failed_symbols)} symbols"
        )
        
        # Find alternative streams for affected symbols
        for symbol in failed_symbols:
            alternative_streams = self.symbol_assignments[symbol] - {stream_id}
            
            if alternative_streams:
                # Symbols are covered by other streams
                self.logger.info(
                    f"Symbol {symbol} covered by {len(alternative_streams)} "
                    f"other streams"
                )
            else:
                # Need to find backup provider
                self.logger.warning(f"No backup stream for symbol {symbol}")
                metrics.stream_failover_required.labels(symbol=symbol).inc()
    
    async def _monitor_health(self):
        """Monitor health of all streams."""
        while self._running:
            try:
                for stream_id, stream_info in self.active_streams.items():
                    if stream_info['status'] != 'running':
                        continue
                    
                    # Calculate health metrics
                    total_operations = (
                        stream_info['success_count'] + 
                        stream_info['error_count']
                    )
                    
                    if total_operations > 0:
                        error_rate = stream_info['error_count'] / total_operations
                        
                        # Check if stream is stale
                        if stream_info['last_data']:
                            time_since_data = (
                                datetime.now() - stream_info['last_data']
                            ).total_seconds()
                        else:
                            time_since_data = float('inf')
                        
                        # Update health status
                        self.stream_health[stream_id] = {
                            'error_rate': error_rate,
                            'time_since_data': time_since_data,
                            'total_operations': total_operations,
                            'status': self._calculate_health_status(
                                error_rate, time_since_data
                            )
                        }
                        
                        # Check if failover needed
                        if error_rate > self.failover_threshold:
                            self.logger.warning(
                                f"Stream {stream_id} error rate {error_rate:.2%} "
                                f"exceeds threshold"
                            )
                            # Could trigger failover here
                        
                        # Update metrics
                        metrics.stream_health.labels(
                            stream_id=stream_id,
                            status=self.stream_health[stream_id]['status']
                        ).set(1 - error_rate)  # Health score
                
                await asyncio.sleep(self.health_check_interval)
                
            except Exception as e:
                self.logger.error(f"Health monitor error: {e}")
                await asyncio.sleep(self.health_check_interval)
    
    def _calculate_health_status(
        self,
        error_rate: float,
        time_since_data: float
    ) -> str:
        """Calculate health status based on metrics."""
        if error_rate > 0.5 or time_since_data > 300:  # 5 minutes
            return 'unhealthy'
        elif error_rate > 0.1 or time_since_data > 60:
            return 'degraded'
        else:
            return 'healthy'
    
    def get_stream_status(self) -> Dict[str, Any]:
        """Get status of all streams."""
        status = {
            'total_streams': len(self.active_streams),
            'active_streams': sum(
                1 for s in self.active_streams.values()
                if s['status'] == 'running'
            ),
            'failed_streams': sum(
                1 for s in self.active_streams.values()
                if s['status'] == 'failed'
            ),
            'streams': {}
        }
        
        for stream_id, stream_info in self.active_streams.items():
            status['streams'][stream_id] = {
                'provider': stream_info['provider'].name,
                'symbols': list(stream_info['symbols']),
                'status': stream_info['status'],
                'priority': stream_info['priority'],
                'success_count': stream_info['success_count'],
                'error_count': stream_info['error_count'],
                'last_data': stream_info['last_data'].isoformat()
                    if stream_info['last_data'] else None,
                'health': self.stream_health.get(stream_id, {})
            }
        
        return status
    
    def get_symbol_coverage(self) -> Dict[str, List[str]]:
        """Get which streams cover which symbols."""
        coverage = {}
        
        for symbol, stream_ids in self.symbol_assignments.items():
            coverage[symbol] = [
                stream_id for stream_id in stream_ids
                if self.active_streams.get(stream_id, {}).get('status') == 'running'
            ]
        
        return coverage
    
    async def rebalance_streams(self):
        """Rebalance symbol assignments across streams."""
        self.logger.info("Starting stream rebalancing")
        
        # Get current load per stream
        stream_loads = {}
        for stream_id, stream_info in self.active_streams.items():
            if stream_info['status'] == 'running':
                stream_loads[stream_id] = len(stream_info['symbols'])
        
        if not stream_loads:
            return
        
        # Calculate average load
        total_symbols = sum(stream_loads.values())
        avg_load = total_symbols / len(stream_loads)
        
        # Identify overloaded and underloaded streams
        overloaded = [
            sid for sid, load in stream_loads.items()
            if load > avg_load * 1.2  # 20% above average
        ]
        underloaded = [
            sid for sid, load in stream_loads.items()
            if load < avg_load * 0.8  # 20% below average
        ]
        
        # TODO: Implement actual rebalancing logic
        self.logger.info(
            f"Rebalancing: {len(overloaded)} overloaded, "
            f"{len(underloaded)} underloaded streams"
        )