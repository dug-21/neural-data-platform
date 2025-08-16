# Neural Trader Backfill Performance Optimization Plan

## Executive Summary

This document outlines a comprehensive performance optimization strategy for the Neural Trader data backfill system. Based on analysis of the current implementation, we've identified critical bottlenecks and designed solutions to achieve 10-100x performance improvements.

## Current Performance Bottlenecks

### 1. **Synchronous File Reading**
- **Issue**: Files are read entirely into memory using pandas `read_csv` with chunking
- **Impact**: Memory spikes, slow startup, inability to handle large files
- **Severity**: HIGH

### 2. **Small Default Batch Sizes**
- **Issue**: Default batch size of 1,000 records leads to excessive database round trips
- **Impact**: Network overhead, poor throughput
- **Severity**: HIGH

### 3. **No Parallel Processing**
- **Issue**: Single-threaded processing of multiple symbols and files
- **Impact**: CPU underutilization, linear scaling
- **Severity**: CRITICAL

### 4. **Inefficient Database Insertion**
- **Issue**: Individual batch inserts without proper connection pooling optimization
- **Impact**: Connection overhead, suboptimal bulk insertion
- **Severity**: MEDIUM

### 5. **Memory-Inefficient Data Structures**
- **Issue**: Full pandas DataFrames in memory, no streaming processing
- **Impact**: High memory usage, GC pressure
- **Severity**: HIGH

## Performance Optimization Strategy

### Phase 1: Batch Size Tuning (Quick Win)

#### Optimization 1.1: Dynamic Batch Sizing
```python
class DynamicBatchManager:
    """Dynamically adjust batch sizes based on performance metrics."""
    
    def __init__(self, initial_size: int = 10000):
        self.batch_size = initial_size
        self.min_size = 5000
        self.max_size = 100000
        self.performance_window = []
        self.target_latency_ms = 100
    
    def adjust_batch_size(self, records_processed: int, time_taken: float):
        """Adjust batch size based on processing performance."""
        throughput = records_processed / time_taken
        latency_ms = (time_taken / records_processed) * 1000
        
        if latency_ms > self.target_latency_ms * 1.5:
            # Reduce batch size if latency too high
            self.batch_size = max(self.min_size, int(self.batch_size * 0.8))
        elif latency_ms < self.target_latency_ms * 0.5:
            # Increase batch size if performing well
            self.batch_size = min(self.max_size, int(self.batch_size * 1.2))
        
        return self.batch_size
```

#### Optimization 1.2: Optimal Batch Size Configuration
```python
# Recommended batch sizes by data type
BATCH_SIZE_CONFIG = {
    'market_data': 50000,      # OHLCV data
    'tick_data': 100000,       # High-frequency tick data
    'order_book': 25000,       # Order book snapshots
    'large_backfill': 100000,  # Historical backfill
    'realtime': 5000          # Real-time ingestion
}
```

### Phase 2: Parallel Processing Implementation

#### Optimization 2.1: Multi-Process File Processing
```python
import multiprocessing as mp
from concurrent.futures import ProcessPoolExecutor, as_completed
import asyncio

class ParallelFileProcessor:
    """Process multiple files in parallel using multiprocessing."""
    
    def __init__(self, num_workers: int = None):
        self.num_workers = num_workers or mp.cpu_count()
        
    async def process_files_parallel(self, file_paths: List[Path]) -> Dict[str, Any]:
        """Process multiple files in parallel."""
        # Divide files among workers
        chunks = [file_paths[i::self.num_workers] for i in range(self.num_workers)]
        
        loop = asyncio.get_event_loop()
        with ProcessPoolExecutor(max_workers=self.num_workers) as executor:
            futures = []
            
            for worker_id, file_chunk in enumerate(chunks):
                future = loop.run_in_executor(
                    executor,
                    self._process_file_chunk,
                    worker_id,
                    file_chunk
                )
                futures.append(future)
            
            # Gather results
            results = await asyncio.gather(*futures)
            
        return self._merge_results(results)
    
    def _process_file_chunk(self, worker_id: int, files: List[Path]) -> Dict:
        """Process a chunk of files in a worker process."""
        # Each worker processes its files independently
        results = {
            'worker_id': worker_id,
            'files_processed': 0,
            'records_processed': 0,
            'errors': []
        }
        
        for file_path in files:
            try:
                records = self._process_single_file(file_path)
                results['records_processed'] += records
                results['files_processed'] += 1
            except Exception as e:
                results['errors'].append(f"{file_path}: {str(e)}")
        
        return results
```

