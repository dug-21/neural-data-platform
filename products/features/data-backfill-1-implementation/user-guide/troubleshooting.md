# Troubleshooting Guide

## Overview

This guide helps diagnose and resolve common issues encountered during data backfill operations.

## Quick Diagnostics

Run the diagnostic tool to check system status:

```bash
python -m data_ingestion.backfill diagnose

# Output will show:
# ✅ AWS credentials: Valid
# ✅ S3 access: Connected
# ✅ Database: Connected (20 connections available)
# ⚠️  Redis: Connection refused
# ✅ Disk space: 1.2TB available
# ❌ Memory: Only 1.5GB available (2GB recommended)
```

## Common Issues

### Authentication and Access

#### Issue: AWS Authentication Failed

**Error Message:**
```
botocore.exceptions.NoCredentialsError: Unable to locate credentials
```

**Solutions:**

1. Check AWS profile exists:
   ```bash
   aws configure list --profile polygon-s3
   ```

2. Verify credentials:
   ```bash
   aws s3 ls s3://flatfiles/ --profile polygon-s3
   ```

3. Set profile explicitly:
   ```bash
   export AWS_PROFILE=polygon-s3
   ```

4. Use IAM role (if on EC2):
   ```bash
   # Check role
   curl http://169.254.169.254/latest/meta-data/iam/security-credentials/
   ```

#### Issue: S3 Access Denied

**Error Message:**
```
botocore.exceptions.ClientError: An error occurred (403) when calling the ListObjectsV2 operation: Access Denied
```

**Solutions:**

1. Verify bucket permissions:
   ```bash
   aws s3api get-bucket-acl --bucket flatfiles --profile polygon-s3
   ```

2. Check IAM policy:
   ```json
   {
     "Version": "2012-10-17",
     "Statement": [
       {
         "Effect": "Allow",
         "Action": [
           "s3:GetObject",
           "s3:ListBucket"
         ],
         "Resource": [
           "arn:aws:s3:::flatfiles/*",
           "arn:aws:s3:::flatfiles"
         ]
       }
     ]
   }
   ```

3. Test with AWS CLI:
   ```bash
   aws s3 cp s3://flatfiles/test-file.txt . --profile polygon-s3
   ```

### Download Issues

#### Issue: Slow Download Speed

**Symptoms:**
- Download speed < 10 MB/s
- ETA keeps increasing

**Solutions:**

1. Check network bandwidth:
   ```bash
   # Install speedtest
   pip install speedtest-cli
   speedtest-cli
   ```

2. Increase concurrent downloads:
   ```bash
   python scripts/download_polygon_s3.py \
     --max-concurrent 20 \
     --chunk-size 16777216  # 16MB chunks
   ```

3. Use S3 Transfer Acceleration:
   ```python
   # In code
   s3_client = boto3.client(
       's3',
       endpoint_url='https://flatfiles.s3-accelerate.amazonaws.com'
   )
   ```

4. Check for throttling:
   ```bash
   # Monitor S3 metrics
   aws cloudwatch get-metric-statistics \
     --namespace AWS/S3 \
     --metric-name AllRequests \
     --dimensions Name=BucketName,Value=flatfiles \
     --start-time 2023-07-24T00:00:00Z \
     --end-time 2023-07-24T23:59:59Z \
     --period 3600 \
     --statistics Sum
   ```

#### Issue: Download Interruptions

**Error Message:**
```
ConnectionResetError: [Errno 104] Connection reset by peer
```

**Solutions:**

1. Enable automatic retry:
   ```python
   from botocore.config import Config
   
   config = Config(
       retries={
           'max_attempts': 10,
           'mode': 'adaptive'
       },
       read_timeout=300,
       connect_timeout=60
   )
   ```

2. Use checkpoint system:
   ```bash
   # Resume from checkpoint
   python scripts/download_polygon_s3.py \
     --profile polygon-s3 \
     --destination /mnt/data \
     --resume
   ```

3. Implement connection pooling:
   ```python
   config = Config(
       max_pool_connections=50
   )
   ```

### Database Issues

#### Issue: Connection Pool Exhausted

**Error Message:**
```
psycopg2.pool.PoolError: connection pool exhausted
```

**Solutions:**

1. Increase pool size:
   ```bash
   export DB_POOL_MAX_SIZE=50
   ```

2. Check active connections:
   ```sql
   SELECT count(*), state 
   FROM pg_stat_activity 
   GROUP BY state;
   ```

3. Kill idle connections:
   ```sql
   SELECT pg_terminate_backend(pid) 
   FROM pg_stat_activity 
   WHERE state = 'idle' 
     AND state_change < current_timestamp - interval '10 minutes';
   ```

4. Optimize connection usage:
   ```python
   # Use connection context manager
   async with db_pool.acquire() as conn:
       await conn.execute(query)
   # Connection automatically returned to pool
   ```

