#!/bin/bash

# Neural Trader Security Check Script
# Validates security configuration before deployment

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Security check results
SECURITY_SCORE=0
TOTAL_CHECKS=0
CRITICAL_ISSUES=0
HIGH_ISSUES=0
MEDIUM_ISSUES=0
LOW_ISSUES=0

# Function to log messages with severity
log_critical() {
    echo -e "${RED}[CRITICAL]${NC} $*"
    ((CRITICAL_ISSUES++))
}

log_high() {
    echo -e "${RED}[HIGH]${NC} $*"
    ((HIGH_ISSUES++))
}

log_medium() {
    echo -e "${YELLOW}[MEDIUM]${NC} $*"
    ((MEDIUM_ISSUES++))
}

log_low() {
    echo -e "${YELLOW}[LOW]${NC} $*"
    ((LOW_ISSUES++))
}

log_pass() {
    echo -e "${GREEN}[PASS]${NC} $*"
    ((SECURITY_SCORE++))
}

log_info() {
    echo -e "${BLUE}[INFO]${NC} $*"
}

# Function to increment total checks
check_count() {
    ((TOTAL_CHECKS++))
}

# Function to check if file exists in git
file_in_git() {
    git ls-files --error-unmatch "$1" >/dev/null 2>&1
}

# Security Check 1: Environment Files
check_environment_files() {
    log_info "Checking environment file security..."
    check_count
    
    # Check if .env is in version control
    if [[ -f "$PROJECT_ROOT/.env" ]] && file_in_git "$PROJECT_ROOT/.env"; then
        log_critical ".env file found in version control - IMMEDIATE SECURITY RISK!"
        log_critical "Action required: git rm .env && echo '.env' >> .gitignore"
    else
        log_pass ".env file not in version control"
    fi
    
    check_count
    # Check if .env.example exists
    if [[ -f "$PROJECT_ROOT/.env.example" ]] || [[ -f "$PROJECT_ROOT/.env.example.secure" ]]; then
        log_pass ".env.example template exists"
    else
        log_medium ".env.example template missing"
    fi
    
    check_count
    # Check .gitignore for .env
    if grep -q "^\.env$" "$PROJECT_ROOT/.gitignore" 2>/dev/null; then
        log_pass ".env properly ignored in .gitignore"
    else
        log_high ".env not found in .gitignore"
    fi
}

# Security Check 2: Hardcoded Secrets
check_hardcoded_secrets() {
    log_info "Checking for hardcoded secrets..."
    
    check_count
    # Check for hardcoded passwords in docker-compose files
    if grep -r "password.*=" "$PROJECT_ROOT"/docker-compose*.yml | grep -v "PASSWORD_FILE" | grep -v "example" >/dev/null 2>&1; then
        log_high "Hardcoded passwords found in docker-compose files"
        echo "Found:"
        grep -r "password.*=" "$PROJECT_ROOT"/docker-compose*.yml | grep -v "PASSWORD_FILE" | grep -v "example"
    else
        log_pass "No hardcoded passwords in docker-compose files"
    fi
    
    check_count
    # Check for API keys in source code
    if find "$PROJECT_ROOT" -name "*.rs" -o -name "*.py" -o -name "*.js" -o -name "*.yml" -o -name "*.yaml" | \
       xargs grep -l "api_key\|API_KEY\|secret_key\|SECRET_KEY" | \
       xargs grep -v "API_KEY_FILE\|SECRET_KEY_FILE\|your_key_here\|CHANGE_ME" >/dev/null 2>&1; then
        log_medium "Potential API keys found in source code"
        echo "Check these files:"
        find "$PROJECT_ROOT" -name "*.rs" -o -name "*.py" -o -name "*.js" -o -name "*.yml" -o -name "*.yaml" | \
        xargs grep -l "api_key\|API_KEY\|secret_key\|SECRET_KEY" | \
        xargs grep -v "API_KEY_FILE\|SECRET_KEY_FILE\|your_key_here\|CHANGE_ME"
    else
        log_pass "No hardcoded API keys found"
    fi
}

