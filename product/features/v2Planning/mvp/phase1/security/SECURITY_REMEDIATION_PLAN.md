# Security Remediation Plan - Config Store
## SPARC Methodology Implementation

### Overview
This plan addresses 8 security vulnerabilities (3 CRITICAL, 5 HIGH) in the config-store component.
**Authentication is excluded** per requirements - to be addressed separately.

### Vulnerabilities to Address

#### CRITICAL Severity
1. **Secret Storage Prevention** - Block all password/secret storage
2. **Unsafe JSON Deserialization** - Add validation and limits
3. **Path Traversal** - Secure file access paths

#### HIGH Severity
4. **Information Disclosure** - Sanitize error messages
5. **Race Conditions** - Add proper locking
6. **Rate Limiting** - Implement request throttling
7. **Input Validation** - Comprehensive sanitization
8. **Dependencies** - Update vulnerable packages

## Phase 1: Specification

### Requirements
- **R1**: System MUST reject any attempt to store passwords, API keys, or secrets
- **R2**: JSON deserialization MUST validate size, depth, and schema
- **R3**: File paths MUST be canonicalized and restricted to allowed directories
- **R4**: Error messages MUST NOT expose internal paths or stack traces
- **R5**: Async operations MUST be thread-safe with proper locking
- **R6**: Rate limiting MUST prevent DoS attacks
- **R7**: All inputs MUST be validated and sanitized
- **R8**: Dependencies MUST be updated to secure versions

### Success Criteria
- All existing tests continue to pass
- No functional regression
- Security tests validate all fixes
- Performance impact < 5%

## Phase 2: Architecture

### Component Structure
```
config-store/
├── src/
│   ├── security/           # NEW: Security module
│   │   ├── mod.rs
│   │   ├── blocklist.rs    # Secret detection & blocking
│   │   ├── validator.rs    # Enhanced input validation
│   │   ├── rate_limiter.rs # Rate limiting implementation
│   │   └── sanitizer.rs    # Error sanitization
│   ├── loader.rs           # MODIFIED: Secure file loading
│   ├── async_store.rs      # MODIFIED: Thread-safe operations
│   ├── error.rs            # MODIFIED: Sanitized errors
│   └── validator.rs        # ENHANCED: Comprehensive validation
└── tests/
    ├── security_tests.rs    # NEW: Security test suite
    └── integration_tests.rs # MODIFIED: Include security scenarios
```

### Security Layer Design
```
Input → Blocklist Check → Validation → Sanitization → Rate Limit → Store
                ↓                ↓              ↓            ↓
            [Reject]        [Reject]      [Sanitize]    [Throttle]
```

## Phase 3: Implementation Plan (TDD)

### Sprint 1: Critical Vulnerabilities (Day 1-2)

#### Task 1: Secret Blocking System
**Test First:**
```rust
#[test]
fn test_blocks_password_storage() {
    assert!(store.set("password", "secret123").is_err());
    assert!(store.set("api_key", "sk_live_xxx").is_err());
    assert!(store.set("secret", "confidential").is_err());
}
```

**Implementation:**
- Create `security/blocklist.rs` with pattern detection
- Integrate into all set operations
- Block common secret patterns and keywords

#### Task 2: Safe JSON Deserialization
**Test First:**
```rust
#[test]
fn test_rejects_large_json() {
    let huge_json = "a".repeat(11_000_000);
    assert!(parse_json(&huge_json).is_err());
}

#[test]
fn test_rejects_deeply_nested_json() {
    let nested = create_nested_json(200);
    assert!(parse_json(&nested).is_err());
}
```

**Implementation:**
- Add size limits (10MB max)
- Add depth limits (128 levels max)
- Validate before deserialization

#### Task 3: Path Traversal Protection
**Test First:**
```rust
#[test]
fn test_blocks_path_traversal() {
    assert!(loader.load("../../etc/passwd").is_err());
    assert!(loader.load("/etc/passwd").is_err());
    assert!(loader.load("config/../../../secret").is_err());
}
```

