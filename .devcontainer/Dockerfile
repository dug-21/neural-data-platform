# Offline-friendly development container for Neural Trader
FROM mcr.microsoft.com/devcontainers/rust:1-bullseye

# Install system dependencies and Python
RUN apt-get update && export DEBIAN_FRONTEND=noninteractive \
    && apt-get -y install --no-install-recommends \
        python3 \
        python3-pip \
        python3-venv \
        git \
        curl \
        wget \
        jq \
        ripgrep \
        htop \
        build-essential \
        pkg-config \
        libssl-dev \
    && apt-get clean \
    && rm -rf /var/lib/apt/lists/*

# Install Node.js 20 (LTS) via NodeSource
RUN curl -fsSL https://deb.nodesource.com/setup_20.x | bash - \
    && apt-get install -y nodejs \
    && apt-get clean \
    && rm -rf /var/lib/apt/lists/*

# Set npm directory for global packages (user-writable)
ENV NPM_CONFIG_PREFIX=/home/vscode/.npm-global
ENV PATH=$NPM_CONFIG_PREFIX/bin:$PATH

# Create npm global directory with proper permissions
RUN mkdir -p /home/vscode/.npm-global \
    && chown -R vscode:vscode /home/vscode/.npm-global

# Create workspace directory with proper permissions
RUN mkdir -p /workspace && chown vscode:vscode /workspace

# Switch to vscode user
USER vscode
WORKDIR /workspace