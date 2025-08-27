# V2 Deployment Architecture - Neural Trader Platform (CORRECTED)

## Overview

This document provides comprehensive deployment architecture for the Neural Trader V2 platform, using a single Rust binary with embedded ruv-FANN models and DAA Coordinator, including container orchestration and operational procedures.

## Container Architecture

### Docker Container Strategy

```dockerfile
# Base Image for Rust Services
FROM rust:1.75-slim as rust-base
WORKDIR /app
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Builder Stage
FROM rust-base as builder
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release

# Runtime Stage
FROM debian:bookworm-slim as runtime
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/neural-trader /usr/local/bin/
RUN useradd -m -u 1001 trader
USER trader

EXPOSE 8080 9090
ENTRYPOINT ["/usr/local/bin/neural-trader"]
```

# REMOVED - No separate Python ML services. All ML is embedded ruv-FANN in Rust binary.

### Container Registry Structure (3 Binaries)

```yaml
registry_structure:
  repository: "gcr.io/neural-trader-v2"
  
  binary_images:
    core_binaries:
      - neural-trader-v2/neural-ml-ops:v2.0.0
      - neural-trader-v2/neural-trading:v2.0.0
      - neural-trader-v2/config-store:v2.0.0
    
    infrastructure:
      - redis/redis-stack:7.0  # Redis Streams + RedisInsight
      - timescale/timescaledb:latest
      - prom/prometheus:latest
      - grafana/grafana:latest
  
  # NO microservices - just 3 binaries + infrastructure
  
  tagging_strategy:
    - latest: "Most recent build from main"
    - v{major}.{minor}.{patch}: "Semantic version"
    - {branch}-{commit}: "Feature branch builds"
    - {environment}: "Environment-specific (prod, staging)"
```

## Kubernetes Deployment

### Namespace Organization (Simplified)

```yaml
apiVersion: v1
kind: Namespace
metadata:
  name: neural-trader-infrastructure
  labels:
    name: neural-trader-infrastructure
    environment: production
    purpose: "Redis, TimescaleDB, monitoring"
---
apiVersion: v1
kind: Namespace
metadata:
  name: neural-trader-binaries
  labels:
    name: neural-trader-binaries
    environment: production
    purpose: "neural-ml-ops, neural-trading, config-store"
# Just 2 namespaces total - infrastructure + binaries
```

### Core Service Deployments

```yaml
# Market Data Service Deployment
apiVersion: apps/v1
kind: Deployment
metadata:
  name: market-data-service
  namespace: neural-trader-trading
  labels:
    app: market-data
    version: v2.0.0
spec:
  replicas: 3
  selector:
    matchLabels:
      app: market-data
  template:
    metadata:
      labels:
        app: market-data
        version: v2.0.0
      annotations:
        prometheus.io/scrape: "true"
        prometheus.io/port: "9090"
        prometheus.io/path: "/metrics"
    spec:
      serviceAccountName: market-data
      
      initContainers:
      - name: migration
        image: gcr.io/neural-trader/market-data:v2.0.0
        command: ["/bin/sh", "-c", "neural-trader migrate"]
        env:
        - name: DATABASE_URL
          valueFrom:
            secretKeyRef:
              name: db-credentials
              key: url
      
      containers:
      - name: market-data
        image: gcr.io/neural-trader/market-data:v2.0.0
        ports:
        - containerPort: 8080
          name: http
          protocol: TCP
        - containerPort: 9090
          name: metrics
          protocol: TCP
        
        env:
        - name: RUST_LOG
          value: "info"
        - name: REDIS_URL
          valueFrom:
            secretKeyRef:
              name: redis-credentials
              key: url
        
        resources:
          requests:
            memory: "512Mi"
            cpu: "500m"
          limits:
            memory: "1Gi"
            cpu: "1000m"
        
        livenessProbe:
          httpGet:
            path: /health/live
            port: 8080
          initialDelaySeconds: 30
          periodSeconds: 10
          timeoutSeconds: 5
          failureThreshold: 3
        
        readinessProbe:
          httpGet:
            path: /health/ready
            port: 8080
          initialDelaySeconds: 10
          periodSeconds: 5
          timeoutSeconds: 3
          failureThreshold: 3
        
        volumeMounts:
        - name: config
          mountPath: /etc/neural-trader
          readOnly: true
        - name: cache
          mountPath: /var/cache/neural-trader
      
      volumes:
      - name: config
        configMap:
          name: market-data-config
      - name: cache
        emptyDir:
          sizeLimit: 1Gi
      
      affinity:
        podAntiAffinity:
          preferredDuringSchedulingIgnoredDuringExecution:
          - weight: 100
            podAffinityTerm:
              labelSelector:
                matchExpressions:
                - key: app
                  operator: In
                  values:
                  - market-data
              topologyKey: kubernetes.io/hostname
```

