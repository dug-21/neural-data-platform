-- TimescaleDB Compression and Retention Policies for Disk Management
-- This script sets up automatic data compression and retention to minimize disk usage

-- Enable TimescaleDB extension if not already enabled
CREATE EXTENSION IF NOT EXISTS timescaledb CASCADE;

-- Function to safely add compression policy
CREATE OR REPLACE FUNCTION add_compression_policy_safe(
    table_name TEXT,
    compress_after INTERVAL
) RETURNS VOID AS $$
BEGIN
    -- Check if policy already exists
    IF NOT EXISTS (
        SELECT 1 FROM timescaledb_information.compression_settings
        WHERE hypertable_name = table_name
    ) THEN
        -- Add compression policy
        PERFORM add_compression_policy(table_name, compress_after);
        RAISE NOTICE 'Added compression policy for % (compress after %)', table_name, compress_after;
    ELSE
        RAISE NOTICE 'Compression policy already exists for %', table_name;
    END IF;
EXCEPTION
    WHEN OTHERS THEN
        RAISE NOTICE 'Failed to add compression policy for %: %', table_name, SQLERRM;
END;
$$ LANGUAGE plpgsql;

-- Function to safely add retention policy
CREATE OR REPLACE FUNCTION add_retention_policy_safe(
    table_name TEXT,
    drop_after INTERVAL
) RETURNS VOID AS $$
BEGIN
    -- Check if policy already exists
    IF NOT EXISTS (
        SELECT 1 FROM timescaledb_information.jobs
        WHERE hypertable_name = table_name
        AND proc_name = 'policy_retention'
    ) THEN
        -- Add retention policy
        PERFORM add_retention_policy(table_name, drop_after);
        RAISE NOTICE 'Added retention policy for % (drop after %)', table_name, drop_after;
    ELSE
        RAISE NOTICE 'Retention policy already exists for %', table_name;
    END IF;
EXCEPTION
    WHEN OTHERS THEN
        RAISE NOTICE 'Failed to add retention policy for %: %', table_name, SQLERRM;
END;
$$ LANGUAGE plpgsql;

-- Configure compression for market_data table
DO $$
BEGIN
    -- Enable compression on market_data
    ALTER TABLE market_data SET (
        timescaledb.compress,
        timescaledb.compress_segmentby = 'symbol,provider',
        timescaledb.compress_orderby = 'time DESC'
    );
    
    -- Add compression policy: compress data older than 7 days
    PERFORM add_compression_policy_safe('market_data', INTERVAL '7 days');
    
    -- Add retention policy: keep data for 1 year
    PERFORM add_retention_policy_safe('market_data', INTERVAL '1 year');
    
EXCEPTION
    WHEN OTHERS THEN
        RAISE NOTICE 'Error configuring market_data compression: %', SQLERRM;
END $$;

-- Configure compression for tick_data table
DO $$
BEGIN
    -- Enable compression on tick_data
    ALTER TABLE tick_data SET (
        timescaledb.compress,
        timescaledb.compress_segmentby = 'symbol,provider',
        timescaledb.compress_orderby = 'time DESC'
    );
    
    -- Add compression policy: compress data older than 1 day
    PERFORM add_compression_policy_safe('tick_data', INTERVAL '1 day');
    
    -- Add retention policy: keep tick data for 30 days only
    PERFORM add_retention_policy_safe('tick_data', INTERVAL '30 days');
    
EXCEPTION
    WHEN OTHERS THEN
        RAISE NOTICE 'Error configuring tick_data compression: %', SQLERRM;
END $$;

-- Configure compression for order_book table
DO $$
BEGIN
    -- Enable compression on order_book
    ALTER TABLE order_book SET (
        timescaledb.compress,
        timescaledb.compress_segmentby = 'symbol,provider',
        timescaledb.compress_orderby = 'time DESC'
    );
    
    -- Add compression policy: compress data older than 1 day
    PERFORM add_compression_policy_safe('order_book', INTERVAL '1 day');
    
    -- Add retention policy: keep order book data for 7 days only
    PERFORM add_retention_policy_safe('order_book', INTERVAL '7 days');
    