#### Optimization 2.2: Concurrent Symbol Processing
```python
class ConcurrentSymbolProcessor:
    """Process multiple symbols concurrently within a single file."""
    
    def __init__(self, max_concurrent: int = 10):
        self.semaphore = asyncio.Semaphore(max_concurrent)
        
    async def process_symbols_concurrent(self, 
                                       data: pd.DataFrame,
                                       storage: TimescaleDB) -> int:
        """Process symbols concurrently with controlled parallelism."""
        # Group data by symbol
        symbol_groups = data.groupby('symbol')
        
        tasks = []
        for symbol, group_data in symbol_groups:
            task = self._process_symbol_data(symbol, group_data, storage)
            tasks.append(task)
        
        # Process all symbols concurrently
        results = await asyncio.gather(*tasks, return_exceptions=True)
        
        # Count successful records
        total_records = sum(r for r in results if isinstance(r, int))
        return total_records
    
    async def _process_symbol_data(self, 
                                  symbol: str, 
                                  data: pd.DataFrame,
                                  storage: TimescaleDB) -> int:
        """Process data for a single symbol with semaphore control."""
        async with self.semaphore:
            # Convert to records
            records = data.to_dict('records')
            
            # Bulk insert
            await storage.insert_market_data(records)
            
            return len(records)
```

### Phase 3: Memory Management Improvements

#### Optimization 3.1: Streaming CSV Reader
```python
import csv
from typing import AsyncIterator, Dict, Any

class StreamingCSVReader:
    """Memory-efficient streaming CSV reader."""
    
    def __init__(self, file_path: Path, batch_size: int = 50000):
        self.file_path = file_path
        self.batch_size = batch_size
        
    async def read_batches(self) -> AsyncIterator[List[Dict[str, Any]]]:
        """Read CSV file in streaming batches."""
        import aiofiles
        
        async with aiofiles.open(self.file_path, mode='r') as f:
            # Read header
            header_line = await f.readline()
            headers = header_line.strip().split(',')
            
            batch = []
            async for line in f:
                # Parse line
                values = line.strip().split(',')
                record = dict(zip(headers, values))
                
                # Type conversion
                record = self._convert_types(record)
                
                batch.append(record)
                
                if len(batch) >= self.batch_size:
                    yield batch
                    batch = []
            
            # Yield remaining records
            if batch:
                yield batch
    
    def _convert_types(self, record: Dict[str, str]) -> Dict[str, Any]:
        """Convert string values to appropriate types."""
        # Timestamp
        if 'timestamp' in record:
            record['timestamp'] = pd.to_datetime(record['timestamp'])
        
        # Numeric fields
        numeric_fields = ['open', 'high', 'low', 'close', 'volume']
        for field in numeric_fields:
            if field in record:
                record[field] = float(record[field]) if field != 'volume' else int(record[field])
        
        return record
```

