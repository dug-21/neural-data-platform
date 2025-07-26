"""File-based data provider for backfill operations."""
import asyncio
import csv
import json
import os
from typing import Dict, Any, List, AsyncIterator, Optional, Tuple
from datetime import datetime
from pathlib import Path
import pandas as pd
import pyarrow.parquet as pq
from dataclasses import dataclass

from .base import BaseProvider, MarketData, TickData, OrderBookData
from utils.logging import get_logger
from utils.metrics import metrics

logger = get_logger(__name__)


@dataclass
class FileMetadata:
    """Metadata about the file being processed."""
    filepath: str
    format: str
    total_rows: Optional[int] = None
    columns: Optional[List[str]] = None
    symbol: Optional[str] = None
    start_time: Optional[datetime] = None
    end_time: Optional[datetime] = None


class CheckpointManager:
    """Manages checkpoints for file processing recovery."""
    
    def __init__(self, checkpoint_dir: str = None):
        if checkpoint_dir is None:
            # Use local directory in development, system directory in production
            if os.path.exists("/var/lib/data-ingestion"):
                checkpoint_dir = "/var/lib/data-ingestion/checkpoints"
            else:
                # Development mode - use local directory
                checkpoint_dir = os.path.join(os.getcwd(), ".checkpoints")
                
        self.checkpoint_dir = Path(checkpoint_dir)
        self.checkpoint_dir.mkdir(parents=True, exist_ok=True)
        
    def _get_checkpoint_path(self, filepath: str) -> Path:
        """Get checkpoint file path for a given file."""
        file_hash = str(hash(filepath))
        return self.checkpoint_dir / f"checkpoint_{file_hash}.json"
        
    def get_checkpoint(self, filepath: str) -> int:
        """Get the last processed row for a file."""
        checkpoint_path = self._get_checkpoint_path(filepath)
        
        if checkpoint_path.exists():
            try:
                with open(checkpoint_path, 'r') as f:
                    data = json.load(f)
                    logger.info(f"Resuming from checkpoint: row {data['last_row']} for {filepath}")
                    return data['last_row']
            except Exception as e:
                logger.warning(f"Failed to load checkpoint: {e}")
                
        return 0
        
    def update_checkpoint(self, filepath: str, last_row: int, metadata: Optional[Dict] = None):
        """Update checkpoint with last processed row."""
        checkpoint_path = self._get_checkpoint_path(filepath)
        
        data = {
            'filepath': filepath,
            'last_row': last_row,
            'updated_at': datetime.now().isoformat(),
            'metadata': metadata or {}
        }
        
        try:
            with open(checkpoint_path, 'w') as f:
                json.dump(data, f)
        except Exception as e:
            logger.error(f"Failed to update checkpoint: {e}")
            
    def clear_checkpoint(self, filepath: str):
        """Clear checkpoint for a file (on successful completion)."""
        checkpoint_path = self._get_checkpoint_path(filepath)
        if checkpoint_path.exists():
            checkpoint_path.unlink()
            logger.info(f"Cleared checkpoint for {filepath}")


