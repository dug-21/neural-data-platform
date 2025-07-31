# Polygon S3 API Reference

## Overview

This document provides a comprehensive reference for interacting with Polygon's S3 storage for historical market data. The S3 interface provides direct access to compressed minute-level aggregate data files.

## S3 Endpoint Configuration

### Base Configuration
```python
POLYGON_S3_CONFIG = {
    'endpoint_url': 'https://files.polygon.io',
    'aws_access_key_id': os.environ['POLYGON_ACCESS_KEY'],
    'aws_secret_access_key': os.environ['POLYGON_SECRET_KEY'],
    'bucket_name': 'flatfiles',
    'region_name': 'us-east-1'
}
```

### Authentication
Polygon uses AWS S3-compatible authentication:
- **Access Key**: Your Polygon API key
- **Secret Key**: Your Polygon secret key
- **Authentication Type**: AWS Signature Version 4

## S3 Directory Structure

### Path Format
```
flatfiles/us_stocks_sip/minute_aggs_v1/{year}/{month}/{date}.csv.gz
```

### Examples
```
# July 23, 2024 data
flatfiles/us_stocks_sip/minute_aggs_v1/2024/07/2024-07-23.csv.gz

# January 1, 2020 data
flatfiles/us_stocks_sip/minute_aggs_v1/2020/01/2020-01-01.csv.gz
```

## File Format

### CSV Structure
Each gzipped CSV file contains the following columns:

| Column | Type | Description | Example |
|--------|------|-------------|---------|
| symbol | string | Stock ticker symbol | AAPL |
| timestamp | int64 | Unix timestamp (milliseconds) | 1721736000000 |
| open | float | Opening price | 195.42 |
| high | float | Highest price | 195.75 |
| low | float | Lowest price | 195.38 |
| close | float | Closing price | 195.65 |
| volume | int64 | Trading volume | 125000 |
| vwap | float | Volume-weighted average price | 195.58 |
| trades | int32 | Number of trades | 450 |

### Sample Data
```csv
symbol,timestamp,open,high,low,close,volume,vwap,trades
AAPL,1721736000000,195.42,195.75,195.38,195.65,125000,195.58,450
AAPL,1721736060000,195.65,195.82,195.60,195.78,98000,195.71,380
MSFT,1721736000000,425.30,425.50,425.15,425.45,85000,425.38,320
```

## Python S3 Client Implementation

### Basic Client
```python
import boto3
from botocore.config import Config

class PolygonS3Client:
    def __init__(self, access_key: str, secret_key: str):
        self.session = boto3.Session(
            aws_access_key_id=access_key,
            aws_secret_access_key=secret_key
        )
        
        # Configure S3 client
        self.s3 = self.session.client(
            's3',
            endpoint_url='https://files.polygon.io',
            config=Config(
                signature_version='s3v4',
                retries={'max_attempts': 3, 'mode': 'adaptive'}
            )
        )
        
        self.bucket = 'flatfiles'
        self.prefix = 'us_stocks_sip/minute_aggs_v1/'
```

### List Available Files
```python
async def list_files(self, start_date: date, end_date: date) -> List[str]:
    """List all available files in date range"""
    
    files = []
    current = start_date
    
    while current <= end_date:
        # Skip weekends
        if current.weekday() < 5:  # Monday = 0, Friday = 4
            key = f"{self.prefix}{current.year}/{current.month:02d}/{current.strftime('%Y-%m-%d')}.csv.gz"
            
            try:
                # Check if file exists
                self.s3.head_object(Bucket=self.bucket, Key=key)
                files.append(key)
            except self.s3.exceptions.NoSuchKey:
                # File doesn't exist (holiday/weekend)
                pass
                
        current += timedelta(days=1)
    
    return files
```

### Download File
```python
async def download_file(self, s3_key: str, local_path: Path) -> bool:
    """Download a single file from S3"""
    
    try:
        # Ensure directory exists
        local_path.parent.mkdir(parents=True, exist_ok=True)
        
        # Download with progress callback
        self.s3.download_file(
            Bucket=self.bucket,
            Key=s3_key,
            Filename=str(local_path),
            Callback=self._progress_callback
        )
        
        return True
        
    except Exception as e:
        logger.error(f"Download failed for {s3_key}: {e}")
        return False
```

