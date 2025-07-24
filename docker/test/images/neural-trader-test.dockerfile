# Test neural-trader image with test configurations
FROM rust:latest as builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Create app directory
WORKDIR /app

# Copy workspace files
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY vendor ./vendor
COPY mcp-trading-server ./mcp-trading-server
COPY benches ./benches
COPY tests ./tests
COPY examples ./examples

# Build the application with test features enabled
RUN cargo build --release --bin neural-trader --features="testing,mock-providers"

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies + test tools
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    curl \
    jq \
    python3 \
    python3-pip \
    && rm -rf /var/lib/apt/lists/*

# Install test utilities
RUN pip3 install --no-cache-dir pytest requests faker numpy pandas

# Create non-root user for security
RUN useradd -m -u 1000 -s /bin/bash trader

# Create necessary directories
RUN mkdir -p /etc/neural-trader /var/lib/neural-trader /var/log/neural-trader /test-fixtures \
    && chown -R trader:trader /etc/neural-trader /var/lib/neural-trader /var/log/neural-trader /test-fixtures

# Copy binary from builder
COPY --from=builder /app/target/release/neural-trader /usr/local/bin/neural-trader

# Copy test configuration files
COPY --chown=trader:trader docker/test/configs/neural-trader/ /var/lib/neural-trader/config/

# Copy test utilities and scripts
COPY --chown=trader:trader docker/test/scripts/test-runner.sh /usr/local/bin/test-runner.sh
COPY --chown=trader:trader docker/test/scripts/integration-tests.py /usr/local/bin/integration-tests.py
RUN chmod +x /usr/local/bin/test-runner.sh /usr/local/bin/integration-tests.py

# Switch to non-root user
USER trader

# Set working directory
WORKDIR /var/lib/neural-trader

# Environment variables for testing
ENV RUST_LOG=debug
ENV TESTING_MODE=true
ENV TEST_CONFIG_PATH=/var/lib/neural-trader/config

# Expose MCP server port (different from production)
EXPOSE 8081

# Health check with test mode
HEALTHCHECK --interval=15s --timeout=3s --start-period=30s --retries=5 \
    CMD ["neural-trader", "health", "--test-mode"] || exit 1

# Default command with test mode
CMD ["neural-trader", "--config", "/var/lib/neural-trader/config/test.toml", "--test-mode"]