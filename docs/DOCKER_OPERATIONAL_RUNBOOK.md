# Docker Production Operational Runbook

## Quick Reference Commands

### Start/Stop Platform
```bash
# Start all services
cd docker/production && ./deploy.sh

# Stop all services  
docker-compose -f docker-compose.prod.yml down

# Restart specific service
docker-compose -f docker-compose.prod.yml restart neural-trader
```

### Health Checks
```bash
# Service status
docker-compose -f docker-compose.prod.yml ps

# Application health
curl http://localhost:8080/health     # Neural Trader
curl http://localhost:8001/health     # Data Ingestion

# Monitoring access
open http://localhost:3000            # Grafana
open http://localhost:9090            # Prometheus
```

## Daily Operations

### Morning Startup Checklist
1. **Verify all services are running**
   ```bash
   docker-compose -f docker-compose.prod.yml ps
   ```
   Expected: All services should show "Up" status

2. **Check application health endpoints**
   ```bash
   curl -f http://localhost:8080/health && echo "✓ Neural Trader OK"
   curl -f http://localhost:8001/health && echo "✓ Data Ingestion OK"
   ```

3. **Verify Prometheus targets**
   ```bash
   curl -s http://localhost:9090/api/v1/targets | jq -r '.data.activeTargets[] | "\(.labels.job): \(.health)"'
   ```
   Expected: All targets should show "up"

4. **Check recent logs for errors**
   ```bash
   docker-compose -f docker-compose.prod.yml logs --since=24h | grep -i error
   ```

5. **Verify data ingestion is active**
   ```bash
   curl -s http://localhost:8001/metrics | grep "data_ingestion_requests_total"
   ```

### Evening Shutdown (Optional)
```bash
# Graceful shutdown
docker-compose -f docker-compose.prod.yml stop

# Force shutdown if needed
docker-compose -f docker-compose.prod.yml down
```

## Weekly Maintenance

### Sunday: System Health Review
1. **Check disk usage**
   ```bash
   docker system df
   docker volume ls
   ```

2. **Review metrics in Grafana**
   - Go to http://localhost:3000
   - Check "Neural Trader Overview" dashboard
   - Look for anomalies in prediction accuracy
   - Review system resource usage

3. **Database maintenance**
   ```bash
   docker-compose -f docker-compose.prod.yml exec timescaledb psql -U neural_trader -d neural_trader_db -c "VACUUM ANALYZE;"
   ```

4. **Clean up old logs**
   ```bash
   docker-compose -f docker-compose.prod.yml exec neural-trader find /var/log -name "*.log" -mtime +7 -delete
   ```

### Wednesday: Performance Review
1. **Check resource utilization**
   ```bash
   docker stats --no-stream
   ```

2. **Review slow queries**
   ```bash
   docker-compose -f docker-compose.prod.yml exec timescaledb psql -U neural_trader -d neural_trader_db -c "
   SELECT query, mean_exec_time, calls 
   FROM pg_stat_statements 
   ORDER BY mean_exec_time DESC 
   LIMIT 10;"
   ```

3. **Monitor model performance**
   ```bash
   curl -s http://localhost:9092/metrics | grep "neural_trader_prediction_accuracy"
   ```

## Monthly Operations

### First Monday: Update and Backup
1. **Create backup**
   ```bash
   cd docker/production
   mkdir -p backups/$(date +%Y%m)
   
   # Database backup
   docker run --rm -v timescaledb_data:/data -v $(pwd)/backups/$(date +%Y%m):/backup alpine \
     tar czf /backup/timescaledb_$(date +%Y%m%d).tar.gz -C /data .
   
   # Configuration backup
   tar czf backups/$(date +%Y%m)/configs_$(date +%Y%m%d).tar.gz configs/
   ```

2. **Update images** (if new version available)
   ```bash
   ./build.sh
   docker-compose -f docker-compose.prod.yml up -d --no-deps neural-trader
   ```

3. **Verify update**
   ```bash
   docker-compose -f docker-compose.prod.yml logs --tail=100 neural-trader
   curl http://localhost:8080/health
   ```

