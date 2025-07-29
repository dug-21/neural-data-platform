# Progress Tracking and Checkpoint System

## Overview
Robust progress tracking and checkpoint system for resumable backfill operations.

## Core Components

### 1. Progress Tracker
```python
import json
import asyncio
from typing import Dict, List, Optional, Any
from datetime import datetime, timedelta
from dataclasses import dataclass, asdict, field
from pathlib import Path
import aiofiles
from enum import Enum

class BackfillStatus(Enum):
    PENDING = "pending"
    DOWNLOADING = "downloading"
    PROCESSING = "processing"
    INSERTING = "inserting"
    COMPLETED = "completed"
    FAILED = "failed"
    PAUSED = "paused"

@dataclass
class SymbolProgress:
    symbol: str
    status: BackfillStatus
    start_date: datetime
    end_date: datetime
    current_date: Optional[datetime] = None
    files_total: int = 0
    files_downloaded: int = 0
    files_processed: int = 0
    records_processed: int = 0
    records_inserted: int = 0
    errors: List[Dict[str, Any]] = field(default_factory=list)
    last_checkpoint: Optional[datetime] = None
    
    @property
    def progress_percentage(self) -> float:
        if self.files_total == 0:
            return 0.0
        return (self.files_processed / self.files_total) * 100
        
    @property
    def days_remaining(self) -> int:
        if not self.current_date:
            return (self.end_date - self.start_date).days
        return (self.end_date - self.current_date).days

@dataclass
class BackfillProgress:
    job_id: str
    started_at: datetime
    updated_at: datetime
    symbols: Dict[str, SymbolProgress] = field(default_factory=dict)
    global_stats: Dict[str, Any] = field(default_factory=dict)
    
    def add_symbol(self, symbol: str, start_date: datetime, end_date: datetime):
        self.symbols[symbol] = SymbolProgress(
            symbol=symbol,
            status=BackfillStatus.PENDING,
            start_date=start_date,
            end_date=end_date
        )
        
    def get_overall_progress(self) -> Dict[str, Any]:
        total_files = sum(s.files_total for s in self.symbols.values())
        processed_files = sum(s.files_processed for s in self.symbols.values())
        
        return {
            'total_symbols': len(self.symbols),
            'completed_symbols': len([s for s in self.symbols.values() if s.status == BackfillStatus.COMPLETED]),
            'failed_symbols': len([s for s in self.symbols.values() if s.status == BackfillStatus.FAILED]),
            'total_files': total_files,
            'processed_files': processed_files,
            'overall_progress': (processed_files / total_files * 100) if total_files > 0 else 0,
            'total_records': sum(s.records_processed for s in self.symbols.values()),
            'total_inserted': sum(s.records_inserted for s in self.symbols.values())
        }

class ProgressTracker:
    """Main progress tracking system with persistence"""
    
    def __init__(
        self,
        checkpoint_dir: Path = Path(".backfill_checkpoints"),
        auto_save_interval: int = 60  # seconds
    ):
        self.checkpoint_dir = checkpoint_dir
        self.checkpoint_dir.mkdir(exist_ok=True)
        self.auto_save_interval = auto_save_interval
        self.current_progress: Optional[BackfillProgress] = None
        self._save_task: Optional[asyncio.Task] = None
        self._lock = asyncio.Lock()
        
    async def create_job(
        self,
        job_id: str,
        symbols: List[str],
        start_date: datetime,
        end_date: datetime
    ) -> BackfillProgress:
        """Create new backfill job"""
        self.current_progress = BackfillProgress(
            job_id=job_id,
            started_at=datetime.utcnow(),
            updated_at=datetime.utcnow()
        )
        
        for symbol in symbols:
            self.current_progress.add_symbol(symbol, start_date, end_date)
            
        await self.save_checkpoint()
        
        # Start auto-save task
        self._save_task = asyncio.create_task(self._auto_save_loop())
        
        return self.current_progress
        
    async def resume_job(self, job_id: str) -> Optional[BackfillProgress]:
        """Resume existing job from checkpoint"""
        checkpoint_file = self.checkpoint_dir / f"{job_id}.json"
        
        if not checkpoint_file.exists():
            return None
            
        async with aiofiles.open(checkpoint_file, 'r') as f:
            data = json.loads(await f.read())
            
        # Reconstruct progress object
        self.current_progress = self._deserialize_progress(data)
        
        # Start auto-save task
        self._save_task = asyncio.create_task(self._auto_save_loop())
        
        return self.current_progress
        
    async def update_symbol_progress(
        self,
        symbol: str,
        **updates
    ):
        """Update progress for specific symbol"""
        async with self._lock:
            if symbol in self.current_progress.symbols:
                progress = self.current_progress.symbols[symbol]
                
                for key, value in updates.items():
                    if hasattr(progress, key):
                        setattr(progress, key, value)
                        
                progress.last_checkpoint = datetime.utcnow()
                self.current_progress.updated_at = datetime.utcnow()
                
    async def save_checkpoint(self):
        """Save current progress to disk"""
        if not self.current_progress:
            return
            
        async with self._lock:
            checkpoint_file = self.checkpoint_dir / f"{self.current_progress.job_id}.json"
            
            # Serialize progress
            data = self._serialize_progress(self.current_progress)
            
            # Write atomically
            temp_file = checkpoint_file.with_suffix('.tmp')
            async with aiofiles.open(temp_file, 'w') as f:
                await f.write(json.dumps(data, indent=2))
                
            # Rename atomically
            temp_file.replace(checkpoint_file)
            
    async def _auto_save_loop(self):
        """Auto-save checkpoint periodically"""
        while True:
            try:
                await asyncio.sleep(self.auto_save_interval)
                await self.save_checkpoint()
            except asyncio.CancelledError:
                break
            except Exception as e:
                logging.error(f"Auto-save failed: {e}")
                
    def _serialize_progress(self, progress: BackfillProgress) -> Dict[str, Any]:
        """Serialize progress object to JSON-compatible format"""
        data = asdict(progress)
        
        # Convert datetime objects
        data['started_at'] = progress.started_at.isoformat()
        data['updated_at'] = progress.updated_at.isoformat()
        
        # Convert symbol progress
        for symbol, sym_progress in data['symbols'].items():
            sym_progress['status'] = sym_progress['status'].value
            sym_progress['start_date'] = sym_progress['start_date'].isoformat()
            sym_progress['end_date'] = sym_progress['end_date'].isoformat()
            if sym_progress['current_date']:
                sym_progress['current_date'] = sym_progress['current_date'].isoformat()
            if sym_progress['last_checkpoint']:
                sym_progress['last_checkpoint'] = sym_progress['last_checkpoint'].isoformat()
                
        return data
        
    def _deserialize_progress(self, data: Dict[str, Any]) -> BackfillProgress:
        """Deserialize JSON data to progress object"""
        # Convert datetime strings
        data['started_at'] = datetime.fromisoformat(data['started_at'])
        data['updated_at'] = datetime.fromisoformat(data['updated_at'])
        
        # Convert symbol progress
        symbols = {}
        for symbol, sym_data in data['symbols'].items():
            sym_data['status'] = BackfillStatus(sym_data['status'])
            sym_data['start_date'] = datetime.fromisoformat(sym_data['start_date'])
            sym_data['end_date'] = datetime.fromisoformat(sym_data['end_date'])
            if sym_data['current_date']:
                sym_data['current_date'] = datetime.fromisoformat(sym_data['current_date'])
            if sym_data['last_checkpoint']:
                sym_data['last_checkpoint'] = datetime.fromisoformat(sym_data['last_checkpoint'])
                
            symbols[symbol] = SymbolProgress(**sym_data)
            
        return BackfillProgress(
            job_id=data['job_id'],
            started_at=data['started_at'],
            updated_at=data['updated_at'],
            symbols=symbols,
            global_stats=data.get('global_stats', {})
        )
```

