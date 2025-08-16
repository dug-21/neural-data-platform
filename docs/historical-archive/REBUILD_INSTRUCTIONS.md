# Rebuild Instructions for Real Model Support

## Changes Made
1. Changed `use_real_models` default from `false` to `true` in the code
2. Fixed the EnhancedNeuralAdapter to respect the configuration
3. The .env file already has `NEURAL_USE_REAL_MODELS=true`

## To Apply Changes

### Option 1: Full Rebuild (Recommended)
```bash
# Stop the current containers
cd /workspaces/neural-trader/docker/production
docker-compose -f docker-compose.prod.yml down

# Rebuild the images with the new code
./build.sh

# Start with the new images
docker-compose -f docker-compose.prod.yml up -d

# Check logs to verify use_real_models=true
docker-compose -f docker-compose.prod.yml logs -f neural-trader | grep "use_real_models"
```

### Option 2: Quick Rebuild (Just the main service)
```bash
# From the project root
cd /workspaces/neural-trader

# Rebuild just the neural-trader image
docker build --no-cache -f docker/production/images/neural-trader.dockerfile -t neural-trader:prod .

# Restart the service
cd docker/production
docker-compose -f docker-compose.prod.yml restart neural-trader

# Check logs
docker-compose -f docker-compose.prod.yml logs -f neural-trader | grep "use_real_models"
```

## Expected Result
After rebuilding, you should see:
- `use_real_models=true` in all log messages
- Real models being used (DeepAR, TCN, etc.) when available
- Enhanced: X, Real: Y (where Y > 0 when real models are loaded)

## Troubleshooting
If you still see `use_real_models=false`:
1. Check that the .env file is being loaded: `docker-compose -f docker-compose.prod.yml config | grep NEURAL`
2. Verify the environment inside the container: `docker exec neural-trader env | grep NEURAL`
3. Ensure no cached layers: use `--no-cache` flag when building