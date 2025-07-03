#!/bin/sh
# Automated backup script for Neural Trader production databases

set -e

# Configuration
BACKUP_DIR="/backups"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
RETENTION_DAYS=7

# Database configuration
DB_HOST="timescaledb"
DB_PORT="5432"
DB_NAME="neural_trader_db"
DB_USER="neural_trader"

# Create backup directory if it doesn't exist
mkdir -p "${BACKUP_DIR}/postgres"
mkdir -p "${BACKUP_DIR}/redis"

echo "[$(date)] Starting backup process..."

# Backup PostgreSQL/TimescaleDB
echo "[$(date)] Backing up PostgreSQL database..."
PGPASSWORD="${PGPASSWORD}" pg_dump \
    -h "${DB_HOST}" \
    -p "${DB_PORT}" \
    -U "${DB_USER}" \
    -d "${DB_NAME}" \
    -Fc \
    -f "${BACKUP_DIR}/postgres/neural_trader_${TIMESTAMP}.dump"

# Compress the backup
echo "[$(date)] Compressing PostgreSQL backup..."
gzip "${BACKUP_DIR}/postgres/neural_trader_${TIMESTAMP}.dump"

# Backup Redis (if accessible)
if command -v redis-cli >/dev/null 2>&1; then
    echo "[$(date)] Backing up Redis..."
    redis-cli -h redis --rdb "${BACKUP_DIR}/redis/redis_${TIMESTAMP}.rdb"
    gzip "${BACKUP_DIR}/redis/redis_${TIMESTAMP}.rdb"
fi

# Clean up old backups
echo "[$(date)] Cleaning up old backups..."
find "${BACKUP_DIR}/postgres" -name "*.dump.gz" -mtime +${RETENTION_DAYS} -delete
find "${BACKUP_DIR}/redis" -name "*.rdb.gz" -mtime +${RETENTION_DAYS} -delete

# Calculate backup sizes
POSTGRES_SIZE=$(du -sh "${BACKUP_DIR}/postgres/neural_trader_${TIMESTAMP}.dump.gz" | cut -f1)
echo "[$(date)] PostgreSQL backup size: ${POSTGRES_SIZE}"

if [ -f "${BACKUP_DIR}/redis/redis_${TIMESTAMP}.rdb.gz" ]; then
    REDIS_SIZE=$(du -sh "${BACKUP_DIR}/redis/redis_${TIMESTAMP}.rdb.gz" | cut -f1)
    echo "[$(date)] Redis backup size: ${REDIS_SIZE}"
fi

echo "[$(date)] Backup completed successfully!"

# Optional: Upload to S3 or other cloud storage
# aws s3 cp "${BACKUP_DIR}/postgres/neural_trader_${TIMESTAMP}.dump.gz" s3://your-backup-bucket/postgres/
# aws s3 cp "${BACKUP_DIR}/redis/redis_${TIMESTAMP}.rdb.gz" s3://your-backup-bucket/redis/