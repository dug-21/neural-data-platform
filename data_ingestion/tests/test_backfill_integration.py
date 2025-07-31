"""
Comprehensive Integration Tests for Backfill Functionality

This module contains integration tests covering:
1. Timezone handling and Unix nanosecond conversion
2. File format handling (CSV, CSV.GZ, JSON, Parquet)
3. Directory traversal and recursive search
4. Symbol filtering (single and multiple symbols)
5. Date range filtering with timezone awareness
6. End-to-end data flow to TimescaleDB
7. Performance with large datasets
8. Error handling and recovery
9. Checkpoint and resume functionality
"""

import pytest
import asyncio
import gzip
import json
import tempfile
import shutil
import pytz
from pathlib import Path
from datetime import datetime, timedelta, timezone
from unittest.mock import AsyncMock, Mock, patch
import pandas as pd
import numpy as np
from typing import List, Dict, Any, Optional

# System imports
import sys
import os
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from utils.file_backfill import FileBackfillHandler
from cli.backfill import BackfillCLI
from providers.historical_backfill import (
    HistoricalBackfillCoordinator,
    BackfillJob,
    BackfillPriority,
    DataGranularity,
    BackfillStatus,
    DataValidationResult
)
from storage.timescale import TimescaleDB
from storage.redis_store import RedisStore
from utils.logging import get_logger


logger = get_logger(__name__)


class TestTimezoneHandling:
    """Test timezone handling and Unix nanosecond conversion"""
    
    def test_unix_nanosecond_to_datetime_conversion(self):
        """Test Unix nanosecond to timezone-aware datetime conversion."""
        # Test cases with different timezones
        test_cases = [
            # (unix_nanoseconds, expected_timezone, expected_datetime_utc)
            (1704067200000000000, 'UTC', datetime(2024, 1, 1, 0, 0, 0, tzinfo=timezone.utc)),
            (1704153600000000000, 'UTC', datetime(2024, 1, 2, 0, 0, 0, tzinfo=timezone.utc)),
            (1704067200000000000, 'US/Eastern', datetime(2024, 1, 1, 0, 0, 0, tzinfo=timezone.utc)),
            (1704067200000000000, 'Asia/Tokyo', datetime(2024, 1, 1, 0, 0, 0, tzinfo=timezone.utc)),
        ]
        
        for unix_nanos, tz_name, expected in test_cases:
            # Convert unix nanoseconds to datetime
            unix_seconds = unix_nanos / 1_000_000_000
            dt = datetime.fromtimestamp(unix_seconds, tz=timezone.utc)
            
            assert dt == expected, f"Failed for {unix_nanos} in {tz_name}"
    
    def test_timezone_aware_date_comparisons(self):
        """Test timezone-aware date comparisons for filtering."""
        # Create test data with different timezones
        utc_time = datetime(2024, 1, 15, 12, 0, 0, tzinfo=timezone.utc)
        
        # Convert to different timezones
        eastern = utc_time.astimezone(pytz.timezone('US/Eastern'))
        pacific = utc_time.astimezone(pytz.timezone('US/Pacific'))
        tokyo = utc_time.astimezone(pytz.timezone('Asia/Tokyo'))
        
        # Filter bounds
        start_date = datetime(2024, 1, 1, tzinfo=timezone.utc)
        end_date = datetime(2024, 2, 1, tzinfo=timezone.utc)
        
        # All should be within range when converted back to UTC
        times_to_test = [utc_time, eastern, pacific, tokyo]
        
        for time_obj in times_to_test:
            # Normalize to UTC for comparison
            utc_normalized = time_obj.astimezone(timezone.utc)
            
            assert start_date <= utc_normalized <= end_date, \
                f"Time {time_obj} (UTC: {utc_normalized}) not in range"
    
    def test_timezone_edge_cases(self):
        """Test edge cases with timezone conversions."""
        # Daylight saving time transition
        dst_transition = datetime(2024, 3, 10, 7, 0, 0, tzinfo=timezone.utc)  # 2AM EST -> 3AM EDT
        
        # Convert to Eastern time
        eastern_tz = pytz.timezone('US/Eastern')
        eastern_time = dst_transition.astimezone(eastern_tz)
        
        # Convert back to UTC
        back_to_utc = eastern_time.astimezone(timezone.utc)
        
        assert back_to_utc == dst_transition, "DST conversion failed"
        
        # Test leap year
        leap_day = datetime(2024, 2, 29, 12, 0, 0, tzinfo=timezone.utc)
        assert leap_day.year == 2024 and leap_day.month == 2 and leap_day.day == 29
        
        # Test year boundaries
        year_boundary = datetime(2023, 12, 31, 23, 59, 59, tzinfo=timezone.utc)
        next_second = year_boundary + timedelta(seconds=1)
        assert next_second.year == 2024


