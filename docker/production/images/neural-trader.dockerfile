# Multi-stage build for neural-trader
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

# Build the application in release mode
RUN cargo build --release --bin neural-trader

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user for security
RUN useradd -m -u 1000 -s /bin/bash trader

# Create necessary directories
RUN mkdir -p /etc/neural-trader /var/lib/neural-trader /var/log/neural-trader \
    && chown -R trader:trader /etc/neural-trader /var/lib/neural-trader /var/log/neural-trader

# Copy binary from builder
COPY --from=builder /app/target/release/neural-trader /usr/local/bin/neural-trader

# Copy configuration files
COPY --chown=trader:trader config/ /var/lib/neural-trader/config/

# Switch to non-root user
USER trader

# Set working directory
WORKDIR /var/lib/neural-trader

# Expose MCP server port
EXPOSE 8080

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD ["neural-trader", "health"] || exit 1

# Default command
CMD ["neural-trader"]