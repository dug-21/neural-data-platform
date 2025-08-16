# S3 Integration Guide

## Overview

This document details the integration with Polygon's S3 storage for historical market data retrieval.

## S3 Architecture

### Bucket Structure
```
flatfiles/
├── us_stocks_sip/
│   ├── day_aggs_v1/          # Daily aggregated OHLCV data
│   │   └── YYYY/MM/
│   │       └── YYYY-MM-DD.csv.gz
│   ├── trades_v1/            # Individual trades
│   │   └── YYYY/MM/DD/
│   │       └── SYMBOL.csv.gz
│   └── quotes_v1/            # Quote data
│       └── YYYY/MM/DD/
│           └── SYMBOL.csv.gz
```

## Authentication

### AWS Profile Configuration
```bash
# Configure AWS profile
aws configure --profile polygon-s3

# Required settings:
AWS_ACCESS_KEY_ID=<your-key>
AWS_SECRET_ACCESS_KEY=<your-secret>
AWS_DEFAULT_REGION=us-east-1
```

### Programmatic Access
```python
# Using boto3 with profile
session = boto3.Session(profile_name='polygon-s3')
s3_client = session.client('s3', region_name='us-east-1')
```

## Data Access Patterns

### 1. List Available Files
```python
def list_files(prefix, start_date, end_date):
    paginator = s3_client.get_paginator('list_objects_v2')
    for page in paginator.paginate(
        Bucket='flatfiles',
        Prefix=prefix
    ):
        for obj in page.get('Contents', []):
            yield obj['Key']
```

### 2. Download with Retry Logic
```python
def download_with_retry(s3_key, local_path, max_retries=3):
    for attempt in range(max_retries):
        try:
            s3_client.download_file(
                Bucket='flatfiles',
                Key=s3_key,
                Filename=local_path,
                Config=TransferConfig(
                    multipart_threshold=1024 * 25,  # 25MB
                    max_concurrency=10,
                    use_threads=True
                )
            )
            return True
        except Exception as e:
            if attempt < max_retries - 1:
                time.sleep(2 ** attempt)
            else:
                raise
```

### 3. Streaming Download
```python
def stream_download(s3_key):
    obj = s3_client.get_object(Bucket='flatfiles', Key=s3_key)
    with gzip.open(obj['Body'], 'rt') as gz:
        reader = csv.DictReader(gz)
        for row in reader:
            yield row
```

## Performance Optimization

### 1. Connection Pooling
```python
config = Config(
    region_name='us-east-1',
    retries={
        'max_attempts': 10,
        'mode': 'adaptive'
    },
    max_pool_connections=50
)
```

### 2. Parallel Downloads
```python
from concurrent.futures import ThreadPoolExecutor

def parallel_download(s3_keys, max_workers=10):
    with ThreadPoolExecutor(max_workers=max_workers) as executor:
        futures = {
            executor.submit(download_file, key): key 
            for key in s3_keys
        }
        for future in as_completed(futures):
            yield futures[future], future.result()
```

### 3. Bandwidth Management
```python
# Limit bandwidth using TransferConfig
config = TransferConfig(
    max_bandwidth=1024 * 1024 * 50  # 50 MB/s
)
```

## Error Handling

### Common S3 Errors
| Error | Cause | Solution |
|-------|-------|----------|
| NoSuchKey | File not found | Check key existence |
| AccessDenied | Permission issue | Verify credentials |
| RequestTimeout | Network issue | Implement retry |
| SlowDown | Rate limiting | Add exponential backoff |

### Rate Limiting Strategy
```python
@retry(
    stop=stop_after_attempt(5),
    wait=wait_exponential(multiplier=1, min=4, max=60),
    retry=retry_if_exception_type(ClientError)
)
def download_with_backoff(s3_key):
    return s3_client.download_file(...)
```

## Cost Optimization

### 1. Request Batching
- Use list_objects_v2 with pagination
- Download multiple small files in parallel
- Minimize HEAD requests

### 2. Data Transfer
- Enable compression for all transfers
- Use appropriate chunk sizes
- Cache frequently accessed data

### 3. Storage Classes
- Standard: Frequently accessed data
- Standard-IA: Older than 30 days
- Glacier: Archive data > 90 days

## Monitoring

### CloudWatch Metrics
- NumberOfObjects
- BucketSizeBytes
- AllRequests
- 4xxErrors
- 5xxErrors

### Application Metrics
```python
# Track download performance
metrics.s3_download_duration.observe(duration)
metrics.s3_download_bytes.inc(file_size)
metrics.s3_download_errors.labels(error_type).inc()
```

## Security Best Practices

1. **Credential Management**
   - Use AWS profiles, not hardcoded credentials
   - Rotate access keys regularly
   - Use IAM roles when possible

2. **Access Control**
   - Implement least privilege principle
   - Use bucket policies for fine-grained control
   - Enable CloudTrail logging

3. **Data Encryption**
   - Use HTTPS for all transfers
   - Enable SSE-S3 for data at rest
   - Verify file checksums after download

## Integration with File Provider

The FileProvider class integrates S3 access through:

1. **Polygon S3 Download Script** (`scripts/download_polygon_s3.py`)
   - Handles authentication and downloads
   - Manages checkpoints for resume capability
   - Organizes files by date on local storage

2. **File Provider** (`providers/file_provider.py`)
   - Reads downloaded files from local storage
   - Processes CSV data with validation
   - Maintains processing checkpoints

## Troubleshooting

### Debug S3 Connection
```bash
# Test S3 access
aws s3 ls s3://flatfiles/us_stocks_sip/ --profile polygon-s3

# Check specific file
aws s3api head-object \
  --bucket flatfiles \
  --key us_stocks_sip/day_aggs_v1/2024/01/2024-01-01.csv.gz \
  --profile polygon-s3
```

### Common Issues
1. **Slow Downloads**: Check network bandwidth and S3 endpoint
2. **Authentication Failures**: Verify AWS credentials and profile
3. **Missing Data**: Confirm file exists in S3 before download
4. **Corrupted Files**: Validate checksums after download