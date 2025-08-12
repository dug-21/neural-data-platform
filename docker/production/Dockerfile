# Production Dockerfile for Neural Trader with Model Persistence
# Week 3 Implementation: Optimized for production deployment

# Build stage
FROM rust:latest as builder

# Install system dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    libpq-dev \
    cmake \
    build-essential \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Set working directory
WORKDIR /app

# Copy dependency files first for better caching
COPY Cargo.toml Cargo.lock ./
COPY benches/ ./benches/
COPY examples/ ./examples/
COPY vendor/ ./vendor/

# Create dummy workspace member to satisfy cargo workspace
RUN mkdir -p mcp-trading-server/src && \
    echo '[package]\nname = "mcp-trading-server"\nversion = "0.1.0"\nedition = "2021"\n\n[dependencies]' > mcp-trading-server/Cargo.toml && \
    echo "fn main() {}" > mcp-trading-server/src/main.rs

# Create dummy main to build dependencies
RUN mkdir src && echo "fn main() {}" > src/main.rs

# Build dependencies only
RUN cargo build --release --bin neural-trader
RUN rm -rf src target/release/deps/neural_trader*

# Copy actual source code
COPY src/ ./src/
COPY tests/ ./tests/
COPY mcp-trading-server/ ./mcp-trading-server/

# Build the application
RUN cargo build --release --bin neural-trader

# Runtime stage
FROM debian:bullseye-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    libssl1.1 \
    libpq5 \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Create neural-trader user
RUN groupadd -r neural && useradd -r -g neural neural

# Create necessary directories
RUN mkdir -p /opt/neural-trader/{models,checkpoints,backup,exports,logs,config} \
    && chown -R neural:neural /opt/neural-trader

# Copy the binary
COPY --from=builder /app/target/release/neural-trader /usr/local/bin/neural-trader

# Copy configuration files
COPY docker/production/configs/ /opt/neural-trader/config/

# Set ownership and permissions
RUN chown neural:neural /usr/local/bin/neural-trader \
    && chmod +x /usr/local/bin/neural-trader

# Switch to non-root user
USER neural

# Set working directory
WORKDIR /opt/neural-trader

# Environment variables for production
ENV RUST_LOG=info
ENV RUST_BACKTRACE=1
ENV MODEL_STORAGE_PATH=/opt/neural-trader/models
ENV MODEL_BACKUP_PATH=/opt/neural-trader/backup
ENV LOG_LEVEL=info

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=60s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1

# Expose ports
EXPOSE 8080 9090

# Command to run the application
CMD ["neural-trader"]