### StatefulSet for Persistent Services

```yaml
# Redis Streams StatefulSet
apiVersion: apps/v1
kind: StatefulSet
metadata:
  name: redis-streams
  namespace: neural-trader-infrastructure
spec:
  serviceName: redis-streams
  replicas: 3
  selector:
    matchLabels:
      app: redis-streams
  template:
    metadata:
      labels:
        app: redis-streams
    spec:
      containers:
      - name: redis
        image: redis:7-alpine
        ports:
        - containerPort: 6379
          name: redis
        command:
        - redis-server
        - /usr/local/etc/redis/redis.conf
        
        volumeMounts:
        - name: data
          mountPath: /data
        - name: config
          mountPath: /usr/local/etc/redis
        
        resources:
          requests:
            memory: "2Gi"
            cpu: "1000m"
          limits:
            memory: "4Gi"
            cpu: "2000m"
      
      volumes:
      - name: config
        configMap:
          name: redis-config
  
  volumeClaimTemplates:
  - metadata:
      name: data
    spec:
      accessModes: ["ReadWriteOnce"]
      storageClassName: fast-ssd
      resources:
        requests:
          storage: 100Gi
```

### Horizontal Pod Autoscaler

```yaml
# HPA for Strategy Engine
apiVersion: autoscaling/v2
kind: HorizontalPodAutoscaler
metadata:
  name: strategy-engine-hpa
  namespace: neural-trader-trading
spec:
  scaleTargetRef:
    apiVersion: apps/v1
    kind: Deployment
    name: strategy-engine
  
  minReplicas: 2
  maxReplicas: 20
  
  metrics:
  - type: Resource
    resource:
      name: cpu
      target:
        type: Utilization
        averageUtilization: 70
  
  - type: Resource
    resource:
      name: memory
      target:
        type: Utilization
        averageUtilization: 80
  
  - type: Pods
    pods:
      metric:
        name: message_queue_depth
      target:
        type: AverageValue
        averageValue: "1000"
  
  behavior:
    scaleDown:
      stabilizationWindowSeconds: 300
      policies:
      - type: Percent
        value: 50
        periodSeconds: 60
    
    scaleUp:
      stabilizationWindowSeconds: 60
      policies:
      - type: Percent
        value: 100
        periodSeconds: 60
      - type: Pods
        value: 4
        periodSeconds: 60
```

## Helm Charts

### Chart Structure

```yaml
# Chart.yaml
apiVersion: v2
name: neural-trader
description: Neural Trader V2 Platform
type: application
version: 2.0.0
appVersion: "2.0.0"

dependencies:
  - name: redis
    version: 17.0.0
    repository: https://charts.bitnami.com/bitnami
    condition: redis.enabled
  
  - name: postgresql
    version: 12.0.0
    repository: https://charts.bitnami.com/bitnami
    condition: postgresql.enabled
  
  - name: prometheus
    version: 19.0.0
    repository: https://prometheus-community.github.io/helm-charts
    condition: monitoring.prometheus.enabled
```

### Values Configuration

