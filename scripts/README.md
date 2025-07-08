# Neural Trader Startup Scripts

This directory contains various startup scripts for different environments and use cases.

## 🚀 Primary Startup Scripts

### For GitHub Codespaces (Limited Resources)

1. **`start_minimal_footprint.sh`** ⭐ RECOMMENDED
   - Runs only databases in Docker
   - Builds app locally to save disk space
   - Best for Codespaces with disk limitations

2. **`start_hybrid_mode.sh`**
   - Similar to minimal footprint but with more options
   - Includes hybrid docker-compose generation
   - Good for development with resource constraints

3. **`start_with_codespaces_env.sh`**
   - Ensures Codespaces secrets are passed to Docker
   - Creates temporary env file for Docker Compose
   - Use when environment variables aren't being recognized

### For Local Development (Full Resources)

1. **`start_full_stock_simulation.sh`** ⭐ RECOMMENDED
   - Original full-stack startup script
   - Runs everything in Docker
   - Best for local development with ample resources

2. **`start_full_stock_simulation_optimized.sh`**
   - Uses optimized Docker configurations
   - BuildKit caching enabled
   - Good for faster rebuilds

### For External Docker (Advanced)

1. **`start_external_docker.sh`**
   - Uses external Docker daemon (e.g., Docker Desktop)
   - For Codespaces with external Docker configured
   - Requires setup_external_docker.sh first

2. **`start_full_stock_simulation_external.sh`**
   - Full simulation using external Docker host
   - Most comprehensive external Docker solution

## 🛠️ Utility Scripts

- **`setup_external_docker.sh`** - Configure external Docker connection
- **`test_external_docker.sh`** - Verify external Docker is working
- **`stop_external_docker.sh`** - Stop containers on external Docker
- **`docker_cleanup.sh`** - Clean up Docker resources to free space
- **`generate_dev_secrets.sh`** - Generate development passwords/secrets

## 📋 Quick Decision Guide

**Question: Which script should I use?**

1. **In GitHub Codespaces?** → Use `start_minimal_footprint.sh`
2. **On local machine?** → Use `start_full_stock_simulation.sh`
3. **Need external Docker?** → Run `setup_external_docker.sh` then `start_external_docker.sh`
4. **Environment variables not working?** → Use `start_with_codespaces_env.sh`
5. **Want faster builds?** → Use `start_full_stock_simulation_optimized.sh`

## 🎯 Usage Examples

```bash
# Codespaces - Minimal approach
./scripts/start_minimal_footprint.sh

# Local development - Full stack
./scripts/start_full_stock_simulation.sh

# External Docker setup
./scripts/setup_external_docker.sh
./scripts/start_external_docker.sh
```

## 📝 Notes

- All scripts assume you're in the project root directory
- Scripts will check for required environment variables
- See `/HOW_TO_START_SIMULATION.md` for detailed instructions
- Check individual script headers for specific requirements