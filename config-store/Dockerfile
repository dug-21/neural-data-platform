# Build stage
FROM rust:1.87-slim as builder

# Install system dependencies
RUN apt-get update && apt-get install -y \
    protobuf-compiler \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

# Create app directory
WORKDIR /usr/src/app

# Copy Cargo files
COPY Cargo.toml Cargo.lock ./

# Copy source code
COPY src ./src
COPY build.rs ./build.rs

# Copy proto files
COPY ../proto ./proto

# Build the application
RUN cargo build --release --bin config-store-server

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Create a non-root user
RUN groupadd -r configstore && useradd -r -g configstore configstore

# Copy the binary from builder stage
COPY --from=builder /usr/src/app/target/release/config-store-server /usr/local/bin/config-store-server

# Change ownership
RUN chown configstore:configstore /usr/local/bin/config-store-server

# Switch to non-root user
USER configstore

# Expose the gRPC port
EXPOSE 50051

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
  CMD /usr/local/bin/config-store-server health || exit 1

# Default command
CMD ["config-store-server", "--host", "0.0.0.0", "--port", "50051"]