### 2. Real-time Progress Monitor
```python
import asyncio
from typing import Optional, Callable, Dict, Any
from datetime import datetime, timedelta
import psutil
import aiohttp

class ProgressMonitor:
    """Real-time monitoring with web dashboard support"""
    
    def __init__(
        self,
        tracker: ProgressTracker,
        update_interval: int = 5  # seconds
    ):
        self.tracker = tracker
        self.update_interval = update_interval
        self.metrics_history: List[Dict[str, Any]] = []
        self.websocket_clients: List[aiohttp.web.WebSocketResponse] = []
        
    async def start_monitoring(self):
        """Start real-time monitoring"""
        asyncio.create_task(self._monitor_loop())
        
    async def _monitor_loop(self):
        """Main monitoring loop"""
        while True:
            try:
                metrics = await self._collect_metrics()
                self.metrics_history.append(metrics)
                
                # Keep only last hour of metrics
                cutoff = datetime.utcnow() - timedelta(hours=1)
                self.metrics_history = [
                    m for m in self.metrics_history 
                    if m['timestamp'] > cutoff
                ]
                
                # Broadcast to websocket clients
                await self._broadcast_metrics(metrics)
                
                await asyncio.sleep(self.update_interval)
                
            except Exception as e:
                logging.error(f"Monitoring error: {e}")
                
    async def _collect_metrics(self) -> Dict[str, Any]:
        """Collect current metrics"""
        if not self.tracker.current_progress:
            return {}
            
        # System metrics
        cpu_percent = psutil.cpu_percent(interval=1)
        memory = psutil.virtual_memory()
        disk = psutil.disk_usage('/')
        
        # Network I/O
        net_io = psutil.net_io_counters()
        
        # Progress metrics
        progress = self.tracker.current_progress
        overall = progress.get_overall_progress()
        
        # Calculate rates
        rates = self._calculate_rates()
        
        return {
            'timestamp': datetime.utcnow(),
            'system': {
                'cpu_percent': cpu_percent,
                'memory_percent': memory.percent,
                'memory_used_gb': memory.used / (1024**3),
                'disk_percent': disk.percent,
                'network_recv_mbps': net_io.bytes_recv / (1024**2),
                'network_sent_mbps': net_io.bytes_sent / (1024**2)
            },
            'progress': overall,
            'rates': rates,
            'active_symbols': [
                {
                    'symbol': s.symbol,
                    'status': s.status.value,
                    'progress': s.progress_percentage,
                    'current_date': s.current_date.isoformat() if s.current_date else None
                }
                for s in progress.symbols.values()
                if s.status in [BackfillStatus.DOWNLOADING, BackfillStatus.PROCESSING]
            ]
        }
        
    def _calculate_rates(self) -> Dict[str, float]:
        """Calculate processing rates from history"""
        if len(self.metrics_history) < 2:
            return {
                'download_mbps': 0,
                'records_per_second': 0,
                'files_per_minute': 0
            }
            
        # Get metrics from 1 minute ago
        one_min_ago = datetime.utcnow() - timedelta(minutes=1)
        old_metrics = next(
            (m for m in reversed(self.metrics_history) if m['timestamp'] <= one_min_ago),
            self.metrics_history[0]
        )
        
        current = self.metrics_history[-1]
        time_diff = (current['timestamp'] - old_metrics['timestamp']).total_seconds()
        
        if time_diff == 0:
            return {
                'download_mbps': 0,
                'records_per_second': 0,
                'files_per_minute': 0
            }
            
        return {
            'download_mbps': (
                current['system']['network_recv_mbps'] - 
                old_metrics['system']['network_recv_mbps']
            ) / time_diff,
            'records_per_second': (
                current['progress']['total_records'] - 
                old_metrics['progress']['total_records']
            ) / time_diff,
            'files_per_minute': (
                current['progress']['processed_files'] - 
                old_metrics['progress']['processed_files']
            ) * 60 / time_diff
        }
```

