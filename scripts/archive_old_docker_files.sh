#!/bin/bash
# Script to archive old Docker files for cleanup

set -e

# Create archive directory
ARCHIVE_DIR="docker/archived"
mkdir -p "$ARCHIVE_DIR"

echo "Archiving old Docker configurations..."

# Archive old docker-compose files
echo "Moving docker-compose files..."
find . -name "docker-compose*.yml" -not -path "./docker/production/*" -not -path "./docker/archived/*" -not -path "./vendor/*" -type f | while read -r file; do
    echo "  Moving: $file"
    mkdir -p "$ARCHIVE_DIR/$(dirname "$file")"
    mv "$file" "$ARCHIVE_DIR/$file"
done

# Archive old startup scripts
echo "Moving startup scripts..."
find ./scripts -name "*docker*.sh" -o -name "*start*.sh" -o -name "*stop*.sh" | grep -v "archive_old_docker_files.sh" | while read -r file; do
    echo "  Moving: $file"
    mv "$file" "$ARCHIVE_DIR/$file"
done

# Move the root Dockerfile to production
if [ -f "Dockerfile" ]; then
    echo "Moving root Dockerfile to docker/production/images/neural-trader-old.dockerfile"
    mv Dockerfile docker/production/images/neural-trader-old.dockerfile
fi

# Create a new simplified root Dockerfile that points to production
cat > Dockerfile << 'EOF'
# This Dockerfile has been moved to docker/production/images/neural-trader.dockerfile
# For production builds, use:
#   cd docker/production && ./build.sh
# 
# Or build directly:
#   docker build -f docker/production/images/neural-trader.dockerfile -t neural-trader:prod .
EOF

echo "Archive complete! Old files moved to: $ARCHIVE_DIR"
echo ""
echo "Production Docker setup is now in: docker/production/"
echo "To build: cd docker/production && ./build.sh"