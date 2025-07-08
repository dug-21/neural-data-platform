#!/bin/bash

# Setup External Docker Host for Neural Trader
# Configures docker-out-of-docker for Codespaces to use external build resources

set -e

echo "🐳 External Docker Host Setup"
echo "============================"
echo ""

# Check if we're in Codespaces
if [ -z "$CODESPACES" ]; then
    echo "⚠️  Warning: Not running in GitHub Codespaces"
    echo "This script is optimized for Codespaces environments"
    read -p "Continue anyway? (y/n): " continue_anyway
    if [ "$continue_anyway" != "y" ]; then
        exit 0
    fi
fi

# Function to test Docker connection
test_docker_connection() {
    local host=$1
    echo -n "Testing connection to $host... "
    if DOCKER_HOST=$host docker version &>/dev/null; then
        echo "✅ Success"
        return 0
    else
        echo "❌ Failed"
        return 1
    fi
}

# Menu for setup options
echo "🔧 External Docker Setup Options:"
echo ""
echo "1) Use Codespaces host Docker daemon (recommended)"
echo "2) Connect to remote Docker host via SSH"
echo "3) Use Docker-in-Docker with resource limits"
echo "4) Configure BuildKit remote builder"
echo "5) Setup hybrid approach (local + remote)"
echo ""

read -p "Select option (1-5): " option

case $option in
    1)
        echo ""
        echo "🚀 Setting up Codespaces host Docker daemon..."
        
        # Check for host Docker socket
        if [ -S /var/run/docker-host.sock ]; then
            echo "✅ Found host Docker socket"
            
            # Create symlink for compatibility
            sudo ln -sf /var/run/docker-host.sock /var/run/docker.sock
            
            # Test connection
            if test_docker_connection "unix:///var/run/docker-host.sock"; then
                # Update shell configuration
                echo "" >> ~/.bashrc
                echo "# Neural Trader Docker configuration" >> ~/.bashrc
                echo "export DOCKER_HOST=unix:///var/run/docker-host.sock" >> ~/.bashrc
                echo "export DOCKER_BUILDKIT=1" >> ~/.bashrc
                echo "export COMPOSE_DOCKER_CLI_BUILD=1" >> ~/.bashrc
                
                echo ""
                echo "✅ Host Docker daemon configured successfully!"
                echo "   Please run: source ~/.bashrc"
            else
                echo "❌ Failed to connect to host Docker daemon"
                exit 1
            fi
        else
            echo "❌ Host Docker socket not found"
            echo "   Make sure your Codespace has Docker access enabled"
            exit 1
        fi
        ;;
        
    2)
        echo ""
        echo "🚀 Setting up remote Docker host..."
        
        read -p "Enter remote Docker host (e.g., tcp://remote-host:2376): " remote_host
        read -p "Use TLS? (y/n): " use_tls
        
        if [ "$use_tls" = "y" ]; then
            read -p "Path to TLS certificates directory: " tls_path
            export DOCKER_TLS_VERIFY=1
            export DOCKER_CERT_PATH=$tls_path
        fi
        
        if test_docker_connection "$remote_host"; then
            # Save configuration
            echo "" >> ~/.bashrc
            echo "# Neural Trader Remote Docker configuration" >> ~/.bashrc
            echo "export DOCKER_HOST=$remote_host" >> ~/.bashrc
            echo "export DOCKER_BUILDKIT=1" >> ~/.bashrc
            echo "export COMPOSE_DOCKER_CLI_BUILD=1" >> ~/.bashrc
            
            if [ "$use_tls" = "y" ]; then
                echo "export DOCKER_TLS_VERIFY=1" >> ~/.bashrc
                echo "export DOCKER_CERT_PATH=$tls_path" >> ~/.bashrc
            fi
            
            echo ""
            echo "✅ Remote Docker host configured successfully!"
            echo "   Please run: source ~/.bashrc"
        else
            echo "❌ Failed to connect to remote Docker host"
            exit 1
        fi
        ;;
        
    3)
        echo ""
        echo "🚀 Setting up Docker-in-Docker with resource limits..."
        
        # Create docker-in-docker compose file
        cat > docker-dind.yml << 'EOF'
version: '3.8'

services:
  docker-dind:
    image: docker:dind
    privileged: true
    environment:
      - DOCKER_TLS_CERTDIR=/certs
    volumes:
      - docker-certs-ca:/certs/ca
      - docker-certs-client:/certs/client
      - dind-storage:/var/lib/docker
    networks:
      - docker-net
    ports:
      - "2376:2376"
    deploy:
      resources:
        limits:
          memory: 4G
          cpus: '2.0'
        reservations:
          memory: 2G
          cpus: '1.0'

  docker-proxy:
    image: docker:cli
    environment:
      - DOCKER_HOST=tcp://docker-dind:2376
      - DOCKER_TLS_CERTDIR=/certs
      - DOCKER_TLS_VERIFY=1
      - DOCKER_CERT_PATH=/certs/client
    volumes:
      - docker-certs-client:/certs/client:ro
    networks:
      - docker-net
    command: ["sh", "-c", "while true; do sleep 3600; done"]

