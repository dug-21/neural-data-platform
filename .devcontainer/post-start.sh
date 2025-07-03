#!/bin/bash
set -e

echo "🔄 Running post-start configuration..."

# Ensure proper permissions
chmod +x .devcontainer/setup.sh
chmod +x .devcontainer/post-start.sh

# Update Rust toolchain
rustup update

# Install/update project dependencies
if [ -f "Cargo.toml" ]; then
    echo "📦 Installing Rust dependencies..."
    cargo fetch
fi

if [ -f "package.json" ]; then
    echo "📦 Installing Node.js dependencies..."
    npm install
fi

if [ -f "requirements.txt" ]; then
    echo "📦 Installing Python dependencies..."
    pip install -r requirements.txt
fi

if [ -f "pyproject.toml" ]; then
    echo "📦 Installing Python dependencies with Poetry..."
    poetry install
fi

# Initialize ruv-swarm if not already done
if [ ! -d ".ruv-swarm" ]; then
    echo "🐝 Initializing ruv-swarm..."
    ruv-swarm init --auto
fi

# Verify MCP connections
echo "🔌 Verifying MCP connections..."
claude mcp list || echo "⚠️  Claude Code MCP setup needed"

# Start background services if needed
echo "🚀 Starting development services..."

# Create a simple development dashboard
cat > dev-dashboard.html << 'EOF'
<!DOCTYPE html>
<html>
<head>
    <title>Development Dashboard</title>
    <style>
        body { font-family: Arial, sans-serif; margin: 20px; }
        .service { margin: 10px 0; padding: 10px; border: 1px solid #ddd; border-radius: 5px; }
        .service.running { background-color: #d4edda; }
        .service.stopped { background-color: #f8d7da; }
    </style>
</head>
<body>
    <h1>🚀 Development Environment Dashboard</h1>
    
    <h2>🛠️ Available Services</h2>
    <div class="service running">
        <h3>🦀 Rust Development</h3>
        <p>Cargo workspace ready. Run <code>cargo run</code> to start.</p>
    </div>
    
    <div class="service running">
        <h3>📦 Node.js Development</h3>
        <p>Node.js and npm ready. Create package.json to start.</p>
    </div>
    
    <div class="service running">
        <h3>🐍 Python Development</h3>
        <p>Python 3.11 ready. Virtual environment recommended.</p>
    </div>
    
    <div class="service running">
        <h3>🤖 Claude Code</h3>
        <p>AI-powered development assistant with MCP integration.</p>
    </div>
    
    <div class="service running">
        <h3>🐝 ruv-swarm</h3>
        <p>Distributed AI swarm coordination platform.</p>
    </div>
    
    <h2>🔗 Quick Links</h2>
    <ul>
        <li><a href="http://localhost:3000">Frontend (Port 3000)</a></li>
        <li><a href="http://localhost:8000">API Server (Port 8000)</a></li>
        <li><a href="http://localhost:5000">Python Server (Port 5000)</a></li>
        <li><a href="http://localhost:9000">Rust Server (Port 9000)</a></li>
    </ul>
    
    <h2>📋 Common Commands</h2>
    <pre>
# Rust
cargo run          # Run the main application
cargo test         # Run tests
cargo build        # Build the project
cargo clippy       # Lint the code

# Node.js
npm start          # Start the application
npm test           # Run tests
npm run build      # Build the project

# Python
python3 app.py     # Run Python application
pytest             # Run tests
pip install -r requirements.txt

# Claude & ruv-swarm
claude --help      # Claude Code help
ruv-swarm --help   # ruv-swarm help
ruv-swarm swarm init  # Initialize swarm
    </pre>
</body>
</html>
EOF

echo "✅ Post-start configuration complete!"
echo "🌐 Development dashboard available at: file://$(pwd)/dev-dashboard.html"
echo "🚀 Ready for development!"