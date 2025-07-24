"""
Performance tests for high-volume data loading
"""
import pytest
import asyncio
import time
import psutil
import gc
from datetime import datetime, timedelta
from typing import List
import numpy as np
from unittest.mock import Mock, AsyncMock, patch

from data_ingestion.providers.historical_backfill import (
    HistoricalBackfillCoordinator, BackfillJob, BackfillPriority, 
    DataGranularity, BackfillStatus
)
from data_ingestion.providers.base import MarketData
from tests.mocks.market_data_generator import MarketDataGenerator


class TestLoadPerformance:
    """Test suite for load performance"""
    
    @pytest.fixture
    def coordinator(self):
        """Create coordinator with mocked dependencies"""
        with patch('data_ingestion.providers.historical_backfill.TimescaleDB'):
            coordinator = HistoricalBackfillCoordinator()
            # Mock storage methods
            coordinator.storage.store_market_data = AsyncMock()
            coordinator.storage.batch_insert = AsyncMock()
            return coordinator
    
    @pytest.fixture
    def large_dataset(self) -> List[MarketData]:
        """Generate large dataset for performance testing"""
        # Generate 1M+ data points
        data = []
        symbols = ['AAPL', 'GOOGL', 'MSFT', 'TSLA', 'NVDA']
        start_date = datetime.now() - timedelta(days=30)
        
        for symbol in symbols:
            # Generate minute data for 30 days (~200K points per symbol)
            symbol_data = MarketDataGenerator.generate_ohlcv_data(
                symbol=symbol,
                start_date=start_date,
                end_date=datetime.now(),
                interval='1min',
                realistic=False  # Simple generation for performance test
            )
            data.extend(symbol_data)
        
        return data
    
    @pytest.mark.performance
    @pytest.mark.asyncio
    async def test_high_volume_ingestion(self, coordinator, large_dataset):
        """Test ingestion of 1M+ data points"""
        # Performance targets
        target_points_per_second = 10000
        max_memory_mb = 2048
        
        # Monitor initial state
        process = psutil.Process()
        initial_memory = process.memory_info().rss / 1024 / 1024  # MB
        
        # Start timing
        start_time = time.time()
        
        # Process data in batches
        batch_size = 10000
        total_points = len(large_dataset)
        points_processed = 0
        
        for i in range(0, total_points, batch_size):
            batch = large_dataset[i:i + batch_size]
            
            # Simulate validation
            validation_result = await coordinator.validate_data(
                batch, DataGranularity.MINUTE
            )
            
            # Simulate storage (mocked)
            await coordinator.storage.batch_insert(batch)
            
            points_processed += len(batch)
            
            # Check memory usage
            current_memory = process.memory_info().rss / 1024 / 1024
            memory_increase = current_memory - initial_memory
            
            # Log progress every 100k points
            if points_processed % 100000 == 0:
                elapsed = time.time() - start_time
                rate = points_processed / elapsed
                print(f"Processed {points_processed:,} points at {rate:.0f} points/sec, "
                      f"Memory: {current_memory:.0f}MB (+{memory_increase:.0f}MB)")
        
        # Calculate final metrics
        total_time = time.time() - start_time
        actual_rate = total_points / total_time
        final_memory = process.memory_info().rss / 1024 / 1024
        peak_memory_increase = final_memory - initial_memory
        
        # Assertions
        assert actual_rate >= target_points_per_second, \
            f"Ingestion rate {actual_rate:.0f} below target {target_points_per_second}"
        
        assert peak_memory_increase <= max_memory_mb, \
            f"Memory usage {peak_memory_increase:.0f}MB exceeds limit {max_memory_mb}MB"
        
        # Performance report
        report = {
            'total_points': total_points,
            'total_time_seconds': round(total_time, 2),
            'points_per_second': round(actual_rate, 0),
            'initial_memory_mb': round(initial_memory, 0),
            'peak_memory_mb': round(final_memory, 0),
            'memory_increase_mb': round(peak_memory_increase, 0),
            'memory_per_million_points': round(peak_memory_increase / (total_points / 1_000_000), 0)
        }
        
        print(f"\nPerformance Report: {report}")
    
    @pytest.mark.performance
    @pytest.mark.asyncio
    async def test_concurrent_job_scaling(self, coordinator):
        """Test performance with 10+ concurrent jobs"""
        # Create multiple backfill jobs
        symbols = ['AAPL', 'GOOGL', 'MSFT', 'TSLA', 'NVDA', 
                   'AMD', 'INTC', 'META', 'AMZN', 'NFLX',
                   'ORCL', 'IBM', 'CSCO', 'ADBE', 'CRM']
        
        jobs = []
        for symbol in symbols:
            job = BackfillJob(
                job_id=f"perf-{symbol}",
                symbol=symbol,
                provider='yahoo',
                start_date=datetime.now() - timedelta(days=7),
                end_date=datetime.now(),
                priority=BackfillPriority.HIGH,
                granularity=DataGranularity.MINUTE
            )
            jobs.append(job)
        
        # Mock provider responses
        for provider_name, provider in coordinator.providers.items():
            provider.get_market_data = AsyncMock(
                side_effect=self._mock_provider_data_generator
            )
        
        # Test different concurrency levels
        concurrency_levels = [1, 5, 10, 15]
        results = {}
        
        for concurrency in concurrency_levels:
            gc.collect()  # Clean up before test
            
            start_time = time.time()
            coordinator.jobs = jobs.copy()
            
            # Execute with specified concurrency
            await coordinator.execute_backfill(max_concurrent=concurrency)
            
            elapsed = time.time() - start_time
            jobs_per_second = len(jobs) / elapsed
            
            results[concurrency] = {
                'elapsed_seconds': round(elapsed, 2),
                'jobs_per_second': round(jobs_per_second, 2)
            }
            
            print(f"Concurrency {concurrency}: {elapsed:.2f}s "
                  f"({jobs_per_second:.2f} jobs/sec)")
        
        # Verify scaling efficiency
        # Should see improvement up to a point
        assert results[5]['jobs_per_second'] > results[1]['jobs_per_second'] * 1.5
        assert results[10]['jobs_per_second'] > results[1]['jobs_per_second'] * 2.0
    
    @pytest.mark.performance
    @pytest.mark.asyncio
    async def test_memory_usage_under_load(self, coordinator):
        """Test memory consumption during heavy load"""
        process = psutil.Process()
        memory_samples = []
        
        # Generate continuous load
        async def continuous_load():
            for i in range(100):
                # Generate batch of data
                data = MarketDataGenerator.generate_ohlcv_data(
                    symbol='AAPL',
                    start_date=datetime.now() - timedelta(hours=1),
                    end_date=datetime.now(),
                    interval='1min',
                    realistic=False
                )
                
                # Validate and "store"
                await coordinator.validate_data(data, DataGranularity.MINUTE)
                await coordinator.storage.batch_insert(data)
                
                # Sample memory
                memory_mb = process.memory_info().rss / 1024 / 1024
                memory_samples.append(memory_mb)
                
                # Small delay to simulate real processing
                await asyncio.sleep(0.1)
        
        # Run load test
        await continuous_load()
        
        # Analyze memory pattern
        memory_array = np.array(memory_samples)
        
        # Check for memory leaks (increasing trend)
        memory_trend = np.polyfit(range(len(memory_samples)), memory_samples, 1)[0]
        
        # Memory should be stable (slight increase acceptable)
        assert memory_trend < 1.0, f"Memory leak detected: {memory_trend:.2f} MB/iteration"
        
        # Check memory variance (should be stable)
        memory_std = np.std(memory_array)
        assert memory_std < 50, f"Memory usage unstable: std={memory_std:.2f}MB"
        
        print(f"\nMemory Statistics:")
        print(f"  Mean: {np.mean(memory_array):.0f}MB")
        print(f"  Std: {memory_std:.0f}MB")
        print(f"  Min: {np.min(memory_array):.0f}MB")
        print(f"  Max: {np.max(memory_array):.0f}MB")
        print(f"  Trend: {memory_trend:.2f}MB/iteration")
    
    @pytest.mark.performance
    @pytest.mark.asyncio
    async def test_database_write_throughput(self, coordinator):
        """Test sustained database write performance"""
        # Mock database with write latency simulation
        write_latencies = []
        
        async def mock_batch_insert(data):
            # Simulate variable database latency
            base_latency = 0.01  # 10ms base
            latency = base_latency + np.random.exponential(0.005)  # Variable component
            
            start = time.time()
            await asyncio.sleep(latency)
            write_latencies.append(time.time() - start)
            
            return len(data)
        
        coordinator.storage.batch_insert = mock_batch_insert
        
        # Test parameters
        test_duration_seconds = 30
        batch_size = 1000
        target_throughput = 50000  # points per second
        
        # Run sustained write test
        start_time = time.time()
        total_points_written = 0
        batches_written = 0
        
        while time.time() - start_time < test_duration_seconds:
            # Generate batch
            data = MarketDataGenerator.generate_ohlcv_data(
                symbol='AAPL',
                start_date=datetime.now() - timedelta(hours=1),
                end_date=datetime.now(),
                interval='1min',
                realistic=False
            )[:batch_size]
            
            # Write batch
            points = await coordinator.storage.batch_insert(data)
            total_points_written += points
            batches_written += 1
            
            # Adaptive delay to maintain throughput
            elapsed = time.time() - start_time
            current_rate = total_points_written / elapsed
            
            if current_rate > target_throughput * 1.2:
                await asyncio.sleep(0.01)  # Slow down if too fast
        
        # Calculate metrics
        total_time = time.time() - start_time
        actual_throughput = total_points_written / total_time
        avg_latency = np.mean(write_latencies) * 1000  # ms
        p95_latency = np.percentile(write_latencies, 95) * 1000  # ms
        p99_latency = np.percentile(write_latencies, 99) * 1000  # ms
        
        # Assertions
        assert actual_throughput >= target_throughput * 0.9, \
            f"Throughput {actual_throughput:.0f} below target {target_throughput}"
        
        assert p95_latency < 50, f"P95 latency {p95_latency:.1f}ms too high"
        
        # Report
        print(f"\nDatabase Write Performance:")
        print(f"  Duration: {total_time:.1f}s")
        print(f"  Total Points: {total_points_written:,}")
        print(f"  Throughput: {actual_throughput:.0f} points/sec")
        print(f"  Batches: {batches_written}")
        print(f"  Avg Latency: {avg_latency:.1f}ms")
        print(f"  P95 Latency: {p95_latency:.1f}ms")
        print(f"  P99 Latency: {p99_latency:.1f}ms")
    
    async def _mock_provider_data_generator(self, symbols, start_time, end_time, interval):
        """Mock provider data generator for testing"""
        for symbol in symbols:
            data = MarketDataGenerator.generate_ohlcv_data(
                symbol=symbol,
                start_date=start_time,
                end_date=end_time,
                interval=interval,
                realistic=False
            )
            
            for point in data:
                yield point
                # Simulate network delay
                await asyncio.sleep(0.0001)
    
    @pytest.mark.performance
    @pytest.mark.asyncio
    async def test_checkpoint_performance(self, coordinator):
        """Test checkpoint save/load performance"""
        # Create many jobs
        jobs = []
        for i in range(1000):
            job = BackfillJob(
                job_id=f"checkpoint-test-{i}",
                symbol=f"SYM{i}",
                provider='yahoo',
                start_date=datetime.now() - timedelta(days=30),
                end_date=datetime.now(),
                priority=BackfillPriority.MEDIUM,
                granularity=DataGranularity.DAY,
                progress=random.random() * 100,
                points_loaded=random.randint(0, 10000),
                checkpoint_data={'last_processed': datetime.now().isoformat()}
            )
            jobs.append(job)
        
        # Test save performance
        save_start = time.time()
        
        for job in jobs:
            await coordinator._save_checkpoint(job)
        
        save_time = time.time() - save_start
        save_rate = len(jobs) / save_time
        
        print(f"\nCheckpoint Save Performance:")
        print(f"  Jobs: {len(jobs)}")
        print(f"  Time: {save_time:.2f}s")
        print(f"  Rate: {save_rate:.0f} jobs/sec")
        
        assert save_rate > 100, f"Checkpoint save too slow: {save_rate:.0f} jobs/sec"
        
        # Test load performance
        coordinator.jobs = []
        coordinator.completed_jobs = []
        coordinator.failed_jobs = []
        
        load_start = time.time()
        coordinator._load_checkpoints()
        load_time = time.time() - load_start
        
        total_loaded = (len(coordinator.jobs) + 
                       len(coordinator.completed_jobs) + 
                       len(coordinator.failed_jobs))
        load_rate = total_loaded / load_time if load_time > 0 else 0
        
        print(f"\nCheckpoint Load Performance:")
        print(f"  Jobs: {total_loaded}")
        print(f"  Time: {load_time:.2f}s")
        print(f"  Rate: {load_rate:.0f} jobs/sec")
        
        assert load_rate > 500, f"Checkpoint load too slow: {load_rate:.0f} jobs/sec"


if __name__ == "__main__":
    pytest.main([__file__, "-v", "-m", "performance"])