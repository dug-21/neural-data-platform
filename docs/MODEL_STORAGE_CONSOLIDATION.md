# Model Storage Consolidation - Complete Implementation

## Overview

Successfully consolidated all model storage to `/opt/neural-trader/` to eliminate any risk to `/var/lib/neural-trader/config`. This change provides a cleaner, more secure architecture with consolidated persistence.

## New Model Storage Architecture

### Directory Structure
```
/opt/neural-trader/
├── models/              # ETF binary models (.bin files)
│   ├── XLF.bin
│   ├── XLK.bin
│   └── ...
└── sector-models/       # Versioned sector models (FANN .fann files)
    ├── XLF/
    │   └── model.fann
    ├── energy_base_model/
    │   ├── 1.1.0/
    │   │   └── model.ruv
    │   └── metadata.json
    └── ...
```

### Volume Mapping
- **Single Volume:** `neural_trader_models:/opt/neural-trader`
- **Complete Persistence:** Entire `/opt/neural-trader` directory is now persistent
- **No Risk:** `/var/lib/neural-trader/config` remains completely untouched

## Changes Implemented

### 1. Application Code Updates

#### `/src/main.rs`
- **Before:** `/var/lib/neural-trader/models/{symbol}/model.fann`
- **After:** `/opt/neural-trader/sector-models/{symbol}/model.fann`

#### Model Storage Configuration
- **`/src/adapters/model_storage.rs`:** Default path changed to `/opt/neural-trader/sector-models`
- **`/src/adapters/model_rollback.rs`:** Base directory updated to sector-models path
- **`/src/integration/model_persistence_service.rs`:** Storage path updated
- **`/src/bin/model_rollback_cli.rs`:** CLI default path updated

### 2. Docker Configuration Updates

#### Environment Variables
```dockerfile
ENV MODEL_STORAGE_PATH=/opt/neural-trader/sector-models
```

#### Volume Consolidation
```yaml
volumes:
  # Before: Two separate volumes
  # - neural_trader_etf_models:/opt/neural-trader/models
  # - neural_trader_sector_models:/var/lib/neural-trader/models
  
  # After: Single consolidated volume
  - neural_trader_models:/opt/neural-trader
```

#### Volume Definitions
```yaml
volumes:
  neural_trader_models:
    driver: local
  # Removed: neural_trader_etf_models, neural_trader_sector_models
```

### 3. Path Mapping Summary

| Model Type | Old Path | New Path | File Types |
|------------|----------|----------|------------|
| ETF Models | `/opt/neural-trader/models/` | `/opt/neural-trader/models/` | `.bin` files |
| Sector Models | `/var/lib/neural-trader/models/` | `/opt/neural-trader/sector-models/` | `.fann`, `.ruv` files |

## Benefits

1. **Security:** No risk to `/var/lib/neural-trader/config`
2. **Simplicity:** Single volume mount for all model persistence
3. **Consistency:** All models under one root directory
4. **Maintainability:** Clear separation between model types
5. **Docker Simplification:** Fewer volumes to manage

## Environment Variable Usage

The `MODEL_STORAGE_PATH` environment variable now points to `/opt/neural-trader/sector-models` and is used by:
- Model persistence service
- Model rollback system
- Model manager service
- CLI tools
- Init scripts

## Compatibility

- **ETF Models:** Continue to work with existing `.bin` format at `/opt/neural-trader/models/`
- **Sector Models:** Seamlessly migrated to `/opt/neural-trader/sector-models/`
- **Existing Volumes:** Safe to migrate by copying data between old and new locations

## Migration Process

When deploying:

1. **Stop Services:**
   ```bash
   docker-compose down
   ```

2. **Copy Existing Data (if needed):**
   ```bash
   # Copy ETF models (if they exist elsewhere)
   docker run --rm -v neural_trader_etf_models:/src -v neural_trader_models:/dst alpine cp -r /src/* /dst/models/
   
   # Copy sector models
   docker run --rm -v neural_trader_sector_models:/src -v neural_trader_models:/dst alpine cp -r /src/* /dst/sector-models/
   ```

3. **Start with New Configuration:**
   ```bash
   docker-compose up -d
   ```

## Verification

After deployment, verify the structure:
```bash
docker exec neural_trader_app ls -la /opt/neural-trader/
docker exec neural_trader_app ls -la /opt/neural-trader/models/
docker exec neural_trader_app ls -la /opt/neural-trader/sector-models/
```

## Files Modified

### Source Code
- `/src/main.rs`
- `/src/adapters/model_storage.rs`
- `/src/adapters/model_rollback.rs`
- `/src/integration/model_persistence_service.rs`
- `/src/bin/model_rollback_cli.rs`

### Docker Configuration
- `/docker/production/docker-compose.prod.yml`
- `/docker/production/Dockerfile`

### Status: ✅ COMPLETE
All model storage has been successfully consolidated under `/opt/neural-trader/` with clear separation between ETF and sector models, eliminating any risk to configuration files.