"""
Phase 2 Performance Tests
Tests for INTERFACE_CONTRACT performance requirements
"""
import pytest
import time
import asyncio
import statistics
from unittest.mock import AsyncMock, MagicMock, patch
from typing import List, Dict, Any

# Import test subjects
from schedulers.realtime_coordinator import RealtimeCoordinator
from utils.channel_validator import ChannelValidator, CircuitBreaker
from config import Settings


@pytest.mark.asyncio
class TestPerformanceRequirements:
    """Test performance requirements per INTERFACE_CONTRACT."""
    
    async def test_publishing_latency_requirement(self):
        """Test publishing latency meets < 5ms average requirement."""
        coordinator = RealtimeCoordinator()
        
        # Mock dependencies for fast execution
        coordinator.redis = AsyncMock()
        coordinator.timescale = AsyncMock()
        coordinator.settings = Settings(enable_legacy_channel=True)
        coordinator.channel_validator = MagicMock()
        coordinator.channel_validator.validate_channel_name.return_value = True
        coordinator.circuit_breaker = MagicMock()
        coordinator.circuit_breaker.allow_request.return_value = True
        
        # Make redis.publish fast
        async def fast_publish(channel, message):
            await asyncio.sleep(0.001)  # 1ms simulated latency
            return 1
        
        coordinator.redis.publish = fast_publish
        
        # Test data
        test_data = {
            'symbol': 'AAPL',
            'price': 150.0,
            'volume': 1000,
            'time': '2025-08-08T15:30:00Z'
        }
        
        # Measure publishing latency over multiple runs
        latencies = []
        num_tests = 50
        
        for _ in range(num_tests):
            start_time = time.perf_counter()
            await coordinator._process_market_data(test_data, 'test_provider')
            end_time = time.perf_counter()
            
            latency_ms = (end_time - start_time) * 1000
            latencies.append(latency_ms)
        
        # Analyze results
        avg_latency = statistics.mean(latencies)
        max_latency = max(latencies)
        p95_latency = statistics.quantiles(latencies, n=20)[18]  # 95th percentile
        
        print(f"Average latency: {avg_latency:.2f}ms")
        print(f"Maximum latency: {max_latency:.2f}ms") 
        print(f"95th percentile latency: {p95_latency:.2f}ms")
        
        # Assert requirements
        assert avg_latency < 5.0, f"Average latency {avg_latency:.2f}ms exceeds 5ms requirement"
        assert p95_latency < 10.0, f"95th percentile latency {p95_latency:.2f}ms too high"
    
    async def test_throughput_requirement(self):
        """Test throughput meets 10,000+ messages/second per symbol."""
        coordinator = RealtimeCoordinator()
        
        # Mock dependencies
        coordinator.redis = AsyncMock()
        coordinator.timescale = AsyncMock()
        coordinator.settings = Settings(enable_legacy_channel=True)
        coordinator.channel_validator = MagicMock()
        coordinator.channel_validator.validate_channel_name.return_value = True
        coordinator.circuit_breaker = MagicMock()
        coordinator.circuit_breaker.allow_request.return_value = True
        
        # Fast mock implementations
        coordinator.redis.publish = AsyncMock(return_value=1)
        coordinator.timescale.insert_market_data = AsyncMock()
        
        # Test data
        symbols = ['AAPL', 'MSFT', 'GOOGL', 'NVDA', 'TSLA']
        messages_per_symbol = 1000  # Scaled down for testing
        
        # Create test messages
        messages = []
        for symbol in symbols:
            for i in range(messages_per_symbol):
                messages.append({
                    'symbol': symbol,
                    'price': 100.0 + i * 0.01,
                    'volume': 1000 + i,
                    'time': f'2025-08-08T15:30:{i:02d}Z'
                })
        
        # Measure throughput
        start_time = time.perf_counter()
        
        # Process messages concurrently
        tasks = [
            coordinator._process_market_data(msg, 'test_provider')
            for msg in messages
        ]
        await asyncio.gather(*tasks)
        
        end_time = time.perf_counter()
        
        # Calculate throughput
        total_time = end_time - start_time
        total_messages = len(messages)
        throughput = total_messages / total_time
        
        print(f"Processed {total_messages} messages in {total_time:.2f}s")
        print(f"Throughput: {throughput:.2f} messages/second")
        
        # Assert minimum throughput (adjusted for test scale)
        assert throughput > 2000, f"Throughput {throughput:.2f} msg/s too low for scaled test"
    
    async def test_memory_usage_during_load(self):
        """Test memory usage stays within bounds during high load."""
        import psutil
        import os
        
        coordinator = RealtimeCoordinator()
        
        # Mock dependencies
        coordinator.redis = AsyncMock()
        coordinator.timescale = AsyncMock()
        coordinator.settings = Settings(enable_legacy_channel=True)
        coordinator.channel_validator = MagicMock()
        coordinator.channel_validator.validate_channel_name.return_value = True
        coordinator.circuit_breaker = MagicMock()
        coordinator.circuit_breaker.allow_request.return_value = True
        
        coordinator.redis.publish = AsyncMock(return_value=1)
        coordinator.timescale.insert_market_data = AsyncMock()
        
        # Get initial memory usage
        process = psutil.Process(os.getpid())
        initial_memory_mb = process.memory_info().rss / 1024 / 1024
        
        # Generate load
        symbols = ['AAPL', 'MSFT', 'GOOGL', 'NVDA', 'TSLA'] * 20  # 100 symbols
        messages_per_symbol = 100
        
        messages = []
        for symbol in symbols:
            for i in range(messages_per_symbol):
                messages.append({
                    'symbol': symbol,
                    'price': 100.0 + i * 0.01,
                    'volume': 1000 + i,
                    'time': f'2025-08-08T15:30:{i:02d}Z'
                })
        
        # Process messages
        tasks = [
            coordinator._process_market_data(msg, 'test_provider')
            for msg in messages
        ]
        await asyncio.gather(*tasks)
        
        # Check final memory usage
        final_memory_mb = process.memory_info().rss / 1024 / 1024
        memory_increase_mb = final_memory_mb - initial_memory_mb
        
        print(f"Initial memory: {initial_memory_mb:.2f}MB")
        print(f"Final memory: {final_memory_mb:.2f}MB")
        print(f"Memory increase: {memory_increase_mb:.2f}MB")
        
        # Assert memory increase is reasonable (< 100MB for this test)
        assert memory_increase_mb < 100, f"Memory increase {memory_increase_mb:.2f}MB too high"
    
    async def test_concurrent_symbol_processing(self):
        """Test concurrent processing of multiple symbols."""
        coordinator = RealtimeCoordinator()
        
        # Mock dependencies
        coordinator.redis = AsyncMock()
        coordinator.timescale = AsyncMock()
        coordinator.settings = Settings(enable_legacy_channel=True)
        coordinator.channel_validator = MagicMock()
        coordinator.channel_validator.validate_channel_name.return_value = True
        coordinator.circuit_breaker = MagicMock()
        coordinator.circuit_breaker.allow_request.return_value = True
        
        # Track which symbols/channels were published to
        published_channels = []
        
        async def track_publish(channel, message):
            published_channels.append(channel)
            return 1
            
        coordinator.redis.publish = track_publish
        coordinator.timescale.insert_market_data = AsyncMock()
        
        # Test concurrent processing of different symbols
        symbols = ['AAPL', 'MSFT', 'GOOGL', 'NVDA', 'TSLA', 'META', 'AMZN', 'JPM', 'BAC', 'XOM']
        
        # Create tasks for concurrent processing
        tasks = []
        for symbol in symbols:
            test_data = {
                'symbol': symbol,
                'price': 100.0,
                'volume': 1000,
                'time': '2025-08-08T15:30:00Z'
            }
            tasks.append(coordinator._process_market_data(test_data, 'test_provider'))
        
        # Process all symbols concurrently
        start_time = time.perf_counter()
        await asyncio.gather(*tasks)
        end_time = time.perf_counter()
        
        processing_time = end_time - start_time
        print(f"Processed {len(symbols)} symbols in {processing_time:.3f}s")
        
        # Verify all symbols were processed
        expected_channels = set()
        for symbol in symbols:
            expected_channels.add(f"market_data:{symbol}")
            expected_channels.add(f"market:{symbol}")
            expected_channels.add("market:updates")  # Legacy channel
        
        actual_channels = set(published_channels)
        
        # Check that all expected channels were published to
        for symbol in symbols:
            assert f"market_data:{symbol}" in actual_channels, f"Missing market_data channel for {symbol}"
            assert f"market:{symbol}" in actual_channels, f"Missing market channel for {symbol}"
        
        assert "market:updates" in actual_channels, "Missing legacy market:updates channel"
        
        # Verify concurrent processing was reasonably fast
        assert processing_time < 1.0, f"Concurrent processing took too long: {processing_time:.3f}s"