class TestFileFormatHandling:
    """Test handling of different file formats"""
    
    @pytest.fixture
    def sample_market_data(self):
        """Create sample market data for testing."""
        return [
            {
                'timestamp': '2024-01-01 09:30:00',
                'symbol': 'AAPL',
                'open': 150.0,
                'high': 152.0,
                'low': 149.0,
                'close': 151.0,
                'volume': 1000000
            },
            {
                'timestamp': '2024-01-01 09:31:00',
                'symbol': 'AAPL',
                'open': 151.0,
                'high': 153.0,
                'low': 150.0,
                'close': 152.0,
                'volume': 1100000
            }
        ]
    
    @pytest.fixture
    def temp_dir(self):
        """Create temporary directory for test files."""
        temp_dir = tempfile.mkdtemp()
        yield Path(temp_dir)
        shutil.rmtree(temp_dir)
    
    def test_compressed_csv_gz_file_processing(self, sample_market_data, temp_dir):
        """Test .csv.gz file processing."""
        # Create compressed CSV file
        csv_content = "timestamp,symbol,open,high,low,close,volume\n"
        for data in sample_market_data:
            csv_content += f"{data['timestamp']},{data['symbol']},{data['open']},{data['high']},{data['low']},{data['close']},{data['volume']}\n"
        
        gz_file = temp_dir / "test_data.csv.gz"
        with gzip.open(gz_file, 'wt', encoding='utf-8') as f:
            f.write(csv_content)
        
        # Test reading compressed file
        df = pd.read_csv(gz_file, compression='gzip')
        
        assert len(df) == 2
        assert list(df.columns) == ['timestamp', 'symbol', 'open', 'high', 'low', 'close', 'volume']
        assert df.iloc[0]['symbol'] == 'AAPL'
        assert df.iloc[0]['open'] == 150.0
    
    def test_uncompressed_csv_file_processing(self, sample_market_data, temp_dir):
        """Test .csv file processing."""
        # Create uncompressed CSV file
        csv_file = temp_dir / "test_data.csv"
        df = pd.DataFrame(sample_market_data)
        df.to_csv(csv_file, index=False)
        
        # Test reading uncompressed file
        loaded_df = pd.read_csv(csv_file)
        
        assert len(loaded_df) == 2
        assert list(loaded_df.columns) == ['timestamp', 'symbol', 'open', 'high', 'low', 'close', 'volume']
        assert loaded_df.iloc[0]['symbol'] == 'AAPL'
    
    def test_json_file_processing(self, sample_market_data, temp_dir):
        """Test JSON file processing."""
        # Create JSON file
        json_file = temp_dir / "test_data.json"
        with open(json_file, 'w') as f:
            json.dump(sample_market_data, f)
        
        # Test reading JSON file
        with open(json_file, 'r') as f:
            loaded_data = json.load(f)
        
        assert len(loaded_data) == 2
        assert loaded_data[0]['symbol'] == 'AAPL'
        assert loaded_data[0]['open'] == 150.0
    
    def test_parquet_file_processing(self, sample_market_data, temp_dir):
        """Test Parquet file processing."""
        pytest.importorskip("pyarrow")
        
        # Create Parquet file
        parquet_file = temp_dir / "test_data.parquet"
        df = pd.DataFrame(sample_market_data)
        df.to_parquet(parquet_file)
        
        # Test reading Parquet file
        loaded_df = pd.read_parquet(parquet_file)
        
        assert len(loaded_df) == 2
        assert loaded_df.iloc[0]['symbol'] == 'AAPL'
        assert loaded_df.iloc[0]['open'] == 150.0
    
    def test_invalid_corrupted_files(self, temp_dir):
        """Test handling of invalid/corrupted files."""
        # Create corrupted CSV
        corrupted_csv = temp_dir / "corrupted.csv"
        with open(corrupted_csv, 'w') as f:
            f.write("invalid,csv,data\nwith,missing,\nand,incomplete")
        
        # Should handle gracefully
        try:
            df = pd.read_csv(corrupted_csv)
            # File might load but with inconsistent columns
            assert len(df.columns) >= 2  # Basic structure check
        except Exception as e:
            # Should not crash the entire process
            assert isinstance(e, (pd.errors.EmptyDataError, pd.errors.ParserError))
        
        # Create corrupted gzip file
        corrupted_gz = temp_dir / "corrupted.csv.gz"
        with open(corrupted_gz, 'wb') as f:
            f.write(b"not a gzip file")
        
        # Should handle gzip errors gracefully
        with pytest.raises((gzip.BadGzipFile, OSError)):
            pd.read_csv(corrupted_gz, compression='gzip')