### Streaming Download
```python
async def stream_file(self, s3_key: str) -> AsyncIterator[bytes]:
    """Stream file content without saving to disk"""
    
    response = self.s3.get_object(Bucket=self.bucket, Key=s3_key)
    
    # Stream in chunks
    async for chunk in response['Body'].iter_chunks(chunk_size=1024*1024):
        yield chunk
```

## Error Handling

### Common S3 Errors
```python
from botocore.exceptions import ClientError

try:
    response = s3.get_object(Bucket=bucket, Key=key)
except ClientError as e:
    error_code = e.response['Error']['Code']
    
    if error_code == 'NoSuchKey':
        # File doesn't exist
        logger.warning(f"File not found: {key}")
    elif error_code == 'AccessDenied':
        # Authentication issue
        logger.error("Access denied - check credentials")
    elif error_code == 'RequestTimeout':
        # Network timeout
        logger.warning("Request timeout - retrying")
    else:
        # Other error
        logger.error(f"S3 error: {error_code}")
```

### Retry Strategy
```python
from tenacity import retry, stop_after_attempt, wait_exponential

@retry(
    stop=stop_after_attempt(3),
    wait=wait_exponential(multiplier=1, min=4, max=10)
)
async def download_with_retry(self, s3_key: str, local_path: Path):
    """Download with automatic retry on failure"""
    
    return await self.download_file(s3_key, local_path)
```

## Performance Optimization

### Concurrent Downloads
```python
import asyncio
from concurrent.futures import ThreadPoolExecutor

class ConcurrentS3Downloader:
    def __init__(self, s3_client: PolygonS3Client, max_concurrent: int = 5):
        self.client = s3_client
        self.semaphore = asyncio.Semaphore(max_concurrent)
        self.executor = ThreadPoolExecutor(max_workers=max_concurrent)
    
    async def download_multiple(self, s3_keys: List[str], 
                              download_dir: Path) -> Dict[str, bool]:
        """Download multiple files concurrently"""
        
        tasks = []
        for key in s3_keys:
            task = self._download_with_limit(key, download_dir)
            tasks.append(task)
        
        results = await asyncio.gather(*tasks, return_exceptions=True)
        
        return {
            key: not isinstance(result, Exception)
            for key, result in zip(s3_keys, results)
        }
    
    async def _download_with_limit(self, s3_key: str, 
                                  download_dir: Path) -> None:
        """Download with concurrency limit"""
        
        async with self.semaphore:
            filename = Path(s3_key).name
            local_path = download_dir / filename
            
            # Run in thread pool to avoid blocking
            await asyncio.get_event_loop().run_in_executor(
                self.executor,
                self.client.s3.download_file,
                self.client.bucket,
                s3_key,
                str(local_path)
            )
```

### Bandwidth Management
```python
class BandwidthLimitedDownloader:
    def __init__(self, s3_client: PolygonS3Client, 
                 max_bandwidth_mbps: float = 50):
        self.client = s3_client
        self.max_bandwidth = max_bandwidth_mbps * 1024 * 1024 / 8
        self.current_bandwidth = 0
        self.lock = asyncio.Lock()
    
    async def download_with_limit(self, s3_key: str, 
                                 local_path: Path) -> None:
        """Download with bandwidth limiting"""
        
        # Get file size
        response = self.client.s3.head_object(
            Bucket=self.client.bucket, 
            Key=s3_key
        )
        file_size = response['ContentLength']
        
        # Calculate download time for bandwidth limit
        target_time = file_size / self.max_bandwidth
        
        start_time = time.time()
        await self.client.download_file(s3_key, local_path)
        actual_time = time.time() - start_time
        
        # Sleep if download was too fast
        if actual_time < target_time:
            await asyncio.sleep(target_time - actual_time)
```

## Data Processing

### Streaming Decompression
```python
import gzip
import pandas as pd
from io import BytesIO

async def process_compressed_file(self, s3_key: str) -> pd.DataFrame:
    """Process gzipped CSV without full download"""
    
    # Stream file from S3
    response = self.client.s3.get_object(
        Bucket=self.client.bucket,
        Key=s3_key
    )
    
    # Decompress on the fly
    with gzip.GzipFile(fileobj=response['Body']) as gz:
        # Read CSV in chunks
        chunks = []
        for chunk in pd.read_csv(gz, chunksize=10000):
            # Process chunk
            chunk['timestamp'] = pd.to_datetime(
                chunk['timestamp'], 
                unit='ms'
            )
            chunks.append(chunk)
    
    return pd.concat(chunks, ignore_index=True)
```