#### Optimization 3.2: Generator-Based Processing
```python
from typing import Generator, Tuple

class GeneratorPipeline:
    """Memory-efficient generator-based processing pipeline."""
    
    def __init__(self):
        self.filters = []
        self.transformers = []
        
    def process_file_generator(self, file_path: Path) -> Generator[Dict, None, None]:
        """Process file using generators for minimal memory usage."""
        with open(file_path, 'r') as f:
            reader = csv.DictReader(f)
            
            for row in reader:
                # Apply filters
                if not self._apply_filters(row):
                    continue
                
                # Apply transformations
                row = self._apply_transformations(row)
                
                yield row
    
    def batch_generator(self, 
                       generator: Generator,
                       batch_size: int) -> Generator[List[Dict], None, None]:
        """Create batches from a generator."""
        batch = []
        
        for item in generator:
            batch.append(item)
            
            if len(batch) >= batch_size:
                yield batch
                batch = []
        
        # Yield remaining items
        if batch:
            yield batch
```

### Phase 4: Database Optimization

#### Optimization 4.1: Bulk COPY Operations
```python
import io
from contextlib import asynccontextmanager

class BulkDatabaseWriter:
    """Optimized bulk database writer using COPY operations."""
    
    def __init__(self, storage: TimescaleDB):
        self.storage = storage
        
    async def bulk_copy_insert(self, table: str, records: List[Dict]) -> int:
        """Use PostgreSQL COPY for ultra-fast bulk inserts."""
        if not records:
            return 0
        
        # Create in-memory CSV
        output = io.StringIO()
        
        # Write headers
        headers = list(records[0].keys())
        
        # Write data
        for record in records:
            row = [str(record.get(h, '')) for h in headers]
            output.write('\t'.join(row) + '\n')
        
        output.seek(0)
        
        # Use COPY command
        async with self.storage.acquire() as conn:
            result = await conn.copy_to_table(
                table,
                source=output,
                columns=headers,
                format='text'
            )
        
        return len(records)
```

#### Optimization 4.2: Connection Pool Optimization
```python
class OptimizedConnectionPool:
    """Optimized connection pool configuration for bulk operations."""
    
    @staticmethod
    def create_pool_config(workload_type: str) -> Dict[str, Any]:
        """Create optimized pool configuration based on workload."""
        configs = {
            'bulk_insert': {
                'min_size': 5,
                'max_size': 20,
                'max_queries': 50000,
                'max_inactive_connection_lifetime': 300,
                'command_timeout': 300,
                'statement_cache_size': 0  # Disable for bulk ops
            },
            'streaming': {
                'min_size': 10,
                'max_size': 50,
                'max_queries': 10000,
                'max_inactive_connection_lifetime': 60,
                'command_timeout': 60
            },
            'mixed': {
                'min_size': 5,
                'max_size': 30,
                'max_queries': 25000,
                'max_inactive_connection_lifetime': 180,
                'command_timeout': 120
            }
        }
        
        return configs.get(workload_type, configs['mixed'])
```

### Phase 5: Progress Tracking Without Overhead

#### Optimization 5.1: Lightweight Progress Tracker
```python
import time
from collections import deque

class LightweightProgressTracker:
    """Minimal overhead progress tracking."""
    
    def __init__(self, report_interval: int = 10000):
        self.report_interval = report_interval
        self.total_processed = 0
        self.last_report = 0
        self.start_time = time.time()
        self.throughput_window = deque(maxlen=100)
        
    def update(self, records: int) -> Optional[Dict[str, Any]]:
        """Update progress with minimal overhead."""
        self.total_processed += records
        
        # Only calculate metrics at intervals
        if self.total_processed - self.last_report >= self.report_interval:
            metrics = self._calculate_metrics()
            self.last_report = self.total_processed
            return metrics
        
        return None
    
    def _calculate_metrics(self) -> Dict[str, Any]:
        """Calculate performance metrics."""
        elapsed = time.time() - self.start_time
        throughput = self.total_processed / elapsed if elapsed > 0 else 0
        
        self.throughput_window.append(throughput)
        avg_throughput = sum(self.throughput_window) / len(self.throughput_window)
        
        return {
            'total_processed': self.total_processed,
            'elapsed_seconds': elapsed,
            'current_throughput': throughput,
            'avg_throughput': avg_throughput,
            'estimated_memory_mb': self._estimate_memory_usage()
        }
    
    def _estimate_memory_usage(self) -> float:
        """Estimate current memory usage."""
        import psutil
        process = psutil.Process()
        return process.memory_info().rss / 1024 / 1024
```

