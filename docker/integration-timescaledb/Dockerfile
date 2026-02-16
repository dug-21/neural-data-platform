# TimescaleDB with pgvector extension for NDP intelligence layer
# INTEGRATION ONLY — mirrors production docker/timescaledb/Dockerfile
# but uses Alpine-compatible commands (base image is Alpine, not Debian)
#
# Production uses: apt-get, postgresql-16-pgvector (Debian package)
# Integration uses: apk + build from source (Alpine equivalent)
FROM timescale/timescaledb:latest-pg16

# Install pgvector from source (Alpine equivalent of production's postgresql-16-pgvector)
# with_llvm=no: skip LLVM bitcode generation (not needed for integration testing)
RUN apk add --no-cache --virtual .build-deps \
        build-base \
        postgresql16-dev \
        git \
    && git clone --branch v0.8.0 https://github.com/pgvector/pgvector.git /tmp/pgvector \
    && cd /tmp/pgvector \
    && make USE_PGXS=1 OPTFLAGS="" with_llvm=no \
    && make USE_PGXS=1 with_llvm=no install \
    && cd / \
    && rm -rf /tmp/pgvector \
    && apk del .build-deps