class TestDirectoryTraversal:
    """Test recursive directory search and file discovery"""
    
    @pytest.fixture
    def nested_directory_structure(self, tmp_path):
        """Create nested directory structure with test files."""
        # Create directory structure
        # root/
        #   2023/
        #     01/
        #       AAPL_2023-01.csv
        #       MSFT_2023-01.csv.gz
        #     02/
        #       AAPL_2023-02.csv
        #       TSLA_2023-02.json
        #   2024/
        #     01/
        #       AAPL_2024-01.csv
        #       mixed_data.parquet
        #   ignored.txt
        
        structure = {
            '2023/01': ['AAPL_2023-01.csv', 'MSFT_2023-01.csv.gz'],
            '2023/02': ['AAPL_2023-02.csv', 'TSLA_2023-02.json'],
            '2024/01': ['AAPL_2024-01.csv', 'mixed_data.parquet'],
            '.': ['ignored.txt']
        }
        
        created_files = []
        for path_str, files in structure.items():
            path = tmp_path / path_str
            path.mkdir(parents=True, exist_ok=True)
            
            for filename in files:
                file_path = path / filename
                if filename.endswith('.csv'):
                    content = "timestamp,symbol,open,high,low,close,volume\n2023-01-01,AAPL,150,151,149,150.5,1000000\n"
                elif filename.endswith('.csv.gz'):
                    content = "timestamp,symbol,open,high,low,close,volume\n2023-01-01,MSFT,300,301,299,300.5,500000\n"
                    with gzip.open(file_path, 'wt') as f:
                        f.write(content)
                    created_files.append(file_path)
                    continue
                elif filename.endswith('.json'):
                    content = '[{"timestamp": "2023-02-01", "symbol": "TSLA", "close": 200.0}]'
                elif filename.endswith('.parquet'):
                    df = pd.DataFrame([{"timestamp": "2024-01-01", "symbol": "NVDA", "close": 400.0}])
                    df.to_parquet(file_path)
                    created_files.append(file_path)
                    continue
                else:
                    content = "This is not a data file"
                
                with open(file_path, 'w') as f:
                    f.write(content)
                created_files.append(file_path)
        
        return tmp_path, created_files
    
    def test_recursive_directory_search(self, nested_directory_structure):
        """Test recursive directory traversal."""
        root_path, created_files = nested_directory_structure
        
        # Test finding all CSV files recursively
        csv_files = list(root_path.rglob("*.csv"))
        assert len(csv_files) == 3  # AAPL files in 2023/01, 2023/02, 2024/01
        
        # Verify paths
        csv_paths = [str(f.relative_to(root_path)) for f in csv_files]
        expected_paths = [
            "2023/01/AAPL_2023-01.csv",
            "2023/02/AAPL_2023-02.csv", 
            "2024/01/AAPL_2024-01.csv"
        ]
        
        for path in expected_paths:
            assert any(path in csv_path for csv_path in csv_paths), f"Missing {path}"
    
    def test_nested_year_month_structure(self, nested_directory_structure):
        """Test handling of nested year/month directory structure."""
        root_path, _ = nested_directory_structure
        
        # Find files by year
        year_2023_files = list((root_path / "2023").rglob("*"))
        year_2024_files = list((root_path / "2024").rglob("*"))
        
        # Filter out directories
        year_2023_files = [f for f in year_2023_files if f.is_file()]
        year_2024_files = [f for f in year_2024_files if f.is_file()]
        
        assert len(year_2023_files) == 4  # 2 files in 01/, 2 files in 02/
        assert len(year_2024_files) == 2  # 2 files in 01/
        
        # Test month-level access
        jan_2023_files = list((root_path / "2023" / "01").glob("*"))
        feb_2023_files = list((root_path / "2023" / "02").glob("*"))
        
        assert len(jan_2023_files) == 2
        assert len(feb_2023_files) == 2
    
    def test_mixed_file_types_in_directory(self, nested_directory_structure):
        """Test handling directories with mixed file types."""
        root_path, _ = nested_directory_structure
        
        # Get all files recursively
        all_files = list(root_path.rglob("*"))
        file_objects = [f for f in all_files if f.is_file()]
        
        # Group by extension
        extensions = {}
        for file_obj in file_objects:
            if file_obj.suffix == '.gz':
                # Handle .csv.gz files
                ext = ''.join(file_obj.suffixes)
            else:
                ext = file_obj.suffix
            
            if ext not in extensions:
                extensions[ext] = []
            extensions[ext].append(file_obj)
        
        # Verify we have different file types
        assert '.csv' in extensions
        assert '.csv.gz' in extensions
        assert '.json' in extensions
        assert '.txt' in extensions
        
        # CSV files should be most common
        assert len(extensions['.csv']) >= 3


