# Configuration Path Environment Variable Updates

## Summary
Updated configuration loading methods to respect environment variables instead of using hardcoded paths.

## Changes Made

### 1. Sector Models Configuration (`src/config/sector_models.rs`)
- **Updated**: `load_default()` method
- **Environment Variable**: `SECTOR_CONFIG_PATH`
- **Default**: `"config/sector_models.toml"`
- **Code**:
```rust
pub fn load_default() -> Result<Self> {
    let config_path = std::env::var("SECTOR_CONFIG_PATH")
        .unwrap_or_else(|_| "config/sector_models.toml".to_string());
    Self::load_from_file(&config_path)
}
```

### 2. Platform Configuration (`src/config/mod.rs`)
- **Updated**: `load_default_config()`, `load_production_config()`, `load_development_config()`
- **Environment Variable**: `PLATFORM_CONFIG_PATH`
- **Defaults**: 
  - Default: `"config/platform.toml"`
  - Production: `"config/production.toml"`
  - Development: `"config/development.toml"`
- **Code**:
```rust
pub fn load_default_config() -> Result<PlatformConfig> {
    let config_path = std::env::var("PLATFORM_CONFIG_PATH")
        .unwrap_or_else(|_| DEFAULT_CONFIG_PATH.to_string());
    PlatformConfig::load_from_file(&config_path)
}
```

### 3. Boolean Parsing Fix (`src/main.rs`)
- **Fixed**: Environment variable boolean parsing
- **Changed from**: `.parse::<bool>()` (which fails on "true"/"false" strings)
- **Changed to**: `.map(|v| v.to_lowercase() == "true")`
- **Affected Variables**:
  - `ENABLE_SECTOR_MODELS`
  - `ENABLE_AUTONOMOUS_TRAINING`
  - `ENABLE_REALTIME_ADAPTATION`
  - `ENABLE_DATA_DISCOVERY`

## Environment Variables Summary

### Configuration Paths
- `PLATFORM_CONFIG_PATH` - Main platform configuration file
- `SECTOR_CONFIG_PATH` - Sector models configuration file
- `AUTONOMOUS_TRAINING_CONFIG` - Autonomous training configuration (not currently used in code)
- `DATA_REQUIREMENTS_CONFIG` - Data requirements configuration (not currently used in code)

### Feature Flags (Boolean)
- `ENABLE_SECTOR_MODELS` - Enable sector-based neural architecture
- `ENABLE_AUTONOMOUS_TRAINING` - Enable autonomous training system
- `ENABLE_REALTIME_ADAPTATION` - Enable real-time adaptation
- `ENABLE_DATA_DISCOVERY` - Enable data discovery system

## Docker Configuration
The environment variables are properly set in:
- `Dockerfile.simple` - Uses absolute paths (`/config/...`)
- `docker-compose.prod.yml` - Passes through environment variables

## Notes
1. All configuration loading now respects environment variables
2. Hardcoded paths have been replaced with environment-aware loading
3. Boolean parsing is now consistent and case-insensitive
4. The autonomous training and data requirements config files are defined in environment variables but not currently loaded by the application