**Implementation:**
- Canonicalize all paths
- Whitelist allowed directories
- Reject paths with ".." or absolute paths outside whitelist

### Sprint 2: High Severity Issues (Day 3-4)

#### Task 4: Error Sanitization
**Test First:**
```rust
#[test]
fn test_sanitizes_error_messages() {
    let err = store.get_invalid_path().unwrap_err();
    assert!(!err.to_string().contains("/home/"));
    assert!(!err.to_string().contains("stack trace"));
}
```

**Implementation:**
- Create production vs debug error modes
- Strip sensitive info in production
- Log full errors internally only

#### Task 5: Thread-Safe Async Operations
**Test First:**
```rust
#[tokio::test]
async fn test_concurrent_access_safe() {
    let store = Arc::new(AsyncStore::new());
    let handles = (0..100).map(|i| {
        let s = store.clone();
        tokio::spawn(async move {
            s.set(&format!("key{}", i), "value").await
        })
    });
    
    futures::future::join_all(handles).await;
    assert_eq!(store.len().await, 100);
}
```

**Implementation:**
- Add RwLock for read operations
- Add Mutex for write operations
- Ensure atomic transactions

#### Task 6: Rate Limiting
**Test First:**
```rust
#[test]
fn test_rate_limiting() {
    let limiter = RateLimiter::new(10, Duration::from_secs(1));
    for _ in 0..10 {
        assert!(limiter.check("client1").is_ok());
    }
    assert!(limiter.check("client1").is_err()); // 11th request blocked
}
```

**Implementation:**
- Token bucket algorithm
- Per-client limits
- Configurable thresholds

#### Task 7: Comprehensive Input Validation
**Test First:**
```rust
#[test]
fn test_validates_input() {
    assert!(validate_key("../../../etc").is_err());
    assert!(validate_key("'; DROP TABLE;").is_err());
    assert!(validate_key("<script>").is_err());
    assert!(validate_value(&"x".repeat(10_000_000)).is_err());
}
```

**Implementation:**
- Key format validation (alphanumeric + limited chars)
- Value size limits
- Injection pattern detection
- Special character escaping

#### Task 8: Dependency Updates
**Test First:**
```rust
#[test]
fn test_dependencies_secure() {
    // Ensure no known vulnerabilities
    assert!(check_cargo_audit().is_ok());
}
```

**Implementation:**
- Update redis to 0.25+
- Pin all dependency versions
- Add cargo-audit to CI

## Phase 4: Testing Strategy

### Security Test Suite
1. **Boundary Testing**: Max sizes, depths, lengths
2. **Injection Testing**: SQL, NoSQL, Command, Path
3. **Concurrency Testing**: Race conditions, deadlocks
4. **Performance Testing**: Rate limits, DoS scenarios
5. **Regression Testing**: Ensure functionality preserved

### Test Coverage Goals
- Unit tests: 95%+ coverage
- Integration tests: All security scenarios
- Fuzzing: JSON parser, input validation
- Performance: < 5% overhead

## Phase 5: Rollout Plan

### Deployment Steps
1. Deploy to staging with monitoring
2. Run security scanner
3. Performance testing under load
4. Gradual rollout with feature flags
5. Monitor for security events

### Monitoring & Alerts
- Track blocked secret attempts
- Monitor rate limit hits
- Log validation failures
- Alert on suspicious patterns

## Success Metrics
- Zero security vulnerabilities in next audit
- < 5% performance impact
- 100% backward compatibility
- All tests passing
- No production incidents

## Timeline
- Day 1-2: Critical fixes (Sprint 1)
- Day 3-4: High severity fixes (Sprint 2)
- Day 5: Integration testing & documentation
- Day 6: Security review & deployment prep

## Risk Mitigation
- Feature flags for gradual rollout
- Rollback plan ready
- Monitoring in place before deployment
- Security team review before production