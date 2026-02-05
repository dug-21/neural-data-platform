# CI/CD vs Local Deployment Split Analysis

**Document**: 05-cicd-local-split-analysis.md
**Date**: 2026-02-05
**Status**: Research
**Scope**: Analyze which deployment functions should move to CI/CD vs remain local on Pi

---

## Executive Summary

This analysis examines the current NDP deployment system (`deploy.sh`) to categorize each function by its optimal execution environment. The goal is to identify what can be pre-computed in CI/CD pipelines before landing on the Pi versus what must remain as local deployment operations.

**Key Finding**: Approximately 60% of current deployment operations are CI/CD candidates, which could reduce Pi deployment time from ~15-30 minutes to under 5 minutes while improving consistency and enabling rollback capabilities.

---

## 1. Function Categorization Table

### 1.1 Current deploy.sh Functions Analysis

| Function | Current Phase | Category | Rationale |
|----------|---------------|----------|-----------|
| **Container Builds** | Phase 2 | **CI/CD** | Most resource-intensive; Pi ARM builds take 15-30 min |
| **Rust Tool Builds** (`handle_tool`) | Phase 2.5 | **CI/CD** | Cargo builds are deterministic and reproducible |
| **Manifest Validation** (`validate_manifest`) | Phase 1 | **CI/CD** | No runtime dependencies; pure JSON/schema validation |
| **DDL Generation** (`generate_silver_ddl`) | Phase 4 | **CI/CD** | Config-driven SQL generation; no DB access needed |
| **Gold DDL Generation** (`handle_gold_table`) | Phase 5 | **Hybrid** | Generation in CI, DB state check local |
| **Domain Config Validation** (`validate_domain_configs`) | Phase 6 | **CI/CD** | Schema validation against config files |
| **Migrations** (`handle_migration`) | Phase 3 | **Local** | Requires database connectivity |
| **Silver Table Apply** | Phase 4 | **Local** | Requires TimescaleDB access |
| **Stream Sync to etcd** (`handle_stream`) | Phase 7 | **Local** | Requires running etcd cluster |
| **Dimension Sync** (`sync_dimensions`) | Phase 8 | **Local** | Requires TimescaleDB COPY operation |
| **Dictionary Sync** (`sync_to_data_dictionary`) | Phase 9 | **Local** | Requires TimescaleDB access |
| **Domain Sync** (`sync_domains_to_data_dictionary`) | Phase 6 | **Local** | Requires TimescaleDB and etcd |
| **Container Restarts** | Phase 10 | **Local** | Must run on target device |
| **Device State Update** | Phase 11 | **Local** | Updates /var/ndp/ on device |
| **Infrastructure Health Checks** | Phase 1 | **Local** | Requires running services |

### 1.2 Detailed Breakdown by Category

#### CI/CD Candidates (Pre-Deployment)

| Operation | Current Time (Pi) | CI/CD Time | Benefit |
|-----------|-------------------|------------|---------|
| Docker image build (ARM) | 15-30 min | 5-10 min | Multi-arch build on powerful runners |
| Rust tool build (`ndp-gold-ddl`, `ndp-validate`) | 3-5 min | 1-2 min | Cross-compile with cargo-cross |
| Manifest validation | <1 sec | <1 sec | Fail fast before deployment |
| Config schema validation | <1 sec | <1 sec | Catch errors early |
| DDL SQL generation | <1 sec | <1 sec | Pre-generate and include in artifact |
| Changelog generation | <1 sec | <1 sec | Automate release notes |

#### Local Required (Runtime Operations)

| Operation | Reason for Local Execution |
|-----------|----------------------------|
| SQL migration execution | Requires database connection |
| etcd config sync | Requires running etcd |
| Data dictionary sync | Requires TimescaleDB access |
| Dimension CSV import | File transfer + DB COPY |
| Container restart | Docker daemon on device |
| Health checks | Service availability verification |
| Device state tracking | Local filesystem access |

