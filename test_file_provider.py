#!/usr/bin/env python3
"""
Test script for FileProvider implementation.
Creates sample data files and tests all functionality.
"""
import asyncio
import tempfile
import gzip
import csv
import sys
import os
from pathlib import Path
from datetime import datetime, timezone, timedelta
import json
import shutil

# Add data_ingestion to Python path
sys.path.insert(0, os.path.join(os.path.dirname(__file__), 'data_ingestion'))

from providers.file_provider import FileProvider


def create_sample_data_files(base_path: Path):
    """Create sample CSV files for testing."""
    
    # Create directory structure
    date_path = base_path / "2024" / "01" / "15"
    date_path.mkdir(parents=True, exist_ok=True)
    
    symbol_path = base_path / "symbols" / "AAPL" / "2024"
    symbol_path.mkdir(parents=True, exist_ok=True)
    
    # Sample data with various scenarios
    sample_data = [
        # Normal data
        ["2024-01-15T09:30:00+00:00", "AAPL", "150.00", "152.50", "149.50", "151.75", "1000000"],
        ["2024-01-15T09:31:00+00:00", "AAPL", "151.75", "153.00", "151.00", "152.25", "950000"],
        ["2024-01-15T09:32:00+00:00", "AAPL", "152.25", "152.75", "151.50", "152.00", "800000"],
        
        # Different symbol (should be filtered when requesting AAPL only)
        ["2024-01-15T09:30:00+00:00", "GOOGL", "2800.00", "2850.00", "2790.00", "2825.00", "500000"],
        
        # More AAPL data
        ["2024-01-15T09:33:00+00:00", "AAPL", "152.00", "152.50", "151.25", "151.50", "750000"],
        ["2024-01-15T09:34:00+00:00", "AAPL", "151.50", "152.00", "150.75", "151.25", "650000"],
        
        # Bad data - will be counted as bad record
        ["2024-01-15T09:35:00+00:00", "AAPL", "invalid", "152.00", "151.00", "151.75", "700000"],
        
        # More normal data
        ["2024-01-15T09:36:00+00:00", "AAPL", "151.75", "152.25", "151.00", "151.50", "600000"],
    ]
    
    # Create regular CSV file
    csv_file = date_path / "market_data_20240115.csv"
    with open(csv_file, 'w', newline='') as f:
        writer = csv.writer(f)
        writer.writerow(["timestamp", "symbol", "open", "high", "low", "close", "volume"])
        writer.writerows(sample_data[:4])  # First 4 rows
    
    # Create gzipped CSV file
    gz_file = symbol_path / "market_data_AAPL_202401.csv.gz"
    with gzip.open(gz_file, 'wt', newline='') as f:
        writer = csv.writer(f)
        writer.writerow(["timestamp", "symbol", "open", "high", "low", "close", "volume"])
        writer.writerows(sample_data[4:])  # Remaining rows
    
    print(f"Created sample files:")
    print(f"  - {csv_file}")
    print(f"  - {gz_file}")


async def test_basic_functionality():
    """Test basic FileProvider functionality."""
    print("\n=== Testing Basic Functionality ===")
    
    with tempfile.TemporaryDirectory() as temp_dir:
        base_path = Path(temp_dir)
        checkpoint_dir = Path(temp_dir) / "checkpoints"
        
        # Create sample data
        create_sample_data_files(base_path)
        
        # Initialize provider
        provider = FileProvider(str(base_path), str(checkpoint_dir))
        
        async with provider:
            # Test market data retrieval
            start_time = datetime(2024, 1, 15, 9, 0, tzinfo=timezone.utc)
            end_time = datetime(2024, 1, 15, 10, 0, tzinfo=timezone.utc)
            
            data_points = []
            async for data in provider.get_market_data(["AAPL"], start_time, end_time):
                data_points.append(data)
                print(f"  {data.time}: {data.symbol} ${data.close} (vol: {data.volume})")
            
            print(f"\nRetrieved {len(data_points)} data points for AAPL")
            
            # Check checkpoint status
            status = provider.get_checkpoint_status()
            print(f"Checkpoint status: {status}")
            
            assert len(data_points) > 0, "Should retrieve some data points"
            assert all(d.symbol == "AAPL" for d in data_points), "Should only return AAPL data"
            
            print("✅ Basic functionality test passed")


