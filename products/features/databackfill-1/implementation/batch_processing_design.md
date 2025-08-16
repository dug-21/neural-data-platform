# Batch Processing System Design

## Overview
High-performance batch processing system for parsing and transforming Polygon.io historical data files.

## Core Components

### 1. File Format Handlers
```python
import gzip
import json
from typing import Iterator, Dict, Any, Optional, List
from pathlib import Path
import pandas as pd
import pyarrow.parquet as pq
from abc import ABC, abstractmethod
from datetime import datetime
import numpy as np

class FileFormatHandler(ABC):
    """Base class for different file format handlers"""
    
    @abstractmethod
    async def parse_file(self, file_path: Path) -> Iterator[Dict[str, Any]]:
        """Parse file and yield records"""
        pass
        
    @abstractmethod
    def validate_record(self, record: Dict[str, Any]) -> bool:
        """Validate single record"""
        pass

class JSONGzipHandler(FileFormatHandler):
    """Handler for gzipped JSON files (Polygon's primary format)"""
    
    def __init__(self, chunk_size: int = 10000):
        self.chunk_size = chunk_size
        
    async def parse_file(self, file_path: Path) -> Iterator[List[Dict[str, Any]]]:
        """Parse gzipped JSON file in chunks"""
        records_buffer = []
        
        with gzip.open(file_path, 'rt') as f:
            for line in f:
                try:
                    record = json.loads(line)
                    if self.validate_record(record):
                        records_buffer.append(self._transform_record(record))
                        
                        if len(records_buffer) >= self.chunk_size:
                            yield records_buffer
                            records_buffer = []
                except json.JSONDecodeError as e:
                    logging.warning(f"Invalid JSON in {file_path}: {e}")
                    continue
                    
        # Yield remaining records
        if records_buffer:
            yield records_buffer
            
    def _transform_record(self, record: Dict[str, Any]) -> Dict[str, Any]:
        """Transform Polygon record to internal format"""
        return {
            'symbol': record['sym'],
            'timestamp': datetime.fromtimestamp(record['t'] / 1000),  # Convert ms to datetime
            'open': float(record['o']),
            'high': float(record['h']),
            'low': float(record['l']),
            'close': float(record['c']),
            'volume': int(record['v']),
            'vwap': float(record.get('vw', 0)),
            'transactions': int(record.get('n', 0))
        }
        
    def validate_record(self, record: Dict[str, Any]) -> bool:
        """Validate Polygon record format"""
        required_fields = ['sym', 't', 'o', 'h', 'l', 'c', 'v']
        return all(field in record for field in required_fields)

class ParquetHandler(FileFormatHandler):
    """Handler for Parquet files (if Polygon provides them)"""
    
    def __init__(self, batch_size: int = 50000):
        self.batch_size = batch_size
        
    async def parse_file(self, file_path: Path) -> Iterator[pd.DataFrame]:
        """Parse Parquet file in batches"""
        parquet_file = pq.ParquetFile(file_path)
        
        for batch in parquet_file.iter_batches(batch_size=self.batch_size):
            df = batch.to_pandas()
            
            # Transform to standard format
            df = self._transform_dataframe(df)
            
            # Validate data
            df = df[df.apply(self.validate_record, axis=1)]
            
            yield df
            
    def _transform_dataframe(self, df: pd.DataFrame) -> pd.DataFrame:
        """Transform Parquet data to standard format"""
        return df.rename(columns={
            'sym': 'symbol',
            't': 'timestamp',
            'o': 'open',
            'h': 'high',
            'l': 'low',
            'c': 'close',
            'v': 'volume',
            'vw': 'vwap',
            'n': 'transactions'
        })
```