# Security Check 3: Docker Security
check_docker_security() {
    log_info "Checking Docker security configuration..."
    
    check_count
    # Check for non-root user in Dockerfiles
    if grep -r "USER.*root" "$PROJECT_ROOT"/Dockerfile* "$PROJECT_ROOT"/docker/ 2>/dev/null; then
        log_high "Root user found in Docker configurations"
    else
        log_pass "Non-root users configured in Docker"
    fi
    
    check_count
    # Check for security options in docker-compose
    if grep -r "security_opt\|no-new-privileges" "$PROJECT_ROOT"/docker-compose*.yml >/dev/null 2>&1; then
        log_pass "Security options configured in docker-compose"
    else
        log_medium "Security options missing in docker-compose files"
    fi
    
    check_count
    # Check for capability dropping
    if grep -r "cap_drop" "$PROJECT_ROOT"/docker-compose*.yml >/dev/null 2>&1; then
        log_pass "Capability dropping configured"
    else
        log_medium "Capability dropping not configured"
    fi
    
    check_count
    # Check for read-only filesystem
    if grep -r "read_only.*true" "$PROJECT_ROOT"/docker-compose*.yml >/dev/null 2>&1; then
        log_pass "Read-only filesystems configured"
    else
        log_low "Read-only filesystems not configured"
    fi
}

# Security Check 4: Network Security
check_network_security() {
    log_info "Checking network security..."
    
    check_count
    # Check for network isolation
    if grep -r "internal.*true" "$PROJECT_ROOT"/docker-compose*.yml >/dev/null 2>&1; then
        log_pass "Network isolation configured"
    else
        log_medium "Network isolation not configured"
    fi
    
    check_count
    # Check for exposed ports
    local exposed_ports=$(grep -r "ports:" "$PROJECT_ROOT"/docker-compose*.yml | wc -l)
    if [[ $exposed_ports -gt 10 ]]; then
        log_medium "Many ports exposed - review necessity"
    else
        log_pass "Reasonable number of exposed ports"
    fi
    
    check_count
    # Check for SSL/TLS configuration
    if grep -r "ssl\|tls\|443" "$PROJECT_ROOT"/docker-compose*.yml >/dev/null 2>&1; then
        log_pass "SSL/TLS configuration found"
    else
        log_high "SSL/TLS configuration missing"
    fi
}

# Security Check 5: Secrets Management
check_secrets_management() {
    log_info "Checking secrets management..."
    
    check_count
    # Check for Docker secrets configuration
    if grep -r "secrets:" "$PROJECT_ROOT"/docker-compose*.yml >/dev/null 2>&1; then
        log_pass "Docker secrets configured"
    else
        log_high "Docker secrets not configured"
    fi
    
    check_count
    # Check for secrets directory
    if [[ -d "$PROJECT_ROOT/secrets" ]]; then
        log_pass "Secrets directory exists"
        
        # Check secrets directory permissions
        local perms=$(stat -c "%a" "$PROJECT_ROOT/secrets" 2>/dev/null || stat -f "%Lp" "$PROJECT_ROOT/secrets" 2>/dev/null)
        if [[ "$perms" == "700" ]]; then
            log_pass "Secrets directory has correct permissions (700)"
        else
            log_medium "Secrets directory permissions should be 700 (current: $perms)"
        fi
    else
        log_medium "Secrets directory not found"
    fi
    
    check_count
    # Check for environment-specific secret files
    if find "$PROJECT_ROOT" -name "*password*.txt" -o -name "*secret*.txt" -o -name "*key*.txt" 2>/dev/null | grep -q .; then
        log_pass "Secret files found"
    else
        log_medium "No secret files found"
    fi
}