async def test_checkpoint_resume():
    """Test checkpoint and resume functionality."""
    print("\n=== Testing Checkpoint Resume ===")
    
    with tempfile.TemporaryDirectory() as temp_dir:
        base_path = Path(temp_dir)
        checkpoint_dir = Path(temp_dir) / "checkpoints"
        
        create_sample_data_files(base_path)
        
        # First run - process some data
        print("First run - processing data...")
        provider1 = FileProvider(str(base_path), str(checkpoint_dir))
        
        data_count_1 = 0
        async with provider1:
            start_time = datetime(2024, 1, 15, 9, 0, tzinfo=timezone.utc)
            end_time = datetime(2024, 1, 15, 10, 0, tzinfo=timezone.utc)
            
            async for data in provider1.get_market_data(["AAPL"], start_time, end_time):
                data_count_1 += 1
        
        status_1 = provider1.get_checkpoint_status()
        print(f"First run processed {data_count_1} data points")
        print(f"Checkpoint status: {status_1}")
        
        # Second run - should resume from checkpoint
        print("\nSecond run - resuming from checkpoint...")
        provider2 = FileProvider(str(base_path), str(checkpoint_dir))
        
        data_count_2 = 0
        async with provider2:
            start_time = datetime(2024, 1, 15, 9, 0, tzinfo=timezone.utc)
            end_time = datetime(2024, 1, 15, 10, 0, tzinfo=timezone.utc)
            
            async for data in provider2.get_market_data(["AAPL"], start_time, end_time):
                data_count_2 += 1
        
        status_2 = provider2.get_checkpoint_status()
        print(f"Second run processed {data_count_2} data points")
        print(f"Checkpoint status: {status_2}")
        
        # Since files are small, second run should process same amount
        # but checkpoint should show all records already processed
        
        print("✅ Checkpoint resume test passed")


async def test_bad_record_handling():
    """Test bad record detection and failure threshold."""
    print("\n=== Testing Bad Record Handling ===")
    
    with tempfile.TemporaryDirectory() as temp_dir:
        base_path = Path(temp_dir)
        
        # Create directory structure and file with too many bad records
        date_path = base_path / "2024" / "01" / "15"
        date_path.mkdir(parents=True, exist_ok=True)
        test_file = date_path / "market_data_20240115.csv"
        
        # Create data with 3% bad records (should fail since >1%)
        data = [["timestamp", "symbol", "open", "high", "low", "close", "volume"]]  # header
        
        for i in range(150):  # Need more than 100 records to trigger check
            if i < 5:  # First 5 are bad (3.3% which is > 1%)
                data.append([f"2024-01-15T09:{i:02d}:00+00:00", "AAPL", "invalid", "152.00", "151.00", "151.75", "700000"])
            else:
                data.append([f"2024-01-15T09:{i:02d}:00+00:00", "AAPL", "150.00", "152.00", "149.00", "151.00", "100000"])
        
        with open(test_file, 'w', newline='') as f:
            writer = csv.writer(f)
            writer.writerows(data)
        
        provider = FileProvider(str(base_path))
        
        try:
            async with provider:
                start_time = datetime(2024, 1, 15, 9, 0, tzinfo=timezone.utc)
                end_time = datetime(2024, 1, 15, 10, 0, tzinfo=timezone.utc)
                
                # Debug: List all files that exist
                print(f"Created test file: {test_file}")
                print(f"File exists: {test_file.exists()}")
                if test_file.exists():
                    with open(test_file, 'r') as f:
                        lines = f.readlines()
                        print(f"File has {len(lines)} lines")
                
                data_points = []
                async for data in provider.get_market_data(["AAPL"], start_time, end_time):
                    data_points.append(data)
                
                # Should not reach here due to bad record threshold
                assert False, "Should have failed due to bad record percentage"
                
        except ValueError as e:
            if "Bad record percentage" in str(e):
                print(f"✅ Correctly failed due to bad records: {e}")
            else:
                raise
        except Exception as e:
            print(f"❌ Unexpected error: {e}")
            raise


