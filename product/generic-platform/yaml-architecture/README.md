# YAML-Based Zero-Code Domain Configuration

This directory contains the architectural design for making the generic data ingestion platform fully configurable through YAML files, requiring zero code changes to deploy new domains.

## Core Principle: Single Source of Truth (SSOT)

Each configuration element lives in exactly ONE location, and all components self-configure by reading these YAML files on deployment.

## Contents

- **YAML_ARCHITECTURE.md** - Complete architectural design
- **CONFIG_ENGINE.md** - Configuration engine design
- **DOMAIN_REGISTRY.md** - Domain discovery and management
- **domain-template.yaml** - Master template for new domains
- **examples/** - Pre-configured domain examples
- **schema/** - JSON Schema for YAML validation

## Benefits

1. **Zero Code Changes**: Deploy new domains by adding YAML files
2. **Rapid Deployment**: Minutes instead of weeks for new use cases
3. **Business User Friendly**: No programming required
4. **GitOps Ready**: Version control domain configurations
5. **A/B Testing**: Run multiple configurations simultaneously

## Quick Start

1. Copy `domain-template.yaml`
2. Customize for your domain
3. Place in `domains/` directory
4. Platform auto-discovers and configures
5. Domain is live!