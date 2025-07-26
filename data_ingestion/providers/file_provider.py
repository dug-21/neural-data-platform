"""File-based data provider for reading market data from mounted external drives."""
import asyncio
import gzip
import csv
import os
import json
from pathlib import Path
from typing import List, AsyncIterator, Optional, Dict, Any, Set
from datetime import datetime, timezone
from dataclasses import dataclass, asdict
import aiofiles
from concurrent.futures import ThreadPoolExecutor

from .base import BaseProvider, MarketData, DataType
from utils.retry import with_retry
from utils.metrics import metrics
from utils.logging import get_logger


@dataclass
class FileCheckpoint:
    """Checkpoint data for resuming file processing."""
    file_path: str
    processed_lines: int
    last_timestamp: Optional[datetime]
    bad_records: int
    total_records: int
    
    def to_dict(self) -> Dict[str, Any]:
        """Convert to dictionary for JSON serialization."""
        data = asdict(self)
        if self.last_timestamp:
            data['last_timestamp'] = self.last_timestamp.isoformat()
        return data
    
    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> 'FileCheckpoint':
        """Create from dictionary."""
        if data.get('last_timestamp'):
            data['last_timestamp'] = datetime.fromisoformat(data['last_timestamp'])
        return cls(**data)


class FileProvider(BaseProvider):
    """
    File-based provider for reading market data from CSV files.
    
    Features:
    - Reads from mounted external drives
    - Parses gzipped CSV files efficiently
    - Filters for specific symbols during processing
    - Validates OHLC consistency
    - Tracks bad records and fails if >1%
    - Supports checkpoint/resume functionality
    """
    
    CSV_EXPECTED_HEADERS = ['timestamp', 'symbol', 'open', 'high', 'low', 'close', 'volume']
    MAX_BAD_RECORD_PERCENTAGE = 0.01  # 1%
    CHUNK_SIZE = 10000  # Process files in chunks
    
    def __init__(self, base_path: str, checkpoint_dir: Optional[str] = None):
        """
        Initialize the file provider.
        
        Args:
            base_path: Base directory path for data files (e.g., /mnt/external/market_data)
            checkpoint_dir: Directory to store checkpoint files for resume capability
        """
        super().__init__("file_provider")
        self.base_path = Path(base_path)
        self.checkpoint_dir = Path(checkpoint_dir) if checkpoint_dir else Path.home() / '.neural_trader' / 'checkpoints'
        self._executor = ThreadPoolExecutor(max_workers=4)
        self._checkpoints: Dict[str, FileCheckpoint] = {}
        self._active_files: Set[str] = set()
        
        # Validate base path exists
        if not self.base_path.exists():
            raise ValueError(f"Base path does not exist: {self.base_path}")
        
        # Create checkpoint directory
        self.checkpoint_dir.mkdir(parents=True, exist_ok=True)
        
        self.logger.info(f"Initialized FileProvider with base_path={self.base_path}, checkpoint_dir={self.checkpoint_dir}")
    
    async def connect(self):
        """Initialize provider connection."""
        await super().connect()
        await self._load_checkpoints()
        self.logger.info("FileProvider connected and checkpoints loaded")
    
    async def disconnect(self):
        """Clean up provider connection."""
        await self._save_checkpoints()
        self._executor.shutdown(wait=True)
        await super().disconnect()
        self.logger.info("FileProvider disconnected and checkpoints saved")
    
    async def _load_checkpoints(self):
        """Load existing checkpoints from disk."""
        checkpoint_file = self.checkpoint_dir / "file_provider_checkpoints.json"
        if checkpoint_file.exists():
            try:
                async with aiofiles.open(checkpoint_file, 'r') as f:
                    data = json.loads(await f.read())
                    self._checkpoints = {
                        k: FileCheckpoint.from_dict(v) 
                        for k, v in data.items()
                    }
                self.logger.info(f"Loaded {len(self._checkpoints)} checkpoints")
            except Exception as e:
                self.logger.error(f"Failed to load checkpoints: {e}")
    
    async def _save_checkpoints(self):
        """Save current checkpoints to disk."""
        checkpoint_file = self.checkpoint_dir / "file_provider_checkpoints.json"
        try:
            data = {
                k: v.to_dict() 
                for k, v in self._checkpoints.items()
            }
            async with aiofiles.open(checkpoint_file, 'w') as f:
                await f.write(json.dumps(data, indent=2))
            self.logger.info(f"Saved {len(self._checkpoints)} checkpoints")
        except Exception as e:
            self.logger.error(f"Failed to save checkpoints: {e}")
    
    def _find_data_files(self, symbols: List[str], start_time: datetime, end_time: datetime) -> List[Path]:
        """
        Find relevant data files based on symbols and time range.
        
        Expected file structure:
        - /base_path/YYYY/MM/DD/market_data_YYYYMMDD.csv.gz
        - /base_path/symbols/SYMBOL/YYYY/market_data_SYMBOL_YYYYMM.csv.gz
        """
        files = []
        
        # Search for daily files in date-based structure
        current = start_time.date()
        end_date = end_time.date()
        
        while current <= end_date:
            # Check date-based path
            date_path = self.base_path / str(current.year) / f"{current.month:02d}" / f"{current.day:02d}"
            if date_path.exists():
                for file_path in date_path.glob("*.csv.gz"):
                    files.append(file_path)
                for file_path in date_path.glob("*.csv"):
                    files.append(file_path)
            
            # Check symbol-based paths
            for symbol in symbols:
                symbol_path = self.base_path / "symbols" / symbol / str(current.year)
                if symbol_path.exists():
                    pattern_gz = f"*{symbol}*{current.year}{current.month:02d}*.csv.gz"
                    pattern_csv = f"*{symbol}*{current.year}{current.month:02d}*.csv"
                    for file_path in symbol_path.glob(pattern_gz):
                        files.append(file_path)
                    for file_path in symbol_path.glob(pattern_csv):
                        files.append(file_path)
            
            # Move to next day
            current = current.replace(day=current.day + 1) if current.day < 28 else \
                     current.replace(month=current.month + 1, day=1) if current.month < 12 else \
                     current.replace(year=current.year + 1, month=1, day=1)
        
        # Remove duplicates and sort
        files = sorted(list(set(files)))
        self.logger.info(f"Found {len(files)} data files for symbols {symbols} from {start_time} to {end_time}")
        
        return files
    
    def _parse_csv_line(self, line: str, line_number: int, file_path: str) -> Optional[Dict[str, Any]]:
        """Parse a single CSV line and return parsed data or None if invalid."""
        try:
            reader = csv.reader([line])
            row = next(reader)
            
            if len(row) < len(self.CSV_EXPECTED_HEADERS):
                self.logger.warning(f"Invalid row in {file_path}:{line_number} - insufficient columns")
                return None
            
            # Parse fields
            data = {
                'timestamp': row[0],
                'symbol': row[1],
                'open': float(row[2]),
                'high': float(row[3]),
                'low': float(row[4]),
                'close': float(row[5]),
                'volume': int(row[6])
            }
            
            return data
            
        except (ValueError, IndexError) as e:
            self.logger.warning(f"Failed to parse line {line_number} in {file_path}: {e}")
            return None
    
    async def _process_file(
        self, 
        file_path: Path, 
        symbols: Set[str],
        start_time: datetime,
        end_time: datetime
    ) -> AsyncIterator[MarketData]:
        """Process a single file and yield market data."""
        file_key = str(file_path)
        
        # Get or create checkpoint
        checkpoint = self._checkpoints.get(file_key, FileCheckpoint(
            file_path=file_key,
            processed_lines=0,
            last_timestamp=None,
            bad_records=0,
            total_records=0
        ))
        
        # Track file as active
        self._active_files.add(file_key)
        
        try:
            # Determine if file is gzipped
            if file_path.suffix == '.gz':
                open_func = gzip.open
                mode = 'rt'
            else:
                open_func = open
                mode = 'r'
            
            with open_func(file_path, mode, encoding='utf-8') as f:
                # Skip to checkpoint if resuming
                for _ in range(checkpoint.processed_lines):
                    next(f, None)
                
                # Skip header if at beginning
                if checkpoint.processed_lines == 0:
                    header = next(f, None)
                    if header:
                        checkpoint.processed_lines += 1
                
                # Process lines in chunks
                chunk = []
                
                for line_num, line in enumerate(f, start=checkpoint.processed_lines + 1):
                    line = line.strip()
                    if not line:
                        continue
                    
                    # Parse line
                    data = self._parse_csv_line(line, line_num, file_key)
                    
                    if data is None:
                        checkpoint.bad_records += 1
                        checkpoint.total_records += 1
                        
                        # Check bad record percentage
                        if checkpoint.total_records > 100:  # Only check after 100 records
                            bad_percentage = checkpoint.bad_records / checkpoint.total_records
                            if bad_percentage > self.MAX_BAD_RECORD_PERCENTAGE:
                                raise ValueError(
                                    f"Bad record percentage ({bad_percentage:.2%}) exceeds "
                                    f"maximum allowed ({self.MAX_BAD_RECORD_PERCENTAGE:.2%}) "
                                    f"in file {file_path}"
                                )
                        continue
                    
                    checkpoint.total_records += 1
                    
                    # Filter by symbol
                    if data['symbol'] not in symbols:
                        continue
                    
                    try:
                        # Parse timestamp
                        timestamp = datetime.fromisoformat(data['timestamp'])
                        if timestamp.tzinfo is None:
                            timestamp = timestamp.replace(tzinfo=timezone.utc)
                        
                        # Filter by time range
                        if timestamp < start_time or timestamp > end_time:
                            continue
                        
                        # Create MarketData object (will validate OHLC)
                        market_data = MarketData(
                            time=timestamp,
                            symbol=data['symbol'],
                            open=data['open'],
                            high=data['high'],
                            low=data['low'],
                            close=data['close'],
                            volume=data['volume'],
                            provider=self.name,
                            metadata={'source_file': file_key, 'line_number': line_num}
                        )
                        
                        chunk.append(market_data)
                        checkpoint.last_timestamp = timestamp
                        
                        # Yield chunk when full
                        if len(chunk) >= self.CHUNK_SIZE:
                            for item in chunk:
                                yield item
                                
                                # Update metrics
                                metrics.data_points_processed.labels(
                                    provider=self.name,
                                    data_type=DataType.MARKET_DATA.value
                                ).inc()
                            
                            chunk = []
                            
                            # Update checkpoint periodically
                            checkpoint.processed_lines = line_num
                            self._checkpoints[file_key] = checkpoint
                            
                            # Allow other tasks to run
                            await asyncio.sleep(0)
                    
                    except ValueError as e:
                        self.logger.warning(f"Invalid data at line {line_num} in {file_path}: {e}")
                        checkpoint.bad_records += 1
                        
                        # Check bad record percentage
                        bad_percentage = checkpoint.bad_records / checkpoint.total_records
                        if bad_percentage > self.MAX_BAD_RECORD_PERCENTAGE:
                            raise ValueError(
                                f"Bad record percentage ({bad_percentage:.2%}) exceeds "
                                f"maximum allowed ({self.MAX_BAD_RECORD_PERCENTAGE:.2%}) "
                                f"in file {file_path}"
                            )
                
                # Yield remaining items
                for item in chunk:
                    yield item
                    metrics.data_points_processed.labels(
                        provider=self.name,
                        data_type=DataType.MARKET_DATA.value
                    ).inc()
                
                # Mark file as completed
                checkpoint.processed_lines = line_num if 'line_num' in locals() else checkpoint.processed_lines
                self._checkpoints[file_key] = checkpoint
                
                self.logger.info(
                    f"Completed processing {file_path}: "
                    f"{checkpoint.total_records} total records, "
                    f"{checkpoint.bad_records} bad records "
                    f"({checkpoint.bad_records/checkpoint.total_records*100:.2f}%)"
                )
        
        except Exception as e:
            self.logger.error(f"Error processing file {file_path}: {e}")
            metrics.processing_errors.labels(
                provider=self.name,
                error_type=type(e).__name__
            ).inc()
            raise
        
        finally:
            # Remove from active files
            self._active_files.discard(file_key)
            
            # Save checkpoint
            await self._save_checkpoints()
    
    async def get_market_data(
        self,
        symbols: List[str],
        start_time: datetime,
        end_time: datetime,
        interval: str = "1min"
    ) -> AsyncIterator[MarketData]:
        """
        Fetch historical market data from files.
        
        Args:
            symbols: List of symbols to fetch
            start_time: Start time for data
            end_time: End time for data
            interval: Data interval (used for timestamp normalization)
        """
        symbols = self._validate_symbols(symbols)
        symbol_set = set(symbols)
        
        # Find relevant files
        files = self._find_data_files(symbols, start_time, end_time)
        
        if not files:
            self.logger.warning(f"No data files found for symbols {symbols} in range {start_time} to {end_time}")
            return
        
        self.logger.info(f"Processing {len(files)} files for symbols {symbols}")
        
        # Process files sequentially to maintain order
        for file_path in files:
            self.logger.info(f"Processing file: {file_path}")
            
            try:
                async for data in self._process_file(file_path, symbol_set, start_time, end_time):
                    yield data
                    
            except Exception as e:
                self.logger.error(f"Failed to process file {file_path}: {e}")
                
                # Check if we should continue or fail
                if "Bad record percentage" in str(e):
                    raise  # Re-raise bad record percentage errors
                
                # Continue with next file for other errors
                continue
    
    async def stream_market_data(self, symbols: List[str]) -> AsyncIterator[MarketData]:
        """
        File provider does not support real-time streaming.
        This method raises NotImplementedError.
        """
        raise NotImplementedError("FileProvider does not support real-time streaming")
    
    def get_checkpoint_status(self) -> Dict[str, Any]:
        """Get current checkpoint status for monitoring."""
        active_count = len(self._active_files)
        completed_count = len([
            cp for cp in self._checkpoints.values()
            if cp.file_path not in self._active_files
        ])
        
        total_records = sum(cp.total_records for cp in self._checkpoints.values())
        total_bad_records = sum(cp.bad_records for cp in self._checkpoints.values())
        
        return {
            'active_files': active_count,
            'completed_files': completed_count,
            'total_checkpoints': len(self._checkpoints),
            'total_records_processed': total_records,
            'total_bad_records': total_bad_records,
            'bad_record_percentage': (total_bad_records / total_records * 100) if total_records > 0 else 0
        }
    
    async def clear_checkpoints(self, file_patterns: Optional[List[str]] = None):
        """
        Clear checkpoints for specific files or all files.
        
        Args:
            file_patterns: Optional list of file patterns to clear. If None, clears all.
        """
        if file_patterns:
            # Clear specific patterns
            cleared = 0
            for pattern in file_patterns:
                for key in list(self._checkpoints.keys()):
                    if pattern in key:
                        del self._checkpoints[key]
                        cleared += 1
            
            self.logger.info(f"Cleared {cleared} checkpoints matching patterns {file_patterns}")
        else:
            # Clear all
            count = len(self._checkpoints)
            self._checkpoints.clear()
            self.logger.info(f"Cleared all {count} checkpoints")
        
        await self._save_checkpoints()