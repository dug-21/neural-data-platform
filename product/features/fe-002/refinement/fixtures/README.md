# FE-002 Golden Master Baseline Fixtures

## Purpose

These fixtures capture the **golden master baseline** of DDL output from `ndp-gold-ddl`
BEFORE the FE-002 YAML-to-JSON migration. They serve as the source of truth for
verifying that JSON-based configuration produces IDENTICAL DDL output.

## Captured Date

**2026-02-05**

## Tool Version

```
ndp-gold-ddl (built from commit on main branch)
Config format: JSON (config/base/streams/*/config.json)
Domain format: YAML (config/domains/indoor-air-quality/domain.yaml)
```

## Baseline Files

### Stream Continuous Aggregates (Sync Mode)

| File | Command | Description |
|------|---------|-------------|
| `stream_air-quality_sync.sql` | `ndp-gold-ddl generate --stream air-quality` | Hourly + daily CAs for indoor air quality |
| `stream_outdoor-weather_sync.sql` | `ndp-gold-ddl generate --stream outdoor-weather` | Hourly + daily CAs for weather |
| `stream_outdoor-air-quality_sync.sql` | `ndp-gold-ddl generate --stream outdoor-air-quality` | Hourly CA for outdoor AQI |
| `stream_home-assistant-state_sync.sql` | `ndp-gold-ddl generate --stream home-assistant-state` | Hourly CA for state events |

### Stream Continuous Aggregates (Recreate Mode)

| File | Command | Description |
|------|---------|-------------|
| `stream_air-quality_recreate.sql` | `ndp-gold-ddl generate --stream air-quality --action recreate` | Drop + create variant |
| `stream_outdoor-weather_recreate.sql` | `ndp-gold-ddl generate --stream outdoor-weather --action recreate` | Drop + create variant |
| `stream_outdoor-air-quality_recreate.sql` | `ndp-gold-ddl generate --stream outdoor-air-quality --action recreate` | Drop + create variant |
| `stream_home-assistant-state_recreate.sql` | `ndp-gold-ddl generate --stream home-assistant-state --action recreate` | Drop + create variant |

### State Transitions

| File | Command | Description |
|------|---------|-------------|
| `stream_home-assistant-state_transitions_sync.sql` | `ndp-gold-ddl generate --stream home-assistant-state --transitions` | State transition materialized view |
| `stream_home-assistant-state_transitions_recreate.sql` | `ndp-gold-ddl generate --stream home-assistant-state --transitions --action recreate` | Drop + create variant |

### Domain Aligned Views

| File | Command | Description |
|------|---------|-------------|
| `domain_indoor-air-quality_sync.sql` | `ndp-gold-ddl generate --domain indoor-air-quality` | Cross-stream aligned materialized view |
| `domain_indoor-air-quality_recreate.sql` | `ndp-gold-ddl generate --domain indoor-air-quality --action recreate` | Drop + create variant |

## Hash Verification

All baseline files are checksummed in `BASELINE-MANIFEST.sha256`.

To verify integrity:
```bash
cd product/features/fe-002/refinement/fixtures
sha256sum -c BASELINE-MANIFEST.sha256
```

## Streams with gold_etl Configuration

The following streams have `gold_etl` sections in their JSON configs:

1. **air-quality** (`config/base/streams/air-quality/config.json`)
   - Granularities: 1 hour, 1 day
   - Fields: pm25, pm10, co2, temperature_c, humidity_pct, tvoc_index, nox_index
   - Features: lag, rolling, trend

2. **outdoor-weather** (`config/base/streams/outdoor-weather/config.json`)
   - Granularities: 1 hour, 1 day
   - Fields: temperature_c, humidity_pct, wind_speed_kmh, pressure_pa, etc.
   - Features: lag, rolling, trend

3. **outdoor-air-quality** (`config/base/streams/outdoor-air-quality/config.json`)
   - Granularities: 1 hour
   - Fields: pm25, pm10, aqi_owm, aqi_epa, o3_ugm3, no2_ugm3, etc.
   - Features: lag, rolling

4. **home-assistant-state** (`config/base/streams/home-assistant-state/config.json`)
   - Granularities: 1 hour
   - Fields: state (count, first, last)
   - Transitions: enabled with duration tracking

## Domain Configuration

The domain `indoor-air-quality` (`config/domains/indoor-air-quality/domain.yaml`) combines:

| Alias | Stream ID | Role |
|-------|-----------|------|
| indoor | air-quality | primary |
| outdoor | outdoor-weather | context |
| state | home-assistant-state | actuator |
| outdoor_aqi | outdoor-air-quality | constraint |

## Verification Process for FE-002

After converting `domain.yaml` to `domain.json`:

1. Run the same commands against the new JSON config
2. Compare output to baseline files
3. Verify SHA256 hashes match

```bash
# Example verification
ndp-gold-ddl generate --domain indoor-air-quality > /tmp/new_output.sql
diff domain_indoor-air-quality_sync.sql /tmp/new_output.sql
```

## Notes

- Column ordering in generated DDL may vary between runs due to HashMap iteration order
- The semantic content must match even if column order differs
- Use `--action recreate` for destructive migrations
- Use `--action sync` (default) for idempotent operations
