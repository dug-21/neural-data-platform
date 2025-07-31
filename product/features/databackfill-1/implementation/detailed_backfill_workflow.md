# Detailed Step-by-Step Backfill Workflow

## Pre-Execution Phase

### Step 1: Environment Setup and Validation
1. **Verify Python environment** (Python 3.8+)
2. **Check dependencies**: asyncio, aiohttp, aioboto3, asyncpg, pandas, psutil
3. **Validate AWS credentials** for S3 access
4. **Test database connectivity** to PostgreSQL
5. **Create working directories**:
   - `/tmp/backfill_downloads` for temporary files
   - `.backfill_checkpoints` for checkpoint files
   - `logs/` for operation logs

### Step 2: Configuration Initialization
1. **Load configuration** from `config.py`:
   - S3 bucket details and access keys
   - Database connection string (DSN)
   - Concurrent download limits (default: 10)
   - Batch size for inserts (default: 50,000)
   - Retry parameters (max_retries: 3, backoff_factor: 2.0)
2. **Initialize logging** with appropriate levels
3. **Set up alert notifications** (email/Slack webhooks)

### Step 3: Job Initialization
1. **Generate unique job ID**: `backfill_YYYYMMDD_HHMMSS`
2. **Parse input parameters**:
   - List of symbols to backfill
   - Start date (YYYY-MM-DD format)
   - End date (YYYY-MM-DD format)
3. **Check for existing checkpoints**:
   - If resuming: Load checkpoint and create recovery plan
   - If new: Create fresh BackfillProgress object

## Initialization Phase

### Step 4: Component Initialization
1. **Initialize Database Pool**:
   ```
   - Create connection pool (min: 10, max: 20 connections)
   - Set command timeout: 60 seconds
   - Test connection with sample query
   ```

2. **Initialize Progress Tracker**:
   ```
   - Create ProgressTracker instance
   - Set auto-save interval: 60 seconds
   - Start auto-save background task
   ```

3. **Initialize Error Handler**:
   ```
   - Create ErrorHandler with alert callback
   - Set up circuit breakers for S3 and database operations
   - Configure retry strategies per error category
   ```

4. **Start Monitoring Services**:
   ```
   - Launch ProgressMonitor (update interval: 5 seconds)
   - Start web dashboard on port 8080
   - Initialize metrics collection
   ```

### Step 5: Progress Tracking Setup
1. **Create/Resume job** in progress tracker
2. **For each symbol**, create SymbolProgress entry:
   - Status: PENDING
   - Calculate total trading days between start and end dates
   - Estimate number of files to download
3. **Save initial checkpoint**

## Discovery Phase

### Step 6: File Discovery and Planning
For each symbol in parallel (using semaphore for rate limiting):

1. **Generate S3 keys** for date range:
   ```
   Pattern: s3://bucket/symbol/YYYY/MM/DD/data.csv.gz
   ```

2. **Verify file existence** (HEAD requests to S3)
3. **Create DownloadTask objects**:
   - Symbol, date, S3 key, local path
   - Set retry_count: 0, max_retries: 3

4. **Update symbol progress**:
   - files_total: Number of files found
   - Status: DOWNLOADING

5. **Save checkpoint** after discovery

## Download Phase

### Step 7: Concurrent Download Execution
1. **Initialize ConcurrentDownloader**:
   - Max concurrent downloads: 10
   - Create aioboto3 session

2. **For each batch of files** (grouped by symbol):
   
   **CHECKPOINT: Before Download Batch**
   - Update symbol status: DOWNLOADING
   - Record current_date for symbol
   - Save checkpoint
   
   a. **Acquire semaphore** for rate limiting
   
   b. **Download file from S3**:
   ```
   - Use aioboto3 to stream download
   - Save to temporary location: /tmp/backfill_downloads/
   - Verify file integrity (size check)
   ```
   
   c. **Handle download errors**:
   ```
   - Network errors: Exponential backoff retry
   - Rate limits: Wait 60 seconds, then retry
   - Auth errors: Stop processing, alert
   ```
   
   d. **Update progress**:
   ```
   - Increment files_downloaded
   - Record download metrics (speed, size)
   ```

3. **Circuit breaker monitoring**:
   - If failure threshold reached (10 failures), open circuit
   - Wait recovery timeout (120 seconds) before retry

**CHECKPOINT: After Download Batch**
- Update files_downloaded count
- Save download metrics
- Save checkpoint

## Processing Phase

### Step 8: Parallel Batch Processing
1. **Initialize BatchProcessor**:
   - Batch size: 10,000 records
   - Max workers: 4 (ProcessPoolExecutor)

2. **For downloaded files** (process in parallel):
   
   **CHECKPOINT: Before Processing**
   - Update symbol status: PROCESSING
   - Save checkpoint
   
   a. **Parse compressed file**:
   ```python
   - Decompress .gz file
   - Read CSV with pandas
   - Parse timestamps to datetime
   - Convert price fields to Decimal
   ```
   
   b. **Validate data quality**:
   ```
   - Check required columns exist
   - Validate timestamp ranges
   - Ensure prices are positive
   - Verify volume is non-negative
   ```
   
   c. **Handle parsing errors**:
   ```
   - Log corrupted files
   - Mark in error list (non-retryable)
   - Continue with next file
   ```
   
   d. **Yield batches** of 10,000 records

3. **Update progress**:
   - Increment files_processed
   - Add to records_processed count

**CHECKPOINT: After Processing**
- Update processing metrics
- Save checkpoint

## Database Insertion Phase

