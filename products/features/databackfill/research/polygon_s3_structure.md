# Polygon.io S3 Flat Files Research Report

## Overview

Polygon.io provides extensive historical market data through their Flat Files service, delivered via S3-compatible endpoints. This report documents the complete structure, access methods, and implementation details for bulk downloading minute aggregate data.

## Access Configuration

### Authentication Requirements
- **Subscription**: Valid Polygon.io paid plan (flat files included in all paid plans)
- **Credentials**: Access Key and Secret Key (available from Polygon.io Dashboard)
- **Endpoint**: `https://files.polygon.io`
- **Bucket**: `flatfiles`

### AWS CLI Configuration
```bash
# Configure AWS CLI with Polygon credentials
aws configure --profile polygon
# AWS Access Key ID: [Your Polygon Access Key]
# AWS Secret Access Key: [Your Polygon Secret Key]
# Default region name: us-east-1
# Default output format: json

# List files
aws s3 ls s3://flatfiles/ --endpoint-url https://files.polygon.io --profile polygon

# Download specific file
aws s3 cp s3://flatfiles/us_stocks_sip/minute_aggs_v1/2024/03/2024-03-07.csv.gz . \
  --endpoint-url https://files.polygon.io --profile polygon
```

## Directory Structure

### Top-Level Organization
```
flatfiles/
├── us_stocks_sip/          # US Stocks (SIP feed)
├── us_options_opra/        # US Options (OPRA feed)
├── us_indices/             # US Indices
├── global_forex/           # Global Forex
└── global_crypto/          # Global Cryptocurrency
```

### US Stocks Minute Aggregates Path Structure
```
flatfiles/us_stocks_sip/minute_aggs_v1/
└── YYYY/                   # Year (e.g., 2024)
    └── MM/                 # Month (e.g., 03)
        └── YYYY-MM-DD.csv.gz  # Daily file (e.g., 2024-03-07.csv.gz)
```

### Example Paths
- Minute aggregates: `flatfiles/us_stocks_sip/minute_aggs_v1/2024/03/2024-03-07.csv.gz`
- Daily aggregates: `flatfiles/us_stocks_sip/day_aggs_v1/2024/03/2024-03-04.csv.gz`
- Trades: `flatfiles/us_stocks_sip/trades_v1/2024/04/2024-04-05.csv.gz`
- Quotes: `flatfiles/us_stocks_sip/quotes_v1/2024/04/2024-04-05.csv.gz`

## Data Format

### Minute Aggregates CSV Structure
```csv
ticker,volume,open,close,high,low,window_start,transactions
AAPL,4930,200.29,200.5,200.63,200.29,1744792500000000000,129
AAPL,1815,200.39,200.34,200.61,200.34,1744792560000000000,57
```

#### Column Definitions
- **ticker**: Stock symbol (string)
- **volume**: Trading volume for the minute (integer)
- **open**: Opening price (float)
- **close**: Closing price (float)
- **high**: Highest price during the minute (float)
- **low**: Lowest price during the minute (float)
- **window_start**: Unix timestamp in nanoseconds (integer)
- **transactions**: Number of transactions in the minute (integer)

### Important Notes
1. **Timestamps**: All timestamps are Unix nanoseconds (not milliseconds)
2. **Compression**: Files are gzip compressed (.csv.gz)
3. **Availability**: Data available by ~11:00 AM ET the following trading day
4. **File Size**: Daily minute aggregate files typically range from 50-200 MB compressed
5. **Coverage**: One file contains all symbols' minute data for that trading day

## Python Implementation Example