### Symbol Filtering
```python
def filter_symbols(df: pd.DataFrame, symbols: List[str]) -> pd.DataFrame:
    """Filter dataframe to specific symbols"""
    
    if symbols:
        return df[df['symbol'].isin(symbols)]
    return df
```

## Usage Examples

### Complete Download Workflow
```python
async def download_date_range(start_date: date, end_date: date, 
                            symbols: List[str]) -> None:
    """Download and process data for date range"""
    
    # Initialize client
    client = PolygonS3Client(
        access_key=os.environ['POLYGON_ACCESS_KEY'],
        secret_key=os.environ['POLYGON_SECRET_KEY']
    )
    
    # List available files
    files = await client.list_files(start_date, end_date)
    logger.info(f"Found {len(files)} files to download")
    
    # Download concurrently
    downloader = ConcurrentS3Downloader(client, max_concurrent=5)
    results = await downloader.download_multiple(files, Path('./data'))
    
    # Process downloaded files
    for file_path in Path('./data').glob('*.csv.gz'):
        df = await process_compressed_file(file_path)
        df_filtered = filter_symbols(df, symbols)
        
        # Store in database
        await store_to_database(df_filtered)
```

### Single Day Download
```python
async def download_single_day(target_date: date) -> pd.DataFrame:
    """Download data for a single day"""
    
    client = PolygonS3Client(
        access_key=os.environ['POLYGON_ACCESS_KEY'],
        secret_key=os.environ['POLYGON_SECRET_KEY']
    )
    
    # Build S3 key
    s3_key = f"us_stocks_sip/minute_aggs_v1/{target_date.year}/{target_date.month:02d}/{target_date.strftime('%Y-%m-%d')}.csv.gz"
    
    # Download and process
    return await client.process_compressed_file(s3_key)
```

## Best Practices

### 1. Connection Pooling
```python
# Reuse S3 client across requests
S3_CLIENT_POOL = {}

def get_s3_client(access_key: str) -> PolygonS3Client:
    if access_key not in S3_CLIENT_POOL:
        S3_CLIENT_POOL[access_key] = PolygonS3Client(
            access_key=access_key,
            secret_key=os.environ['POLYGON_SECRET_KEY']
        )
    return S3_CLIENT_POOL[access_key]
```

### 2. Error Recovery
```python
async def download_with_checkpoint(s3_keys: List[str], 
                                 checkpoint_file: Path):
    """Download with checkpoint support"""
    
    # Load checkpoint
    completed = set()
    if checkpoint_file.exists():
        completed = set(json.loads(checkpoint_file.read_text()))
    
    # Download remaining files
    for key in s3_keys:
        if key in completed:
            continue
            
        try:
            await download_file(key)
            completed.add(key)
            
            # Save checkpoint
            checkpoint_file.write_text(json.dumps(list(completed)))
            
        except Exception as e:
            logger.error(f"Failed to download {key}: {e}")
            # Continue with next file
```

### 3. Resource Management
```python
class S3ResourceManager:
    def __init__(self, max_memory_mb: int = 2048):
        self.max_memory = max_memory_mb * 1024 * 1024
        self.current_memory = 0
        
    async def __aenter__(self):
        return self
        
    async def __aexit__(self, exc_type, exc_val, exc_tb):
        # Cleanup resources
        gc.collect()
```

## Troubleshooting

### Authentication Issues
```bash
# Test credentials
aws s3 ls s3://flatfiles/us_stocks_sip/minute_aggs_v1/ \
    --endpoint-url https://files.polygon.io
```

### Network Issues
```python
# Test connectivity
import requests

response = requests.head('https://files.polygon.io')
print(f"Status: {response.status_code}")
```

### Performance Issues
```python
# Profile download speed
import time

start = time.time()
await client.download_file(s3_key, local_path)
elapsed = time.time() - start

file_size = local_path.stat().st_size
speed = file_size / elapsed / 1024 / 1024
print(f"Download speed: {speed:.2f} MB/s")
```

---

*Document Version: 1.0.0 | Last Updated: July 2024*