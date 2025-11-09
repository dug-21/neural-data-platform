#!/bin/bash
# Alert Mechanisms for Drift Detection and System Monitoring

set -e

# Configuration
ALERT_CONFIG=${ALERT_CONFIG:-/workspaces/neural-trader/configs/alerts.yaml}
WEBHOOK_URL=${WEBHOOK_URL:-}  # Slack/Discord webhook
EMAIL_RECIPIENTS=${EMAIL_RECIPIENTS:-}
ALERT_LOG=${ALERT_LOG:-/workspaces/neural-trader/logs/alerts.log}
METRICS_DIR=${METRICS_DIR:-/workspaces/neural-trader/metrics}

# Color output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
MAGENTA='\033[0;35m'
NC='\033[0m'

log_info() { echo -e "${GREEN}[INFO]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }
log_alert() { echo -e "${MAGENTA}[ALERT]${NC} $1"; }
log_critical() { echo -e "${RED}[CRITICAL]${NC} $1"; }

# Alert severity levels
SEVERITY_INFO="INFO"
SEVERITY_WARN="WARNING"
SEVERITY_ERROR="ERROR"
SEVERITY_CRITICAL="CRITICAL"

# Initialize alert log
mkdir -p $(dirname "$ALERT_LOG")
touch "$ALERT_LOG"

# Send alert to log file
log_alert_to_file() {
    local severity=$1
    local category=$2
    local message=$3
    local details=$4
    
    local timestamp=$(date -u +%Y-%m-%dT%H:%M:%SZ)
    
    cat >> "$ALERT_LOG" << EOF
{
    "timestamp": "$timestamp",
    "severity": "$severity",
    "category": "$category",
    "message": "$message",
    "details": "$details"
}
EOF
}

# Send webhook notification
send_webhook_alert() {
    local severity=$1
    local category=$2
    local message=$3
    local details=$4
    
    if [ -z "$WEBHOOK_URL" ]; then
        return
    fi
    
    local color="#808080"
    case "$severity" in
        "$SEVERITY_INFO") color="#36a64f" ;;
        "$SEVERITY_WARN") color="#ff9900" ;;
        "$SEVERITY_ERROR") color="#ff0000" ;;
        "$SEVERITY_CRITICAL") color="#990000" ;;
    esac
    
    # Slack-compatible webhook payload
    local payload=$(cat << EOF
{
    "attachments": [
        {
            "color": "$color",
            "title": "[$severity] $category Alert",
            "text": "$message",
            "fields": [
                {
                    "title": "Details",
                    "value": "$details",
                    "short": false
                },
                {
                    "title": "Time",
                    "value": "$(date)",
                    "short": true
                },
                {
                    "title": "System",
                    "value": "Neural Trader V2",
                    "short": true
                }
            ],
            "footer": "Alert System",
            "footer_icon": "https://platform.slack-edge.com/img/default_application_icon.png",
            "ts": $(date +%s)
        }
    ]
}
EOF
)
    
    curl -X POST -H 'Content-Type: application/json' \
        -d "$payload" \
        "$WEBHOOK_URL" \
        -s -o /dev/null 2>&1 || log_warn "Failed to send webhook alert"
}

# Send email alert
send_email_alert() {
    local severity=$1
    local category=$2
    local message=$3
    local details=$4
    
    if [ -z "$EMAIL_RECIPIENTS" ]; then
        return
    fi
    
    if command -v mail > /dev/null 2>&1; then
        echo -e "Alert Details:\n\n$message\n\n$details\n\nTime: $(date)" | \
            mail -s "[$severity] Neural Trader Alert: $category" "$EMAIL_RECIPIENTS"
    fi
}

# Send desktop notification
send_desktop_notification() {
    local severity=$1
    local category=$2
    local message=$3
    
    if command -v notify-send > /dev/null 2>&1; then
        notify-send -u critical "Neural Trader Alert [$severity]" "$category: $message"
    elif command -v osascript > /dev/null 2>&1; then
        osascript -e "display notification \"$message\" with title \"Neural Trader Alert [$severity]\" subtitle \"$category\""
    fi
}

