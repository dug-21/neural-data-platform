# Polygon S3 Data Downloader

A standalone Python script for downloading Polygon market data from S3 to an external drive. This script runs on the host machine (outside containers) and provides robust downloading with checkpoint/resume functionality.

## Features

- **AWS Profile Authentication**: Uses AWS profile credentials (no environment variables needed)
- **External Drive Support**: Downloads directly to configurable external drive location
- **Date-based Organization**: Automatically organizes files by date structure
- **Checkpoint/Resume**: Automatically saves progress and resumes interrupted downloads
- **Network Resilience**: Handles network interruptions with automatic retries
- **Progress Tracking**: Detailed logging and optional progress bars
- **Graceful Shutdown**: Saves checkpoint on Ctrl+C interruption

## Installation

1. **Install Python dependencies** (on host machine):
```bash
pip install -r scripts/requirements_polygon_download.txt
```

2. **Configure AWS credentials** - Choose one method:

   **Option A: Environment Variables (Recommended)**
   ```bash
   export AWS_ACCESS_KEY_ID=your_polygon_access_key
   export AWS_SECRET_ACCESS_KEY=your_polygon_secret_key
   ```

   **Option B: AWS Profile**
   ```bash
   aws configure --profile polygon
   # Enter your AWS Access Key ID, Secret Access Key, and region
   ```

## Usage

### Basic Usage

```bash
# Download daily aggregates to external drive (using environment variables)
python3 scripts/download_polygon_s3.py \
    --destination /mnt/external/polygon_data

# Or with AWS profile
python3 scripts/download_polygon_s3.py \
    --profile polygon \
    --destination /mnt/external/polygon_data
```

### Advanced Usage

```bash
# Download specific date range with pattern matching
python3 scripts/download_polygon_s3.py \
    --profile polygon \
    --destination /mnt/external/polygon_data \
    --prefix us_stocks_sip/trades_v1/ \
    --start-date 2024-01-01 \
    --end-date 2024-01-31 \
    --pattern AAPL \
    --max-files 100
```

### Command Line Options

| Option | Required | Default | Description |
|--------|----------|---------|-------------|
| `--profile` | Yes | - | AWS profile name for authentication |
| `--destination` | Yes | - | External drive path for downloads |
| `--bucket` | No | `flatfiles` | S3 bucket name |
| `--prefix` | No | `us_stocks_sip/day_aggs_v1/` | S3 prefix to download |
| `--start-date` | No | - | Start date (YYYY-MM-DD) |
| `--end-date` | No | - | End date (YYYY-MM-DD) |
| `--pattern` | No | - | File pattern to match |
| `--max-files` | No | - | Maximum number of files to download |
| `--retry-failed` | No | False | Retry previously failed downloads |
| `--log-file` | No | `polygon_download.log` | Log file name |
| `--region` | No | `us-east-1` | AWS region |

## File Organization

The script automatically organizes downloaded files by date:

```
/mnt/external/polygon_data/
├── 2024/
│   ├── 01/
│   │   ├── 01/
│   │   │   └── trades_files...
│   │   ├── 02/
│   │   │   └── trades_files...
│   │   └── 2024-01-15.csv.gz  # Daily aggregates
│   └── 02/
│       └── ...
└── checkpoint files...
```

## Checkpoint System

The script maintains a checkpoint file (`.polygon_download_checkpoint.pkl`) that tracks:
- Completed downloads
- Failed downloads with error details
- Total size and file count
- Last S3 listing position

This allows you to:
- Resume interrupted downloads
- Skip already downloaded files
- Retry failed downloads
- Track overall progress

## Network Resilience

The script handles network issues gracefully:
- **Automatic retries** with exponential backoff
- **Connection timeouts** and read timeouts
- **Partial download detection** and retry
- **File integrity verification** using size checks

## Examples

### 1. Download Daily Aggregates (Most Common)

```bash
python3 scripts/download_polygon_s3.py \
    --profile polygon \
    --destination /media/external/polygon \
    --prefix us_stocks_sip/day_aggs_v1/ \
    --start-date 2024-01-01 \
    --end-date 2024-12-31
```

### 2. Download Individual Stock Trades

```bash
python3 scripts/download_polygon_s3.py \
    --profile polygon \
    --destination /media/external/polygon \
    --prefix us_stocks_sip/trades_v1/ \
    --pattern TSLA \
    --start-date 2024-06-01 \
    --end-date 2024-06-30
```

### 3. Download Options Data

```bash
python3 scripts/download_polygon_s3.py \
    --profile polygon \
    --destination /media/external/polygon \
    --prefix us_options_opra/trades_v1/ \
    --start-date 2024-01-01 \
    --max-files 1000
```

### 4. Retry Failed Downloads

```bash
python3 scripts/download_polygon_s3.py \
    --profile polygon \
    --destination /media/external/polygon \
    --retry-failed
```

### 5. Test Download (Limited Files)

```bash
python3 scripts/download_polygon_s3.py \
    --profile polygon \
    --destination /tmp/polygon_test \
    --max-files 10
```

## Monitoring Progress

The script provides detailed logging:

```
2024-01-15 10:30:15 - Connected to S3 bucket 'flatfiles' using profile 'polygon'
2024-01-15 10:30:16 - Starting batch download from prefix: us_stocks_sip/day_aggs_v1/
2024-01-15 10:30:18 - Found 365 files to download
2024-01-15 10:30:20 - Processing 1/365: us_stocks_sip/day_aggs_v1/2024/01/2024-01-01.csv.gz
2024-01-15 10:30:25 - Downloading: us_stocks_sip/day_aggs_v1/2024/01/2024-01-01.csv.gz
2024-01-15 10:30:30 - Progress: 2.7% | Success: 10 | Failed: 0 | Total size: 1.23 GB
```

## Troubleshooting

### AWS Authentication Issues

1. **Check AWS profile**:
```bash
aws configure list --profile polygon
```

2. **Test S3 access**:
```bash
aws s3 ls s3://flatfiles/ --profile polygon
```

### Storage Issues

1. **Check external drive space**:
```bash
df -h /mnt/external
```

2. **Check permissions**:
```bash
ls -la /mnt/external
```

### Network Issues

The script automatically handles most network issues, but you can:
- Increase retry attempts in the code
- Use `--max-files` to limit batch size
- Monitor logs for specific error patterns

### Resume Interrupted Downloads

Simply run the same command again - the script will automatically resume from where it left off using the checkpoint file.

## File Structure

- `download_polygon_s3.py` - Main script
- `requirements_polygon_download.txt` - Python dependencies
- `.polygon_download_checkpoint.pkl` - Progress checkpoint (created automatically)
- `polygon_download.log` - Download log (created automatically)

## Security Notes

- Uses AWS profile credentials (more secure than environment variables)
- No credentials stored in the script
- Supports AWS IAM roles and temporary credentials
- All network connections use HTTPS

## Performance Tips

1. **Use fast external drive** (USB 3.0+ or Thunderbolt)
2. **Stable internet connection** (wired preferred)
3. **Run during off-peak hours** for better S3 performance
4. **Monitor disk space** before starting large downloads
5. **Use specific date ranges** to limit download scope