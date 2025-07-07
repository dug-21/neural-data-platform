"""Example usage of NASDAQ/Quandl provider."""
import asyncio
from datetime import datetime, timedelta
import os
import sys

# Add parent directory to path for imports
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from providers.nasdaq import NASDAQProvider


async def main():
    """Demonstrate NASDAQ/Quandl provider functionality."""
    
    # Initialize provider
    async with NASDAQProvider() as provider:
        print("Connected to NASDAQ/Quandl API\n")
        
        # 1. Get popular datasets
        print("=== Popular Datasets ===")
        popular = await provider.get_popular_datasets()
        for code, description in list(popular.items())[:5]:
            print(f"{code}: {description}")
        print()
        
        # 2. Search for datasets
        print("=== Searching for Tesla datasets ===")
        try:
            results = await provider.search_datasets("Tesla", per_page=5)
            for result in results:
                print(f"- {result['database_code']}/{result['dataset_code']}: {result['name']}")
        except Exception as e:
            print(f"Search error: {e}")
        print()
        
        # 3. Get stock data
        print("=== Fetching Apple stock data (if available) ===")
        try:
            end_date = datetime.now()
            start_date = end_date - timedelta(days=7)
            
            data_count = 0
            async for data in provider.get_market_data(
                ["WIKI/AAPL"],
                start_date,
                end_date,
                interval="1day"
            ):
                print(f"{data.time.strftime('%Y-%m-%d')}: "
                      f"O={data.open:.2f}, H={data.high:.2f}, "
                      f"L={data.low:.2f}, C={data.close:.2f}, "
                      f"V={data.volume:,}")
                data_count += 1
                
                if data_count >= 5:  # Limit output
                    break
            
            if data_count == 0:
                print("No data available for WIKI/AAPL")
        except Exception as e:
            print(f"Error fetching stock data: {e}")
        print()
        
        # 4. Get economic data from FRED database
        print("=== Fetching GDP data from FRED ===")
        try:
            end_date = datetime.now()
            start_date = end_date - timedelta(days=365)
            
            data_count = 0
            async for data in provider.get_dataset(
                "FRED", "GDP",
                start_date,
                end_date,
                collapse="quarterly"
            ):
                print(f"{data.time.strftime('%Y-%m-%d')}: GDP = ${data.close:,.2f}B")
                data_count += 1
                
                if data_count >= 4:  # Show last 4 quarters
                    break
            
            if data_count == 0:
                print("No GDP data available")
        except Exception as e:
            print(f"Error fetching GDP data: {e}")
        print()
        
        # 5. Get commodity futures data
        print("=== Fetching Gold futures data ===")
        try:
            end_date = datetime.now()
            start_date = end_date - timedelta(days=30)
            
            data_count = 0
            async for data in provider.get_dataset(
                "CHRIS", "CME_GC1",
                start_date,
                end_date,
                limit=5
            ):
                print(f"{data.time.strftime('%Y-%m-%d')}: "
                      f"Gold = ${data.close:.2f}/oz")
                data_count += 1
            
            if data_count == 0:
                print("No Gold futures data available")
        except Exception as e:
            print(f"Error fetching Gold data: {e}")
        print()
        
        # 6. Get dataset metadata
        print("=== Dataset Metadata ===")
        try:
            metadata = await provider.get_metadata("WIKI", "AAPL")
            print(f"Dataset: {metadata['name']}")
            print(f"Description: {metadata.get('description', 'N/A')[:100]}...")
            print(f"Date Range: {metadata.get('oldest_available_date')} to {metadata.get('newest_available_date')}")
            print(f"Columns: {', '.join(metadata.get('column_names', []))}")
            print(f"Frequency: {metadata.get('frequency')}")
        except Exception as e:
            print(f"Error fetching metadata: {e}")
        print()
        
        # 7. List available databases
        print("=== Available Databases (first 5) ===")
        try:
            databases = await provider.get_databases()
            for db in databases[:5]:
                print(f"- {db['database_code']}: {db['name']} "
                      f"({db['datasets_count']:,} datasets)")
        except Exception as e:
            print(f"Error listing databases: {e}")


if __name__ == "__main__":
    # Check for API key
    if not os.getenv("QUANDL_API_KEY"):
        print("Error: Please set QUANDL_API_KEY environment variable")
        print("You can get a free API key at: https://www.quandl.com/sign-up")
        sys.exit(1)
    
    # Run the example
    asyncio.run(main())