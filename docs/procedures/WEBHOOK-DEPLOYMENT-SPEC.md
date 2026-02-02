# Webhook-Triggered Deployment Specification

This document specifies the automated deployment system for the Neural Data Platform. When a release tag is pushed to GitHub, a webhook triggers automatic deployment on the target Raspberry Pi device.

**Status**: Specification (implementation planned for dp-023)

## Table of Contents

- [Overview](#overview)
- [Architecture](#architecture)
- [GitHub Webhook Configuration](#github-webhook-configuration)
  - [Webhook Events](#webhook-events)
  - [Payload Structure](#payload-structure)
  - [Webhook Secret](#webhook-secret)
- [Pi Webhook Receiver](#pi-webhook-receiver)
  - [Requirements](#requirements)
  - [Endpoint Specification](#endpoint-specification)
  - [Authentication](#authentication)
  - [Request Validation](#request-validation)
- [Deployment Flow](#deployment-flow)
  - [Sequence Diagram](#sequence-diagram)
  - [Phase Details](#phase-details)
- [Security Considerations](#security-considerations)
  - [Transport Security](#transport-security)
  - [Payload Verification](#payload-verification)
  - [Access Control](#access-control)
  - [Rate Limiting](#rate-limiting)
- [Error Handling](#error-handling)
  - [Error Categories](#error-categories)
  - [Retry Strategy](#retry-strategy)
  - [Rollback on Failure](#rollback-on-failure)
- [Status Reporting](#status-reporting)
  - [GitHub Deployment API](#github-deployment-api)
  - [Local Status Files](#local-status-files)
  - [Notification Options](#notification-options)
- [Configuration](#configuration)
  - [Pi Receiver Config](#pi-receiver-config)
  - [GitHub Repository Settings](#github-repository-settings)
- [Monitoring](#monitoring)
- [Troubleshooting](#troubleshooting)
- [Future Enhancements](#future-enhancements)

---

## Overview

The webhook deployment system automates the deployment of NDP releases:

```
Developer pushes tag v1.2.0
    |
    v
GitHub sends webhook to Pi
    |
    v
Pi pulls code, locates manifest, deploys
    |
    v
Pi reports status back to GitHub
```

**Benefits**:
- Zero manual intervention for deployments
- Consistent deployment process
- Audit trail via GitHub Deployments API
- Immediate feedback on deployment status

**Prerequisites**:
- NDP follows [Release Policy](RELEASE-POLICY.md)
- Manifests exist at `.deploy/releases/v{X}.{Y}.{Z}.manifest.json`
- Git tags match manifest versions

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│                              GitHub                                     │
│                                                                         │
│  ┌─────────────┐     ┌─────────────┐     ┌─────────────────────────┐   │
│  │ Repository  │────►│  Webhook    │────►│  Deployments API        │   │
│  │             │     │  (on push)  │     │  (status reporting)     │   │
│  └─────────────┘     └──────┬──────┘     └─────────────────────────┘   │
│                             │                         ▲                 │
└─────────────────────────────┼─────────────────────────┼─────────────────┘
                              │                         │
                              │ HTTPS POST              │ HTTPS POST
                              │ (signed payload)        │ (status update)
                              │                         │
┌─────────────────────────────┼─────────────────────────┼─────────────────┐
│                             ▼                         │                 │
│                    ┌─────────────────┐                │                 │
│                    │ Webhook Receiver│────────────────┘                 │
│                    │ (port 9200)     │                                  │
│                    └────────┬────────┘                                  │
│                             │                                           │
│                             ▼                                           │
│                    ┌─────────────────┐                                  │
│                    │ Deployment      │                                  │
│                    │ Orchestrator    │                                  │
│                    └────────┬────────┘                                  │
│                             │                                           │
│              ┌──────────────┼──────────────┐                            │
│              ▼              ▼              ▼                            │
│       ┌──────────┐   ┌──────────┐   ┌──────────┐                        │
│       │ git pull │   │ deploy.sh│   │ verify   │                        │
│       │          │   │ apply    │   │ status   │                        │
│       └──────────┘   └──────────┘   └──────────┘                        │
│                                                                         │
│                         Raspberry Pi                                    │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## GitHub Webhook Configuration

### Webhook Events

Configure the webhook to trigger on **tag push** events only:

| Event | Trigger | Filter |
|-------|---------|--------|
| `push` | Any push | Filter for `refs/tags/v*` |
| `create` | Tag/branch creation | Tag type, `v*` pattern |

**Recommended**: Use `push` event with tag filtering for reliability.

### Payload Structure

GitHub sends a JSON payload when a tag is pushed:

```json
{
  "ref": "refs/tags/v1.2.0",
  "ref_type": "tag",
  "repository": {
    "full_name": "dug-21/neural-data-platform",
    "clone_url": "https://github.com/dug-21/neural-data-platform.git"
  },
  "sender": {
    "login": "developer-username"
  },
  "head_commit": {
    "id": "abc123...",
    "message": "release: v1.2.0 - Add weather-station stream"
  }
}
```

**Key fields for deployment**:

| Field | Usage |
|-------|-------|
| `ref` | Extract version: `refs/tags/v1.2.0` -> `v1.2.0` |
| `repository.full_name` | Verify correct repository |
| `sender.login` | Audit trail |

### Webhook Secret

**REQUIRED**: Always configure a webhook secret for payload verification.

```bash
# Generate secret
openssl rand -hex 32
# Example: 8a7b3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b1c2d3e4f5a6b7c8d9e0f1a2b

# Store on Pi
echo "WEBHOOK_SECRET=8a7b3c4d..." >> /etc/ndp/webhook.env
```

---

## Pi Webhook Receiver

### Requirements

The webhook receiver is a lightweight HTTP server running on the Pi:

| Requirement | Specification |
|-------------|---------------|
| Language | Rust (preferred) or Shell+netcat for MVP |
| Port | 9200 (configurable) |
| Protocol | HTTPS required for production |
| Dependencies | git, deploy.sh, curl (for status reporting) |

### Endpoint Specification

```
POST /webhook/deploy
Content-Type: application/json
X-Hub-Signature-256: sha256=<signature>
X-GitHub-Event: push
X-GitHub-Delivery: <uuid>
```

**Response codes**:

| Code | Meaning |
|------|---------|
| 200 | Webhook received, deployment queued |
| 400 | Invalid payload |
| 401 | Invalid signature |
| 403 | Event type not allowed |
| 409 | Deployment already in progress |
| 500 | Internal error |

**Response body**:

```json
{
  "status": "queued",
  "deployment_id": "deploy-v1.2.0-20260202T103000Z",
  "version": "v1.2.0"
}
```

### Authentication

Two-layer authentication:

1. **Webhook signature verification** (GitHub -> Pi)
2. **GitHub token for status reporting** (Pi -> GitHub)

```bash
# Pi configuration (/etc/ndp/webhook.env)
WEBHOOK_SECRET=<github-webhook-secret>
GITHUB_TOKEN=<personal-access-token-with-repo-deployment-scope>
GITHUB_REPO=dug-21/neural-data-platform
```

### Request Validation

The receiver MUST validate:

1. **Signature**: Verify `X-Hub-Signature-256` header
2. **Event type**: Only process `push` events
3. **Ref pattern**: Only process `refs/tags/v*`
4. **Repository**: Match configured repository
5. **Manifest exists**: Verify `.deploy/releases/v{X}.{Y}.{Z}.manifest.json` exists after pull

**Signature verification pseudocode**:

```python
import hmac
import hashlib

def verify_signature(payload_body, signature_header, secret):
    expected = 'sha256=' + hmac.new(
        secret.encode(),
        payload_body,
        hashlib.sha256
    ).hexdigest()
    return hmac.compare_digest(expected, signature_header)
```

---

## Deployment Flow

### Sequence Diagram

```
GitHub          Pi Receiver        Orchestrator       deploy.sh       GitHub API
   │                 │                  │                 │                │
   │  POST /webhook  │                  │                 │                │
   │────────────────►│                  │                 │                │
   │                 │                  │                 │                │
   │                 │ verify signature │                 │                │
   │                 │ validate payload │                 │                │
   │                 │                  │                 │                │
   │  200 OK         │                  │                 │                │
   │◄────────────────│                  │                 │                │
   │                 │                  │                 │                │
   │                 │  queue deploy    │                 │                │
   │                 │─────────────────►│                 │                │
   │                 │                  │                 │                │
   │                 │                  │  create deployment (pending)     │
   │                 │                  │────────────────────────────────►│
   │                 │                  │                 │                │
   │                 │                  │  git fetch --tags                │
   │                 │                  │  git checkout v1.2.0             │
   │                 │                  │                 │                │
   │                 │                  │  update deployment (in_progress) │
   │                 │                  │────────────────────────────────►│
   │                 │                  │                 │                │
   │                 │                  │  apply manifest │                │
   │                 │                  │────────────────►│                │
   │                 │                  │                 │                │
   │                 │                  │  (9 phases)     │                │
   │                 │                  │◄────────────────│                │
   │                 │                  │                 │                │
   │                 │                  │  verify services                 │
   │                 │                  │                 │                │
   │                 │                  │  update deployment (success)     │
   │                 │                  │────────────────────────────────►│
   │                 │                  │                 │                │
```

### Phase Details

| Phase | Action | Failure Handling |
|-------|--------|------------------|
| 1. Receive | Validate webhook, return 200 | Return error code immediately |
| 2. Queue | Add to deployment queue | Reject if queue full |
| 3. Report pending | Create GitHub deployment | Continue on failure (non-blocking) |
| 4. Fetch | `git fetch --tags` | Abort, report failure |
| 5. Checkout | `git checkout v{X}.{Y}.{Z}` | Abort, report failure |
| 6. Locate manifest | Find `.deploy/releases/v{X}.{Y}.{Z}.manifest.json` | Abort, report failure |
| 7. Report in_progress | Update GitHub deployment status | Continue on failure |
| 8. Deploy | `./deploy.sh apply <manifest>` | Rollback, report failure |
| 9. Verify | Check services, data flow | Report warning if issues |
| 10. Report success | Update GitHub deployment status | Log error if fails |

---

## Security Considerations

### Transport Security

**Production MUST use HTTPS**:

```bash
# Option 1: Nginx reverse proxy with Let's Encrypt
# Nginx terminates TLS, proxies to localhost:9200

# Option 2: Cloudflare Tunnel
# Zero exposed ports, Cloudflare handles TLS

# Option 3: Tailscale/WireGuard
# VPN tunnel between GitHub Actions runner and Pi
```

**TLS certificate requirements**:
- Valid certificate (not self-signed for GitHub webhooks)
- Auto-renewal configured
- Minimum TLS 1.2

### Payload Verification

**Always verify the webhook signature**:

```bash
# Example verification in shell
verify_signature() {
    local payload="$1"
    local signature="$2"
    local secret="$3"

    expected="sha256=$(echo -n "$payload" | openssl dgst -sha256 -hmac "$secret" | cut -d' ' -f2)"

    if [ "$expected" = "$signature" ]; then
        return 0
    else
        return 1
    fi
}
```

### Access Control

| Control | Implementation |
|---------|----------------|
| IP allowlist | Only allow GitHub webhook IPs (see [GitHub Meta API](https://api.github.com/meta)) |
| Repository check | Verify `repository.full_name` matches expected |
| User allowlist | Optional: Only deploy from authorized senders |
| Tag pattern | Only deploy `v*` tags matching semver |

**GitHub webhook IP ranges** (query dynamically):

```bash
curl -s https://api.github.com/meta | jq '.hooks'
```

### Rate Limiting

Prevent abuse with rate limiting:

| Limit | Value | Rationale |
|-------|-------|-----------|
| Requests per minute | 10 | Prevent webhook flood |
| Concurrent deployments | 1 | Prevent conflicts |
| Deployment cooldown | 60 seconds | Prevent rapid-fire |

---

## Error Handling

### Error Categories

| Category | Example | Response |
|----------|---------|----------|
| Validation | Invalid signature | 401, no deployment |
| Git | Fetch failed | Report failure, no retry |
| Manifest | File not found | Report failure, log |
| Deploy | Phase failed | Rollback, report failure |
| Network | GitHub API unreachable | Log, continue (non-blocking) |

### Retry Strategy

| Error Type | Retry | Max Attempts | Backoff |
|------------|-------|--------------|---------|
| Git network | Yes | 3 | Exponential (1s, 2s, 4s) |
| Deploy phase | No | 1 | N/A (rollback instead) |
| GitHub API | Yes | 3 | Fixed (5s) |

### Rollback on Failure

If deployment fails after Phase 7 (after checkout):

```bash
# Automatic rollback
previous_version=$(cat /var/ndp/deployed-version)
previous_manifest=$(cat /var/ndp/deployed-manifest)

./deploy.sh apply "$previous_manifest"

# Report rollback
report_status "failure" "Deployment failed, rolled back to $previous_version"
```

---

## Status Reporting

### GitHub Deployment API

Report deployment status back to GitHub:

```bash
# Create deployment
curl -X POST \
  -H "Authorization: token $GITHUB_TOKEN" \
  -H "Accept: application/vnd.github+json" \
  "https://api.github.com/repos/$REPO/deployments" \
  -d '{
    "ref": "v1.2.0",
    "environment": "production",
    "description": "Automated deployment from webhook",
    "auto_merge": false,
    "required_contexts": []
  }'

# Update deployment status
curl -X POST \
  -H "Authorization: token $GITHUB_TOKEN" \
  -H "Accept: application/vnd.github+json" \
  "https://api.github.com/repos/$REPO/deployments/$DEPLOYMENT_ID/statuses" \
  -d '{
    "state": "success",
    "description": "Deployment completed successfully",
    "environment": "production"
  }'
```

**Deployment states**:

| State | When |
|-------|------|
| `pending` | Webhook received, queued |
| `in_progress` | `deploy.sh apply` started |
| `success` | Deployment and verification complete |
| `failure` | Any phase failed |
| `error` | Unexpected error |

### Local Status Files

Update device state files (existing from dp-020):

| File | Content |
|------|---------|
| `/var/ndp/deployed-version` | `v1.2.0` |
| `/var/ndp/deployed-manifest` | `.deploy/releases/v1.2.0.manifest.json` |
| `/var/ndp/deployed-timestamp` | `2026-02-02T10:30:00Z` |
| `/var/ndp/deployed-trigger` | `webhook` or `manual` |
| `/var/ndp/last-deployment.log` | Full deployment log |

### Notification Options

Optional integrations for deployment notifications:

| Channel | Integration |
|---------|-------------|
| Slack | Incoming webhook |
| Discord | Webhook |
| Email | SMTP or SendGrid |
| Pushover | Mobile push notification |

---

## Configuration

### Pi Receiver Config

Location: `/etc/ndp/webhook.env`

```bash
# Webhook receiver configuration
WEBHOOK_PORT=9200
WEBHOOK_SECRET=<github-webhook-secret>

# GitHub API (for status reporting)
GITHUB_TOKEN=<personal-access-token>
GITHUB_REPO=dug-21/neural-data-platform

# Deployment settings
DEPLOY_ROOT=/home/pi/neural-data-platform
DEPLOY_TIMEOUT=600  # 10 minutes max
DEPLOY_COOLDOWN=60  # Seconds between deployments

# Notification (optional)
SLACK_WEBHOOK_URL=https://hooks.slack.com/services/...

# Security
ALLOWED_IPS=140.82.112.0/20,192.30.252.0/22  # GitHub webhook IPs
```

### GitHub Repository Settings

1. Navigate to Repository -> Settings -> Webhooks
2. Add webhook:
   - **Payload URL**: `https://your-pi-domain.com/webhook/deploy`
   - **Content type**: `application/json`
   - **Secret**: `<same-as-WEBHOOK_SECRET>`
   - **Events**: "Just the push event"
   - **Active**: Checked

3. Create Personal Access Token:
   - Settings -> Developer settings -> Personal access tokens
   - Scopes: `repo:status`, `repo_deployment`

---

## Monitoring

### Health Check Endpoint

```
GET /health

Response:
{
  "status": "healthy",
  "last_deployment": "v1.2.0",
  "last_deployment_time": "2026-02-02T10:30:00Z",
  "deployment_in_progress": false,
  "queue_length": 0
}
```

### Metrics

Expose metrics for monitoring:

| Metric | Type | Description |
|--------|------|-------------|
| `ndp_deployments_total` | Counter | Total deployments attempted |
| `ndp_deployments_success` | Counter | Successful deployments |
| `ndp_deployments_failure` | Counter | Failed deployments |
| `ndp_deployment_duration_seconds` | Histogram | Deployment duration |
| `ndp_webhook_requests_total` | Counter | Webhook requests received |
| `ndp_webhook_invalid_total` | Counter | Invalid/rejected webhooks |

### Log Files

| Log | Location | Content |
|-----|----------|---------|
| Receiver log | `/var/log/ndp/webhook-receiver.log` | HTTP requests, validation |
| Deployment log | `/var/log/ndp/deployments/` | Per-deployment logs |
| Current deployment | `/var/ndp/last-deployment.log` | Most recent deployment |

---

## Troubleshooting

### Webhook Not Received

```
Symptom: GitHub shows delivery failure
```

**Check**:
1. Pi is reachable from internet (port forwarding, Cloudflare tunnel, etc.)
2. TLS certificate is valid
3. Receiver is running: `systemctl status ndp-webhook-receiver`
4. Firewall allows port 9200

### Signature Verification Failed

```
Symptom: 401 Unauthorized
```

**Check**:
1. Webhook secret matches between GitHub and Pi
2. No trailing whitespace in secret
3. Payload is not modified by proxy

### Deployment Fails

```
Symptom: GitHub shows deployment failed
```

**Check**:
1. View deployment log: `cat /var/ndp/last-deployment.log`
2. Verify manifest exists: `ls .deploy/releases/v{X}.{Y}.{Z}.manifest.json`
3. Check services: `./deploy.sh status`
4. Check git state: `git status`, `git describe --tags`

### Status Not Reported to GitHub

```
Symptom: Deployment succeeds but GitHub shows pending
```

**Check**:
1. GitHub token is valid: `curl -H "Authorization: token $GITHUB_TOKEN" https://api.github.com/user`
2. Token has correct scopes
3. Network connectivity to GitHub API

---

## Future Enhancements

### Planned for dp-023

| Enhancement | Description |
|-------------|-------------|
| Multi-device | Deploy to fleet of Pis |
| Canary deployments | Deploy to subset, verify, then full rollout |
| Approval gates | Require approval before production deploy |
| Environment promotion | Dev -> Staging -> Production pipeline |

### Potential Extensions

| Extension | Description |
|-----------|-------------|
| GitHub Actions integration | Use Actions instead of direct webhooks |
| ArgoCD-style GitOps | Continuous reconciliation |
| Deployment preview | Dry-run before deploy |
| A/B deployments | Run two versions simultaneously |

---

## See Also

- [Release Policy](RELEASE-POLICY.md) - Versioning and release artifacts
- [Declarative Deploy](DEPLOYMENT-DECLARATIVES.md) - Manifest format
- [Pi Deployment](../../deploy/pi/README.md) - Manual deployment commands
- [dp-021 SCOPE](../../product/features/dp-021/SCOPE.md) - Feature documentation
- [dp-023](../../product/features/dp-023/) - Webhook implementation (future)

---

*Document created: 2026-02-02*
*Feature: dp-021 Config Lifecycle & Release Management*
*Status: Specification (implementation in dp-023)*
