# S3 Downloader Module Design

## Overview
Efficient, concurrent S3 download system for Polygon.io historical data files.

## Key Components

### S3Client Wrapper
```python
import aioboto3
import asyncio
from typing import Optional, List, Dict, AsyncIterator
from dataclasses import dataclass
import aiofiles
from pathlib import Path

@dataclass
class S3Config:
    bucket_name: str = "polygon-flat-files"
    region: str = "us-east-1"
    access_key: Optional[str] = None
    secret_key: Optional[str] = None
    endpoint_url: Optional[str] = None  # For S3-compatible services

class S3Client:
    def __init__(self, config: S3Config):
        self.config = config
        self.session = None
        
    async def __aenter__(self):
        self.session = aioboto3.Session()
        return self
        
    async def __aexit__(self, exc_type, exc_val, exc_tb):
        # Cleanup
        pass
    
    async def list_objects(self, prefix: str) -> AsyncIterator[Dict]:
        """List objects with pagination support"""
        async with self.session.client('s3') as s3:
            paginator = s3.get_paginator('list_objects_v2')
            async for page in paginator.paginate(
                Bucket=self.config.bucket_name,
                Prefix=prefix
            ):
                for obj in page.get('Contents', []):
                    yield {
                        'key': obj['Key'],
                        'size': obj['Size'],
                        'last_modified': obj['LastModified']
                    }
    
    async def download_file(self, s3_key: str, local_path: Path) -> Dict:
        """Download single file with progress tracking"""
        local_path.parent.mkdir(parents=True, exist_ok=True)
        
        async with self.session.client('s3') as s3:
            # Get object size for progress tracking
            response = await s3.head_object(
                Bucket=self.config.bucket_name,
                Key=s3_key
            )
            total_size = response['ContentLength']
            
            # Stream download
            response = await s3.get_object(
                Bucket=self.config.bucket_name,
                Key=s3_key
            )
            
            downloaded = 0
            async with aiofiles.open(local_path, 'wb') as f:
                async for chunk in response['Body']:
                    await f.write(chunk)
                    downloaded += len(chunk)
                    yield {
                        'progress': downloaded / total_size,
                        'downloaded': downloaded,
                        'total': total_size
                    }
```

### Concurrent Download Manager
```python
import asyncio
from typing import List, Dict, Callable, Optional
from datetime import datetime
import hashlib

@dataclass
class DownloadJob:
    symbol: str
    date: str
    s3_key: str
    local_path: Path
    expected_size: Optional[int] = None
    checksum: Optional[str] = None
    attempts: int = 0
    error: Optional[str] = None
    
    @property
    def id(self) -> str:
        return f"{self.symbol}_{self.date}"

class ConcurrentDownloadManager:
    def __init__(
        self,
        s3_client: S3Client,
        max_concurrent: int = 10,
        max_retries: int = 3,
        progress_callback: Optional[Callable] = None
    ):
        self.s3_client = s3_client
        self.max_concurrent = max_concurrent
        self.max_retries = max_retries
        self.progress_callback = progress_callback
        self.semaphore = asyncio.Semaphore(max_concurrent)
        self.active_downloads: Dict[str, DownloadJob] = {}
        self.completed_downloads: List[DownloadJob] = []
        self.failed_downloads: List[DownloadJob] = []
        
    async def download_batch(self, jobs: List[DownloadJob]) -> Dict[str, Any]:
        """Download multiple files concurrently"""
        start_time = datetime.utcnow()
        
        # Create download tasks
        tasks = [self._download_with_retry(job) for job in jobs]
        
        # Execute concurrently
        results = await asyncio.gather(*tasks, return_exceptions=True)
        
        # Process results
        total_size = sum(j.expected_size or 0 for j in self.completed_downloads)
        duration = (datetime.utcnow() - start_time).total_seconds()
        
        return {
            'total_jobs': len(jobs),
            'completed': len(self.completed_downloads),
            'failed': len(self.failed_downloads),
            'total_size_mb': total_size / (1024 * 1024),
            'duration_seconds': duration,
            'avg_speed_mbps': (total_size * 8) / (duration * 1_000_000) if duration > 0 else 0,
            'failed_jobs': self.failed_downloads
        }
    
    async def _download_with_retry(self, job: DownloadJob) -> Optional[DownloadJob]:
        """Download with retry logic"""
        async with self.semaphore:
            self.active_downloads[job.id] = job
            
            for attempt in range(self.max_retries):
                job.attempts = attempt + 1
                try:
                    await self._download_single(job)
                    self.completed_downloads.append(job)
                    del self.active_downloads[job.id]
                    return job
                except Exception as e:
                    job.error = str(e)
                    if attempt < self.max_retries - 1:
                        await asyncio.sleep(2 ** attempt)  # Exponential backoff
                    else:
                        self.failed_downloads.append(job)
                        del self.active_downloads[job.id]
                        return None
    
    async def _download_single(self, job: DownloadJob):
        """Download and verify single file"""
        # Download with progress tracking
        progress_data = []
        async for progress in self.s3_client.download_file(job.s3_key, job.local_path):
            progress_data.append(progress)
            if self.progress_callback and len(progress_data) % 10 == 0:
                await self.progress_callback(job, progress)
        
        # Verify download
        if job.expected_size:
            actual_size = job.local_path.stat().st_size
            if actual_size != job.expected_size:
                raise ValueError(f"Size mismatch: expected {job.expected_size}, got {actual_size}")
        
        # Verify checksum if provided
        if job.checksum:
            actual_checksum = await self._calculate_checksum(job.local_path)
            if actual_checksum != job.checksum:
                raise ValueError(f"Checksum mismatch: expected {job.checksum}, got {actual_checksum}")
```

