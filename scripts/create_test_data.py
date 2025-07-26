#!/usr/bin/env python3
"""
Create test data files for the enhanced FileProvider.
Generates sample market data in CSV, JSON, and Parquet formats.
"""
import os
import sys
import json
import gzip
import pandas as pd
import numpy as np
from datetime import datetime, timedelta, timezone
from pathlib import Path

# Add data_ingestion to path
sys.path.insert(0, os.path.join(os.path.dirname(os.path.dirname(__file__)), 'data_ingestion'))

def generate_sample_data(symbol: str, start_time: datetime, periods: int = 1000) -> pd.DataFrame:
    """Generate realistic market data for testing."""
    np.random.seed(42)  # For reproducibility
    
    # Generate timestamps
    timestamps = pd.date_range(start=start_time, periods=periods, freq='1min', tz=timezone.utc)
    
    # Generate realistic price movements
    base_price = 150.0 if symbol == 'AAPL' else 2800.0 if symbol == 'GOOGL' else 100.0
    price_noise = np.random.randn(periods) * 0.5
    price_trend = np.cumsum(np.random.randn(periods) * 0.1)
    prices = base_price + price_noise + price_trend
    
    # Generate OHLC data
    data = []
    for i, timestamp in enumerate(timestamps):
        close = prices[i]
        open_price = close + np.random.uniform(-0.5, 0.5)
        high = max(open_price, close) + abs(np.random.uniform(0, 0.5))
        low = min(open_price, close) - abs(np.random.uniform(0, 0.5))
        volume = int(np.random.uniform(100000, 5000000))
        
        data.append({
            'timestamp': timestamp.isoformat(),
            'symbol': symbol,
            'open': round(open_price, 2),
            'high': round(high, 2),
            'low': round(low, 2),
            'close': round(close, 2),
            'volume': volume
        })
    
    return pd.DataFrame(data)

def create_csv_files(base_path: Path):
    """Create CSV test files."""
    print("Creating CSV files...")
    
    # Create date-based structure
    date_path = base_path / "2024" / "01" / "15"
    date_path.mkdir(parents=True, exist_ok=True)
    
    # Generate data for multiple symbols
    all_data = []
    for symbol in ['AAPL', 'GOOGL', 'MSFT']:
        df = generate_sample_data(symbol, datetime(2024, 1, 15, 9, 30, tzinfo=timezone.utc), 100)
        all_data.append(df)
    
    combined_df = pd.concat(all_data).sort_values('timestamp')
    
    # Save as regular CSV
    csv_file = date_path / "market_data_20240115.csv"
    combined_df.to_csv(csv_file, index=False)
    print(f"  Created: {csv_file}")
    
    # Save as gzipped CSV
    csv_gz_file = date_path / "market_data_20240115.csv.gz"
    with gzip.open(csv_gz_file, 'wt', encoding='utf-8') as f:
        combined_df.to_csv(f, index=False)
    print(f"  Created: {csv_gz_file}")

def create_json_files(base_path: Path):
    """Create JSON test files."""
    print("\nCreating JSON files...")
    
    # Create symbol-based structure
    symbol_path = base_path / "symbols" / "AAPL" / "2024"
    symbol_path.mkdir(parents=True, exist_ok=True)
    
    # Generate AAPL data
    df = generate_sample_data('AAPL', datetime(2024, 1, 15, 10, 0, tzinfo=timezone.utc), 200)
    
    # Convert to JSON with different field names (to test parser flexibility)
    records = []
    for _, row in df.iterrows():
        records.append({
            't': row['timestamp'],  # Short field names
            'sym': row['symbol'],
            'o': row['open'],
            'h': row['high'],
            'l': row['low'],
            'c': row['close'],
            'v': row['volume']
        })
    
    # Save as regular JSON
    json_file = symbol_path / "market_data_AAPL_202401.json"
    with open(json_file, 'w') as f:
        json.dump(records, f, indent=2)
    print(f"  Created: {json_file}")
    
    # Save as gzipped JSON
    json_gz_file = symbol_path / "market_data_AAPL_202401.json.gz"
    with gzip.open(json_gz_file, 'wt', encoding='utf-8') as f:
        json.dump(records, f, indent=2)
    print(f"  Created: {json_gz_file}")