### 3. Progress Dashboard
```python
from aiohttp import web
import aiohttp_jinja2
import jinja2

class ProgressDashboard:
    """Web-based progress dashboard"""
    
    def __init__(self, monitor: ProgressMonitor, port: int = 8080):
        self.monitor = monitor
        self.port = port
        self.app = web.Application()
        self._setup_routes()
        
    def _setup_routes(self):
        """Setup web routes"""
        self.app.router.add_get('/', self.index)
        self.app.router.add_get('/api/metrics', self.get_metrics)
        self.app.router.add_get('/api/progress/{symbol}', self.get_symbol_progress)
        self.app.router.add_get('/ws', self.websocket_handler)
        self.app.router.add_static('/static', 'static')
        
        # Setup Jinja2 templates
        aiohttp_jinja2.setup(
            self.app,
            loader=jinja2.FileSystemLoader('templates')
        )
        
    async def index(self, request):
        """Render dashboard page"""
        context = {
            'job_id': self.monitor.tracker.current_progress.job_id if self.monitor.tracker.current_progress else None,
            'started_at': self.monitor.tracker.current_progress.started_at if self.monitor.tracker.current_progress else None
        }
        return aiohttp_jinja2.render_template('dashboard.html', request, context)
        
    async def get_metrics(self, request):
        """Get current metrics as JSON"""
        if self.monitor.metrics_history:
            return web.json_response(self.monitor.metrics_history[-1])
        return web.json_response({})
        
    async def get_symbol_progress(self, request):
        """Get progress for specific symbol"""
        symbol = request.match_info['symbol']
        
        if self.monitor.tracker.current_progress and symbol in self.monitor.tracker.current_progress.symbols:
            progress = self.monitor.tracker.current_progress.symbols[symbol]
            return web.json_response({
                'symbol': progress.symbol,
                'status': progress.status.value,
                'progress_percentage': progress.progress_percentage,
                'files_processed': progress.files_processed,
                'files_total': progress.files_total,
                'records_processed': progress.records_processed,
                'errors': progress.errors
            })
            
        return web.json_response({'error': 'Symbol not found'}, status=404)
        
    async def websocket_handler(self, request):
        """WebSocket for real-time updates"""
        ws = web.WebSocketResponse()
        await ws.prepare(request)
        
        self.monitor.websocket_clients.append(ws)
        
        try:
            async for msg in ws:
                if msg.type == aiohttp.WSMsgType.TEXT:
                    if msg.data == 'close':
                        await ws.close()
                elif msg.type == aiohttp.WSMsgType.ERROR:
                    logging.error(f'WebSocket error: {ws.exception()}')
                    
        finally:
            self.monitor.websocket_clients.remove(ws)
            
        return ws
        
    async def start(self):
        """Start dashboard server"""
        runner = web.AppRunner(self.app)
        await runner.setup()
        site = web.TCPSite(runner, 'localhost', self.port)
        await site.start()
        logging.info(f"Dashboard running at http://localhost:{self.port}")
```

