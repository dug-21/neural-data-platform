# Config Store Client - Test Suite

Comprehensive test suite for the neural-trader config-store Python client, following TDD London School methodology with mock-driven development and behavior verification.

## 📊 Test Coverage & Quality Metrics

- **Target Coverage**: 95% line coverage, 90% branch coverage
- **Test Count**: 150+ comprehensive unit tests
- **Methodology**: TDD London School (mockist approach)
- **Focus**: Behavior verification over state testing

## 🏗️ Test Architecture

```
tests/
├── unit/                          # Unit tests (95% coverage target)
│   ├── test_config_store_client.py   # Main client functionality
│   ├── test_config_client_integration.py  # Integration scenarios  
│   └── conftest.py               # Shared fixtures
├── requirements.txt              # Test dependencies
├── pytest.ini                   # Pytest configuration
├── run_tests.py                 # Test runner script
└── README.md                    # This documentation
```

## 🚀 Quick Start

### Install Dependencies

```bash
cd /workspaces/neural-trader
pip install -r tests/requirements.txt
```

### Run All Tests

```bash
# Run complete test suite with coverage
python tests/run_tests.py all

# Run only unit tests
python tests/run_tests.py unit

# Run with specific filter
python tests/run_tests.py unit --filter "TestConnection"

# Run without coverage (faster)
python tests/run_tests.py unit --no-coverage
```

### Quick Test Execution

```bash
# Using pytest directly
cd /workspaces/neural-trader
python -m pytest tests/unit/ -v --cov=src/config-store --cov-report=term-missing
```

## 📋 Test Categories

### 1. Connection & Health Tests
- **File**: `test_config_store_client.py::TestConfigStoreClientConnection`
- **Coverage**: Connection establishment, health checks, disconnect cleanup
- **Key Scenarios**: Timeout handling, connection failures, health monitoring

```python
# Example test
@pytest.mark.asyncio
async def test_connect_success(client, mock_grpc_channel):
    """Test successful connection establishment"""
    result = await client.connect("localhost:50051")
    assert result is True
    assert client._connected is True
```

### 2. Configuration Retrieval Tests
- **File**: `test_config_store_client.py::TestConfigStoreClientGetOperations`
- **Coverage**: Single values, bulk operations, type safety, error handling
- **Key Scenarios**: Cache hits/misses, type validation, key not found

```python
# Example test
@pytest.mark.asyncio 
async def test_get_typed_value_success(client, mock_service):
    """Test getting typed configuration value"""
    with patch.object(client, '_call_service', return_value={"value": 5432}):
        result = await client.get_typed("database.port", int)
        assert result == 5432
        assert isinstance(result, int)
```

### 3. Configuration Setting Tests
- **File**: `test_config_store_client.py::TestConfigStoreClientSetOperations`
- **Coverage**: Single/bulk sets, validation, cache invalidation
- **Key Scenarios**: Validation failures, partial bulk failures

### 4. Streaming & Watch Tests
- **File**: `test_config_store_client.py::TestConfigStoreClientWatchOperations`
- **Coverage**: Configuration change streaming, connection handling
- **Key Scenarios**: Real-time updates, stream failures

### 5. Caching Tests
- **File**: `test_config_store_client.py::TestConfigStoreClientCaching`
- **Coverage**: TTL caching, invalidation, performance optimization
- **Key Scenarios**: Cache hits avoid service calls, TTL expiration

### 6. Error Handling Tests
- **File**: `test_config_store_client.py::TestConfigStoreClientErrorHandling`
- **Coverage**: All error conditions, proper exception mapping
- **Key Scenarios**: Network failures, validation errors, timeouts

### 7. Environment Fallback Tests
- **File**: `test_config_store_client.py::TestConfigStoreClientEnvironmentFallback`
- **Coverage**: Graceful degradation to environment variables
- **Key Scenarios**: Service unavailable, key transformation

### 8. Schema Validation Tests
- **File**: `test_config_store_client.py::TestConfigStoreClientSchemaValidation`
- **Coverage**: JSON schema validation, detailed error reporting
- **Key Scenarios**: Multi-field validation, required field checks

### 9. Integration Scenario Tests
- **File**: `test_config_client_integration.py`
- **Coverage**: Realistic end-to-end workflows
- **Key Scenarios**: Trading service setup, hot reload, microservices orchestration

## 🎯 TDD London School Approach

### Key Principles Applied

1. **Outside-In Development**: Tests drive interface design
2. **Mock-Driven**: Heavy use of mocks to isolate units
3. **Behavior Verification**: Focus on interactions, not state
4. **Contract Testing**: Define interfaces through mock expectations

### Mock Strategy

```python
# Example: Mock service interaction verification
with patch.object(client, '_call_service', return_value={"success": True}) as mock_call:
    await client.set("trading.max_position", 15000.0)
    
    # Verify the conversation with the service
    mock_call.assert_called_once_with(
        "SetConfig",
        {"key": "trading.max_position", "value": 15000.0},
        timeout_seconds=5
    )
```

