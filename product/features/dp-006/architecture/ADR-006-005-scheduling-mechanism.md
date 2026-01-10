# ADR-006-005: Scheduling Mechanism

**Feature**: dp-006 (Silver Layer Implementation)
**Status**: Accepted
**Date**: 2026-01-10
**Author**: NDP Architect
**Supersedes**: None

---

## Context

The Silver ETL process transforms Bronze Parquet data into TimescaleDB on a regular schedule. The scheduling mechanism must handle:

1. **Timing** - When to run ETL (hourly is target)
2. **Missed runs** - Catch-up after downtime
3. **Monitoring** - Visibility into execution status
4. **Failure handling** - What happens on ETL error
5. **Resource coordination** - Not overlap with other Pi tasks

### Constraints

- **Platform**: Raspberry Pi 5 running Linux (Debian/Ubuntu)
- **Container runtime**: Docker Compose
- **Existing infrastructure**: systemd available
- **ETL duration**: <60 seconds expected
- **Data freshness**: <5 minutes lag acceptable

---

## Decision

**Use systemd timer** for scheduling, running hourly at 5 minutes past the hour.

```ini
# /etc/systemd/system/ndp-silver-etl.timer
[Unit]
Description=NDP Silver ETL Timer
Requires=docker.service
After=docker.service

[Timer]
OnCalendar=*:05:00
Persistent=true
RandomizedDelaySec=60

[Install]
WantedBy=timers.target
```

```ini
# /etc/systemd/system/ndp-silver-etl.service
[Unit]
Description=NDP Silver ETL Run
Requires=docker.service
After=docker.service

[Service]
Type=oneshot
ExecStart=/usr/bin/docker compose -f /opt/ndp/deploy/pi/docker-compose.yml --profile etl run --rm silver-etl
WorkingDirectory=/opt/ndp/deploy/pi
StandardOutput=journal
StandardError=journal
TimeoutStartSec=300
Restart=on-failure
RestartSec=60

[Install]
WantedBy=multi-user.target
```

---

## Consequences

### Positive

1. **Persistent=true** - Catches up missed runs after Pi reboot or downtime
2. **RandomizedDelaySec** - Prevents thundering herd with other scheduled tasks
3. **Journal logging** - Integrated with systemd journal for monitoring
4. **Standard tooling** - `systemctl list-timers`, `journalctl -u ndp-silver-etl`
5. **Restart on failure** - Automatic retry with backoff
6. **No application state** - Timer state managed by systemd, not application

### Negative

1. **External to application** - Requires systemd on host (not in container)
2. **Deployment complexity** - Timer files must be installed on Pi
3. **Container startup overhead** - Each run starts fresh container

### Neutral

1. **Hourly granularity** - Sufficient for dashboard use case
2. **5-minute offset** - Allows Bronze writes to settle after hour boundary

---

## Alternatives Considered

### Alternative 1: Cron

**Description**: Traditional Unix cron scheduler.

```bash
# /etc/cron.d/ndp-etl
5 * * * * root docker compose -f /opt/ndp/deploy/pi/docker-compose.yml --profile etl run --rm silver-etl >> /var/log/ndp-etl.log 2>&1
```

| Factor | Cron | Systemd Timer |
|--------|------|---------------|
| Missed job handling | No | Yes (Persistent=true) |
| Logging | File-based | Journal-integrated |
| Monitoring | `crontab -l` | `systemctl list-timers` |
| Randomization | No | Yes |
| Failure retry | No | Yes |

**Rejected because**: No catch-up for missed runs. Less visibility. No integrated retry.

---

### Alternative 2: Embedded Scheduler (in-app)

**Description**: Build scheduler into silver-etl binary using `tokio-cron-scheduler`.

```rust
use tokio_cron_scheduler::{Job, JobScheduler};

async fn main() {
    let mut sched = JobScheduler::new().await?;

    sched.add(
        Job::new_async("0 5 * * * *", |_, _| {
            Box::pin(async { run_etl().await })
        })?
    ).await?;

    sched.start().await?;
}
```

| Factor | Embedded | Systemd Timer |
|--------|----------|---------------|
| Process lifecycle | Long-running | One-shot |
| Memory usage | Continuous | Transient |
| State management | In-app | External |
| Deployment | Single binary | Binary + timer files |
| Monitoring | Custom | systemd tools |

**Rejected because**: Adds complexity. Long-running process uses memory continuously. Requires state persistence for catch-up.

---

### Alternative 3: File Watch Trigger (inotify)

**Description**: Trigger ETL when new Parquet files appear.

```bash
inotifywait -m -r -e close_write /data/raw/ |
while read directory event filename; do
    if [[ "$filename" == *.parquet ]]; then
        trigger_etl
    fi
done
```

| Factor | File Watch | Systemd Timer |
|--------|------------|---------------|
| Latency | Near real-time | Up to 1 hour |
| Trigger frequency | Per file | Per hour |
| Complexity | High | Low |
| Batching | Manual | Natural |
| Resource usage | Continuous | Transient |

**Rejected because**: Over-complicated. May trigger multiple times. Inotify has watch limits. Near real-time not required.

---

### Alternative 4: PostgreSQL pg_cron

**Description**: Use PostgreSQL's pg_cron extension to trigger ETL.

