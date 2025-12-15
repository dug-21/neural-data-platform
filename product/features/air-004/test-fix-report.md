# AIR-004: Integration Test Fix Report

## Summary
Successfully fixed 5 failing integration tests in the air-quality-app by correcting how query parameters are passed to `axum-test` test requests.

## Problem Analysis

### Root Cause
The tests were failing with 404 errors because query parameters were being passed incorrectly to the `axum-test` library. The tests were embedding query parameters directly in the URL string:

```rust
// INCORRECT - causes 404
server.get("/api/v1/alerts?location_id=test-loc&time_range=active").await
```

When query parameters are embedded in the URL string passed to `.get()`, `axum-test` (version 14.0) does not parse them correctly, leading to route matching failures and 404 responses.

### Investigation Process
1. Verified router configuration was correct (routes were properly registered)
2. Noticed that routes without query params worked (`test_locations_endpoint`)
3. Discovered `test_invalid_query_params` passed (proving route was registered)
4. Identified discrepancy: same route worked without params, failed with params
5. Researched `axum-test` API and found proper query parameter methods

### Solution
Updated all failing tests to use the correct `axum-test` API method `.add_query_params()`:

```rust
// CORRECT - works properly
server.get("/api/v1/alerts")
    .add_query_params(serde_json::json!({
        "location_id": "test-loc",
        "time_range": "active"
    }))
    .await
```

## Tests Fixed

### 1. test_alerts_endpoint
- **File**: `/workspaces/neural-data-platform/apps/air-quality-app/src/api/routes.rs` (lines 213-231)
- **Change**: Added `.add_query_params()` with `location_id` and `time_range` parameters
- **Status**: PASSING

### 2. test_latest_readings_endpoint_with_data
- **File**: `/workspaces/neural-data-platform/apps/air-quality-app/src/api/routes.rs` (lines 251-310)
- **Change**: Added `.add_query_params()` with `location_id` parameter
- **Status**: PASSING

### 3. test_readings_time_range_query
- **File**: `/workspaces/neural-data-platform/apps/air-quality-app/src/api/routes.rs` (lines 305-381)
- **Change**: Added `.add_query_params()` with `location_id`, `start`, and `end` parameters
- **Removed**: URL encoding logic (no longer needed)
- **Status**: PASSING

### 4. test_aggregate_endpoint_mean
- **File**: `/workspaces/neural-data-platform/apps/air-quality-app/src/api/routes.rs` (lines 383-445)
- **Change**: Added `.add_query_params()` with `location_id`, `start`, `end`, `interval`, and `agg` parameters
- **Removed**: URL encoding logic (no longer needed)
- **Status**: PASSING

### 5. test_forecast_endpoint
- **File**: `/workspaces/neural-data-platform/apps/air-quality-app/src/api/routes.rs` (lines 447-511)
- **Change**: Added `.add_query_params()` with `location_id`, `metric`, and `horizon` parameters
- **Status**: PASSING

## Test Results

### Before Fix
```
test result: FAILED. 54 passed; 5 failed; 0 ignored
```

Failing tests:
- api::routes::tests::test_aggregate_endpoint_mean
- api::routes::tests::test_alerts_endpoint
- api::routes::tests::test_forecast_endpoint
- api::routes::tests::test_latest_readings_endpoint_with_data
- api::routes::tests::test_readings_time_range_query

### After Fix
```
test result: ok. 59 passed; 0 failed; 0 ignored
```

All tests passing, including:
- 10 route integration tests
- 49 other tests across the codebase
- No regressions introduced

## Technical Details

### axum-test Query Parameter API
The `axum-test` crate provides the `.add_query_params()` method on `TestRequest` which:
- Takes a serializable structure (JSON, struct, or tuple array)
- Properly serializes parameters into the request query string
- Ensures correct URL encoding and format
- Maintains compatibility with Axum's `Query<T>` extractor

### Benefits of This Approach
1. **Type Safety**: Using `serde_json::json!` provides compile-time type checking
2. **Automatic Encoding**: No need for manual URL encoding with `urlencoding::encode()`
3. **Cleaner Code**: More readable and maintainable test code
4. **Correctness**: Follows the intended `axum-test` API design

## London TDD Compliance
- Tests define expected behavior (HTTP 200 responses with valid JSON)
- Implementation (router configuration) was already correct
- Fixed test infrastructure to match expected testing patterns
- No changes to production code were needed
- Maintained backward compatibility

## Verification
- All 59 tests in air-quality-app pass
- No regressions in existing tests
- Route handlers function correctly with proper query parameter extraction
- CORS and other middleware continue to work as expected

## References
- [axum-test documentation](https://docs.rs/axum-test/latest/axum_test/)
- [TestRequest::add_query_params()](https://docs.rs/axum-test/latest/axum_test/struct.TestRequest.html)
- Axum Query extraction: https://docs.rs/axum/latest/axum/extract/struct.Query.html

## Files Modified
- `/workspaces/neural-data-platform/apps/air-quality-app/src/api/routes.rs`
  - Updated 5 test functions to use correct query parameter API
  - Removed manual URL encoding logic
  - Total lines changed: ~30

## Success Criteria Met
- All 5 failing tests now pass ✓
- No regression in other tests (54 → 59 all passing) ✓
- Root cause identified and documented ✓
- Solution follows best practices ✓
- Maintains backward compatibility ✓
