# Custom TimescaleDB image optimized for trading data
FROM timescale/timescaledb:latest-pg16

# Install additional extensions and tools
RUN apt-get update && apt-get install -y \
    postgresql-16-pg-stat-kcache \
    postgresql-16-pgrouting \
    postgresql-16-pgvector \
    postgresql-16-cron \
    htop \
    iotop \
    && rm -rf /var/lib/apt/lists/*

# Copy custom PostgreSQL configuration
COPY postgresql.conf /etc/postgresql/postgresql.conf

# Copy initialization scripts
COPY init-scripts/ /docker-entrypoint-initdb.d/

# Set environment variables for performance tuning
ENV POSTGRES_SHARED_BUFFERS="4GB"
ENV POSTGRES_EFFECTIVE_CACHE_SIZE="12GB"
ENV POSTGRES_MAINTENANCE_WORK_MEM="1GB"
ENV POSTGRES_WORK_MEM="256MB"
ENV POSTGRES_MAX_CONNECTIONS="200"
ENV POSTGRES_MAX_PARALLEL_WORKERS="8"
ENV POSTGRES_MAX_PARALLEL_WORKERS_PER_GATHER="4"

# Expose PostgreSQL port
EXPOSE 5432

# Use custom entrypoint script
COPY docker-entrypoint.sh /usr/local/bin/
RUN chmod +x /usr/local/bin/docker-entrypoint.sh

ENTRYPOINT ["/usr/local/bin/docker-entrypoint.sh"]
CMD ["postgres"]