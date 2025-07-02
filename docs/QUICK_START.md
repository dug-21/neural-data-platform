# Neural Trader Autonomous Platform Quick Start

## Prerequisites

### System Requirements
- Rust 1.70+ (Install from [rustup.rs](https://rustup.rs/))
- Docker and Docker Compose (Install from [docker.com](https://www.docker.com/))
- 8GB RAM minimum (16GB recommended)
- PostgreSQL 13+ (handled via Docker)
- Redis 6+ (handled via Docker)

### Development Tools (Optional)
- Cargo tarpaulin for code coverage
- Your preferred Rust IDE (VS Code with rust-analyzer recommended)

## Installation

### 1. Clone the Repository
```bash
git clone <repository-url>
cd neural-trader
```

### 2. Start Database Services
The platform requires PostgreSQL and Redis. Use Docker Compose to start these services:

```bash
# Start all required services in the background
docker-compose up -d

# Check that services are running
docker-compose ps
```

This will start:
- PostgreSQL database (port 5432)
- Redis cache (port 6379)

### 3. Build the Project
```bash
# Build in release mode for optimal performance
cargo build --release

# Or build in debug mode for development
cargo build
```

### 4. Run the Platform
```bash
# Run the compiled binary
cargo run --release

# Or run in development mode
cargo run
```

## Basic Usage

### Configuration

The platform uses TOML configuration files located in the `config/` directory:

- `config/platform.toml` - Main platform configuration

Example configuration:
```toml
[platform]
name = "neural-trader-autonomous"
version = "0.1.0"

[database]
url = "postgres://neural_trader:neural_trader_pass@localhost/neural_trader_db"
max_connections = 20

[neural]
memory_gb = 1.0
models = ["NHITS", "DeepAR", "TCN", "MLP"]
prediction_cache_ttl = 300
```

### Running Tests

```bash
# Run all tests
cargo test

# Run tests with coverage
cargo tarpaulin --out html
```

### Core Components

The platform consists of several key modules:

1. **Data Module** (`src/data/`) - Data acquisition, caching, and storage
2. **Integration Module** (`src/integration/`) - Neural network integration and predictions
3. **Streaming Module** (`src/streaming/`) - Real-time event processing
4. **Adapters Module** (`src/adapters/`) - External system adapters

### Example Usage

```rust
use autonomous_platform::{PlatformConfig, load_default_config, Result};

#[tokio::main]
async fn main() -> Result<()> {
    // Load configuration
    let config = load_default_config()?;
    
    // Initialize platform components
    // (Implementation details in examples/basic_usage.rs)
    
    Ok(())
}
```

## Next Steps

1. **Read the Architecture Guide** - [`docs/ARCHITECTURE.md`](ARCHITECTURE.md)
2. **Review Examples** - See `examples/` directory for working code samples
3. **Development Setup** - [`docs/DEVELOPMENT.md`](DEVELOPMENT.md)
4. **Deployment Guide** - [`docs/DEPLOYMENT.md`](DEPLOYMENT.md)

## Common Issues

### Port Already in Use
If you get port binding errors:
```bash
# Check what's using the ports
lsof -i :5432  # PostgreSQL
lsof -i :6379  # Redis

# Stop existing Docker containers
docker-compose down
```

### Database Connection Issues
If the platform can't connect to the database:
```bash
# Check container logs
docker-compose logs postgres
docker-compose logs redis

# Restart services
docker-compose restart
```

### Build Issues
If you encounter build errors:
```bash
# Update Rust toolchain
rustup update

# Clean and rebuild
cargo clean && cargo build
```

## Getting Help

- Check [`docs/TROUBLESHOOTING.md`](TROUBLESHOOTING.md) for common issues
- Review the test files in `tests/` for usage examples
- Read the API documentation: `cargo doc --open`

## Performance Notes

- Use `--release` flag for production builds
- The platform automatically configures connection pooling
- Neural model predictions are cached to improve performance
- Metrics are exported on configurable intervals (default: 60 seconds)