# Security Check 6: Monitoring and Logging
check_monitoring_security() {
    log_info "Checking monitoring and logging security..."
    
    check_count
    # Check for security monitoring
    if grep -r "security\|audit" "$PROJECT_ROOT"/docker/grafana/ "$PROJECT_ROOT"/docker/prometheus/ 2>/dev/null; then
        log_pass "Security monitoring configured"
    else
        log_low "Security monitoring not configured"
    fi
    
    check_count
    # Check for log retention
    if grep -r "max-file\|max-size" "$PROJECT_ROOT"/docker-compose*.yml >/dev/null 2>&1; then
        log_pass "Log retention configured"
    else
        log_medium "Log retention not configured"
    fi
    
    check_count
    # Check for health checks
    if grep -r "healthcheck" "$PROJECT_ROOT"/docker-compose*.yml >/dev/null 2>&1; then
        log_pass "Health checks configured"
    else
        log_medium "Health checks missing"
    fi
}

# Security Check 7: Backup Security
check_backup_security() {
    log_info "Checking backup security..."
    
    check_count
    # Check for backup encryption
    if grep -r "encryption\|gpg\|encrypt" "$PROJECT_ROOT"/docker/scripts/ "$PROJECT_ROOT"/scripts/ 2>/dev/null; then
        log_pass "Backup encryption configured"
    else
        log_high "Backup encryption not configured"
    fi
    
    check_count
    # Check for backup rotation
    if grep -r "retention\|rotate\|cleanup" "$PROJECT_ROOT"/docker/scripts/ "$PROJECT_ROOT"/scripts/ 2>/dev/null; then
        log_pass "Backup rotation configured"
    else
        log_medium "Backup rotation not configured"
    fi
}

# Security Check 8: Image Security
check_image_security() {
    log_info "Checking container image security..."
    
    check_count
    # Check for base image versions
    if grep -r "FROM.*latest" "$PROJECT_ROOT"/Dockerfile* "$PROJECT_ROOT"/docker/ 2>/dev/null; then
        log_medium "Using 'latest' tags for base images - consider specific versions"
    else
        log_pass "Using specific versions for base images"
    fi
    
    check_count
    # Check for multi-stage builds
    if grep -r "FROM.*as" "$PROJECT_ROOT"/Dockerfile* 2>/dev/null; then
        log_pass "Multi-stage builds configured"
    else
        log_low "Multi-stage builds not used"
    fi
    
    check_count
    # Check for package updates
    if grep -r "apt-get update.*upgrade\|apk update.*upgrade" "$PROJECT_ROOT"/Dockerfile* 2>/dev/null; then
        log_pass "Package updates in Dockerfiles"
    else
        log_medium "Package updates not found in Dockerfiles"
    fi
}

# Security Check 9: Access Control
check_access_control() {
    log_info "Checking access control..."
    
    check_count
    # Check for authentication configuration
    if grep -r "auth\|authentication\|login" "$PROJECT_ROOT"/docker-compose*.yml >/dev/null 2>&1; then
        log_pass "Authentication configuration found"
    else
        log_high "Authentication configuration missing"
    fi
    
    check_count
    # Check for CORS configuration
    if grep -r "cors\|CORS" "$PROJECT_ROOT" --exclude-dir=target --exclude-dir=node_modules >/dev/null 2>&1; then
        log_pass "CORS configuration found"
    else
        log_medium "CORS configuration not found"
    fi
    
    check_count
    # Check for rate limiting
    if grep -r "rate.*limit\|throttle" "$PROJECT_ROOT" --exclude-dir=target --exclude-dir=node_modules >/dev/null 2>&1; then
        log_pass "Rate limiting configuration found"
    else
        log_medium "Rate limiting not configured"
    fi
}

