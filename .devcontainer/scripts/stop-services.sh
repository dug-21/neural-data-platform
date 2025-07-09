#!/bin/bash
# Stop all Podman services and clean up

echo "🛑 Stopping Neural Trader services..."

# Stop all pods (this stops all containers in the pods)
echo "📦 Stopping pods..."
podman pod stop neural-trader-db-pod 2>/dev/null || true
podman pod stop neural-trader-cache-pod 2>/dev/null || true
podman pod stop neural-trader-monitoring-pod 2>/dev/null || true
podman pod stop neural-trader-app-pod 2>/dev/null || true

# Remove pods if requested
if [ "$1" == "--clean" ]; then
    echo "🧹 Removing pods and containers..."
    podman pod rm -f neural-trader-db-pod 2>/dev/null || true
    podman pod rm -f neural-trader-cache-pod 2>/dev/null || true
    podman pod rm -f neural-trader-monitoring-pod 2>/dev/null || true
    podman pod rm -f neural-trader-app-pod 2>/dev/null || true
    
    # Remove any orphaned containers
    podman rm -f timescaledb pgadmin redis redis-commander prometheus grafana 2>/dev/null || true
fi

# Show status
echo ""
echo "📊 Current status:"
podman pod ps
podman ps -a

echo ""
echo "✅ Services stopped!"

if [ "$1" != "--clean" ]; then
    echo ""
    echo "💡 To remove pods completely, run: $0 --clean"
fi