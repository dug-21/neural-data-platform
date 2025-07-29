# Neural Trader Operational Quick Reference

## 🚨 Emergency Response Procedures

### WebSocket Connection Lost
```bash
# IMMEDIATE ACTION (0-30 seconds)
1. Check metrics: websocket_connection_uptime_ratio
2. Verify automatic failover triggered
3. Monitor message buffer status

# IF NOT AUTO-RECOVERED (30-60 seconds)
4. Force provider switch: 
   curl -X POST localhost:8080/api/failover/force
5. Check circuit breaker status
6. Verify data continuity

# ESCALATION (>60 seconds)
7. Page on-call engineer
8. Prepare for manual intervention
9. Document incident timeline
```

### High Memory Usage (>80%)
```bash
# AUTOMATED RESPONSE TRIGGERED:
- Garbage collection forced
- Buffer sizes reduced
- Non-critical processes paused

# MANUAL VERIFICATION:
docker stats neural-trader-ingestion
kubectl top pods -n trading

# IF STILL HIGH:
systemctl restart neural-trader-ingestion
# State automatically preserved and restored
```

### Data Quality Issues
```bash
# CHECK VALIDATION METRICS:
curl localhost:9090/metrics | grep validation_failures

# VIEW QUARANTINED DATA:
psql -d neural_trader -c "SELECT * FROM quarantine_log ORDER BY timestamp DESC LIMIT 10;"

# FORCE CONSENSUS MODE:
curl -X POST localhost:8080/api/data/consensus-mode
```

## 📋 Daily Operations Checklist

### Morning (Pre-Market)
- [ ] Check overnight incident reports
- [ ] Verify all connections green
- [ ] Review resource usage trends
- [ ] Confirm backup systems ready
- [ ] Test failover mechanism

### Market Hours
- [ ] Monitor real-time dashboard
- [ ] Check latency metrics (target <50ms)
- [ ] Verify data completeness
- [ ] Watch for anomaly alerts

### End of Day
- [ ] Review error logs
- [ ] Check data reconciliation
- [ ] Update incident tracker
- [ ] Plan next day priorities

## 📏 Key Metrics & Thresholds

| Metric | Green | Yellow | Red | Action |
|--------|-------|---------|-----|--------|
| Uptime | >99.9% | >99% | <99% | Investigate root cause |
| Latency p95 | <50ms | <200ms | >500ms | Scale resources |
| Error Rate | <0.1% | <1% | >5% | Circuit breaker activates |
| Memory Usage | <70% | <85% | >90% | Auto-restart triggered |
| Queue Depth | <1000 | <5000 | >10000 | Backpressure enabled |
| Reconnections/hr | 0 | <3 | >5 | Provider health check |

## 🔧 Common Commands

### Health Checks
```bash
# Overall system health
curl localhost:8080/health

# Provider-specific health
curl localhost:8080/health/providers

# WebSocket connection status
curl localhost:8080/ws/status
```

### Force Actions
```bash
# Switch to backup provider
curl -X POST localhost:8080/api/provider/switch

# Clear message buffer
curl -X POST localhost:8080/api/buffer/clear

# Trigger garbage collection
curl -X POST localhost:8080/api/gc/force
```

### Monitoring
```bash
# View real-time logs
tail -f /var/log/neural-trader/ingestion.log | grep ERROR

# Check metrics
curl -s localhost:9090/metrics | grep -E '(websocket|error|latency)'

# Database connection pool
psql -d neural_trader -c "SELECT * FROM pg_stat_activity WHERE application_name='neural-trader';"
```

## 📡 Alert Response Matrix

| Alert | Severity | Response Time | Auto-Action | Manual Action |
|-------|----------|---------------|-------------|---------------|
| WS Disconnect | CRITICAL | Immediate | Failover | Verify recovery |
| High Latency | WARNING | 15 min | Scale up | Check bottleneck |
| Memory >90% | CRITICAL | Immediate | Restart | Check for leaks |
| Data Gap | CRITICAL | Immediate | Backfill | Verify source |
| Circuit Open | WARNING | 15 min | Wait | Check provider |
| Queue Full | WARNING | 15 min | Throttle | Scale workers |

## 🔄 Recovery Procedures

### From WebSocket Failure
1. Automatic failover completes
2. Verify data continuity
3. Queue replay initiated
4. Monitor for gaps
5. Document incident

### From System Crash
1. State restored from checkpoint
2. Subscriptions re-established
3. Gap detection runs
4. Backfill triggered
5. Normal operation resumes

### From Data Corruption
1. Bad data quarantined
2. Provider marked unhealthy
3. Consensus mode activated
4. Clean data flows resume
5. Post-mortem scheduled

## 📞 Emergency Contacts

| Role | Contact | When to Call |
|------|---------|-------------|
| Primary On-Call | PagerDuty | Any critical alert |
| Engineering Lead | Slack #eng-lead | Major incidents |
| DevOps | Slack #devops | Infrastructure issues |
| Data Team | Slack #data-quality | Data integrity concerns |

## 🎯 Quick Win Implementations

### This Week's Priorities
1. **WebSocket Heartbeat** - Prevents silent failures
2. **Connection Pool** - Enables instant failover  
3. **Circuit Breaker** - Stops cascade failures
4. **Alert Rules** - Immediate issue awareness

### Configuration Changes
```yaml
# WebSocket config (polygon_websocket.py)
heartbeat_interval: 30  # seconds
reconnect_delay: 1      # initial seconds
max_reconnect_delay: 60 # max seconds
connection_pool_size: 3 # primary + 2 backup

# Circuit breaker (circuit_breaker.py)
failure_threshold: 5    # failures
time_window: 60         # seconds
recovery_timeout: 30    # seconds
success_threshold: 3    # consecutive
```

---

**Remember:** The system is designed to self-heal. Only intervene if automated recovery fails!