#### Hybrid (CI Preparation + Local Execution)

| Operation | CI Phase | Local Phase |
|-----------|----------|-------------|
| Gold DDL | Generate SQL, validate syntax | Check DB state, apply if needed |
| Domain config | Validate JSON/YAML, generate views | Sync to etcd, apply DDL |
| Silver tables | Generate DDL from config | Apply to TimescaleDB |

---

## 2. CI/CD Patterns for Edge Deployment

### 2.1 GitHub Actions Workflow Design

```
                                   GitHub Actions
                    +-----------------------------------------+
                    |                                         |
    Push/Tag        |  1. Validate  2. Build   3. Package    |
   --------->       |     configs     images     artifacts   |
                    |        |          |           |        |
                    +--------|----------|-----------|--------+
                             |          |           |
                             v          v           v
                    +--------|----------|-----------|--------+
                    |     GitHub Container Registry (ghcr.io) |
                    |  +--------+ +--------+ +-------------+  |
                    |  | ARM64  | | amd64  | | Release     |  |
                    |  | Images | | Images | | Artifacts   |  |
                    |  +--------+ +--------+ +-------------+  |
                    +-----------------------------------------+
                                        |
                                        | Pull/Deploy
                                        v
                    +-----------------------------------------+
                    |            Raspberry Pi                  |
                    |  1. Pull artifacts   4. Sync configs    |
                    |  2. Load images      5. Apply migrations|
                    |  3. Verify manifest  6. Restart services|
                    +-----------------------------------------+
```

### 2.2 Recommended Workflow Structure

```yaml
# .github/workflows/release.yml
name: NDP Release Pipeline

on:
  push:
    tags:
      - 'v*'

jobs:
  # Phase 1: Validation (CI)
  validate:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Validate manifest schema
        run: |
          npx ajv validate -s schemas/manifest.schema.json \
            -d .deploy/releases/${{ github.ref_name }}.manifest.json
      - name: Validate stream configs
        run: ./tools/ndp-validate/ndp-validate.sh --all
      - name: Validate domain configs
        run: |
          for domain in config/domains/*/domain.json; do
            ./target/release/ndp-validate --domain "$domain"
          done

  # Phase 2: Build Artifacts (CI)
  build:
    needs: validate
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Set up QEMU (ARM64 emulation)
        uses: docker/setup-qemu-action@v3
      - name: Set up Docker Buildx
        uses: docker/setup-buildx-action@v3
      - name: Build ARM64 images
        run: |
          docker buildx build \
            --platform linux/arm64 \
            --tag ghcr.io/${{ github.repository }}/air-quality-app:${{ github.ref_name }} \
            --push .
      - name: Build Rust tools (cross-compile)
        run: |
          cargo install cross
          cross build --release --target aarch64-unknown-linux-gnu \
            -p ndp-gold-ddl -p ndp-validate

  # Phase 3: Generate Artifacts (CI)
  generate:
    needs: build
    runs-on: ubuntu-latest
    steps:
      - name: Generate Silver DDL
        run: |
          ./deploy/pi/ddl-generator.sh all > artifacts/silver-ddl.sql
      - name: Generate Gold DDL
        run: |
          ./target/release/ndp-gold-ddl --config-dir config \
            generate --all > artifacts/gold-ddl.sql
      - name: Package release bundle
        run: |
          tar -czf ndp-${{ github.ref_name }}-arm64.tar.gz \
            artifacts/ \
            target/aarch64-unknown-linux-gnu/release/ndp-* \
            config/ \
            .deploy/releases/${{ github.ref_name }}.manifest.json

  # Phase 4: Publish (CI)
  publish:
    needs: generate
    runs-on: ubuntu-latest
    steps:
      - name: Upload to GitHub Releases
        uses: softprops/action-gh-release@v1
        with:
          files: ndp-${{ github.ref_name }}-arm64.tar.gz
```

