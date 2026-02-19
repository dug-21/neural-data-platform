#!/bin/bash
set -e

echo "🚀 Setting up minimal development environment..."

# Update system packages
sudo apt-get update && sudo apt-get upgrade -y

# Install only essential system dependencies
sudo apt-get install -y \
    build-essential \
    pkg-config \
    libssl-dev \
    curl \
    git \
    jq \
    ripgrep \
    htop

# Install Claude Code CLI
echo "🤖 Installing Claude Code CLI..."
if ! command -v claude &> /dev/null; then
    npm install -g @anthropic-ai/claude-code
fi

# Install ruv-swarm
echo "🐝 Installing ruv-swarm..."
if ! command -v ruv-swarm &> /dev/null; then
    npm install -g ruv-swarm
fi

# Fix ruv-swarm permissions after install
echo "🔧 Checking ruv-swarm installation..."
# Re-check for ruv-swarm after installation
POSSIBLE_PATHS=(
    "$(npm config get prefix)/lib/node_modules/ruv-swarm"
    "$HOME/.npm-global/lib/node_modules/ruv-swarm"
    "/usr/local/lib/node_modules/ruv-swarm"
)

for path in "${POSSIBLE_PATHS[@]}"; do
    if [ -d "$path" ]; then
        sudo mkdir -p "$path/data" 2>/dev/null || true
        sudo chmod -R 777 "$path/data" 2>/dev/null || true
        echo "✅ ruv-swarm data directory configured at $path/data"
        break
    fi
done

# Wait for Claude to be available
echo "⏳ Waiting for Claude CLI..."
max_wait=30
waited=0
while ! command -v claude &> /dev/null; do
    if [ $waited -ge $max_wait ]; then
        echo "❌ Claude CLI installation timeout"
        break
    fi
    sleep 2
    waited=$((waited + 2))
done

# Debug: Show current user and home directory
echo "🔍 Debug info:"
echo "  Current user: $(whoami)"
echo "  Home directory: $HOME"
echo "  NPM prefix: $(npm config get prefix)"
echo "  Node version: $(node -v)"
echo "  NPM global packages location:"
npm list -g --depth=0 | head -5 || true

# Clarify .claude directories
echo ""
echo "📁 Understanding .claude directories:"
echo "  1. /home/vscode/.claude/ - Persistent volume for Claude auth/config"
echo "  2. /workspace/.claude/   - Project-specific Claude settings (CLAUDE.md, etc.)"
echo ""

# Create project directory for configs (NOT the same as user .claude)
mkdir -p /workspace/.claude

# Fix ruv-swarm permissions for database persistence
echo "🔧 Setting up ruv-swarm data directory..."
# Find where npm installs global packages
NPM_PREFIX=$(npm config get prefix)
echo "  NPM prefix: $NPM_PREFIX"

# Check multiple possible locations for ruv-swarm
POSSIBLE_PATHS=(
    "$NPM_PREFIX/lib/node_modules/ruv-swarm"
    "$HOME/.npm-global/lib/node_modules/ruv-swarm"
    "/usr/local/lib/node_modules/ruv-swarm"
)

RUV_SWARM_PATH=""
for path in "${POSSIBLE_PATHS[@]}"; do
    if [ -d "$path" ]; then
        RUV_SWARM_PATH="$path"
        echo "  Found ruv-swarm at: $RUV_SWARM_PATH"
        break
    fi
done

if [ -n "$RUV_SWARM_PATH" ]; then
    sudo mkdir -p "$RUV_SWARM_PATH/data" 2>/dev/null || true
    sudo chmod -R 777 "$RUV_SWARM_PATH/data" 2>/dev/null || true
    echo "✅ ruv-swarm permissions fixed at $RUV_SWARM_PATH/data"
else
    echo "⚠️  ruv-swarm not found in expected locations, will check after install"
fi

# Create symlink for Claude configuration persistence
echo "🔗 Setting up Claude configuration persistence..."

