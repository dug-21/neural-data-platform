#!/bin/bash
# Pre-commit Hook for Phase 3 Production Validation
# ZERO TOLERANCE FOR INCOMPLETE IMPLEMENTATIONS
#
# This hook runs essential validations before each commit
# Install: cp scripts/validation/pre-commit-hook.sh .git/hooks/pre-commit

set -euo pipefail

# Color codes
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
BOLD='\033[1m'
NC='\033[0m'

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
TEMP_DIR=$(mktemp -d)

print_header() {
    echo -e "${BLUE}${BOLD}"
    echo "🔍 Phase 3 Pre-Commit Validation (ZERO TOLERANCE)"
    echo -e "${NC}"
}

cleanup() {
    rm -rf "$TEMP_DIR"
}

trap cleanup EXIT

log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

log_critical() {
    echo -e "${RED}${BOLD}[CRITICAL]${NC} $1"
}

# Check for forbidden patterns in staged files
check_forbidden_patterns() {
    log_info "Checking for forbidden patterns in staged files..."
    
    # Get list of staged Rust and Python files
    local staged_files
    staged_files=$(git diff --cached --name-only --diff-filter=ACM | grep -E '\.(rs|py)$' | head -100)
    
    if [ -z "$staged_files" ]; then
        log_info "No Rust or Python files staged for commit"
        return 0
    fi
    
    local forbidden_found=false
    
    # Check each staged file
    while IFS= read -r file; do
        if [ ! -f "$file" ]; then
            continue
        fi
        
        log_info "Checking: $file"
        
        # Extract staged content to temporary file
        git show ":$file" > "$TEMP_DIR/$(basename "$file")"
        local temp_file="$TEMP_DIR/$(basename "$file")"
        
        # Define forbidden patterns
        local patterns=(
            'todo!()'
            'unimplemented!()'
            'panic!("not implemented")'
            'panic!("TODO'
            'TODO:'
            'FIXME:'
            'XXX:'
            'HACK:'
            'NotImplementedError'
            'raise NotImplementedError'
            'pass  # TODO'
            'pass # TODO'
            'MockService'
            'FakeService'
            'StubService'
            'mock_'
            'fake_'
            'stub_'
            'return Ok(());  // TODO'
            'return Ok(());// TODO'
            'return Err("not implemented"'
            'return None  # TODO'
            'return None # TODO'
            'println!("DEBUG'
            'print("DEBUG'
            'console\.log\('
            'debugger;'
            'localhost'
            '127\.0\.0\.1'
            'password.*=.*".*"'
            'secret.*=.*".*"'
            'api_key.*=.*".*"'
            'token.*=.*".*"'
        )
        
        # Check for forbidden patterns
        for pattern in "${patterns[@]}"; do
            if grep -qE "$pattern" "$temp_file"; then
                log_critical "Forbidden pattern found in $file: $pattern"
                log_error "$(grep -n "$pattern" "$temp_file" | head -3)"
                forbidden_found=true
            fi
        done
        
        # Check for empty function implementations
        if [[ "$file" == *.rs ]]; then
            # Check for empty Rust functions
            if grep -qE 'fn.*\{\s*\}' "$temp_file"; then
                log_critical "Empty function found in $file"
                forbidden_found=true
            fi
            
            # Check for functions that only return Ok(())
            if grep -qE 'fn.*\{\s*Ok\(\(\)\)\s*\}' "$temp_file"; then
                log_critical "Function returning only Ok(()) found in $file"
                forbidden_found=true
            fi
        elif [[ "$file" == *.py ]]; then
            # Check for empty Python functions
            if grep -qE 'def.*:\s*pass\s*$' "$temp_file"; then
                log_critical "Function with only 'pass' found in $file"
                forbidden_found=true
            fi
        fi
        
    done <<< "$staged_files"
    
    if [ "$forbidden_found" = true ]; then
        log_critical "🚨 COMMIT BLOCKED: Forbidden patterns detected"
        log_error "Fix all issues before committing"
        return 1
    fi
    
    log_success "No forbidden patterns detected"
    return 0
}

