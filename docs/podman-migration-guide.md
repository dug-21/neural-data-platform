# Docker to Podman Migration Guide for Neural Trader

## Executive Summary

This guide outlines the key differences between Docker and Podman that will affect the migration of the Neural Trader development environment. Podman offers enhanced security through rootless containers, simplified architecture without a daemon, and better integration with systemd, making it an excellent choice for both development and production environments.

## Key Differences and Practical Implications

### 1. Rootless Containers by Default

**Docker:**
- Runs with root privileges by default
- Docker daemon runs as root
- Container processes can potentially escape and gain root access

**Podman:**
- Runs rootless by default - containers run as your user
- No privilege escalation risks
- Better security isolation

**Practical Implications:**
```bash
# Docker (requires sudo or docker group)
sudo docker run -v /host/path:/container/path image

# Podman (no sudo needed)
podman run -v /host/path:/container/path image
```

**Migration Impact:**
- ✅ **Benefit**: Enhanced security without configuration changes
- ⚠️ **Consideration**: May need to adjust file permissions for volumes
- ⚠️ **Consideration**: Ports < 1024 require special handling in rootless mode

### 2. No Daemon - Direct Process Execution

**Docker:**
- Requires dockerd daemon running
- Client-server architecture
- Daemon manages all containers

**Podman:**
- No daemon required
- Fork-exec model (like regular Linux processes)
- Each container is a child process

**Practical Implications:**
```bash
# Docker - check daemon status
sudo systemctl status docker

# Podman - no daemon to manage
podman ps  # Works immediately
```

**Migration Impact:**
- ✅ **Benefit**: Simpler architecture, fewer points of failure
- ✅ **Benefit**: Lower resource overhead
- ✅ **Benefit**: Containers survive user logout (with systemd integration)

### 3. Pod Concept for Container Grouping

**Docker:**
- Containers are individual units
- Use docker-compose for multi-container apps
- Network namespace per container

**Podman:**
- Native pod support (like Kubernetes)
- Containers in a pod share network namespace
- Can generate Kubernetes YAML from pods

**Practical Implications:**
```bash
# Create a pod for Neural Trader services
podman pod create --name neural-trader-pod -p 5432:5432 -p 6379:6379 -p 3030:3030

# Add containers to the pod
podman run -d --pod neural-trader-pod --name timescaledb timescale/timescaledb
podman run -d --pod neural-trader-pod --name redis redis:alpine
```

**Migration Impact:**
- ✅ **Benefit**: Better container orchestration
- ✅ **Benefit**: Easier migration to Kubernetes
- 🔄 **Change**: Can use pods instead of docker-compose networking

### 4. Different Networking Model

**Docker:**
- Creates docker0 bridge by default
- Complex NAT and iptables rules
- Container-to-container communication via bridge

**Podman:**
- Uses CNI (Container Network Interface)
- Simpler networking model
- Rootless networking via slirp4netns

**Practical Implications:**
```yaml
# Docker Compose network
networks:
  neural_trader_net:
    driver: bridge
    ipam:
      config:
        - subnet: 172.20.0.0/16

# Podman equivalent - automatic with pods or:
podman network create neural_trader_net --subnet 172.20.0.0/16
```

**Migration Impact:**
- ✅ **Benefit**: More standard networking approach
- ⚠️ **Consideration**: Custom networks need explicit creation
- ⚠️ **Consideration**: Different DNS resolution in rootless mode

### 5. Volume Mount Differences

**Docker:**
- Manages volumes in /var/lib/docker/volumes
- Root ownership by default
- SELinux labels often problematic

**Podman:**
- Rootless volumes in ~/.local/share/containers/storage
- User ownership by default
- Better SELinux integration

**Practical Implications:**
```bash
# Docker volume with permissions issues
docker run -v ./data:/data image  # Often requires chmod/chown

# Podman with automatic user mapping
podman run -v ./data:/data:Z image  # :Z for SELinux context
```

**Migration Impact:**
- ✅ **Benefit**: Fewer permission issues
- ✅ **Benefit**: Better security with SELinux
- ⚠️ **Consideration**: Use :Z or :z flags for SELinux systems

### 6. Docker Compose vs Podman Alternatives

**Docker Compose:**
```yaml
version: '3.8'
services:
  timescaledb:
    build: ./docker/timescaledb
    environment:
      - POSTGRES_PASSWORD=${POSTGRES_PASSWORD}
```

**Podman Options:**

