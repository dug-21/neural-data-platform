# Neural Trader Development Container Setup

This setup provides a development environment for Neural Trader with support for Rust, Python, and TypeScript/JavaScript development.

## Quick Start (Minimal Setup)

The default configuration uses a **minimal setup** that installs only essential tools to get you coding faster:

- ✅ Rust toolchain with clippy, rustfmt, rust-analyzer
- ✅ cargo-watch (for auto-recompilation)  
- ✅ Claude Code CLI and ruv-swarm
- ✅ Essential system tools (git, ripgrep, etc.)
- ⏱️ **Starts in ~2-3 minutes**

## Full Setup (Optional)

If you need ALL the development tools, you can switch to the full setup:

1. Edit `.devcontainer/devcontainer.json`
2. Change `setup-minimal.sh` to `setup.sh`
3. Change `post-start-minimal.sh` to `post-start.sh`
4. Rebuild the container

The full setup includes:
- All minimal setup tools PLUS:
- 10+ additional cargo tools (cargo-edit, cargo-tree, cargo-nextest, etc.)
- Extended shell configurations (zsh, oh-my-zsh)
- Additional Python tools (black, flake8, mypy, pytest)
- Additional Node.js tools (TypeScript, ESLint, Prettier, PM2)
- ⏱️ **Takes 10-15 minutes to compile all tools**

## Running Simulations

Simulation containers are run from the host using Podman:

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    Host Machine (macOS)                  │
│                                                         │
│  ┌─────────────────────────────────────────────────┐  │
│  │            Podman Desktop / Machine              │  │
│  │                                                  │  │
│  │  ┌───────────────────────────────────────────┐  │  │
│  │  │       Dev Container (VS Code IDE)         │  │  │
│  │  │                                           │  │  │
│  │  │  - Rust toolchain & analyzer             │  │  │
│  │  │  - Python 3.11                           │  │  │
│  │  │  - Node.js (via features)               │  │  │
│  │  │  - Podman CLI (socket mounted)          │  │  │
│  │  │                                          │  │  │
│  │  │  /run/podman/podman.sock ───────────────┼──┼──┤ Socket Mount
│  │  └───────────────────────────────────────────┘  │  │
│  │                                                  │  │
│  │  ┌───────────────────────────────────────────┐  │  │
│  │  │    Simulation Containers (spawned from    │  │  │
│  │  │    dev container using podman socket)     │  │  │
│  │  │                                           │  │  │
│  │  │  ┌─────────────┐  ┌─────────────┐       │  │  │
│  │  │  │ PostgreSQL  │  │    Redis    │       │  │  │
│  │  │  └─────────────┘  └─────────────┘       │  │  │
│  │  │                                           │  │  │
│  │  │  ┌─────────────────────────────────┐    │  │  │
│  │  │  │   Neural Trader Application     │    │  │  │
│  │  │  │   (Simulation/Test Mode)        │    │  │  │
│  │  │  └─────────────────────────────────┘    │  │  │
│  │  └───────────────────────────────────────────────┘  │  │
│  └─────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────┘
```

## Key Features

1. **Development Container**: A lightweight container with all development tools
2. **Podman Socket Mount**: Allows spawning containers from within the dev container
3. **Simulation Environment**: Separate containers for testing without affecting dev environment

## Usage

### 1. Start Development Container

Open VS Code and use the Remote-Containers extension to open in container.

### 2. Run Simulation

From within the dev container terminal:

```bash
# Make sure you're in the workspace directory
cd /workspace

# Run the simulation
.devcontainer/run-simulation.sh
```

### 3. Develop and Test

- Edit code in VS Code
- Build Rust application: `cargo build`
- Run tests: `cargo test`
- Python development: `python3 data_ingestion/main.py`

## Benefits

- **Isolation**: Development tools don't interfere with simulation environment
- **Flexibility**: Can spawn multiple simulation environments
- **Consistency**: Same environment across different machines
- **Resource Efficiency**: Only install heavy development tools once

## Troubleshooting

### Podman Socket Issues
If you get "Podman not found" errors:
1. Ensure Podman Desktop is running on host
2. Check socket mount in devcontainer.json
3. Verify socket exists: `ls ~/.local/share/containers/podman/machine/`

### Memory Issues During Build
The Dockerfile installs minimal tools during build. Heavy tools like cargo-watch are installed after container creation via postCreateCommand.

### Container Communication
Simulation containers use `--network host` for simplicity. In production, use proper networks.