# TimescaleDB with pgvector extension for NDP intelligence layer
# Used in integration testing environment
FROM timescale/timescaledb:latest-pg16

# Install pgvector
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
       build-essential \
       postgresql-server-dev-16 \
       git \
       ca-certificates \
    && git clone --branch v0.8.0 https://github.com/pgvector/pgvector.git /tmp/pgvector \
    && cd /tmp/pgvector \
    && make \
    && make install \
    && cd / \
    && rm -rf /tmp/pgvector \
    && apt-get remove -y build-essential postgresql-server-dev-16 git \
    && apt-get autoremove -y \
    && rm -rf /var/lib/apt/lists/*