class FileProvider(BaseProvider):
    """File-based data provider for backfill operations."""
    
    def __init__(self, config: Optional[Dict[str, Any]] = None):
        """Initialize file provider."""
        # Initialize with 'file' as provider name
        super().__init__("file")
        
        # Override with config if provided
        if config:
            self.config = config
        else:
            self.config = {}
            
        self.supported_formats = ['csv', 'json', 'parquet']
        self.checkpoint_manager = CheckpointManager()
        self.batch_size = self.config.get('batch_size', 1000)
        self.encoding = self.config.get('encoding', 'utf-8')
        
    async def connect(self):
        """No connection needed for file provider."""
        self._connected = True
        logger.info("File provider ready")
        
    async def disconnect(self):
        """No disconnection needed for file provider."""
        self._connected = False
        logger.info("File provider stopped")
        
    async def load_from_file(
        self, 
        filepath: str, 
        format: str = 'csv',
        symbol: Optional[str] = None,
        data_type: str = 'market_data'
    ) -> AsyncIterator[MarketData]:
        """
        Load data from file with progress tracking and checkpoint recovery.
        
        Args:
            filepath: Path to the file to load
            format: File format (csv, json, parquet)
            symbol: Symbol override (if not in file)
            data_type: Type of data to parse
            
        Yields:
            MarketData objects
        """
        if format not in self.supported_formats:
            raise ValueError(f"Unsupported format: {format}. Supported: {self.supported_formats}")
            
        if not os.path.exists(filepath):
            raise FileNotFoundError(f"File not found: {filepath}")
            
        # Get file metadata
        metadata = await self._get_file_metadata(filepath, format)
        if symbol:
            metadata.symbol = symbol
            
        # Resume from checkpoint if exists
        start_row = self.checkpoint_manager.get_checkpoint(filepath)
        
        logger.info(
            f"Loading {format} file: {filepath} "
            f"(starting from row {start_row}, batch size: {self.batch_size})"
        )
        
        # Start timing for metrics
        start_time = datetime.now()
        
        try:
            processed_count = 0
            async for batch in self._stream_file(filepath, format, start_row):
                for row_num, row_data in batch:
                    try:
                        # Parse row based on data type
                        if data_type == 'market_data':
                            market_data = self._parse_market_data(row_data, metadata)
                            yield market_data
                            
                        processed_count += 1
                        
                        # Update checkpoint every batch
                        if processed_count % self.batch_size == 0:
                            self.checkpoint_manager.update_checkpoint(
                                filepath, 
                                row_num,
                                {'processed': processed_count}
                            )
                            logger.info(f"Processed {processed_count} rows, checkpoint updated")
                            
                            # Update progress metric
                            if metadata.total_rows:
                                progress = processed_count / metadata.total_rows
                                metrics.file_backfill_progress.labels(
                                    file=os.path.basename(filepath),
                                    format=format
                                ).set(progress)
                            
                    except Exception as e:
                        logger.error(f"Error parsing row {row_num}: {e}")
                        continue
                        
            # Clear checkpoint on successful completion
            self.checkpoint_manager.clear_checkpoint(filepath)
            logger.info(f"Successfully loaded {processed_count} rows from {filepath}")
            
            # Update final metrics
            elapsed = (datetime.now() - start_time).total_seconds()
            metrics.file_backfill_duration.labels(format=format).observe(elapsed)
            metrics.file_backfill_rows.labels(
                file=os.path.basename(filepath),
                format=format,
                status='success'
            ).inc(processed_count)
            
            # Set progress to 100%
            metrics.file_backfill_progress.labels(
                file=os.path.basename(filepath),
                format=format
            ).set(1.0)
            
        except Exception as e:
            logger.error(f"Error loading file {filepath}: {e}")
            raise
            
    async def _get_file_metadata(self, filepath: str, format: str) -> FileMetadata:
        """Extract metadata from file."""
        metadata = FileMetadata(filepath=filepath, format=format)
        
        try:
            if format == 'csv':
                # Read first few rows to get columns
                with open(filepath, 'r', encoding=self.encoding) as f:
                    reader = csv.DictReader(f)
                    metadata.columns = reader.fieldnames
                    
                    # Count total rows (for progress tracking)
                    row_count = sum(1 for _ in f)
                    metadata.total_rows = row_count
                    
            elif format == 'json':
                with open(filepath, 'r', encoding=self.encoding) as f:
                    data = json.load(f)
                    if isinstance(data, list) and data:
                        metadata.columns = list(data[0].keys())
                        metadata.total_rows = len(data)
                        
            elif format == 'parquet':
                pf = pq.ParquetFile(filepath)
                metadata.columns = pf.schema.names
                metadata.total_rows = pf.metadata.num_rows
                
        except Exception as e:
            logger.warning(f"Failed to extract metadata: {e}")
            
        return metadata
        
    async def _stream_file(
        self, 
        filepath: str, 
        format: str, 
        start_row: int = 0
    ) -> AsyncIterator[List[Tuple[int, Dict]]]:
        """
        Stream file in batches.
        
        Yields:
            List of tuples (row_number, row_data)
        """
        if format == 'csv':
            async for batch in self._stream_csv(filepath, start_row):
                yield batch
                
        elif format == 'json':
            async for batch in self._stream_json(filepath, start_row):
                yield batch
                
        elif format == 'parquet':
            async for batch in self._stream_parquet(filepath, start_row):
                yield batch
                
    async def _stream_csv(self, filepath: str, start_row: int = 0) -> AsyncIterator[List[Tuple[int, Dict]]]:
        """Stream CSV file in batches."""
        def read_csv_batch():
            batch = []
            with open(filepath, 'r', encoding=self.encoding) as f:
                reader = csv.DictReader(f)
                
                # Skip to start row
                for _ in range(start_row):
                    next(reader, None)
                    
                row_num = start_row
                for row in reader:
                    batch.append((row_num, row))
                    row_num += 1
                    
                    if len(batch) >= self.batch_size:
                        yield batch
                        batch = []
                        
                if batch:
                    yield batch
                    
        # Run in executor to avoid blocking
        loop = asyncio.get_event_loop()
        for batch in read_csv_batch():
            yield batch
            await asyncio.sleep(0)  # Allow other tasks to run
            
    async def _stream_json(self, filepath: str, start_row: int = 0) -> AsyncIterator[List[Tuple[int, Dict]]]:
        """Stream JSON file in batches."""
        def read_json_batch():
            with open(filepath, 'r', encoding=self.encoding) as f:
                data = json.load(f)
                
                if not isinstance(data, list):
                    data = [data]
                    
                batch = []
                for row_num, row in enumerate(data[start_row:], start=start_row):
                    batch.append((row_num, row))
                    
                    if len(batch) >= self.batch_size:
                        yield batch
                        batch = []
                        
                if batch:
                    yield batch
                    
        loop = asyncio.get_event_loop()
        for batch in read_json_batch():
            yield batch
            await asyncio.sleep(0)
            
    async def _stream_parquet(self, filepath: str, start_row: int = 0) -> AsyncIterator[List[Tuple[int, Dict]]]:
        """Stream Parquet file in batches."""
        def read_parquet_batch():
            pf = pq.ParquetFile(filepath)
            batch = []
            row_num = 0
            
            for batch_df in pf.iter_batches(batch_size=self.batch_size):
                df = batch_df.to_pandas()
                
                for idx, row in df.iterrows():
                    if row_num >= start_row:
                        batch.append((row_num, row.to_dict()))
                        
                        if len(batch) >= self.batch_size:
                            yield batch
                            batch = []
                            
                    row_num += 1
                    
            if batch:
                yield batch
                
        loop = asyncio.get_event_loop()
        for batch in read_parquet_batch():
            yield batch
            await asyncio.sleep(0)
            
    def _parse_market_data(self, row_data: Dict, metadata: FileMetadata) -> MarketData:
        """Parse row data into MarketData object."""
        # Common field mappings
        field_mappings = {
            'timestamp': ['timestamp', 'time', 'date', 'datetime'],
            'symbol': ['symbol', 'ticker', 'code'],
            'open': ['open', 'open_price', 'o'],
            'high': ['high', 'high_price', 'h'],
            'low': ['low', 'low_price', 'l'],
            'close': ['close', 'close_price', 'c'],
            'volume': ['volume', 'vol', 'v']
        }
        
        # Extract fields with fallbacks
        parsed = {}
        for field, possible_names in field_mappings.items():
            for name in possible_names:
                if name in row_data:
                    parsed[field] = row_data[name]
                    break
                    
        # Parse timestamp
        if 'timestamp' in parsed:
            if isinstance(parsed['timestamp'], str):
                try:
                    # Try common formats
                    for fmt in ['%Y-%m-%d %H:%M:%S', '%Y-%m-%dT%H:%M:%S', '%Y-%m-%d']:
                        try:
                            parsed['timestamp'] = datetime.strptime(parsed['timestamp'], fmt)
                            break
                        except ValueError:
                            continue
                except:
                    parsed['timestamp'] = datetime.now()
            elif isinstance(parsed['timestamp'], (int, float)):
                parsed['timestamp'] = datetime.fromtimestamp(parsed['timestamp'])
        else:
            parsed['timestamp'] = datetime.now()
            
        # Use metadata symbol if not in row
        if 'symbol' not in parsed and metadata.symbol:
            parsed['symbol'] = metadata.symbol
            
        # Create MarketData object
        return MarketData(
            time=parsed.get('timestamp', datetime.now()),
            symbol=parsed.get('symbol', 'UNKNOWN'),
            open=float(parsed.get('open', 0)),
            high=float(parsed.get('high', 0)),
            low=float(parsed.get('low', 0)),
            close=float(parsed.get('close', 0)),
            volume=int(float(parsed.get('volume', 0))),
            provider=self.name,
            metadata={
                'source_file': metadata.filepath,
                'format': metadata.format
            }
        )
        
    # Implement required abstract methods
    async def get_market_data(
        self,
        symbols: List[str],
        start_time: datetime,
        end_time: datetime,
        interval: str = "1min"
    ) -> AsyncIterator[MarketData]:
        """Not implemented for file provider - use load_from_file instead."""
        raise NotImplementedError("Use load_from_file method for file-based data")
        
    async def stream_market_data(
        self,
        symbols: List[str]
    ) -> AsyncIterator[MarketData]:
        """Not implemented for file provider - use load_from_file instead."""
        raise NotImplementedError("File provider does not support streaming")
        
    async def get_tick_data(
        self,
        symbols: List[str],
        start_time: datetime,
        end_time: datetime
    ) -> AsyncIterator[TickData]:
        """Not implemented for file provider."""
        raise NotImplementedError("Use load_from_file method for file-based data")
        
    async def stream_tick_data(
        self,
        symbols: List[str]
    ) -> AsyncIterator[TickData]:
        """Not implemented for file provider."""
        raise NotImplementedError("File provider does not support streaming")
        
    async def get_order_book(
        self,
        symbols: List[str]
    ) -> AsyncIterator[OrderBookData]:
        """Not implemented for file provider."""
        raise NotImplementedError("File provider does not support order book data")