volumes:
  docker-certs-ca:
  docker-certs-client:
  dind-storage:

networks:
  docker-net:
    driver: bridge
EOF
        
        echo "Starting Docker-in-Docker service..."
        docker-compose -f docker-dind.yml up -d
        
        echo ""
        echo "✅ Docker-in-Docker configured with resource limits"
        echo "   Memory limit: 4GB"
        echo "   CPU limit: 2 cores"
        echo ""
        echo "To use: export DOCKER_HOST=tcp://localhost:2376"
        ;;
        
    4)
        echo ""
        echo "🚀 Setting up BuildKit remote builder..."
        
        # Create BuildKit configuration
        mkdir -p ~/.docker
        cat > ~/.docker/buildx-config.toml << 'EOF'
[registry."docker.io"]
  mirrors = ["mirror.gcr.io"]

[worker.oci]
  max-parallelism = 4

[gc]
  enabled = true
  keepStorage = "10GB"
  keepDuration = "168h"
  
[[policy]]
  all = true
  keepBytes = "10GB"
  keepDuration = "168h"
EOF
        
        # Create buildx builder
        docker buildx create \
            --name neural-trader-builder \
            --driver docker-container \
            --driver-opt network=host \
            --config ~/.docker/buildx-config.toml \
            --use
        
        echo ""
        echo "✅ BuildKit remote builder configured"
        echo "   Builder name: neural-trader-builder"
        echo "   Max storage: 10GB"
        echo "   Retention: 7 days"
        ;;
        
    5)
        echo ""
        echo "🚀 Setting up hybrid Docker approach..."
        
        # Create hybrid configuration script
        cat > ~/.docker/neural-trader-docker.sh << 'EOF'
#!/bin/bash

# Neural Trader Hybrid Docker Configuration
# Automatically selects best Docker strategy based on operation

# Function to get image size
get_image_size() {
    docker images --format "{{.Size}}" "$1" 2>/dev/null | head -1
}

# Function to check available space
check_available_space() {
    df -BG / | awk 'NR==2 {print $4}' | sed 's/G//'
}

# Determine which Docker to use
select_docker_host() {
    local operation=$1
    local available_space=$(check_available_space)
    
    if [ "$operation" = "build" ] && [ "$available_space" -lt 10 ]; then
        # Use external Docker for builds when space is low
        export DOCKER_HOST=${EXTERNAL_DOCKER_HOST:-unix:///var/run/docker-host.sock}
        echo "Using external Docker (low space: ${available_space}GB)" >&2
    else
        # Use local Docker
        unset DOCKER_HOST
        echo "Using local Docker (available space: ${available_space}GB)" >&2
    fi
}

# Wrapper for docker command
docker() {
    select_docker_host "$1"
    command docker "$@"
}

# Wrapper for docker-compose command
docker-compose() {
    select_docker_host "compose"
    command docker-compose "$@"
}

# Export functions
export -f docker
export -f docker-compose
export -f select_docker_host
EOF
        
        chmod +x ~/.docker/neural-trader-docker.sh
        
        echo "" >> ~/.bashrc
        echo "# Neural Trader Hybrid Docker" >> ~/.bashrc
        echo "source ~/.docker/neural-trader-docker.sh" >> ~/.bashrc
        
        echo ""
        echo "✅ Hybrid Docker approach configured"
        echo "   Automatically uses external Docker when space < 10GB"
        echo "   Please run: source ~/.bashrc"
        ;;
        
    *)
        echo "❌ Invalid option"
        exit 1
        ;;
esac

# Create Docker best practices guide
cat > ~/DOCKER_OPTIMIZATION.md << 'EOF'
# Docker Optimization Guide for Neural Trader

## Best Practices for Codespaces

### 1. Use BuildKit
Always enable BuildKit for better caching:
```bash
export DOCKER_BUILDKIT=1
export COMPOSE_DOCKER_CLI_BUILD=1
```

### 2. Multi-stage Builds
- Use specific stages for development vs production
- Copy only necessary files between stages
- Use cache mounts for package managers

### 3. Layer Caching
- Order Dockerfile commands from least to most frequently changing
- Separate dependency installation from code copying
- Use .dockerignore to exclude unnecessary files

### 4. Resource Management
- Set memory and CPU limits in docker-compose.yml
- Use tmpfs mounts for temporary data
- Regularly clean unused resources

### 5. External Builds
When running low on space:
```bash
# Use external Docker host
export DOCKER_HOST=unix:///var/run/docker-host.sock

# Or use BuildKit remote builder
docker buildx build --builder neural-trader-builder .
```

### 6. Monitoring
Check resource usage regularly:
```bash
docker system df
docker stats --no-stream
df -h /
```

### 7. Cleanup Commands
```bash
# Remove stopped containers
docker container prune -f

# Remove unused images
docker image prune -af

# Clean build cache (keep 1GB)
docker builder prune -f --keep-storage=1GB

# Full cleanup
docker system prune -af --volumes
```
EOF

echo ""
echo "📚 Created Docker optimization guide: ~/DOCKER_OPTIMIZATION.md"
echo ""
echo "🎉 Setup complete!"