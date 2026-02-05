# Edge Deployment Patterns for Raspberry Pi

**Research Date:** 2026-02-05
**Context:** Neural Data Platform on Raspberry Pi 5 (ARM64, Ubuntu)
**Current State:** Docker Compose orchestration, 2,868-line deploy.sh

---

## Executive Summary

This research analyzes deployment patterns optimized for Raspberry Pi edge devices, evaluating full-featured IoT platforms against lightweight alternatives. The recommendation is to **stay with Docker Compose** and adopt a **modular Bash + justfile hybrid** approach rather than introducing heavy IoT fleet management platforms.

**Key Finding:** For a single-device deployment without fleet management needs, Balena/Mender are overkill. The current Docker Compose approach is appropriate; the main improvement opportunity is modularizing the 2,868-line deploy.sh.

---

## Pattern Comparison Matrix

| Pattern | Binary Size | Memory Overhead | ARM64 Support | Complexity | Fleet Mgmt | Best For |
|---------|-------------|-----------------|---------------|------------|------------|----------|
| **Docker Compose** | N/A (uses Docker) | ~50MB daemon | Native | Low | No | Single device, current stack |
| **k3s** | <100MB | ~512MB | Native | Medium | Yes | Multi-node Pi clusters |
| **Nomad** | ~75MB | ~100MB | Native | Medium | Yes | Mixed workloads (non-container) |
| **Balena** | N/A (OS-level) | ~200MB | Native | High | Yes | Large IoT fleets |
| **Mender** | ~7MB client | ~50MB | Native | Medium | Yes | OTA-focused fleets |
| **SWUpdate** | ~1.3MB | <10MB | Native | Low | Partial | A/B rootfs updates |
| **Podman** | ~50MB | 65% less than Docker | Native | Low | No | Rootless containers |
| **just** | ~5MB | <5MB | Native | Low | No | Task runner replacement |
| **Make** | Built-in | Negligible | Native | Low | No | Build automation |

---

## Detailed Analysis

### 1. Full IoT Platforms (Balena, Mender, Torizon)

#### Balena

**Architecture:** Complete OS replacement (BalenaOS) with container-focused fleet management.

**Pros:**
- True container deltas for bandwidth-efficient updates
- Minimal storage wear-and-tear
- Full device management lifecycle (build, deploy, provision, control, decommission)
- OpenBalena available for self-hosting

**Cons:**
- Requires adopting BalenaOS (vendor lock-in)
- Overkill for single-device deployments
- ~200MB memory overhead for agent
- Imposes development constraints

**Verdict:** Overkill for NDP's single-Pi deployment. Better suited for commercial IoT products with 10+ devices.

#### Mender

**Architecture:** Client-server OTA update system with A/B partitioning.

