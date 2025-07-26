"""File-based backfill handler for processing historical data from mounted files."""
import asyncio
import json
from datetime import datetime
from pathlib import Path
from typing import List, Optional, Dict, Any, AsyncIterator
import pandas as pd
try:
    import pyarrow.parquet as pq
except ImportError:
    pq = None

from utils.logging import get_logger
from utils.metrics import metrics
from storage.timescale import TimescaleDB
from storage.redis_store import RedisStore

logger = get_logger(__name__)


class FileBackfillHandler:
    """Handler for backfilling historical data from files."""
    
    def __init__(
        self,
        path: Path,
        format: str = 'csv',
        symbols: Optional[List[str]] = None,
        start_date: Optional[datetime] = None,
        end_date: Optional[datetime] = None,
        batch_size: int = 1000,
        use_checkpoint: bool = True,
        dry_run: bool = False
    ):
        """
        Initialize the file backfill handler.
        
        Args:
            path: Path to file or directory
            format: File format (csv, json, parquet)
            symbols: List of symbols to filter (None = all)
            start_date: Start date for filtering
            end_date: End date for filtering
            batch_size: Number of records per batch
            use_checkpoint: Whether to use checkpoint system
            dry_run: Preview mode without writing data
        """
        self.path = path
        self.format = format.lower()
        self.symbols = set(symbols) if symbols else None
        self.start_date = start_date
        self.end_date = end_date
        self.batch_size = batch_size
        self.use_checkpoint = use_checkpoint
        self.dry_run = dry_run
        
        # Initialize components
        self.redis_store = RedisStore() if use_checkpoint else None
        self.storage = TimescaleDB() if not dry_run else None
        
        # Statistics
        self.stats = {
            'total_files': 0,
            'processed_files': 0,
            'total_records': 0,
            'filtered_records': 0,
            'written_records': 0,
            'skipped_records': 0,
            'errors': 0
        }
    
    async def run(self):
        """Run the backfill process."""
        try:
            logger.info("Starting file backfill process")
            
            # Initialize storage connections (skip in dry-run mode)
            if self.storage and not self.dry_run:
                await self.storage.connect()
            
            if self.redis_store and self.use_checkpoint:
                try:
                    await self.redis_store.connect()
                except Exception as e:
                    logger.warning(f"Could not connect to Redis for checkpoints: {e}")
                    logger.info("Proceeding without checkpoint functionality")
                    self.redis_store = None
                    self.use_checkpoint = False
            
            # Get list of files to process
            files = self._get_files_to_process()
            self.stats['total_files'] = len(files)
            
            if not files:
                logger.warning("No files found to process")
                return
            
            logger.info(f"Found {len(files)} files to process")
            
            # Process each file
            for file_path in files:
                await self._process_file(file_path)
            
            # Log final statistics
            self._log_statistics()
            
        except Exception as e:
            logger.error(f"File backfill failed: {e}")
            raise
        finally:
            # Cleanup connections
            if self.storage and not self.dry_run:
                await self.storage.disconnect()
            if self.redis_store:
                await self.redis_store.disconnect()
    
    def _get_files_to_process(self) -> List[Path]:
        """Get list of files to process based on path and format."""
        files = []
        
        if self.path.is_file():
            # Single file
            if self._is_valid_format(self.path):
                files.append(self.path)
        else:
            # Directory - scan for files
            pattern = f"*.{self.format}"
            files = list(self.path.glob(pattern))
            
            # Also check subdirectories
            files.extend(list(self.path.rglob(pattern)))
        
        # Sort files by name for consistent processing
        files.sort()
        
        return files
    
    def _is_valid_format(self, file_path: Path) -> bool:
        """Check if file has the expected format."""
        return file_path.suffix.lower() == f".{self.format}"
    
    async def _process_file(self, file_path: Path):
        """Process a single file."""
        try:
            logger.info(f"Processing file: {file_path}")
            
            # Check if already processed (checkpoint)
            if self.use_checkpoint and self.redis_store:
                checkpoint_key = f"file_backfill:{file_path}"
                checkpoint = await self._get_checkpoint(checkpoint_key)
                
                if checkpoint and checkpoint.get('completed'):
                    logger.info(f"File already processed (checkpoint found): {file_path}")
                    self.stats['skipped_records'] += checkpoint.get('records', 0)
                    return
            
            # Read and process file in batches
            record_count = 0
            async for batch in self._read_file_batches(file_path):
                if not self.dry_run:
                    await self._process_batch(batch)
                record_count += len(batch)
                
                # Update checkpoint periodically
                if self.use_checkpoint and record_count % (self.batch_size * 10) == 0:
                    await self._update_checkpoint(file_path, record_count, completed=False)
            
            # Mark file as completed
            if self.use_checkpoint:
                await self._update_checkpoint(file_path, record_count, completed=True)
            
            self.stats['processed_files'] += 1
            logger.info(f"Completed processing {file_path}: {record_count} records")
            
        except Exception as e:
            logger.error(f"Error processing file {file_path}: {e}")
            self.stats['errors'] += 1
            metrics.data_ingestion_errors.labels(source='file_backfill', file=str(file_path)).inc()
    
    async def _read_file_batches(self, file_path: Path) -> AsyncIterator[pd.DataFrame]:
        """Read file in batches based on format."""
        try:
            if self.format == 'csv':
                # Read CSV in chunks
                for chunk in pd.read_csv(file_path, chunksize=self.batch_size):
                    yield await self._filter_data(chunk)
                    
            elif self.format == 'json':
                # Read JSON file
                with open(file_path, 'r') as f:
                    data = json.load(f)
                    
                # Convert to DataFrame
                if isinstance(data, list):
                    df = pd.DataFrame(data)
                else:
                    df = pd.DataFrame([data])
                
                # Process in batches
                for i in range(0, len(df), self.batch_size):
                    batch = df.iloc[i:i + self.batch_size]
                    yield await self._filter_data(batch)
                    
            elif self.format == 'parquet':
                if pq is None:
                    raise ImportError("pyarrow is required to read parquet files. Install with: pip install pyarrow")
                
                # Read Parquet file
                table = pq.read_table(file_path)
                df = table.to_pandas()
                
                # Process in batches
                for i in range(0, len(df), self.batch_size):
                    batch = df.iloc[i:i + self.batch_size]
                    yield await self._filter_data(batch)
                    
        except Exception as e:
            logger.error(f"Error reading file {file_path}: {e}")
            raise
    
    async def _filter_data(self, df: pd.DataFrame) -> pd.DataFrame:
        """Apply filters to data based on symbols and date range."""
        original_count = len(df)
        self.stats['total_records'] += original_count
        
        # Apply symbol filter
        if self.symbols and 'symbol' in df.columns:
            df = df[df['symbol'].isin(self.symbols)]
        
        # Apply date range filter
        if 'timestamp' in df.columns:
            # Convert timestamp column to datetime if needed
            if not pd.api.types.is_datetime64_any_dtype(df['timestamp']):
                df = df.copy()  # Avoid SettingWithCopyWarning
                df['timestamp'] = pd.to_datetime(df['timestamp'])
            
            if self.start_date:
                df = df[df['timestamp'] >= self.start_date]
            
            if self.end_date:
                df = df[df['timestamp'] <= self.end_date]
        
        filtered_count = original_count - len(df)
        if filtered_count > 0:
            self.stats['filtered_records'] += filtered_count
            logger.debug(f"Filtered out {filtered_count} records")
        
        return df
    
    async def _process_batch(self, batch: pd.DataFrame):
        """Process a batch of data."""
        if batch.empty:
            return
        
        try:
            # Convert DataFrame to records for storage
            records = batch.to_dict('records')
            
            # Store data based on type (assuming market data for now)
            # This could be enhanced to detect data type from columns
            if 'open' in batch.columns and 'close' in batch.columns:
                # OHLCV data
                await self._store_market_data(records)
            else:
                # Generic time series data
                await self._store_generic_data(records)
            
            self.stats['written_records'] += len(records)
            
            # Record metrics
            metrics.data_points_processed.labels(
                provider='file_import',
                data_type='market_data'
            ).inc(len(records))
            
        except Exception as e:
            logger.error(f"Error processing batch: {e}")
            self.stats['errors'] += 1
            raise
    
    async def _store_market_data(self, records: List[Dict[str, Any]]):
        """Store market data records."""
        if self.dry_run:
            logger.info(f"[DRY RUN] Would store {len(records)} market data records")
            return
        
        # Transform records to match TimescaleDB format
        formatted_records = []
        for record in records:
            # Ensure required fields and correct field names
            formatted_record = {
                'time': record.get('timestamp') or record.get('time'),
                'symbol': record.get('symbol'),
                'open': record.get('open'),
                'high': record.get('high'),
                'low': record.get('low'),
                'close': record.get('close'),
                'volume': record.get('volume'),
                'provider': 'file_import'
            }
            formatted_records.append(formatted_record)
        
        # Store using TimescaleDB method
        await self.storage.insert_market_data(formatted_records)
    
    async def _store_generic_data(self, records: List[Dict[str, Any]]):
        """Store generic time series data."""
        if self.dry_run:
            logger.info(f"[DRY RUN] Would store {len(records)} generic data records")
            return
        
        # For generic data, try to store as market data if it has OHLCV fields
        # Otherwise, we would need a generic storage method
        await self._store_market_data(records)
    
    async def _get_checkpoint(self, checkpoint_key: str) -> Optional[Dict[str, Any]]:
        """Get checkpoint data from Redis."""
        if not self.redis_store:
            return None
        
        data = await self.redis_store.cache_get(checkpoint_key)
        return data
    
    async def _update_checkpoint(self, file_path: Path, records: int, completed: bool):
        """Update checkpoint for file processing."""
        if not self.redis_store:
            return
        
        checkpoint_key = f"file_backfill:{file_path}"
        checkpoint_data = {
            'file': str(file_path),
            'records': records,
            'completed': completed,
            'timestamp': datetime.utcnow().isoformat()
        }
        
        # Store checkpoint with 7-day TTL
        await self.redis_store.cache_set(checkpoint_key, checkpoint_data, ttl=7*24*3600)
    
    def _log_statistics(self):
        """Log final processing statistics."""
        logger.info("=" * 50)
        logger.info("File Backfill Statistics:")
        logger.info(f"  Total files found: {self.stats['total_files']}")
        logger.info(f"  Files processed: {self.stats['processed_files']}")
        logger.info(f"  Total records: {self.stats['total_records']}")
        logger.info(f"  Records filtered: {self.stats['filtered_records']}")
        logger.info(f"  Records written: {self.stats['written_records']}")
        logger.info(f"  Records skipped: {self.stats['skipped_records']}")
        logger.info(f"  Errors: {self.stats['errors']}")
        logger.info("=" * 50)
        
        # Record final metrics using generic counters
        try:
            for key, value in self.stats.items():
                if hasattr(metrics, 'data_points_processed'):
                    metrics.data_points_processed.labels(
                        provider='file_import',
                        data_type=f'backfill_{key}'
                    ).inc(value)
        except Exception as e:
            logger.debug(f"Could not record metrics: {e}")