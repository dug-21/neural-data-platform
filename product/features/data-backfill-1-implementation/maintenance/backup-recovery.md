# Backup and Recovery Guide

## Overview

This guide provides procedures for backing up and recovering data backfill operations, including database backups, checkpoint management, and disaster recovery scenarios.

## Backup Strategy

### Components to Backup

1. **Database (Critical)**
   - Market data tables
   - Continuous aggregates
   - System metadata

2. **Checkpoints (Important)**
   - Redis snapshots
   - File-based checkpoints
   - Operation state

3. **Configuration (Important)**
   - Application configs
   - Environment variables
   - AWS credentials

4. **Downloaded Data (Optional)**
   - S3 cached files
   - Processed archives

## Database Backup Procedures

### Automated Backups

#### Daily Full Backup
```bash
#!/bin/bash
# /opt/neural-trader/scripts/backup_daily.sh

set -e

# Configuration
BACKUP_DIR="/backup/postgresql/daily"
DB_NAME="trading"
DB_USER="postgres"
RETENTION_DAYS=7
TIMESTAMP=$(date +%Y%m%d_%H%M%S)

# Create backup directory
mkdir -p $BACKUP_DIR

# Perform backup
echo "Starting database backup at $(date)"
pg_dump -h localhost -U $DB_USER -d $DB_NAME -Fc -f "$BACKUP_DIR/trading_$TIMESTAMP.dump"

# Compress backup
gzip "$BACKUP_DIR/trading_$TIMESTAMP.dump"

# Remove old backups
find $BACKUP_DIR -name "trading_*.dump.gz" -mtime +$RETENTION_DAYS -delete

echo "Backup completed: trading_$TIMESTAMP.dump.gz"
```

#### Incremental WAL Archiving
```bash
# postgresql.conf
wal_level = replica
archive_mode = on
archive_command = 'test ! -f /backup/wal/%f && cp %p /backup/wal/%f'
```

#### Continuous Backup with pg_basebackup
```bash
# Create base backup
pg_basebackup -h localhost -U replicator -D /backup/base -Fp -Xs -P

# Schedule regular base backups (weekly)
0 2 * * 0 pg_basebackup -h localhost -U replicator -D /backup/base/$(date +\%Y\%m\%d) -Fp -Xs -P
```

### Manual Backup Procedures

#### Pre-Deployment Backup
```bash
# Full database backup before major changes
pg_dump -h localhost -U postgres -d trading -Fc -v \
  -f "/backup/pre_deployment_$(date +%Y%m%d_%H%M%S).dump"

# Backup specific tables only
pg_dump -h localhost -U postgres -d trading \
  -t market_data -t market_data_daily \
  -Fc -f "/backup/market_data_$(date +%Y%m%d).dump"
```

#### Logical Backup with Copy
```sql
-- Export to CSV for portability
COPY (
    SELECT * FROM market_data 
    WHERE time >= '2024-01-01' 
    AND time < '2024-02-01'
) TO '/backup/exports/market_data_202401.csv' 
WITH (FORMAT CSV, HEADER);

-- Compressed export
COPY (
    SELECT * FROM market_data 
    WHERE symbol = 'AAPL'
) TO PROGRAM 'gzip > /backup/exports/AAPL_full.csv.gz' 
WITH (FORMAT CSV, HEADER);
```

## Checkpoint Backup

### Redis Checkpoint Backup

#### Automated Redis Backup
```bash
#!/bin/bash
# /opt/neural-trader/scripts/backup_redis.sh

REDIS_CLI="redis-cli -a $REDIS_PASSWORD"
BACKUP_DIR="/backup/redis"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)

# Create backup directory
mkdir -p $BACKUP_DIR

# Trigger Redis save
$REDIS_CLI BGSAVE

# Wait for save to complete
while [ $($REDIS_CLI LASTSAVE) -eq $($REDIS_CLI LASTSAVE) ]; do
    sleep 1
done

# Copy RDB file
cp /var/lib/redis/dump.rdb "$BACKUP_DIR/dump_$TIMESTAMP.rdb"

# Compress
gzip "$BACKUP_DIR/dump_$TIMESTAMP.rdb"

echo "Redis backup completed: dump_$TIMESTAMP.rdb.gz"
```

