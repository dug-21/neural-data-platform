FROM rust:1.85 AS builder
WORKDIR /app
COPY . .
RUN cargo build --release --bin air-quality-server

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/air-quality-server /usr/local/bin/
COPY --from=builder /app/apps/air-quality-app/config.yaml /etc/air-quality-app/
WORKDIR /app
CMD ["air-quality-server"]
