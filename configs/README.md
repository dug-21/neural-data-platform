# Neural Trader Configurations

This directory contains configuration files for the Neural Trader platform.

## GitOps Workflow

In production, these configs should be in a separate repository for true GitOps:
- Development: `https://github.com/[your-org]/neural-trader-configs.git` (branch: `dev`)
- Staging: `https://github.com/[your-org]/neural-trader-configs.git` (branch: `staging`)  
- Production: `https://github.com/[your-org]/neural-trader-configs.git` (branch: `main`)

## Local Development

For local development, you have three options:

1. **Embedded Configs** (Fastest):
   ```bash
   CONFIG_FALLBACK=embedded docker-compose up
   ```

2. **Local Git Server**:
   ```bash
   # In one terminal
   git daemon --base-path=. --export-all --reuseaddr --informative-errors --verbose
   
   # In another terminal  
   CONFIG_REPO_URL=git://localhost/configs docker-compose up
   ```

3. **Fork & Push**:
   - Fork the config repo
   - Push your changes
   - Use your fork URL

## Directory Structure

```
configs/
├── base/               # Base configurations (all environments)
├── dev/                # Development overrides
├── staging/            # Staging overrides
├── production/         # Production overrides
└── schemas/            # JSON schemas for validation
```

## Environment Variables

- `CONFIG_REPO_URL`: Git repository URL (default: embedded configs)
- `CONFIG_BRANCH`: Git branch to use (default: main)
- `CONFIG_ENV`: Environment (dev/staging/production)
- `CONFIG_SYNC_INTERVAL`: How often to pull updates (default: 60s)