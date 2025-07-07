# 🚀 Neural Trader Codespaces Setup Guide

## Quick Fix for Docker Issues

If you're getting Docker permission errors, here are the solutions:

### Option 1: Restart the Codespace (Recommended)
The devcontainer configuration has been updated to properly support Docker-in-Docker. Simply:

1. **Rebuild the devcontainer**:
   - Press `Cmd/Ctrl + Shift + P`
   - Type "Rebuild Container"
   - Select "Dev Containers: Rebuild Container"
   - Wait for the rebuild to complete

### Option 2: Fix Docker Permissions Manually
If you can't rebuild, run these commands:

```bash
# Add vscode user to docker group and fix permissions
sudo usermod -aG docker vscode
sudo chmod 666 /var/run/docker.sock

# Start Docker daemon if not running
sudo service docker start

# Test Docker access
docker ps
```

### Option 3: Use Docker-in-Docker Commands
```bash
# Check if Docker daemon is running
sudo service docker status

# Start Docker daemon
sudo service docker start

# Test Docker
docker --version
docker-compose --version
```

## 🐳 Docker Setup Verification

After fixing permissions, verify everything works:

```bash
# Test basic Docker functionality
docker run hello-world

# Test Docker Compose
docker-compose --version

# Test building images
docker build -t test-image .
```

## 🔧 Environment Setup for Trading

1. **Set up your API key**:
   ```bash
   # Get a free API key from https://finnhub.io/register
   export FINNHUB_API_KEY="your_actual_key_here"
   export PRIMARY_PROVIDER="finnhub"
   ```

2. **Load secure passwords**:
   ```bash
   # Generate secure passwords if they don't exist
   if [ ! -f .env.generated ]; then
       ./scripts/generate-passwords.sh
   fi
   
   # Load the generated passwords
   source .env.generated
   ```

3. **Start the trading platform**:
   ```bash
   # Use the full stack script
   ./scripts/start_full_stock_simulation.sh
   ```

## 📊 Accessing Dashboards in Codespaces

Once Docker is working and services are running:

1. **Check the Ports tab** at the bottom of VS Code
2. **Click the globe icon** next to each port to open:
   - Port 3000: Grafana Dashboard
   - Port 9090: Prometheus Metrics
   - Port 8081: Redis Commander
   - Port 8082: pgAdmin

## 🛠️ Troubleshooting

### Docker Issues
```bash
# Check Docker daemon status
sudo service docker status

# Restart Docker daemon
sudo service docker restart

# Check Docker permissions
ls -la /var/run/docker.sock
groups $USER
```

### Port Access Issues
```bash
# List forwarded ports
gh codespace ports list

# Forward a port manually
gh codespace ports forward 3000:3000

# Make a port public temporarily
gh codespace ports visibility 3000:public
```

### Service Issues
```bash
# Check running containers
docker ps

# View service logs
docker-compose logs -f [service-name]

# Restart specific service
docker-compose restart [service-name]
```

## 🔐 Security Notes for Codespaces

- **Keep ports private** by default
- **Never commit API keys** to the repository
- **Use environment variables** for all secrets
- **Delete Codespace** when done to avoid charges
- **Set Codespace timeout** to automatically stop when idle

## 📱 Mobile Access

You can access Grafana and other dashboards from mobile:

1. Make the port **public temporarily**
2. Copy the external URL
3. Access from any device
4. **Set back to private** when done

## ⚡ Performance Tips

- **Use larger Codespace instances** for better performance:
  - 4-core minimum recommended
  - 8GB RAM minimum
- **Enable Codespace prebuild** for faster startup
- **Use Docker BuildKit** for faster builds (already configured)
- **Limit log retention** to save space:
  ```bash
  docker system prune -f
  ```

Ready to trade? Follow the setup steps above and run `./scripts/start_full_stock_simulation.sh`!