### 2.3 Pi Deployment Script (Minimal Footprint)

```bash
#!/bin/bash
# deploy-from-ci.sh - Minimal local deployment script
# Expects pre-built artifacts from CI

set -e

VERSION="$1"
ARTIFACT_URL="https://github.com/org/ndp/releases/download/${VERSION}/ndp-${VERSION}-arm64.tar.gz"

log() { echo "[DEPLOY] $1"; }

# 1. Download and extract artifact bundle
log "Downloading release bundle..."
curl -L "$ARTIFACT_URL" | tar -xz -C /opt/ndp/releases/${VERSION}

# 2. Load pre-built Docker images
log "Loading Docker images..."
docker load < /opt/ndp/releases/${VERSION}/images/air-quality-app.tar

# 3. Apply pre-generated DDL
log "Applying database changes..."
psql -U postgres -d ndp < /opt/ndp/releases/${VERSION}/artifacts/silver-ddl.sql
psql -U postgres -d ndp < /opt/ndp/releases/${VERSION}/artifacts/gold-ddl.sql

# 4. Sync configs to etcd
log "Syncing configurations..."
for config in /opt/ndp/releases/${VERSION}/config/base/streams/*/config.json; do
    stream_id=$(basename $(dirname "$config"))
    etcdctl put "/streams/${stream_id}/config" < "$config"
done

# 5. Restart services with new images
log "Restarting services..."
docker compose up -d --no-build

# 6. Update device state
echo "${VERSION}" > /var/ndp/deployed-version
echo "$(date -Iseconds)" > /var/ndp/deployed-at

log "Deployment complete: ${VERSION}"
```

---

## 3. Deployment Artifact Strategy

### 3.1 What Should Be Packaged (CI Output)

| Artifact | Format | Size Est. | Contents |
|----------|--------|-----------|----------|
| Docker images | OCI tar | ~200-500MB | ARM64 images for all services |
| Rust binaries | ELF | ~5-15MB | `ndp-gold-ddl`, `ndp-validate` |
| Generated DDL | SQL | ~10-50KB | Silver + Gold table definitions |
| Config bundle | JSON/YAML | ~20-100KB | Stream, domain, dimension configs |
| Manifest | JSON | ~1KB | Release metadata and checksums |

### 3.2 What Should Be Configured (Not Packaged)

| Item | Reason |
|------|--------|
| Database credentials | Security; environment-specific |
| API keys (NWS, etc.) | Security; user-specific |
| Device-specific paths | `/data/bronze`, `/var/ndp` |
| Network configuration | Pi IP, ports, MQTT broker |
| Retention policies | May vary per deployment |

### 3.3 Release Bundle Structure

```
ndp-v1.2.0-arm64.tar.gz
├── manifest.json           # Release metadata, checksums
├── images/
│   ├── air-quality-app.tar
│   ├── ndp-mcp-server.tar
│   └── silver-etl.tar
├── binaries/
│   ├── ndp-gold-ddl
│   └── ndp-validate
├── ddl/
│   ├── silver-all.sql
│   ├── gold-all.sql
│   └── migrations/
│       └── 005-new-feature.sql
├── config/
│   ├── base/streams/
│   ├── domains/
│   └── dimensions/
└── deploy.sh               # Minimal local deploy script
```

### 3.4 Versioning Strategy

```
Release Version: v1.2.0
Artifact Name: ndp-v1.2.0-arm64.tar.gz

Contents:
- manifest.json includes:
  {
    "release_version": "1.2.0",
    "build_commit": "abc123",
    "build_timestamp": "2026-02-05T10:30:00Z",
    "checksums": {
      "air-quality-app.tar": "sha256:...",
      "ndp-gold-ddl": "sha256:..."
    },
    "compatible_with": {
      "min_version": "1.0.0",
      "max_version": "2.0.0"
    }
  }
```

---

## 4. Local Deployment Footprint Minimization

### 4.1 Current vs Target Deployment Time

