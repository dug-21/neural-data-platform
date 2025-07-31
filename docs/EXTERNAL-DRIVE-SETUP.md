# External Drive Setup Summary

Neural Trader now supports mounting external drives to access historical trading data, models, and other large datasets without copying them into Docker containers.

## Quick Start

1. **Enable external drive mounting:**
   ```bash
   # Run the setup script
   ./scripts/setup-external-mount.sh
   
   # Or manually edit .env
   EXTERNAL_DATA_ENABLED=true
   EXTERNAL_MOUNT_PATH=/path/to/your/external/drive
   ```

2. **Validate configuration:**
   ```bash
   ./scripts/validate-docker-config.sh
   ```

3. **Start services:**
   ```bash
   docker-compose up -d
   ```

4. **Test access:**
   ```bash
   docker exec neural_trader_data_ingestion ls -la /mnt/external-data
   ```

## What's Added

### 1. Docker Compose Updates
- Added external volume mounts to `data-ingestion` and `neural-trader` services
- Environment variables for external data configuration
- Read-only mounting by default for safety

### 2. Environment Configuration
Updated `.env.example` with external drive settings:
```bash
EXTERNAL_DATA_ENABLED=false
EXTERNAL_MOUNT_PATH=/mnt/external
EXTERNAL_DATA_PATH=/mnt/external-data
EXTERNAL_MOUNT_MODE=ro
```

### 3. Comprehensive Documentation
- **[docker-external-mount-guide.md](docker-external-mount-guide.md)**: Complete mounting guide
- Platform-specific instructions (Linux, macOS, Windows/WSL2)
- Security considerations and troubleshooting

### 4. Utilities and Scripts
- **[setup-external-mount.sh](../scripts/setup-external-mount.sh)**: Interactive setup script
- **[validate-docker-config.sh](../scripts/validate-docker-config.sh)**: Configuration validation
- **[external_data_utils.py](../utils/external_data_utils.py)**: Python utilities for external data

### 5. Examples and Templates
- **[docker-compose.override.example.yml](../docker-compose.override.example.yml)**: Customization examples
- **[docker-compose.advanced-mounts.yml](../examples/docker-compose.advanced-mounts.yml)**: Advanced mounting scenarios
- **[docker-permissions-example.dockerfile](docker-permissions-example.dockerfile)**: Permission handling examples

## Supported Scenarios

### Basic USB Drive
```bash
EXTERNAL_MOUNT_PATH=/media/user/TradingData
```

### Network Attached Storage (NAS)
```bash
# First mount the NAS share
sudo mount -t nfs 192.168.1.100:/trading-data /mnt/nas-trading
EXTERNAL_MOUNT_PATH=/mnt/nas-trading
```

### Multiple External Drives
Use `docker-compose.override.yml` to add additional mounts:
```yaml
services:
  data-ingestion:
    volumes:
      - /mnt/drive1:/mnt/historical-data:ro
      - /mnt/drive2:/mnt/backup-data:ro
```

### Development with Local Data
```bash
EXTERNAL_MOUNT_PATH=./local-data
EXTERNAL_MOUNT_MODE=rw  # Allow writes in development
```

## Security Features

- **Read-only by default**: Prevents accidental data modification
- **Permission validation**: Scripts check file access before mounting
- **User context**: Examples show proper user/group configuration
- **SELinux/AppArmor**: Documentation covers security contexts

## Troubleshooting

### Permission Denied
```bash
# Check permissions
ls -la /path/to/external/drive

# Fix permissions (if needed)
sudo chown -R $USER:docker /path/to/external/drive
sudo chmod -R 750 /path/to/external/drive
```

### Mount Not Visible
```bash
# Verify mount in container
docker exec neural_trader_data_ingestion ls -la /mnt/external-data

# Check Docker Compose config
docker-compose config | grep -A5 volumes
```

### Performance Issues
- Use local SSD for frequently accessed data
- Avoid network mounts for real-time processing
- Consider caching frequently used files locally

## Integration in Code

### Python Example
```python
import os
from utils.external_data_utils import ExternalDataManager

# Initialize manager
manager = ExternalDataManager()

# Check if external data is available
if manager.is_available():
    # Find CSV files
    csv_files = manager.find_data_files(extensions=['.csv'])
    
    # Read a specific file
    success, content = manager.read_file_safely('stocks/AAPL.csv')
    if success:
        # Process the data
        pass
```

### Environment Variables in Applications
```python
# Check configuration
external_enabled = os.environ.get('EXTERNAL_DATA_ENABLED', 'false').lower() == 'true'
external_path = os.environ.get('EXTERNAL_DATA_PATH', '/mnt/external-data')

# Use fallback if external data not available
if external_enabled and os.path.exists(external_path):
    data_source = external_path
else:
    data_source = '/app/data/default'
```

## Best Practices

1. **Always start with read-only mounts** for safety
2. **Test with small datasets** before mounting large drives
3. **Use the validation script** before starting services
4. **Monitor disk I/O** when using external drives
5. **Document your mount paths** in project documentation
6. **Implement fallback logic** when external data is unavailable

## Files Modified/Added

### Modified Files
- `docker-compose.yml`: Added external volume mounts and environment variables
- `.env.example`: Added external drive configuration section

### New Files
- `docs/docker-external-mount-guide.md`: Comprehensive mounting guide
- `scripts/setup-external-mount.sh`: Interactive setup script
- `scripts/validate-docker-config.sh`: Configuration validation script
- `utils/external_data_utils.py`: Python utilities for external data
- `docker-compose.override.example.yml`: Customization template
- `examples/docker-compose.advanced-mounts.yml`: Advanced examples
- `docs/docker-permissions-example.dockerfile`: Permission handling example

## Support

For issues or questions:
1. Check the [troubleshooting section](docker-external-mount-guide.md#troubleshooting) in the main guide
2. Run the validation script: `./scripts/validate-docker-config.sh`
3. Review Docker logs: `docker-compose logs -f`
4. Use the Python utilities to debug: `python utils/external_data_utils.py --check`

The external drive mounting feature provides flexible, secure access to large datasets while maintaining container isolation and data safety.