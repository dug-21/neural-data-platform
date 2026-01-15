-- Computed columns for Silver layer tables
-- These are derived values that support dashboard queries
-- Run after tables are created by Silver ETL

-- lead_time_hours: Hours between forecast issue and validity
-- Used by forecast accuracy dashboards to bucket forecasts by lead time
-- Formula: (valid_time - issue_time) in hours
DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM information_schema.tables
        WHERE table_schema = 'silver' AND table_name = 'weather_forecasts'
    ) THEN
        -- Add lead_time_hours if it doesn't exist
        IF NOT EXISTS (
            SELECT 1 FROM information_schema.columns
            WHERE table_schema = 'silver'
            AND table_name = 'weather_forecasts'
            AND column_name = 'lead_time_hours'
        ) THEN
            ALTER TABLE silver.weather_forecasts
            ADD COLUMN lead_time_hours double precision
            GENERATED ALWAYS AS (EXTRACT(EPOCH FROM (valid_time - issue_time)) / 3600.0) STORED;

            RAISE NOTICE 'Added lead_time_hours column to silver.weather_forecasts';
        END IF;
    END IF;
END $$;