### Mid-month: Capacity Planning
1. **Review storage growth**
   ```bash
   docker exec $(docker-compose -f docker-compose.prod.yml ps -q timescaledb) \
     psql -U neural_trader -d neural_trader_db -c "
     SELECT 
       schemaname,
       tablename,
       pg_size_pretty(pg_total_relation_size(schemaname||'.'||tablename)) as size
     FROM pg_tables 
     WHERE schemaname = 'public' 
     ORDER BY pg_total_relation_size(schemaname||'.'||tablename) DESC;"
   ```

2. **Monitor data ingestion rates**
   ```bash
   curl -s http://localhost:9091/metrics | grep "data_ingestion_records_processed_total"
   ```

3. **Review prediction accuracy trends**
   - Access Grafana dashboard
   - Export monthly accuracy report
   - Compare with previous months

## Incident Response

### Service Down
1. **Check container status**
   ```bash
   docker-compose -f docker-compose.prod.yml ps
   ```

2. **Review recent logs**
   ```bash
   docker-compose -f docker-compose.prod.yml logs --tail=100 [service-name]
   ```

3. **Restart affected service**
   ```bash
   docker-compose -f docker-compose.prod.yml restart [service-name]
   ```

4. **If restart fails, recreate container**
   ```bash
   docker-compose -f docker-compose.prod.yml up -d --force-recreate [service-name]
   ```

### Database Connection Issues
1. **Check TimescaleDB status**
   ```bash
   docker-compose -f docker-compose.prod.yml exec timescaledb pg_isready -U neural_trader
   ```

2. **Review database logs**
   ```bash
   docker-compose -f docker-compose.prod.yml logs timescaledb
   ```

3. **Test connection manually**
   ```bash
   docker-compose -f docker-compose.prod.yml exec timescaledb \
     psql -U neural_trader -d neural_trader_db -c "SELECT version();"
   ```

4. **If database is corrupted, restore from backup**
   ```bash
   docker-compose -f docker-compose.prod.yml down
   docker volume rm $(docker volume ls -q | grep timescaledb)
   # Restore from backup (see backup section)
   docker-compose -f docker-compose.prod.yml up -d
   ```

### High Memory Usage
1. **Identify memory-heavy containers**
   ```bash
   docker stats --no-stream --format "table {{.Name}}\t{{.MemUsage}}\t{{.MemPerc}}"
   ```

2. **Check for memory leaks in logs**
   ```bash
   docker-compose -f docker-compose.prod.yml logs | grep -i "memory\|oom"
   ```

3. **Restart high-memory services**
   ```bash
   docker-compose -f docker-compose.prod.yml restart neural-trader
   ```

4. **If persistent, adjust memory limits**
   - Edit `docker-compose.prod.yml`
   - Increase memory limits for affected services
   - Redeploy: `docker-compose -f docker-compose.prod.yml up -d`

### Disk Space Issues
1. **Check Docker disk usage**
   ```bash
   docker system df
   ```

2. **Clean up unused resources**
   ```bash
   docker system prune -f
   docker image prune -f
   ```

3. **Check volume usage**
   ```bash
   docker exec $(docker-compose -f docker-compose.prod.yml ps -q timescaledb) \
     du -sh /var/lib/postgresql/data/*
   ```

4. **Archive old data** (if needed)
   ```bash
   docker-compose -f docker-compose.prod.yml exec timescaledb psql -U neural_trader -d neural_trader_db -c "
   DELETE FROM market_data WHERE time < NOW() - INTERVAL '6 months';"
   ```

### Monitoring Alerts
1. **Prometheus down**
   ```bash
   docker-compose -f docker-compose.prod.yml logs prometheus
   docker-compose -f docker-compose.prod.yml restart prometheus
   ```

2. **Grafana cannot connect to Prometheus**
   ```bash
   # Test connectivity
   docker-compose -f docker-compose.prod.yml exec grafana \
     wget -qO- http://prometheus:9090/api/v1/query?query=up
   ```

3. **Missing metrics data**
   ```bash
   # Check if targets are up
   curl http://localhost:9090/targets
   
   # Test metrics endpoints
   curl http://localhost:9092/metrics | head
   curl http://localhost:9091/metrics | head
   ```

## Performance Optimization

