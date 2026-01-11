-- Create required PostgreSQL extensions
-- This runs first (00-) before other init scripts

-- TimescaleDB for time-series hypertables
CREATE EXTENSION IF NOT EXISTS timescaledb CASCADE;

-- Confirm extension is installed
DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM pg_extension WHERE extname = 'timescaledb') THEN
        RAISE NOTICE 'TimescaleDB extension installed successfully';
    ELSE
        RAISE EXCEPTION 'TimescaleDB extension failed to install';
    END IF;
END $$;