```yaml
# values.yaml
global:
  environment: production
  region: us-east-1
  domain: neural-trader.io

infrastructure:
  redis:
    enabled: true
    cluster:
      enabled: true
      nodes: 6
    persistence:
      enabled: true
      size: 100Gi
      storageClass: fast-ssd
  
  postgresql:
    enabled: true
    replication:
      enabled: true
      readReplicas: 2
    persistence:
      size: 500Gi
      storageClass: fast-ssd

services:
  marketData:
    replicas: 3
    image:
      repository: gcr.io/neural-trader/market-data
      tag: v2.0.0
    resources:
      requests:
        memory: 512Mi
        cpu: 500m
      limits:
        memory: 1Gi
        cpu: 1000m
    
    autoscaling:
      enabled: true
      minReplicas: 3
      maxReplicas: 10
      targetCPU: 70
      targetMemory: 80
  
  strategyEngine:
    replicas: 2
    image:
      repository: gcr.io/neural-trader/strategy-engine
      tag: v2.0.0
    resources:
      requests:
        memory: 1Gi
        cpu: 1000m
      limits:
        memory: 2Gi
        cpu: 2000m

monitoring:
  prometheus:
    enabled: true
    retention: 30d
    storageSize: 100Gi
  
  grafana:
    enabled: true
    dashboards:
      - trading-overview
      - system-performance
      - ml-metrics
  
  alertmanager:
    enabled: true
    config:
      receivers:
        - name: pagerduty
          pagerduty_configs:
            - service_key: ${PAGERDUTY_KEY}
```

## Infrastructure as Code

### Terraform Configuration

```hcl
# main.tf
terraform {
  required_version = ">= 1.0"
  
  required_providers {
    google = {
      source  = "hashicorp/google"
      version = "~> 4.0"
    }
    kubernetes = {
      source  = "hashicorp/kubernetes"
      version = "~> 2.0"
    }
  }
  
  backend "gcs" {
    bucket = "neural-trader-terraform-state"
    prefix = "v2/production"
  }
}

# GKE Cluster
resource "google_container_cluster" "primary" {
  name     = "neural-trader-primary"
  location = var.region
  
  initial_node_count       = 1
  remove_default_node_pool = true
  
  network    = google_compute_network.main.id
  subnetwork = google_compute_subnetwork.main.id
  
  workload_identity_config {
    workload_pool = "${var.project_id}.svc.id.goog"
  }
  
  addons_config {
    horizontal_pod_autoscaling {
      disabled = false
    }
    
    http_load_balancing {
      disabled = false
    }
    
    network_policy_config {
      disabled = false
    }
    
    gce_persistent_disk_csi_driver_config {
      enabled = true
    }
  }
  
  cluster_autoscaling {
    enabled = true
    
    resource_limits {
      resource_type = "cpu"
      minimum       = 10
      maximum       = 100
    }
    
    resource_limits {
      resource_type = "memory"
      minimum       = 40
      maximum       = 400
    }
  }
}

# Node Pools
resource "google_container_node_pool" "system" {
  name       = "system-pool"
  location   = var.region
  cluster    = google_container_cluster.primary.name
  node_count = 3
  
  node_config {
    preemptible  = false
    machine_type = "n2-standard-4"
    
    disk_size_gb = 100
    disk_type    = "pd-ssd"
    
    labels = {
      workload = "system"
    }
    
    taint {
      key    = "workload"
      value  = "system"
      effect = "NO_SCHEDULE"
    }
    
    oauth_scopes = [
      "https://www.googleapis.com/auth/cloud-platform"
    ]
  }
  
  autoscaling {
    min_node_count = 3
    max_node_count = 10
  }
}

resource "google_container_node_pool" "compute" {
  name     = "compute-pool"
  location = var.region
  cluster  = google_container_cluster.primary.name
  
  node_config {
    preemptible  = true
    machine_type = "n2-highcpu-8"
    
    disk_size_gb = 200
    disk_type    = "pd-ssd"
    
    labels = {
      workload = "compute"
    }
    
    taint {
      key    = "workload"
      value  = "compute"
      effect = "NO_SCHEDULE"
    }
  }
  
  autoscaling {
    min_node_count = 2
    max_node_count = 50
  }
}

# REMOVED - No separate ML nodes needed, ruv-FANN runs in main compute pool
```

### Ansible Playbooks

