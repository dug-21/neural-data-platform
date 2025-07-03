#!/bin/bash
set -e

# Function to update PostgreSQL configuration
update_postgresql_conf() {
    local setting=$1
    local value=$2
    
    # Check if setting exists
    if grep -q "^${setting}" "$PGDATA/postgresql.conf"; then
        sed -i "s/^${setting}.*/${setting} = ${value}/" "$PGDATA/postgresql.conf"
    else
        echo "${setting} = ${value}" >> "$PGDATA/postgresql.conf"
    fi
}

# If PGDATA doesn't exist, run the parent entrypoint to initialize
if [ ! -s "$PGDATA/PG_VERSION" ]; then
    echo "Initializing database..."
    /usr/local/bin/docker-entrypoint.sh postgres &
    
    # Wait for database to be ready
    until pg_isready -U "$POSTGRES_USER" -d "$POSTGRES_DB"; do
        echo "Waiting for database to be ready..."
        sleep 2
    done
    
    # Stop the temporary server
    pg_ctl -D "$PGDATA" -m fast -w stop
fi

# Apply custom configuration
echo "Applying custom PostgreSQL configuration..."

# Copy our custom config if it doesn't exist
if [ ! -f "$PGDATA/postgresql.conf.custom" ]; then
    cp /etc/postgresql/postgresql.conf "$PGDATA/postgresql.conf.custom"
fi

# Apply environment variable overrides
if [ -n "$POSTGRES_SHARED_BUFFERS" ]; then
    update_postgresql_conf "shared_buffers" "$POSTGRES_SHARED_BUFFERS"
fi

if [ -n "$POSTGRES_EFFECTIVE_CACHE_SIZE" ]; then
    update_postgresql_conf "effective_cache_size" "$POSTGRES_EFFECTIVE_CACHE_SIZE"
fi

if [ -n "$POSTGRES_MAINTENANCE_WORK_MEM" ]; then
    update_postgresql_conf "maintenance_work_mem" "$POSTGRES_MAINTENANCE_WORK_MEM"
fi

if [ -n "$POSTGRES_WORK_MEM" ]; then
    update_postgresql_conf "work_mem" "$POSTGRES_WORK_MEM"
fi

if [ -n "$POSTGRES_MAX_CONNECTIONS" ]; then
    update_postgresql_conf "max_connections" "$POSTGRES_MAX_CONNECTIONS"
fi

if [ -n "$POSTGRES_MAX_PARALLEL_WORKERS" ]; then
    update_postgresql_conf "max_parallel_workers" "$POSTGRES_MAX_PARALLEL_WORKERS"
fi

if [ -n "$POSTGRES_MAX_PARALLEL_WORKERS_PER_GATHER" ]; then
    update_postgresql_conf "max_parallel_workers_per_gather" "$POSTGRES_MAX_PARALLEL_WORKERS_PER_GATHER"
fi

# Include custom config
if ! grep -q "include = 'postgresql.conf.custom'" "$PGDATA/postgresql.conf"; then
    echo "include = 'postgresql.conf.custom'" >> "$PGDATA/postgresql.conf"
fi

# Start PostgreSQL
exec /usr/local/bin/docker-entrypoint.sh postgres "$@"