#### Export Checkpoints to JSON
```python
import redis
import json
from datetime import datetime

def export_checkpoints(redis_url, output_file):
    """Export all checkpoints to JSON for portability."""
    r = redis.from_url(redis_url)
    
    checkpoints = {}
    for key in r.scan_iter("checkpoint:*"):
        value = r.get(key)
        if value:
            checkpoints[key.decode()] = json.loads(value)
    
    with open(output_file, 'w') as f:
        json.dump({
            'exported_at': datetime.utcnow().isoformat(),
            'total_checkpoints': len(checkpoints),
            'checkpoints': checkpoints
        }, f, indent=2)
    
    print(f"Exported {len(checkpoints)} checkpoints to {output_file}")
```

### File-based Checkpoint Backup

```bash
# Backup checkpoint files
tar -czf /backup/checkpoints/checkpoints_$(date +%Y%m%d).tar.gz \
  ~/.neural_trader/checkpoints/ \
  /var/lib/neural_trader/checkpoints/

# Sync to remote storage
aws s3 sync /backup/checkpoints/ s3://backup-bucket/neural-trader/checkpoints/
```

## Configuration Backup

### Application Configuration

```bash
#!/bin/bash
# Backup all configuration files

CONFIG_BACKUP="/backup/config/$(date +%Y%m%d)"
mkdir -p $CONFIG_BACKUP

# Application configs
cp -r ~/.neural_trader/ $CONFIG_BACKUP/
cp -r /etc/neural-trader/ $CONFIG_BACKUP/

# Environment files
cp ~/.neural_trader/backfill.env $CONFIG_BACKUP/
cp /etc/systemd/system/neural-trader-*.service $CONFIG_BACKUP/

# Database configs
cp /etc/postgresql/*/main/postgresql.conf $CONFIG_BACKUP/
cp /etc/postgresql/*/main/pg_hba.conf $CONFIG_BACKUP/

# Create encrypted archive
tar -czf - $CONFIG_BACKUP | gpg -c > "$CONFIG_BACKUP.tar.gz.gpg"
```

## Recovery Procedures

### Database Recovery

#### Point-in-Time Recovery (PITR)

```bash
# Stop database
sudo systemctl stop postgresql

# Restore base backup
rm -rf /var/lib/postgresql/14/main/*
tar -xzf /backup/base/base_20240720.tar.gz -C /var/lib/postgresql/14/main/

# Create recovery configuration
cat > /var/lib/postgresql/14/main/recovery.conf << EOF
restore_command = 'cp /backup/wal/%f %p'
recovery_target_time = '2024-07-23 14:30:00'
recovery_target_inclusive = true
EOF

# Start recovery
sudo systemctl start postgresql

# Monitor recovery
tail -f /var/log/postgresql/postgresql-14-main.log
```

#### Restore from Dump

```bash
# Create new database if needed
createdb -h localhost -U postgres trading_restore

# Restore from dump
pg_restore -h localhost -U postgres -d trading_restore -v \
  /backup/trading_20240723.dump

# Verify restoration
psql -h localhost -U postgres -d trading_restore -c \
  "SELECT COUNT(*) FROM market_data;"
```

#### Selective Table Restore

```sql
-- Restore specific date range
BEGIN;

-- Clear existing data for date range
DELETE FROM market_data 
WHERE time >= '2024-01-15' AND time < '2024-01-16';

-- Import from backup
\COPY market_data FROM '/backup/exports/market_data_20240115.csv' WITH CSV HEADER;

COMMIT;
```

### Checkpoint Recovery

#### Redis Recovery

```bash
# Stop Redis
sudo systemctl stop redis

# Restore RDB file
cp /backup/redis/dump_20240723.rdb.gz /var/lib/redis/
gunzip /var/lib/redis/dump_20240723.rdb.gz
mv /var/lib/redis/dump_20240723.rdb /var/lib/redis/dump.rdb

# Set permissions
chown redis:redis /var/lib/redis/dump.rdb

# Start Redis
sudo systemctl start redis

# Verify
redis-cli -a $REDIS_PASSWORD DBSIZE
```