```yaml
# deploy.yml
---
- name: Deploy Neural Trader V2
  hosts: localhost
  gather_facts: no
  
  vars:
    environment: "{{ env | default('staging') }}"
    namespace: "neural-trader-{{ environment }}"
    chart_version: "2.0.0"
  
  tasks:
    - name: Create namespace
      kubernetes.core.k8s:
        name: "{{ namespace }}"
        api_version: v1
        kind: Namespace
        state: present
    
    - name: Deploy secrets
      kubernetes.core.k8s:
        definition:
          apiVersion: v1
          kind: Secret
          metadata:
            name: "{{ item.name }}"
            namespace: "{{ namespace }}"
          data: "{{ item.data }}"
      loop:
        - name: db-credentials
          data:
            url: "{{ db_url | b64encode }}"
        - name: redis-credentials
          data:
            url: "{{ redis_url | b64encode }}"
    
    - name: Deploy Helm chart
      kubernetes.core.helm:
        name: neural-trader
        chart_ref: ./charts/neural-trader
        release_namespace: "{{ namespace }}"
        values_files:
          - "./values/{{ environment }}.yaml"
        wait: true
        wait_timeout: 600
    
    - name: Wait for deployments
      kubernetes.core.k8s_info:
        api_version: apps/v1
        kind: Deployment
        namespace: "{{ namespace }}"
        wait_condition:
          type: Progressing
          status: "True"
        wait_timeout: 600
```

## CI/CD Pipeline

### GitHub Actions Workflow

```yaml
# .github/workflows/deploy.yml
name: Deploy Neural Trader V2

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

env:
  GCP_PROJECT: neural-trader
  GKE_CLUSTER: neural-trader-primary
  GKE_ZONE: us-east1-b
  REGISTRY: gcr.io

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      
      - name: Setup Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
          override: true
      
      - name: Run tests
        run: |
          cargo test --all-features
          cargo clippy -- -D warnings
      
      - name: Run integration tests
        run: |
          docker-compose -f docker-compose.test.yml up -d
          cargo test --test integration
          docker-compose -f docker-compose.test.yml down

  build:
    needs: test
    runs-on: ubuntu-latest
    strategy:
      matrix:
        service: [neural-trader]  # Single Rust binary only
    
    steps:
      - uses: actions/checkout@v3
      
      - name: Set up Docker Buildx
        uses: docker/setup-buildx-action@v2
      
      - name: Authenticate to Google Cloud
        uses: google-github-actions/auth@v1
        with:
          credentials_json: ${{ secrets.GCP_SA_KEY }}
      
      - name: Configure Docker for GCR
        run: gcloud auth configure-docker
      
      - name: Build and push Docker image
        uses: docker/build-push-action@v4
        with:
          context: ./services/${{ matrix.service }}
          push: true
          tags: |
            ${{ env.REGISTRY }}/${{ env.GCP_PROJECT }}/${{ matrix.service }}:${{ github.sha }}
            ${{ env.REGISTRY }}/${{ env.GCP_PROJECT }}/${{ matrix.service }}:latest
          cache-from: type=gha
          cache-to: type=gha,mode=max

  deploy-staging:
    needs: build
    runs-on: ubuntu-latest
    environment: staging
    
    steps:
      - uses: actions/checkout@v3
      
      - name: Authenticate to Google Cloud
        uses: google-github-actions/auth@v1
        with:
          credentials_json: ${{ secrets.GCP_SA_KEY }}
      
      - name: Get GKE credentials
        uses: google-github-actions/get-gke-credentials@v1
        with:
          cluster_name: ${{ env.GKE_CLUSTER }}-staging
          location: ${{ env.GKE_ZONE }}
      
      - name: Deploy to staging
        run: |
          helm upgrade --install neural-trader ./charts/neural-trader \
            --namespace neural-trader-staging \
            --create-namespace \
            --values ./charts/neural-trader/values/staging.yaml \
            --set-string global.imageTag=${{ github.sha }} \
            --wait

  deploy-production:
    needs: deploy-staging
    runs-on: ubuntu-latest
    environment: production
    if: github.ref == 'refs/heads/main'
    
    steps:
      - uses: actions/checkout@v3
      
      - name: Authenticate to Google Cloud
        uses: google-github-actions/auth@v1
        with:
          credentials_json: ${{ secrets.GCP_SA_KEY }}
      
      - name: Get GKE credentials
        uses: google-github-actions/get-gke-credentials@v1
        with:
          cluster_name: ${{ env.GKE_CLUSTER }}
          location: ${{ env.GKE_ZONE }}
      
      - name: Blue-Green Deployment
        run: |
          # Deploy to green environment
          helm upgrade --install neural-trader-green ./charts/neural-trader \
            --namespace neural-trader-production \
            --create-namespace \
            --values ./charts/neural-trader/values/production.yaml \
            --set-string global.imageTag=${{ github.sha }} \
            --set-string global.deployment=green \
            --wait
          
          # Run smoke tests
          ./scripts/smoke-tests.sh green
          
          # Switch traffic to green
          kubectl patch service neural-trader \
            -n neural-trader-production \
            -p '{"spec":{"selector":{"deployment":"green"}}}'
          
          # Wait and monitor
          sleep 60
          ./scripts/monitor-deployment.sh
          
          # Clean up blue deployment
          helm uninstall neural-trader-blue -n neural-trader-production || true
```