### 2. Parallel Batch Processor
```python
import asyncio
from concurrent.futures import ProcessPoolExecutor, ThreadPoolExecutor
from typing import List, Dict, Any, Callable, Optional
import multiprocessing as mp
from dataclasses import dataclass
import psutil

@dataclass
class ProcessingStats:
    files_processed: int = 0
    records_processed: int = 0
    errors: int = 0
    processing_time: float = 0
    avg_records_per_second: float = 0

class ParallelBatchProcessor:
    """
    High-performance parallel batch processor using multiprocessing
    """
    def __init__(
        self,
        max_workers: Optional[int] = None,
        max_memory_gb: float = 4.0,
        use_processes: bool = True
    ):
        self.max_workers = max_workers or mp.cpu_count()
        self.max_memory_gb = max_memory_gb
        self.use_processes = use_processes
        self.stats = ProcessingStats()
        
        # Create appropriate executor
        if use_processes:
            self.executor = ProcessPoolExecutor(max_workers=self.max_workers)
        else:
            self.executor = ThreadPoolExecutor(max_workers=self.max_workers)
            
    async def process_files_parallel(
        self,
        file_paths: List[Path],
        handler: FileFormatHandler,
        transform_fn: Optional[Callable] = None,
        progress_callback: Optional[Callable] = None
    ) -> AsyncIterator[List[Dict[str, Any]]]:
        """Process multiple files in parallel"""
        
        # Create processing tasks
        loop = asyncio.get_event_loop()
        
        # Chunk files for parallel processing
        file_chunks = self._chunk_files_by_size(file_paths)
        
        for chunk in file_chunks:
            # Process chunk in parallel
            futures = []
            for file_path in chunk:
                future = loop.run_in_executor(
                    self.executor,
                    self._process_single_file,
                    file_path,
                    handler,
                    transform_fn
                )
                futures.append(future)
                
            # Gather results
            results = await asyncio.gather(*futures, return_exceptions=True)
            
            # Yield successful results
            for result in results:
                if isinstance(result, Exception):
                    self.stats.errors += 1
                    logging.error(f"Processing error: {result}")
                else:
                    self.stats.files_processed += 1
                    self.stats.records_processed += len(result)
                    
                    if progress_callback:
                        await progress_callback(self.stats)
                        
                    yield result
                    
    def _chunk_files_by_size(self, file_paths: List[Path]) -> List[List[Path]]:
        """Chunk files to avoid memory overflow"""
        chunks = []
        current_chunk = []
        current_size = 0
        max_chunk_size = self.max_memory_gb * 1024 * 1024 * 1024  # Convert to bytes
        
        for file_path in file_paths:
            file_size = file_path.stat().st_size
            
            if current_size + file_size > max_chunk_size and current_chunk:
                chunks.append(current_chunk)
                current_chunk = []
                current_size = 0
                
            current_chunk.append(file_path)
            current_size += file_size
            
        if current_chunk:
            chunks.append(current_chunk)
            
        return chunks
        
    def _process_single_file(
        self,
        file_path: Path,
        handler: FileFormatHandler,
        transform_fn: Optional[Callable]
    ) -> List[Dict[str, Any]]:
        """Process single file (runs in separate process)"""
        all_records = []
        
        # Parse file
        for records_chunk in handler.parse_file(file_path):
            # Apply custom transformation if provided
            if transform_fn:
                records_chunk = transform_fn(records_chunk)
                
            all_records.extend(records_chunk)
            
        return all_records
```