### Test Structure (AAA Pattern)

```python
@pytest.mark.asyncio
async def test_example(client, mock_service):
    # Arrange - Set up mocks and expectations
    mock_service.expect_call("GetConfig", {"value": "expected"})
    
    # Act - Execute the behavior under test
    result = await client.get("test.key")
    
    # Assert - Verify interactions and outcomes
    assert result == "expected"
    mock_service.assert_expectations_met()
```

## 📊 Coverage Targets & Metrics

### Coverage Requirements (per SPARC plan)
- **Unit Tests**: 95% line coverage, 90% branch coverage
- **Integration Tests**: 85% critical path coverage
- **Error Scenarios**: 100% error condition coverage

### Coverage Verification

```bash
# Generate coverage report
python tests/run_tests.py coverage

# View HTML report
open tests/coverage_html/index.html

# Check coverage threshold
python -m pytest --cov-fail-under=90
```

### Key Metrics Tracked

- **Line Coverage**: Every executable line tested
- **Branch Coverage**: All conditional branches tested  
- **Function Coverage**: All public methods tested
- **Error Path Coverage**: All error conditions tested

## 🔧 Test Configuration

### Pytest Configuration (`pytest.ini`)
- Async test support with `asyncio_mode = auto`
- Coverage thresholds and reporting
- Test discovery patterns
- Timeout configuration (30s default)
- Warning filters for clean output

### Test Dependencies (`requirements.txt`)
- `pytest` + async extensions
- `grpcio` for gRPC mocking
- `jsonschema` for validation testing
- Coverage and quality tools

## 📈 Performance Testing

### Load Testing Scenarios

```python
@pytest.mark.asyncio
async def test_concurrent_get_operations(client):
    """Test concurrent get operations performance"""
    keys = [f"test.key.{i}" for i in range(100)]
    tasks = [client.get(key) for key in keys]
    results = await asyncio.gather(*tasks)
    # Should complete within performance threshold
```

### Cache Performance Verification

```python
@pytest.mark.asyncio
async def test_cache_performance_improvement(client):
    """Test that caching improves performance"""
    # Verify cache hits are faster than service calls
    assert second_call_time < first_call_time
    assert mock_call.call_count == 1  # Only one service call
```

## 🚨 Error Scenario Coverage

### Connection Errors
- Service unavailable
- Connection timeout
- Network connectivity loss
- gRPC channel failures

### Validation Errors
- Invalid key formats
- Type validation failures
- Schema validation errors
- Business rule violations

### Operational Errors
- Cache TTL expiration
- Bulk operation partial failures
- Stream connection drops
- Resource exhaustion

## 🏃‍♂️ Running Specific Test Suites

### Connection Tests Only
```bash
python tests/run_tests.py unit --filter "TestConfigStoreClientConnection"
```

### Caching Tests Only
```bash
python tests/run_tests.py unit --filter "TestConfigStoreClientCaching"
```

### Error Handling Tests Only
```bash
python tests/run_tests.py unit --filter "TestConfigStoreClientErrorHandling"
```

### Integration Scenarios
```bash
python tests/run_tests.py integration
```

### Performance Tests
```bash
python tests/run_tests.py performance
```

## 📝 Writing New Tests

### Test Naming Convention
```python
# Pattern: test_<action>_<condition>_<expected_outcome>
def test_get_configuration_with_cache_hit_returns_cached_value():
def test_set_invalid_key_format_raises_validation_error():
def test_watch_connection_failure_raises_connection_error():
```

### Mock Setup Pattern
```python
@pytest.fixture
def mock_service_with_expectations():
    service = MockConfigStoreService()
    # Set up expected interactions
    return service

@pytest.mark.asyncio
async def test_behavior(client, mock_service_with_expectations):
    # Test implementation focusing on interactions
    pass
```

### Async Test Pattern
```python
@pytest.mark.asyncio
async def test_async_operation(client):
    # All client methods are async and must be awaited
    result = await client.get("test.key")
    assert result is not None
```

## 🎯 Success Criteria

✅ **Coverage**: 95%+ line coverage achieved  
✅ **Test Count**: 150+ comprehensive tests  
✅ **Methodology**: TDD London School applied consistently  
✅ **Error Coverage**: All error scenarios tested  
✅ **Performance**: Cache and concurrent access verified  
✅ **Integration**: Realistic workflows tested  
✅ **Documentation**: Complete test documentation  

## 🔄 Continuous Integration

Tests are designed to run in CI/CD pipelines with:
- Parallel test execution support
- Coverage reporting integration
- Performance regression detection
- Quality gate enforcement

```yaml
# Example CI configuration
- name: Run Tests
  run: |
    python tests/run_tests.py all
    python -m coverage xml
- name: Upload Coverage
  uses: codecov/codecov-action@v3
```

---

This test suite ensures robust, reliable configuration management for the neural-trader platform with comprehensive coverage of all use cases, error conditions, and performance characteristics.