| Phase | Current (On-Device) | With CI/CD | Savings |
|-------|---------------------|------------|---------|
| Container build | 15-30 min | 0 (pre-built) | 15-30 min |
| Rust tool build | 3-5 min | 0 (pre-built) | 3-5 min |
| DDL generation | <1 sec | 0 (pre-generated) | - |
| Validation | <1 sec | 0 (done in CI) | - |
| Image load | - | ~30 sec | - |
| DDL apply | ~5 sec | ~5 sec | - |
| Config sync | ~10 sec | ~10 sec | - |
| Service restart | ~30 sec | ~30 sec | - |
| **Total** | **~20-35 min** | **~2-3 min** | **85-90%** |

### 4.2 Minimal Local Operations

After CI/CD handles builds and validation, the Pi needs only:

```bash
# Required local operations (irreducible)
1. docker load < images/*.tar              # Load pre-built images
2. psql -f ddl/*.sql                       # Apply database changes
3. etcdctl put /streams/...                # Sync configs
4. docker compose up -d                     # Restart services
5. echo "vX.Y.Z" > /var/ndp/deployed-version  # Track state
```

### 4.3 Dependency Reduction

**Current Dependencies on Pi**:
- Docker + Docker Compose
- Rust toolchain + Cargo
- Python 3 (for YAML parsing)
- jq, yq (config processing)
- git (for updates)

**Minimal Dependencies (Post CI/CD)**:
- Docker + Docker Compose
- jq (JSON processing only)
- curl (artifact download)
- psql client (migration apply)

**Removed from Pi**:
- Rust toolchain (~1GB)
- Build dependencies
- git (optional - webhook-triggered updates)

---

## 5. Configuration Drift Detection

### 5.1 Drift Detection Strategy

```yaml
# .github/workflows/drift-check.yml
name: Configuration Drift Check

on:
  schedule:
    - cron: '0 */6 * * *'  # Every 6 hours

jobs:
  check-drift:
    runs-on: ubuntu-latest
    steps:
      - name: Fetch current device state
        run: |
          # SSH to Pi and capture state
          ssh pi@device "cat /var/ndp/deployed-version" > device-version.txt
          ssh pi@device "etcdctl get --prefix /streams/" > device-config.txt
          ssh pi@device "psql -c 'SELECT table_name FROM information_schema.tables WHERE table_schema = '\''silver'\'''" > device-tables.txt

      - name: Compare with expected state
        run: |
          # Compare device state with repository state
          expected_version=$(cat .deploy/releases/latest.manifest.json | jq -r '.release_version')
          actual_version=$(cat device-version.txt)

          if [ "$expected_version" != "$actual_version" ]; then
            echo "::warning::Version drift detected: expected $expected_version, got $actual_version"
          fi

      - name: Report drift
        if: failure()
        uses: actions/github-script@v7
        with:
          script: |
            github.rest.issues.create({
              owner: context.repo.owner,
              repo: context.repo.repo,
              title: 'Configuration Drift Detected',
              body: 'Device configuration has drifted from expected state.'
            })
```

### 5.2 State Reconciliation

```bash
# reconcile.sh - Bring device back to expected state
#!/bin/bash

EXPECTED_VERSION=$(curl -s https://api.github.com/repos/org/ndp/releases/latest | jq -r '.tag_name')
ACTUAL_VERSION=$(cat /var/ndp/deployed-version)

if [ "$EXPECTED_VERSION" != "$ACTUAL_VERSION" ]; then
    echo "Drift detected: $ACTUAL_VERSION -> $EXPECTED_VERSION"
    ./deploy-from-ci.sh "$EXPECTED_VERSION"
fi
```

---

## 6. Blue-Green and Canary Deployment for Edge

### 6.1 Blue-Green Pattern (Recommended)

Not fully applicable to single-Pi deployments, but can be approximated:

