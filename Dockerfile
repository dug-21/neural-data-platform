# Multi-stage build for Neural Trader Rust application

# Base builder stage with nightly for edition2024 support
FROM rustlang/rust:nightly AS base

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    libpq-dev \
    cmake \
    g++ \
    && rm -rf /var/lib/apt/lists/*

# Create app directory
WORKDIR /app

# Copy all source files
COPY . .

# Build the application
RUN cargo build --release --bin neural-trader

# Development stage
FROM base AS development

# Install development tools
RUN apt-get update && apt-get install -y \
    gdb \
    strace \
    valgrind \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user for development first
RUN groupadd -r neuraltrader && useradd -r -g neuraltrader neuraltrader

# Copy source for development and set permissions
COPY . .
RUN chown -R neuraltrader:neuraltrader /app
RUN chown -R neuraltrader:neuraltrader /usr/local/cargo

USER neuraltrader

# Build in debug mode for development
RUN cargo build --bin neural-trader

# Expose ports
EXPOSE 3030

# Set development environment
ENV RUST_LOG=debug \
    RUST_BACKTRACE=full

# Development command with hot reload capability
CMD ["cargo", "run", "--bin", "neural-trader"]

# Test stage
FROM base AS test

# Copy all source including tests
COPY . .

# Run tests
RUN cargo test --all

# Testing stage
FROM development AS testing
CMD ["cargo", "test", "--all"]

# Production builder stage  
FROM base AS builder

# Copy source for production build
COPY . .

# Build the application
RUN cargo build --release --bin neural-trader

# Runtime stage
FROM debian:bookworm-slim AS production

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    libpq5 \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN groupadd -r neuraltrader && useradd -r -g neuraltrader neuraltrader

# Create app directory
WORKDIR /app

# Copy binary from builder
COPY --from=builder /app/target/release/neural-trader /app/neural-trader

# Create necessary directories
RUN mkdir -p /app/logs /app/config && \
    chown -R neuraltrader:neuraltrader /app

# Copy configuration files
COPY config/*.toml /app/config/
COPY config/*.yaml /app/config/

# Switch to non-root user
USER neuraltrader

# Expose ports
EXPOSE 3030

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=30s --retries=3 \
    CMD curl -f http://localhost:3030/health || exit 1

# Set environment variables
ENV RUST_LOG=info \
    RUST_BACKTRACE=1

# Run the application
CMD ["/app/neural-trader"]