# Security Check 10: Production Readiness
check_production_readiness() {
    log_info "Checking production readiness..."
    
    check_count
    # Check for production docker-compose
    if [[ -f "$PROJECT_ROOT/docker-compose.prod.yml" ]] || [[ -f "$PROJECT_ROOT/docker-compose.secure.yml" ]]; then
        log_pass "Production docker-compose configuration exists"
    else
        log_high "Production docker-compose configuration missing"
    fi
    
    check_count
    # Check for resource limits
    if grep -r "resources:\|limits:\|memory:\|cpus:" "$PROJECT_ROOT"/docker-compose*.yml >/dev/null 2>&1; then
        log_pass "Resource limits configured"
    else
        log_medium "Resource limits not configured"
    fi
    
    check_count
    # Check for restart policies
    if grep -r "restart:" "$PROJECT_ROOT"/docker-compose*.yml >/dev/null 2>&1; then
        log_pass "Restart policies configured"
    else
        log_medium "Restart policies not configured"
    fi
}

# Function to generate security report
generate_report() {
    echo ""
    echo "=========================================="
    echo "        SECURITY AUDIT SUMMARY"
    echo "=========================================="
    echo ""
    
    local score_percentage=$((SECURITY_SCORE * 100 / TOTAL_CHECKS))
    
    echo "Overall Security Score: $SECURITY_SCORE/$TOTAL_CHECKS ($score_percentage%)"
    echo ""
    
    echo "Issues by Severity:"
    echo "  Critical: $CRITICAL_ISSUES"
    echo "  High:     $HIGH_ISSUES"
    echo "  Medium:   $MEDIUM_ISSUES"
    echo "  Low:      $LOW_ISSUES"
    echo ""
    
    # Security rating
    if [[ $CRITICAL_ISSUES -gt 0 ]]; then
        echo -e "Security Rating: ${RED}CRITICAL - IMMEDIATE ACTION REQUIRED${NC}"
        echo "❌ Do not deploy to production until critical issues are resolved"
    elif [[ $HIGH_ISSUES -gt 0 ]]; then
        echo -e "Security Rating: ${RED}HIGH RISK${NC}"
        echo "⚠️  Address high-priority issues before production deployment"
    elif [[ $MEDIUM_ISSUES -gt 3 ]]; then
        echo -e "Security Rating: ${YELLOW}MEDIUM RISK${NC}"
        echo "⚠️  Consider addressing medium-priority issues"
    elif [[ $score_percentage -ge 80 ]]; then
        echo -e "Security Rating: ${GREEN}GOOD${NC}"
        echo "✅ Ready for production deployment"
    else
        echo -e "Security Rating: ${YELLOW}FAIR${NC}"
        echo "⚠️  Consider improving security posture"
    fi
    
    echo ""
    echo "Recommendations:"
    
    if [[ $CRITICAL_ISSUES -gt 0 ]]; then
        echo "1. Remove any secrets from version control immediately"
        echo "2. Rotate all exposed credentials"
        echo "3. Implement proper secrets management"
    fi
    
    if [[ $HIGH_ISSUES -gt 0 ]]; then
        echo "4. Configure SSL/TLS for all external connections"
        echo "5. Implement authentication and authorization"
        echo "6. Set up proper backup encryption"
    fi
    
    if [[ $MEDIUM_ISSUES -gt 0 ]]; then
        echo "7. Configure network isolation"
        echo "8. Set up comprehensive monitoring"
        echo "9. Implement security scanning in CI/CD"
    fi
    
    echo ""
    echo "Next Steps:"
    echo "1. Review and address all critical and high-priority issues"
    echo "2. Run security scan again to verify fixes"
    echo "3. Consider penetration testing for production"
    echo "4. Set up continuous security monitoring"
    
    # Return appropriate exit code
    if [[ $CRITICAL_ISSUES -gt 0 ]]; then
        return 2
    elif [[ $HIGH_ISSUES -gt 0 ]]; then
        return 1
    else
        return 0
    fi
}

# Main function
main() {
    echo "Neural Trader Security Assessment"
    echo "================================="
    echo ""
    
    cd "$PROJECT_ROOT"
    
    # Run all security checks
    check_environment_files
    check_hardcoded_secrets
    check_docker_security
    check_network_security
    check_secrets_management
    check_monitoring_security
    check_backup_security
    check_image_security
    check_access_control
    check_production_readiness
    
    # Generate final report
    generate_report
}

# Run main function
main "$@"