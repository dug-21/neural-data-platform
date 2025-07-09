#!/bin/bash
set -e

echo "🔄 Running minimal post-start configuration..."

# Debug: Check volume mount
echo "🔍 Checking persistent volume mount:"
if mount | grep -q "/home/vscode/.claude"; then
    echo "✅ Claude volume is mounted"
    mount | grep "/home/vscode/.claude"
else
    echo "⚠️  Claude volume not detected in mounts"
fi

echo "📁 Directory contents:"
ls -la /home/vscode/ | grep -E "claude|\.claude" || echo "No claude files found"

echo "🔍 Checking Claude symlink:"
if [ -L "/home/vscode/.claude.json" ]; then
    echo "✅ Symlink exists:"
    ls -la /home/vscode/.claude.json
    echo "  Target: $(readlink /home/vscode/.claude.json)"
else
    echo "⚠️  No symlink found at /home/vscode/.claude.json"
fi

echo "🔍 Checking .claude directory:"
if [ -d "/home/vscode/.claude" ]; then
    echo "✅ .claude directory exists:"
    ls -la /home/vscode/.claude/
    
    # Fix permissions if needed
    if [ ! -w "/home/vscode/.claude" ]; then
        echo "🔧 Fixing .claude directory permissions..."
        sudo chown -R vscode:vscode /home/vscode/.claude 2>/dev/null || true
        chmod -R 755 /home/vscode/.claude 2>/dev/null || true
    fi
else
    echo "⚠️  .claude directory not found"
fi

# Ensure permissions
chmod +x .devcontainer/*.sh 2>/dev/null || true

# Update Rust toolchain
rustup update

# Only fetch dependencies, don't build everything
if [ -f "Cargo.toml" ]; then
    echo "📦 Fetching Rust dependencies (not building)..."
    cargo fetch
fi

# Install Python dependencies if requirements.txt exists
if [ -f "data_ingestion/requirements.txt" ]; then
    echo "📦 Installing Python dependencies..."
    pip3 install --user -r data_ingestion/requirements.txt
fi

# Quick MCP check
if command -v claude &> /dev/null; then
    echo "📋 MCP servers configured:"
    claude mcp list 2>/dev/null || echo "  MCP not configured yet"
fi

echo "✅ Minimal post-start complete!"
echo "🚀 Ready for development!"
echo ""
echo "💡 Tips:"
echo "  - Use 'cargo watch -x run' to auto-rebuild on changes"
echo "  - Use 'cargo check' for fast syntax checking"
echo "  - Run full setup later with: .devcontainer/setup.sh"