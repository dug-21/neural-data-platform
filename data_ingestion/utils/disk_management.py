"""
Disk usage management for data ingestion service.

Implements efficient disk usage strategies including:
- Streaming data processing
- Automatic cleanup
- Compression policies
- Memory-efficient operations
"""

import os
import shutil
import asyncio
import aiofiles
from pathlib import Path
from datetime import datetime, timedelta
from typing import Dict, List, Optional, Tuple
import psutil
import logging
from dataclasses import dataclass
from enum import Enum

from utils.logging import get_logger

logger = get_logger(__name__)


class CleanupPriority(Enum):
    """Cleanup priority levels"""
    LOW = 1      # Normal cleanup
    MEDIUM = 2   # Warning threshold reached
    HIGH = 3     # Critical threshold reached
    EMERGENCY = 4  # Disk almost full


@dataclass
class DiskUsageConfig:
    """Configuration for disk usage management"""
    max_disk_usage_mb: int = 1024  # 1 GB
    warning_threshold: float = 0.8   # 80%
    critical_threshold: float = 0.95  # 95%
    cleanup_interval_seconds: int = 300  # 5 minutes
    temp_file_ttl_hours: int = 1
    log_file_ttl_days: int = 3
    cache_ttl_hours: int = 24


@dataclass
class DiskUsageStats:
    """Current disk usage statistics"""
    total_bytes: int
    used_bytes: int
    free_bytes: int
    usage_percent: float
    path: str
    timestamp: datetime


