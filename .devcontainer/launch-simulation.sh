#!/bin/bash
# This script is meant to be run from INSIDE the dev container
# It creates a marker file that the host watches to trigger the simulation

echo "🚀 Requesting simulation launch..."

# Create a launch request file
cat > /workspace/.simulation-request.json << EOF
{
  "action": "launch",
  "timestamp": "$(date -u +"%Y-%m-%dT%H:%M:%SZ")",
  "config": {
    "build": true,
    "run": true,
    "services": ["postgres", "redis", "neural-trader"]
  }
}
EOF

echo "✅ Launch request created!"
echo ""
echo "The host system will detect this request and start the simulation containers."
echo "Check the host terminal for progress..."
echo ""
echo "📊 Once running, services will be available at:"
echo "  - Neural Trader API: http://localhost:3030"
echo "  - PostgreSQL: localhost:5432"
echo "  - Redis: localhost:6379"