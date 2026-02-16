-- ops-008: Layer 0 Foundation — Silver Utility Functions
-- Source: Silver schema utility functions (Section 1)
-- Run order: 3rd (depends on silver schema)
-- Idempotent: Yes (CREATE OR REPLACE)

-- Linear interpolation helper for AQI calculation
CREATE OR REPLACE FUNCTION silver.linear_interpolate(
    value DOUBLE PRECISION,
    bp_low DOUBLE PRECISION,
    bp_high DOUBLE PRECISION,
    aqi_low INTEGER,
    aqi_high INTEGER
) RETURNS SMALLINT AS $$
BEGIN
    IF value IS NULL OR bp_high = bp_low THEN
        RETURN NULL;
    END IF;
    RETURN ROUND(
        ((aqi_high - aqi_low)::DOUBLE PRECISION / (bp_high - bp_low))
        * (value - bp_low) + aqi_low
    )::SMALLINT;
END;
$$ LANGUAGE plpgsql IMMUTABLE PARALLEL SAFE;

-- EPA PM2.5 AQI calculation (2024 breakpoints)
CREATE OR REPLACE FUNCTION silver.calculate_aqi_pm25(pm25_value DOUBLE PRECISION)
RETURNS SMALLINT AS $$
BEGIN
    IF pm25_value IS NULL THEN
        RETURN NULL;
    ELSIF pm25_value <= 9.0 THEN
        RETURN silver.linear_interpolate(pm25_value, 0, 9.0, 0, 50);
    ELSIF pm25_value <= 35.4 THEN
        RETURN silver.linear_interpolate(pm25_value, 9.1, 35.4, 51, 100);
    ELSIF pm25_value <= 55.4 THEN
        RETURN silver.linear_interpolate(pm25_value, 35.5, 55.4, 101, 150);
    ELSIF pm25_value <= 125.4 THEN
        RETURN silver.linear_interpolate(pm25_value, 55.5, 125.4, 151, 200);
    ELSIF pm25_value <= 225.4 THEN
        RETURN silver.linear_interpolate(pm25_value, 125.5, 225.4, 201, 300);
    ELSIF pm25_value <= 325.4 THEN
        RETURN silver.linear_interpolate(pm25_value, 225.5, 325.4, 301, 500);
    ELSE
        RETURN 500;  -- Beyond scale
    END IF;
END;
$$ LANGUAGE plpgsql IMMUTABLE PARALLEL SAFE;

-- Mold risk index calculation
CREATE OR REPLACE FUNCTION silver.calculate_mold_risk(
    temp_c DOUBLE PRECISION,
    humidity_pct DOUBLE PRECISION
) RETURNS TEXT AS $$
BEGIN
    IF humidity_pct IS NULL THEN
        RETURN 'UNKNOWN';
    ELSIF humidity_pct < 50 THEN
        RETURN 'LOW';
    ELSIF humidity_pct < 60 THEN
        RETURN 'MODERATE';
    ELSIF humidity_pct < 65 THEN
        RETURN 'ELEVATED';
    ELSIF humidity_pct < 80 THEN
        RETURN 'HIGH';
    ELSE
        RETURN 'CRITICAL';
    END IF;
END;
$$ LANGUAGE plpgsql IMMUTABLE PARALLEL SAFE;

DO $$ BEGIN
  RAISE NOTICE 'NDP init [003]: Silver functions created — linear_interpolate, calculate_aqi_pm25, calculate_mold_risk';
END $$;