# Centralized alert dispatcher
send_alert() {
    local severity=$1
    local category=$2
    local message=$3
    local details=${4:-"No additional details"}
    
    # Log alert
    log_alert_to_file "$severity" "$category" "$message" "$details"
    
    # Console output
    case "$severity" in
        "$SEVERITY_INFO")
            log_info "[$category] $message"
            ;;
        "$SEVERITY_WARN")
            log_warn "[$category] $message"
            ;;
        "$SEVERITY_ERROR")
            log_error "[$category] $message"
            ;;
        "$SEVERITY_CRITICAL")
            log_critical "[$category] $message"
            ;;
    esac
    
    # Send to configured channels
    send_webhook_alert "$severity" "$category" "$message" "$details"
    send_email_alert "$severity" "$category" "$message" "$details"
    
    # Desktop notification for ERROR and CRITICAL
    if [ "$severity" = "$SEVERITY_ERROR" ] || [ "$severity" = "$SEVERITY_CRITICAL" ]; then
        send_desktop_notification "$severity" "$category" "$message"
    fi
}

# Check performance thresholds
check_performance_alerts() {
    log_info "Checking performance thresholds..."
    
    # Load latest baseline
    local baseline_file=$(ls -t "$METRICS_DIR"/baseline/baseline_*.json 2>/dev/null | head -1)
    
    if [ -z "$baseline_file" ]; then
        send_alert "$SEVERITY_WARN" "Performance" "No baseline metrics found" \
            "Run baseline-metrics.sh to establish baselines"
        return
    fi
    
    # Check build time
    local build_threshold=$(jq -r '.thresholds.build_max_ms' "$baseline_file")
    local current_build_time=$(cat "$METRICS_DIR"/current/build_time.txt 2>/dev/null || echo "0")
    
    if [ "$current_build_time" -gt "$build_threshold" ]; then
        send_alert "$SEVERITY_WARN" "Performance" "Build time exceeds threshold" \
            "Current: ${current_build_time}ms > Threshold: ${build_threshold}ms"
    fi
    
    # Check memory usage
    local memory_threshold=$(jq -r '.thresholds.memory_max_mb' "$baseline_file")
    local current_memory=$(docker stats --no-stream --format "{{.MemUsage}}" data-ingestion 2>/dev/null | grep -oE '[0-9.]+' | head -1)
    
    if (( $(echo "$current_memory > $memory_threshold" | bc -l) )); then
        send_alert "$SEVERITY_ERROR" "Resources" "Memory usage exceeds threshold" \
            "Current: ${current_memory}MB > Threshold: ${memory_threshold}MB"
    fi
}

