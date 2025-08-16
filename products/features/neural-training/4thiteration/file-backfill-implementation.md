# Enhanced File Backfill Implementation Design

## Executive Summary

This document outlines enhancements to the existing file backfill functionality in the neural-trader data ingestion system. The current implementation provides basic file loading capabilities for CSV, JSON, and Parquet formats. This design extends the system with advanced features for production-grade file-based data ingestion.

## Current State Analysis

### Existing Components

1. **FileBackfillHandler** (`data_ingestion/utils/file_backfill.py`)
   - Basic file format support (CSV, JSON, Parquet)
   - Simple batch processing
   - Redis-based checkpointing
   - Basic validation and filtering

2. **FileProvider** (`data_ingestion/providers/file_provider.py`)
   - CSV file reading with gzip support
   - Line-by-line processing
   - OHLC validation
   - File-based checkpointing

3. **BackfillCLI** (`data_ingestion/cli/backfill.py`)
   - Command-line interface
   - Multiple subcommands (file, s3, status, validate)
   - Configuration management
   - Progress tracking

### Current Limitations

1. **Performance Issues**
   - Sequential file processing
   - Limited parallelism
   - No streaming for large files in JSON/Parquet handlers

2. **Data Quality**
   - Basic validation only
   - No schema evolution handling
   - Limited error recovery

3. **Format Support**
   - Fixed column mappings
   - No flexible schema configuration
   - Limited timestamp format handling

4. **Monitoring**
   - Basic statistics only
   - No real-time progress visualization
   - Limited error diagnostics

## Enhanced Architecture

### Core Components

```
┌─────────────────────────────────────────────────────────────┐
│                    File Backfill System                      │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  ┌─────────────────┐    ┌─────────────────┐                │
│  │   CLI Interface │    │   Web Dashboard  │                │
│  └────────┬────────┘    └────────┬────────┘                │
│           │                       │                          │
│  ┌────────┴───────────────────────┴────────┐                │
│  │         Backfill Orchestrator           │                │
│  └────────┬───────────────────────┬────────┘                │
│           │                       │                          │
│  ┌────────┴────────┐     ┌───────┴────────┐                │
│  │  File Discovery │     │ Schema Manager  │                │
│  └─────────────────┘     └────────────────┘                │
│                                                              │
│  ┌─────────────────────────────────────────┐                │
│  │         Processing Pipeline              │                │
│  ├─────────────────────────────────────────┤                │
│  │                                          │                │
│  │  ┌──────────┐  ┌──────────┐  ┌────────┐│                │
│  │  │  Reader  │→ │Validator │→ │ Writer ││                │
│  │  └──────────┘  └──────────┘  └────────┘│                │
│  │       ↓              ↓            ↓      │                │
│  │  ┌──────────┐  ┌──────────┐  ┌────────┐│                │
│  │  │Checkpoint│  │  Metrics │  │Storage ││                │
│  │  └──────────┘  └──────────┘  └────────┘│                │
│  └─────────────────────────────────────────┘                │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

### Enhanced Features

## 1. Advanced File Format Support

### 1.1 Flexible Schema Configuration

```python
@dataclass
class SchemaConfig:
    """Configurable schema mapping for different file formats."""
    
    # Column mappings
    timestamp_columns: List[str] = field(default_factory=lambda: ['timestamp', 'time', 'date', 'datetime'])
    symbol_columns: List[str] = field(default_factory=lambda: ['symbol', 'ticker', 'instrument'])
    open_columns: List[str] = field(default_factory=lambda: ['open', 'open_price', 'o'])
    high_columns: List[str] = field(default_factory=lambda: ['high', 'high_price', 'h'])
    low_columns: List[str] = field(default_factory=lambda: ['low', 'low_price', 'l'])
    close_columns: List[str] = field(default_factory=lambda: ['close', 'close_price', 'c'])
    volume_columns: List[str] = field(default_factory=lambda: ['volume', 'vol', 'v'])
    
    # Timestamp parsing
    timestamp_formats: List[str] = field(default_factory=lambda: [
        '%Y-%m-%d %H:%M:%S',
        '%Y-%m-%dT%H:%M:%S',
        '%Y-%m-%dT%H:%M:%SZ',
        '%Y-%m-%d %H:%M:%S.%f',
        '%Y/%m/%d %H:%M:%S',
        'auto'  # Automatic detection
    ])
    
    # Data type hints
    column_types: Dict[str, str] = field(default_factory=dict)
    
    # Validation rules
    required_columns: List[str] = field(default_factory=lambda: ['timestamp', 'symbol'])
    nullable_columns: List[str] = field(default_factory=list)
    
    def detect_columns(self, available_columns: List[str]) -> Dict[str, str]:
        """Auto-detect column mappings from available columns."""
        mappings = {}
        
        # Helper to find best match
        def find_match(candidates: List[str], columns: List[str]) -> Optional[str]:
            for col in columns:
                if col.lower() in [c.lower() for c in candidates]:
                    return col
            return None
        
        # Map each field
        mappings['timestamp'] = find_match(available_columns, self.timestamp_columns)
        mappings['symbol'] = find_match(available_columns, self.symbol_columns)
        mappings['open'] = find_match(available_columns, self.open_columns)
        mappings['high'] = find_match(available_columns, self.high_columns)
        mappings['low'] = find_match(available_columns, self.low_columns)
        mappings['close'] = find_match(available_columns, self.close_columns)
        mappings['volume'] = find_match(available_columns, self.volume_columns)
        
        return {k: v for k, v in mappings.items() if v is not None}