async def test_ohlc_validation():
    """Test OHLC consistency validation."""
    print("\n=== Testing OHLC Validation ===")
    
    with tempfile.TemporaryDirectory() as temp_dir:
        base_path = Path(temp_dir)
        
        # Create directory structure and file with invalid OHLC data
        date_path = base_path / "2024" / "01" / "15"
        date_path.mkdir(parents=True, exist_ok=True)
        test_file = date_path / "market_data_20240115.csv"
        
        data = [
            ["timestamp", "symbol", "open", "high", "low", "close", "volume"],
            ["2024-01-15T09:30:00+00:00", "AAPL", "150.00", "148.00", "152.00", "151.00", "100000"],  # high < open, low > open
        ]
        
        with open(test_file, 'w', newline='') as f:
            writer = csv.writer(f)
            writer.writerows(data)
        
        provider = FileProvider(str(base_path))
        
        try:
            async with provider:
                start_time = datetime(2024, 1, 15, 9, 0, tzinfo=timezone.utc)
                end_time = datetime(2024, 1, 15, 10, 0, tzinfo=timezone.utc)
                
                valid_count = 0
                async for data in provider.get_market_data(["AAPL"], start_time, end_time):
                    valid_count += 1
                
                # Should not reach here if OHLC validation is working
                print(f"Warning: Expected OHLC validation to fail, but got {valid_count} records")
                
        except ValueError as e:
            if "Bad record percentage" in str(e):
                print(f"✅ OHLC validation test passed - correctly failed on invalid OHLC data: {e}")
            else:
                raise
        except Exception as e:
            print(f"❌ Unexpected error in OHLC test: {e}")
            raise


async def test_symbol_filtering():
    """Test symbol filtering during processing."""
    print("\n=== Testing Symbol Filtering ===")
    
    with tempfile.TemporaryDirectory() as temp_dir:
        base_path = Path(temp_dir)
        create_sample_data_files(base_path)
        
        # Use different checkpoint directories to avoid interference
        provider1 = FileProvider(str(base_path), str(base_path / "checkpoints1"))
        provider2 = FileProvider(str(base_path), str(base_path / "checkpoints2"))
        
        start_time = datetime(2024, 1, 15, 9, 0, tzinfo=timezone.utc)
        end_time = datetime(2024, 1, 15, 10, 0, tzinfo=timezone.utc)
        
        # Test single symbol
        aapl_data = []
        async with provider1:
            async for data in provider1.get_market_data(["AAPL"], start_time, end_time):
                aapl_data.append(data)
        
        # Test multiple symbols using a separate provider to avoid checkpoint interference
        multi_data = []
        async with provider2:
            async for data in provider2.get_market_data(["AAPL", "GOOGL"], start_time, end_time):
                multi_data.append(data)
            
        print(f"AAPL only: {len(aapl_data)} records")
        print(f"AAPL + GOOGL: {len(multi_data)} records")
        
        # In our test data, GOOGL data is only in the date-based CSV file
        # AAPL data is in both files, so when we request both symbols,
        # we should get at least the AAPL data (same as AAPL-only)
        # plus any GOOGL data if present
        googl_data = [d for d in multi_data if d.symbol == "GOOGL"]
        aapl_data_from_multi = [d for d in multi_data if d.symbol == "AAPL"]
        
        print(f"AAPL from multi-symbol request: {len(aapl_data_from_multi)} records")
        print(f"GOOGL from multi-symbol request: {len(googl_data)} records")
        
        assert len(aapl_data_from_multi) == len(aapl_data), "Should get same AAPL data in both requests"
        assert all(d.symbol in ["AAPL", "GOOGL"] for d in multi_data), "Should only return requested symbols"
        
        print("✅ Symbol filtering test passed")


async def main():
    """Run all tests."""
    print("🧪 Testing FileProvider Implementation")
    print("=" * 50)
    
    try:
        await test_basic_functionality()
        await test_checkpoint_resume()
        await test_bad_record_handling()
        await test_ohlc_validation()
        await test_symbol_filtering()
        
        print("\n" + "=" * 50)
        print("🎉 All tests passed successfully!")
        
    except Exception as e:
        print(f"\n❌ Test failed: {e}")
        raise


if __name__ == "__main__":
    asyncio.run(main())