### 3. Data Validation and Quality Checks
```python
from typing import List, Dict, Any, Tuple
import numpy as np
from datetime import datetime, timedelta

class DataValidator:
    """Comprehensive data validation for market data"""
    
    def __init__(self):
        self.validation_rules = {
            'price_range': self._validate_price_range,
            'volume': self._validate_volume,
            'timestamp': self._validate_timestamp,
            'price_consistency': self._validate_price_consistency,
            'outliers': self._detect_outliers
        }
        self.validation_stats = {
            'total_records': 0,
            'valid_records': 0,
            'invalid_records': 0,
            'validation_errors': {}
        }
        
    def validate_batch(
        self,
        records: List[Dict[str, Any]]
    ) -> Tuple[List[Dict[str, Any]], List[Dict[str, Any]]]:
        """Validate batch of records, return (valid, invalid)"""
        valid_records = []
        invalid_records = []
        
        for record in records:
            is_valid, errors = self._validate_record(record)
            
            if is_valid:
                valid_records.append(record)
                self.validation_stats['valid_records'] += 1
            else:
                record['validation_errors'] = errors
                invalid_records.append(record)
                self.validation_stats['invalid_records'] += 1
                
                # Track error types
                for error in errors:
                    self.validation_stats['validation_errors'][error] = \
                        self.validation_stats['validation_errors'].get(error, 0) + 1
                        
        self.validation_stats['total_records'] += len(records)
        
        return valid_records, invalid_records
        
    def _validate_record(self, record: Dict[str, Any]) -> Tuple[bool, List[str]]:
        """Validate single record against all rules"""
        errors = []
        
        for rule_name, rule_fn in self.validation_rules.items():
            try:
                if not rule_fn(record):
                    errors.append(rule_name)
            except Exception as e:
                errors.append(f"{rule_name}_error: {str(e)}")
                
        return len(errors) == 0, errors
        
    def _validate_price_range(self, record: Dict[str, Any]) -> bool:
        """Validate price values are within reasonable range"""
        prices = [record['open'], record['high'], record['low'], record['close']]
        
        # Check for negative prices
        if any(p < 0 for p in prices):
            return False
            
        # Check for unrealistic prices (e.g., > $1M per share)
        if any(p > 1_000_000 for p in prices):
            return False
            
        # High >= Low
        if record['high'] < record['low']:
            return False
            
        # High >= Open, Close
        if record['high'] < record['open'] or record['high'] < record['close']:
            return False
            
        # Low <= Open, Close
        if record['low'] > record['open'] or record['low'] > record['close']:
            return False
            
        return True
        
    def _validate_volume(self, record: Dict[str, Any]) -> bool:
        """Validate volume is reasonable"""
        volume = record['volume']
        
        # Volume should be non-negative
        if volume < 0:
            return False
            
        # Check for unrealistic volume (e.g., > 10 billion shares)
        if volume > 10_000_000_000:
            return False
            
        return True
        
    def _validate_timestamp(self, record: Dict[str, Any]) -> bool:
        """Validate timestamp is reasonable"""
        ts = record['timestamp']
        
        # Check if timestamp is a datetime
        if not isinstance(ts, datetime):
            return False
            
        # Check if timestamp is within reasonable range (not future, not too old)
        now = datetime.utcnow()
        if ts > now + timedelta(days=1):  # Allow 1 day for timezone differences
            return False
            
        if ts < datetime(1970, 1, 1):  # Nothing before Unix epoch
            return False
            
        return True
        
    def _validate_price_consistency(self, record: Dict[str, Any]) -> bool:
        """Validate OHLC consistency"""
        # VWAP should be between low and high
        if 'vwap' in record and record['vwap'] > 0:
            if record['vwap'] < record['low'] or record['vwap'] > record['high']:
                return False
                
        return True
        
    def _detect_outliers(self, record: Dict[str, Any]) -> bool:
        """Detect statistical outliers (requires historical context)"""
        # This is a simplified check - in production, you'd compare against
        # historical data for the symbol
        
        # Check for extreme price movements (> 50% in a day)
        price_range = record['high'] - record['low']
        avg_price = (record['high'] + record['low']) / 2
        
        if avg_price > 0:
            volatility = price_range / avg_price
            if volatility > 0.5:  # 50% intraday move
                return False
                
        return True
```

