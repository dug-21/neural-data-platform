-- Initialize TimescaleDB extension
CREATE EXTENSION IF NOT EXISTS timescaledb;

-- Note: The main user is created automatically by PostgreSQL Docker image
-- via POSTGRES_USER environment variable, so we don't need to create it here

-- Ensure the user has all necessary permissions (in case they don't already)
-- Using IF EXISTS pattern to make this idempotent
DO $$ 
DECLARE
    db_owner text;
BEGIN
    -- Get the database owner (which should be POSTGRES_USER)
    SELECT pg_database.datdba::regrole::text INTO db_owner 
    FROM pg_database 
    WHERE datname = current_database();
    
    -- Grant database permissions
    EXECUTE format('GRANT ALL PRIVILEGES ON DATABASE %I TO %I', current_database(), db_owner);
    
    -- Grant schema permissions
    EXECUTE format('GRANT ALL PRIVILEGES ON SCHEMA public TO %I', db_owner);
    EXECUTE format('GRANT ALL PRIVILEGES ON ALL TABLES IN SCHEMA public TO %I', db_owner);
    EXECUTE format('GRANT ALL PRIVILEGES ON ALL SEQUENCES IN SCHEMA public TO %I', db_owner);
    
    -- Grant permissions on future objects
    EXECUTE format('ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT ALL ON TABLES TO %I', db_owner);
    EXECUTE format('ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT ALL ON SEQUENCES TO %I', db_owner);
    
EXCEPTION
    WHEN undefined_object THEN
        -- User doesn't exist yet, which is fine - Docker will create it
        RAISE NOTICE 'User will be created by Docker initialization';
END $$;

-- Create read-only user for monitoring (if not exists)
DO $$
DECLARE
    db_owner text;
    readonly_user text;
BEGIN
    -- Get the database owner
    SELECT pg_database.datdba::regrole::text INTO db_owner 
    FROM pg_database 
    WHERE datname = current_database();
    
    -- Create readonly username based on the database owner
    readonly_user := db_owner || '_readonly';
    
    IF NOT EXISTS (SELECT FROM pg_user WHERE usename = readonly_user) THEN
        EXECUTE format('CREATE USER %I WITH PASSWORD %L', readonly_user, 'readonly_password_changeme');
    END IF;
    
    -- Grant permissions
    EXECUTE format('GRANT CONNECT ON DATABASE %I TO %I', current_database(), readonly_user);
    EXECUTE format('GRANT USAGE ON SCHEMA public TO %I', readonly_user);
END $$;