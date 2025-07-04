#!/bin/bash
set -e

echo "🔌 Configuring MCP servers..."

# Check if Claude CLI is available
if ! command -v claude &> /dev/null; then
    echo "❌ Claude CLI not found. Please ensure it's installed first."
    exit 1
fi

# Function to check if an MCP server exists
mcp_exists() {
    local server_name=$1
    claude mcp list 2>/dev/null | grep -q "^${server_name}:" && return 0 || return 1
}

# Function to add MCP server if it doesn't exist
add_mcp_if_not_exists() {
    local server_name=$1
    shift  # Remove server_name from arguments
    
    if mcp_exists "$server_name"; then
        echo "  ✓ $server_name already configured"
    else
        echo "  + Adding $server_name MCP server..."
        claude mcp add --scope local "$server_name" "$@"
    fi
}

# Configure ruv-swarm MCP server
add_mcp_if_not_exists "ruv-swarm" npx ruv-swarm mcp start

# Configure GitHub MCP server
if [ -n "$AGENT_TOKEN" ]; then
    add_mcp_if_not_exists "github" npx @modelcontextprotocol/server-github -e GITHUB_PERSONAL_ACCESS_TOKEN="$AGENT_TOKEN"
else
    echo "  ⚠️  AGENT_TOKEN not set, configuring GitHub MCP without token"
    add_mcp_if_not_exists "github" npx @modelcontextprotocol/server-github
fi

# Configure filesystem MCP server
add_mcp_if_not_exists "filesystem" npx @modelcontextprotocol/server-filesystem /workspaces/neural-trader

# Add any additional MCP servers here in the future
# Example:
# add_mcp_if_not_exists "my-new-server" npx @my-org/my-server --some-args

echo "📋 Current MCP servers:"
claude mcp list