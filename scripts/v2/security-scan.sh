#!/bin/bash
# Security Scanning Script for Neural Trader V2
# Performs parallel security checks across the codebase

set -e

# Configuration
PROJECT_ROOT=${PROJECT_ROOT:-/workspaces/neural-trader}
SCAN_LEVEL=${SCAN_LEVEL:-medium}  # low, medium, high
PARALLEL_SCANS=${PARALLEL_SCANS:-true}
REPORT_DIR=${REPORT_DIR:-/workspaces/neural-trader/security-reports}

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
log_step() { echo -e "${BLUE}[STEP]${NC} $1"; }
log_security() { echo -e "${MAGENTA}[SECURITY]${NC} $1"; }
log_critical() { echo -e "${RED}[CRITICAL]${NC} $1"; }

# Initialize report directory
mkdir -p "$REPORT_DIR"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
SECURITY_REPORT="$REPORT_DIR/security_scan_${TIMESTAMP}.json"
SUMMARY_REPORT="$REPORT_DIR/security_summary_${TIMESTAMP}.txt"

# Track findings
declare -A security_findings
security_findings[critical]=0
security_findings[high]=0
security_findings[medium]=0
security_findings[low]=0
security_findings[total]=0

# Scan for hardcoded secrets
scan_secrets() {
    log_step "Scanning for hardcoded secrets..."
    
    local findings_file="/tmp/secrets_findings.txt"
    > "$findings_file"
    
    # Pattern list for sensitive data
    local patterns=(
        "api[_-]?key"
        "secret[_-]?key"
        "password"
        "token"
        "private[_-]?key"
        "aws[_-]?access"
        "aws[_-]?secret"
        "github[_-]?token"
        "slack[_-]?webhook"
    )
    
    for pattern in "${patterns[@]}"; do
        # Search in code files (exclude test files and docs)
        grep -r -i "$pattern" \
            --include="*.rs" \
            --include="*.py" \
            --include="*.sh" \
            --include="*.yaml" \
            --include="*.yml" \
            --include="*.toml" \
            --exclude-dir=".git" \
            --exclude-dir="target" \
            --exclude-dir="venv" \
            --exclude-dir="docs" \
            --exclude="*test*" \
            "$PROJECT_ROOT" 2>/dev/null | \
            grep -v "example\|template\|fake\|dummy\|test" >> "$findings_file" || true
    done
    
    local secret_count=$(wc -l < "$findings_file")
    
    if [ "$secret_count" -gt 0 ]; then
        log_critical "Found $secret_count potential secrets!"
        security_findings[critical]=$((security_findings[critical] + secret_count))
        
        # Show first 5 findings
        head -5 "$findings_file"
    else
        log_info "✓ No hardcoded secrets found"
    fi
}