# Check service health
check_service_health_alerts() {
    log_info "Checking service health..."
    
    local unhealthy_services=()
    
    for service in config-store data-ingestion data-staging neural-ml-ops neural-trading; do
        if ! docker ps --filter "name=$service" --filter "status=running" --format "{{.Names}}" | grep -q "$service"; then
            unhealthy_services+=("$service")
        fi
    done
    
    if [ ${#unhealthy_services[@]} -gt 0 ]; then
        send_alert "$SEVERITY_CRITICAL" "Services" "${#unhealthy_services[@]} services are down" \
            "Affected services: ${unhealthy_services[*]}"
    fi
}

# Check drift detection results
check_drift_alerts() {
    log_info "Checking for drift..."
    
    # Find latest drift detection results
    local drift_results=$(ls -t "$METRICS_DIR"/drift/drift_test_*.json 2>/dev/null | head -1)
    
    if [ -z "$drift_results" ]; then
        return
    fi
    
    # Check for detected drift
    local build_drift=$(jq -r '.drift_detected.build' "$drift_results")
    local memory_drift=$(jq -r '.drift_detected.memory' "$drift_results")
    local throughput_drift=$(jq -r '.drift_detected.throughput' "$drift_results")
    local config_drift=$(jq -r '.drift_detected.config' "$drift_results")
    local data_drift=$(jq -r '.drift_detected.data_quality' "$drift_results")
    local model_drift=$(jq -r '.drift_detected.model' "$drift_results")
    
    [ "$build_drift" = "true" ] && \
        send_alert "$SEVERITY_WARN" "Drift" "Build performance drift detected" \
            "Build times have increased beyond acceptable threshold"
    
    [ "$memory_drift" = "true" ] && \
        send_alert "$SEVERITY_ERROR" "Drift" "Memory usage drift detected" \
            "Memory consumption has increased significantly"
    
    [ "$throughput_drift" = "true" ] && \
        send_alert "$SEVERITY_ERROR" "Drift" "Throughput drift detected" \
            "System throughput has degraded below minimum threshold"
    
    [ "$config_drift" = "true" ] && \
        send_alert "$SEVERITY_WARN" "Drift" "Configuration drift detected" \
            "Uncommitted configuration changes found"
    
    [ "$data_drift" = "true" ] && \
        send_alert "$SEVERITY_ERROR" "Drift" "Data quality drift detected" \
            "Data anomalies or quality issues detected"
    
    [ "$model_drift" = "true" ] && \
        send_alert "$SEVERITY_CRITICAL" "Drift" "Model performance drift detected" \
            "Model accuracy has degraded below acceptable threshold"
}

# Check error logs
check_error_log_alerts() {
    log_info "Checking error logs..."
    
    local error_count=0
    local critical_errors=0
    
    # Check container logs for errors
    for service in data-ingestion data-staging neural-ml-ops neural-trading; do
        local errors=$(docker logs "$service" --since "5m" 2>&1 | grep -ci "error\|panic\|fatal" || echo "0")
        error_count=$((error_count + errors))
        
        local criticals=$(docker logs "$service" --since "5m" 2>&1 | grep -ci "panic\|fatal" || echo "0")
        critical_errors=$((critical_errors + criticals))
    done
    
    if [ $critical_errors -gt 0 ]; then
        send_alert "$SEVERITY_CRITICAL" "Logs" "Critical errors detected in logs" \
            "$critical_errors critical errors found in last 5 minutes"
    elif [ $error_count -gt 10 ]; then
        send_alert "$SEVERITY_ERROR" "Logs" "High error rate detected" \
            "$error_count errors found in last 5 minutes"
    fi
}

# Check database connectivity
check_database_alerts() {
    log_info "Checking database connectivity..."
    
    if ! PGPASSWORD=postgres psql -h localhost -U postgres -d neural_trader_v2 -c "SELECT 1" > /dev/null 2>&1; then
        send_alert "$SEVERITY_CRITICAL" "Database" "Database connection failed" \
            "Unable to connect to TimescaleDB"
    fi
    
    # Check for slow queries
    local slow_queries=$(PGPASSWORD=postgres psql -h localhost -U postgres -d neural_trader_v2 -t -c "
        SELECT COUNT(*) 
        FROM pg_stat_activity 
        WHERE state != 'idle' 
        AND query_start < NOW() - INTERVAL '30 seconds'
    " 2>/dev/null | xargs)
    
    if [ "$slow_queries" -gt "5" ]; then
        send_alert "$SEVERITY_WARN" "Database" "Multiple slow queries detected" \
            "$slow_queries queries running for more than 30 seconds"
    fi
}

# Setup alert rules configuration
create_alert_config() {
    log_info "Creating alert configuration..."
    
    mkdir -p $(dirname "$ALERT_CONFIG")
    
    cat > "$ALERT_CONFIG" << 'EOF'
# Alert Configuration
alerts:
  performance:
    enabled: true
    thresholds:
      build_time_ms: 180000
      test_time_ms: 60000
      memory_mb: 500
      cpu_percent: 80
      
  services:
    enabled: true
    check_interval: 60
    restart_on_failure: false
    
  drift:
    enabled: true
    check_interval: 300
    auto_remediate: false
    
  errors:
    enabled: true
    error_threshold: 10
    critical_threshold: 1
    window_minutes: 5
    
  database:
    enabled: true
    slow_query_seconds: 30
    connection_timeout: 5
    
channels:
  webhook:
    enabled: false
    url: ""
    
  email:
    enabled: false
    recipients: ""
    smtp_server: ""
    
  desktop:
    enabled: true
    
  log:
    enabled: true
    path: /workspaces/neural-trader/logs/alerts.log
    
escalation:
  rules:
    - severity: CRITICAL
      channels: [webhook, email, desktop, log]
      immediate: true
      
    - severity: ERROR
      channels: [webhook, desktop, log]
      immediate: false
      
    - severity: WARNING
      channels: [log]
      immediate: false
      
    - severity: INFO
      channels: [log]
      immediate: false
EOF
    
    log_info "Alert configuration saved to: $ALERT_CONFIG"
}

# Monitor continuously
monitor_loop() {
    log_info "Starting continuous monitoring..."
    
    local check_interval=${CHECK_INTERVAL:-60}
    
    while true; do
        log_info "Running alert checks..."
        
        check_service_health_alerts
        check_performance_alerts
        check_drift_alerts
        check_error_log_alerts
        check_database_alerts
        
        log_info "Alert check complete. Next check in ${check_interval}s"
        sleep "$check_interval"
    done
}

# Generate alert summary
generate_alert_summary() {
    log_info "Generating alert summary..."
    
    local summary_file="$METRICS_DIR/alert_summary_$(date +%Y%m%d).txt"
    
    cat > "$summary_file" << EOF
=====================================
Alert Summary Report
=====================================
Date: $(date)

Recent Alerts (Last 24 Hours)
------------------------------
$(tail -100 "$ALERT_LOG" 2>/dev/null | jq -r '"\(.timestamp) [\(.severity)] \(.category): \(.message)"' | tail -20)

Alert Statistics
----------------
Critical: $(grep -c "CRITICAL" "$ALERT_LOG" 2>/dev/null || echo 0)
Error: $(grep -c "ERROR" "$ALERT_LOG" 2>/dev/null || echo 0)
Warning: $(grep -c "WARNING" "$ALERT_LOG" 2>/dev/null || echo 0)
Info: $(grep -c "INFO" "$ALERT_LOG" 2>/dev/null || echo 0)

Most Common Alert Categories
-----------------------------
$(tail -1000 "$ALERT_LOG" 2>/dev/null | jq -r '.category' | sort | uniq -c | sort -rn | head -5)

Alert Configuration
-------------------
Config File: $ALERT_CONFIG
Log File: $ALERT_LOG
Webhook: $([ -n "$WEBHOOK_URL" ] && echo "Configured" || echo "Not configured")
Email: $([ -n "$EMAIL_RECIPIENTS" ] && echo "Configured" || echo "Not configured")

System Status
-------------
Services Running: $(docker ps --filter "label=neural-trader" --format "{{.Names}}" | wc -l)
Database Connected: $(PGPASSWORD=postgres psql -h localhost -U postgres -d neural_trader_v2 -c "SELECT 1" > /dev/null 2>&1 && echo "Yes" || echo "No")
Redis Connected: $(redis-cli ping > /dev/null 2>&1 && echo "Yes" || echo "No")

EOF
    
    log_info "Alert summary saved to: $summary_file"
    cat "$summary_file"
}

# Main execution
main() {
    local mode=${1:-check}
    
    case "$mode" in
        check)
            log_info "Running single alert check..."
            check_service_health_alerts
            check_performance_alerts
            check_drift_alerts
            check_error_log_alerts
            check_database_alerts
            generate_alert_summary
            ;;
            
        monitor)
            log_info "Starting continuous monitoring mode..."
            monitor_loop
            ;;
            
        test)
            log_info "Testing alert mechanisms..."
            send_alert "$SEVERITY_INFO" "Test" "Alert system test" "This is a test alert"
            send_alert "$SEVERITY_WARN" "Test" "Warning test alert" "Testing warning severity"
            send_alert "$SEVERITY_ERROR" "Test" "Error test alert" "Testing error severity"
            send_alert "$SEVERITY_CRITICAL" "Test" "Critical test alert" "Testing critical severity"
            log_info "Test alerts sent. Check configured channels."
            ;;
            
        config)
            create_alert_config
            ;;
            
        summary)
            generate_alert_summary
            ;;
            
        *)
            echo "Usage: $0 [check|monitor|test|config|summary]"
            echo "  check   - Run single alert check"
            echo "  monitor - Start continuous monitoring"
            echo "  test    - Send test alerts"
            echo "  config  - Create alert configuration"
            echo "  summary - Generate alert summary"
            exit 1
            ;;
    esac
}

main "$@"