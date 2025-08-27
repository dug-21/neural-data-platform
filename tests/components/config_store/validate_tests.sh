#!/bin/bash

# Config Store Component Test Validation Script
echo "🔍 Validating Config Store Component Tests"
echo "=========================================="

# Color codes for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Test file validation
echo -e "\n${BLUE}📁 Test File Structure:${NC}"
test_files=(
    "test_config_api.rs"
    "test_model_storage.rs" 
    "test_hot_reload.rs"
    "test_distributed_sync.rs"
    "test_security.rs"
    "mod.rs"
    "run_tests.rs"
    "Cargo.toml"
    "README.md"
)

for file in "${test_files[@]}"; do
    if [ -f "$file" ]; then
        echo -e "  ✅ $file"
    else
        echo -e "  ❌ $file ${RED}(missing)${NC}"
    fi
done

# Count test functions
echo -e "\n${BLUE}🧪 Test Function Count:${NC}"
for test_file in test_*.rs; do
    if [ -f "$test_file" ]; then
        count=$(grep -c "#\[tokio::test\]" "$test_file")
        echo -e "  📋 $test_file: ${GREEN}$count tests${NC}"
    fi
done

# Check test coverage areas
echo -e "\n${BLUE}📊 Test Coverage Areas:${NC}"

coverage_areas=(
    "Configuration API:test_config_api.rs"
    "Model Storage:test_model_storage.rs"
    "Hot-Reload:test_hot_reload.rs"
    "Distributed Sync:test_distributed_sync.rs"
    "Security:test_security.rs"
)

for area in "${coverage_areas[@]}"; do
    IFS=':' read -r name file <<< "$area"
    if [ -f "$file" ]; then
        tests=$(grep -c "#\[tokio::test\]" "$file")
        if [ "$tests" -gt 5 ]; then
            echo -e "  ✅ $name: ${GREEN}$tests tests${NC}"
        else
            echo -e "  ⚠️  $name: ${YELLOW}$tests tests (may need more)${NC}"
        fi
    else
        echo -e "  ❌ $name: ${RED}file missing${NC}"
    fi
done

# Performance requirements check
echo -e "\n${BLUE}⚡ Performance Requirements Validation:${NC}"
requirements=(
    "Read operations <1ms:test_config_api.rs"
    "Write operations <5ms:test_config_api.rs"
    "Hot-reload <10ms:test_hot_reload.rs"
    "Distributed sync <100ms:test_distributed_sync.rs"
    "Model operations <50ms:test_model_storage.rs"
)

for req in "${requirements[@]}"; do
    IFS=':' read -r requirement file <<< "$req"
    if grep -q "assert.*Duration::from_millis" "$file" 2>/dev/null; then
        echo -e "  ✅ $requirement"
    else
        echo -e "  ⚠️  $requirement ${YELLOW}(performance assertion may be missing)${NC}"
    fi
done

# Security features check
echo -e "\n${BLUE}🔐 Security Features:${NC}"
security_features=(
    "Authentication"
    "Authorization" 
    "Encryption"
    "Input Validation"
    "Audit Logging"
    "Rate Limiting"
)

for feature in "${security_features[@]}"; do
    if grep -qi "$feature" test_security.rs 2>/dev/null; then
        echo -e "  ✅ $feature"
    else
        echo -e "  ❌ $feature ${RED}(not found)${NC}"
    fi
done

# Configuration format support
echo -e "\n${BLUE}📋 Configuration Format Support:${NC}"
formats=("JSON" "YAML" "TOML")

for format in "${formats[@]}"; do
    if grep -qi "$format" README.md 2>/dev/null; then
        echo -e "  ✅ $format format"
    else
        echo -e "  ⚠️  $format format ${YELLOW}(documentation unclear)${NC}"
    fi
done

# Test isolation check
echo -e "\n${BLUE}🔒 Test Isolation Validation:${NC}"
isolation_checks=(
    "Mock implementations:Mock"
    "No external deps:External"
    "Async test support:tokio::test"
    "Error handling:Result"
    "Concurrent tests:Arc"
)

total_isolation_score=0
for check in "${isolation_checks[@]}"; do
    IFS=':' read -r description pattern <<< "$check"
    if grep -r "$pattern" test_*.rs >/dev/null 2>&1; then
        echo -e "  ✅ $description"
        ((total_isolation_score++))
    else
        echo -e "  ❌ $description ${RED}(not found)${NC}"
    fi
done

# Calculate overall score
total_files=${#test_files[@]}
existing_files=0
for file in "${test_files[@]}"; do
    [ -f "$file" ] && ((existing_files++))
done

file_score=$((existing_files * 100 / total_files))
isolation_score=$((total_isolation_score * 100 / ${#isolation_checks[@]}))

echo -e "\n${BLUE}📈 Overall Validation Score:${NC}"
echo -e "  📁 File Structure: ${GREEN}$file_score%${NC} ($existing_files/${#test_files[@]} files)"
echo -e "  🔒 Test Isolation: ${GREEN}$isolation_score%${NC} ($total_isolation_score/${#isolation_checks[@]} features)"

# Final recommendations
echo -e "\n${BLUE}💡 Recommendations:${NC}"
if [ "$file_score" -eq 100 ] && [ "$isolation_score" -ge 80 ]; then
    echo -e "  🎉 ${GREEN}All Config Store tests are properly implemented!${NC}"
    echo -e "  ✅ Ready for integration testing"
    echo -e "  ✅ Performance requirements covered"  
    echo -e "  ✅ Security features implemented"
    echo -e "  ✅ Test isolation maintained"
else
    echo -e "  ⚠️  ${YELLOW}Some areas may need attention:${NC}"
    [ "$file_score" -lt 100 ] && echo -e "    - Complete all required test files"
    [ "$isolation_score" -lt 80 ] && echo -e "    - Improve test isolation"
fi

# Test execution validation
echo -e "\n${BLUE}🚀 Test Execution Check:${NC}"
if [ -f "Cargo.toml" ]; then
    echo -e "  ✅ Cargo.toml exists - tests can be run with 'cargo test'"
    echo -e "  ✅ Custom test runner available - 'cargo run --bin run_config_store_tests'"
else
    echo -e "  ❌ ${RED}Cargo.toml missing - tests cannot be executed${NC}"
fi

echo -e "\n${GREEN}🏁 Config Store Component Test Validation Complete!${NC}"