## Implementation Patterns

### Pattern 1: Async/Await Optimization
```python
class AsyncBatchProcessor:
    """Optimized async batch processing pattern."""
    
    async def process_with_pipeline(self, file_path: Path):
        """Process file with async pipeline."""
        reader = StreamingCSVReader(file_path)
        batch_manager = DynamicBatchManager()
        progress = LightweightProgressTracker()
        
        # Create processing pipeline
        async for batch in reader.read_batches():
            # Process batch asynchronously
            start_time = time.time()
            
            # Parallel symbol processing
            processor = ConcurrentSymbolProcessor()
            records_processed = await processor.process_symbols_concurrent(
                pd.DataFrame(batch),
                self.storage
            )
            
            # Update batch size based on performance
            elapsed = time.time() - start_time
            new_size = batch_manager.adjust_batch_size(records_processed, elapsed)
            reader.batch_size = new_size
            
            # Track progress
            if metrics := progress.update(records_processed):
                logger.info(f"Progress: {metrics}")
```

### Pattern 2: Memory-Efficient Data Structures
```python
import numpy as np
from dataclasses import dataclass
from array import array

@dataclass
class MemoryEfficientMarketData:
    """Memory-efficient market data structure using numpy arrays."""
    
    def __init__(self, capacity: int = 100000):
        # Use numpy arrays for numeric data
        self.timestamps = np.empty(capacity, dtype='datetime64[ns]')
        self.open_prices = np.empty(capacity, dtype=np.float32)
        self.high_prices = np.empty(capacity, dtype=np.float32)
        self.low_prices = np.empty(capacity, dtype=np.float32)
        self.close_prices = np.empty(capacity, dtype=np.float32)
        self.volumes = np.empty(capacity, dtype=np.int64)
        
        # Use array for symbol indices
        self.symbol_indices = array('i', [0] * capacity)
        
        # Symbol mapping
        self.symbol_map = {}
        self.reverse_symbol_map = []
        
        self.size = 0
        self.capacity = capacity
    
    def add_record(self, timestamp, symbol, open_p, high, low, close, volume):
        """Add a record with minimal memory allocation."""
        if self.size >= self.capacity:
            self._grow()
        
        # Get or create symbol index
        if symbol not in self.symbol_map:
            symbol_idx = len(self.reverse_symbol_map)
            self.symbol_map[symbol] = symbol_idx
            self.reverse_symbol_map.append(symbol)
        else:
            symbol_idx = self.symbol_map[symbol]
        
        # Store data
        idx = self.size
        self.timestamps[idx] = np.datetime64(timestamp)
        self.symbol_indices[idx] = symbol_idx
        self.open_prices[idx] = open_p
        self.high_prices[idx] = high
        self.low_prices[idx] = low
        self.close_prices[idx] = close
        self.volumes[idx] = volume
        
        self.size += 1
    
    def to_records(self) -> List[Dict]:
        """Convert to records for database insertion."""
        records = []
        for i in range(self.size):
            records.append({
                'time': self.timestamps[i],
                'symbol': self.reverse_symbol_map[self.symbol_indices[i]],
                'open': float(self.open_prices[i]),
                'high': float(self.high_prices[i]),
                'low': float(self.low_prices[i]),
                'close': float(self.close_prices[i]),
                'volume': int(self.volumes[i])
            })
        return records
```

## Performance Targets and Benchmarks

### Target Metrics