```sql
SELECT cron.schedule('silver-etl', '5 * * * *',
    $$CALL silver.run_etl()$$
);
```

**Rejected because**: ETL logic is in Rust binary, not SQL. Would require stored procedure wrapper. Adds PostgreSQL complexity.

---

## Timer Configuration Details

### OnCalendar Syntax

```ini
OnCalendar=*:05:00
```

- `*` - Every day
- `:05` - At 5 minutes past
- `:00` - At 0 seconds

**Result**: Runs at 00:05, 01:05, 02:05, ... 23:05 daily.

### Persistent=true

When Pi reboots or timer was missed:
- systemd checks last run time
- If more than one interval passed, triggers immediately
- Only catches up ONE missed run (not all)

### RandomizedDelaySec=60

Adds random 0-60 second delay to:
- Prevent exact-same-second execution with other services
- Spread load if multiple streams have timers
- Reduce thundering herd on database

### TimeoutStartSec=300

ETL has 5 minutes to complete. If exceeded:
- Service is killed
- Logged as failure
- Restart attempted after RestartSec

---

## Deployment

### Installation Script

```bash
#!/bin/bash
# deploy/pi/install-timers.sh

set -e

# Copy timer files
sudo cp /opt/ndp/deploy/pi/systemd/ndp-silver-etl.timer /etc/systemd/system/
sudo cp /opt/ndp/deploy/pi/systemd/ndp-silver-etl.service /etc/systemd/system/

# Reload systemd
sudo systemctl daemon-reload

# Enable and start timer
sudo systemctl enable ndp-silver-etl.timer
sudo systemctl start ndp-silver-etl.timer

# Verify
systemctl list-timers ndp-silver-etl.timer
```

### Manual Trigger

```bash
# Run ETL immediately (outside timer)
sudo systemctl start ndp-silver-etl.service

# Check status
systemctl status ndp-silver-etl.service

# View logs
journalctl -u ndp-silver-etl.service -f
```

### Timer Management

```bash
# List all timers
systemctl list-timers --all

# Check timer status
systemctl status ndp-silver-etl.timer

# Disable timer
sudo systemctl stop ndp-silver-etl.timer
sudo systemctl disable ndp-silver-etl.timer

# View next run time
systemctl list-timers ndp-silver-etl.timer --no-pager
```

---

## Monitoring

### Grafana Dashboard Panel

```sql
-- ETL execution history from systemd journal
-- Requires promtail/loki or journal exporter

-- Alternative: ETL writes to status table
SELECT
    run_time,
    streams_processed,
    rows_inserted,
    duration_seconds,
    status
FROM silver.etl_runs
ORDER BY run_time DESC
LIMIT 10;
```

### Alerting

```yaml
# Grafana alert for ETL failures
alert:
  name: Silver ETL Failed
  condition: |
    SELECT COUNT(*) FROM silver.etl_runs
    WHERE run_time > NOW() - INTERVAL '2 hours'
    AND status = 'success'
    HAVING COUNT(*) = 0
  for: 30m
  labels:
    severity: warning
```

### ETL Status Table

```sql
CREATE TABLE silver.etl_runs (
    run_id          SERIAL PRIMARY KEY,
    run_time        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    streams         TEXT[] NOT NULL,
    rows_inserted   INTEGER NOT NULL DEFAULT 0,
    duration_ms     INTEGER NOT NULL,
    status          TEXT NOT NULL,  -- 'success', 'partial', 'failed'
    error_message   TEXT,
    watermarks      JSONB  -- {"stream": "timestamp"} for each stream
);
```

---

## Integration with deploy.sh

```bash
# deploy/pi/deploy.sh additions

function install_timers() {
    echo "Installing systemd timers..."
    sudo cp "${SCRIPT_DIR}/systemd/ndp-silver-etl.timer" /etc/systemd/system/
    sudo cp "${SCRIPT_DIR}/systemd/ndp-silver-etl.service" /etc/systemd/system/
    sudo systemctl daemon-reload
    sudo systemctl enable ndp-silver-etl.timer
    sudo systemctl start ndp-silver-etl.timer
    echo "Timer installed. Next run:"
    systemctl list-timers ndp-silver-etl.timer --no-pager
}

function run_etl() {
    echo "Running Silver ETL manually..."
    docker compose --profile etl run --rm silver-etl
}

# Add to status command
function status() {
    # ... existing status checks ...

    echo ""
    echo "=== Silver ETL Timer ==="
    systemctl list-timers ndp-silver-etl.timer --no-pager 2>/dev/null || echo "Timer not installed"

    echo ""
    echo "=== Last ETL Run ==="
    journalctl -u ndp-silver-etl.service -n 5 --no-pager 2>/dev/null || echo "No runs yet"
}
```

---

## References

1. systemd.timer documentation: https://www.freedesktop.org/software/systemd/man/systemd.timer.html
2. Systemd Timers vs Cron: https://akashrajpurohit.com/blog/systemd-timers-vs-cron-jobs/
3. Research: `research/agenticdataplatform/silver/02-etl-alternatives.md` - Scheduling section
4. ADR-006-002: Binary Architecture (container execution pattern)

---

## Document History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2026-01-10 | NDP Architect | Initial decision |
