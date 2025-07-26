# Example Dockerfile showing proper permission setup for external mounts
# This demonstrates how to handle file permissions when mounting external drives

FROM python:3.11-slim

# Create a non-root user for security
ARG USER_ID=1000
ARG GROUP_ID=1000
ARG USERNAME=trader

# Create group and user with specific IDs to match host system
RUN groupadd -g ${GROUP_ID} ${USERNAME} && \
    useradd -u ${USER_ID} -g ${GROUP_ID} -m -s /bin/bash ${USERNAME}

# Install system dependencies
RUN apt-get update && apt-get install -y \
    # Required for external mount access
    fuse \
    # Useful for troubleshooting permissions
    sudo \
    # Clean up
    && rm -rf /var/lib/apt/lists/*

# Add user to sudo group for debugging (optional, remove in production)
RUN usermod -aG sudo ${USERNAME}
RUN echo "${USERNAME} ALL=(ALL) NOPASSWD:ALL" >> /etc/sudoers

# Create mount points with proper ownership
RUN mkdir -p /mnt/external-data && \
    chown ${USER_ID}:${GROUP_ID} /mnt/external-data && \
    chmod 755 /mnt/external-data

# Create application directory
RUN mkdir -p /app && \
    chown ${USER_ID}:${GROUP_ID} /app

# Set working directory
WORKDIR /app

# Copy requirements first for better caching
COPY --chown=${USER_ID}:${GROUP_ID} requirements.txt .

# Install Python dependencies
RUN pip install --no-cache-dir -r requirements.txt

# Copy application code
COPY --chown=${USER_ID}:${GROUP_ID} . .

# Create entrypoint script that handles permissions
RUN cat << 'EOF' > /docker-entrypoint.sh
#!/bin/bash
set -e

# Function to fix permissions on external mount
fix_external_permissions() {
    local mount_path="/mnt/external-data"
    
    if [ -d "$mount_path" ] && [ "$(ls -A $mount_path 2>/dev/null)" ]; then
        echo "External data found at $mount_path"
        
        # Check if we can read the directory
        if [ -r "$mount_path" ]; then
            echo "External data is readable"
            
            # List contents for debugging
            echo "External data contents:"
            ls -la "$mount_path" | head -10
        else
            echo "Warning: External data is not readable"
            echo "Current user: $(id)"
            echo "Mount permissions: $(ls -ld $mount_path)"
            
            # Attempt to fix permissions if running as root
            if [ "$(id -u)" = "0" ]; then
                echo "Attempting to fix permissions..."
                chmod -R +r "$mount_path" || echo "Failed to fix permissions"
            fi
        fi
    else
        echo "No external data mounted at $mount_path"
    fi
}

# Check external data permissions
fix_external_permissions

# Execute the original command
exec "$@"
EOF

RUN chmod +x /docker-entrypoint.sh

# Switch to non-root user
USER ${USERNAME}

# Set environment variables for external data
ENV EXTERNAL_DATA_PATH=/mnt/external-data
ENV EXTERNAL_DATA_ENABLED=false

# Use the entrypoint script
ENTRYPOINT ["/docker-entrypoint.sh"]

# Default command
CMD ["python", "app.py"]

# Example build command:
# docker build --build-arg USER_ID=$(id -u) --build-arg GROUP_ID=$(id -g) -t neural-trader-app .

# Example run command with proper mount:
# docker run -v /host/path:/mnt/external-data:ro neural-trader-app