| Metric | Current | Target | Improvement |
|--------|---------|--------|-------------|
| Records/Second | ~5,000 | 100,000+ | 20x |
| Memory Usage (GB) | 8-16 | 2-4 | 4x reduction |
| Batch Size | 1,000 | 50,000-100,000 | 50-100x |
| Parallel Workers | 1 | CPU count | 4-16x |
| DB Round Trips | 1 per 1K records | 1 per 100K records | 100x reduction |
| File Processing | Sequential | Parallel | 4-16x |

### Benchmark Suite
```python
class BackfillBenchmark:
    """Comprehensive benchmark suite for backfill performance."""
    
    async def run_benchmarks(self):
        """Run all performance benchmarks."""
        results = {}
        
        # Test 1: Single large file processing
        results['single_file'] = await self.benchmark_single_file(
            file_size_gb=10,
            record_count=100_000_000
        )
        
        # Test 2: Multiple file parallel processing
        results['multi_file'] = await self.benchmark_multi_file(
            num_files=100,
            records_per_file=1_000_000
        )
        
        # Test 3: High symbol count
        results['high_symbol'] = await self.benchmark_symbol_count(
            num_symbols=1000,
            records_per_symbol=100_000
        )
        
        # Test 4: Memory efficiency
        results['memory'] = await self.benchmark_memory_usage(
            target_records=50_000_000,
            memory_limit_gb=4
        )
        
        return results
```

## Monitoring and Optimization Loop

### Real-time Performance Monitoring
```python
class PerformanceMonitor:
    """Real-time performance monitoring and optimization."""
    
    def __init__(self):
        self.metrics = {
            'throughput': deque(maxlen=1000),
            'latency': deque(maxlen=1000),
            'memory': deque(maxlen=1000),
            'errors': deque(maxlen=1000)
        }
        
    async def monitor_and_optimize(self, processor):
        """Monitor performance and apply optimizations."""
        while True:
            metrics = await self.collect_metrics(processor)
            
            # Analyze performance
            if self.detect_bottleneck(metrics):
                optimization = self.suggest_optimization(metrics)
                await self.apply_optimization(processor, optimization)
            
            await asyncio.sleep(1)  # Check every second
    
    def detect_bottleneck(self, metrics):
        """Detect performance bottlenecks."""
        # Check for throughput degradation
        if len(self.metrics['throughput']) > 10:
            recent_avg = sum(list(self.metrics['throughput'])[-10:]) / 10
            overall_avg = sum(self.metrics['throughput']) / len(self.metrics['throughput'])
            
            if recent_avg < overall_avg * 0.8:
                return True
        
        # Check for memory pressure
        if metrics['memory_usage_pct'] > 80:
            return True
        
        return False
```

## Rollout Plan

### Phase 1: Quick Wins (Week 1)
1. Increase default batch sizes to 50,000
2. Implement dynamic batch sizing
3. Add basic progress tracking

### Phase 2: Core Optimizations (Week 2-3)
1. Implement parallel file processing
2. Add streaming CSV reader
3. Optimize database connection pooling

### Phase 3: Advanced Features (Week 4)
1. Implement COPY-based bulk inserts
2. Add memory-efficient data structures
3. Complete performance monitoring

### Phase 4: Testing and Tuning (Week 5)
1. Run comprehensive benchmarks
2. Fine-tune parameters
3. Document optimal configurations

## Summary

This optimization plan addresses all identified bottlenecks through:

1. **Batch Size Optimization**: 50-100x larger batches with dynamic sizing
2. **Parallel Processing**: Multi-process file handling and concurrent symbol processing
3. **Memory Efficiency**: Streaming readers and optimized data structures
4. **Database Performance**: COPY operations and optimized connection pooling
5. **Intelligent Monitoring**: Real-time performance tracking and auto-optimization

Expected improvements:
- **20x throughput increase** (from 5K to 100K+ records/second)
- **4x memory reduction** (from 16GB to 4GB for large operations)
- **100x fewer database round trips**
- **Linear scalability** with CPU cores

The implementation follows a phased approach, allowing quick wins while building toward comprehensive optimization.