**Option 1: podman-compose (Drop-in replacement)**
```bash
pip install podman-compose
podman-compose up -d
```

**Option 2: Podman pods with systemd**
```bash
# Generate systemd units from running containers
podman generate systemd --new --files --name neural-trader-pod
```

**Option 3: Kubernetes YAML**
```bash
podman generate kube neural-trader-pod > neural-trader.yaml
podman play kube neural-trader.yaml
```

**Migration Impact:**
- ✅ **Benefit**: Multiple deployment options
- ✅ **Benefit**: Easy Kubernetes migration path
- 🔄 **Change**: Choose best approach for your workflow

### 7. SELinux Considerations

**Docker:**
- Often conflicts with SELinux
- Requires manual label management
- Common source of permission errors

**Podman:**
- Designed with SELinux in mind
- Automatic label handling
- Use :Z (private) or :z (shared) mount options

**Practical Implications:**
```bash
# Podman with SELinux
podman run -v ./config:/app/config:ro,Z nginx  # Private unshared label
podman run -v ./shared:/data:z nginx           # Shared label
```

**Migration Impact:**
- ✅ **Benefit**: Works out-of-box on SELinux systems
- ✅ **Benefit**: Better security isolation
- 📝 **Note**: Add :Z or :z to volume mounts

### 8. Systemd Integration Benefits

**Docker:**
- Requires docker.service
- Basic systemd integration
- Containers stop when daemon stops

**Podman:**
- Native systemd integration
- Can run containers as systemd services
- Containers can use systemd inside

**Practical Implications:**
```bash
# Create systemd service for Neural Trader
podman create --name neural-trader-app your-image
podman generate systemd --new --name neural-trader-app > ~/.config/systemd/user/neural-trader.service
systemctl --user enable --now neural-trader.service
```

**Migration Impact:**
- ✅ **Benefit**: Better service management
- ✅ **Benefit**: Automatic container restart
- ✅ **Benefit**: User-level services (no root)

## Migration Strategy for Neural Trader

### Phase 1: Development Environment
1. Install Podman and podman-compose
2. Test with existing docker-compose.yml
3. Adjust volume permissions if needed
4. Update documentation

### Phase 2: Optimization
1. Convert to Podman pods for better integration
2. Implement systemd services for production
3. Optimize networking configuration
4. Enable SELinux protections

### Phase 3: Production Deployment
1. Generate Kubernetes manifests
2. Set up systemd services
3. Configure monitoring integration
4. Document operational procedures

## Specific Changes for Neural Trader

### Volume Mounts
```yaml
# Add SELinux context to all volumes
volumes:
  - ./data_ingestion:/app/data_ingestion:ro,Z
  - ./config:/app/config:ro,Z
  - ingestion_logs:/app/logs:Z
```

### Network Configuration
```bash
# Create network explicitly
podman network create neural_trader_net --subnet 172.20.0.0/16
```

### Resource Limits
```yaml
# Podman supports the same syntax
deploy:
  resources:
    limits:
      cpus: '4'
      memory: 8G
```

### Health Checks
```yaml
# Identical syntax, better integration
healthcheck:
  test: ["CMD-SHELL", "pg_isready -U neural_trader"]
  interval: 10s
```

## Benefits Summary

1. **Security**: Rootless by default, better isolation
2. **Simplicity**: No daemon, direct process management
3. **Compatibility**: Docker CLI compatibility, easy migration
4. **Kubernetes-ready**: Native pod support, YAML generation
5. **Performance**: Lower overhead, better resource usage
6. **Integration**: Native systemd support, SELinux-friendly
7. **Flexibility**: Multiple deployment options

## Common Issues and Solutions

### Issue: Port binding < 1024 in rootless mode
**Solution**: Use port forwarding or enable unprivileged port binding
```bash
echo "net.ipv4.ip_unprivileged_port_start=80" | sudo tee /etc/sysctl.d/99-rootless.conf
```

### Issue: Volume permission errors
**Solution**: Use proper SELinux labels and user namespaces
```bash
podman run -v ./data:/data:Z --userns=keep-id image
```

### Issue: Container name resolution
**Solution**: Use pod networking or explicit network creation
```bash
podman pod create --name myapp -p 8080:80
podman run -d --pod myapp --name web nginx
```

## Conclusion

Migrating from Docker to Podman for Neural Trader will provide enhanced security, simplified operations, and better production readiness. The migration can be done incrementally, starting with the development environment and gradually optimizing for Podman's unique features. The compatibility layer ensures minimal disruption while gaining significant operational benefits.