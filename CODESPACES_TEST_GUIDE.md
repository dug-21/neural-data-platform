# Running Neural Trader in GitHub Codespaces

## 🚀 Quick Start (Test Mode)

### Option 1: Run Everything in Codespaces (Recommended)

1. **Set up test environment variables:**
```bash
# Create a test .env file with default values
cat > .env.test << EOF
# Database
POSTGRES_PASSWORD=testpass123
POSTGRES_USER=neural_trader
POSTGRES_DB=neural_trader_db
DATABASE_URL=postgresql://neural_trader:testpass123@localhost:5432/neural_trader_db

# Redis
REDIS_PASSWORD=testredis123
REDIS_URL=redis://:testredis123@localhost:6379

# API Keys (use test/demo keys or leave empty for testing)
ALPHA_VANTAGE_API_KEY=demo
POLYGON_API_KEY=test_key
FINNHUB_API_KEY=test_key

# Admin passwords
PGADMIN_DEFAULT_PASSWORD=admin123
GRAFANA_ADMIN_PASSWORD=admin123
EOF
```

2. **Start the services with Docker:**
```bash
# Start all services
sudo docker-compose --env-file .env.test up -d

# Check status
sudo docker-compose ps

# View logs
sudo docker-compose logs -f
```

3. **Initialize the database:**
```bash
# Wait for services to start (about 30 seconds)
sleep 30

# Run migrations
cargo run --bin migration

# Load some test data (optional)
cargo test --test seed_test_data
```

4. **Start the MCP server:**
```bash
# In a new terminal
cargo run --bin mcp_server
```

5. **Test with Claude:**
Now you can interact with your system through Claude using the MCP tools!

### Option 2: Minimal Test Setup (Without Full Docker Stack)

If you want to test just the core functionality:

1. **Use SQLite for testing (no Docker needed):**
```bash
# Create test config
cat > config/test.toml << EOF
[platform]
name = "neural-trader-test"
version = "0.1.0"

[database]
url = "sqlite://test.db"
max_connections = 5

[redis]
url = "redis://localhost:6379"
max_connections = 10

[neural]
memory_gb = 1.0
models = ["MLP"]
device = "cpu"
EOF
```

2. **Run tests directly:**
```bash
# Run unit tests
cargo test

# Run integration tests
cargo test --test '*' --features test-mode
```

### Option 3: Using Podman on Your Local Machine

Since you have Podman, you can also:

1. **Export from Codespaces:**
```bash
# Build the application
cargo build --release

# Create a deployment package
tar -czf neural-trader.tar.gz \
  target/release/neural-trader \
  target/release/mcp_server \
  config/ \
  docker-compose.yml \
  scripts/
```

2. **On your local machine with Podman:**
```bash
# Extract the package
tar -xzf neural-trader.tar.gz

# Convert docker-compose to podman
podman-compose up -d

# Or run individual containers
podman run -d --name timescaledb \
  -e POSTGRES_PASSWORD=testpass123 \
  -p 5432:5432 \
  timescale/timescaledb:latest-pg15

podman run -d --name redis \
  -e REDIS_PASSWORD=testredis123 \
  -p 6379:6379 \
  redis:7-alpine
```

## 📊 Testing the MCP Integration

### Quick Smoke Test:
```bash
# Test database connection
psql -h localhost -U neural_trader -d neural_trader_db -c "SELECT 1;"

# Test Redis
redis-cli -a testredis123 ping

# Test MCP tools
curl -X POST http://localhost:8080/mcp/tools/system_status \
  -H "Content-Type: application/json" \
  -d '{"detailed": true}'
```

### Using the Development Dashboard:
```bash
# Open in Codespaces
python -m http.server 8000

# Then open: http://localhost:8000/dev-dashboard.html
```

## 🛠️ Troubleshooting

### Issue: Docker permission denied
```bash
# Add sudo to all docker commands
sudo docker-compose up -d
```

### Issue: Port already in use
```bash
# Check what's using the port
sudo lsof -i :5432

# Stop conflicting service
sudo systemctl stop postgresql
```

### Issue: Out of memory
```bash
# Reduce service memory limits in docker-compose.yml
# Or use the 'test' profile:
sudo docker-compose --profile test up -d
```

## 🎯 What to Test

1. **Market Data Query:**
   - Insert test data into TimescaleDB
   - Query through MCP tools

2. **Cache Operations:**
   - Store test data in Redis
   - Retrieve through MCP tools

3. **Mock Predictions:**
   - The neural models return simulated predictions
   - Perfect for testing the integration

4. **Agent Decisions:**
   - Test with different market scenarios
   - Verify risk assessment logic

## 💡 Tips for Codespaces

- Codespaces has 4 CPU cores and 8GB RAM by default
- Use `htop` to monitor resource usage
- Services auto-start when you reopen the Codespace
- Your work persists between sessions
- Forward ports using the Ports panel in VS Code

## 🚢 Ready for Production?

Once tested in Codespaces, you can:
1. Build production Docker images
2. Deploy to Kubernetes/ECS/etc
3. Use managed databases (RDS, ElastiCache)
4. Set up proper secrets management

For now, Codespaces is perfect for development and testing!