EXCEPTION
    WHEN OTHERS THEN
        RAISE NOTICE 'Error configuring order_book compression: %', SQLERRM;
END $$;

-- Configure compression for technical_indicators table
DO $$
BEGIN
    -- Enable compression on technical_indicators
    ALTER TABLE technical_indicators SET (
        timescaledb.compress,
        timescaledb.compress_segmentby = 'symbol,indicator',
        timescaledb.compress_orderby = 'time DESC'
    );
    
    -- Add compression policy: compress data older than 7 days
    PERFORM add_compression_policy_safe('technical_indicators', INTERVAL '7 days');
    
    -- Add retention policy: keep indicators for 90 days
    PERFORM add_retention_policy_safe('technical_indicators', INTERVAL '90 days');
    
EXCEPTION
    WHEN OTHERS THEN
        RAISE NOTICE 'Error configuring technical_indicators compression: %', SQLERRM;
END $$;

-- Create continuous aggregates for efficient querying of compressed data
-- 1-hour aggregates for market data
CREATE MATERIALIZED VIEW IF NOT EXISTS market_data_hourly
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('1 hour', time) AS hour,
    symbol,
    provider,
    FIRST(open, time) AS open,
    MAX(high) AS high,
    MIN(low) AS low,
    LAST(close, time) AS close,
    SUM(volume) AS volume,
    COUNT(*) AS sample_count
FROM market_data
GROUP BY hour, symbol, provider
WITH NO DATA;

-- Add refresh policy for hourly aggregates
SELECT add_continuous_aggregate_policy('market_data_hourly',
    start_offset => INTERVAL '3 days',
    end_offset => INTERVAL '1 hour',
    schedule_interval => INTERVAL '1 hour');

-- Daily aggregates for market data
CREATE MATERIALIZED VIEW IF NOT EXISTS market_data_daily
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('1 day', time) AS day,
    symbol,
    provider,
    FIRST(open, time) AS open,
    MAX(high) AS high,
    MIN(low) AS low,
    LAST(close, time) AS close,
    SUM(volume) AS volume,
    COUNT(*) AS sample_count
FROM market_data
GROUP BY day, symbol, provider
WITH NO DATA;

-- Add refresh policy for daily aggregates
SELECT add_continuous_aggregate_policy('market_data_daily',
    start_offset => INTERVAL '7 days',
    end_offset => INTERVAL '1 day',
    schedule_interval => INTERVAL '1 day');

-- Create a function to manually trigger compression for recent data
CREATE OR REPLACE FUNCTION compress_recent_data(
    table_name TEXT,
    older_than INTERVAL DEFAULT INTERVAL '1 day'
) RETURNS TABLE(chunk_name TEXT, before_size TEXT, after_size TEXT) AS $$
DECLARE
    chunk RECORD;
    before_size BIGINT;
    after_size BIGINT;
BEGIN
    FOR chunk IN
        SELECT chunk_schema, chunk_name, range_start, range_end
        FROM timescaledb_information.chunks
        WHERE hypertable_name = table_name
        AND NOT is_compressed
        AND range_end < NOW() - older_than
        ORDER BY range_start
    LOOP
        -- Get size before compression
        SELECT pg_size_pretty(pg_total_relation_size(format('%I.%I', chunk.chunk_schema, chunk.chunk_name)))::TEXT 
        INTO before_size;
        
        -- Compress the chunk
        PERFORM compress_chunk(format('%I.%I', chunk.chunk_schema, chunk.chunk_name));
        
        -- Get size after compression
        SELECT pg_size_pretty(pg_total_relation_size(format('%I.%I', chunk.chunk_schema, chunk.chunk_name)))::TEXT 
        INTO after_size;
        
        RETURN QUERY SELECT chunk.chunk_name::TEXT, before_size::TEXT, after_size::TEXT;
    END LOOP;
END;
$$ LANGUAGE plpgsql;