```python
import boto3
import pandas as pd
import gzip
from datetime import datetime, timedelta
from io import StringIO
import asyncio
import aiofiles
from pathlib import Path
import logging

class PolygonS3Client:
    def __init__(self, access_key, secret_key):
        self.s3 = boto3.client(
            's3',
            endpoint_url='https://files.polygon.io',
            aws_access_key_id=access_key,
            aws_secret_access_key=secret_key,
            region_name='us-east-1'  # Required even though not used
        )
        self.bucket = 'flatfiles'
        self.logger = logging.getLogger(__name__)
    
    def download_minute_aggregates(self, date):
        """Download minute aggregates for a specific date"""
        # Format path
        year = date.strftime('%Y')
        month = date.strftime('%m')
        day = date.strftime('%Y-%m-%d')
        
        key = f'us_stocks_sip/minute_aggs_v1/{year}/{month}/{day}.csv.gz'
        
        # Download file
        try:
            response = self.s3.get_object(Bucket=self.bucket, Key=key)
            compressed_data = response['Body'].read()
            
            # Decompress and parse
            decompressed = gzip.decompress(compressed_data).decode('utf-8')
            df = pd.read_csv(StringIO(decompressed))
            
            # Convert timestamp from nanoseconds to datetime
            df['timestamp'] = pd.to_datetime(df['window_start'], unit='ns')
            
            return df
            
        except Exception as e:
            self.logger.error(f"Error downloading {key}: {e}")
            return None
    
    def download_to_file(self, date, output_dir):
        """Download minute aggregates directly to file"""
        year = date.strftime('%Y')
        month = date.strftime('%m')
        day = date.strftime('%Y-%m-%d')
        
        key = f'us_stocks_sip/minute_aggs_v1/{year}/{month}/{day}.csv.gz'
        output_path = Path(output_dir) / f"{day}.csv.gz"
        
        try:
            self.s3.download_file(self.bucket, key, str(output_path))
            self.logger.info(f"Downloaded {key} to {output_path}")
            return output_path
        except Exception as e:
            self.logger.error(f"Error downloading {key}: {e}")
            return None
    
    def list_available_dates(self, year, month):
        """List all available dates for a given year/month"""
        prefix = f'us_stocks_sip/minute_aggs_v1/{year:04d}/{month:02d}/'
        
        response = self.s3.list_objects_v2(
            Bucket=self.bucket,
            Prefix=prefix
        )
        
        files = []
        if 'Contents' in response:
            for obj in response['Contents']:
                # Extract date from filename
                filename = obj['Key'].split('/')[-1]
                date_str = filename.replace('.csv.gz', '')
                files.append({
                    'date': date_str,
                    'size_mb': obj['Size'] / (1024 * 1024),
                    'last_modified': obj['LastModified']
                })
        
        return sorted(files, key=lambda x: x['date'])
    
    def filter_by_symbols(self, df, symbols):
        """Filter dataframe to only include specific symbols"""
        return df[df['ticker'].isin(symbols)]
    
    async def parallel_download(self, dates, output_dir, max_concurrent=5):
        """Download multiple dates in parallel"""
        semaphore = asyncio.Semaphore(max_concurrent)
        
        async def download_with_limit(date):
            async with semaphore:
                return await asyncio.get_event_loop().run_in_executor(
                    None, self.download_to_file, date, output_dir
                )
        
        tasks = [download_with_limit(date) for date in dates]
        return await asyncio.gather(*tasks)

# Example usage
async def backfill_minute_data():
    # Initialize client
    client = PolygonS3Client(
        access_key='YOUR_POLYGON_ACCESS_KEY',
        secret_key='YOUR_POLYGON_SECRET_KEY'
    )
    
    # Download last 30 days
    end_date = datetime.now()
    start_date = end_date - timedelta(days=30)
    
    dates = pd.date_range(start_date, end_date, freq='B')  # Business days only
    
    # Parallel download
    output_dir = Path('data/polygon/minute_aggs')
    output_dir.mkdir(parents=True, exist_ok=True)
    
    results = await client.parallel_download(dates, output_dir)
    
    # Process downloaded files
    for file_path in results:
        if file_path:
            # Load and filter data
            df = pd.read_csv(file_path, compression='gzip')
            
            # Filter for specific symbols if needed
            symbols = ['AAPL', 'GOOGL', 'MSFT', 'TSLA', 'NVDA']
            filtered_df = client.filter_by_symbols(df, symbols)
            
            # Store in database or process further
            print(f"Processed {len(filtered_df)} records from {file_path}")
```

## Alternative Implementation Using Boto3 Session