def create_parquet_files(base_path: Path):
    """Create Parquet test files."""
    try:
        import pyarrow.parquet as pq
    except ImportError:
        print("\nSkipping Parquet files - pyarrow not installed")
        print("Install with: pip install pyarrow")
        return
    
    print("\nCreating Parquet files...")
    
    # Create another date-based path
    date_path = base_path / "2024" / "01" / "16"
    date_path.mkdir(parents=True, exist_ok=True)
    
    # Generate mixed symbol data
    all_data = []
    for symbol in ['AAPL', 'GOOGL', 'MSFT', 'TSLA']:
        df = generate_sample_data(symbol, datetime(2024, 1, 16, 9, 30, tzinfo=timezone.utc), 150)
        all_data.append(df)
    
    combined_df = pd.concat(all_data).sort_values('timestamp')
    
    # Save as Parquet
    parquet_file = date_path / "market_data_20240116.parquet"
    combined_df.to_parquet(parquet_file, index=False)
    print(f"  Created: {parquet_file}")

def create_sample_backfill_script(base_path: Path):
    """Create a sample script for running backfill."""
    script_content = f'''#!/usr/bin/env python3
"""
Sample backfill script using the enhanced FileProvider.
This demonstrates how to use the FileProvider with different formats.
"""
import asyncio
import sys
import os
from datetime import datetime, timezone
from pathlib import Path

# Add data_ingestion to path
sys.path.insert(0, os.path.join(os.path.dirname(os.path.dirname(__file__)), 'data_ingestion'))

from providers.file_provider import FileProvider

async def run_backfill():
    """Run backfill from test data files."""
    base_path = "{base_path}"
    checkpoint_dir = Path.home() / ".neural_trader" / "test_checkpoints"
    
    # Initialize FileProvider
    provider = FileProvider(base_path, str(checkpoint_dir))
    
    async with provider:
        # Define time range
        start_time = datetime(2024, 1, 15, 9, 0, tzinfo=timezone.utc)
        end_time = datetime(2024, 1, 16, 17, 0, tzinfo=timezone.utc)
        
        # Request data for specific symbols
        symbols = ["AAPL", "GOOGL"]
        
        print(f"Starting backfill for {{symbols}} from {{start_time}} to {{end_time}}")
        print("-" * 60)
        
        count = 0
        async for data in provider.get_market_data(symbols, start_time, end_time):
            count += 1
            if count <= 5:
                print(f"{{data.time}}: {{data.symbol}} ${{data.close}} (vol: {{data.volume:,}})")
            elif count == 6:
                print("... (continuing to process)")
        
        print("-" * 60)
        print(f"Processed {{count}} data points")
        
        # Show checkpoint status
        status = provider.get_checkpoint_status()
        print(f"\\nCheckpoint Status:")
        print(f"  Completed files: {{status['completed_files']}}")
        print(f"  Total records: {{status['total_records_processed']:,}}")
        print(f"  Bad records: {{status['total_bad_records']}} ({{status['bad_record_percentage']:.2f}}%)")

if __name__ == "__main__":
    asyncio.run(run_backfill())
'''
    
    script_path = base_path / "run_file_backfill.py"
    with open(script_path, 'w') as f:
        f.write(script_content)
    os.chmod(script_path, 0o755)
    print(f"\nCreated backfill script: {script_path}")

def main():
    """Create all test data files."""
    # Create test data directory
    base_path = Path("/tmp/neural_trader_test_data")
    base_path.mkdir(parents=True, exist_ok=True)
    
    print(f"Creating test data in: {base_path}")
    print("=" * 60)
    
    # Create files in different formats
    create_csv_files(base_path)
    create_json_files(base_path)
    create_parquet_files(base_path)
    
    # Create sample backfill script
    create_sample_backfill_script(base_path)
    
    print("\n" + "=" * 60)
    print("Test data creation complete!")
    print(f"\nTest the FileProvider with:")
    print(f"  python {base_path}/run_file_backfill.py")
    print(f"\nOr use the CLI:")
    print(f"  python -m data_ingestion.cli.backfill file --path {base_path} --format csv --symbols AAPL,GOOGL")

if __name__ == "__main__":
    main()