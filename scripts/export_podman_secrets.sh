#!/bin/bash
# Export Podman secrets as environment variables
# Usage: eval "$(./export_podman_secrets.sh)"

# Get all secrets and output export commands
podman secret ls --format '{{.Name}}' | while read -r secret; do
    if [ -n "$secret" ]; then
        # Convert to environment variable name
        env_var=$(echo "$secret" | tr '[:lower:]' '[:upper:]' | tr '-' '_')
        
        # Get secret value using podman run
        value=$(podman run --rm --secret="$secret" alpine sh -c "cat /run/secrets/$secret" 2>/dev/null)
        
        if [ -n "$value" ]; then
            # Output export command with proper syntax
            echo "export \"$env_var\"=\"$value\""
        fi
    fi
done