class TestSymbolFiltering:
    """Test symbol filtering functionality"""
    
    @pytest.fixture
    def multi_symbol_data(self):
        """Create test data with multiple symbols."""
        symbols = ['AAPL', 'MSFT', 'GOOGL', 'TSLA', 'NVDA']
        data = []
        base_time = datetime(2024, 1, 1, 9, 30)
        
        for i, symbol in enumerate(symbols):
            for j in range(10):  # 10 data points per symbol
                data.append({
                    'timestamp': (base_time + timedelta(minutes=j)).isoformat(),
                    'symbol': symbol,
                    'open': 100.0 + i * 50 + j,
                    'high': 102.0 + i * 50 + j,
                    'low': 99.0 + i * 50 + j,
                    'close': 101.0 + i * 50 + j,
                    'volume': 1000000 + j * 100000
                })
        
        return data
    
    def test_single_symbol_filtering(self, multi_symbol_data):
        """Test filtering by single symbol."""
        df = pd.DataFrame(multi_symbol_data)
        
        # Filter for AAPL only
        aapl_data = df[df['symbol'] == 'AAPL']
        
        assert len(aapl_data) == 10
        assert all(aapl_data['symbol'] == 'AAPL')
        assert aapl_data.iloc[0]['open'] == 100.0  # First AAPL record
    
    def test_multiple_symbol_filtering(self, multi_symbol_data):
        """Test filtering by multiple symbols."""
        df = pd.DataFrame(multi_symbol_data)
        
        # Filter for AAPL and TSLA
        target_symbols = ['AAPL', 'TSLA']
        filtered_data = df[df['symbol'].isin(target_symbols)]
        
        assert len(filtered_data) == 20  # 10 records each for AAPL and TSLA
        assert set(filtered_data['symbol'].unique()) == set(target_symbols)
    
    def test_case_sensitivity_handling(self, multi_symbol_data):
        """Test case sensitivity in symbol filtering."""
        df = pd.DataFrame(multi_symbol_data)
        
        # Test lowercase filtering
        lowercase_filter = df[df['symbol'].str.lower() == 'aapl']
        assert len(lowercase_filter) == 10
        
        # Test mixed case
        mixed_case_symbols = ['aapl', 'MSFT', 'Googl']
        # Normalize both sides for comparison
        normalized_filter = df[df['symbol'].str.upper().isin([s.upper() for s in mixed_case_symbols])]
        expected_symbols = ['AAPL', 'MSFT', 'GOOGL']
        assert set(normalized_filter['symbol'].unique()) == set(expected_symbols)
    
    def test_symbol_filtering_performance(self, multi_symbol_data):
        """Test performance of symbol filtering with large datasets."""
        # Create larger dataset
        large_data = multi_symbol_data * 1000  # 50,000 records
        df = pd.DataFrame(large_data)
        
        import time
        
        # Time the filtering operation
        start_time = time.time()
        filtered = df[df['symbol'] == 'AAPL']
        end_time = time.time()
        
        # Should complete quickly (under 1 second for this size)
        assert end_time - start_time < 1.0
        assert len(filtered) == 10000  # 10 * 1000
    
    def test_symbol_not_found_handling(self, multi_symbol_data):
        """Test handling when requested symbol is not found."""
        df = pd.DataFrame(multi_symbol_data)
        
        # Filter for non-existent symbol
        empty_result = df[df['symbol'] == 'NONEXISTENT']
        
        assert len(empty_result) == 0
        assert empty_result.empty
        
        # Filter with some existing and some non-existent
        mixed_symbols = ['AAPL', 'NONEXISTENT', 'MSFT', 'FAKE']
        mixed_result = df[df['symbol'].isin(mixed_symbols)]
        
        # Should only return data for existing symbols
        assert len(mixed_result) == 20  # 10 each for AAPL and MSFT
        assert set(mixed_result['symbol'].unique()) == {'AAPL', 'MSFT'}