### 4. Stream Processing Pipeline
```python
class StreamProcessor:
    """
    Stream processing pipeline for continuous data flow
    """
    def __init__(
        self,
        batch_size: int = 10000,
        buffer_size: int = 100000
    ):
        self.batch_size = batch_size
        self.buffer_size = buffer_size
        self.buffer: asyncio.Queue = asyncio.Queue(maxsize=buffer_size)
        self.processors: List[Callable] = []
        
    def add_processor(self, processor: Callable) -> 'StreamProcessor':
        """Add processor to pipeline"""
        self.processors.append(processor)
        return self
        
    async def process_stream(
        self,
        input_stream: AsyncIterator[List[Dict[str, Any]]]
    ) -> AsyncIterator[List[Dict[str, Any]]]:
        """Process data stream through pipeline"""
        
        # Start consumer task
        consumer_task = asyncio.create_task(self._consume_buffer())
        
        try:
            # Feed data into buffer
            async for batch in input_stream:
                # Apply processors in sequence
                processed_batch = batch
                for processor in self.processors:
                    processed_batch = await processor(processed_batch)
                    
                # Add to buffer
                await self.buffer.put(processed_batch)
                
            # Signal end of stream
            await self.buffer.put(None)
            
            # Wait for consumer to finish
            await consumer_task
            
        except Exception as e:
            consumer_task.cancel()
            raise
            
    async def _consume_buffer(self):
        """Consume from buffer and yield batches"""
        current_batch = []
        
        while True:
            item = await self.buffer.get()
            
            if item is None:  # End of stream
                if current_batch:
                    yield current_batch
                break
                
            current_batch.extend(item)
            
            if len(current_batch) >= self.batch_size:
                yield current_batch[:self.batch_size]
                current_batch = current_batch[self.batch_size:]
```

### 5. Integration Pipeline
```python
async def create_processing_pipeline(
    file_paths: List[Path],
    db_pool: OptimizedDatabasePool
) -> Dict[str, Any]:
    """Create complete processing pipeline"""
    
    # Initialize components
    processor = ParallelBatchProcessor(max_workers=8, max_memory_gb=4.0)
    validator = DataValidator()
    handler = JSONGzipHandler(chunk_size=10000)
    
    # Create stream processor with pipeline
    stream = StreamProcessor(batch_size=50000)
    stream.add_processor(validator.validate_batch) \
          .add_processor(enrich_with_metadata) \
          .add_processor(calculate_derived_metrics)
    
    # Process files
    total_stats = {
        'files_processed': 0,
        'records_processed': 0,
        'records_inserted': 0,
        'validation_errors': 0,
        'processing_time': 0
    }
    
    start_time = datetime.utcnow()
    
    # Process in parallel batches
    async for batch in processor.process_files_parallel(file_paths, handler):
        # Validate batch
        valid_records, invalid_records = validator.validate_batch(batch)
        
        if invalid_records:
            # Log invalid records for review
            await log_invalid_records(invalid_records)
            total_stats['validation_errors'] += len(invalid_records)
            
        if valid_records:
            # Insert into database
            result = await db_bulk_insert(valid_records, db_pool)
            total_stats['records_inserted'] += result['records_inserted']
            
        total_stats['records_processed'] += len(batch)
        
    total_stats['processing_time'] = (datetime.utcnow() - start_time).total_seconds()
    total_stats['files_processed'] = processor.stats.files_processed
    
    return total_stats

async def enrich_with_metadata(records: List[Dict[str, Any]]) -> List[Dict[str, Any]]:
    """Enrich records with additional metadata"""
    for record in records:
        # Add processing metadata
        record['processed_at'] = datetime.utcnow()
        record['data_source'] = 'polygon_s3'
        record['backfill_version'] = '1.0'
        
    return records

async def calculate_derived_metrics(records: List[Dict[str, Any]]) -> List[Dict[str, Any]]:
    """Calculate additional metrics"""
    for record in records:
        # Calculate typical price
        record['typical_price'] = (record['high'] + record['low'] + record['close']) / 3
        
        # Calculate true range (if we have previous close)
        # This would need access to previous data in production
        record['range'] = record['high'] - record['low']
        
    return records
```

## Performance Optimization

1. **Memory Management**: Process files in chunks to avoid OOM
2. **Parallel Processing**: Use multiprocessing for CPU-bound parsing
3. **Async I/O**: Use asyncio for I/O-bound operations
4. **Buffer Management**: Use queues to smooth data flow
5. **Batch Sizing**: Optimize batch sizes based on available memory
6. **Data Validation**: Validate early to avoid processing bad data
7. **Compression**: Keep files compressed until processing