class DiskManager:
    """Manages disk usage for the data ingestion service"""
    
    def __init__(self, config: Optional[DiskUsageConfig] = None):
        self.config = config or DiskUsageConfig()
        self.monitored_paths = {
            'temp': Path('/tmp/neural_trader'),
            'cache': Path('/app/cache'),
            'logs': Path('/app/logs'),
            'data_buffer': Path('/tmp/data_buffer')
        }
        self._cleanup_task = None
        self._stats_cache = {}
        
    async def start(self):
        """Start the disk management background task"""
        self._cleanup_task = asyncio.create_task(self._cleanup_loop())
        logger.info("Disk manager started")
        
    async def stop(self):
        """Stop the disk management task"""
        if self._cleanup_task:
            self._cleanup_task.cancel()
            await asyncio.gather(self._cleanup_task, return_exceptions=True)
        logger.info("Disk manager stopped")
        
    async def _cleanup_loop(self):
        """Main cleanup loop"""
        while True:
            try:
                await asyncio.sleep(self.config.cleanup_interval_seconds)
                
                # Check disk usage
                stats = await self.get_disk_usage()
                
                # Determine cleanup priority
                priority = self._determine_cleanup_priority(stats)
                
                # Perform cleanup based on priority
                await self._perform_cleanup(priority)
                
            except asyncio.CancelledError:
                break
            except Exception as e:
                logger.error(f"Error in cleanup loop: {e}", exc_info=True)
                
    def _determine_cleanup_priority(self, stats: DiskUsageStats) -> CleanupPriority:
        """Determine cleanup priority based on disk usage"""
        usage_ratio = stats.usage_percent / 100.0
        
        if usage_ratio >= self.config.critical_threshold:
            return CleanupPriority.EMERGENCY
        elif usage_ratio >= self.config.warning_threshold:
            return CleanupPriority.HIGH
        elif usage_ratio >= 0.6:
            return CleanupPriority.MEDIUM
        else:
            return CleanupPriority.LOW
            
    async def _perform_cleanup(self, priority: CleanupPriority):
        """Perform cleanup based on priority level"""
        logger.info(f"Performing {priority.name} priority cleanup")
        
        if priority == CleanupPriority.LOW:
            # Routine cleanup
            await self._cleanup_old_temp_files()
            await self._rotate_logs()
            
        elif priority == CleanupPriority.MEDIUM:
            # More aggressive cleanup
            await self._cleanup_old_temp_files(ttl_hours=0.5)
            await self._cleanup_cache(keep_recent_hours=12)
            await self._compress_logs()
            
        elif priority == CleanupPriority.HIGH:
            # Aggressive cleanup
            await self._cleanup_old_temp_files(ttl_hours=0)
            await self._cleanup_cache(keep_recent_hours=1)
            await self._compress_logs()
            await self._cleanup_data_buffers()
            
        elif priority == CleanupPriority.EMERGENCY:
            # Emergency cleanup - remove everything non-essential
            await self._emergency_cleanup()
            
    async def get_disk_usage(self, path: Optional[str] = None) -> DiskUsageStats:
        """Get current disk usage statistics"""
        target_path = path or '/'
        
        # Check cache
        cache_key = f"{target_path}:{datetime.now().minute}"
        if cache_key in self._stats_cache:
            return self._stats_cache[cache_key]
        
        usage = psutil.disk_usage(target_path)
        
        stats = DiskUsageStats(
            total_bytes=usage.total,
            used_bytes=usage.used,
            free_bytes=usage.free,
            usage_percent=usage.percent,
            path=target_path,
            timestamp=datetime.now()
        )
        
        # Cache for 1 minute
        self._stats_cache[cache_key] = stats
        
        # Log if usage is high
        if stats.usage_percent >= self.config.warning_threshold * 100:
            logger.warning(
                f"High disk usage: {stats.usage_percent:.1f}% "
                f"({stats.used_bytes / 1e9:.1f} GB / {stats.total_bytes / 1e9:.1f} GB)"
            )
            
        return stats
        
    async def _cleanup_old_temp_files(self, ttl_hours: Optional[float] = None):
        """Remove temporary files older than TTL"""
        ttl = ttl_hours or self.config.temp_file_ttl_hours
        cutoff_time = datetime.now() - timedelta(hours=ttl)
        
        temp_path = self.monitored_paths['temp']
        if not temp_path.exists():
            return
            
        removed_count = 0
        removed_bytes = 0
        
        for file_path in temp_path.rglob('*'):
            if file_path.is_file():
                try:
                    stat = file_path.stat()
                    if datetime.fromtimestamp(stat.st_mtime) < cutoff_time:
                        removed_bytes += stat.st_size
                        file_path.unlink()
                        removed_count += 1
                except Exception as e:
                    logger.error(f"Failed to remove {file_path}: {e}")
                    
        if removed_count > 0:
            logger.info(
                f"Removed {removed_count} temp files "
                f"({removed_bytes / 1e6:.1f} MB)"
            )
            
    async def _cleanup_cache(self, keep_recent_hours: int = 24):
        """Clean up cache files"""
        cache_path = self.monitored_paths['cache']
        if not cache_path.exists():
            return
            
        cutoff_time = datetime.now() - timedelta(hours=keep_recent_hours)
        
        for cache_file in cache_path.rglob('*.cache'):
            try:
                stat = cache_file.stat()
                if datetime.fromtimestamp(stat.st_mtime) < cutoff_time:
                    cache_file.unlink()
            except Exception as e:
                logger.error(f"Failed to remove cache {cache_file}: {e}")
                
    async def _rotate_logs(self):
        """Rotate log files to prevent unlimited growth"""
        log_path = self.monitored_paths['logs']
        if not log_path.exists():
            return
            
        max_log_size = 100 * 1024 * 1024  # 100 MB
        
        for log_file in log_path.glob('*.log'):
            try:
                if log_file.stat().st_size > max_log_size:
                    # Rotate the log
                    rotated_name = f"{log_file.stem}.{datetime.now():%Y%m%d_%H%M%S}.log"
                    rotated_path = log_file.parent / rotated_name
                    log_file.rename(rotated_path)
                    
                    # Compress rotated log
                    await self._compress_file(rotated_path)
                    
            except Exception as e:
                logger.error(f"Failed to rotate log {log_file}: {e}")
                
    async def _compress_logs(self):
        """Compress old log files"""
        log_path = self.monitored_paths['logs']
        if not log_path.exists():
            return
            
        import gzip
        
        for log_file in log_path.glob('*.log'):
            # Skip if already compressed or too small
            if log_file.suffix == '.gz' or log_file.stat().st_size < 10 * 1024 * 1024:
                continue
                
            try:
                compressed_path = log_file.with_suffix('.log.gz')
                
                async with aiofiles.open(log_file, 'rb') as f_in:
                    content = await f_in.read()
                    
                with gzip.open(compressed_path, 'wb') as f_out:
                    f_out.write(content)
                    
                # Remove original after successful compression
                log_file.unlink()
                
                logger.info(f"Compressed {log_file.name}")
                
            except Exception as e:
                logger.error(f"Failed to compress {log_file}: {e}")
                
    async def _compress_file(self, file_path: Path):
        """Compress a single file"""
        import gzip
        
        try:
            compressed_path = file_path.with_suffix(file_path.suffix + '.gz')
            
            async with aiofiles.open(file_path, 'rb') as f_in:
                content = await f_in.read()
                
            with gzip.open(compressed_path, 'wb') as f_out:
                f_out.write(content)
                
            file_path.unlink()
            
        except Exception as e:
            logger.error(f"Failed to compress {file_path}: {e}")
            
    async def _cleanup_data_buffers(self):
        """Clean up data buffer files"""
        buffer_path = self.monitored_paths['data_buffer']
        if not buffer_path.exists():
            return
            
        # Remove all buffer files older than 10 minutes
        cutoff_time = datetime.now() - timedelta(minutes=10)
        
        for buffer_file in buffer_path.glob('*.buffer'):
            try:
                stat = buffer_file.stat()
                if datetime.fromtimestamp(stat.st_mtime) < cutoff_time:
                    buffer_file.unlink()
            except Exception as e:
                logger.error(f"Failed to remove buffer {buffer_file}: {e}")
                
    async def _emergency_cleanup(self):
        """Emergency cleanup when disk is almost full"""
        logger.warning("Performing emergency disk cleanup!")
        
        # Clear all temporary directories
        for name, path in self.monitored_paths.items():
            if name in ['temp', 'cache', 'data_buffer']:
                try:
                    if path.exists():
                        shutil.rmtree(path)
                        path.mkdir(exist_ok=True)
                        logger.info(f"Cleared {name} directory")
                except Exception as e:
                    logger.error(f"Failed to clear {name}: {e}")
                    
        # Truncate large log files
        await self._truncate_logs()
        
    async def _truncate_logs(self):
        """Truncate log files to save space"""
        log_path = self.monitored_paths['logs']
        if not log_path.exists():
            return
            
        max_lines = 1000  # Keep last 1000 lines
        
        for log_file in log_path.glob('*.log'):
            try:
                # Read last N lines
                async with aiofiles.open(log_file, 'r') as f:
                    lines = await f.readlines()
                    
                if len(lines) > max_lines:
                    # Keep only last N lines
                    async with aiofiles.open(log_file, 'w') as f:
                        await f.writelines(lines[-max_lines:])
                        
                    logger.info(f"Truncated {log_file.name} to {max_lines} lines")
                    
            except Exception as e:
                logger.error(f"Failed to truncate {log_file}: {e}")