class TestDateRangeFiltering:
    """Test timezone-aware date range filtering"""
    
    @pytest.fixture
    def time_series_data(self):
        """Create time series data spanning multiple days and timezones."""
        data = []
        start_date = datetime(2024, 1, 1, tzinfo=timezone.utc)
        
        # Create data every hour for 7 days
        for i in range(24 * 7):  # 168 hours
            timestamp = start_date + timedelta(hours=i)
            data.append({
                'timestamp': timestamp.isoformat(),
                'symbol': 'AAPL',
                'open': 150.0 + (i % 24),  # Daily pattern
                'high': 152.0 + (i % 24),
                'low': 149.0 + (i % 24),
                'close': 151.0 + (i % 24),
                'volume': 1000000 + i * 10000
            })
        
        return data
    
    def test_timezone_aware_date_filtering(self, time_series_data):
        """Test timezone-aware date filtering."""
        df = pd.DataFrame(time_series_data)
        df['timestamp'] = pd.to_datetime(df['timestamp'])
        
        # Filter for January 2-4, 2024 (UTC)
        start_filter = datetime(2024, 1, 2, tzinfo=timezone.utc)
        end_filter = datetime(2024, 1, 4, tzinfo=timezone.utc)
        
        filtered_data = df[
            (df['timestamp'] >= start_filter) & 
            (df['timestamp'] <= end_filter)
        ]
        
        # Should include 48 hours of data (2 full days)
        assert len(filtered_data) == 49  # Jan 2 00:00 to Jan 4 00:00 inclusive
        
        # Verify date boundaries
        assert filtered_data['timestamp'].min().date() == start_filter.date()
        assert filtered_data['timestamp'].max().date() == end_filter.date()
    
    def test_cross_timezone_date_filtering(self, time_series_data):
        """Test date filtering across different timezones."""
        df = pd.DataFrame(time_series_data)
        df['timestamp'] = pd.to_datetime(df['timestamp'])
        
        # Convert filter dates to different timezones
        eastern_tz = pytz.timezone('US/Eastern')
        pacific_tz = pytz.timezone('US/Pacific')
        
        # Jan 2, 2024 midnight Eastern = Jan 2, 2024 5:00 AM UTC
        start_eastern = eastern_tz.localize(datetime(2024, 1, 2, 0, 0, 0))
        start_utc = start_eastern.astimezone(timezone.utc)
        
        # Jan 3, 2024 midnight Pacific = Jan 3, 2024 8:00 AM UTC
        end_pacific = pacific_tz.localize(datetime(2024, 1, 3, 0, 0, 0))
        end_utc = end_pacific.astimezone(timezone.utc)
        
        filtered_data = df[
            (df['timestamp'] >= start_utc) & 
            (df['timestamp'] <= end_utc)
        ]
        
        # Verify the range spans the expected hours
        hours_span = (end_utc - start_utc).total_seconds() / 3600
        assert len(filtered_data) == int(hours_span) + 1  # +1 for inclusive end
    
    def test_daylight_saving_time_handling(self, time_series_data):
        """Test handling of daylight saving time transitions."""
        # Create data around DST transition (March 10, 2024 - Spring forward)
        dst_data = []
        base_time = datetime(2024, 3, 9, 12, 0, 0, tzinfo=timezone.utc)
        
        for i in range(48):  # 48 hours around DST transition
            timestamp = base_time + timedelta(hours=i)
            dst_data.append({
                'timestamp': timestamp.isoformat(),
                'symbol': 'SPY',
                'close': 400.0 + i
            })
        
        df = pd.DataFrame(dst_data)
        df['timestamp'] = pd.to_datetime(df['timestamp'])
        
        # Filter for the DST transition day in Eastern timezone
        eastern_tz = pytz.timezone('US/Eastern')
        dst_date = datetime(2024, 3, 10)  # DST transition day
        
        # Convert to UTC for filtering
        start_utc = eastern_tz.localize(dst_date).astimezone(timezone.utc)
        end_utc = eastern_tz.localize(dst_date + timedelta(days=1)).astimezone(timezone.utc)
        
        filtered_data = df[
            (df['timestamp'] >= start_utc) & 
            (df['timestamp'] < end_utc)
        ]
        
        # Should have 23 hours of data due to spring forward (2 AM -> 3 AM)
        # But since our test data is in UTC, we still get 24 hours
        assert len(filtered_data) == 24
    
    def test_boundary_conditions(self, time_series_data):
        """Test edge cases and boundary conditions."""
        df = pd.DataFrame(time_series_data)
        df['timestamp'] = pd.to_datetime(df['timestamp'])
        
        # Test exact boundary matches
        first_timestamp = df['timestamp'].min()
        last_timestamp = df['timestamp'].max()
        
        # Filter with exact boundaries
        exact_match = df[
            (df['timestamp'] >= first_timestamp) & 
            (df['timestamp'] <= last_timestamp)
        ]
        
        assert len(exact_match) == len(df)  # Should include all data
        
        # Test microsecond precision
        precise_start = first_timestamp + timedelta(microseconds=1)
        precise_filter = df[df['timestamp'] >= precise_start]
        
        assert len(precise_filter) == len(df) - 1  # Should exclude first record
        
        # Test future date filtering
        future_date = last_timestamp + timedelta(days=1)
        future_filter = df[df['timestamp'] >= future_date]
        
        assert len(future_filter) == 0  # Should be empty