```

### 1.2 Format-Specific Readers

```python
class FormatReader(ABC):
    """Abstract base class for format-specific readers."""
    
    @abstractmethod
    async def read_schema(self, file_path: Path) -> Dict[str, Any]:
        """Read and return file schema information."""
        pass
    
    @abstractmethod
    async def read_batches(
        self, 
        file_path: Path, 
        batch_size: int,
        filters: Optional[Dict[str, Any]] = None
    ) -> AsyncIterator[pd.DataFrame]:
        """Read file in batches with optional filtering."""
        pass


class ParquetReader(FormatReader):
    """Optimized Parquet file reader with predicate pushdown."""
    
    async def read_schema(self, file_path: Path) -> Dict[str, Any]:
        """Read Parquet schema without loading data."""
        import pyarrow.parquet as pq
        
        # Read schema only
        schema = pq.read_schema(file_path)
        
        return {
            'columns': schema.names,
            'types': {name: str(field.type) for name, field in zip(schema.names, schema)},
            'metadata': schema.metadata,
            'num_rows': pq.read_metadata(file_path).num_rows
        }
    
    async def read_batches(
        self, 
        file_path: Path, 
        batch_size: int,
        filters: Optional[Dict[str, Any]] = None
    ) -> AsyncIterator[pd.DataFrame]:
        """Read Parquet with predicate pushdown for efficiency."""
        import pyarrow.parquet as pq
        import pyarrow.compute as pc
        
        # Build filter expressions
        filter_expr = None
        if filters:
            if 'symbols' in filters:
                filter_expr = pc.is_in(
                    pc.field('symbol'), 
                    pa.array(filters['symbols'])
                )
            
            if 'start_time' in filters and 'timestamp' in self.schema['columns']:
                time_filter = pc.greater_equal(
                    pc.field('timestamp'),
                    pc.timestamp(filters['start_time'])
                )
                filter_expr = filter_expr & time_filter if filter_expr else time_filter
        
        # Read with filters
        table = pq.read_table(
            file_path,
            filters=filter_expr,
            memory_map=True,  # Memory-efficient reading
            use_threads=True
        )
        
        # Convert to batches
        for batch in table.to_batches(batch_size):
            yield batch.to_pandas()


class CSVReader(FormatReader):
    """Enhanced CSV reader with automatic delimiter detection."""
    
    async def detect_delimiter(self, file_path: Path) -> str:
        """Automatically detect CSV delimiter."""
        import csv
        
        with open(file_path, 'r', encoding='utf-8') as f:
            # Read sample
            sample = f.read(8192)
            
            # Try to detect delimiter
            sniffer = csv.Sniffer()
            try:
                dialect = sniffer.sniff(sample)
                return dialect.delimiter
            except:
                # Fallback to common delimiters
                for delim in [',', '\t', '|', ';']:
                    if delim in sample:
                        return delim
                return ','
    
    async def read_batches(
        self, 
        file_path: Path, 
        batch_size: int,
        filters: Optional[Dict[str, Any]] = None
    ) -> AsyncIterator[pd.DataFrame]:
        """Stream CSV file in chunks."""
        delimiter = await self.detect_delimiter(file_path)
        
        # Determine if compressed
        compression = None
        if file_path.suffix == '.gz':
            compression = 'gzip'
        elif file_path.suffix == '.bz2':
            compression = 'bz2'
        elif file_path.suffix == '.xz':
            compression = 'xz'
        
        # Read in chunks
        for chunk in pd.read_csv(
            file_path,
            chunksize=batch_size,
            delimiter=delimiter,
            compression=compression,
            parse_dates=['timestamp'],  # Will be mapped later
            infer_datetime_format=True
        ):
            yield chunk