### Scale Services
```bash
# Add more neural-trader replicas
docker-compose -f docker-compose.prod.yml up -d --scale neural-trader=3

# Scale data ingestion (if bottleneck)
docker-compose -f docker-compose.prod.yml up -d --scale data-ingestion=2
```

### Database Performance
```bash
# Check slow queries
docker-compose -f docker-compose.prod.yml exec timescaledb psql -U neural_trader -d neural_trader_db -c "
SELECT query, mean_exec_time, calls, total_exec_time
FROM pg_stat_statements 
ORDER BY mean_exec_time DESC 
LIMIT 5;"

# Analyze table statistics
docker-compose -f docker-compose.prod.yml exec timescaledb psql -U neural_trader -d neural_trader_db -c "
ANALYZE;"
```

### Memory Optimization
```bash
# Check memory usage by service
docker stats --no-stream --format "table {{.Name}}\t{{.MemUsage}}\t{{.CPUPerc}}"

# Tune TimescaleDB memory settings (if needed)
# Edit docker/production/configs/timescaledb/postgresql.conf
# Restart TimescaleDB
```

## Security Operations

### Weekly Security Checks
1. **Review access logs**
   ```bash
   docker-compose -f docker-compose.prod.yml logs nginx | grep -E "(GET|POST|PUT|DELETE)"
   ```

2. **Check for failed authentication attempts**
   ```bash
   docker-compose -f docker-compose.prod.yml logs grafana | grep -i "failed"
   ```

3. **Update passwords monthly**
   - Update Docker secrets files
   - Restart affected services

### Security Incident Response
1. **Block suspicious IP (if using nginx)**
   ```bash
   # Add to nginx config and reload
   docker-compose -f docker-compose.prod.yml exec nginx nginx -s reload
   ```

2. **Check for compromise indicators**
   ```bash
   # Unusual process activity
   docker-compose -f docker-compose.prod.yml exec neural-trader ps aux
   
   # Network connections
   docker-compose -f docker-compose.prod.yml exec neural-trader netstat -tulpn
   ```

3. **Emergency shutdown**
   ```bash
   docker-compose -f docker-compose.prod.yml down
   ```

## Backup and Recovery Procedures

### Automated Daily Backup Script
Create `/etc/cron.daily/neural-trader-backup`:
```bash
#!/bin/bash
cd /path/to/neural-trader/docker/production
DATE=$(date +%Y%m%d)
BACKUP_DIR="backups/daily"
mkdir -p $BACKUP_DIR

# Database backup
docker run --rm \
  -v timescaledb_data:/data \
  -v $(pwd)/$BACKUP_DIR:/backup \
  alpine tar czf /backup/db_$DATE.tar.gz -C /data .

# Remove backups older than 7 days
find $BACKUP_DIR -name "*.tar.gz" -mtime +7 -delete

# Log backup completion
echo "$(date): Backup completed" >> /var/log/neural-trader-backup.log
```

### Recovery Procedures
```bash
# 1. Stop services
docker-compose -f docker-compose.prod.yml down

# 2. Remove damaged volume
docker volume rm timescaledb_data

# 3. Restore from backup
docker run --rm \
  -v timescaledb_data:/data \
  -v $(pwd)/backups/daily:/backup \
  alpine tar xzf /backup/db_20250803.tar.gz -C /data

# 4. Restart services
docker-compose -f docker-compose.prod.yml up -d

# 5. Verify restoration
docker-compose -f docker-compose.prod.yml exec timescaledb \
  psql -U neural_trader -d neural_trader_db -c "SELECT COUNT(*) FROM market_data;"
```

## Contact Information and Escalation

### On-Call Procedures
1. **Level 1**: Restart affected service
2. **Level 2**: Full system restart
3. **Level 3**: Restore from backup
4. **Level 4**: Contact development team

### Key Metrics to Monitor
- Service uptime: >99.9%
- Database response time: <100ms
- Prediction accuracy: >70%
- Memory usage: <80% of allocated
- Disk usage: <85% of available

### Log Locations
- Application logs: `docker-compose logs [service]`
- System logs: `/var/log/docker/`
- Backup logs: `/var/log/neural-trader-backup.log`

This runbook should be customized for your specific environment and requirements.