```
Version A (Blue) ─────┐
                      ├─── Traffic (100%)
Version B (Green) ────┘

Deployment Flow:
1. Pull new version alongside existing
2. Validate new version (health checks)
3. Switch traffic atomically (docker compose up)
4. Keep old version for rollback (2 versions retained)
```

### 6.2 Practical Implementation

```bash
# Blue-green style deployment
CURRENT_VERSION=$(cat /var/ndp/deployed-version)
NEW_VERSION="$1"

# Stage new version
mkdir -p /opt/ndp/versions/${NEW_VERSION}
tar -xzf ndp-${NEW_VERSION}.tar.gz -C /opt/ndp/versions/${NEW_VERSION}

# Pre-flight validation
docker load < /opt/ndp/versions/${NEW_VERSION}/images/*.tar
docker run --rm ndp-validator validate-config

# Atomic switch
ln -sfn /opt/ndp/versions/${NEW_VERSION} /opt/ndp/current
docker compose -f /opt/ndp/current/docker-compose.yml up -d

# Verify
if ! curl -sf http://localhost:8080/health; then
    echo "Rollback triggered"
    ln -sfn /opt/ndp/versions/${CURRENT_VERSION} /opt/ndp/current
    docker compose -f /opt/ndp/current/docker-compose.yml up -d
    exit 1
fi

# Cleanup old versions (keep last 2)
ls -1d /opt/ndp/versions/v* | head -n -2 | xargs rm -rf
```

### 6.3 Canary for Multi-Device Fleets (Future)

If NDP scales to multiple Pis:

```yaml
# Canary deployment strategy
strategy:
  type: canary
  stages:
    - name: canary
      devices: ["pi-test-01"]
      duration: 24h
      metrics:
        - error_rate < 0.01
        - latency_p99 < 500ms
    - name: rollout
      devices: ["pi-prod-*"]
      batch_size: 25%
      interval: 1h
```

---

## 7. Manifest Validation in CI

### 7.1 Pre-Deployment Validation Pipeline

```yaml
# Validation stages before deployment
validate-manifest:
  steps:
    # 1. Schema validation
    - name: Validate JSON schema
      run: npx ajv validate -s schemas/manifest.schema.json -d $MANIFEST

    # 2. Semantic validation
    - name: Check stream references exist
      run: |
        for stream_id in $(jq -r '.changes[] | select(.type=="stream") | .id' $MANIFEST); do
          if [ ! -f "config/base/streams/${stream_id}/config.json" ]; then
            echo "ERROR: Stream config not found: ${stream_id}"
            exit 1
          fi
        done

    # 3. DDL generation test (dry-run)
    - name: Test DDL generation
      run: |
        ./deploy/pi/ddl-generator.sh all > /dev/null
        if [ $? -ne 0 ]; then
          echo "ERROR: DDL generation failed"
          exit 1
        fi

    # 4. Migration syntax check
    - name: Validate SQL migrations
      run: |
        for sql in $(jq -r '.changes[] | select(.type=="migration") | .file' $MANIFEST); do
          pgsql-parser "$sql" || exit 1
        done

    # 5. Version compatibility check
    - name: Check version bump
      run: |
        NEW_VERSION=$(jq -r '.release_version' $MANIFEST)
        CURRENT_VERSION=$(git describe --tags --abbrev=0)
        # Verify semantic versioning rules
        ./scripts/check-semver.sh "$CURRENT_VERSION" "$NEW_VERSION"
```

### 7.2 Generated Artifact Validation

```yaml
validate-artifacts:
  steps:
    # Verify Docker images
    - name: Scan images for vulnerabilities
      run: |
        trivy image ghcr.io/${{ github.repository }}/air-quality-app:${{ github.ref_name }}

    # Verify Rust binaries
    - name: Test Rust tools
      run: |
        ./target/release/ndp-validate --help
        ./target/release/ndp-gold-ddl --help

    # Verify SQL syntax
    - name: Lint generated SQL
      run: |
        sqlfluff lint artifacts/silver-ddl.sql
        sqlfluff lint artifacts/gold-ddl.sql

    # Verify checksums
    - name: Generate artifact checksums
      run: |
        sha256sum artifacts/* > artifacts/CHECKSUMS.txt
```

