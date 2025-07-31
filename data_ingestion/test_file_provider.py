#!/usr/bin/env python3
"""Test script for file provider implementation."""
import asyncio
import csv
import os
from datetime import datetime, timedelta
import random
from pathlib import Path

# Create test data directory
test_dir = Path("test-data/backfill")
test_dir.mkdir(parents=True, exist_ok=True)

# Generate test CSV file
csv_file = test_dir / "AAPL_test.csv"

def generate_test_data():
    """Generate test market data CSV."""
    print(f"Generating test data at {csv_file}")
    
    # Generate 1000 rows of test data
    rows = []
    base_price = 150.0
    current_time = datetime.now() - timedelta(days=10)
    
    with open(csv_file, 'w', newline='') as f:
        writer = csv.DictWriter(f, fieldnames=[
            'timestamp', 'symbol', 'open', 'high', 'low', 'close', 'volume'
        ])
        writer.writeheader()
        
        for i in range(1000):
            # Generate realistic OHLC data
            open_price = base_price + random.uniform(-2, 2)
            close_price = open_price + random.uniform(-1, 1)
            high_price = max(open_price, close_price) + random.uniform(0, 0.5)
            low_price = min(open_price, close_price) - random.uniform(0, 0.5)
            
            row = {
                'timestamp': current_time.strftime('%Y-%m-%d %H:%M:%S'),
                'symbol': 'AAPL',
                'open': round(open_price, 2),
                'high': round(high_price, 2),
                'low': round(low_price, 2),
                'close': round(close_price, 2),
                'volume': random.randint(1000000, 5000000)
            }
            writer.writerow(row)
            
            # Update for next row
            base_price = close_price
            current_time += timedelta(minutes=1)
            
    print(f"Generated {csv_file} with 1000 rows")


async def test_file_provider():
    """Test the file provider implementation."""
    from providers.file_provider import FileProvider
    
    # Create provider
    provider = FileProvider({
        'batch_size': 100  # Small batch for testing
    })
    
    # Connect (no-op for file provider)
    await provider.connect()
    
    print("\n=== Testing File Provider ===")
    print(f"Loading data from: {csv_file}")
    
    # Test loading with progress tracking
    count = 0
    start_time = datetime.now()
    
    try:
        async for market_data in provider.load_from_file(str(csv_file), format='csv'):
            count += 1
            
            # Print progress every 100 rows
            if count % 100 == 0:
                print(f"Processed {count} rows... Latest: {market_data.symbol} @ ${market_data.close}")
                
            # Simulate interruption at row 500 to test checkpoint
            if count == 500:
                print("\n!!! Simulating interruption at row 500 !!!")
                break
                
    except Exception as e:
        print(f"Error during load: {e}")
        
    elapsed = (datetime.now() - start_time).total_seconds()
    print(f"\nFirst load: Processed {count} rows in {elapsed:.2f} seconds")
    
    # Test checkpoint recovery
    print("\n=== Testing Checkpoint Recovery ===")
    count2 = 0
    start_time2 = datetime.now()
    
    async for market_data in provider.load_from_file(str(csv_file), format='csv'):
        count2 += 1
        
        if count2 % 100 == 0:
            print(f"Processed {count2} rows (resumed)... Latest: {market_data.symbol} @ ${market_data.close}")
            
    elapsed2 = (datetime.now() - start_time2).total_seconds()
    print(f"\nResumed load: Processed {count2} rows in {elapsed2:.2f} seconds")
    print(f"Total rows processed: {count + count2}")
    
    # Disconnect
    await provider.disconnect()
    
    print("\n✅ File provider test completed!")


if __name__ == "__main__":
    # Generate test data
    generate_test_data()
    
    # Run async test
    asyncio.run(test_file_provider())