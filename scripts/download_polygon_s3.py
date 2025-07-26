#!/usr/bin/env python3
"""
Polygon S3 Data Downloader

A standalone script to download Polygon market data from S3 to an external drive.
Runs on the host machine (outside containers) and uses AWS profile credentials.

Features:
- AWS profile-based authentication
- Configurable external drive destination
- Date-based file organization
- Checkpoint/resume functionality
- Network interruption handling
- Progress tracking and logging
"""

import os
import sys
import json
import time
import logging
import argparse
import hashlib
from datetime import datetime, timedelta
from pathlib import Path
from typing import Optional, Dict, List, Tuple
import signal
import pickle

try:
    import boto3
    from botocore.exceptions import (
        ClientError, ConnectionError as BotoConnectionError,
        ReadTimeoutError, ConnectTimeoutError
    )
    from botocore.config import Config
except ImportError:
    print("ERROR: boto3 is required. Install with: pip install boto3")
    sys.exit(1)

try:
    from tqdm import tqdm
except ImportError:
    print("WARNING: tqdm not installed. Install with: pip install tqdm for progress bars")
    tqdm = None


class PolygonS3Downloader:
    """Handles downloading Polygon data from S3 to external storage."""
    
    def __init__(
        self,
        aws_profile: str,
        external_drive_path: str,
        bucket_name: str = "flatfiles",
        region: str = "us-east-1",
        checkpoint_file: str = ".polygon_download_checkpoint.pkl",
        log_file: Optional[str] = None
    ):
        """
        Initialize the downloader.
        
        Args:
            aws_profile: AWS profile name to use for authentication
            external_drive_path: Path to external drive for downloads
            bucket_name: S3 bucket name (default: flatfiles)
            region: AWS region (default: us-east-1)
            checkpoint_file: File to store download progress
            log_file: Optional log file path
        """
        self.aws_profile = aws_profile
        self.external_drive_path = Path(external_drive_path)
        self.bucket_name = bucket_name
        self.region = region
        self.checkpoint_file = self.external_drive_path / checkpoint_file
        
        # Ensure external drive path exists
        self.external_drive_path.mkdir(parents=True, exist_ok=True)
        
        # Setup logging
        self._setup_logging(log_file)
        
        # Initialize S3 client with profile
        self._init_s3_client()
        
        # Load checkpoint if exists
        self.checkpoint = self._load_checkpoint()
        
        # Track current download for graceful shutdown
        self.current_download = None
        self._setup_signal_handlers()
        
    def _setup_logging(self, log_file: Optional[str] = None):
        """Configure logging."""
        log_format = '%(asctime)s - %(name)s - %(levelname)s - %(message)s'
        
        handlers = [logging.StreamHandler(sys.stdout)]
        
        if log_file:
            log_path = self.external_drive_path / log_file
            handlers.append(logging.FileHandler(log_path))
        
        logging.basicConfig(
            level=logging.INFO,
            format=log_format,
            handlers=handlers
        )
        
        self.logger = logging.getLogger(__name__)
        
    def _init_s3_client(self):
        """Initialize S3 client with AWS profile."""
        try:
            # Create session with profile
            session = boto3.Session(profile_name=self.aws_profile)
            
            # Configure with retries and timeouts
            config = Config(
                region_name=self.region,
                retries={
                    'max_attempts': 10,
                    'mode': 'adaptive'
                },
                read_timeout=300,
                connect_timeout=60
            )
            
            self.s3_client = session.client('s3', config=config)
            
            # Test connection
            self.s3_client.head_bucket(Bucket=self.bucket_name)
            self.logger.info(f"Connected to S3 bucket '{self.bucket_name}' using profile '{self.aws_profile}'")
            
        except Exception as e:
            self.logger.error(f"Failed to initialize S3 client: {e}")
            raise
            
    def _setup_signal_handlers(self):
        """Setup graceful shutdown handlers."""
        def signal_handler(sig, frame):
            self.logger.info("Received interrupt signal. Saving checkpoint...")
            self._save_checkpoint()
            sys.exit(0)
            
        signal.signal(signal.SIGINT, signal_handler)
        signal.signal(signal.SIGTERM, signal_handler)
        
    def _load_checkpoint(self) -> Dict:
        """Load checkpoint from file."""
        if self.checkpoint_file.exists():
            try:
                with open(self.checkpoint_file, 'rb') as f:
                    checkpoint = pickle.load(f)
                self.logger.info(f"Loaded checkpoint with {len(checkpoint.get('completed', []))} completed files")
                return checkpoint
            except Exception as e:
                self.logger.warning(f"Failed to load checkpoint: {e}")
                
        return {
            'completed': set(),
            'failed': {},
            'last_marker': None,
            'total_size': 0,
            'total_files': 0
        }
        
    def _save_checkpoint(self):
        """Save current progress to checkpoint file."""
        try:
            with open(self.checkpoint_file, 'wb') as f:
                pickle.dump(self.checkpoint, f)
            self.logger.debug("Checkpoint saved")
        except Exception as e:
            self.logger.error(f"Failed to save checkpoint: {e}")
            
    def _get_date_path(self, s3_key: str) -> Path:
        """
        Extract date from S3 key and create organized path.
        
        Example S3 paths:
        - us_stocks_sip/day_aggs_v1/2024/01/2024-01-15.csv.gz
        - us_stocks_sip/trades_v1/2024/01/15/AAPL.csv.gz
        """
        parts = s3_key.split('/')
        
        # Try to extract date components
        year = None
        month = None
        day = None
        
        for i, part in enumerate(parts):
            # Check for year (4 digits)
            if len(part) == 4 and part.isdigit() and 2000 <= int(part) <= 2030:
                year = part
                # Check next parts for month and day
                if i + 1 < len(parts) and len(parts[i + 1]) == 2 and parts[i + 1].isdigit():
                    month = parts[i + 1]
                    if i + 2 < len(parts) and len(parts[i + 2]) == 2 and parts[i + 2].isdigit():
                        day = parts[i + 2]
        
        # Build organized path
        base_path = self.external_drive_path / "polygon_data"
        
        if year and month:
            if day:
                # Full date path
                date_path = base_path / year / month / day
            else:
                # Year/month path
                date_path = base_path / year / month
        else:
            # Fallback to preserving S3 structure
            date_path = base_path / os.path.dirname(s3_key)
            
        return date_path
        
    def _download_file(self, s3_key: str, retries: int = 3) -> bool:
        """
        Download a single file from S3.
        
        Args:
            s3_key: S3 object key
            retries: Number of retry attempts
            
        Returns:
            True if successful, False otherwise
        """
        # Skip if already completed
        if s3_key in self.checkpoint['completed']:
            return True
            
        # Get destination path
        dest_dir = self._get_date_path(s3_key)
        dest_dir.mkdir(parents=True, exist_ok=True)
        
        dest_file = dest_dir / os.path.basename(s3_key)
        
        # Track current download
        self.current_download = s3_key
        
        for attempt in range(retries):
            try:
                # Get object metadata
                response = self.s3_client.head_object(Bucket=self.bucket_name, Key=s3_key)
                file_size = response['ContentLength']
                
                # Skip if file exists and size matches
                if dest_file.exists() and dest_file.stat().st_size == file_size:
                    self.logger.info(f"File already exists: {dest_file}")
                    self.checkpoint['completed'].add(s3_key)
                    return True
                
                # Download with progress
                self.logger.info(f"Downloading: {s3_key} -> {dest_file}")
                
                if tqdm:
                    # Download with progress bar
                    with tqdm(total=file_size, unit='B', unit_scale=True, desc=os.path.basename(s3_key)) as pbar:
                        def download_callback(bytes_transferred):
                            pbar.update(bytes_transferred - pbar.n)
                            
                        self.s3_client.download_file(
                            self.bucket_name, s3_key, str(dest_file),
                            Callback=download_callback
                        )
                else:
                    # Simple download without progress bar
                    self.s3_client.download_file(self.bucket_name, s3_key, str(dest_file))
                
                # Verify download
                if dest_file.stat().st_size == file_size:
                    self.checkpoint['completed'].add(s3_key)
                    self.checkpoint['total_size'] += file_size
                    self.checkpoint['total_files'] += 1
                    self._save_checkpoint()
                    return True
                else:
                    raise Exception("File size mismatch after download")
                    
            except (BotoConnectionError, ReadTimeoutError, ConnectTimeoutError) as e:
                self.logger.warning(f"Network error on attempt {attempt + 1}/{retries}: {e}")
                if attempt < retries - 1:
                    time.sleep(2 ** attempt)  # Exponential backoff
                    
            except ClientError as e:
                error_code = e.response['Error']['Code']
                if error_code == '404':
                    self.logger.error(f"File not found: {s3_key}")
                else:
                    self.logger.error(f"S3 error: {e}")
                self.checkpoint['failed'][s3_key] = str(e)
                return False
                
            except Exception as e:
                self.logger.error(f"Unexpected error downloading {s3_key}: {e}")
                if attempt == retries - 1:
                    self.checkpoint['failed'][s3_key] = str(e)
                    
        return False
        
    def list_files(
        self,
        prefix: str,
        start_date: Optional[datetime] = None,
        end_date: Optional[datetime] = None,
        file_pattern: Optional[str] = None
    ) -> List[str]:
        """
        List files in S3 bucket with optional filtering.
        
        Args:
            prefix: S3 prefix to search
            start_date: Optional start date filter
            end_date: Optional end date filter
            file_pattern: Optional file pattern to match
            
        Returns:
            List of S3 keys
        """
        files = []
        paginator = self.s3_client.get_paginator('list_objects_v2')
        
        # Use checkpoint marker if available
        pagination_config = {'PageSize': 1000}
        if self.checkpoint.get('last_marker'):
            pagination_config['StartingToken'] = self.checkpoint['last_marker']
            
        try:
            for page in paginator.paginate(
                Bucket=self.bucket_name,
                Prefix=prefix,
                PaginationConfig=pagination_config
            ):
                if 'Contents' not in page:
                    continue
                    
                for obj in page['Contents']:
                    key = obj['Key']
                    
                    # Apply filters
                    if file_pattern and file_pattern not in key:
                        continue
                        
                    # Date filtering (basic implementation)
                    if start_date or end_date:
                        # Try to extract date from filename
                        basename = os.path.basename(key)
                        date_match = None
                        
                        # Try YYYY-MM-DD format
                        import re
                        date_pattern = r'(\d{4})-(\d{2})-(\d{2})'
                        match = re.search(date_pattern, basename)
                        if match:
                            try:
                                file_date = datetime(
                                    int(match.group(1)),
                                    int(match.group(2)),
                                    int(match.group(3))
                                )
                                
                                if start_date and file_date < start_date:
                                    continue
                                if end_date and file_date > end_date:
                                    continue
                                    
                            except ValueError:
                                pass
                                
                    files.append(key)
                    
                # Update checkpoint marker
                if 'NextContinuationToken' in page:
                    self.checkpoint['last_marker'] = page['NextContinuationToken']
                    
        except Exception as e:
            self.logger.error(f"Error listing files: {e}")
            raise
            
        return files
        
    def download_batch(
        self,
        prefix: str,
        start_date: Optional[datetime] = None,
        end_date: Optional[datetime] = None,
        file_pattern: Optional[str] = None,
        max_files: Optional[int] = None
    ):
        """
        Download a batch of files from S3.
        
        Args:
            prefix: S3 prefix to download from
            start_date: Optional start date filter
            end_date: Optional end date filter
            file_pattern: Optional file pattern to match
            max_files: Maximum number of files to download
        """
        self.logger.info(f"Starting batch download from prefix: {prefix}")
        
        # List files
        files = self.list_files(prefix, start_date, end_date, file_pattern)
        
        # Filter out completed files
        pending_files = [f for f in files if f not in self.checkpoint['completed']]
        
        if max_files:
            pending_files = pending_files[:max_files]
            
        self.logger.info(f"Found {len(pending_files)} files to download")
        
        # Download files
        success_count = 0
        failed_count = 0
        
        for i, s3_key in enumerate(pending_files, 1):
            self.logger.info(f"Processing {i}/{len(pending_files)}: {s3_key}")
            
            if self._download_file(s3_key):
                success_count += 1
            else:
                failed_count += 1
                
            # Log progress
            if i % 10 == 0:
                self._log_progress(success_count, failed_count, len(pending_files))
                
        # Final summary
        self._log_summary(success_count, failed_count)
        
    def _log_progress(self, success: int, failed: int, total: int):
        """Log download progress."""
        completed_pct = (success + failed) / total * 100
        size_gb = self.checkpoint['total_size'] / (1024 ** 3)
        
        self.logger.info(
            f"Progress: {completed_pct:.1f}% | "
            f"Success: {success} | Failed: {failed} | "
            f"Total size: {size_gb:.2f} GB"
        )
        
    def _log_summary(self, success: int, failed: int):
        """Log download summary."""
        total_files = self.checkpoint['total_files']
        total_size_gb = self.checkpoint['total_size'] / (1024 ** 3)
        
        self.logger.info("=" * 60)
        self.logger.info("Download Summary:")
        self.logger.info(f"  Total files downloaded: {total_files}")
        self.logger.info(f"  Total size: {total_size_gb:.2f} GB")
        self.logger.info(f"  Session success: {success}")
        self.logger.info(f"  Session failed: {failed}")
        self.logger.info(f"  Failed files: {len(self.checkpoint['failed'])}")
        self.logger.info("=" * 60)
        
    def retry_failed(self):
        """Retry downloading failed files."""
        failed_files = list(self.checkpoint['failed'].keys())
        
        if not failed_files:
            self.logger.info("No failed files to retry")
            return
            
        self.logger.info(f"Retrying {len(failed_files)} failed files...")
        
        # Clear failed status for retry
        for key in failed_files:
            del self.checkpoint['failed'][key]
            
        # Retry downloads
        success_count = 0
        for s3_key in failed_files:
            if self._download_file(s3_key, retries=5):
                success_count += 1
                
        self.logger.info(f"Retry complete: {success_count}/{len(failed_files)} successful")
        
    def get_status(self) -> Dict:
        """Get current download status."""
        return {
            'completed_files': len(self.checkpoint['completed']),
            'failed_files': len(self.checkpoint['failed']),
            'total_size_gb': self.checkpoint['total_size'] / (1024 ** 3),
            'destination': str(self.external_drive_path)
        }


