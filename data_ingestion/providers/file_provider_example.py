#!/usr/bin/env python3
"""
Example usage of the FileProvider for reading market data from files.

This example demonstrates how to use the FileProvider to read market data
from CSV files stored on mounted external drives or local file systems.
"""

import asyncio
from datetime import datetime, timezone
from pathlib import Path

# Add data_ingestion to Python path
import sys
import os
sys.path.insert(0, os.path.dirname(__file__))

from file_provider import FileProvider


async def basic_usage_example():
    """Basic example of using FileProvider."""
    print("=== Basic FileProvider Usage ===")
    
    # Initialize the FileProvider
    # base_path: Location where your data files are stored
    # checkpoint_dir: Optional directory for checkpoint files (for resume capability)
    base_path = "/mnt/external/market_data"  # Example external drive path
    checkpoint_dir = "/home/user/.neural_trader/checkpoints"  # Optional
    
    provider = FileProvider(base_path, checkpoint_dir)
    
    async with provider:
        # Define time range for data retrieval
        start_time = datetime(2024, 1, 15, 9, 0, tzinfo=timezone.utc)
        end_time = datetime(2024, 1, 15, 17, 0, tzinfo=timezone.utc)
        
        # Request market data for specific symbols
        symbols = ["AAPL", "GOOGL", "MSFT"]
        
        print(f"Fetching data for {symbols} from {start_time} to {end_time}")
        
        # Iterate through the data
        data_count = 0
        async for market_data in provider.get_market_data(symbols, start_time, end_time):
            print(f"{market_data.time}: {market_data.symbol} "
                  f"${market_data.close} (vol: {market_data.volume:,})")
            data_count += 1
            
            # Limit output for example
            if data_count >= 10:
                print(f"... (showing first 10 records)")
                break
        
        # Check processing status
        status = provider.get_checkpoint_status()
        print(f"\nProcessing status:")
        print(f"  Files processed: {status['completed_files']}")
        print(f"  Total records: {status['total_records_processed']:,}")
        print(f"  Bad records: {status['total_bad_records']} ({status['bad_record_percentage']:.1f}%)")


async def file_structure_example():
    """Example showing expected file structure."""
    print("=== Expected File Structure ===")
    print("""
    FileProvider supports two directory structures:
    
    1. Date-based structure:
       /base_path/YYYY/MM/DD/market_data_YYYYMMDD.csv[.gz]
       
    2. Symbol-based structure:
       /base_path/symbols/SYMBOL/YYYY/market_data_SYMBOL_YYYYMM.csv[.gz]
    
    Examples:
       /mnt/external/market_data/2024/01/15/market_data_20240115.csv.gz
       /mnt/external/market_data/symbols/AAPL/2024/market_data_AAPL_202401.csv.gz
    
    CSV Format (with header):
       timestamp,symbol,open,high,low,close,volume
       2024-01-15T09:30:00+00:00,AAPL,150.00,152.50,149.50,151.75,1000000
    """)


async def checkpoint_management_example():
    """Example of checkpoint management."""
    print("=== Checkpoint Management ===")
    
    base_path = "/mnt/external/market_data"
    provider = FileProvider(base_path)
    
    async with provider:
        # Check current checkpoint status
        status = provider.get_checkpoint_status()
        print(f"Current checkpoints: {status['total_checkpoints']}")
        print(f"Active files: {status['active_files']}")
        
        # Clear specific checkpoints (useful for reprocessing)
        if status['total_checkpoints'] > 0:
            print("Clearing checkpoints for January 2024 files...")
            await provider.clear_checkpoints(["*202401*"])
        
        # Clear all checkpoints
        # await provider.clear_checkpoints()  # Uncomment to clear all


async def error_handling_example():
    """Example of error handling and data quality."""
    print("=== Error Handling and Data Quality ===")
    
    base_path = "/mnt/external/market_data"
    provider = FileProvider(base_path)
    
    try:
        async with provider:
            start_time = datetime(2024, 1, 15, 9, 0, tzinfo=timezone.utc)
            end_time = datetime(2024, 1, 15, 17, 0, tzinfo=timezone.utc)
            
            # This will automatically handle:
            # - Bad CSV parsing (invalid data types)
            # - OHLC consistency validation
            # - Bad record percentage monitoring (fails if >1%)
            
            async for data in provider.get_market_data(["AAPL"], start_time, end_time):
                print(f"Valid data: {data.symbol} ${data.close}")
                
    except ValueError as e:
        if "Bad record percentage" in str(e):
            print(f"❌ Data quality issue: {e}")
            print("Too many invalid records in the data files.")
        else:
            print(f"❌ Data validation error: {e}")
    except Exception as e:
        print(f"❌ Unexpected error: {e}")


def main():
    """Run all examples."""
    print("🔧 FileProvider Examples")
    print("=" * 50)
    
    # Note: These examples assume you have actual data files
    # For testing, you can use the test_file_provider.py script
    
    print("Note: These examples require actual data files.")
    print("See test_file_provider.py for a complete working example with sample data.")
    print()
    
    asyncio.run(file_structure_example())


if __name__ == "__main__":
    main()