#### Issue: Slow Insert Performance

**Symptoms:**
- Processing rate < 1000 records/sec
- Database CPU at 100%

**Solutions:**

1. Use COPY instead of INSERT:
   ```python
   # Fast bulk insert
   await conn.copy_records_to_table(
       'market_data',
       records=batch,
       columns=['time', 'symbol', 'open', 'high', 'low', 'close', 'volume']
   )
   ```

2. Disable indexes during bulk load:
   ```sql
   -- Before backfill
   ALTER TABLE market_data DISABLE TRIGGER ALL;
   DROP INDEX idx_market_data_symbol_time;
   
   -- After backfill
   CREATE INDEX CONCURRENTLY idx_market_data_symbol_time ON market_data(symbol, time);
   ALTER TABLE market_data ENABLE TRIGGER ALL;
   ```

3. Tune PostgreSQL settings:
   ```sql
   -- Temporary settings for bulk load
   SET maintenance_work_mem = '2GB';
   SET work_mem = '256MB';
   SET synchronous_commit = 'off';
   SET checkpoint_completion_target = 0.9;
   ```

4. Use unlogged tables for staging:
   ```sql
   -- Create unlogged staging table
   CREATE UNLOGGED TABLE market_data_staging (LIKE market_data);
   
   -- After load, convert to logged
   ALTER TABLE market_data_staging SET LOGGED;
   ```

### Memory Issues

#### Issue: Out of Memory Errors

**Error Message:**
```
MemoryError: Unable to allocate array
```

**Solutions:**

1. Reduce batch size:
   ```bash
   python -m data_ingestion.backfill file \
     --batch-size 5000 \
     --memory-limit 1024
   ```

2. Enable memory profiling:
   ```python
   import tracemalloc
   tracemalloc.start()
   
   # Your code here
   
   current, peak = tracemalloc.get_traced_memory()
   print(f"Peak memory: {peak / 1024 / 1024:.2f} MB")
   ```

3. Use streaming processing:
   ```python
   # Don't load entire file
   for chunk in pd.read_csv(file_path, chunksize=10000):
       process_chunk(chunk)
   ```

4. Force garbage collection:
   ```python
   import gc
   
   # After processing large batch
   gc.collect()
   ```

### Data Quality Issues

#### Issue: High Bad Record Percentage

**Error Message:**
```
ValueError: Bad record percentage (2.5%) exceeds maximum allowed (1.0%)
```

**Solutions:**

1. Inspect bad records:
   ```bash
   # Extract sample of problematic file
   zcat problem_file.csv.gz | head -n 1000 > sample.csv
   
   # Check for encoding issues
   file -i sample.csv
   
   # Look for malformed rows
   awk -F',' 'NF != 7' sample.csv
   ```

2. Relax validation temporarily:
   ```python
   provider = FileProvider(
       base_path="/mnt/data",
       max_bad_record_percentage=0.05  # Allow 5%
   )
   ```

3. Skip problematic files:
   ```bash
   python -m data_ingestion.backfill file \
     --skip-errors \
     --error-log failed_files.txt
   ```

4. Clean data before import:
   ```python
   def clean_record(row):
       # Remove null bytes
       row = {k: v.replace('\x00', '') if isinstance(v, str) else v 
              for k, v in row.items()}
       
       # Fix timestamps
       if row['timestamp']:
           row['timestamp'] = pd.to_datetime(row['timestamp'], errors='coerce')
       
       return row
   ```

#### Issue: OHLC Validation Failures

**Error Message:**
```
ValidationError: OHLC consistency check failed: high (100.5) < low (101.0)
```

**Solutions:**

1. Identify problematic records:
   ```sql
   SELECT * FROM market_data
   WHERE high < low 
      OR high < open 
      OR high < close
      OR low > open
      OR low > close;
   ```

2. Fix with tolerance:
   ```python
   def fix_ohlc(row):
       # Ensure high is highest
       row['high'] = max(row['open'], row['high'], row['low'], row['close'])
       
       # Ensure low is lowest
       row['low'] = min(row['open'], row['high'], row['low'], row['close'])
       
       return row
   ```

3. Report to data provider:
   ```bash
   # Generate report of bad data
   python -m data_ingestion.backfill validate \
     --report bad_ohlc_report.csv \
     --export-bad-records
   ```

### Performance Issues

#### Issue: Processing Rate Declining

**Symptoms:**
- Initial rate: 15,000 records/sec
- Current rate: 2,000 records/sec

**Solutions:**

1. Check for memory leaks:
   ```python
   import psutil
   
   process = psutil.Process()
   print(f"Memory: {process.memory_info().rss / 1024 / 1024:.2f} MB")
   print(f"Open files: {len(process.open_files())}")
   ```