@pytest.mark.integration
class TestEndToEndDataFlow:
    """Test end-to-end data flow to TimescaleDB"""
    
    @pytest.fixture
    def mock_timescale_db(self):
        """Mock TimescaleDB for testing."""
        db = AsyncMock(spec=TimescaleDB)
        db.connect = AsyncMock()
        db.disconnect = AsyncMock()
        db.insert_market_data = AsyncMock()
        db.execute = AsyncMock()
        db.fetch = AsyncMock(return_value=[])
        return db
    
    @pytest.fixture
    def mock_redis_store(self):
        """Mock Redis store for checkpointing."""
        redis = AsyncMock(spec=RedisStore)
        redis.connect = AsyncMock()
        redis.disconnect = AsyncMock()
        redis.cache_set = AsyncMock()
        redis.cache_get = AsyncMock(return_value=None)
        return redis
    
    @pytest.mark.asyncio
    async def test_complete_file_to_database_flow(self, tmp_path, mock_timescale_db, mock_redis_store):
        """Test complete flow from file reading to database insertion."""
        # Create test CSV file
        test_data = [
            ['timestamp', 'symbol', 'open', 'high', 'low', 'close', 'volume'],
            ['2024-01-01 09:30:00', 'AAPL', '150.0', '152.0', '149.0', '151.0', '1000000'],
            ['2024-01-01 09:31:00', 'AAPL', '151.0', '153.0', '150.0', '152.0', '1100000'],
            ['2024-01-01 09:32:00', 'AAPL', '152.0', '154.0', '151.0', '153.0', '1200000']
        ]
        
        csv_file = tmp_path / "test_data.csv"
        with open(csv_file, 'w') as f:
            for row in test_data:
                f.write(','.join(row) + '\n')
        
        # Create file backfill handler with mocked dependencies
        handler = FileBackfillHandler(
            path=csv_file,
            format='csv',
            symbols=['AAPL'],
            batch_size=10,
            use_checkpoint=True,
            dry_run=False
        )
        
        # Inject mocked dependencies
        handler.storage = mock_timescale_db
        handler.redis_store = mock_redis_store
        
        # Run the backfill
        await handler.run()
        
        # Verify database connection was established
        mock_timescale_db.connect.assert_called_once()
        
        # Verify data was inserted
        mock_timescale_db.insert_market_data.assert_called()
        
        # Verify checkpoint was created
        mock_redis_store.cache_set.assert_called()
        
        # Verify cleanup
        mock_timescale_db.disconnect.assert_called_once()
    
    @pytest.mark.asyncio
    async def test_batch_processing_performance(self, tmp_path, mock_timescale_db):
        """Test batch processing performance with different batch sizes."""
        # Create larger test file
        num_records = 10000
        csv_file = tmp_path / "large_test.csv"
        
        with open(csv_file, 'w') as f:
            f.write('timestamp,symbol,open,high,low,close,volume\n')
            for i in range(num_records):
                timestamp = datetime(2024, 1, 1, 9, 30) + timedelta(seconds=i)
                f.write(f'{timestamp.isoformat()},AAPL,{150+i*0.01},{151+i*0.01},{149+i*0.01},{150.5+i*0.01},{1000000+i}\n')
        
        # Test different batch sizes
        batch_sizes = [100, 1000, 5000]
        
        for batch_size in batch_sizes:
            # Reset mock
            mock_timescale_db.reset_mock()
            
            handler = FileBackfillHandler(
                path=csv_file,
                format='csv',
                batch_size=batch_size,
                dry_run=False
            )
            handler.storage = mock_timescale_db
            
            # Run and measure
            import time
            start_time = time.time()
            await handler.run()
            end_time = time.time()
            
            # Verify processing completed
            assert mock_timescale_db.insert_market_data.called
            
            # Larger batch sizes should generally be faster
            processing_time = end_time - start_time
            assert processing_time < 10.0  # Should complete within 10 seconds
    
    @pytest.mark.asyncio
    async def test_error_handling_and_recovery(self, tmp_path, mock_timescale_db):
        """Test error handling and recovery mechanisms."""
        # Create test file
        csv_file = tmp_path / "test_error.csv"
        with open(csv_file, 'w') as f:
            f.write('timestamp,symbol,open,high,low,close,volume\n')
            f.write('2024-01-01 09:30:00,AAPL,150.0,152.0,149.0,151.0,1000000\n')
            f.write('invalid_timestamp,AAPL,150.0,152.0,149.0,151.0,1000000\n')  # Invalid row
            f.write('2024-01-01 09:32:00,AAPL,152.0,154.0,151.0,153.0,1200000\n')
        
        # Configure mock to raise error on second batch
        call_count = 0
        def side_effect(*args, **kwargs):
            nonlocal call_count
            call_count += 1
            if call_count == 2:
                raise Exception("Database connection error")
            return None
        
        mock_timescale_db.insert_market_data.side_effect = side_effect
        
        handler = FileBackfillHandler(
            path=csv_file,
            format='csv',
            batch_size=1,  # Process one record at a time
            dry_run=False
        )
        handler.storage = mock_timescale_db
        
        # Should handle error gracefully
        with pytest.raises(Exception):
            await handler.run()
        
        # Should have attempted at least one insert
        assert mock_timescale_db.insert_market_data.called
    
    @pytest.mark.asyncio
    async def test_data_transformation_and_validation(self, tmp_path, mock_timescale_db):
        """Test data transformation and validation during processing."""
        # Create test file with various data quality issues
        test_data = [
            'timestamp,symbol,open,high,low,close,volume\n',
            '2024-01-01 09:30:00,AAPL,150.0,152.0,149.0,151.0,1000000\n',  # Valid
            '2024-01-01 09:31:00,AAPL,-10.0,152.0,149.0,151.0,1000000\n',  # Negative price
            '2024-01-01 09:32:00,AAPL,150.0,149.0,152.0,151.0,1000000\n',  # High < Low
            '2024-01-01 09:33:00,,150.0,152.0,149.0,151.0,1000000\n',      # Missing symbol
            '2024-01-01 09:34:00,AAPL,150.0,152.0,149.0,151.0,1000000\n'   # Valid
        ]
        
        csv_file = tmp_path / "validation_test.csv"
        with open(csv_file, 'w') as f:
            f.writelines(test_data)
        
        handler = FileBackfillHandler(
            path=csv_file,
            format='csv',
            batch_size=10,
            dry_run=False
        )
        handler.storage = mock_timescale_db
        
        await handler.run()
        
        # Should have processed some records (likely filtered invalid ones)
        assert mock_timescale_db.insert_market_data.called


