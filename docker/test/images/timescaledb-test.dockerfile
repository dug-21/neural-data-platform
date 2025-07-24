# Test TimescaleDB image with test data and schema
FROM timescale/timescaledb:latest-pg16

# Install additional tools for testing
RUN apt-get update && apt-get install -y \
    python3 \
    python3-pip \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Copy test-specific initialization scripts
COPY configs/timescaledb/test-init.sql /docker-entrypoint-initdb.d/01-test-init.sql
COPY configs/timescaledb/test-schema.sql /docker-entrypoint-initdb.d/02-test-schema.sql
COPY configs/timescaledb/test-data.sql /docker-entrypoint-initdb.d/03-test-data.sql

# Copy test data loading script
COPY scripts/load-test-data.py /docker-entrypoint-initdb.d/99-load-test-data.py
RUN chmod +x /docker-entrypoint-initdb.d/99-load-test-data.py

# Test environment configuration
ENV POSTGRES_DB=neural_trader_test
ENV POSTGRES_USER=test_user
ENV POSTGRES_PASSWORD=test_password_123

# TimescaleDB tuning optimized for testing (faster, less memory)
RUN echo "shared_preload_libraries = 'timescaledb'" >> /usr/share/postgresql/postgresql.conf.sample && \
    echo "max_connections = 50" >> /usr/share/postgresql/postgresql.conf.sample && \
    echo "shared_buffers = 128MB" >> /usr/share/postgresql/postgresql.conf.sample && \
    echo "effective_cache_size = 512MB" >> /usr/share/postgresql/postgresql.conf.sample && \
    echo "maintenance_work_mem = 32MB" >> /usr/share/postgresql/postgresql.conf.sample && \
    echo "checkpoint_completion_target = 0.9" >> /usr/share/postgresql/postgresql.conf.sample && \
    echo "wal_buffers = 8MB" >> /usr/share/postgresql/postgresql.conf.sample && \
    echo "default_statistics_target = 50" >> /usr/share/postgresql/postgresql.conf.sample && \
    echo "random_page_cost = 1.1" >> /usr/share/postgresql/postgresql.conf.sample && \
    echo "log_statement = 'all'" >> /usr/share/postgresql/postgresql.conf.sample && \
    echo "log_duration = on" >> /usr/share/postgresql/postgresql.conf.sample

# Add test-specific extensions
RUN echo "CREATE EXTENSION IF NOT EXISTS timescaledb CASCADE;" > /docker-entrypoint-initdb.d/00-extensions.sql

# Health check
HEALTHCHECK --interval=10s --timeout=3s --start-period=10s --retries=3 \
    CMD pg_isready -U $POSTGRES_USER -d $POSTGRES_DB || exit 1