```

## 2. Streaming Processing Pipeline

### 2.1 Async Pipeline Architecture

```python
class ProcessingPipeline:
    """Async streaming pipeline for file processing."""
    
    def __init__(self, max_workers: int = 4):
        self.max_workers = max_workers
        self.queue_size = max_workers * 2
        self.stats = PipelineStats()
        
    async def process_files(
        self,
        files: List[Path],
        reader: FormatReader,
        validator: DataValidator,
        writer: DataWriter,
        progress_callback: Optional[Callable] = None
    ):
        """Process multiple files with parallel pipeline stages."""
        
        # Create queues for pipeline stages
        read_queue = asyncio.Queue(maxsize=self.queue_size)
        validate_queue = asyncio.Queue(maxsize=self.queue_size)
        write_queue = asyncio.Queue(maxsize=self.queue_size)
        
        # Start pipeline workers
        tasks = []
        
        # File readers
        for i in range(min(len(files), self.max_workers)):
            tasks.append(
                asyncio.create_task(
                    self._read_worker(files[i::self.max_workers], reader, read_queue)
                )
            )
        
        # Validators
        for i in range(self.max_workers):
            tasks.append(
                asyncio.create_task(
                    self._validate_worker(read_queue, validate_queue, validator)
                )
            )
        
        # Writers
        for i in range(self.max_workers // 2):  # Fewer writers needed
            tasks.append(
                asyncio.create_task(
                    self._write_worker(validate_queue, write_queue, writer)
                )
            )
        
        # Progress monitor
        if progress_callback:
            tasks.append(
                asyncio.create_task(
                    self._progress_monitor(write_queue, progress_callback)
                )
            )
        
        # Wait for completion
        await asyncio.gather(*tasks)
    
    async def _read_worker(self, files: List[Path], reader: FormatReader, output_queue: asyncio.Queue):
        """Worker to read files and push to queue."""
        for file_path in files:
            try:
                async for batch in reader.read_batches(file_path, batch_size=10000):
                    await output_queue.put(('batch', file_path, batch))
                    self.stats.batches_read += 1
            except Exception as e:
                await output_queue.put(('error', file_path, e))
                self.stats.read_errors += 1
        
        # Signal completion
        await output_queue.put(('done', None, None))
```

### 2.2 Advanced Validation

```python
class DataValidator:
    """Comprehensive data validation with auto-correction."""
    
    def __init__(self, config: ValidationConfig):
        self.config = config
        self.validation_stats = defaultdict(int)
        
    async def validate_batch(self, batch: pd.DataFrame) -> Tuple[pd.DataFrame, List[Dict]]:
        """Validate and potentially correct a batch of data."""
        issues = []
        
        # Schema validation
        batch, schema_issues = await self._validate_schema(batch)
        issues.extend(schema_issues)
        
        # Data quality checks
        batch, quality_issues = await self._validate_quality(batch)
        issues.extend(quality_issues)
        
        # Business rules
        batch, rule_issues = await self._validate_business_rules(batch)
        issues.extend(rule_issues)
        
        return batch, issues
    
    async def _validate_quality(self, batch: pd.DataFrame) -> Tuple[pd.DataFrame, List[Dict]]:
        """Perform data quality validation."""
        issues = []
        
        # Check for outliers using IQR method
        numeric_columns = ['open', 'high', 'low', 'close', 'volume']
        for col in numeric_columns:
            if col in batch.columns:
                Q1 = batch[col].quantile(0.25)
                Q3 = batch[col].quantile(0.75)
                IQR = Q3 - Q1
                
                lower_bound = Q1 - self.config.outlier_iqr_multiplier * IQR
                upper_bound = Q3 + self.config.outlier_iqr_multiplier * IQR
                
                outliers = (batch[col] < lower_bound) | (batch[col] > upper_bound)
                
                if outliers.any():
                    outlier_count = outliers.sum()
                    issues.append({
                        'type': 'outlier',
                        'column': col,
                        'count': outlier_count,
                        'severity': 'warning'
                    })
                    
                    if self.config.handle_outliers == 'cap':
                        # Cap outliers to bounds
                        batch.loc[batch[col] < lower_bound, col] = lower_bound
                        batch.loc[batch[col] > upper_bound, col] = upper_bound
                    elif self.config.handle_outliers == 'remove':
                        # Remove outlier rows
                        batch = batch[~outliers]
        
        # Check for duplicates
        duplicate_cols = ['timestamp', 'symbol']
        duplicates = batch.duplicated(subset=duplicate_cols, keep='first')
        
        if duplicates.any():
            duplicate_count = duplicates.sum()
            issues.append({
                'type': 'duplicate',
                'count': duplicate_count,
                'severity': 'warning'
            })
            
            # Remove duplicates, keeping first occurrence
            batch = batch[~duplicates]
        
        return batch, issues
```

## 3. Progress Tracking and Monitoring

### 3.1 Real-time Progress Dashboard

```python
class ProgressTracker:
    """Real-time progress tracking with web dashboard."""
    
    def __init__(self):
        self.start_time = datetime.now()
        self.stats = {
            'files_total': 0,
            'files_completed': 0,
            'records_processed': 0,
            'records_failed': 0,
            'bytes_processed': 0,
            'current_file': None,
            'current_speed': 0,
            'eta': None
        }
        self.history = deque(maxlen=1000)  # Keep last 1000 updates
        
    async def update_progress(self, update: Dict[str, Any]):
        """Update progress statistics."""
        self.stats.update(update)
        
        # Calculate speed
        elapsed = (datetime.now() - self.start_time).total_seconds()
        if elapsed > 0:
            self.stats['current_speed'] = self.stats['records_processed'] / elapsed
            
            # Estimate ETA
            if self.stats['files_total'] > 0:
                progress_pct = self.stats['files_completed'] / self.stats['files_total']
                if progress_pct > 0:
                    total_time = elapsed / progress_pct
                    remaining_time = total_time - elapsed
                    self.stats['eta'] = datetime.now() + timedelta(seconds=remaining_time)
        
        # Add to history
        self.history.append({
            'timestamp': datetime.now(),
            'stats': self.stats.copy()
        })
        
        # Broadcast to dashboard
        await self._broadcast_update()
    
    def get_dashboard_data(self) -> Dict[str, Any]:
        """Get data for dashboard display."""
        return {
            'current': self.stats,
            'history': list(self.history),
            'summary': {
                'duration': str(datetime.now() - self.start_time),
                'success_rate': (
                    self.stats['records_processed'] / 
                    (self.stats['records_processed'] + self.stats['records_failed'])
                    if self.stats['records_processed'] > 0 else 0
                ),
                'files_per_hour': (
                    self.stats['files_completed'] / 
                    ((datetime.now() - self.start_time).total_seconds() / 3600)
                    if self.stats['files_completed'] > 0 else 0
                )
            }
        }
```

### 3.2 CLI Progress Display

```python
class CLIProgress:
    """Rich CLI progress display with multiple progress bars."""
    
    def __init__(self):
        self.console = Console()
        self.progress = Progress(
            SpinnerColumn(),
            TextColumn("[progress.description]{task.description}"),
            BarColumn(),
            TextColumn("[progress.percentage]{task.percentage:>3.0f}%"),
            TimeElapsedColumn(),
            TimeRemainingColumn(),
            TextColumn("{task.fields[speed]:.0f} rec/s"),
            console=self.console,
            refresh_per_second=2
        )
        self.tasks = {}
        
    def add_file_task(self, file_path: Path, total_records: int) -> int:
        """Add a file processing task."""
        task_id = self.progress.add_task(
            f"[cyan]{file_path.name}",
            total=total_records,
            speed=0
        )
        self.tasks[str(file_path)] = task_id
        return task_id
    
    def update_task(self, file_path: Path, completed: int, speed: float):
        """Update task progress."""
        task_id = self.tasks.get(str(file_path))
        if task_id:
            self.progress.update(
                task_id,
                completed=completed,
                speed=speed
            )
    
    def create_summary_table(self, stats: Dict[str, Any]) -> Table:
        """Create summary statistics table."""
        table = Table(title="Backfill Summary", show_header=True)
        
        table.add_column("Metric", style="cyan")
        table.add_column("Value", justify="right")
        
        table.add_row("Total Files", f"{stats['files_total']:,}")
        table.add_row("Completed Files", f"{stats['files_completed']:,}")
        table.add_row("Total Records", f"{stats['records_processed']:,}")
        table.add_row("Failed Records", f"{stats['records_failed']:,}")
        table.add_row("Success Rate", f"{stats['success_rate']:.1%}")
        table.add_row("Processing Speed", f"{stats['current_speed']:.0f} rec/s")
        
        if stats.get('eta'):
            table.add_row("ETA", stats['eta'].strftime('%Y-%m-%d %H:%M:%S'))
        
        return table
```

## 4. Enhanced CLI Interface

### 4.1 New CLI Commands

```bash
# Enhanced file import with schema detection
backfill file \
  --path /mnt/data \
  --format auto \
  --schema-config schema.yaml \
  --parallel 4 \
  --validate strict \
  --monitor http://localhost:8080

# Preview mode with data profiling
backfill preview \
  --path /mnt/data/sample.csv \
  --rows 1000 \
  --profile \
  --detect-schema

# Schema management
backfill schema \
  --action create \
  --from-file /mnt/data/sample.csv \
  --output schema.yaml

# Advanced validation
backfill validate \
  --path /mnt/data \
  --rules validation_rules.yaml \
  --fix-errors \
  --report validation_report.html

# Performance testing
backfill benchmark \
  --path /mnt/data \
  --formats csv,parquet,json \
  --sizes 1MB,100MB,1GB \
  --report benchmark_results.json
```

### 4.2 Configuration File Support

```yaml
# backfill_config.yaml
backfill:
  # File discovery
  discovery:
    paths:
      - /mnt/data/daily
      - /mnt/data/historical
    patterns:
      - "*.csv"
      - "*.csv.gz"
      - "*.parquet"
    recursive: true
    follow_symlinks: false
  
  # Schema configuration
  schema:
    auto_detect: true
    timestamp_formats:
      - "%Y-%m-%d %H:%M:%S"
      - "auto"
    column_mappings:
      timestamp: ["timestamp", "time", "date"]
      symbol: ["symbol", "ticker", "sym"]
  
  # Processing
  processing:
    batch_size: 50000
    parallel_workers: 4
    memory_limit: "2GB"
    temp_directory: "/tmp/backfill"
  
  # Validation
  validation:
    mode: "strict"  # strict, permissive, repair
    outlier_detection:
      method: "iqr"
      multiplier: 3.0
      action: "log"  # log, cap, remove
    duplicate_handling: "remove"
    null_handling: "skip"
  
  # Storage
  storage:
    write_batch_size: 10000
    compression: true
    partitioning:
      - "year"
      - "month"
      - "symbol"
  
  # Monitoring
  monitoring:
    progress_port: 8080
    metrics_export: "prometheus"
    log_level: "info"
    alert_on_errors: true
```

## 5. Error Handling and Recovery

### 5.1 Comprehensive Error Recovery

```python
class ErrorHandler:
    """Advanced error handling with recovery strategies."""
    
    def __init__(self, config: ErrorConfig):
        self.config = config
        self.error_log = []
        self.recovery_strategies = {
            'file_not_found': self._handle_file_not_found,
            'permission_denied': self._handle_permission_denied,
            'corrupt_file': self._handle_corrupt_file,
            'schema_mismatch': self._handle_schema_mismatch,
            'network_error': self._handle_network_error,
            'storage_error': self._handle_storage_error
        }
    
    async def handle_error(self, error: Exception, context: Dict[str, Any]) -> bool:
        """Handle error with appropriate recovery strategy."""
        error_type = self._classify_error(error)
        
        # Log error
        self.error_log.append({
            'timestamp': datetime.now(),
            'error_type': error_type,
            'error': str(error),
            'context': context
        })
        
        # Apply recovery strategy
        if error_type in self.recovery_strategies:
            handler = self.recovery_strategies[error_type]
            return await handler(error, context)
        
        # Default handling
        if self.config.on_unknown_error == 'skip':
            logger.warning(f"Skipping due to error: {error}")
            return True  # Continue processing
        elif self.config.on_unknown_error == 'fail':
            raise error
        else:
            # Retry with backoff
            return await self._retry_with_backoff(context)
    
    async def _handle_corrupt_file(self, error: Exception, context: Dict[str, Any]) -> bool:
        """Handle corrupt file errors."""
        file_path = context.get('file_path')
        
        if self.config.corrupt_file_action == 'skip':
            logger.warning(f"Skipping corrupt file: {file_path}")
            return True
        elif self.config.corrupt_file_action == 'quarantine':
            # Move to quarantine directory
            quarantine_path = self.config.quarantine_dir / file_path.name
            shutil.move(str(file_path), str(quarantine_path))
            logger.info(f"Moved corrupt file to quarantine: {quarantine_path}")
            return True
        elif self.config.corrupt_file_action == 'repair':
            # Attempt to repair file
            repaired = await self._attempt_file_repair(file_path)
            if repaired:
                # Retry processing
                return await self._retry_processing(context)
            else:
                logger.error(f"Failed to repair file: {file_path}")
                return False
```

## 6. Performance Optimizations

### 6.1 Memory-Efficient Processing

```python
class MemoryManager:
    """Manage memory usage during processing."""
    
    def __init__(self, memory_limit_gb: float = 2.0):
        self.memory_limit_bytes = memory_limit_gb * 1024 * 1024 * 1024
        self.current_usage = 0
        self.peak_usage = 0
        
    async def allocate_batch(self, estimated_size: int) -> bool:
        """Check if batch can be allocated within memory limits."""
        import psutil
        
        # Get current process memory
        process = psutil.Process()
        mem_info = process.memory_info()
        self.current_usage = mem_info.rss
        
        # Check if allocation would exceed limit
        if self.current_usage + estimated_size > self.memory_limit_bytes:
            # Trigger garbage collection
            gc.collect()
            
            # Re-check after GC
            mem_info = process.memory_info()
            self.current_usage = mem_info.rss
            
            if self.current_usage + estimated_size > self.memory_limit_bytes:
                return False
        
        # Update peak usage
        self.peak_usage = max(self.peak_usage, self.current_usage + estimated_size)
        
        return True
    
    def get_optimal_batch_size(self, record_size: int) -> int:
        """Calculate optimal batch size based on available memory."""
        # Reserve 20% memory headroom
        available_memory = (self.memory_limit_bytes - self.current_usage) * 0.8
        
        # Calculate batch size
        optimal_size = int(available_memory / record_size)
        
        # Apply bounds
        return max(1000, min(100000, optimal_size))
```

### 6.2 Parallel File Processing

```python
class ParallelProcessor:
    """Process multiple files in parallel with resource management."""
    
    def __init__(self, max_workers: int = 4):
        self.max_workers = max_workers
        self.semaphore = asyncio.Semaphore(max_workers)
        self.resource_monitor = ResourceMonitor()
        
    async def process_files_parallel(
        self,
        files: List[Path],
        processor: FileProcessor
    ) -> ProcessingResult:
        """Process files in parallel with adaptive concurrency."""
        
        # Group files by size for better load balancing
        file_groups = self._group_files_by_size(files)
        
        # Create processing tasks
        tasks = []
        for group in file_groups:
            task = asyncio.create_task(
                self._process_file_group(group, processor)
            )
            tasks.append(task)
        
        # Monitor and adjust concurrency
        monitor_task = asyncio.create_task(
            self._monitor_and_adjust_concurrency()
        )
        
        # Wait for all processing
        results = await asyncio.gather(*tasks, return_exceptions=True)
        monitor_task.cancel()
        
        # Aggregate results
        return self._aggregate_results(results)
    
    async def _monitor_and_adjust_concurrency(self):
        """Dynamically adjust concurrency based on system resources."""
        while True:
            await asyncio.sleep(5)  # Check every 5 seconds
            
            cpu_percent = psutil.cpu_percent(interval=1)
            memory_percent = psutil.virtual_memory().percent
            
            if cpu_percent > 90 or memory_percent > 85:
                # Reduce concurrency
                if self.semaphore._value > 1:
                    self.semaphore._value -= 1
                    logger.info(f"Reduced concurrency to {self.semaphore._value}")
            elif cpu_percent < 50 and memory_percent < 60:
                # Increase concurrency
                if self.semaphore._value < self.max_workers:
                    self.semaphore._value += 1
                    logger.info(f"Increased concurrency to {self.semaphore._value}")
```

## 7. Integration Points

### 7.1 Storage Integration

```python
class StorageAdapter:
    """Unified storage adapter for different backends."""
    
    def __init__(self, storage_config: StorageConfig):
        self.config = storage_config
        self.backends = {
            'timescale': TimescaleBackend(storage_config.timescale),
            'clickhouse': ClickHouseBackend(storage_config.clickhouse),
            'parquet': ParquetBackend(storage_config.parquet),
            's3': S3Backend(storage_config.s3)
        }
        self.primary_backend = self.backends[storage_config.primary]
        
    async def write_batch(self, batch: pd.DataFrame, metadata: Dict[str, Any]):
        """Write batch to storage with automatic partitioning."""
        
        # Apply partitioning if configured
        if self.config.partitioning:
            partitions = self._partition_data(batch, self.config.partitioning)
            
            # Write each partition
            tasks = []
            for partition_key, partition_data in partitions.items():
                task = self._write_partition(
                    partition_data,
                    partition_key,
                    metadata
                )
                tasks.append(task)
            
            await asyncio.gather(*tasks)
        else:
            # Write entire batch
            await self.primary_backend.write(batch, metadata)
    
    def _partition_data(
        self,
        data: pd.DataFrame,
        partition_keys: List[str]
    ) -> Dict[str, pd.DataFrame]:
        """Partition data by specified keys."""
        partitions = {}
        
        # Group by partition keys
        for keys, group in data.groupby(partition_keys):
            if isinstance(keys, tuple):
                partition_key = '/'.join(str(k) for k in keys)
            else:
                partition_key = str(keys)
            
            partitions[partition_key] = group
        
        return partitions
```

### 7.2 Monitoring Integration

```python
class MonitoringIntegration:
    """Integration with monitoring systems."""
    
    def __init__(self, config: MonitoringConfig):
        self.config = config
        self.exporters = []
        
        # Initialize exporters
        if 'prometheus' in config.exporters:
            from prometheus_client import start_http_server, Counter, Gauge, Histogram
            
            self.metrics = {
                'files_processed': Counter('backfill_files_processed', 'Files processed'),
                'records_processed': Counter('backfill_records_processed', 'Records processed'),
                'errors': Counter('backfill_errors', 'Processing errors', ['error_type']),
                'processing_time': Histogram('backfill_processing_time', 'Processing time'),
                'current_speed': Gauge('backfill_current_speed', 'Current processing speed'),
                'memory_usage': Gauge('backfill_memory_usage', 'Memory usage in bytes')
            }
            
            start_http_server(config.prometheus_port)
        
        if 'datadog' in config.exporters:
            from datadog import initialize, statsd
            initialize(api_key=config.datadog_api_key)
            self.datadog = statsd
    
    def record_metric(self, metric_name: str, value: float, tags: Dict[str, str] = None):
        """Record metric to all configured exporters."""
        
        # Prometheus
        if hasattr(self, 'metrics') and metric_name in self.metrics:
            metric = self.metrics[metric_name]
            if isinstance(metric, Counter):
                metric.inc(value)
            elif isinstance(metric, Gauge):
                metric.set(value)
            elif isinstance(metric, Histogram):
                metric.observe(value)
        
        # Datadog
        if hasattr(self, 'datadog'):
            self.datadog.gauge(f'backfill.{metric_name}', value, tags=tags)
```

## 8. Testing Strategy

### 8.1 Unit Tests

```python
class TestFileBackfill:
    """Comprehensive unit tests for file backfill."""
    
    @pytest.fixture
    async def sample_files(self, tmp_path):
        """Create sample test files."""
        # CSV file
        csv_file = tmp_path / "test.csv"
        csv_file.write_text(
            "timestamp,symbol,open,high,low,close,volume\n"
            "2024-01-01 09:30:00,AAPL,150.0,151.0,149.5,150.5,1000000\n"
            "2024-01-01 09:31:00,AAPL,150.5,151.5,150.0,151.0,900000\n"
        )
        
        # Parquet file
        df = pd.DataFrame({
            'timestamp': pd.date_range('2024-01-01', periods=100, freq='1min'),
            'symbol': 'AAPL',
            'open': np.random.uniform(150, 151, 100),
            'high': np.random.uniform(151, 152, 100),
            'low': np.random.uniform(149, 150, 100),
            'close': np.random.uniform(150, 151, 100),
            'volume': np.random.randint(100000, 1000000, 100)
        })
        parquet_file = tmp_path / "test.parquet"
        df.to_parquet(parquet_file)
        
        return {'csv': csv_file, 'parquet': parquet_file}
    
    @pytest.mark.asyncio
    async def test_schema_detection(self, sample_files):
        """Test automatic schema detection."""
        reader = CSVReader()
        schema = await reader.read_schema(sample_files['csv'])
        
        assert 'timestamp' in schema['columns']
        assert 'symbol' in schema['columns']
        assert len(schema['columns']) == 7
    
    @pytest.mark.asyncio
    async def test_parallel_processing(self, sample_files):
        """Test parallel file processing."""
        processor = ParallelProcessor(max_workers=2)
        
        # Create multiple files
        files = []
        for i in range(10):
            file_path = sample_files['csv'].parent / f"test_{i}.csv"
            shutil.copy(sample_files['csv'], file_path)
            files.append(file_path)
        
        # Process in parallel
        start_time = time.time()
        result = await processor.process_files_parallel(files, FileProcessor())
        elapsed = time.time() - start_time
        
        assert result.files_processed == 10
        assert result.errors == 0
        assert elapsed < 5.0  # Should be fast with parallelism
```

### 8.2 Integration Tests

```python
class TestBackfillIntegration:
    """Integration tests for complete backfill workflow."""
    
    @pytest.mark.integration
    async def test_end_to_end_backfill(self, test_database, sample_data_files):
        """Test complete backfill workflow."""
        
        # Initialize components
        config = BackfillConfig.from_yaml("test_config.yaml")
        orchestrator = BackfillOrchestrator(config)
        
        # Run backfill
        result = await orchestrator.run_backfill(
            files=sample_data_files,
            symbols=['AAPL', 'GOOGL'],
            start_date=datetime(2024, 1, 1),
            end_date=datetime(2024, 1, 31)
        )
        
        # Verify results
        assert result.status == 'completed'
        assert result.files_processed == len(sample_data_files)
        assert result.records_processed > 0
        assert result.errors == 0
        
        # Verify data in database
        async with test_database.connect() as conn:
            count = await conn.fetchval(
                "SELECT COUNT(*) FROM market_data WHERE symbol IN ('AAPL', 'GOOGL')"
            )
            assert count == result.records_processed
```

## 9. Deployment Guide

### 9.1 Docker Deployment

```dockerfile
# Dockerfile for backfill service
FROM python:3.11-slim

# Install system dependencies
RUN apt-get update && apt-get install -y \
    gcc \
    g++ \
    && rm -rf /var/lib/apt/lists/*

# Install Python dependencies
WORKDIR /app
COPY requirements.txt .
RUN pip install --no-cache-dir -r requirements.txt

# Copy application
COPY . .

# Create directories
RUN mkdir -p /data /checkpoints /logs

# Entry point
ENTRYPOINT ["python", "-m", "data_ingestion.cli.backfill"]
```

### 9.2 Kubernetes Deployment

```yaml
apiVersion: batch/v1
kind: Job
metadata:
  name: backfill-job
spec:
  parallelism: 4
  completions: 4
  template:
    spec:
      containers:
      - name: backfill
        image: neural-trader/backfill:latest
        command: ["backfill", "file"]
        args:
          - "--path=/data"
          - "--parallel=4"
          - "--config=/config/backfill.yaml"
        resources:
          requests:
            memory: "2Gi"
            cpu: "2"
          limits:
            memory: "4Gi"
            cpu: "4"
        volumeMounts:
        - name: data
          mountPath: /data
        - name: config
          mountPath: /config
      volumes:
      - name: data
        persistentVolumeClaim:
          claimName: market-data-pvc
      - name: config
        configMap:
          name: backfill-config
```

## 10. Migration Path

### Phase 1: Enhanced Schema Support (Week 1-2)
- Implement flexible schema configuration
- Add schema detection and validation
- Update existing file readers

### Phase 2: Streaming Pipeline (Week 3-4)
- Implement async pipeline architecture
- Add parallel processing support
- Integrate memory management

### Phase 3: Monitoring & UI (Week 5-6)
- Build progress tracking system
- Create web dashboard
- Add CLI enhancements

### Phase 4: Testing & Optimization (Week 7-8)
- Comprehensive testing
- Performance optimization
- Documentation and training

## Conclusion

This enhanced file backfill implementation provides a production-grade solution for ingesting large volumes of historical market data. Key improvements include:

1. **Flexibility**: Support for multiple file formats with automatic schema detection
2. **Performance**: Parallel processing with streaming architecture
3. **Reliability**: Comprehensive error handling and recovery
4. **Monitoring**: Real-time progress tracking and diagnostics
5. **Scalability**: Memory-efficient processing with resource management

The implementation maintains backward compatibility while adding powerful new features for enterprise-scale data ingestion.