@pytest.mark.performance
class TestPerformanceWithLargeDatasets:
    """Test performance with large datasets"""
    
    @pytest.mark.slow
    def test_large_csv_file_processing(self, tmp_path):
        """Test processing large CSV files efficiently."""
        # Create large CSV file (1M records)
        large_csv = tmp_path / "large_data.csv"
        num_records = 1_000_000
        
        with open(large_csv, 'w') as f:
            f.write('timestamp,symbol,open,high,low,close,volume\n')
            
            # Write in chunks to avoid memory issues
            chunk_size = 10000
            for chunk_start in range(0, num_records, chunk_size):
                chunk_data = []
                for i in range(chunk_start, min(chunk_start + chunk_size, num_records)):
                    timestamp = datetime(2024, 1, 1, 9, 30) + timedelta(seconds=i)
                    chunk_data.append(
                        f'{timestamp.isoformat()},AAPL,{150+i*0.001},{151+i*0.001},'
                        f'{149+i*0.001},{150.5+i*0.001},{1000000+i}'
                    )
                f.write('\n'.join(chunk_data) + '\n')
        
        # Test reading in chunks
        import time
        start_time = time.time()
        
        total_rows = 0
        for chunk in pd.read_csv(large_csv, chunksize=10000):
            total_rows += len(chunk)
            
            # Verify chunk processing
            assert len(chunk) <= 10000
            assert 'timestamp' in chunk.columns
        
        end_time = time.time()
        processing_time = end_time - start_time
        
        assert total_rows == num_records
        assert processing_time < 60.0  # Should process 1M records in under 1 minute
        
        # Calculate throughput
        throughput = total_rows / processing_time
        assert throughput > 10000  # Should process >10k records per second
    
    @pytest.mark.slow
    def test_memory_usage_during_processing(self, tmp_path):
        """Test memory usage remains controlled during large file processing."""
        pytest.importorskip("psutil")
        import psutil
        
        # Create moderately large file
        csv_file = tmp_path / "memory_test.csv"
        num_records = 100_000
        
        with open(csv_file, 'w') as f:
            f.write('timestamp,symbol,open,high,low,close,volume\n')
            for i in range(num_records):
                timestamp = datetime(2024, 1, 1, 9, 30) + timedelta(seconds=i)
                f.write(f'{timestamp.isoformat()},AAPL,{150+i*0.01},{151+i*0.01},{149+i*0.01},{150.5+i*0.01},{1000000+i}\n')
        
        # Monitor memory usage
        process = psutil.Process()
        initial_memory = process.memory_info().rss / 1024 / 1024  # MB
        peak_memory = initial_memory
        
        def memory_monitor():
            nonlocal peak_memory
            current_memory = process.memory_info().rss / 1024 / 1024
            peak_memory = max(peak_memory, current_memory)
        
        # Process file in chunks and monitor memory
        for chunk in pd.read_csv(csv_file, chunksize=5000):
            memory_monitor()
            
            # Simulate processing
            chunk_processed = chunk.copy()
            chunk_processed['processed'] = True
            
            memory_monitor()
            
            # Clean up chunk to help with memory
            del chunk_processed
        
        final_memory = process.memory_info().rss / 1024 / 1024
        memory_increase = peak_memory - initial_memory
        
        # Memory increase should be reasonable (less than 500MB for 100k records)
        assert memory_increase < 500
        
        # Memory should return close to initial levels
        assert abs(final_memory - initial_memory) < 100
    
    def test_concurrent_file_processing(self, tmp_path):
        """Test processing multiple files concurrently."""
        import concurrent.futures
        import time
        
        # Create multiple test files
        files = []
        for i in range(5):
            file_path = tmp_path / f"concurrent_test_{i}.csv"
            with open(file_path, 'w') as f:
                f.write('timestamp,symbol,open,high,low,close,volume\n')
                for j in range(10000):  # 10k records per file
                    timestamp = datetime(2024, 1, 1, 9, 30) + timedelta(seconds=j)
                    symbol = f'STOCK{i}'
                    f.write(f'{timestamp.isoformat()},{symbol},{150+j*0.01},{151+j*0.01},{149+j*0.01},{150.5+j*0.01},{1000000+j}\n')
            files.append(file_path)
        
        def process_file(file_path):
            """Process a single file."""
            start_time = time.time()
            total_rows = 0
            
            for chunk in pd.read_csv(file_path, chunksize=1000):
                total_rows += len(chunk)
                # Simulate processing time
                chunk['processed'] = True
            
            return {
                'file': file_path.name,
                'rows': total_rows,
                'time': time.time() - start_time
            }
        
        # Process files sequentially
        sequential_start = time.time()
        sequential_results = [process_file(f) for f in files]
        sequential_time = time.time() - sequential_start
        
        # Process files concurrently
        concurrent_start = time.time()
        with concurrent.futures.ThreadPoolExecutor(max_workers=3) as executor:
            concurrent_results = list(executor.map(process_file, files))
        concurrent_time = time.time() - concurrent_start
        
        # Verify results
        assert len(sequential_results) == len(concurrent_results) == 5
        
        # Concurrent should be faster (allowing for some overhead)
        assert concurrent_time < sequential_time * 0.8
        
        # All files should have been processed
        for result in concurrent_results:
            assert result['rows'] == 10000


