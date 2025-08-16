# Emergency Test Infrastructure Guide

## 🚨 Quick Start

The emergency test infrastructure provides immediate protection for refactoring the neural-trader codebase without fixing the 487+ compilation errors in the existing test suite.

### Running Tests

```bash
# Run all emergency tests
./scripts/run-emergency-tests.sh

# Run specific test categories
./scripts/run-emergency-tests.sh health    # System health only
./scripts/run-emergency-tests.sh trading   # Trading decisions
./scripts/run-emergency-tests.sh data      # Data pipeline
./scripts/run-emergency-tests.sh neural    # Neural models
./scripts/run-emergency-tests.sh quick     # Quick health check only

# Watch mode (runs every 10 seconds)
./scripts/run-emergency-tests.sh watch
```

### Manual Test Execution

```bash
# Navigate to emergency test directory
cd tests/emergency

# Run all tests
cargo test --release -- --test-threads=1 --nocapture

# Run specific test
cargo test test_system_health --release

# Run with custom database
DATABASE_URL=postgres://user:pass@host/db cargo test
```

## 📋 Test Coverage

### 1. System Health (`test_health.rs`)
- ✅ Health endpoint availability
- ✅ Component status checks
- ✅ Database connectivity
- ✅ Redis connectivity
- ✅ Process monitoring
- ✅ Log file analysis

### 2. Trading Flow (`test_trading.rs`)
- ✅ Trading decision generation
- ✅ Market data submission
- ✅ Risk limit enforcement
- ✅ Symbol processing

### 3. Data Pipeline (`test_data.rs`)
- ✅ Data insertion verification
- ✅ Data retrieval validation
- ✅ Hourly aggregation checks
- ✅ TimescaleDB continuous aggregates
- ✅ Data integrity validation

### 4. Neural Models (`test_neural.rs`)
- ✅ Model file persistence
- ✅ Prediction API functionality
- ✅ Sector model structure
- ✅ Model size validation
- ✅ Output validity checks

## 🔧 Environment Configuration

### Required Environment Variables

```bash
# Database connection
export DATABASE_URL="postgres://postgres:postgres@localhost:5432/neural_trader_db"

# Redis connection
export REDIS_URL="redis://localhost:6379"

# API endpoint (optional, defaults to localhost:8080)
export API_URL="http://localhost:8080"
```

### Docker Testing

```bash
# Start test dependencies
docker-compose -f docker-compose.test.yml up -d

# Wait for services
sleep 30

# Run tests
./scripts/run-emergency-tests.sh

# Cleanup
docker-compose -f docker-compose.test.yml down
```

## 📊 Test Output Interpretation

### Success Indicators
```
✅ Test passed - System functioning correctly
```

### Warning Indicators
```
⚠️  Test skipped - Component unavailable but not critical
ℹ️  Information - Non-critical observation
```

### Failure Indicators
```
❌ Test failed - Critical issue detected
```

### Example Output
```
🚨 EMERGENCY TEST SUITE 🚨
==========================

🏥 Running Health Tests...
  ✅ Health endpoint responding
    - Healthy: true
    - Models loaded: 5
    - Database: ✅
    - Redis: ✅

📊 Running Data Pipeline Tests...
  ✅ Connected to database
  ✅ Test data inserted (60 records)
  ✅ Data stored successfully
  ✅ Data validation passed

🧠 Running Neural Model Tests...
  ✅ Found 5 model files
  ✅ Prediction API working
    - Predictions: 5 values
    - Confidence: 87.50%

💹 Running Trading Flow Tests...
  ✅ Market data submitted
  ✅ Decision received: hold (confidence: 65.00%)

📈 SUMMARY
===========
Passed:  4
Failed:  0
Skipped: 0

✅ All critical tests passed!
System is safe for refactoring.
```

## 🛠️ Troubleshooting

### Common Issues

#### 1. Database Connection Failed
```bash
# Check PostgreSQL is running
docker ps | grep postgres

# Check connection string
psql $DATABASE_URL -c "SELECT 1"
```

#### 2. Redis Connection Failed
```bash
# Check Redis is running
docker ps | grep redis

# Test connection
redis-cli ping
```

#### 3. API Not Responding
```bash
# Check if neural-trader is running
pgrep -f neural-trader

# Check logs
tail -f /var/log/neural-trader/neural-trader.log
```

#### 4. Compilation Errors
```bash
# Clean and rebuild
cd tests/emergency
cargo clean
cargo build --release
```

## 🔄 CI/CD Integration

### GitHub Actions

```yaml
# .github/workflows/emergency-tests.yml
name: Emergency Tests
on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2
      
      - name: Setup Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
          
      - name: Start services
        run: docker-compose up -d
        
      - name: Wait for services
        run: sleep 30
        
      - name: Run emergency tests
        run: ./scripts/run-emergency-tests.sh
        
      - name: Cleanup
        if: always()
        run: docker-compose down
```

## 📈 Next Steps

### After Emergency Tests Pass

1. **Begin Refactoring** - Safe to modify code with test protection
2. **Add More Tests** - Expand coverage incrementally
3. **Fix Legacy Tests** - Gradually repair the 487+ broken tests
4. **Convert to Unit Tests** - Move from integration to unit testing

### Test Expansion Priority

1. **High Priority**
   - Order execution flow
   - Position management
   - Risk calculations
   - Model training triggers

2. **Medium Priority**
   - Configuration loading
   - Performance thresholds
   - Error recovery
   - API authentication

3. **Low Priority**
   - Edge cases
   - Concurrent operations
   - Load testing
   - Chaos testing

## 📝 Maintenance

### Adding New Tests

1. Create test file in `tests/emergency/`
2. Add module to `test_all.rs`
3. Update runner script if needed
4. Document in this guide

### Test Review Schedule

- **Daily**: Run quick health check
- **Before Refactoring**: Run full suite
- **After Changes**: Run relevant category
- **Weekly**: Review and expand tests

## 🎯 Success Metrics

- **2-3 hours**: Time to initial protection
- **4 tests**: Minimum critical coverage
- **< 2 minutes**: Full test execution time
- **0 dependencies**: On broken test infrastructure

---

*Emergency Test Infrastructure v1.0*  
*Created to enable safe refactoring without fixing 487+ broken tests*