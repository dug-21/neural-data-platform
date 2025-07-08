#!/bin/bash

# Docker Cleanup Script for Neural Trader
# Helps manage disk space in constrained environments like Codespaces

set -e

echo "🧹 Docker Cleanup Utility"
echo "========================"
echo ""

# Function to format bytes to human readable
format_bytes() {
    if [ $1 -lt 1024 ]; then
        echo "${1}B"
    elif [ $1 -lt 1048576 ]; then
        echo "$((($1 + 512) / 1024))KB"
    elif [ $1 -lt 1073741824 ]; then
        echo "$((($1 + 524288) / 1048576))MB"
    else
        echo "$((($1 + 536870912) / 1073741824))GB"
    fi
}

# Show current usage
echo "📊 Current Docker Usage:"
docker system df
echo ""

# Show disk usage
echo "💾 Disk Usage:"
df -h / | grep -v Filesystem
echo ""

# Calculate space to be freed
echo "🔍 Analyzing reclaimable space..."
RECLAIMABLE=$(docker system df --format "table {{.Reclaimable}}" | tail -n +2 | grep -oE '[0-9.]+[GMK]B' | head -1)
echo "  Potential space to free: $RECLAIMABLE"
echo ""

# Menu options
echo "🔧 Cleanup Options:"
echo "  1) Quick cleanup (stopped containers, unused networks)"
echo "  2) Standard cleanup (+ dangling images, build cache)"
echo "  3) Deep cleanup (+ unused images, keep recent)"
echo "  4) Full cleanup (⚠️  removes ALL unused resources)"
echo "  5) Custom cleanup (choose what to remove)"
echo "  6) Exit"
echo ""

read -p "Select option (1-6): " option

case $option in
    1)
        echo "🚀 Running quick cleanup..."
        docker container prune -f
        docker network prune -f
        ;;
    2)
        echo "🚀 Running standard cleanup..."
        docker system prune -f
        ;;
    3)
        echo "🚀 Running deep cleanup..."
        docker system prune -af --filter "until=24h"
        docker builder prune -f --keep-storage=1GB
        ;;
    4)
        echo "⚠️  WARNING: This will remove ALL unused Docker resources!"
        read -p "Are you sure? (yes/no): " confirm
        if [ "$confirm" = "yes" ]; then
            echo "🚀 Running full cleanup..."
            docker system prune -af --volumes
            docker builder prune -af
        else
            echo "❌ Cleanup cancelled"
            exit 0
        fi
        ;;
    5)
        echo "🎯 Custom cleanup options:"
        
        read -p "Remove stopped containers? (y/n): " remove_containers
        if [ "$remove_containers" = "y" ]; then
            docker container prune -f
        fi
        
        read -p "Remove unused networks? (y/n): " remove_networks
        if [ "$remove_networks" = "y" ]; then
            docker network prune -f
        fi
        
        read -p "Remove dangling images? (y/n): " remove_dangling
        if [ "$remove_dangling" = "y" ]; then
            docker image prune -f
        fi
        
        read -p "Remove ALL unused images? (y/n): " remove_unused
        if [ "$remove_unused" = "y" ]; then
            docker image prune -af
        fi
        
        read -p "Remove build cache? (y/n): " remove_cache
        if [ "$remove_cache" = "y" ]; then
            read -p "Keep how much cache? (e.g., 1GB, 500MB): " keep_cache
            docker builder prune -f --keep-storage=$keep_cache
        fi
        
        read -p "Remove unused volumes? (y/n): " remove_volumes
        if [ "$remove_volumes" = "y" ]; then
            docker volume prune -f
        fi
        ;;
    6)
        echo "👋 Exiting..."
        exit 0
        ;;
    *)
        echo "❌ Invalid option"
        exit 1
        ;;
esac

# Show results
echo ""
echo "✅ Cleanup complete!"
echo ""
echo "📊 Updated Docker Usage:"
docker system df
echo ""
echo "💾 Updated Disk Usage:"
df -h / | grep -v Filesystem
echo ""

# Additional tips
echo "💡 Additional Tips:"
echo "  - Use 'docker-compose down -v' to remove volumes when stopping services"
echo "  - Enable BuildKit: export DOCKER_BUILDKIT=1"
echo "  - Use .dockerignore to exclude unnecessary files from build context"
echo "  - Consider using --no-cache flag sparingly when building"
echo ""