2. Monitor database performance:
   ```sql
   -- Check for blocking queries
   SELECT pid, usename, query, state, wait_event_type, wait_event
   FROM pg_stat_activity
   WHERE state != 'idle'
   ORDER BY query_start;
   
   -- Check table bloat
   SELECT schemaname, tablename, 
          pg_size_pretty(pg_total_relation_size(schemaname||'.'||tablename)) as size
   FROM pg_tables
   WHERE tablename = 'market_data';
   ```

3. Clear caches periodically:
   ```python
   # Clear internal caches
   if records_processed % 1000000 == 0:
       provider._checkpoints.clear()
       gc.collect()
   ```

4. Restart workers periodically:
   ```python
   # Worker recycling
   if records_processed % 10000000 == 0:
       await restart_workers()
   ```

### Checkpoint Issues

#### Issue: Corrupt Checkpoint File

**Error Message:**
```
pickle.UnpicklingError: invalid load key
```

**Solutions:**

1. Remove corrupt checkpoint:
   ```bash
   # Backup first
   cp ~/.neural_trader/checkpoints/backfill.pkl ~/.neural_trader/checkpoints/backfill.pkl.bak
   
   # Remove
   rm ~/.neural_trader/checkpoints/backfill.pkl
   ```

2. Use Redis for checkpoints:
   ```python
   # More reliable than file-based
   handler = FileBackfillHandler(
       checkpoint_backend='redis',
       redis_url='redis://localhost:6379/0'
   )
   ```

3. Validate checkpoints:
   ```python
   def validate_checkpoint(checkpoint_file):
       try:
           with open(checkpoint_file, 'rb') as f:
               data = pickle.load(f)
           return True
       except:
           return False
   ```

## Advanced Diagnostics

### System Resource Analysis

```bash
# Complete system check
cat > check_system.sh << 'EOF'
#!/bin/bash
echo "=== System Resources ==="
echo "CPU Cores: $(nproc)"
echo "Memory: $(free -h | grep Mem | awk '{print $2}')"
echo "Disk Space:"
df -h | grep -E "^/dev/"

echo -e "\n=== Database Status ==="
psql -U $DB_USER -d $DB_NAME -c "SELECT version();"
psql -U $DB_USER -d $DB_NAME -c "SELECT count(*) as connections FROM pg_stat_activity;"

echo -e "\n=== Network Connectivity ==="
ping -c 3 s3.amazonaws.com
curl -s -o /dev/null -w "S3 Response Time: %{time_total}s\n" https://flatfiles.s3.amazonaws.com/

echo -e "\n=== Process Status ==="
ps aux | grep -E "backfill|python" | grep -v grep
EOF

chmod +x check_system.sh
./check_system.sh
```

### Debug Mode

Enable comprehensive debugging:

```bash
# Set debug environment
export PYTHONDEBUG=1
export LOG_LEVEL=DEBUG
export BACKFILL_DEBUG=true

# Run with debug flags
python -m data_ingestion.backfill file \
  --path /mnt/data \
  --debug \
  --trace-memory \
  --profile-cpu \
  --slow-query-log \
  2>&1 | tee debug.log
```

### Performance Profiling

```python
# CPU profiling
import cProfile
import pstats

profiler = cProfile.Profile()
profiler.enable()

# Run backfill
await backfill_handler.run()

profiler.disable()
stats = pstats.Stats(profiler)
stats.sort_stats('cumulative')
stats.print_stats(50)
```

## Getting Support

If issues persist:

1. **Collect Diagnostic Information:**
   ```bash
   python -m data_ingestion.backfill diagnose --export diagnostics.json
   ```

2. **Create Debug Bundle:**
   ```bash
   tar -czf debug_bundle.tar.gz \
     diagnostics.json \
     ~/.neural_trader/logs/ \
     ~/.neural_trader/checkpoints/ \
     /var/log/postgresql/
   ```

3. **Report Issue:**
   - GitHub Issues: Include debug bundle
   - Stack trace: Full error output
   - Configuration: Sanitized config file
   - System info: OS, Python version, etc.

## Prevention Tips

1. **Regular Health Checks:**
   ```bash
   # Add to crontab
   0 * * * * /usr/bin/python -m data_ingestion.backfill diagnose --alert-on-issues
   ```

2. **Monitor Key Metrics:**
   - Processing rate
   - Error rate
   - Memory usage
   - Disk space
   - Database connections

3. **Implement Alerts:**
   ```yaml
   alerts:
     - name: high_error_rate
       condition: error_rate > 0.02
       action: email
     
     - name: low_disk_space
       condition: disk_free_gb < 100
       action: pause_backfill
   ```

4. **Regular Maintenance:**
   - Vacuum database tables
   - Clean old checkpoints
   - Rotate logs
   - Update dependencies