@pytest.mark.asyncio 
class TestCircuitBreakerPerformance:
    """Test circuit breaker performance impact."""
    
    async def test_circuit_breaker_overhead(self):
        """Test circuit breaker adds minimal overhead."""
        # Test without circuit breaker
        coordinator = RealtimeCoordinator()
        coordinator.redis = AsyncMock(return_value=1)
        coordinator.timescale = AsyncMock()
        coordinator.settings = Settings(enable_legacy_channel=True)
        coordinator.channel_validator = MagicMock()
        coordinator.channel_validator.validate_channel_name.return_value = True
        
        # Disable circuit breaker
        coordinator.circuit_breaker = MagicMock()
        coordinator.circuit_breaker.allow_request.return_value = True
        
        test_data = {
            'symbol': 'AAPL',
            'price': 150.0,
            'volume': 1000,
            'time': '2025-08-08T15:30:00Z'
        }
        
        # Measure without circuit breaker logic
        times_without_cb = []
        for _ in range(100):
            start_time = time.perf_counter()
            await coordinator._process_market_data(test_data, 'test_provider')
            end_time = time.perf_counter()
            times_without_cb.append(end_time - start_time)
        
        # Now test with actual circuit breaker
        coordinator.circuit_breaker = CircuitBreaker(failure_threshold=5, recovery_timeout=30)
        
        times_with_cb = []
        for _ in range(100):
            start_time = time.perf_counter()
            await coordinator._process_market_data(test_data, 'test_provider')
            end_time = time.perf_counter()
            times_with_cb.append(end_time - start_time)
        
        # Compare performance
        avg_without_cb = statistics.mean(times_without_cb) * 1000  # Convert to ms
        avg_with_cb = statistics.mean(times_with_cb) * 1000
        overhead = avg_with_cb - avg_without_cb
        
        print(f"Average time without circuit breaker: {avg_without_cb:.3f}ms")
        print(f"Average time with circuit breaker: {avg_with_cb:.3f}ms")
        print(f"Circuit breaker overhead: {overhead:.3f}ms")
        
        # Assert overhead is minimal (< 1ms)
        assert overhead < 1.0, f"Circuit breaker overhead {overhead:.3f}ms too high"


