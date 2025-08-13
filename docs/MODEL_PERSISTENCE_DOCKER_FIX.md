# Model Persistence Docker Configuration Fix

## Issue Summary
The Docker configuration was not properly persisting neural network models between container restarts, causing models to be retrained on every startup.

## Root Cause Analysis
1. **Two model storage locations** existed in the container:
   - `/opt/neural-trader/models` - Contains ETF .bin files (XLF.bin, etc.)
   - `/var/lib/neural-trader/models` - Contains versioned sector models (energy_base_model/1.1.0/model.ruv, etc.)

2. **Volume persistence was commented out** in docker-compose.prod.yml (line 88)

3. **Config directory preservation** - `/var/lib/neural-trader/config` contains critical config files that must be preserved but not volume-mounted

## Solution Implemented

### 1. Docker Compose Volume Configuration
**File:** `/workspaces/neural-trader/docker/production/docker-compose.prod.yml`

**Changes:**
- **Line 88-91:** Added specific volume mounts for both model directories:
  ```yaml
  volumes:
    # Model persistence for both storage locations
    - neural_trader_etf_models:/opt/neural-trader/models
    - neural_trader_sector_models:/var/lib/neural-trader/models
    - neural_trader_logs:/var/log/neural-trader
  ```

- **Lines 286-289:** Added new volume definitions:
  ```yaml
  neural_trader_etf_models:
    driver: local
  neural_trader_sector_models:
    driver: local
  ```

### 2. Dockerfile Updates
**File:** `/workspaces/neural-trader/docker/production/Dockerfile`

**Changes:**
- **Lines 61-64:** Added `/var/lib/neural-trader` directory creation:
  ```dockerfile
  RUN mkdir -p /opt/neural-trader/{models,checkpoints,backup,exports,logs,config} \
      && mkdir -p /var/lib/neural-trader/{models,config} \
      && chown -R neural:neural /opt/neural-trader \
      && chown -R neural:neural /var/lib/neural-trader
  ```

- **Line 71:** Added config file copying to proper location:
  ```dockerfile
  COPY neural-trader-config/ /var/lib/neural-trader/config/
  ```

## Key Design Principles Followed

1. **Selective Volume Mounting:** Only mounted specific model directories, not the parent `/var/lib/neural-trader` directory
2. **Config Preservation:** Config files are copied during build and preserved in the image, not volume-mounted
3. **Dual Storage Support:** Both ETF model and sector model storage locations are persisted
4. **Security:** Proper ownership and permissions maintained for the neural user

## Benefits

1. **Model Persistence:** Models will survive container restarts
2. **Faster Startup:** No retraining required on restart
3. **Data Integrity:** Both storage locations properly persisted
4. **Configuration Safety:** Config files preserved without risk of external override

## Verification

To verify the fix:
1. Start the containers: `docker-compose -f docker/production/docker-compose.prod.yml up -d`
2. Train some models and let them save
3. Restart the neural-trader container: `docker-compose restart neural-trader`
4. Check that models are still available and no retraining occurs

## Files Modified

1. `/workspaces/neural-trader/docker/production/docker-compose.prod.yml`
2. `/workspaces/neural-trader/docker/production/Dockerfile`

## Model Storage Locations

- **ETF Models:** `/opt/neural-trader/models` → `neural_trader_etf_models` volume
- **Sector Models:** `/var/lib/neural-trader/models` → `neural_trader_sector_models` volume  
- **Config Files:** `/var/lib/neural-trader/config` → Copied during build (not volume-mounted)