def main():
    """Main entry point."""
    parser = argparse.ArgumentParser(
        description="Download Polygon market data from S3 to external drive"
    )
    
    # Required arguments
    parser.add_argument(
        '--profile',
        required=True,
        help='AWS profile name to use for authentication'
    )
    parser.add_argument(
        '--destination',
        required=True,
        help='External drive path for downloads (e.g., /mnt/external/polygon)'
    )
    
    # Optional arguments
    parser.add_argument(
        '--bucket',
        default='flatfiles',
        help='S3 bucket name (default: flatfiles)'
    )
    parser.add_argument(
        '--prefix',
        default='us_stocks_sip/day_aggs_v1/',
        help='S3 prefix to download (default: us_stocks_sip/day_aggs_v1/)'
    )
    parser.add_argument(
        '--start-date',
        help='Start date (YYYY-MM-DD)'
    )
    parser.add_argument(
        '--end-date',
        help='End date (YYYY-MM-DD)'
    )
    parser.add_argument(
        '--pattern',
        help='File pattern to match (e.g., "AAPL" for Apple stock)'
    )
    parser.add_argument(
        '--max-files',
        type=int,
        help='Maximum number of files to download'
    )
    parser.add_argument(
        '--retry-failed',
        action='store_true',
        help='Retry previously failed downloads'
    )
    parser.add_argument(
        '--log-file',
        default='polygon_download.log',
        help='Log file name (default: polygon_download.log)'
    )
    parser.add_argument(
        '--region',
        default='us-east-1',
        help='AWS region (default: us-east-1)'
    )
    
    args = parser.parse_args()
    
    # Parse dates
    start_date = None
    end_date = None
    
    if args.start_date:
        try:
            start_date = datetime.strptime(args.start_date, '%Y-%m-%d')
        except ValueError:
            print(f"ERROR: Invalid start date format: {args.start_date}")
            sys.exit(1)
            
    if args.end_date:
        try:
            end_date = datetime.strptime(args.end_date, '%Y-%m-%d')
        except ValueError:
            print(f"ERROR: Invalid end date format: {args.end_date}")
            sys.exit(1)
    
    # Create downloader
    try:
        downloader = PolygonS3Downloader(
            aws_profile=args.profile,
            external_drive_path=args.destination,
            bucket_name=args.bucket,
            region=args.region,
            log_file=args.log_file
        )
    except Exception as e:
        print(f"ERROR: Failed to initialize downloader: {e}")
        sys.exit(1)
    
    # Show current status
    status = downloader.get_status()
    print("\nCurrent Status:")
    print(f"  Destination: {status['destination']}")
    print(f"  Completed files: {status['completed_files']}")
    print(f"  Failed files: {status['failed_files']}")
    print(f"  Total size: {status['total_size_gb']:.2f} GB")
    print()
    
    # Execute requested operation
    try:
        if args.retry_failed:
            downloader.retry_failed()
        else:
            downloader.download_batch(
                prefix=args.prefix,
                start_date=start_date,
                end_date=end_date,
                file_pattern=args.pattern,
                max_files=args.max_files
            )
    except KeyboardInterrupt:
        print("\nDownload interrupted by user")
    except Exception as e:
        print(f"\nERROR: {e}")
        sys.exit(1)


if __name__ == '__main__':
    main()