@pytest.mark.asyncio
class TestCheckpointAndResume:
    """Test checkpoint and resume functionality"""
    
    @pytest.fixture
    def mock_redis_store(self):
        """Mock Redis store for checkpointing."""
        redis = AsyncMock(spec=RedisStore)
        redis.connect = AsyncMock()
        redis.disconnect = AsyncMock()
        redis.cache_set = AsyncMock()
        redis.cache_get = AsyncMock()
        redis.cache_scan = AsyncMock()
        return redis
    
    async def test_checkpoint_creation_and_retrieval(self, tmp_path, mock_redis_store):
        """Test creation and retrieval of checkpoints."""
        # Create test file
        csv_file = tmp_path / "checkpoint_test.csv"
        with open(csv_file, 'w') as f:
            f.write('timestamp,symbol,open,high,low,close,volume\n')
            for i in range(100):
                timestamp = datetime(2024, 1, 1, 9, 30) + timedelta(minutes=i)
                f.write(f'{timestamp.isoformat()},AAPL,{150+i*0.01},{151+i*0.01},{149+i*0.01},{150.5+i*0.01},{1000000+i}\n')
        
        # Configure mock to return no existing checkpoint
        mock_redis_store.cache_get.return_value = None
        
        handler = FileBackfillHandler(
            path=csv_file,
            format='csv',
            batch_size=10,
            use_checkpoint=True,
            dry_run=True
        )
        handler.redis_store = mock_redis_store
        
        await handler.run()
        
        # Verify checkpoint was created
        mock_redis_store.cache_set.assert_called()
        
        # Check checkpoint data structure
        checkpoint_calls = mock_redis_store.cache_set.call_args_list
        assert len(checkpoint_calls) > 0
        
        # Verify checkpoint key format
        checkpoint_key = checkpoint_calls[-1][0][0]  # First argument of last call
        assert 'file_backfill:' in checkpoint_key
        assert str(csv_file) in checkpoint_key
    
    async def test_resume_from_checkpoint(self, tmp_path, mock_redis_store):
        """Test resuming from an existing checkpoint."""
        # Create test file
        csv_file = tmp_path / "resume_test.csv"
        with open(csv_file, 'w') as f:
            f.write('timestamp,symbol,open,high,low,close,volume\n')
            for i in range(100):
                timestamp = datetime(2024, 1, 1, 9, 30) + timedelta(minutes=i)
                f.write(f'{timestamp.isoformat()},AAPL,{150+i*0.01},{151+i*0.01},{149+i*0.01},{150.5+i*0.01},{1000000+i}\n')
        
        # Configure mock to return existing checkpoint (50% complete)
        existing_checkpoint = {
            'file': str(csv_file),
            'records': 50,
            'completed': False,
            'timestamp': datetime.utcnow().isoformat()
        }
        mock_redis_store.cache_get.return_value = existing_checkpoint
        
        handler = FileBackfillHandler(
            path=csv_file,
            format='csv',
            batch_size=10,
            use_checkpoint=True,
            dry_run=True
        )
        handler.redis_store = mock_redis_store
        
        await handler.run()
        
        # Verify checkpoint was checked
        mock_redis_store.cache_get.assert_called()
        
        # Should still process (in dry run mode)
        # In real implementation, would skip already processed records
    
    async def test_checkpoint_corruption_handling(self, tmp_path, mock_redis_store):
        """Test handling of corrupted checkpoint data."""
        csv_file = tmp_path / "corruption_test.csv"
        with open(csv_file, 'w') as f:
            f.write('timestamp,symbol,open,high,low,close,volume\n')
            f.write('2024-01-01 09:30:00,AAPL,150.0,152.0,149.0,151.0,1000000\n')
        
        # Configure mock to return corrupted checkpoint
        mock_redis_store.cache_get.return_value = {"invalid": "checkpoint"}
        
        handler = FileBackfillHandler(
            path=csv_file,
            format='csv',
            use_checkpoint=True,
            dry_run=True
        )
        handler.redis_store = mock_redis_store
        
        # Should handle corrupted checkpoint gracefully
        await handler.run()
        
        # Should have attempted to create new checkpoint
        mock_redis_store.cache_set.assert_called()
    
    async def test_checkpoint_ttl_and_cleanup(self, mock_redis_store):
        """Test checkpoint TTL and cleanup behavior."""
        # Test that checkpoints are created with appropriate TTL
        handler = FileBackfillHandler(
            path=Path("/fake/path.csv"),
            format='csv',
            use_checkpoint=True,
            dry_run=True
        )
        handler.redis_store = mock_redis_store
        
        # Simulate checkpoint update
        await handler._update_checkpoint(Path("/fake/path.csv"), 100, completed=True)
        
        # Verify TTL was set (7 days = 7*24*3600 seconds)
        mock_redis_store.cache_set.assert_called()
        call_args = mock_redis_store.cache_set.call_args
        
        # Check that TTL parameter was passed
        assert 'ttl' in call_args.kwargs or len(call_args.args) >= 3


if __name__ == "__main__":
    pytest.main([__file__, "-v", "--tb=short"])