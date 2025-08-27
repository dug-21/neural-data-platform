# Config Store Migration Scripts

This directory contains scripts and tools for migrating Neural Trader from environment-based configuration to the centralized config-store system.

## Files Overview

### Core Migration Scripts
- **`migrate_config.py`** - Main migration script that transfers configuration from environment variables and files to config-store
- **`config_store_setup.sh`** - Complete setup automation script that handles building, deployment, and migration
- **`validate_config_migration.py`** - Validation script to verify migration success and functionality
- **`requirements.txt`** - Python dependencies for migration scripts

### Migration Artifacts
- **`migration_report.json`** - Generated report with migration results and statistics
- **`validation_report.json`** - Generated report with validation test results

## Quick Start

### 1. Automated Setup (Recommended)
```bash
# Complete setup with all steps
./scripts/config_store_setup.sh

# Dry run to preview changes
./scripts/config_store_setup.sh --dry-run

# Skip Docker build (if images already exist)
./scripts/config_store_setup.sh --skip-build
```

### 2. Manual Step-by-Step Setup

#### Install Dependencies
```bash
pip3 install -r scripts/requirements.txt
```

#### Run Migration
```bash
# Dry run first
python3 scripts/migrate_config.py --dry-run

# Actual migration
python3 scripts/migrate_config.py
```

#### Validate Migration
```bash
python3 scripts/validate_config_migration.py
```

## Script Usage Details

### migrate_config.py

Migrates existing configuration from various sources to config-store.

```bash
python3 scripts/migrate_config.py [OPTIONS]

Options:
  --redis-url URL           Redis connection URL (default: redis://localhost:6379)
  --dry-run                 Preview changes without applying them
  --seed-file PATH          Output file for seed data (default: config/config_store_seed.json)
  --report-file PATH        Output file for migration report (default: scripts/migration_report.json)

Examples:
  # Basic migration
  python3 scripts/migrate_config.py
  
  # Dry run to preview
  python3 scripts/migrate_config.py --dry-run
  
  # Custom Redis URL
  python3 scripts/migrate_config.py --redis-url redis://custom-redis:6379
```

## Support

For issues or questions:
1. Check the troubleshooting section above
2. Review migration and validation reports
3. Check config-store service logs
4. Consult the main migration documentation at `docs/migration/CONFIG_STORE_MIGRATION.md`