---

## 8. Transition Roadmap

### Phase 1: Foundation (Weeks 1-2)

**Goal**: Set up CI infrastructure without changing local deployment.

| Task | Effort | Dependencies |
|------|--------|--------------|
| Create GitHub Actions workflow skeleton | 2h | None |
| Set up QEMU for ARM64 builds | 1h | GHA |
| Configure ghcr.io container registry | 1h | GHA |
| Add manifest schema validation job | 2h | Schema exists |
| Test ARM64 image builds in CI | 4h | QEMU |

**Deliverables**:
- `.github/workflows/ci.yml` - Basic CI pipeline
- ARM64 test builds working
- Validation running on PRs

### Phase 2: Build Migration (Weeks 3-4)

**Goal**: Move container and Rust builds to CI.

| Task | Effort | Dependencies |
|------|--------|--------------|
| Multi-arch Docker build setup | 4h | Phase 1 |
| Cross-compile Rust tools for ARM64 | 4h | cargo-cross |
| Create release artifact packaging | 4h | Builds working |
| Set up GitHub Releases publishing | 2h | Packaging |
| Update deploy.sh to detect pre-built binaries | 2h | Artifacts |

**Deliverables**:
- Docker images built in CI, pushed to ghcr.io
- Rust binaries cross-compiled and included in release
- Release bundles published to GitHub Releases

### Phase 3: DDL Pre-Generation (Weeks 5-6)

**Goal**: Generate SQL in CI, apply locally.

| Task | Effort | Dependencies |
|------|--------|--------------|
| Add DDL generation to CI workflow | 2h | Phase 2 |
| Include generated SQL in release bundle | 1h | DDL gen |
| Modify deploy.sh to use pre-generated DDL | 4h | Bundle format |
| Add DDL validation (syntax check) | 2h | SQL tools |
| Test end-to-end with pre-gen DDL | 4h | All above |

**Deliverables**:
- Silver and Gold DDL generated in CI
- Deploy script prefers pre-generated DDL
- Syntax validation in CI

### Phase 4: Minimal Deploy Script (Weeks 7-8)

**Goal**: Create lightweight Pi deployment.

| Task | Effort | Dependencies |
|------|--------|--------------|
| Create `deploy-from-ci.sh` script | 4h | Phase 3 |
| Implement artifact download and extraction | 2h | Release bundles |
| Add image loading from tar files | 2h | Docker |
| Implement rollback capability | 4h | Multi-version |
| Remove build dependencies from Pi | 2h | New script |
| Documentation and runbook | 4h | All above |

**Deliverables**:
- `deploy-from-ci.sh` - Minimal local script
- Rollback procedure documented
- Reduced Pi dependencies

### Phase 5: Automation (Weeks 9-10)

**Goal**: Enable webhook-triggered deployments.

| Task | Effort | Dependencies |
|------|--------|--------------|
| Implement Pi webhook endpoint | 4h | Phase 4 |
| Add GitHub webhook on release publish | 2h | Webhook |
| Implement drift detection workflow | 4h | GHA scheduled |
| Add deployment notifications (optional) | 2h | Webhook |
| End-to-end testing | 4h | All above |

**Deliverables**:
- Webhook-triggered deployments (future: dp-022)
- Drift detection running on schedule
- Automated release notifications

---

## 9. Risk Assessment

### 9.1 Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| ARM64 build failures | Medium | High | Multi-platform testing, fallback to on-device |
| Network issues during artifact pull | Medium | Medium | Local artifact caching, retry logic |
| Cross-compilation issues | Low | Medium | Test thoroughly, document workarounds |
| Rollback failures | Low | High | Keep 2 versions, test rollback procedure |
| Drift detection false positives | Medium | Low | Tune thresholds, manual verification |