```python
import boto3
from botocore.config import Config

class PolygonS3Session:
    """Alternative implementation using boto3 Session"""
    
    def __init__(self, access_key, secret_key):
        # Create custom session
        self.session = boto3.Session(
            aws_access_key_id=access_key,
            aws_secret_access_key=secret_key
        )
        
        # Configure S3 client with custom endpoint
        self.s3 = self.session.client(
            's3',
            endpoint_url='https://files.polygon.io',
            config=Config(
                signature_version='s3v4',
                s3={'addressing_style': 'path'}
            )
        )
        
    def get_paginated_objects(self, prefix, max_keys=1000):
        """Get all objects with pagination support"""
        paginator = self.s3.get_paginator('list_objects_v2')
        
        pages = paginator.paginate(
            Bucket='flatfiles',
            Prefix=prefix,
            PaginationConfig={'MaxKeys': max_keys}
        )
        
        all_objects = []
        for page in pages:
            if 'Contents' in page:
                all_objects.extend(page['Contents'])
        
        return all_objects
```

## Environment Variables Configuration

```bash
# .env file
POLYGON_S3_ACCESS_KEY=your_access_key_here
POLYGON_S3_SECRET_KEY=your_secret_key_here
POLYGON_S3_ENDPOINT=https://files.polygon.io
POLYGON_S3_BUCKET=flatfiles
```

## Integration with Existing Polygon Provider

```python
# Extension for data_ingestion/providers/polygon.py

class PolygonProvider(BaseProvider):
    # ... existing code ...
    
    async def backfill_from_s3(self, symbols, start_date, end_date):
        """Backfill historical data from S3 flat files"""
        if not hasattr(self, 's3_client'):
            self.s3_client = PolygonS3Client(
                access_key=self.settings.polygon_s3_access_key,
                secret_key=self.settings.polygon_s3_secret_key
            )
        
        # Generate date range
        dates = pd.date_range(start_date, end_date, freq='B')
        
        for date in dates:
            # Download data
            df = self.s3_client.download_minute_aggregates(date)
            
            if df is not None:
                # Filter for requested symbols
                filtered_df = df[df['ticker'].isin(symbols)]
                
                # Convert to MarketData objects
                for _, row in filtered_df.iterrows():
                    yield MarketData(
                        time=row['timestamp'],
                        symbol=row['ticker'],
                        open=row['open'],
                        high=row['high'],
                        low=row['low'],
                        close=row['close'],
                        volume=row['volume'],
                        provider=self.name,
                        metadata={
                            'source': 's3_flat_file',
                            'transactions': row['transactions']
                        }
                    )
```

## Rate Limits and Best Practices

### Download Limits
- No explicit rate limits on S3 downloads
- Concurrent downloads recommended: 5-10 connections
- Bandwidth considerations: Each file can be 50-200 MB

### Best Practices
1. **Parallel Downloads**: Use multiple threads/processes for different dates
2. **Incremental Updates**: Only download new dates, not entire history
3. **Local Caching**: Store downloaded files locally to avoid re-downloading
4. **Error Handling**: Implement retry logic for failed downloads
5. **Monitoring**: Track download progress and validate data integrity

### Storage Estimates
- **Per Trading Day**: ~100-200 MB compressed, ~500MB-1GB uncompressed
- **Per Year**: ~25-50 GB compressed, ~125-250 GB uncompressed
- **5-Year History**: ~125-250 GB compressed, ~625GB-1.25TB uncompressed

## Integration with Neural Trader

### Recommended Approach
1. **Bulk Historical Load**: Download all historical data via S3 for initial backfill
2. **Daily Updates**: Schedule daily downloads at 11:30 AM ET for previous day
3. **Storage**: Load into TimescaleDB with proper partitioning
4. **Validation**: Cross-reference with real-time data for accuracy

### Implementation Steps
1. Create S3 download module in `data_ingestion/providers/polygon_s3.py`
2. Add configuration for AWS credentials
3. Implement parallel download manager
4. Create data validation pipeline
5. Integrate with existing TimescaleDB storage

## Advantages Over API

1. **Speed**: Bulk downloads vs individual API calls (100x faster)
2. **Cost**: No API call limits or quotas
3. **Reliability**: Single download vs thousands of API calls
4. **Completeness**: Guaranteed complete daily datasets
5. **Efficiency**: Pre-aggregated data reduces processing overhead

## Conclusion

Polygon's S3 flat files provide the most efficient method for historical data backfill. The minute aggregate files contain all necessary OHLCV data in a simple CSV format, making them ideal for:
- Initial historical data population
- Daily batch updates
- Machine learning training datasets
- Backtesting systems
- Data validation and reconciliation

The S3 approach should be the primary method for historical data ingestion, with the WebSocket API reserved for real-time updates only.