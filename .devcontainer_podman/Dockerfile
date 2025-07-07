# Podman-optimized development container for Neural Trader
FROM mcr.microsoft.com/devcontainers/rust:1-bookworm

# Install Podman and development tools
RUN apt-get update && export DEBIAN_FRONTEND=noninteractive \
    && apt-get -y install --no-install-recommends \
        podman \
        podman-compose \
        buildah \
        skopeo \
        fuse-overlayfs \
        slirp4netns \
        postgresql-client \
        redis-tools \
        python3-pip \
        python3-venv \
        jq \
        httpie \
        tmux \
        htop \
        iotop \
        net-tools \
        dnsutils \
        tcpdump \
        strace \
        ltrace \
    && apt-get clean \
    && rm -rf /var/lib/apt/lists/*

# Configure Podman for rootless operation
RUN echo 'vscode:100000:65536' >> /etc/subuid \
    && echo 'vscode:100000:65536' >> /etc/subgid

# Create Podman configuration directory
RUN mkdir -p /home/vscode/.config/containers \
    && chown -R vscode:vscode /home/vscode/.config

# Copy Podman configuration files
COPY --chown=vscode:vscode containers.conf /home/vscode/.config/containers/containers.conf
COPY --chown=vscode:vscode storage.conf /home/vscode/.config/containers/storage.conf
COPY --chown=vscode:vscode registries.conf /etc/containers/registries.conf

# Enable lingering for the vscode user (for systemd services)
RUN mkdir -p /var/lib/systemd/linger \
    && touch /var/lib/systemd/linger/vscode

# Install Python dependencies for data ingestion development
COPY data_ingestion/requirements.txt /tmp/requirements.txt
RUN pip3 install --no-cache-dir -r /tmp/requirements.txt \
    && rm /tmp/requirements.txt

# Install Rust development tools
USER vscode
RUN rustup component add rustfmt clippy rust-src rust-analyzer \
    && cargo install cargo-watch cargo-edit cargo-expand cargo-outdated \
    && cargo install sqlx-cli --no-default-features --features postgres,rustls

# Create workspace directory
RUN mkdir -p /workspace

# Switch back to root for final setup
USER root

# Set up Podman socket directory
RUN mkdir -p /run/user/1000 \
    && chown -R vscode:vscode /run/user/1000

# Final user switch
USER vscode
WORKDIR /workspace

# Set Podman as the default container runtime
ENV DOCKER_HOST=unix:///run/user/1000/podman/podman.sock
ENV CONTAINER_HOST=unix:///run/user/1000/podman/podman.sock