### 9.2 Fallback Strategy

If CI/CD deployment fails, the Pi can always fall back to the current approach:

```bash
# Fallback to local build
if [ ! -f "/opt/ndp/releases/${VERSION}/images/air-quality-app.tar" ]; then
    echo "Artifact not found, falling back to local build"
    cd /opt/ndp/repo && ./deploy.sh deploy
fi
```

---

## 10. Conclusion

### Recommended Approach

1. **Start with container builds** - Biggest time savings (15-30 min)
2. **Add Rust cross-compilation** - Removes toolchain from Pi
3. **Pre-generate DDL** - Reduces runtime complexity
4. **Create minimal deploy script** - Simplifies Pi operations
5. **Add drift detection** - Ensures consistency

### Expected Outcomes

| Metric | Current | After CI/CD | Improvement |
|--------|---------|-------------|-------------|
| Deployment time | 20-35 min | 2-3 min | 90% faster |
| Pi disk usage | +1GB (Rust) | -1GB | Reduced footprint |
| Build consistency | Variable | Reproducible | 100% consistent |
| Rollback time | Manual | <1 min | Automated |
| Validation coverage | Partial | Complete | Fail-fast |

### Next Steps

1. Review this analysis with stakeholders
2. Prioritize Phase 1 tasks
3. Create tracking issue for CI/CD migration (dp-022)
4. Begin implementing GitHub Actions workflow

---

## Appendix A: Current Function Inventory

### deploy.sh Functions by Category

```
Build Functions (CI/CD Candidates):
  - build()                    - Docker compose build
  - handle_container_build()   - Individual container build
  - handle_tool()              - Rust tool build

Validation Functions (CI/CD Candidates):
  - validate_manifest()        - JSON schema validation
  - validate_domain_configs()  - Domain config validation
  - check_prereqs()            - Dependency checks

Generation Functions (CI/CD Candidates):
  - generate_silver_ddl()      - Silver layer DDL
  - generate_create_table_ddl() - Table DDL
  - generate_indexes_ddl()      - Index DDL
  - generate_hypertable_ddl()   - TimescaleDB hypertable
  - generate_policies_ddl()     - Compression/retention
  - generate_permissions_ddl()  - Role grants

Sync Functions (Local Required):
  - sync_config()              - etcd configuration
  - sync_to_data_dictionary()  - TimescaleDB dictionary
  - sync_dimensions()          - Dimension tables
  - sync_domains_to_data_dictionary() - Domain objectives
  - handle_stream()            - Stream to etcd

Apply Functions (Local Required):
  - handle_silver_table()      - Apply Silver DDL
  - handle_gold_table()        - Apply Gold DDL
  - handle_migration()         - Apply SQL migration
  - handle_domain()            - Apply domain config

Service Functions (Local Required):
  - start()                    - Start services
  - stop()                     - Stop services
  - handle_container_restart() - Restart containers
  - wait_for_health()          - Health check polling

State Functions (Local Required):
  - apply() Phase 11           - Update /var/ndp/
  - status()                   - Service status display
```

### Manifest Declaration Types

| Type | Handler | CI/CD Phase | Local Phase |
|------|---------|-------------|-------------|
| `container` (build) | `handle_container_build` | Build image | Load image |
| `container` (restart) | `handle_container_restart` | - | Restart |
| `tool` | `handle_tool` | Cross-compile | - |
| `migration` | `handle_migration` | Validate syntax | Apply SQL |
| `silver-table` | `handle_silver_table` | Generate DDL | Apply DDL |
| `gold-tables` | `handle_gold_table` | Generate DDL | Apply DDL |
| `domain` | `handle_domain` | Validate config | Sync to etcd/DB |
| `stream` | `handle_stream` | Validate config | Sync to etcd |
| `dimensions` | `handle_dimensions` | - | COPY to DB |
| `dictionary` | `handle_dictionary` | - | Sync to DB |

