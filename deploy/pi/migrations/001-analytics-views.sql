-- ops-008: Migration — Analytics Views
-- Source: deploy/timescaledb/init/001_silver_schema.sql Section 9
-- Runs after Phase 4 Silver table creation (auto-migrations)
-- Idempotent: Yes (CREATE OR REPLACE, guarded by table existence check)

CREATE SCHEMA IF NOT EXISTS analytics;

-- Only create views if Silver tables exist
DO $$ BEGIN
  IF NOT EXISTS (SELECT 1 FROM information_schema.tables
    WHERE table_schema = 'silver' AND table_name = 'weather_forecasts') THEN
    RAISE NOTICE 'ops-008 migration [001]: Silver tables not yet created, skipping analytics views';
    RETURN;
  END IF;

  -- Forecast Accuracy View
  CREATE OR REPLACE VIEW analytics.forecast_accuracy AS
  SELECT
      f.valid_time,
      f.issue_time,
      f.lead_time_hours,
      f.ndp_id,
      f.source_stream AS forecast_source,
      f.temperature_c AS forecast_temp_c,
      f.humidity_pct AS forecast_humidity_pct,
      f.wind_speed_kmh AS forecast_wind_kmh,
      f.precip_prob_pct AS forecast_precip_prob,
      o.temperature_c AS observed_temp_c,
      o.humidity_pct AS observed_humidity_pct,
      o.wind_speed_kmh AS observed_wind_kmh,
      ABS(f.temperature_c - o.temperature_c) AS temp_error_c,
      ABS(f.humidity_pct - o.humidity_pct) AS humidity_error_pct,
      ABS(f.wind_speed_kmh - o.wind_speed_kmh) AS wind_error_kmh,
      f.temperature_c - o.temperature_c AS temp_bias_c,
      f.humidity_pct - o.humidity_pct AS humidity_bias_pct,
      f.wind_speed_kmh - o.wind_speed_kmh AS wind_bias_kmh
  FROM silver.weather_forecasts f
  INNER JOIN silver.weather_observations o
      ON f.valid_time = o.observation_time
     AND f.ndp_id = o.ndp_id
  WHERE (f.dq_flags IS NULL OR array_length(f.dq_flags, 1) = 0)
    AND (o.dq_flags IS NULL OR array_length(o.dq_flags, 1) = 0);

  COMMENT ON VIEW analytics.forecast_accuracy IS
      'Joins forecasts to observations for accuracy analysis. Filters rows with DQ flags.';

  -- Indoor/Outdoor Comparison View
  CREATE OR REPLACE VIEW analytics.indoor_outdoor_comparison AS
  WITH indoor AS (
      SELECT
          time_bucket('1 hour', observation_time) AS hour,
          AVG(COALESCE(pm25_compensated, pm25)) AS indoor_pm25,
          AVG(co2) AS indoor_co2,
          AVG(COALESCE(temperature_c_compensated, temperature_c)) AS indoor_temp_c,
          AVG(COALESCE(humidity_pct_compensated, humidity_pct)) AS indoor_humidity_pct
      FROM silver.air_quality_observations
      WHERE location_type = 'indoor'
      GROUP BY 1
  ),
  outdoor_aq AS (
      SELECT
          time_bucket('1 hour', observation_time) AS hour,
          AVG(pm25) AS outdoor_pm25,
          AVG(o3) AS outdoor_ozone
      FROM silver.outdoor_air_quality
      GROUP BY 1
  ),
  weather AS (
      SELECT
          time_bucket('1 hour', observation_time) AS hour,
          AVG(temperature_c) AS outdoor_temp_c,
          AVG(humidity_pct) AS outdoor_humidity_pct,
          AVG(wind_speed_kmh) AS outdoor_wind_kmh
      FROM silver.weather_observations
      GROUP BY 1
  )
  SELECT
      COALESCE(i.hour, o.hour, w.hour) AS hour,
      i.indoor_pm25,
      i.indoor_co2,
      i.indoor_temp_c,
      i.indoor_humidity_pct,
      o.outdoor_pm25,
      o.outdoor_ozone,
      w.outdoor_temp_c,
      w.outdoor_humidity_pct,
      w.outdoor_wind_kmh,
      i.indoor_pm25 - o.outdoor_pm25 AS pm25_differential,
      i.indoor_temp_c - w.outdoor_temp_c AS temp_differential_c,
      CASE
          WHEN o.outdoor_pm25 < i.indoor_pm25 * 0.8
               AND w.outdoor_temp_c BETWEEN 18 AND 26
               AND w.outdoor_humidity_pct < 80
          THEN 'OPEN_WINDOWS'
          WHEN o.outdoor_pm25 > i.indoor_pm25 * 1.2
          THEN 'KEEP_CLOSED'
          ELSE 'NEUTRAL'
      END AS window_recommendation
  FROM indoor i
  FULL OUTER JOIN outdoor_aq o ON i.hour = o.hour
  FULL OUTER JOIN weather w ON COALESCE(i.hour, o.hour) = w.hour;

  COMMENT ON VIEW analytics.indoor_outdoor_comparison IS
      'Compares indoor and outdoor conditions hourly for window management decisions.';

  -- Latest Readings View
  CREATE OR REPLACE VIEW analytics.latest_readings AS
  WITH latest_indoor AS (
      SELECT DISTINCT ON (ndp_id)
          ndp_id,
          observation_time,
          location_path,
          co2,
          COALESCE(pm25_compensated, pm25) AS pm25,
          COALESCE(temperature_c_compensated, temperature_c) AS temperature_c,
          COALESCE(humidity_pct_compensated, humidity_pct) AS humidity_pct,
          tvoc_index,
          nox_index
      FROM silver.air_quality_observations
      ORDER BY ndp_id, observation_time DESC
  ),
  latest_outdoor_aq AS (
      SELECT DISTINCT ON (ndp_id)
          ndp_id,
          observation_time,
          aqi_owm,
          pm25,
          o3,
          no2
      FROM silver.outdoor_air_quality
      ORDER BY ndp_id, observation_time DESC
  ),
  latest_weather AS (
      SELECT DISTINCT ON (ndp_id)
          ndp_id,
          observation_time,
          station_id,
          temperature_c,
          humidity_pct,
          wind_speed_kmh,
          wind_direction_deg,
          text_description
      FROM silver.weather_observations
      ORDER BY ndp_id, observation_time DESC
  )
  SELECT
      'indoor_aq' AS data_type,
      i.ndp_id,
      i.observation_time,
      i.location_path AS location,
      jsonb_build_object(
          'co2', i.co2,
          'pm25', i.pm25,
          'temperature_c', i.temperature_c,
          'humidity_pct', i.humidity_pct,
          'tvoc_index', i.tvoc_index
      ) AS metrics
  FROM latest_indoor i
  UNION ALL
  SELECT
      'outdoor_aq' AS data_type,
      o.ndp_id,
      o.observation_time,
      NULL AS location,
      jsonb_build_object(
          'aqi_owm', o.aqi_owm,
          'pm25', o.pm25,
          'o3', o.o3,
          'no2', o.no2
      ) AS metrics
  FROM latest_outdoor_aq o
  UNION ALL
  SELECT
      'weather' AS data_type,
      w.ndp_id,
      w.observation_time,
      w.station_id AS location,
      jsonb_build_object(
          'temperature_c', w.temperature_c,
          'humidity_pct', w.humidity_pct,
          'wind_speed_kmh', w.wind_speed_kmh,
          'description', w.text_description
      ) AS metrics
  FROM latest_weather w;

  COMMENT ON VIEW analytics.latest_readings IS
      'Latest readings from all Silver tables for dashboard display.';

  RAISE NOTICE 'ops-008 migration [001]: Analytics views created — forecast_accuracy, indoor_outdoor_comparison, latest_readings';
END $$;

-- Grants (safe even if views were skipped — GRANT on empty schema is fine)
GRANT SELECT ON ALL TABLES IN SCHEMA analytics TO grafana_reader;
GRANT SELECT ON ALL TABLES IN SCHEMA analytics TO ndp_app;