### Step 9: Bulk Database Insertion
1. **Initialize BulkInserter**:
   - Batch size: 50,000 records
   - Use connection pool

2. **For each batch of records**:
   
   **CHECKPOINT: Before Insert**
   - Update symbol status: INSERTING
   - Save checkpoint
   
   a. **Prepare data for COPY**:
   ```python
   - Convert records to tuples
   - Order: (symbol, timestamp, open, high, low, close, volume, vwap, transactions)
   ```
   
   b. **Execute bulk insert**:
   ```sql
   - Use PostgreSQL COPY protocol
   - Handle duplicates with ON CONFLICT
   - Update existing records if needed
   ```
   
   c. **Handle database errors**:
   ```
   - Connection errors: Reset pool, retry
   - Deadlocks: Wait 1 second, retry
   - Constraint violations: Log and skip
   ```
   
   d. **Update progress**:
   ```
   - Increment records_inserted
   - Calculate insert rate
   ```

3. **Transaction management**:
   - Commit every 50,000 records
   - Rollback on critical errors

**CHECKPOINT: After Insert Batch**
- Update insertion metrics
- Save checkpoint with exact record count

## Verification Phase

### Step 10: Data Verification
1. **For each completed symbol**:
   
   a. **Query database** for record count:
   ```sql
   SELECT COUNT(*) FROM market_data 
   WHERE symbol = ? AND timestamp BETWEEN ? AND ?
   ```
   
   b. **Compare counts**:
   - Expected: trading days × records per day
   - Actual: query result
   - Calculate coverage percentage
   
   c. **Identify gaps**:
   ```sql
   - Find missing dates
   - Log any data gaps
   ```

2. **Update symbol status**:
   - If coverage > 95%: COMPLETED
   - If coverage < 95%: FAILED (needs retry)

**CHECKPOINT: After Verification**
- Update final status
- Record verification results
- Save checkpoint

## Completion Phase

### Step 11: Cleanup and Reporting
1. **Clean temporary files**:
   ```
   - Delete downloaded files from /tmp/backfill_downloads/
   - Keep files < 1 hour old (might be in use)
   ```

2. **Generate final report**:
   ```
   - Total runtime
   - Records processed/inserted
   - Average rates (download/insert)
   - Error summary by category
   - Symbol completion status
   ```

3. **Update global statistics**:
   ```
   - Total data volume processed
   - Peak performance metrics
   - Resource utilization stats
   ```

4. **Send completion notification**:
   - Email/Slack with summary
   - Link to detailed dashboard

### Step 12: Graceful Shutdown
1. **Stop monitoring tasks**:
   - Cancel auto-save task
   - Close websocket connections
   - Stop metrics collection

2. **Close resources**:
   - Drain database connection pool
   - Close S3 client sessions
   - Flush remaining logs

3. **Save final checkpoint**:
   - Mark job as COMPLETED
   - Record end timestamp
   - Archive checkpoint file

## Error Recovery Workflow

### Recovery Step 1: Load Checkpoint
1. **Read checkpoint file** for job_id
2. **Deserialize BackfillProgress object**
3. **Analyze incomplete work**:
   - Symbols in DOWNLOADING state
   - Symbols in PROCESSING state
   - Symbols in FAILED state

### Recovery Step 2: Create Recovery Plan
1. **For DOWNLOADING symbols**:
   - Resume from current_date
   - Re-download last date (might be partial)

2. **For PROCESSING symbols**:
   - Reprocess last date
   - Verify temporary files exist

3. **For FAILED symbols**:
   - Analyze error history
   - Determine if retryable
   - Reset to PENDING if retrying

### Recovery Step 3: Execute Recovery
1. **Re-initialize components** with existing state
2. **Skip already completed work**
3. **Resume from last checkpoint**
4. **Continue normal workflow**

## Monitoring During Execution

### Real-time Metrics Dashboard (http://localhost:8080)
- **Overall Progress**: Percentage complete, ETA
- **Symbol Status**: Live status for each symbol
- **Performance Metrics**:
  - Download speed (MB/s)
  - Processing rate (records/sec)
  - Insert rate (records/sec)
- **System Resources**:
  - CPU usage
  - Memory usage
  - Network I/O
  - Disk usage
- **Error Summary**: Errors by category with counts

### Checkpoint File Structure
```json
{
  "job_id": "backfill_20240315_143022",
  "started_at": "2024-03-15T14:30:22Z",
  "updated_at": "2024-03-15T15:45:33Z",
  "symbols": {
    "AAPL": {
      "status": "PROCESSING",
      "start_date": "2023-01-01",
      "end_date": "2023-12-31",
      "current_date": "2023-06-15",
      "files_total": 252,
      "files_downloaded": 134,
      "files_processed": 130,
      "records_processed": 1950000,
      "records_inserted": 1950000,
      "errors": [],
      "last_checkpoint": "2024-03-15T15:45:33Z"
    }
  },
  "global_stats": {
    "total_records": 5850000,
    "download_speed_mbps": 25.4,
    "insert_rate_per_sec": 45000
  }
}
```

## Critical Success Factors

1. **Checkpoint Frequency**: Save every 60 seconds and after major operations
2. **Error Categorization**: Properly classify errors for appropriate handling
3. **Resource Management**: Monitor and limit concurrent operations
4. **Data Validation**: Verify data quality before insertion
5. **Progress Visibility**: Real-time dashboard for monitoring
6. **Graceful Degradation**: Continue processing other symbols if one fails
7. **Atomic Operations**: Ensure checkpoint saves are atomic (temp file + rename)

This workflow ensures reliable, resumable, and monitorable historical data backfill operations.