## Monitoring & Operations

### Prometheus Configuration

```yaml
# prometheus-config.yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: prometheus-config
  namespace: neural-trader-infrastructure
data:
  prometheus.yml: |
    global:
      scrape_interval: 15s
      evaluation_interval: 15s
    
    rule_files:
      - /etc/prometheus/rules/*.yml
    
    alerting:
      alertmanagers:
        - static_configs:
            - targets:
                - alertmanager:9093
    
    scrape_configs:
      - job_name: kubernetes-pods
        kubernetes_sd_configs:
          - role: pod
        relabel_configs:
          - source_labels: [__meta_kubernetes_pod_annotation_prometheus_io_scrape]
            action: keep
            regex: true
          - source_labels: [__meta_kubernetes_pod_annotation_prometheus_io_path]
            action: replace
            target_label: __metrics_path__
            regex: (.+)
      
      - job_name: neural-trader-services
        kubernetes_sd_configs:
          - role: service
            namespaces:
              names:
                - neural-trader-trading
                - neural-trader-ml
                - neural-trader-platform
        relabel_configs:
          - source_labels: [__meta_kubernetes_service_label_monitoring]
            action: keep
            regex: prometheus
```

### Grafana Dashboards

```json
{
  "dashboard": {
    "title": "Neural Trader Trading Performance",
    "panels": [
      {
        "title": "Order Execution Latency",
        "targets": [
          {
            "expr": "histogram_quantile(0.99, sum(rate(order_execution_duration_seconds_bucket[5m])) by (le))",
            "legendFormat": "p99 latency"
          }
        ]
      },
      {
        "title": "Trading Signals Generated",
        "targets": [
          {
            "expr": "sum(rate(trading_signals_total[5m])) by (strategy)",
            "legendFormat": "{{ strategy }}"
          }
        ]
      },
      {
        "title": "Model Inference Performance",
        "targets": [
          {
            "expr": "histogram_quantile(0.95, sum(rate(model_inference_duration_seconds_bucket[5m])) by (le, model))",
            "legendFormat": "{{ model }} p95"
          }
        ]
      }
    ]
  }
}
```

## Disaster Recovery

### Backup Strategy

```yaml
# backup-cronjob.yaml
apiVersion: batch/v1
kind: CronJob
metadata:
  name: database-backup
  namespace: neural-trader-infrastructure
spec:
  schedule: "0 */6 * * *"  # Every 6 hours
  jobTemplate:
    spec:
      template:
        spec:
          containers:
          - name: backup
            image: gcr.io/neural-trader/backup-tool:latest
            command:
            - /bin/sh
            - -c
            - |
              # Backup PostgreSQL
              pg_dump $DATABASE_URL | gzip > /backup/db-$(date +%Y%m%d-%H%M%S).sql.gz
              
              # Backup Redis
              redis-cli --rdb /backup/redis-$(date +%Y%m%d-%H%M%S).rdb
              
              # Upload to GCS
              gsutil -m cp /backup/* gs://neural-trader-backups/$(date +%Y%m%d)/
              
              # Clean old backups
              find /backup -mtime +7 -delete
            
            env:
            - name: DATABASE_URL
              valueFrom:
                secretKeyRef:
                  name: db-credentials
                  key: url
            
            volumeMounts:
            - name: backup
              mountPath: /backup
          
          volumes:
          - name: backup
            persistentVolumeClaim:
              claimName: backup-pvc
          
          restartPolicy: OnFailure
```