class StreamingDataHandler:
    """Handles data in streaming fashion to minimize disk usage"""
    
    def __init__(self, batch_size: int = 1000):
        self.batch_size = batch_size
        self.buffer = []
        self.processed_count = 0
        
    async def process_data(self, data_item: Dict, processor_func):
        """Process data item without storing to disk"""
        self.buffer.append(data_item)
        
        if len(self.buffer) >= self.batch_size:
            await self._flush_buffer(processor_func)
            
    async def _flush_buffer(self, processor_func):
        """Process buffered data and clear buffer"""
        if not self.buffer:
            return
            
        try:
            # Process batch
            await processor_func(self.buffer)
            self.processed_count += len(self.buffer)
            
            # Clear buffer to free memory
            self.buffer.clear()
            
        except Exception as e:
            logger.error(f"Error processing batch: {e}")
            # Clear buffer even on error to prevent memory buildup
            self.buffer.clear()
            
    async def flush(self, processor_func):
        """Force flush any remaining data"""
        await self._flush_buffer(processor_func)
        
    def get_stats(self) -> Dict:
        """Get processing statistics"""
        return {
            'processed_count': self.processed_count,
            'buffer_size': len(self.buffer),
            'memory_usage_mb': self._estimate_buffer_memory() / 1e6
        }
        
    def _estimate_buffer_memory(self) -> int:
        """Estimate memory usage of buffer in bytes"""
        import sys
        
        # Rough estimate - actual usage may vary
        if not self.buffer:
            return 0
            
        sample_size = min(10, len(self.buffer))
        sample_memory = sum(sys.getsizeof(self.buffer[i]) for i in range(sample_size))
        avg_item_size = sample_memory / sample_size
        
        return int(avg_item_size * len(self.buffer))


# Global disk manager instance
disk_manager = DiskManager()


async def setup_disk_management(config: Optional[DiskUsageConfig] = None):
    """Setup and start disk management"""
    global disk_manager
    
    if config:
        disk_manager = DiskManager(config)
        
    await disk_manager.start()
    
    
async def cleanup_disk_management():
    """Cleanup disk management"""
    global disk_manager
    
    if disk_manager:
        await disk_manager.stop()