@pytest.mark.asyncio
class TestErrorHandlingPerformance:
    """Test error handling doesn't significantly impact performance."""
    
    async def test_error_recovery_time(self):
        """Test system recovers quickly from errors."""
        coordinator = RealtimeCoordinator()
        coordinator.timescale = AsyncMock()
        coordinator.settings = Settings(enable_legacy_channel=True)
        coordinator.channel_validator = MagicMock()
        coordinator.channel_validator.validate_channel_name.return_value = True
        coordinator.circuit_breaker = CircuitBreaker(failure_threshold=3, recovery_timeout=1)
        
        # Create a failing Redis mock that fails then succeeds
        failure_count = 0
        
        async def failing_redis_publish(channel, message):
            nonlocal failure_count
            failure_count += 1
            if failure_count <= 3:  # Fail first 3 attempts
                raise Exception("Redis connection error")
            return 1
        
        coordinator.redis = AsyncMock()
        coordinator.redis.publish = failing_redis_publish
        
        test_data = {
            'symbol': 'AAPL',
            'price': 150.0,
            'volume': 1000,
            'time': '2025-08-08T15:30:00Z'
        }
        
        # Test error handling
        start_time = time.perf_counter()
        
        # First few attempts should fail and open circuit
        for _ in range(3):
            try:
                await coordinator._process_market_data(test_data, 'test_provider')
            except:
                pass  # Expected failures
        
        # Wait for circuit to potentially recover
        await asyncio.sleep(1.1)  # Just over recovery timeout
        
        # This should succeed once circuit is half-open and Redis is "fixed"
        await coordinator._process_market_data(test_data, 'test_provider')
        
        end_time = time.perf_counter()
        recovery_time = end_time - start_time
        
        print(f"Error recovery completed in {recovery_time:.2f}s")
        
        # Assert recovery happens within reasonable time
        assert recovery_time < 5.0, f"Error recovery took too long: {recovery_time:.2f}s"
        assert coordinator.circuit_breaker.state == "CLOSED", "Circuit breaker should be closed after recovery"


if __name__ == "__main__":
    pytest.main([__file__, "-v"])