### Smart Download Scheduler
```python
from typing import List, Dict, Set
import asyncio
from datetime import datetime, timedelta

class SmartDownloadScheduler:
    """Intelligent scheduling based on data patterns and network conditions"""
    
    def __init__(
        self,
        downloader: ConcurrentDownloadManager,
        bandwidth_limit_mbps: Optional[float] = None
    ):
        self.downloader = downloader
        self.bandwidth_limit = bandwidth_limit_mbps
        self.download_history: List[Dict] = []
        self.active_symbols: Set[str] = set()
        
    async def schedule_downloads(
        self,
        symbols: List[str],
        start_date: datetime,
        end_date: datetime,
        priority_symbols: Optional[List[str]] = None
    ) -> AsyncIterator[Dict]:
        """Schedule downloads optimally"""
        
        # Generate all download jobs
        all_jobs = await self._generate_jobs(symbols, start_date, end_date)
        
        # Prioritize jobs
        prioritized = self._prioritize_jobs(all_jobs, priority_symbols)
        
        # Download in optimized batches
        for batch in self._create_optimized_batches(prioritized):
            # Adjust concurrency based on bandwidth
            if self.bandwidth_limit:
                self.downloader.max_concurrent = self._calculate_optimal_concurrency()
            
            # Execute batch
            result = await self.downloader.download_batch(batch)
            self.download_history.append(result)
            
            yield {
                'batch_num': len(self.download_history),
                'result': result,
                'active_symbols': list(self.active_symbols),
                'estimated_time_remaining': self._estimate_time_remaining(all_jobs)
            }
    
    def _prioritize_jobs(
        self,
        jobs: List[DownloadJob],
        priority_symbols: Optional[List[str]]
    ) -> List[DownloadJob]:
        """Smart prioritization based on multiple factors"""
        
        def priority_score(job: DownloadJob) -> tuple:
            # Higher score = higher priority
            scores = []
            
            # Priority symbols first
            if priority_symbols and job.symbol in priority_symbols:
                scores.append(1000 - priority_symbols.index(job.symbol))
            else:
                scores.append(0)
            
            # Recent dates have higher priority
            date_score = (datetime.now() - datetime.strptime(job.date, '%Y-%m-%d')).days
            scores.append(-date_score)  # Negative so recent dates come first
            
            # Smaller files first for quick wins
            scores.append(-(job.expected_size or float('inf')))
            
            return tuple(scores)
        
        return sorted(jobs, key=priority_score, reverse=True)
```

## Integration Points

### With Batch Processor
```python
# Hand off downloaded files for processing
async def download_and_process_pipeline():
    async for download_result in scheduler.schedule_downloads(symbols, start, end):
        completed_files = [
            job.local_path 
            for job in download_result['result']['completed_downloads']
        ]
        
        # Send to batch processor
        await batch_processor.queue_files(completed_files)
```

### With Progress Tracker
```python
# Update progress after each batch
async def progress_callback(job: DownloadJob, progress: Dict):
    await tracker.update_download_progress(
        symbol=job.symbol,
        date=job.date,
        progress=progress['progress'],
        speed_mbps=calculate_speed(progress)
    )
```

## Performance Considerations

1. **Connection Pooling**: Reuse S3 connections across downloads
2. **Chunk Size**: Optimize download chunk size (default 8MB)
3. **Concurrent Limits**: Balance between speed and resource usage
4. **Bandwidth Management**: Optional rate limiting to avoid network saturation
5. **Local Storage**: Use fast SSD for temporary download storage
6. **Memory Efficiency**: Stream downloads instead of loading to memory

## Error Handling

1. **Network Errors**: Automatic retry with exponential backoff
2. **Corrupt Files**: Checksum verification and re-download
3. **Storage Errors**: Check disk space before download
4. **Rate Limits**: Respect S3 rate limits with adaptive throttling
5. **Partial Downloads**: Resume capability for large files