### 4. Recovery Manager
```python
class RecoveryManager:
    """Handle recovery from failures and interruptions"""
    
    def __init__(
        self,
        tracker: ProgressTracker,
        downloader: ConcurrentDownloadManager,
        processor: ParallelBatchProcessor
    ):
        self.tracker = tracker
        self.downloader = downloader
        self.processor = processor
        
    async def recover_job(self, job_id: str) -> Optional[BackfillProgress]:
        """Recover and resume interrupted job"""
        # Load checkpoint
        progress = await self.tracker.resume_job(job_id)
        
        if not progress:
            logging.error(f"No checkpoint found for job {job_id}")
            return None
            
        logging.info(f"Resuming job {job_id} from checkpoint")
        
        # Analyze what needs to be redone
        recovery_plan = self._create_recovery_plan(progress)
        
        # Execute recovery
        await self._execute_recovery(recovery_plan)
        
        return progress
        
    def _create_recovery_plan(self, progress: BackfillProgress) -> Dict[str, Any]:
        """Create recovery plan based on checkpoint"""
        plan = {
            'resume_downloads': [],
            'resume_processing': [],
            'verify_inserts': [],
            'retry_failed': []
        }
        
        for symbol, sym_progress in progress.symbols.items():
            if sym_progress.status == BackfillStatus.DOWNLOADING:
                # Resume download from current date
                plan['resume_downloads'].append({
                    'symbol': symbol,
                    'start_date': sym_progress.current_date or sym_progress.start_date,
                    'end_date': sym_progress.end_date
                })
                
            elif sym_progress.status == BackfillStatus.PROCESSING:
                # Reprocess last day to ensure completeness
                if sym_progress.current_date:
                    plan['resume_processing'].append({
                        'symbol': symbol,
                        'date': sym_progress.current_date
                    })
                    
            elif sym_progress.status == BackfillStatus.FAILED:
                # Retry failed symbols
                plan['retry_failed'].append({
                    'symbol': symbol,
                    'errors': sym_progress.errors
                })
                
        return plan
        
    async def _execute_recovery(self, plan: Dict[str, Any]):
        """Execute recovery plan"""
        # Resume downloads
        for item in plan['resume_downloads']:
            logging.info(f"Resuming download for {item['symbol']} from {item['start_date']}")
            # Create download jobs and execute
            
        # Resume processing
        for item in plan['resume_processing']:
            logging.info(f"Reprocessing {item['symbol']} for {item['date']}")
            # Reprocess specific date
            
        # Retry failed
        for item in plan['retry_failed']:
            logging.info(f"Retrying failed symbol {item['symbol']}")
            # Analyze errors and retry with fixes
```

## Integration Example

```python
async def run_backfill_with_monitoring(
    symbols: List[str],
    start_date: datetime,
    end_date: datetime
):
    """Run backfill with full monitoring and recovery"""
    
    # Initialize components
    tracker = ProgressTracker()
    monitor = ProgressMonitor(tracker)
    dashboard = ProgressDashboard(monitor)
    
    # Start monitoring and dashboard
    await monitor.start_monitoring()
    await dashboard.start()
    
    # Create or resume job
    job_id = f"backfill_{datetime.utcnow().strftime('%Y%m%d_%H%M%S')}"
    progress = await tracker.create_job(job_id, symbols, start_date, end_date)
    
    # Run backfill with progress updates
    try:
        # Main backfill logic here
        pass
    except Exception as e:
        logging.error(f"Backfill failed: {e}")
        # Progress is automatically saved for recovery
        
    finally:
        # Save final checkpoint
        await tracker.save_checkpoint()
```

## Key Features

1. **Atomic Checkpoints**: Save progress atomically to prevent corruption
2. **Symbol-level Tracking**: Track each symbol independently
3. **Real-time Monitoring**: Live metrics and progress updates
4. **Web Dashboard**: Visual progress monitoring
5. **Automatic Recovery**: Resume from last known good state
6. **Performance Metrics**: Track download/processing rates
7. **Error Tracking**: Detailed error logs for debugging