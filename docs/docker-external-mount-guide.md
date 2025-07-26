# Docker External Drive Mounting Guide

This guide explains how to mount external drives and directories containing historical trading data into the Neural Trader Docker containers.

## Table of Contents

1. [Overview](#overview)
2. [Prerequisites](#prerequisites)
3. [Configuration Steps](#configuration-steps)
4. [Platform-Specific Instructions](#platform-specific-instructions)
5. [Security Considerations](#security-considerations)
6. [Troubleshooting](#troubleshooting)
7. [Examples](#examples)

## Overview

Neural Trader supports mounting external drives to access historical data without copying it into the container. This is useful for:

- Large historical datasets stored on external drives
- Network-attached storage (NAS) systems
- Shared data directories
- USB drives with market data archives

## Prerequisites

1. Docker and Docker Compose installed
2. External drive properly mounted on the host system
3. Read permissions for the Docker daemon to access the mounted path
4. Sufficient permissions for the user running Docker

## Configuration Steps

### 1. Enable External Drive Mounting

Edit your `.env` file (copy from `.env.example` if not exists):

```bash
# Enable external drive mounting
EXTERNAL_DATA_ENABLED=true

# Set the host path to your external drive
EXTERNAL_MOUNT_PATH=/path/to/your/external/drive

# Container path (usually keep default)
EXTERNAL_DATA_PATH=/mnt/external-data

# Mount mode (ro = read-only, rw = read-write)
EXTERNAL_MOUNT_MODE=ro
```

### 2. Verify Host Path

Ensure your external drive is mounted and accessible:

```bash
# Check if the path exists
ls -la /path/to/your/external/drive

# Check permissions
stat /path/to/your/external/drive
```

### 3. Update Docker Compose

The `docker-compose.yml` is already configured to use these environment variables. The mount is added to both `data-ingestion` and `neural-trader` services:

```yaml
volumes:
  # ... other volumes ...
  - ${EXTERNAL_MOUNT_PATH:-/mnt/external}:/mnt/external-data:${EXTERNAL_MOUNT_MODE:-ro}
```

## Platform-Specific Instructions

### Linux

1. **USB Drive Mounting**:
   ```bash
   # Find your device
   lsblk
   
   # Mount the device
   sudo mkdir -p /mnt/trading-data
   sudo mount /dev/sdb1 /mnt/trading-data
   
   # Set in .env
   EXTERNAL_MOUNT_PATH=/mnt/trading-data
   ```

2. **Auto-mount on Boot** (optional):
   ```bash
   # Edit fstab
   sudo nano /etc/fstab
   
   # Add line (replace UUID with your device's UUID from `blkid`)
   UUID=your-device-uuid /mnt/trading-data ext4 defaults,ro 0 2
   ```

### macOS

1. **USB Drive Path**:
   ```bash
   # USB drives typically mount to /Volumes
   ls /Volumes/
   
   # Set in .env
   EXTERNAL_MOUNT_PATH=/Volumes/YourDriveName
   ```

2. **Permission Issues**:
   ```bash
   # Grant Docker Desktop disk access in:
   # System Preferences > Security & Privacy > Privacy > Files and Folders
   ```

### Windows (WSL2)

1. **Access Windows Drives**:
   ```bash
   # Windows drives are mounted under /mnt in WSL2
   # For D: drive
   EXTERNAL_MOUNT_PATH=/mnt/d/trading-data
   ```

2. **USB Drive in WSL2**:
   ```bash
   # USB drives require additional setup in WSL2
   # See: https://docs.microsoft.com/en-us/windows/wsl/connect-usb
   ```

## Security Considerations

### 1. Read-Only Mounting (Recommended)

Always use read-only mounting for external data to prevent accidental modifications:

```bash
EXTERNAL_MOUNT_MODE=ro
```

### 2. File Permissions

Set appropriate permissions on the host:

```bash
# Restrict access to your user and Docker group
sudo chown -R $USER:docker /path/to/external/drive
sudo chmod -R 750 /path/to/external/drive
```

### 3. SELinux (Red Hat/CentOS)

If using SELinux, add the `:z` flag:

```yaml
volumes:
  - ${EXTERNAL_MOUNT_PATH}:/mnt/external-data:ro,z
```

### 4. AppArmor (Ubuntu)

May need to update AppArmor profiles for Docker:

```bash
sudo aa-complain /usr/bin/docker
```

## Troubleshooting

### Permission Denied Errors

1. **Check Docker daemon permissions**:
   ```bash
   # Add your user to docker group
   sudo usermod -aG docker $USER
   # Log out and back in
   ```

2. **Check directory permissions**:
   ```bash
   ls -la /path/to/external/drive
   # Should be readable by your user or docker group
   ```

3. **SELinux context** (if applicable):
   ```bash
   # Check context
   ls -Z /path/to/external/drive
   
   # Set Docker context
   sudo chcon -Rt svirt_sandbox_file_t /path/to/external/drive
   ```

### Mount Not Visible in Container

1. **Verify environment variables**:
   ```bash
   docker-compose config | grep -A5 volumes
   ```

2. **Check container mounts**:
   ```bash
   docker exec neural_trader_data_ingestion ls -la /mnt/external-data
   ```

3. **Inspect container**:
   ```bash
   docker inspect neural_trader_data_ingestion | grep -A10 Mounts
   ```

### Performance Issues

1. **Use local SSD for best performance**
2. **Avoid network mounts for real-time data**
3. **Consider copying frequently accessed data locally**

## Examples

### Example 1: USB Drive with Historical CSV Data

```bash
# .env configuration
EXTERNAL_DATA_ENABLED=true
EXTERNAL_MOUNT_PATH=/media/user/TradingData
EXTERNAL_DATA_PATH=/mnt/external-data
EXTERNAL_MOUNT_MODE=ro

# Directory structure on USB drive
/media/user/TradingData/
├── stocks/
│   ├── daily/
│   │   ├── AAPL.csv
│   │   ├── GOOGL.csv
│   │   └── MSFT.csv
│   └── intraday/
│       ├── AAPL_1min.csv
│       └── GOOGL_1min.csv
└── forex/
    ├── EURUSD.csv
    └── GBPUSD.csv
```

### Example 2: Network Attached Storage (NAS)

```bash
# Mount NAS share
sudo mkdir -p /mnt/nas-trading
sudo mount -t nfs 192.168.1.100:/trading-data /mnt/nas-trading

# .env configuration
EXTERNAL_DATA_ENABLED=true
EXTERNAL_MOUNT_PATH=/mnt/nas-trading
EXTERNAL_DATA_PATH=/mnt/external-data
EXTERNAL_MOUNT_MODE=ro
```

### Example 3: Multiple External Drives

For multiple drives, modify `docker-compose.yml`:

```yaml
volumes:
  # Primary external drive
  - ${EXTERNAL_MOUNT_PATH:-/mnt/external}:/mnt/external-data:ro
  # Secondary drive (add manually)
  - /mnt/backup-drive:/mnt/backup-data:ro
  # Archive drive
  - /mnt/archive:/mnt/archive-data:ro
```

### Example 4: Development with Local Data Directory

```bash
# .env configuration for development
EXTERNAL_DATA_ENABLED=true
EXTERNAL_MOUNT_PATH=/home/developer/trading-data
EXTERNAL_DATA_PATH=/mnt/external-data
EXTERNAL_MOUNT_MODE=rw  # Allow writes in development
```

## Using External Data in Applications

Once mounted, access the data in your Python scripts:

```python
import os
import pandas as pd

# Check if external data is available
external_data_path = os.environ.get('EXTERNAL_DATA_PATH', '/mnt/external-data')
external_data_enabled = os.environ.get('EXTERNAL_DATA_ENABLED', 'false').lower() == 'true'

if external_data_enabled and os.path.exists(external_data_path):
    # Read historical data
    csv_path = os.path.join(external_data_path, 'stocks/daily/AAPL.csv')
    if os.path.exists(csv_path):
        df = pd.read_csv(csv_path)
        print(f"Loaded {len(df)} rows from external drive")
else:
    print("External data not available, using default data source")
```

## Best Practices

1. **Always use read-only mounts** for production data
2. **Test with small datasets** before mounting large drives
3. **Monitor disk I/O performance** when using external drives
4. **Create backups** before enabling write access
5. **Document your mount paths** in your project README
6. **Use environment variables** for flexibility across environments
7. **Implement fallback logic** when external data is unavailable

## Additional Resources

- [Docker Volumes Documentation](https://docs.docker.com/storage/volumes/)
- [Docker Compose Volume Configuration](https://docs.docker.com/compose/compose-file/compose-file-v3/#volumes)
- [Linux File Permissions Guide](https://www.linux.com/training-tutorials/understanding-linux-file-permissions/)
- [Docker Security Best Practices](https://docs.docker.com/develop/security-best-practices/)