# Ensure the .claude directory exists (should be mounted as volume)
if [ ! -d "/home/vscode/.claude" ]; then
    echo "⚠️  Warning: /home/vscode/.claude directory not found. Volume may not be mounted."
    mkdir -p /home/vscode/.claude
fi

# Fix ownership and permissions of the .claude directory
echo "🔧 Setting up .claude directory permissions..."
sudo chown -R vscode:vscode /home/vscode/.claude 2>/dev/null || true
chmod -R 755 /home/vscode/.claude 2>/dev/null || true
echo "✅ .claude directory permissions set"

# Remove existing .claude.json if it's a regular file (not a symlink)
if [ -f "/home/vscode/.claude.json" ] && [ ! -L "/home/vscode/.claude.json" ]; then
    echo "  Removing existing .claude.json file to create symlink..."
    rm -f /home/vscode/.claude.json
fi

# Create symlink if it doesn't exist
if [ ! -e "/home/vscode/.claude.json" ]; then
    # Ensure the target file exists
    if [ ! -f "/home/vscode/.claude/claude.json" ]; then
        echo "  Creating new claude.json in persistent volume..."
        echo '{}' > /home/vscode/.claude/claude.json
    fi
    
    # Create the symlink
    ln -s /home/vscode/.claude/claude.json /home/vscode/.claude.json
    echo "✅ Claude configuration symlink created"
else
    echo "✅ Claude configuration symlink already exists"
fi

# Verify the symlink
if [ -L "/home/vscode/.claude.json" ]; then
    echo "✅ Claude configuration will persist across rebuilds"
    ls -la /home/vscode/.claude.json
else
    echo "⚠️  Warning: Claude configuration symlink creation may have failed"
fi

# Run MCP setup
if [ -f ".devcontainer/mcp_setup.sh" ]; then
    chmod +x .devcontainer/mcp_setup.sh
    .devcontainer/mcp_setup.sh
fi

# Basic git config
git config --global init.defaultBranch main
git config --global pull.rebase false

# Minimal Rust setup - just the essentials
echo "🦀 Setting up minimal Rust environment..."
rustup update
rustup component add clippy rustfmt rust-src

# Only install cargo-watch for development - it's the most useful
echo "📦 Installing cargo-watch (other tools can be installed on-demand)..."
cargo install cargo-watch

# Initialize ruv-swarm in the workspace
echo "🔧 Initializing ruv-swarm in workspace..."
cd /workspace && npx -y ruv-swarm init --claude || echo "ruv-swarm already initialized"

# Create simple aliases
cat >> ~/.bashrc << 'EOF'

# NPM global directory
export NPM_CONFIG_PREFIX=$HOME/.npm-global
export PATH=$NPM_CONFIG_PREFIX/bin:$PATH

# Essential aliases
alias ll='ls -la'
alias cb='cargo build'
alias ct='cargo test'
alias cr='cargo run'
alias cw='cargo watch'
alias cc='cargo clippy'
alias cf='cargo fmt'

# Claude and ruv-swarm
alias claude='claude'
alias swarm='ruv-swarm'
alias dsp='claude --dangerously-skip-permissions'
alias dspc='claude --dangerously-skip-permissions -c'

# Git shortcuts
alias gs='git status'
alias ga='git add'
alias gc='git commit'
alias gp='git push'
alias gl='git log --oneline'
alias gd='git diff'
EOF

source ~/.bashrc

echo "✅ Minimal setup complete!"
echo ""
echo "🚀 Available tools:"
echo "  - Rust: $(rustc --version)"
echo "  - Node.js: $(node --version)"
echo "  - Python: $(python3 --version)"
echo "  - Claude Code: $(claude --version 2>/dev/null || echo 'Installing...')"
echo "  - cargo-watch: for auto-recompiling"
echo ""
echo "💡 Additional Rust tools can be installed on-demand:"
echo "  cargo install cargo-edit     # for 'cargo add' command"
echo "  cargo install cargo-tree     # for dependency trees"
echo "  cargo install cargo-nextest  # for faster test runner"
echo ""
echo "🔧 To use the full setup with all tools, run:"
echo "  .devcontainer/setup.sh"