**Pros:**
- Open source (self-hostable)
- Robust A/B partitioning and atomic rollback
- Delta updates supported
- Works with existing OS (doesn't require custom image)

**Cons:**
- Focused only on OTA updates, not full orchestration
- ~7MB client binary
- Server infrastructure required for fleet management
- Less complete than Balena for device lifecycle

**Verdict:** Good if NDP needed robust OTA updates across a fleet, but not necessary for current single-device scenario.

#### SWUpdate

**Architecture:** Lightweight embedded Linux update agent.

**Pros:**
- Very lightweight (~1.3MB binary vs 7MB for Mender)
- Supports delta updates
- Well-integrated with Yocto/Buildroot
- A/B rootfs updates

**Cons:**
- Primarily for firmware/OS updates, not application deployment
- Requires more setup than Mender
- Less turnkey solution

**Verdict:** Best option if NDP eventually needs reliable OS-level updates, but current Docker-based approach handles application updates already.

### 2. Container Orchestration (k3s, Nomad)

#### k3s

**Architecture:** Lightweight Kubernetes distribution in single binary (<100MB).

**Pros:**
- Full Kubernetes API compatibility
- Persistent volumes, ingress, service mesh
- Active community, extensive ecosystem
- Ideal for Pi clusters

**Cons:**
- ~512MB memory overhead
- Complexity overkill for single node
- Learning curve for Kubernetes concepts
- More moving parts than Docker Compose

**Verdict:** Would be valuable if NDP expanded to multi-node cluster. For single Pi, Docker Compose is simpler and sufficient.

#### HashiCorp Nomad

**Architecture:** Single-binary orchestrator supporting containers, VMs, and native apps.

**Pros:**
- Superior CPU/memory efficiency vs k3s
- Can run non-container workloads
- Simpler than Kubernetes
- ~75MB single binary

**Cons:**
- Smaller ecosystem than Kubernetes
- Still overkill for single device
- Requires Consul for service discovery (additional complexity)

**Verdict:** Better than k3s for mixed workloads, but still unnecessary for NDP's current scope.

### 3. Container Runtime Alternatives

#### Podman vs Docker

| Metric | Docker | Podman |
|--------|--------|--------|
| Architecture | Daemon-based | Daemonless |
| Memory per container | Baseline | 15-20% less |
| Idle consumption | Continuous (daemon) | Zero |
| Startup time | ~1.2s | ~0.8s (30% faster) |
| Rootless support | Limited | Native |
| Compose compatibility | Native | podman-compose |

**Verdict:** Podman offers 65% lower memory footprint and better security (rootless). Migration path exists via podman-compose. Consider for future optimization if memory becomes critical.

### 4. Task Runners (just, Make, Taskfile)

#### Current State: 2,868-line deploy.sh

Google's Bash style guide recommends scripts under 50 lines. The current deploy.sh is 57x that size, indicating need for modularization.

#### just (justfile)

**Pros:**
- Written in Rust, single ~5MB binary
- Cross-platform (Linux, macOS, Windows)
- Simpler syntax than Make (no tabs required)
- Built-in `--list` command for discoverability
- No file dependency tracking (pure command runner)
- Variables and parameter support

**Cons:**
- External dependency (must install)
- Less universal than Make
- No ARM64 pre-built binary in some package managers (but `cargo install` works)

**Example justfile:**
```just
# List available commands
default:
    @just --list

# Deploy full stack
deploy: build start sync
    @echo "Deployment complete"

# Build containers
build target="all":
    ./scripts/build.sh {{target}}

# Start services
start:
    docker compose up -d

# Sync configuration to etcd
sync:
    ./scripts/sync-config.sh
```

#### Make (Makefile)

**Pros:**
- Pre-installed on all Unix systems
- No additional dependencies
- Well-understood by developers
- File dependency tracking (if needed)

**Cons:**
- Tab-sensitive syntax (common source of errors)
- Designed for file-based builds, awkward for pure commands
- `.PHONY` required for non-file targets
- Variable assignment has multiple confusing forms

**Example Makefile:**
```makefile
.PHONY: deploy build start sync

deploy: build start sync
	@echo "Deployment complete"

build:
	./scripts/build.sh $(TARGET)

start:
	docker compose up -d

sync:
	./scripts/sync-config.sh
```

#### Recommendation: Hybrid Approach

**Use justfile as orchestrator + modular Bash scripts.**

1. **justfile** - Top-level command runner (~50 commands max)
2. **Modular scripts** - Separate files in `deploy/pi/lib/`:
   - `lib/ddl.sh` - DDL generation functions
   - `lib/etcd.sh` - etcd sync functions
   - `lib/silver.sh` - Silver ETL functions
   - `lib/containers.sh` - Container build/restart
   - `lib/validation.sh` - Manifest validation
   - `lib/yaml-helpers.sh` - YAML parsing utilities

---

## Bash Modularization Best Practices

### Current Problems with 2,868-line deploy.sh

1. **Maintainability** - Hard to find specific functionality
2. **Testing** - Can't unit test individual functions
3. **Reusability** - Functions tightly coupled
4. **Readability** - Cognitive load is high

### Recommended Structure

```
deploy/pi/
├── deploy.sh              # Thin orchestrator (~100 lines)
├── justfile               # Alternative task runner (optional)
├── lib/
│   ├── common.sh          # Colors, logging, error handling
│   ├── validation.sh      # Manifest validation
│   ├── ddl-generator.sh   # DDL generation (extracted)
│   ├── etcd-sync.sh       # etcd operations
│   ├── silver-etl.sh      # Silver layer operations
│   ├── containers.sh      # Docker operations
│   ├── dictionary.sh      # Data dictionary sync
│   └── dimensions.sh      # Dimension sync
├── scripts/
│   ├── verify-*.sh        # Verification scripts
│   └── check-*.sh         # Health check scripts
└── configs/
    └── streams/           # Stream management scripts
```

### Modularization Guidelines

1. **Function size limit:** Keep functions under 20 lines
2. **Single responsibility:** Each script handles one domain
3. **Local variables:** Use `local` for all non-exported vars
4. **Error handling:** Use `set -euo pipefail` consistently
5. **Documentation:** Header comments in each module
6. **Testing:** Write shellspec or bats tests for critical functions

### Module Loading Pattern

```bash
#!/bin/bash
# deploy.sh - Thin orchestrator

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

# Load modules
source "$SCRIPT_DIR/lib/common.sh"
source "$SCRIPT_DIR/lib/validation.sh"
source "$SCRIPT_DIR/lib/containers.sh"
source "$SCRIPT_DIR/lib/etcd-sync.sh"
source "$SCRIPT_DIR/lib/silver-etl.sh"

# Main dispatch
case "${1:-}" in
    deploy)  cmd_deploy ;;
    start)   cmd_start ;;
    stop)    cmd_stop ;;
    apply)   cmd_apply "$2" ;;
    *)       cmd_help ;;
esac
```

---

## Pi-Specific Considerations

### Hardware Constraints (Raspberry Pi 5)

| Resource | Pi 5 (8GB) | Recommendation |
|----------|------------|----------------|
| RAM | 8GB | Keep total service usage <6GB |
| CPU | 4x Cortex-A76 | Avoid CPU-bound builds on device |
| Storage | SD card / NVMe | Minimize writes (SD wear) |
| Network | 1Gbps Ethernet | Local deployments preferred |

### Current NDP Resource Usage (Estimated)

| Service | Memory | Notes |
|---------|--------|-------|
| TimescaleDB | ~1-2GB | Primary memory consumer |
| Grafana | ~200-500MB | Depends on dashboards |
| Air Quality App | ~50-100MB | Rust binary, efficient |
| etcd | ~50-100MB | Configuration store |
| Silver ETL | ~50-100MB | Rust binary |
| Docker daemon | ~100-200MB | Overhead |
| **Total** | **~2-3GB** | Comfortable headroom |

### Build vs Deploy Separation

**Recommendation:** Move builds to CI/CD, keep deploys local.

| Operation | Where | Why |
|-----------|-------|-----|
| Rust compilation | CI/CD (GitHub Actions) | CPU-intensive, slow on Pi |
| Docker builds | CI/CD or Pi | Registry pull faster than build |
| Config deployment | Local (Pi) | Fast, needs local access |
| Database migrations | Local (Pi) | Requires DB connection |
| Service restart | Local (Pi) | Fast, idempotent |

---

## Resource Footprint Analysis

### Lightweight Tool Comparison

| Tool | Binary Size | Runtime Memory | Dependencies | ARM64 Install |
|------|-------------|----------------|--------------|---------------|
| just | ~5MB | <5MB | None | `cargo install just` |
| Make | Built-in | Negligible | None | Pre-installed |
| Taskfile (task) | ~10MB | <10MB | None | `go install` |
| Ansible | ~150MB+ | ~100MB+ | Python | `apt install` |
| Bash | Built-in | Negligible | None | Pre-installed |

### Orchestrator Comparison

| Tool | Binary | Runtime Memory | Min Resources |
|------|--------|----------------|---------------|
| Docker Compose | N/A | ~50MB (daemon) | 1GB RAM |
| k3s | 100MB | ~512MB | 2GB RAM |
| Nomad | 75MB | ~100MB | 1GB RAM |
| Podman | 50MB | 65% less than Docker | 512MB RAM |

---

## Recommendations

### Immediate (Low Effort, High Impact)

1. **Modularize deploy.sh**
   - Extract functions into `lib/*.sh` modules
   - Keep main script as thin dispatcher
   - Target: No file over 300 lines

2. **Add justfile (optional)**
   - Install: `cargo install just` (one-time)
   - Provides better CLI UX than raw Bash
   - Coexists with deploy.sh

### Short-term (Medium Effort)

3. **CI/CD for builds**
   - Move Rust compilation to GitHub Actions
   - Build ARM64 binaries in CI
   - Pull pre-built images on Pi

4. **Consider Podman migration**
   - Test podman-compose compatibility
   - Evaluate memory savings
   - Rootless containers improve security

### Long-term (High Effort, Conditional)

5. **SWUpdate for OS updates** (if needed)
   - Only if robust A/B rootfs updates required
   - Not necessary for application-level deployments

6. **k3s cluster** (if scaling)
   - Only if NDP expands to multiple Pis
   - Current single-device setup doesn't need it

---

## Decision Matrix

| Requirement | Current (Docker Compose) | With Modular Bash | With k3s | With Balena |
|-------------|-------------------------|-------------------|----------|-------------|
| Single device | Good | Good | Overkill | Overkill |
| Multi-device fleet | Poor | Poor | Excellent | Excellent |
| Maintainability | Poor (2.8k lines) | Excellent | Good | Good |
| Memory efficiency | Good | Good | Poor (+512MB) | Poor (+200MB) |
| Learning curve | Low | Low | High | High |
| OTA updates | Manual | Manual | Built-in | Built-in |
| Vendor lock-in | None | None | None | High |

---

## Conclusion

**Recommended Path:** Stay with Docker Compose + modular Bash scripts.

The current deployment approach is fundamentally sound. The 2,868-line deploy.sh is the main technical debt, not the orchestration choice. Balena, Mender, and k3s solve problems NDP doesn't currently have (fleet management, multi-node orchestration).

**Action Items:**
1. Modularize deploy.sh into `lib/*.sh` components
2. Optionally add justfile as friendlier CLI layer
3. Move Rust builds to CI/CD
4. Defer Podman/k3s evaluation until requirements change

---

## Sources

### Edge Deployment & IoT Platforms
- [OpenNebula on Raspberry Pi](https://opennebula.io/blog/innovation/opennebula-on-a-raspberry-pi/)
- [Pico-Cloud Research Paper](https://arxiv.org/pdf/2511.13253)
- [Performance Characterization of Containers in Edge Computing](https://arxiv.org/html/2505.02082v2)
- [IoT Fleet Management: Torizon, Balena, Mender](https://www.ics.com/blog/iot-fleet-management-system-torizon-balena-mender)
- [Balena vs Mender Comparison](https://www.saashub.com/compare-balena-io-vs-mender-io)

### Container Runtime
- [Podman vs Docker 2025 Comparison](https://uptrace.dev/comparisons/podman-vs-docker)
- [Docker vs Podman for Modern Developers](https://www.linuxjournal.com/content/containers-2025-docker-vs-podman-modern-developers)
- [Podman vs Docker Key Differences](https://cyberpanel.net/blog/podman-vs-docker)

### Orchestration
- [k3s vs Other Lightweight Solutions](https://medium.com/@veritasautomata/k3s-vs-other-lightweight-container-orchestration-solutions-a-comparative-analysis-for-distributed-b80810309d91)
- [k3s on Raspberry Pi](https://calje.medium.com/running-a-kubernetes-cluster-on-raspberry-pi-with-k3s-cheap-low-power-fully-functional-9e2cc50ba64f)
- [Nomad vs Kubernetes](https://developer.hashicorp.com/nomad/docs/what-is-nomad)
- [Why Nomad for Edge Computing](https://www.linkedin.com/pulse/why-can-hashicorp-nomad-more-relevant-than-kubernetes-cedric-derue)

### Task Runners
- [justfile as Task Runner](https://tduyng.com/blog/justfile-my-favorite-task-runner/)
- [Just vs Make Comparison](https://spin.atomicobject.com/just-task-runner/)
- [Just Make a Task (Make vs Taskfile vs Just)](https://appliedgo.net/spotlight/just-make-a-task/)
- [Why Justfile Outshines Makefile](https://suyog942.medium.com/why-justfile-outshines-makefile-in-modern-devops-workflows-a64d99b2e9f0)
- [just GitHub Repository](https://github.com/casey/just)

### Bash Best Practices
- [Bash Scripting Best Practices 2025](https://medium.com/@prasanna.a1.usage/best-practices-we-need-to-follow-in-bash-scripting-in-2025-cebcdf254768)
- [Modularizing Bash Script Code](https://medium.com/mkdir-awesome/the-ultimate-guide-to-modularizing-bash-script-code-f4a4d53000c2)
- [Shell Script Modularity](https://moldstud.com/articles/p-maximizing-shell-script-modularity-for-reusability-and-maintainability)

### OTA Updates
- [SWUpdate Documentation 2025](https://sbabic.github.io/swupdate/swupdate.html)
- [OTA Updates Comparison](https://www.embedded.com/ota-updates-for-embedded-linux-part-2-a-comparison-of-off-the-shelf-update-systems/)
- [Delta OTA with SWUpdate](https://www.thegoodpenguin.co.uk/blog/delta-ota-update-with-swupdate/)

### Configuration Management
- [Shell Scripts vs Ansible](https://medium.com/@devopskeerti/when-to-use-what-shell-scripts-vs-ansible-for-configuration-management-98e8d4fb6d20)
- [Ansible for Edge Computing](https://www.redhat.com/en/technologies/management/ansible/edge)
- [Managing Red Hat Device Edge](https://www.redhat.com/en/blog/managing-red-hat-device-edge-tools-and-strategies)

### Single Binary Tools
- [Building CLIs in 2025: Node.js vs Go vs Rust](https://medium.com/@no-non-sense-guy/building-great-clis-in-2025-node-js-vs-go-vs-rust-e8e4bf7ee10e)
- [Rust vs Go 2025](https://blog.jetbrains.com/rust/2025/06/12/rust-vs-go/)
- [Porting Software to ARM64](https://blog.cloudflare.com/porting-our-software-to-arm64/)