# Check code formatting
check_formatting() {
    log_info "Checking code formatting..."
    
    # Check Rust formatting
    local rust_files
    rust_files=$(git diff --cached --name-only --diff-filter=ACM | grep -E '\.rs$' | head -50)
    
    if [ -n "$rust_files" ]; then
        log_info "Checking Rust code formatting..."
        if ! cargo fmt -- --check; then
            log_critical "🚨 COMMIT BLOCKED: Rust code formatting issues detected"
            log_error "Run 'cargo fmt' to fix formatting issues"
            return 1
        fi
        log_success "Rust code formatting is correct"
    fi
    
    # Check Python formatting (if black is available)
    local python_files
    python_files=$(git diff --cached --name-only --diff-filter=ACM | grep -E '\.py$' | head -50)
    
    if [ -n "$python_files" ] && command -v black >/dev/null 2>&1; then
        log_info "Checking Python code formatting..."
        if ! black --check --quiet $python_files; then
            log_critical "🚨 COMMIT BLOCKED: Python code formatting issues detected"
            log_error "Run 'black .' to fix formatting issues"
            return 1
        fi
        log_success "Python code formatting is correct"
    fi
    
    return 0
}

# Check for large files
check_file_sizes() {
    log_info "Checking file sizes..."
    
    local large_files
    large_files=$(git diff --cached --name-only --diff-filter=ACM | xargs -I {} sh -c '[ -f "{}" ] && [ $(stat -c%s "{}") -gt 1048576 ] && echo "{}"')
    
    if [ -n "$large_files" ]; then
        log_warning "Large files detected (>1MB):"
        echo "$large_files"
        
        read -p "Continue with commit? [y/N]: " -n 1 -r
        echo
        if [[ ! $REPLY =~ ^[Yy]$ ]]; then
            log_error "Commit aborted by user"
            return 1
        fi
    fi
    
    return 0
}

# Run basic linting
check_basic_linting() {
    log_info "Running basic linting checks..."
    
    # Check Rust files with clippy (if available)
    local rust_files
    rust_files=$(git diff --cached --name-only --diff-filter=ACM | grep -E '\.rs$' | head -10)
    
    if [ -n "$rust_files" ] && command -v cargo >/dev/null 2>&1; then
        log_info "Running Rust clippy checks..."
        if ! cargo clippy --quiet -- -D warnings; then
            log_warning "Clippy warnings detected (not blocking commit)"
        else
            log_success "Clippy checks passed"
        fi
    fi
    
    return 0
}

# Check commit message format
check_commit_message() {
    local commit_file="$1"
    
    if [ ! -f "$commit_file" ]; then
        return 0
    fi
    
    log_info "Checking commit message format..."
    
    local commit_msg=$(head -n 1 "$commit_file")
    
    # Check conventional commits format
    if [[ ! "$commit_msg" =~ ^(feat|fix|docs|style|refactor|perf|test|chore|security)(\(.+\))?:\ .+ ]]; then
        log_critical "🚨 COMMIT BLOCKED: Invalid commit message format"
        log_error "Commit message must follow conventional commits format:"
        log_error "  type(scope): description"
        log_error "Types: feat, fix, docs, style, refactor, perf, test, chore, security"
        log_error "Current message: $commit_msg"
        return 1
    fi
    
    # Check message length
    if [ ${#commit_msg} -gt 72 ]; then
        log_warning "Commit message is longer than 72 characters"
    fi
    
    log_success "Commit message format is correct"
    return 0
}

# Main execution
main() {
    print_header
    
    cd "$PROJECT_ROOT"
    
    # Run checks
    local checks_failed=false
    
    if ! check_forbidden_patterns; then
        checks_failed=true
    fi
    
    if ! check_formatting; then
        checks_failed=true
    fi
    
    if ! check_file_sizes; then
        checks_failed=true
    fi
    
    check_basic_linting  # This is non-blocking
    
    # Check commit message if we're called as a commit-msg hook
    if [ "${1:-}" = "commit-msg" ]; then
        if ! check_commit_message "$2"; then
            checks_failed=true
        fi
    fi
    
    if [ "$checks_failed" = true ]; then
        echo -e "\n${RED}${BOLD}❌ PRE-COMMIT VALIDATION FAILED${NC}"
        echo -e "${RED}🔧 Fix all issues before committing${NC}"
        echo -e "${YELLOW}💡 Run the full validation with:${NC}"
        echo -e "${YELLOW}   ./scripts/validation/run-production-validation.sh --validator=code-completeness${NC}"
        exit 1
    fi
    
    echo -e "\n${GREEN}${BOLD}✅ PRE-COMMIT VALIDATION PASSED${NC}"
    echo -e "${GREEN}🎯 Code changes look good for commit${NC}"
    
    # Optional: Run quick validation if requested
    if [ "${NEURAL_TRADER_FULL_VALIDATION:-}" = "true" ]; then
        log_info "Running additional validation checks..."
        if command -v "$PROJECT_ROOT/scripts/validation/run-production-validation.sh" >/dev/null 2>&1; then
            "$PROJECT_ROOT/scripts/validation/run-production-validation.sh" --validator=code-completeness --mode=development
        fi
    fi
    
    exit 0
}

# Execute main function
main "$@"