#### Import JSON Checkpoints

```python
def import_checkpoints(redis_url, input_file):
    """Import checkpoints from JSON backup."""
    r = redis.from_url(redis_url)
    
    with open(input_file, 'r') as f:
        data = json.load(f)
    
    imported = 0
    for key, value in data['checkpoints'].items():
        r.set(key, json.dumps(value))
        imported += 1
    
    print(f"Imported {imported} checkpoints from {input_file}")
```

### Data Recovery Scenarios

#### Scenario 1: Corrupt Data for Specific Symbol

```sql
-- Identify corrupt data
SELECT date_trunc('day', time) as day, COUNT(*) as bad_records
FROM market_data
WHERE symbol = 'AAPL'
  AND (high < low OR high < open OR high < close OR low > open OR low > close)
GROUP BY day
ORDER BY day;

-- Remove corrupt data
BEGIN;

DELETE FROM market_data
WHERE symbol = 'AAPL'
  AND time >= '2024-01-15'
  AND time < '2024-01-16';

-- Re-import from backup or source
-- ... import process ...

COMMIT;
```

#### Scenario 2: Missing Data Recovery

```python
async def recover_missing_data(symbol, date, source='s3'):
    """Recover missing data for specific symbol and date."""
    
    # Check what's missing
    missing_query = """
        WITH expected_minutes AS (
            SELECT generate_series(
                $1::timestamp,
                $1::timestamp + interval '1 day' - interval '1 minute',
                interval '1 minute'
            ) AS minute
        )
        SELECT COUNT(*) 
        FROM expected_minutes e
        LEFT JOIN market_data m ON date_trunc('minute', m.time) = e.minute
            AND m.symbol = $2
        WHERE m.time IS NULL
          AND extract(dow from e.minute) NOT IN (0, 6)
          AND e.minute::time >= '09:30:00'
          AND e.minute::time <= '16:00:00'
    """
    
    # Re-download from source
    if source == 's3':
        downloader = PolygonS3Downloader(...)
        await downloader.download_specific(symbol, date)
    
    # Re-process
    handler = FileBackfillHandler(...)
    await handler.process_date(symbol, date)
```

#### Scenario 3: Full System Recovery

```bash
#!/bin/bash
# Full system recovery script

set -e

echo "Starting full system recovery..."

# 1. Restore database
echo "Restoring database..."
pg_restore -h localhost -U postgres -d trading_new -v \
  /backup/trading_latest.dump

# 2. Restore Redis
echo "Restoring Redis checkpoints..."
sudo systemctl stop redis
cp /backup/redis/dump_latest.rdb /var/lib/redis/dump.rdb
sudo systemctl start redis

# 3. Restore configuration
echo "Restoring configuration..."
tar -xzf /backup/config/config_latest.tar.gz -C /

# 4. Validate restoration
echo "Validating restoration..."
python -m data_ingestion.backfill validate \
  --symbols AAPL,MSFT \
  --start-date 2024-01-01 \
  --end-date 2024-01-31

echo "Recovery completed!"
```

## Disaster Recovery Plan

### RTO and RPO Targets

- **Recovery Time Objective (RTO)**: 4 hours
- **Recovery Point Objective (RPO)**: 1 hour

### DR Procedures

#### 1. Database Failure

```bash
# Failover to standby
pg_ctl promote -D /var/lib/postgresql/14/standby/

# Update connection strings
sed -i 's/primary-db/standby-db/g' ~/.neural_trader/backfill.env

# Restart services
sudo systemctl restart neural-trader-backfill
```

#### 2. Complete System Failure

```bash
# Provision new instance
terraform apply -var="instance_type=m5.2xlarge"

# Restore from S3 backups
aws s3 sync s3://backup-bucket/neural-trader/ /backup/

# Run recovery playbook
ansible-playbook restore-system.yml
```

## Backup Monitoring

### Backup Status Check