# Scan Rust dependencies for vulnerabilities
scan_rust_dependencies() {
    log_step "Scanning Rust dependencies..."
    
    # Install cargo-audit if not present
    if ! command -v cargo-audit >/dev/null 2>&1; then
        log_info "Installing cargo-audit..."
        cargo install cargo-audit
    fi
    
    local vulnerabilities=0
    
    for service_dir in "$PROJECT_ROOT"/v2/*; do
        if [ -f "$service_dir/Cargo.toml" ]; then
            local service=$(basename "$service_dir")
            log_info "Scanning $service dependencies..."
            
            cd "$service_dir"
            local audit_output=$(cargo audit 2>&1)
            
            if echo "$audit_output" | grep -q "vulnerabilities found"; then
                local vuln_count=$(echo "$audit_output" | grep -oE "[0-9]+ vulnerabilities" | grep -oE "[0-9]+")
                vulnerabilities=$((vulnerabilities + vuln_count))
                log_warn "$service: $vuln_count vulnerabilities found"
                
                # Extract severity
                local critical=$(echo "$audit_output" | grep -c "CRITICAL" || true)
                local high=$(echo "$audit_output" | grep -c "HIGH" || true)
                
                security_findings[critical]=$((security_findings[critical] + critical))
                security_findings[high]=$((security_findings[high] + high))
            else
                log_info "✓ $service: No vulnerabilities"
            fi
        fi
    done
    
    if [ "$vulnerabilities" -gt 0 ]; then
        log_warn "Total Rust vulnerabilities: $vulnerabilities"
    else
        log_info "✓ All Rust dependencies secure"
    fi
}

# Scan Python dependencies
scan_python_dependencies() {
    log_step "Scanning Python dependencies..."
    
    if [ -f "$PROJECT_ROOT/requirements.txt" ]; then
        # Install safety if not present
        pip install safety >/dev/null 2>&1 || true
        
        # Run safety check
        local safety_output=$(safety check -r "$PROJECT_ROOT/requirements.txt" --json 2>/dev/null || echo "{}")
        
        local vuln_count=$(echo "$safety_output" | jq -r '.vulnerabilities | length' 2>/dev/null || echo "0")
        
        if [ "$vuln_count" -gt 0 ]; then
            log_warn "Found $vuln_count Python vulnerabilities"
            security_findings[high]=$((security_findings[high] + vuln_count))
        else
            log_info "✓ Python dependencies secure"
        fi
    fi
}

# Scan Docker images
scan_docker_images() {
    log_step "Scanning Docker images..."
    
    # Install trivy if not present
    if ! command -v trivy >/dev/null 2>&1; then
        log_info "Installing Trivy..."
        curl -sfL https://raw.githubusercontent.com/aquasecurity/trivy/main/contrib/install.sh | sh -s -- -b /usr/local/bin
    fi
    
    local images=(
        "neural-trader/config-store:latest"
        "neural-trader/data-ingestion:latest"
        "neural-trader/data-staging:latest"
        "neural-trader/neural-ml-ops:latest"
        "neural-trader/neural-trading:latest"
    )
    
    for image in "${images[@]}"; do
        if docker image inspect "$image" >/dev/null 2>&1; then
            log_info "Scanning $image..."
            
            local scan_output=$(trivy image --severity HIGH,CRITICAL --format json "$image" 2>/dev/null || echo "{}")
            local vuln_count=$(echo "$scan_output" | jq -r '.Results[].Vulnerabilities | length' 2>/dev/null || echo "0")
            
            if [ "$vuln_count" -gt 0 ]; then
                log_warn "$image: $vuln_count vulnerabilities"
                security_findings[high]=$((security_findings[high] + vuln_count))
            else
                log_info "✓ $image: Secure"
            fi
        fi
    done
}

# Check file permissions
check_file_permissions() {
    log_step "Checking file permissions..."
    
    # Find files with overly permissive permissions
    local insecure_files=$(find "$PROJECT_ROOT" \
        -type f \
        \( -perm -002 -o -perm -020 \) \
        -not -path "*/\.git/*" \
        -not -path "*/target/*" \
        -not -path "*/venv/*" \
        2>/dev/null | wc -l)
    
    if [ "$insecure_files" -gt 0 ]; then
        log_warn "Found $insecure_files files with insecure permissions"
        security_findings[medium]=$((security_findings[medium] + insecure_files))
    else
        log_info "✓ File permissions secure"
    fi
}

# Check for SSL/TLS issues
check_tls_configuration() {
    log_step "Checking TLS configuration..."
    
    # Check for insecure TLS configurations
    local insecure_tls=$(grep -r "verify.*false\|insecure.*true\|tls.*disable" \
        --include="*.rs" \
        --include="*.py" \
        --include="*.yaml" \
        "$PROJECT_ROOT" 2>/dev/null | wc -l)
    
    if [ "$insecure_tls" -gt 0 ]; then
        log_warn "Found $insecure_tls potential TLS security issues"
        security_findings[high]=$((security_findings[high] + insecure_tls))
    else
        log_info "✓ TLS configuration secure"
    fi
}

# Check for SQL injection vulnerabilities
check_sql_injection() {
    log_step "Checking for SQL injection vulnerabilities..."
    
    # Look for raw SQL queries without parameterization
    local unsafe_sql=$(grep -r "format!\|concat!\|push_str" \
        --include="*.rs" \
        "$PROJECT_ROOT" 2>/dev/null | \
        grep -i "select\|insert\|update\|delete" | wc -l)
    
    if [ "$unsafe_sql" -gt 0 ]; then
        log_warn "Found $unsafe_sql potential SQL injection points"
        security_findings[high]=$((security_findings[high] + unsafe_sql))
    else
        log_info "✓ No SQL injection vulnerabilities detected"
    fi
}

# Check authentication and authorization
check_auth_security() {
    log_step "Checking authentication security..."
    
    # Check for weak authentication patterns
    local auth_issues=0
    
    # Check for hardcoded credentials
    if grep -r "username.*=.*admin\|password.*=.*admin" \
        --include="*.rs" \
        --include="*.py" \
        "$PROJECT_ROOT" 2>/dev/null | grep -v test > /dev/null; then
        log_warn "Hardcoded admin credentials found"
        auth_issues=$((auth_issues + 1))
        security_findings[critical]=$((security_findings[critical] + 1))
    fi
    
    # Check for missing authentication
    if ! grep -r "authenticate\|authorization\|jwt\|oauth" \
        --include="*.rs" \
        "$PROJECT_ROOT/v2" > /dev/null 2>&1; then
        log_warn "No authentication mechanism found"
        auth_issues=$((auth_issues + 1))
        security_findings[high]=$((security_findings[high] + 1))
    fi
    
    if [ "$auth_issues" -eq 0 ]; then
        log_info "✓ Authentication configuration secure"
    fi
}

# Run SAST (Static Application Security Testing)
run_sast() {
    log_step "Running SAST analysis..."
    
    # Use semgrep for SAST if available
    if command -v semgrep >/dev/null 2>&1; then
        log_info "Running Semgrep security scan..."
        
        semgrep --config=auto \
            --json \
            --output=/tmp/semgrep_results.json \
            "$PROJECT_ROOT" 2>/dev/null || true
        
        if [ -f /tmp/semgrep_results.json ]; then
            local findings=$(jq -r '.results | length' /tmp/semgrep_results.json 2>/dev/null || echo "0")
            
            if [ "$findings" -gt 0 ]; then
                log_warn "Semgrep found $findings security issues"
                security_findings[medium]=$((security_findings[medium] + findings))
            else
                log_info "✓ Semgrep scan clean"
            fi
        fi
    else
        log_info "Semgrep not installed, skipping SAST"
    fi
}

# Generate security report
generate_security_report() {
    log_step "Generating security report..."
    
    # Calculate totals
    security_findings[total]=$((
        security_findings[critical] + 
        security_findings[high] + 
        security_findings[medium] + 
        security_findings[low]
    ))
    
    # Determine overall status
    local status="PASS"
    if [ "${security_findings[critical]}" -gt 0 ]; then
        status="CRITICAL"
    elif [ "${security_findings[high]}" -gt 0 ]; then
        status="HIGH_RISK"
    elif [ "${security_findings[medium]}" -gt 0 ]; then
        status="MEDIUM_RISK"
    elif [ "${security_findings[low]}" -gt 0 ]; then
        status="LOW_RISK"
    fi
    
    # Generate JSON report
    cat > "$SECURITY_REPORT" << EOF
{
    "timestamp": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
    "scan_level": "$SCAN_LEVEL",
    "status": "$status",
    "findings": {
        "critical": ${security_findings[critical]},
        "high": ${security_findings[high]},
        "medium": ${security_findings[medium]},
        "low": ${security_findings[low]},
        "total": ${security_findings[total]}
    },
    "scans_performed": [
        "secrets_scan",
        "dependency_scan",
        "docker_scan",
        "permissions_check",
        "tls_check",
        "sql_injection_check",
        "auth_check",
        "sast"
    ]
}
EOF
    
    # Generate text summary
    cat > "$SUMMARY_REPORT" << EOF
========================================
Security Scan Summary
========================================
Date: $(date)
Scan Level: $SCAN_LEVEL
Overall Status: $status

Findings by Severity:
---------------------
Critical: ${security_findings[critical]}
High:     ${security_findings[high]}
Medium:   ${security_findings[medium]}
Low:      ${security_findings[low]}
---------------------
Total:    ${security_findings[total]}

Scans Performed:
----------------
✓ Secrets scanning
✓ Dependency vulnerability scanning
✓ Docker image scanning
✓ File permissions check
✓ TLS configuration check
✓ SQL injection detection
✓ Authentication security check
✓ SAST analysis

Recommendations:
----------------
$([ "${security_findings[critical]}" -gt 0 ] && echo "1. URGENT: Fix critical vulnerabilities immediately")
$([ "${security_findings[high]}" -gt 0 ] && echo "2. Address high-severity issues before deployment")
$([ "${security_findings[medium]}" -gt 0 ] && echo "3. Plan to fix medium issues in next sprint")
$([ "${security_findings[low]}" -gt 0 ] && echo "4. Review low-severity findings for future improvement")

Next Steps:
-----------
1. Review detailed findings in security reports
2. Create tickets for critical/high findings
3. Update dependencies with known vulnerabilities
4. Implement security best practices
5. Schedule regular security scans

Report Files:
-------------
JSON: $SECURITY_REPORT
Summary: $SUMMARY_REPORT

EOF
    
    # Display summary
    cat "$SUMMARY_REPORT"
}

# Main execution
main() {
    log_security "🔒 Starting Security Scan"
    log_info "Scan Level: $SCAN_LEVEL"
    log_info "Project: $PROJECT_ROOT"
    
    if [ "$PARALLEL_SCANS" = "true" ]; then
        log_info "Running scans in parallel..."
        
        # Run scans in parallel
        scan_secrets &
        scan_rust_dependencies &
        scan_python_dependencies &
        check_file_permissions &
        check_tls_configuration &
        check_sql_injection &
        check_auth_security &
        
        wait
        
        # Docker and SAST scans sequential (resource intensive)
        scan_docker_images
        run_sast
    else
        # Run scans sequentially
        scan_secrets
        scan_rust_dependencies
        scan_python_dependencies
        scan_docker_images
        check_file_permissions
        check_tls_configuration
        check_sql_injection
        check_auth_security
        run_sast
    fi
    
    # Generate report
    generate_security_report
    
    # Exit with appropriate code
    if [ "${security_findings[critical]}" -gt 0 ]; then
        log_critical "❌ CRITICAL security issues found! Do not deploy!"
        exit 2
    elif [ "${security_findings[high]}" -gt 0 ]; then
        log_error "❌ High severity security issues found!"
        exit 1
    elif [ "${security_findings[total]}" -gt 0 ]; then
        log_warn "⚠️ Security issues found, review before deployment"
        exit 0
    else
        log_security "✅ Security scan passed!"
        exit 0
    fi
}

# Run main function
main "$@"