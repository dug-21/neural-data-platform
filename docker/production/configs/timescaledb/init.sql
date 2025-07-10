-- Initialize TimescaleDB extension
CREATE EXTENSION IF NOT EXISTS timescaledb;

-- Note: The main user 'neural_trader' is created automatically by PostgreSQL Docker image
-- via POSTGRES_USER environment variable, so we don't need to create it here

-- Ensure the user has all necessary permissions (in case they don't already)
-- Using IF EXISTS pattern to make this idempotent
DO $$ 
BEGIN
    -- Grant database permissions
    EXECUTE format('GRANT ALL PRIVILEGES ON DATABASE %I TO %I', current_database(), current_user);
    
    -- Grant schema permissions
    GRANT ALL PRIVILEGES ON SCHEMA public TO neural_trader;
    GRANT ALL PRIVILEGES ON ALL TABLES IN SCHEMA public TO neural_trader;
    GRANT ALL PRIVILEGES ON ALL SEQUENCES IN SCHEMA public TO neural_trader;
    
    -- Grant permissions on future objects
    ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT ALL ON TABLES TO neural_trader;
    ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT ALL ON SEQUENCES TO neural_trader;
    
EXCEPTION
    WHEN undefined_object THEN
        -- User doesn't exist yet, which is fine - Docker will create it
        RAISE NOTICE 'User will be created by Docker initialization';
END $$;

-- Create read-only user for monitoring (if not exists)
DO $$
BEGIN
    IF NOT EXISTS (SELECT FROM pg_user WHERE usename = 'neural_trader_readonly') THEN
        CREATE USER neural_trader_readonly WITH PASSWORD 'readonly_password_changeme';
    END IF;
END $$;

GRANT CONNECT ON DATABASE neural_trader TO neural_trader_readonly;
GRANT USAGE ON SCHEMA public TO neural_trader_readonly;