### Disaster Recovery Procedures

```yaml
disaster_recovery:
  backup_schedule:
    databases: "Every 6 hours"
    configuration: "On change"
    models: "After training"
    code: "Continuous (Git)"
  
  recovery_procedures:
    data_loss:
      1: "Identify last known good backup"
      2: "Restore database from backup"
      3: "Replay events from event store"
      4: "Validate data integrity"
      5: "Resume operations"
    
    region_failure:
      1: "Detect region failure via health checks"
      2: "Promote secondary region to primary"
      3: "Update DNS to point to new primary"
      4: "Start replication to new secondary"
      5: "Notify operations team"
    
    corruption:
      1: "Isolate affected components"
      2: "Identify corruption source"
      3: "Restore from clean backup"
      4: "Validate restored data"
      5: "Apply missing transactions"
  
  testing_schedule:
    backup_restore: "Monthly"
    region_failover: "Quarterly"
    full_dr_drill: "Bi-annually"
```

## Security Hardening

### Network Policies

```yaml
# network-policy.yaml
apiVersion: networking.k8s.io/v1
kind: NetworkPolicy
metadata:
  name: trading-services-policy
  namespace: neural-trader-trading
spec:
  podSelector:
    matchLabels:
      tier: trading
  
  policyTypes:
  - Ingress
  - Egress
  
  ingress:
  - from:
    - namespaceSelector:
        matchLabels:
          name: neural-trader-platform
    - podSelector:
        matchLabels:
          tier: api-gateway
    ports:
    - protocol: TCP
      port: 8080
  
  egress:
  - to:
    - namespaceSelector:
        matchLabels:
          name: neural-trader-infrastructure
    ports:
    - protocol: TCP
      port: 6379  # Redis
    - protocol: TCP
      port: 5432  # PostgreSQL
```

### Pod Security Policies

```yaml
# pod-security-policy.yaml
apiVersion: policy/v1beta1
kind: PodSecurityPolicy
metadata:
  name: neural-trader-restricted
spec:
  privileged: false
  allowPrivilegeEscalation: false
  requiredDropCapabilities:
    - ALL
  
  volumes:
    - configMap
    - emptyDir
    - projected
    - secret
    - persistentVolumeClaim
  
  runAsUser:
    rule: MustRunAsNonRoot
  
  seLinux:
    rule: RunAsAny
  
  fsGroup:
    rule: RunAsAny
  
  readOnlyRootFilesystem: true
```

## Performance Optimization

### Resource Optimization

```yaml
resource_optimization:
  vertical_pod_autoscaler:
    enabled: true
    update_mode: "Auto"
    
    recommendations:
      - service: market-data
        cpu: "800m -> 1200m"
        memory: "768Mi -> 1Gi"
      
      - service: strategy-engine
        cpu: "1500m -> 2000m"
        memory: "1.5Gi -> 2Gi"
  
  cluster_autoscaler:
    scale_down_delay: "10m"
    scale_down_utilization_threshold: 0.5
    max_node_provision_time: "15m"
    
    node_groups:
      - name: compute-pool
        min: 2
        max: 50
        target_utilization: 0.7
      
      - name: ml-pool
        min: 1
        max: 10
        target_utilization: 0.8
```

## Cost Management

### Cost Optimization Strategies

```yaml
cost_optimization:
  compute:
    - Use preemptible instances for non-critical workloads
    - Right-size instances based on actual usage
    - Use committed use discounts for baseline capacity
    - Implement aggressive autoscaling policies
  
  storage:
    - Use tiered storage (hot/warm/cold)
    - Implement data retention policies
    - Compress historical data
    - Use object storage for backups
  
  network:
    - Minimize cross-region traffic
    - Use CDN for static content
    - Implement caching strategies
    - Optimize API payloads
  
  monitoring:
    budget_alerts:
      - threshold: 80%
        notification: email
      - threshold: 90%
        notification: slack
      - threshold: 100%
        notification: pagerduty
```