---

## Appendix B: Sample GitHub Actions Workflow

```yaml
# .github/workflows/release.yml
name: NDP Release

on:
  push:
    tags:
      - 'v*'

env:
  REGISTRY: ghcr.io
  IMAGE_NAME: ${{ github.repository }}

jobs:
  validate:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Validate manifest
        run: |
          VERSION="${GITHUB_REF_NAME}"
          MANIFEST=".deploy/releases/${VERSION}.manifest.json"

          if [ ! -f "$MANIFEST" ]; then
            echo "Manifest not found: $MANIFEST"
            exit 1
          fi

          # Schema validation
          npx ajv validate -s schemas/manifest.schema.json -d "$MANIFEST"

          # Version match
          MANIFEST_VERSION=$(jq -r '.release_version' "$MANIFEST")
          if [ "v${MANIFEST_VERSION}" != "${VERSION}" ]; then
            echo "Version mismatch: manifest=${MANIFEST_VERSION}, tag=${VERSION}"
            exit 1
          fi

  build-images:
    needs: validate
    runs-on: ubuntu-latest
    permissions:
      contents: read
      packages: write
    steps:
      - uses: actions/checkout@v4

      - name: Set up QEMU
        uses: docker/setup-qemu-action@v3

      - name: Set up Docker Buildx
        uses: docker/setup-buildx-action@v3

      - name: Log in to registry
        uses: docker/login-action@v3
        with:
          registry: ${{ env.REGISTRY }}
          username: ${{ github.actor }}
          password: ${{ secrets.GITHUB_TOKEN }}

      - name: Build and push air-quality-app
        uses: docker/build-push-action@v5
        with:
          context: .
          file: apps/air-quality-app/Dockerfile
          platforms: linux/arm64
          push: true
          tags: ${{ env.REGISTRY }}/${{ env.IMAGE_NAME }}/air-quality-app:${{ github.ref_name }}
          cache-from: type=gha
          cache-to: type=gha,mode=max

  build-tools:
    needs: validate
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust
        uses: dtolnay/rust-action@stable

      - name: Install cross
        run: cargo install cross --git https://github.com/cross-rs/cross

      - name: Build for ARM64
        run: |
          cross build --release --target aarch64-unknown-linux-gnu \
            -p ndp-gold-ddl -p ndp-validate

      - name: Upload binaries
        uses: actions/upload-artifact@v4
        with:
          name: rust-binaries-arm64
          path: target/aarch64-unknown-linux-gnu/release/ndp-*

  generate-ddl:
    needs: validate
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Generate Silver DDL
        run: ./deploy/pi/ddl-generator.sh all > silver-ddl.sql

      - name: Upload DDL
        uses: actions/upload-artifact@v4
        with:
          name: generated-ddl
          path: "*.sql"

  package:
    needs: [build-images, build-tools, generate-ddl]
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Download artifacts
        uses: actions/download-artifact@v4

      - name: Package release bundle
        run: |
          VERSION="${GITHUB_REF_NAME}"
          mkdir -p release/{images,binaries,ddl,config}

          # Copy artifacts
          cp rust-binaries-arm64/ndp-* release/binaries/
          cp generated-ddl/*.sql release/ddl/
          cp -r config/base release/config/
          cp .deploy/releases/${VERSION}.manifest.json release/manifest.json

          # Generate checksums
          cd release && sha256sum binaries/* ddl/* > CHECKSUMS.txt

          # Create tarball
          cd .. && tar -czf ndp-${VERSION}-arm64.tar.gz release/

      - name: Create GitHub Release
        uses: softprops/action-gh-release@v1
        with:
          files: ndp-${{ github.ref_name }}-arm64.tar.gz
          generate_release_notes: true
```

---

*Document generated for NDP CI/CD planning. Implementation tracked in future feature dp-022.*
