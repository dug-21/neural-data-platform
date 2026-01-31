-- ============================================================================
-- SEED: 004_seed_silver_data_dictionary.sql
-- Purpose: Populate Silver data dictionary from YAML stream configurations
-- Feature: dp-009 (Silver Layer Data Dictionary)
-- Author: NDP Architect
-- Date: 2026-01-31
--
-- This script populates the data_dictionary tables with metadata for all
-- Silver layer tables defined in config/base/streams/*/config.yaml
--
-- Idempotent: Uses INSERT ... ON CONFLICT DO UPDATE
-- ============================================================================

-- ============================================================================
-- SILVER TABLES: Register all Silver hypertables
-- ============================================================================

-- Air Quality Observations (from air-quality stream)
INSERT INTO data_dictionary.silver_tables (
    table_name, schema_name, description, grain, source_streams,
    hypertable_column, chunk_interval
) VALUES (
    'silver.air_quality_observations',
    'silver',
    'Indoor air quality measurements from AirGradient sensors',
    'One row per sensor reading (~1 minute intervals)',
    ARRAY['air-quality'],
    'observation_time',
    INTERVAL '1 day'
)
ON CONFLICT (table_name) DO UPDATE SET
    description = EXCLUDED.description,
    grain = EXCLUDED.grain,
    source_streams = EXCLUDED.source_streams,
    updated_at = NOW();

-- Weather Observations (from outdoor-weather stream)
INSERT INTO data_dictionary.silver_tables (
    table_name, schema_name, description, grain, source_streams,
    hypertable_column, chunk_interval
) VALUES (
    'silver.weather_observations',
    'silver',
    'Outdoor weather observations from OpenWeatherMap API',
    'One row per observation (~10 minute intervals)',
    ARRAY['outdoor-weather'],
    'observation_time',
    INTERVAL '1 day'
)
ON CONFLICT (table_name) DO UPDATE SET
    description = EXCLUDED.description,
    grain = EXCLUDED.grain,
    source_streams = EXCLUDED.source_streams,
    updated_at = NOW();

-- Outdoor Air Quality (from outdoor-air-quality stream)
INSERT INTO data_dictionary.silver_tables (
    table_name, schema_name, description, grain, source_streams,
    hypertable_column, chunk_interval
) VALUES (
    'silver.outdoor_air_quality',
    'silver',
    'Outdoor air quality from OpenWeatherMap Air Pollution API',
    'One row per observation (~10 minute intervals)',
    ARRAY['outdoor-air-quality'],
    'observation_time',
    INTERVAL '1 day'
)
ON CONFLICT (table_name) DO UPDATE SET
    description = EXCLUDED.description,
    grain = EXCLUDED.grain,
    source_streams = EXCLUDED.source_streams,
    updated_at = NOW();

-- State Events (from home-assistant-state stream)
INSERT INTO data_dictionary.silver_tables (
    table_name, schema_name, description, grain, source_streams,
    hypertable_column, chunk_interval
) VALUES (
    'silver.state_events',
    'silver',
    'Window/door state change events from Home Assistant',
    'One row per state change event',
    ARRAY['home-assistant-state'],
    'event_time',
    INTERVAL '1 day'
)
ON CONFLICT (table_name) DO UPDATE SET
    description = EXCLUDED.description,
    grain = EXCLUDED.grain,
    source_streams = EXCLUDED.source_streams,
    updated_at = NOW();

-- ============================================================================
-- SILVER COLUMNS: air_quality_observations
-- ============================================================================

INSERT INTO data_dictionary.silver_columns (table_name, column_name, data_type, unit, description, nullable, is_primary_key, sort_order)
VALUES
    ('silver.air_quality_observations', 'observation_time', 'TIMESTAMPTZ', NULL, 'Observation timestamp (UTC)', false, true, 1),
    ('silver.air_quality_observations', 'ndp_id', 'TEXT', NULL, 'Unique device/sensor identifier', false, true, 2),
    ('silver.air_quality_observations', 'pm25', 'DOUBLE PRECISION', 'ug/m3', 'PM2.5 particulate matter concentration (humidity-compensated)', false, false, 10),
    ('silver.air_quality_observations', 'pm10', 'DOUBLE PRECISION', 'ug/m3', 'PM10 particulate matter concentration', true, false, 11),
    ('silver.air_quality_observations', 'co2', 'SMALLINT', 'ppm', 'Carbon dioxide concentration', true, false, 12),
    ('silver.air_quality_observations', 'temperature_c', 'DOUBLE PRECISION', 'Celsius', 'Ambient temperature (sensor-compensated)', true, false, 13),
    ('silver.air_quality_observations', 'humidity_pct', 'DOUBLE PRECISION', '%', 'Relative humidity (compensated)', true, false, 14),
    ('silver.air_quality_observations', 'tvoc_index', 'SMALLINT', 'index', 'Total VOC index (1-500 scale)', true, false, 15),
    ('silver.air_quality_observations', 'nox_index', 'SMALLINT', 'index', 'NOx index (1-500 scale)', true, false, 16),
    ('silver.air_quality_observations', 'dq_flags', 'TEXT[]', NULL, 'Data quality flags from ETL validation', true, false, 99)
ON CONFLICT (table_name, column_name) DO UPDATE SET
    data_type = EXCLUDED.data_type,
    unit = EXCLUDED.unit,
    description = EXCLUDED.description,
    nullable = EXCLUDED.nullable,
    updated_at = NOW();

-- ============================================================================
-- SILVER COLUMNS: weather_observations
-- ============================================================================

INSERT INTO data_dictionary.silver_columns (table_name, column_name, data_type, unit, description, nullable, is_primary_key, sort_order)
VALUES
    ('silver.weather_observations', 'observation_time', 'TIMESTAMPTZ', NULL, 'Observation timestamp (UTC)', false, true, 1),
    ('silver.weather_observations', 'ndp_id', 'TEXT', NULL, 'Unique location/source identifier', false, true, 2),
    ('silver.weather_observations', 'temperature_c', 'DOUBLE PRECISION', 'Celsius', 'Ambient air temperature', false, false, 10),
    ('silver.weather_observations', 'feels_like_c', 'DOUBLE PRECISION', 'Celsius', 'Apparent temperature (feels-like)', true, false, 11),
    ('silver.weather_observations', 'humidity_pct', 'DOUBLE PRECISION', '%', 'Relative humidity', true, false, 12),
    ('silver.weather_observations', 'pressure_pa', 'DOUBLE PRECISION', 'Pa', 'Atmospheric pressure (converted from hPa)', true, false, 13),
    ('silver.weather_observations', 'wind_speed_kmh', 'DOUBLE PRECISION', 'km/h', 'Wind speed (converted from m/s)', true, false, 14),
    ('silver.weather_observations', 'wind_gust_kmh', 'DOUBLE PRECISION', 'km/h', 'Wind gust speed (converted from m/s)', true, false, 15),
    ('silver.weather_observations', 'wind_direction_deg', 'DOUBLE PRECISION', 'degrees', 'Wind direction (0-360, 0=North)', true, false, 16),
    ('silver.weather_observations', 'cloud_cover_pct', 'DOUBLE PRECISION', '%', 'Cloud cover percentage', true, false, 17),
    ('silver.weather_observations', 'visibility_m', 'DOUBLE PRECISION', 'meters', 'Visibility distance', true, false, 18),
    ('silver.weather_observations', 'precipitation_mm', 'DOUBLE PRECISION', 'mm', 'Precipitation in last hour', true, false, 19),
    ('silver.weather_observations', 'dq_flags', 'TEXT[]', NULL, 'Data quality flags from ETL validation', true, false, 99)
ON CONFLICT (table_name, column_name) DO UPDATE SET
    data_type = EXCLUDED.data_type,
    unit = EXCLUDED.unit,
    description = EXCLUDED.description,
    nullable = EXCLUDED.nullable,
    updated_at = NOW();

-- ============================================================================
-- SILVER COLUMNS: outdoor_air_quality
-- ============================================================================

INSERT INTO data_dictionary.silver_columns (table_name, column_name, data_type, unit, description, nullable, is_primary_key, sort_order)
VALUES
    ('silver.outdoor_air_quality', 'observation_time', 'TIMESTAMPTZ', NULL, 'Observation timestamp (UTC)', false, true, 1),
    ('silver.outdoor_air_quality', 'ndp_id', 'TEXT', NULL, 'Unique location/source identifier', false, true, 2),
    ('silver.outdoor_air_quality', 'aqi_owm', 'SMALLINT', '1-5 scale', 'OpenWeatherMap Air Quality Index', false, false, 10),
    ('silver.outdoor_air_quality', 'aqi_epa', 'SMALLINT', '0-500 scale', 'EPA Air Quality Index (derived from OWM)', true, false, 11),
    ('silver.outdoor_air_quality', 'pm25', 'DOUBLE PRECISION', 'ug/m3', 'PM2.5 particulate matter', false, false, 12),
    ('silver.outdoor_air_quality', 'pm10', 'DOUBLE PRECISION', 'ug/m3', 'PM10 particulate matter', true, false, 13),
    ('silver.outdoor_air_quality', 'co_ugm3', 'DOUBLE PRECISION', 'ug/m3', 'Carbon monoxide concentration', true, false, 14),
    ('silver.outdoor_air_quality', 'no_ugm3', 'DOUBLE PRECISION', 'ug/m3', 'Nitrogen monoxide concentration', true, false, 15),
    ('silver.outdoor_air_quality', 'no2_ugm3', 'DOUBLE PRECISION', 'ug/m3', 'Nitrogen dioxide concentration', true, false, 16),
    ('silver.outdoor_air_quality', 'o3_ugm3', 'DOUBLE PRECISION', 'ug/m3', 'Ozone concentration', true, false, 17),
    ('silver.outdoor_air_quality', 'so2_ugm3', 'DOUBLE PRECISION', 'ug/m3', 'Sulfur dioxide concentration', true, false, 18),
    ('silver.outdoor_air_quality', 'nh3_ugm3', 'DOUBLE PRECISION', 'ug/m3', 'Ammonia concentration', true, false, 19),
    ('silver.outdoor_air_quality', 'dq_flags', 'TEXT[]', NULL, 'Data quality flags from ETL validation', true, false, 99)
ON CONFLICT (table_name, column_name) DO UPDATE SET
    data_type = EXCLUDED.data_type,
    unit = EXCLUDED.unit,
    description = EXCLUDED.description,
    nullable = EXCLUDED.nullable,
    updated_at = NOW();

-- ============================================================================
-- SILVER COLUMNS: state_events
-- ============================================================================

INSERT INTO data_dictionary.silver_columns (table_name, column_name, data_type, unit, description, nullable, is_primary_key, sort_order)
VALUES
    ('silver.state_events', 'event_time', 'TIMESTAMPTZ', NULL, 'Event timestamp (UTC)', false, true, 1),
    ('silver.state_events', 'ndp_id', 'TEXT', NULL, 'Unique device identifier (from topic segment)', false, true, 2),
    ('silver.state_events', 'state', 'TEXT', NULL, 'Binary state (on/off)', false, false, 10),
    ('silver.state_events', 'source_entity_id', 'TEXT', NULL, 'Home Assistant entity ID (ndp_id from topic)', true, false, 11),
    ('silver.state_events', 'dq_flags', 'TEXT[]', NULL, 'Data quality flags from ETL validation', true, false, 99)
ON CONFLICT (table_name, column_name) DO UPDATE SET
    data_type = EXCLUDED.data_type,
    unit = EXCLUDED.unit,
    description = EXCLUDED.description,
    nullable = EXCLUDED.nullable,
    updated_at = NOW();

-- ============================================================================
-- DQ RULES: air_quality_observations
-- ============================================================================

-- Column-level range checks
INSERT INTO data_dictionary.silver_dq_rules (silver_table, silver_column, rule_name, rule_params, action)
VALUES
    ('silver.air_quality_observations', 'pm25', 'range_check', '{"min": 0.0, "max": 1000.0}'::jsonb, 'flag'),
    ('silver.air_quality_observations', 'pm10', 'range_check', '{"min": 0.0, "max": 2000.0}'::jsonb, 'flag'),
    ('silver.air_quality_observations', 'co2', 'range_check', '{"min": 380, "max": 10000}'::jsonb, 'flag'),
    ('silver.air_quality_observations', 'temperature_c', 'range_check', '{"min": -40.0, "max": 85.0}'::jsonb, 'flag'),
    ('silver.air_quality_observations', 'humidity_pct', 'range_check', '{"min": 0.0, "max": 100.0, "clamp_to_bounds": true}'::jsonb, 'clamp'),
    ('silver.air_quality_observations', 'tvoc_index', 'range_check', '{"min": 1, "max": 500, "clamp_to_bounds": true}'::jsonb, 'clamp'),
    ('silver.air_quality_observations', 'nox_index', 'range_check', '{"min": 1, "max": 500, "clamp_to_bounds": true}'::jsonb, 'clamp')
ON CONFLICT ON CONSTRAINT idx_silver_dq_rules_unique DO UPDATE SET
    rule_params = EXCLUDED.rule_params,
    action = EXCLUDED.action,
    updated_at = NOW();

-- Cross-field and batch-level rules
INSERT INTO data_dictionary.silver_dq_rules (silver_table, silver_column, rule_name, rule_params, action)
VALUES
    ('silver.air_quality_observations', NULL, 'cross_field_check', '{"name": "pm10_gte_pm25", "expression": "pm10 IS NULL OR pm25 IS NULL OR pm10 >= pm25"}'::jsonb, 'flag'),
    ('silver.air_quality_observations', 'observation_time', 'freshness_check', '{"max_age": "2 hours", "max_future": "5 minutes"}'::jsonb, 'flag'),
    ('silver.air_quality_observations', 'pm25', 'rate_of_change', '{"max_change_per_minute": 100.0, "partition_by": ["ndp_id"]}'::jsonb, 'flag'),
    ('silver.air_quality_observations', 'temperature_c', 'rate_of_change', '{"max_change_per_minute": 3.0, "partition_by": ["ndp_id"]}'::jsonb, 'flag'),
    ('silver.air_quality_observations', 'pm25', 'completeness_check', '{"level": "batch", "min_completeness": 0.95}'::jsonb, 'warn')
ON CONFLICT ON CONSTRAINT idx_silver_dq_rules_unique DO UPDATE SET
    rule_params = EXCLUDED.rule_params,
    action = EXCLUDED.action,
    updated_at = NOW();

-- ============================================================================
-- DQ RULES: weather_observations
-- ============================================================================

INSERT INTO data_dictionary.silver_dq_rules (silver_table, silver_column, rule_name, rule_params, action)
VALUES
    ('silver.weather_observations', 'temperature_c', 'range_check', '{"min": -60.0, "max": 60.0}'::jsonb, 'flag'),
    ('silver.weather_observations', 'feels_like_c', 'range_check', '{"min": -60.0, "max": 60.0}'::jsonb, 'flag'),
    ('silver.weather_observations', 'humidity_pct', 'range_check', '{"min": 0.0, "max": 100.0, "clamp_to_bounds": true}'::jsonb, 'clamp'),
    ('silver.weather_observations', 'pressure_pa', 'range_check', '{"min": 80000.0, "max": 120000.0}'::jsonb, 'flag'),
    ('silver.weather_observations', 'wind_speed_kmh', 'range_check', '{"min": 0.0, "max": 400.0}'::jsonb, 'flag'),
    ('silver.weather_observations', 'wind_gust_kmh', 'range_check', '{"min": 0.0, "max": 500.0}'::jsonb, 'flag'),
    ('silver.weather_observations', 'wind_direction_deg', 'range_check', '{"min": 0.0, "max": 360.0, "clamp_to_bounds": true}'::jsonb, 'clamp'),
    ('silver.weather_observations', 'cloud_cover_pct', 'range_check', '{"min": 0.0, "max": 100.0, "clamp_to_bounds": true}'::jsonb, 'clamp'),
    ('silver.weather_observations', 'visibility_m', 'range_check', '{"min": 0.0, "max": 50000.0}'::jsonb, 'flag'),
    ('silver.weather_observations', 'precipitation_mm', 'range_check', '{"min": 0.0, "max": 500.0}'::jsonb, 'flag')
ON CONFLICT ON CONSTRAINT idx_silver_dq_rules_unique DO UPDATE SET
    rule_params = EXCLUDED.rule_params,
    action = EXCLUDED.action,
    updated_at = NOW();

-- Cross-field and batch-level rules
INSERT INTO data_dictionary.silver_dq_rules (silver_table, silver_column, rule_name, rule_params, action)
VALUES
    ('silver.weather_observations', NULL, 'cross_field_check', '{"name": "wind_gust_gte_speed", "expression": "wind_gust_kmh IS NULL OR wind_gust_kmh >= wind_speed_kmh"}'::jsonb, 'flag'),
    ('silver.weather_observations', NULL, 'cross_field_check', '{"name": "feels_like_reasonable", "expression": "feels_like_c IS NULL OR ABS(feels_like_c - temperature_c) <= 20"}'::jsonb, 'flag'),
    ('silver.weather_observations', 'observation_time', 'freshness_check', '{"max_age": "3 hours", "max_future": "10 minutes"}'::jsonb, 'flag'),
    ('silver.weather_observations', 'temperature_c', 'rate_of_change', '{"max_change_per_minute": 2.0, "partition_by": ["ndp_id"]}'::jsonb, 'flag'),
    ('silver.weather_observations', 'pressure_pa', 'rate_of_change', '{"max_change_per_minute": 500.0, "partition_by": ["ndp_id"]}'::jsonb, 'flag'),
    ('silver.weather_observations', 'temperature_c', 'completeness_check', '{"level": "batch", "min_completeness": 0.98}'::jsonb, 'warn')
ON CONFLICT ON CONSTRAINT idx_silver_dq_rules_unique DO UPDATE SET
    rule_params = EXCLUDED.rule_params,
    action = EXCLUDED.action,
    updated_at = NOW();

-- ============================================================================
-- DQ RULES: outdoor_air_quality
-- ============================================================================

INSERT INTO data_dictionary.silver_dq_rules (silver_table, silver_column, rule_name, rule_params, action)
VALUES
    ('silver.outdoor_air_quality', 'aqi_owm', 'range_check', '{"min": 1, "max": 5}'::jsonb, 'flag'),
    ('silver.outdoor_air_quality', 'aqi_owm', 'null_check', '{}'::jsonb, 'reject'),
    ('silver.outdoor_air_quality', 'pm25', 'range_check', '{"min": 0.0, "max": 1000.0}'::jsonb, 'flag'),
    ('silver.outdoor_air_quality', 'pm10', 'range_check', '{"min": 0.0, "max": 2000.0}'::jsonb, 'flag'),
    ('silver.outdoor_air_quality', 'co_ugm3', 'range_check', '{"min": 0.0, "max": 50000.0}'::jsonb, 'flag'),
    ('silver.outdoor_air_quality', 'no_ugm3', 'range_check', '{"min": 0.0, "max": 1000.0}'::jsonb, 'flag'),
    ('silver.outdoor_air_quality', 'no2_ugm3', 'range_check', '{"min": 0.0, "max": 1000.0}'::jsonb, 'flag'),
    ('silver.outdoor_air_quality', 'o3_ugm3', 'range_check', '{"min": 0.0, "max": 1000.0}'::jsonb, 'flag'),
    ('silver.outdoor_air_quality', 'so2_ugm3', 'range_check', '{"min": 0.0, "max": 1000.0}'::jsonb, 'flag'),
    ('silver.outdoor_air_quality', 'nh3_ugm3', 'range_check', '{"min": 0.0, "max": 200.0}'::jsonb, 'flag')
ON CONFLICT ON CONSTRAINT idx_silver_dq_rules_unique DO UPDATE SET
    rule_params = EXCLUDED.rule_params,
    action = EXCLUDED.action,
    updated_at = NOW();

-- Cross-field and batch-level rules
INSERT INTO data_dictionary.silver_dq_rules (silver_table, silver_column, rule_name, rule_params, action)
VALUES
    ('silver.outdoor_air_quality', NULL, 'cross_field_check', '{"name": "pm10_gte_pm25", "expression": "pm10 IS NULL OR pm25 IS NULL OR pm10 >= pm25"}'::jsonb, 'flag'),
    ('silver.outdoor_air_quality', 'observation_time', 'freshness_check', '{"max_age": "2 hours", "max_future": "10 minutes"}'::jsonb, 'flag'),
    ('silver.outdoor_air_quality', 'pm25', 'rate_of_change', '{"max_change_per_minute": 50.0, "partition_by": ["ndp_id"]}'::jsonb, 'flag'),
    ('silver.outdoor_air_quality', 'pm25', 'completeness_check', '{"level": "batch", "min_completeness": 0.95}'::jsonb, 'warn'),
    ('silver.outdoor_air_quality', 'aqi_owm', 'completeness_check', '{"level": "batch", "min_completeness": 0.98}'::jsonb, 'warn')
ON CONFLICT ON CONSTRAINT idx_silver_dq_rules_unique DO UPDATE SET
    rule_params = EXCLUDED.rule_params,
    action = EXCLUDED.action,
    updated_at = NOW();

-- ============================================================================
-- DQ RULES: state_events
-- ============================================================================

INSERT INTO data_dictionary.silver_dq_rules (silver_table, silver_column, rule_name, rule_params, action)
VALUES
    ('silver.state_events', 'state', 'enum_check', '{"allowed_values": ["on", "off"]}'::jsonb, 'flag'),
    ('silver.state_events', 'ndp_id', 'null_check', '{}'::jsonb, 'reject'),
    ('silver.state_events', 'event_time', 'freshness_check', '{"max_future": "5 minutes"}'::jsonb, 'flag')
ON CONFLICT ON CONSTRAINT idx_silver_dq_rules_unique DO UPDATE SET
    rule_params = EXCLUDED.rule_params,
    action = EXCLUDED.action,
    updated_at = NOW();

-- ============================================================================
-- LINEAGE: Bronze to Silver mappings
-- ============================================================================

-- Air Quality Observations lineage
INSERT INTO data_dictionary.silver_lineage (silver_table, silver_column, source_stream, source_path, transformation)
VALUES
    ('silver.air_quality_observations', 'observation_time', 'air-quality', 'timestamp', 'microseconds_to_timestamp'),
    ('silver.air_quality_observations', 'ndp_id', 'air-quality', 'ndp_id', 'direct'),
    ('silver.air_quality_observations', 'pm25', 'air-quality', 'raw_payload.pm02Compensated', 'direct'),
    ('silver.air_quality_observations', 'pm10', 'air-quality', 'raw_payload.pm10', 'direct'),
    ('silver.air_quality_observations', 'co2', 'air-quality', 'raw_payload.rco2', 'direct'),
    ('silver.air_quality_observations', 'temperature_c', 'air-quality', 'raw_payload.atmpCompensated', 'direct'),
    ('silver.air_quality_observations', 'humidity_pct', 'air-quality', 'raw_payload.rhumCompensated', 'direct'),
    ('silver.air_quality_observations', 'tvoc_index', 'air-quality', 'raw_payload.tvocIndex', 'direct'),
    ('silver.air_quality_observations', 'nox_index', 'air-quality', 'raw_payload.noxIndex', 'direct')
ON CONFLICT (silver_table, silver_column, source_stream) DO UPDATE SET
    source_path = EXCLUDED.source_path,
    transformation = EXCLUDED.transformation,
    updated_at = NOW();

-- Weather Observations lineage
INSERT INTO data_dictionary.silver_lineage (silver_table, silver_column, source_stream, source_path, transformation)
VALUES
    ('silver.weather_observations', 'observation_time', 'outdoor-weather', 'timestamp', 'microseconds_to_timestamp'),
    ('silver.weather_observations', 'ndp_id', 'outdoor-weather', 'ndp_id', 'direct'),
    ('silver.weather_observations', 'temperature_c', 'outdoor-weather', 'raw_payload.main.temp', 'direct'),
    ('silver.weather_observations', 'feels_like_c', 'outdoor-weather', 'raw_payload.main.feels_like', 'direct'),
    ('silver.weather_observations', 'humidity_pct', 'outdoor-weather', 'raw_payload.main.humidity', 'direct'),
    ('silver.weather_observations', 'pressure_pa', 'outdoor-weather', 'raw_payload.main.pressure', 'unit_conversion:hpa_to_pa'),
    ('silver.weather_observations', 'wind_speed_kmh', 'outdoor-weather', 'raw_payload.wind.speed', 'unit_conversion:ms_to_kmh'),
    ('silver.weather_observations', 'wind_gust_kmh', 'outdoor-weather', 'raw_payload.wind.gust', 'unit_conversion:ms_to_kmh'),
    ('silver.weather_observations', 'wind_direction_deg', 'outdoor-weather', 'raw_payload.wind.deg', 'direct'),
    ('silver.weather_observations', 'cloud_cover_pct', 'outdoor-weather', 'raw_payload.clouds.all', 'direct'),
    ('silver.weather_observations', 'visibility_m', 'outdoor-weather', 'raw_payload.visibility', 'direct'),
    ('silver.weather_observations', 'precipitation_mm', 'outdoor-weather', 'raw_payload.rain.1h', 'direct')
ON CONFLICT (silver_table, silver_column, source_stream) DO UPDATE SET
    source_path = EXCLUDED.source_path,
    transformation = EXCLUDED.transformation,
    updated_at = NOW();

-- Outdoor Air Quality lineage
INSERT INTO data_dictionary.silver_lineage (silver_table, silver_column, source_stream, source_path, transformation)
VALUES
    ('silver.outdoor_air_quality', 'observation_time', 'outdoor-air-quality', 'timestamp', 'microseconds_to_timestamp'),
    ('silver.outdoor_air_quality', 'ndp_id', 'outdoor-air-quality', 'ndp_id', 'direct'),
    ('silver.outdoor_air_quality', 'aqi_owm', 'outdoor-air-quality', 'raw_payload.list[0].main.aqi', 'direct'),
    ('silver.outdoor_air_quality', 'aqi_epa', 'outdoor-air-quality', 'raw_payload.list[0].main.aqi', 'expression:owm_to_epa'),
    ('silver.outdoor_air_quality', 'pm25', 'outdoor-air-quality', 'raw_payload.list[0].components.pm2_5', 'direct'),
    ('silver.outdoor_air_quality', 'pm10', 'outdoor-air-quality', 'raw_payload.list[0].components.pm10', 'direct'),
    ('silver.outdoor_air_quality', 'co_ugm3', 'outdoor-air-quality', 'raw_payload.list[0].components.co', 'direct'),
    ('silver.outdoor_air_quality', 'no_ugm3', 'outdoor-air-quality', 'raw_payload.list[0].components.no', 'direct'),
    ('silver.outdoor_air_quality', 'no2_ugm3', 'outdoor-air-quality', 'raw_payload.list[0].components.no2', 'direct'),
    ('silver.outdoor_air_quality', 'o3_ugm3', 'outdoor-air-quality', 'raw_payload.list[0].components.o3', 'direct'),
    ('silver.outdoor_air_quality', 'so2_ugm3', 'outdoor-air-quality', 'raw_payload.list[0].components.so2', 'direct'),
    ('silver.outdoor_air_quality', 'nh3_ugm3', 'outdoor-air-quality', 'raw_payload.list[0].components.nh3', 'direct')
ON CONFLICT (silver_table, silver_column, source_stream) DO UPDATE SET
    source_path = EXCLUDED.source_path,
    transformation = EXCLUDED.transformation,
    updated_at = NOW();

-- State Events lineage
INSERT INTO data_dictionary.silver_lineage (silver_table, silver_column, source_stream, source_path, transformation)
VALUES
    ('silver.state_events', 'event_time', 'home-assistant-state', 'timestamp', 'microseconds_to_timestamp'),
    ('silver.state_events', 'ndp_id', 'home-assistant-state', 'ndp_id', 'direct'),
    ('silver.state_events', 'state', 'home-assistant-state', 'raw_payload._raw_text', 'direct'),
    ('silver.state_events', 'source_entity_id', 'home-assistant-state', 'ndp_id', 'direct')
ON CONFLICT (silver_table, silver_column, source_stream) DO UPDATE SET
    source_path = EXCLUDED.source_path,
    transformation = EXCLUDED.transformation,
    updated_at = NOW();

-- ============================================================================
-- SUCCESS MESSAGE
-- ============================================================================

DO $$
DECLARE
    table_count INTEGER;
    column_count INTEGER;
    rule_count INTEGER;
    lineage_count INTEGER;
BEGIN
    SELECT COUNT(*) INTO table_count FROM data_dictionary.silver_tables;
    SELECT COUNT(*) INTO column_count FROM data_dictionary.silver_columns;
    SELECT COUNT(*) INTO rule_count FROM data_dictionary.silver_dq_rules;
    SELECT COUNT(*) INTO lineage_count FROM data_dictionary.silver_lineage;

    RAISE NOTICE 'Silver Data Dictionary seeded successfully';
    RAISE NOTICE 'Tables: %, Columns: %, DQ Rules: %, Lineage mappings: %',
        table_count, column_count, rule_count, lineage_count;
END $$;