```bash
#!/bin/bash
# Check backup status and alert on failures

BACKUP_AGE_LIMIT=86400  # 24 hours in seconds

# Check database backup
LATEST_DB_BACKUP=$(find /backup/postgresql -name "*.dump.gz" -type f -printf '%T@ %p\n' | sort -n | tail -1 | cut -f2- -d" ")
DB_BACKUP_AGE=$(($(date +%s) - $(stat -c %Y "$LATEST_DB_BACKUP")))

if [ $DB_BACKUP_AGE -gt $BACKUP_AGE_LIMIT ]; then
    echo "WARNING: Database backup is older than 24 hours!"
    # Send alert
fi

# Check Redis backup
LATEST_REDIS_BACKUP=$(find /backup/redis -name "*.rdb.gz" -type f -printf '%T@ %p\n' | sort -n | tail -1 | cut -f2- -d" ")
REDIS_BACKUP_AGE=$(($(date +%s) - $(stat -c %Y "$LATEST_REDIS_BACKUP")))

if [ $REDIS_BACKUP_AGE -gt $BACKUP_AGE_LIMIT ]; then
    echo "WARNING: Redis backup is older than 24 hours!"
    # Send alert
fi
```

### Backup Validation

```python
async def validate_backup(backup_file, sample_size=1000):
    """Validate backup file integrity."""
    
    # Test restore to temporary database
    temp_db = f"trading_validate_{int(time.time())}"
    
    try:
        # Create temp database
        await conn.execute(f"CREATE DATABASE {temp_db}")
        
        # Restore backup
        restore_cmd = f"pg_restore -h localhost -U postgres -d {temp_db} {backup_file}"
        subprocess.run(restore_cmd, shell=True, check=True)
        
        # Validate data
        temp_conn = await asyncpg.connect(database=temp_db)
        
        # Check record count
        count = await temp_conn.fetchval("SELECT COUNT(*) FROM market_data")
        print(f"Backup contains {count:,} records")
        
        # Sample data validation
        samples = await temp_conn.fetch(
            "SELECT * FROM market_data ORDER BY RANDOM() LIMIT $1",
            sample_size
        )
        
        invalid = 0
        for record in samples:
            if not validate_ohlc(record):
                invalid += 1
        
        print(f"Sample validation: {invalid}/{sample_size} invalid records")
        
        return invalid == 0
        
    finally:
        # Cleanup
        await conn.execute(f"DROP DATABASE IF EXISTS {temp_db}")
```

## Best Practices

### Backup Best Practices

1. **3-2-1 Rule**
   - 3 copies of data
   - 2 different storage media
   - 1 offsite backup

2. **Regular Testing**
   - Monthly restore drills
   - Automated validation
   - Document restore times

3. **Retention Policy**
   - Daily backups: 7 days
   - Weekly backups: 4 weeks
   - Monthly backups: 12 months
   - Yearly backups: 7 years

4. **Security**
   - Encrypt backups at rest
   - Secure transfer protocols
   - Access control on backup files
   - Audit backup access

### Recovery Best Practices

1. **Documentation**
   - Keep procedures updated
   - Include all commands
   - Note dependencies
   - Record contact info

2. **Automation**
   - Script common procedures
   - Automate validation
   - Use configuration management
   - Implement monitoring

3. **Communication**
   - Define escalation paths
   - Maintain contact lists
   - Regular training
   - Clear responsibilities

## Recovery Checklist

### Pre-Recovery
- [ ] Identify failure scope
- [ ] Notify stakeholders
- [ ] Locate latest backups
- [ ] Prepare recovery environment
- [ ] Review recovery procedures

### During Recovery
- [ ] Stop affected services
- [ ] Restore from backup
- [ ] Validate restoration
- [ ] Update configurations
- [ ] Test functionality

### Post-Recovery
- [ ] Verify data integrity
- [ ] Update monitoring
- [ ] Document incident
- [ ] Review and improve procedures
- [ ] Schedule follow-up review

---

*Document Version: 1.0.0*  
*Last Updated: July 24, 2024*  
*Next Review: October 24, 2024*