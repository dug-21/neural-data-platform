#!/bin/bash
set -e

echo "🚀 Setting up multi-language development environment..."

# Update system packages
sudo apt-get update && sudo apt-get upgrade -y

# Install additional system dependencies
sudo apt-get install -y \
    build-essential \
    pkg-config \
    libssl-dev \
    libsqlite3-dev \
    curl \
    wget \
    git \
    vim \
    htop \
    jq \
    ripgrep \
    fd-find \
    bat \
    exa \
    zsh \
    tmux

# Install Claude Code CLI
echo "🤖 Installing Claude Code CLI..."
if ! command -v claude &> /dev/null; then
    curl -fsSL https://claude.ai/install.sh | sh
fi

# Install ruv-swarm
echo "🐝 Installing ruv-swarm..."
if ! command -v ruv-swarm &> /dev/null; then
    npm install -g ruv-swarm
fi

# Set up MCP configuration directory
mkdir -p ~/.claude
mkdir -p ~/.claude/mcp

# Create Claude Code MCP configuration
cat > ~/.claude/mcp/config.json << 'EOF'
{
  "mcpServers": {
    "ruv-swarm": {
      "command": "npx",
      "args": ["ruv-swarm", "mcp", "start"],
      "transport": "stdio"
    },
    "github": {
      "command": "npx",
      "args": ["@modelcontextprotocol/server-github"],
      "transport": "stdio",
      "env": {
        "GITHUB_PERSONAL_ACCESS_TOKEN": "${GITHUB_TOKEN}"
      }
    },
    "filesystem": {
      "command": "npx",
      "args": ["@modelcontextprotocol/server-filesystem", "/workspaces/neural-trader"],
      "transport": "stdio"
    }
  }
}
EOF

# Create workspace-specific Claude configuration
cat > .claude/settings.json << 'EOF'
{
  "workspaceSettings": {
    "defaultModel": "claude-3-5-sonnet-20241022",
    "temperature": 0.1,
    "maxTokens": 4096
  },
  "mcpServers": {
    "ruv-swarm": {
      "command": "npx",
      "args": ["ruv-swarm", "mcp", "start"],
      "transport": "stdio"
    },
    "github": {
      "command": "npx",
      "args": ["@modelcontextprotocol/server-github"],
      "transport": "stdio"
    }
  },
  "hooks": {
    "pre-edit": "npx ruv-swarm hook pre-edit --file ${file}",
    "post-edit": "npx ruv-swarm hook post-edit --file ${file}",
    "pre-task": "npx ruv-swarm hook pre-task --description '${description}'",
    "post-task": "npx ruv-swarm hook post-task --task-id '${taskId}'"
  },
  "tools": {
    "enabled": ["all"],
    "disabled": []
  }
}
EOF

# Set up git configuration for Codespaces
git config --global init.defaultBranch main
git config --global pull.rebase false
git config --global user.name "Codespace User"
git config --global user.email "codespace@example.com"

# Create useful aliases
cat >> ~/.bashrc << 'EOF'

# Development aliases
alias ll='exa -la'
alias la='exa -la'
alias tree='exa --tree'
alias cat='bat'
alias find='fd'
alias grep='rg'

# Rust aliases
alias cb='cargo build'
alias ct='cargo test'
alias cr='cargo run'
alias cw='cargo watch'
alias cc='cargo clippy'
alias cf='cargo fmt'

# Node aliases
alias ni='npm install'
alias nr='npm run'
alias ns='npm start'
alias nt='npm test'
alias nb='npm run build'

# Python aliases
alias py='python3'
alias pip='pip3'
alias venv='python3 -m venv'

# Claude and ruv-swarm aliases
alias claude='claude --mcp'
alias swarm='ruv-swarm'
alias swarm-init='ruv-swarm swarm init'
alias swarm-status='ruv-swarm swarm status'

# Git aliases
alias gs='git status'
alias ga='git add'
alias gc='git commit'
alias gp='git push'
alias gl='git log --oneline'
alias gd='git diff'
EOF


# Install oh-my-zsh for better shell experience
# if [ ! -d "$HOME/.oh-my-zsh" ]; then
#    sh -c "$(curl -fsSL https://raw.github.com/ohmyzsh/ohmyzsh/master/tools/install.sh)" "" --unattended
# fi

# Set up Rust environment
echo "🦀 Setting up Rust environment..."
rustup update
rustup component add clippy rustfmt rust-src
cargo install cargo-watch cargo-edit cargo-tree cargo-audit

# Install common Rust tools
cargo install \
    tokio-console \
    cargo-nextest \
    cargo-deny \
    cargo-outdated \
    cargo-udeps \
    cargo-expand

# Set up Node.js environment
echo "📦 Setting up Node.js environment..."
npm install -g \
    typescript \
    ts-node \
    @types/node \
    eslint \
    prettier \
    nodemon \
    pm2 \
    create-react-app \
    @vue/cli \
    @angular/cli

# Set up Python environment
echo "🐍 Setting up Python environment..."
pip install --upgrade pip
pip install \
    pipenv \
    black \
    flake8 \
    mypy \
    pytest 
    

# Source the new aliases
source ~/.bashrc

echo "✅ Development environment setup complete!"
echo "🚀 Available tools:"
echo "  - Rust: $(rustc --version)"
echo "  - Node.js: $(node --version)"
echo "  - Python: $(python3 --version)"
echo "  - Claude Code: $(claude --version 2>/dev/null || echo 'Installing...')"
echo "  - ruv-swarm: $(ruv-swarm --version 2>/dev/null || echo 'Installing...')"
echo ""
echo "🔧 Next steps:"
echo "  1. Set GITHUB_TOKEN environment variable"
echo "  2. Run 'claude mcp list' to verify MCP connections"
echo "  3. Run 'ruv-swarm swarm init' to initialize swarm"
echo "  4. Start coding! 🎉"