-- Create a monitoring view for disk usage
CREATE OR REPLACE VIEW disk_usage_summary AS
WITH table_sizes AS (
    SELECT
        hypertable_name AS table_name,
        pg_size_pretty(hypertable_size(format('%I.%I', hypertable_schema, hypertable_name))) AS total_size,
        pg_size_pretty(compression_total_chunk_size) AS compressed_size,
        pg_size_pretty(uncompressed_total_chunk_size) AS uncompressed_size,
        compression_ratio,
        number_compressed_chunks,
        number_uncompressed_chunks
    FROM timescaledb_information.hypertables
    LEFT JOIN LATERAL (
        SELECT
            COUNT(*) FILTER (WHERE is_compressed) AS number_compressed_chunks,
            COUNT(*) FILTER (WHERE NOT is_compressed) AS number_uncompressed_chunks,
            SUM(CASE WHEN is_compressed THEN total_bytes ELSE 0 END) AS compression_total_chunk_size,
            SUM(CASE WHEN NOT is_compressed THEN total_bytes ELSE 0 END) AS uncompressed_total_chunk_size,
            CASE 
                WHEN SUM(CASE WHEN is_compressed THEN uncompressed_total_bytes ELSE 0 END) > 0
                THEN ROUND(
                    SUM(CASE WHEN is_compressed THEN uncompressed_total_bytes ELSE 0 END)::NUMERIC / 
                    SUM(CASE WHEN is_compressed THEN total_bytes ELSE 0 END)::NUMERIC, 
                    2
                )
                ELSE NULL
            END AS compression_ratio
        FROM timescaledb_information.chunks
        WHERE hypertable_name = hypertables.hypertable_name
    ) chunks ON true
)
SELECT
    table_name,
    total_size,
    compressed_size,
    uncompressed_size,
    COALESCE(compression_ratio, 1.0) AS compression_ratio,
    COALESCE(number_compressed_chunks, 0) AS compressed_chunks,
    COALESCE(number_uncompressed_chunks, 0) AS uncompressed_chunks,
    ROUND(
        100.0 * COALESCE(number_compressed_chunks, 0) / 
        GREATEST(COALESCE(number_compressed_chunks, 0) + COALESCE(number_uncompressed_chunks, 0), 1),
        1
    ) AS compression_percentage
FROM table_sizes
ORDER BY 
    pg_size_bytes(total_size) DESC;

-- Create an alert function for disk usage
CREATE OR REPLACE FUNCTION check_disk_usage(
    warning_threshold_gb NUMERIC DEFAULT 10,
    critical_threshold_gb NUMERIC DEFAULT 50
) RETURNS TABLE(
    severity TEXT,
    message TEXT,
    current_size_gb NUMERIC,
    table_name TEXT
) AS $$
BEGIN
    RETURN QUERY
    WITH usage AS (
        SELECT
            h.hypertable_name,
            pg_size_bytes(pg_size_pretty(hypertable_size(format('%I.%I', h.hypertable_schema, h.hypertable_name)))) / 1e9 AS size_gb
        FROM timescaledb_information.hypertables h
    )
    SELECT
        CASE 
            WHEN u.size_gb >= critical_threshold_gb THEN 'CRITICAL'
            WHEN u.size_gb >= warning_threshold_gb THEN 'WARNING'
            ELSE 'OK'
        END AS severity,
        CASE 
            WHEN u.size_gb >= critical_threshold_gb THEN 
                format('Table %s is using %.1f GB (critical threshold: %.1f GB)', u.hypertable_name, u.size_gb, critical_threshold_gb)
            WHEN u.size_gb >= warning_threshold_gb THEN 
                format('Table %s is using %.1f GB (warning threshold: %.1f GB)', u.hypertable_name, u.size_gb, warning_threshold_gb)
            ELSE 
                format('Table %s disk usage is normal (%.1f GB)', u.hypertable_name, u.size_gb)
        END AS message,
        u.size_gb AS current_size_gb,
        u.hypertable_name AS table_name
    FROM usage u
    WHERE u.size_gb >= warning_threshold_gb
    ORDER BY u.size_gb DESC;
END;
$$ LANGUAGE plpgsql;

-- Log completion
DO $$
BEGIN
    RAISE NOTICE 'Compression and retention policies have been configured successfully';
    RAISE NOTICE 'Run SELECT * FROM disk_usage_summary; to see current disk usage';
    RAISE NOTICE 'Run SELECT * FROM check_disk_usage(); to check for disk usage alerts';
END $$;