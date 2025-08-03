# Debug Environment Variables

## Check what's actually running in your container:

```bash
# 1. Check the environment variables in the running container
docker exec neural-trader env | grep NEURAL

# 2. Check what docker-compose is using from the .env file
cd /workspaces/neural-trader/docker/production
docker-compose -f docker-compose.prod.yml config | grep -A5 -B5 NEURAL_USE_REAL_MODELS

# 3. Verify the .env file is being read
cat .env | grep NEURAL_USE_REAL_MODELS

# 4. Check if there's an override file
ls -la docker-compose.override.yml 2>/dev/null || echo "No override file"

# 5. See the actual config being used
docker inspect neural-trader | grep -A10 "Env"
```

## Possible Issues:

1. **Shell Environment Override**: If you have `NEURAL_USE_REAL_MODELS=false` exported in your shell, it will override the .env file.
   ```bash
   # Check your shell environment
   echo $NEURAL_USE_REAL_MODELS
   
   # If it's set to false, unset it
   unset NEURAL_USE_REAL_MODELS
   ```

2. **Wrong .env file**: Make sure you're using the .env in the docker/production directory
   ```bash
   # Should show NEURAL_USE_REAL_MODELS=true
   grep NEURAL_USE_REAL_MODELS /workspaces/neural-trader/docker/production/.env
   ```

3. **Cached Configuration**: Docker-compose might be using cached configuration
   ```bash
   # Force recreate with new config
   docker-compose -f docker-compose.prod.yml up -d --force-recreate neural-trader
   ```

## The Real Issue:

Looking at your log message again:
```
Generated 5 hybrid ensemble predictions using 3 models (Enhanced: 0, Real: 0, FANN: 2) with feature flag use_real_models=false
```

This suggests that even if the environment variable is set correctly, the code might not be reading it properly. Let me check...</content>
</invoke>