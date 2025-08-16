# Neural Trader - Deployment Architecture

## Container Deployment Overview

The Neural Trader platform deploys as a multi-container Docker application with production-ready orchestration, monitoring, and data persistence.

## Production Deployment Architecture

```mermaid
graph TB
    %% External Network
    subgraph "External Network"
        INTERNET[Internet]
        MARKET_APIS[Market Data APIs<br/>- Alpaca WebSocket<br/>- Polygon API<br/>- Yahoo Finance]
        ADMIN[Administrator<br/>Access]
    end

    %% Reverse Proxy Layer
    subgraph "Reverse Proxy Layer"
        NGINX[Nginx<br/>Port 80/443<br/>SSL Termination<br/>Load Balancing]
    end

    %% Application Layer
    subgraph "Application Services"
        DI[Data Ingestion<br/>Python:8000<br/>Real-time streaming]
        NT[Neural Trader<br/>Rust:8080<br/>Trading engine]
        MM[Model Manager<br/>Port 8090<br/>ML model storage]
    end

    %% Data Layer
    subgraph "Data Platform"
        TS[TimescaleDB<br/>Port 5432<br/>Time-series storage]
        REDIS[Redis<br/>Port 6379<br/>Cache & pub/sub]
        MODELS[Model Storage<br/>Persistent volume<br/>Neural networks]
    end

    %% Monitoring Layer
    subgraph "Monitoring Stack"
        PROM[Prometheus<br/>Port 9090<br/>Metrics collection]
        GRAF[Grafana<br/>Port 3000<br/>Visualization]
        ALERT[Alert Manager<br/>Port 9093<br/>Notifications]
    end

    %% Network Connections
    INTERNET --> NGINX
    MARKET_APIS --> DI
    ADMIN --> NGINX

    NGINX --> DI
    NGINX --> NT
    NGINX --> GRAF

    DI --> TS
    DI --> REDIS
    DI --> PROM

    NT --> TS
    NT --> REDIS
    NT --> MODELS
    NT --> PROM

    MM --> MODELS
    MM --> PROM

    PROM --> GRAF
    PROM --> ALERT
```

## Docker Container Architecture

### 1. Container Definitions

```yaml
# Production container stack
services:
  # Data ingestion service
  data-ingestion:
    image: neural-trader/data-ingestion:latest
    ports: ["8000:8000"]
    environment:
      - DATABASE_URL=postgresql://user:pass@timescaledb:5432/neural_trader
      - REDIS_URL=redis://redis:6379
      - ALPACA_API_KEY=${ALPACA_API_KEY}
    volumes:
      - ./config:/app/config:ro
    depends_on: [timescaledb, redis]
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8000/health"]
      interval: 30s
      timeout: 10s
      retries: 3

  # Neural trading engine
  neural-trader:
    image: neural-trader/neural-trader:latest
    ports: ["8080:8080"]
    environment:
      - DATABASE_URL=postgresql://user:pass@timescaledb:5432/neural_trader
      - REDIS_URL=redis://redis:6379
      - MODEL_STORAGE_PATH=/models
    volumes:
      - ./models:/models
      - ./config:/app/config:ro
    depends_on: [timescaledb, redis, data-ingestion]
    
  # TimescaleDB with initialization
  timescaledb:
    image: timescale/timescaledb:latest-pg15
    ports: ["5432:5432"]
    environment:
      - POSTGRES_DB=neural_trader
      - POSTGRES_USER=neural_trader
      - POSTGRES_PASSWORD=${POSTGRES_PASSWORD}
    volumes:
      - timescale_data:/var/lib/postgresql/data
      - ./docker/timescaledb/init-scripts:/docker-entrypoint-initdb.d:ro
    
  # Redis for caching and pub/sub
  redis:
    image: redis:7-alpine
    ports: ["6379:6379"]
    environment:
      - REDIS_PASSWORD=${REDIS_PASSWORD}
    volumes:
      - redis_data:/data
      - ./docker/redis/redis.conf:/usr/local/etc/redis/redis.conf:ro
    command: redis-server /usr/local/etc/redis/redis.conf

  # Prometheus monitoring
  prometheus:
    image: prom/prometheus:latest
    ports: ["9090:9090"]
    volumes:
      - ./docker/prometheus/prometheus.yml:/etc/prometheus/prometheus.yml:ro
      - prometheus_data:/prometheus
    command:
      - '--config.file=/etc/prometheus/prometheus.yml'
      - '--storage.tsdb.path=/prometheus'
      - '--web.console.libraries=/etc/prometheus/console_libraries'
      - '--web.console.templates=/etc/prometheus/consoles'

  # Grafana dashboards
  grafana:
    image: grafana/grafana:latest
    ports: ["3000:3000"]
    environment:
      - GF_SECURITY_ADMIN_PASSWORD=${GRAFANA_PASSWORD}
    volumes:
      - grafana_data:/var/lib/grafana
      - ./docker/grafana/provisioning:/etc/grafana/provisioning:ro
      - ./docker/grafana/dashboards:/var/lib/grafana/dashboards:ro

volumes:
  timescale_data:
    driver: local
  redis_data:
    driver: local
  prometheus_data:
    driver: local
  grafana_data:
    driver: local
```

### 2. Container Networking

```mermaid
graph TB
    subgraph "Docker Network: neural-trader-network"
        subgraph "Frontend Services"
            NGINX_C[nginx<br/>neural-trader-proxy<br/>80:80, 443:443]
        end
        
        subgraph "Application Services"
            DI_C[data-ingestion<br/>neural-trader-ingestion<br/>8000:8000]
            NT_C[neural-trader<br/>neural-trader-engine<br/>8080:8080]
            MM_C[model-manager<br/>neural-trader-models<br/>8090:8090]
        end
        
        subgraph "Data Services"
            TS_C[timescaledb<br/>neural-trader-db<br/>5432:5432]
            REDIS_C[redis<br/>neural-trader-cache<br/>6379:6379]
        end
        
        subgraph "Monitoring Services"
            PROM_C[prometheus<br/>neural-trader-metrics<br/>9090:9090]
            GRAF_C[grafana<br/>neural-trader-dashboards<br/>3000:3000]
        end
    end

    %% Internal network connections
    NGINX_C --> DI_C
    NGINX_C --> NT_C
    NGINX_C --> GRAF_C
    
    DI_C --> TS_C
    DI_C --> REDIS_C
    DI_C --> PROM_C
    
    NT_C --> TS_C
    NT_C --> REDIS_C
    NT_C --> PROM_C
    
    MM_C --> PROM_C
    
    PROM_C --> GRAF_C
```

## Volume and Data Persistence

### 1. Persistent Volume Architecture

```mermaid
graph TB
    subgraph "Host File System"
        HOST_CONFIG[/opt/neural-trader/config<br/>Configuration files]
        HOST_MODELS[/opt/neural-trader/models<br/>Neural network models]
        HOST_LOGS[/opt/neural-trader/logs<br/>Application logs]
        HOST_BACKUPS[/opt/neural-trader/backups<br/>Database backups]
    end

    subgraph "Docker Volumes"
        VOL_TIMESCALE[timescale_data<br/>PostgreSQL data]
        VOL_REDIS[redis_data<br/>Redis persistence]
        VOL_PROMETHEUS[prometheus_data<br/>Metrics storage]
        VOL_GRAFANA[grafana_data<br/>Dashboard configs]
    end

    subgraph "Container Mounts"
        CONT_CONFIG[Container /app/config<br/>Read-only configs]
        CONT_MODELS[Container /models<br/>Read-write models]
        CONT_LOGS[Container /var/log<br/>Write-only logs]
    end

    %% Mount relationships
    HOST_CONFIG --> CONT_CONFIG
    HOST_MODELS --> CONT_MODELS
    HOST_LOGS --> CONT_LOGS
    
    VOL_TIMESCALE -.->|persistent| CONT_CONFIG
    VOL_REDIS -.->|persistent| CONT_CONFIG
    VOL_PROMETHEUS -.->|persistent| CONT_CONFIG
    VOL_GRAFANA -.->|persistent| CONT_CONFIG
```

### 2. Storage Requirements

| Component | Storage Type | Size Estimate | Backup Frequency | Retention |
|-----------|-------------|---------------|------------------|-----------|
| TimescaleDB | Block storage | 10GB/month | Daily | 1 year |
| Redis | Memory + disk | 1GB | Hourly snapshots | 7 days |
| Model storage | File system | 500MB | On model update | All versions |
| Prometheus | Time-series | 5GB/month | Weekly | 6 months |
| Grafana | Configuration | 100MB | On config change | All versions |
| Application logs | Log files | 1GB/month | Daily | 3 months |

## Health Checks and Service Discovery

### 1. Health Check Configuration

```mermaid
sequenceDiagram
    participant Docker as Docker Engine
    participant App as Application
    participant Health as Health Endpoint
    participant Monitor as Monitoring

    Note over Docker,Monitor: Container health monitoring

    loop Every 30 seconds
        Docker->>App: Health check request
        App->>Health: Check internal status
        Health->>Health: Validate dependencies
        Health->>App: Health status
        App->>Docker: HTTP 200/503 response
        
        alt Healthy
            Docker->>Monitor: Container healthy
        else Unhealthy
            Docker->>Monitor: Container unhealthy
            Docker->>App: Restart container
        end
    end
```

### 2. Service Dependencies

```mermaid
graph TB
    subgraph "Service Startup Order"
        STEP1[1. Infrastructure Services<br/>TimescaleDB, Redis]
        STEP2[2. Data Services<br/>Data Ingestion]
        STEP3[3. Processing Services<br/>Neural Trader Engine]
        STEP4[4. Monitoring Services<br/>Prometheus, Grafana]
        STEP5[5. Proxy Services<br/>Nginx]
    end

    subgraph "Health Dependencies"
        TS_HEALTH[TimescaleDB<br/>Ready when accepting connections]
        REDIS_HEALTH[Redis<br/>Ready when ping responds]
        DI_HEALTH[Data Ingestion<br/>Ready when /health returns 200]
        NT_HEALTH[Neural Trader<br/>Ready when models loaded]
    end

    STEP1 --> STEP2
    STEP2 --> STEP3
    STEP3 --> STEP4
    STEP4 --> STEP5

    TS_HEALTH -.->|dependency| DI_HEALTH
    REDIS_HEALTH -.->|dependency| DI_HEALTH
    DI_HEALTH -.->|dependency| NT_HEALTH
```

## Security Architecture

### 1. Network Security

```mermaid
graph TB
    subgraph "External Network"
        INTERNET[Internet Traffic]
        APIS[External APIs]
    end

    subgraph "DMZ (Demilitarized Zone)"
        NGINX[Nginx Reverse Proxy<br/>- SSL termination<br/>- Rate limiting<br/>- Request filtering]
        WAF[Web Application Firewall<br/>- SQL injection protection<br/>- XSS prevention<br/>- DDoS mitigation]
    end

    subgraph "Application Network (Isolated)"
        APP_SERVICES[Application Services<br/>- Data ingestion<br/>- Neural trader<br/>- Model manager]
        
        subgraph "Data Network (Highly Restricted)"
            DATA_SERVICES[Data Services<br/>- TimescaleDB<br/>- Redis<br/>- Model storage]
        end
    end

    %% Security boundaries
    INTERNET --> WAF
    APIS --> NGINX
    WAF --> NGINX
    NGINX --> APP_SERVICES
    APP_SERVICES --> DATA_SERVICES
```

### 2. Container Security

```yaml
# Security configuration examples
security_opt:
  - no-new-privileges:true
  - seccomp:./docker/security/seccomp-profile.json

# Resource limits
deploy:
  resources:
    limits:
      cpus: '2.0'
      memory: 4G
    reservations:
      cpus: '1.0'
      memory: 2G

# User permissions (non-root)
user: 1000:1000

# Read-only root filesystem
read_only: true
tmpfs:
  - /tmp
  - /var/log
```

## Monitoring and Observability

### 1. Metrics Collection Flow

```mermaid
flowchart LR
    subgraph "Application Metrics"
        APP_DI[Data Ingestion<br/>:8000/metrics]
        APP_NT[Neural Trader<br/>:8080/metrics]
        APP_MM[Model Manager<br/>:8090/metrics]
    end

    subgraph "Infrastructure Metrics"
        DOCKER[Docker Stats<br/>Container metrics]
        HOST[Host Metrics<br/>System resources]
        DB[Database Metrics<br/>TimescaleDB stats]
    end

    subgraph "Metrics Storage"
        PROMETHEUS[Prometheus<br/>- Scrape endpoints<br/>- Store time series<br/>- Apply recording rules]
    end

    subgraph "Alerting & Visualization"
        ALERTMGR[Alert Manager<br/>- Threshold alerts<br/>- Notification routing<br/>- Escalation]
        GRAFANA[Grafana<br/>- Real-time dashboards<br/>- Historical analysis<br/>- Custom alerts]
    end

    %% Metrics flow
    APP_DI --> PROMETHEUS
    APP_NT --> PROMETHEUS
    APP_MM --> PROMETHEUS
    DOCKER --> PROMETHEUS
    HOST --> PROMETHEUS
    DB --> PROMETHEUS

    PROMETHEUS --> ALERTMGR
    PROMETHEUS --> GRAFANA
```

### 2. Log Aggregation Architecture

```mermaid
graph TB
    subgraph "Log Sources"
        APP_LOGS[Application Logs<br/>- Structured JSON<br/>- Error tracking<br/>- Performance logs]
        CONTAINER_LOGS[Container Logs<br/>- Docker stdout/stderr<br/>- System messages<br/>- Health check logs]
        SYSTEM_LOGS[System Logs<br/>- OS events<br/>- Docker daemon<br/>- Network events]
    end

    subgraph "Log Collection"
        FILEBEAT[Filebeat<br/>- Log shipping<br/>- Multi-line parsing<br/>- Metadata enrichment]
        DOCKER_DRIVER[Docker Log Driver<br/>- JSON file driver<br/>- Log rotation<br/>- Size limits]
    end

    subgraph "Log Processing"
        LOGSTASH[Logstash<br/>- Log parsing<br/>- Field extraction<br/>- Format standardization]
        PIPELINE[Processing Pipeline<br/>- Filter & transform<br/>- Enrich with metadata<br/>- Route by severity]
    end

    subgraph "Log Storage & Search"
        ELASTICSEARCH[Elasticsearch<br/>- Full-text indexing<br/>- Time-based indices<br/>- Retention policies]
        KIBANA[Kibana<br/>- Log visualization<br/>- Search interface<br/>- Dashboard creation]
    end

    %% Log flow
    APP_LOGS --> FILEBEAT
    CONTAINER_LOGS --> DOCKER_DRIVER
    SYSTEM_LOGS --> FILEBEAT

    FILEBEAT --> LOGSTASH
    DOCKER_DRIVER --> LOGSTASH

    LOGSTASH --> PIPELINE
    PIPELINE --> ELASTICSEARCH
    ELASTICSEARCH --> KIBANA
```

## Disaster Recovery and Backup

### 1. Backup Strategy

```mermaid
graph TB
    subgraph "Data Sources"
        TS_DB[TimescaleDB<br/>Primary database]
        REDIS_MEM[Redis<br/>Cache & sessions]
        MODEL_FILES[Model Storage<br/>Neural networks]
        CONFIG_FILES[Configuration<br/>System settings]
    end

    subgraph "Backup Types"
        FULL_BACKUP[Full Backup<br/>- Complete database dump<br/>- All model versions<br/>- Configuration snapshot]
        INCREMENTAL[Incremental Backup<br/>- WAL files<br/>- Changed models<br/>- Config diffs]
        SNAPSHOT[Point-in-time Snapshot<br/>- Volume snapshots<br/>- Container state<br/>- Memory dumps]
    end

    subgraph "Backup Storage"
        LOCAL_STORAGE[Local Storage<br/>- Immediate recovery<br/>- Fast access<br/>- Limited retention]
        CLOUD_STORAGE[Cloud Storage<br/>- Long-term retention<br/>- Geographic distribution<br/>- Cost-effective]
        OFFSITE[Offsite Backup<br/>- Disaster recovery<br/>- Air-gapped storage<br/>- Compliance archive]
    end

    %% Backup relationships
    TS_DB --> FULL_BACKUP
    TS_DB --> INCREMENTAL
    REDIS_MEM --> SNAPSHOT
    MODEL_FILES --> FULL_BACKUP
    CONFIG_FILES --> SNAPSHOT

    FULL_BACKUP --> LOCAL_STORAGE
    INCREMENTAL --> LOCAL_STORAGE
    SNAPSHOT --> LOCAL_STORAGE

    LOCAL_STORAGE --> CLOUD_STORAGE
    CLOUD_STORAGE --> OFFSITE
```

### 2. Recovery Procedures

| Scenario | Recovery Time Objective | Recovery Point Objective | Procedure |
|----------|------------------------|-------------------------|-----------|
| Container failure | < 5 minutes | 0 (real-time replication) | Docker restart, health check |
| Database corruption | < 30 minutes | < 1 hour | Restore from latest backup |
| Complete system failure | < 2 hours | < 4 hours | Full system restore from backups |
| Data center outage | < 4 hours | < 8 hours | Failover to backup region |
| Model corruption | < 15 minutes | 0 (versioned storage) | Rollback to previous model version |

## Scaling and Performance

### 1. Horizontal Scaling Architecture

```mermaid
graph TB
    subgraph "Load Balancer Layer"
        LB[Load Balancer<br/>- Round-robin<br/>- Health-based routing<br/>- Sticky sessions]
    end

    subgraph "Application Layer (Scalable)"
        DI1[Data Ingestion 1<br/>Primary provider]
        DI2[Data Ingestion 2<br/>Secondary provider]
        DI3[Data Ingestion 3<br/>Backup provider]
        
        NT1[Neural Trader 1<br/>AAPL, MSFT]
        NT2[Neural Trader 2<br/>GOOGL, AMZN]
        NT3[Neural Trader 3<br/>TSLA, NVDA]
    end

    subgraph "Data Layer (Clustered)"
        TS_PRIMARY[TimescaleDB Primary<br/>Write operations]
        TS_REPLICA1[TimescaleDB Replica 1<br/>Read operations]
        TS_REPLICA2[TimescaleDB Replica 2<br/>Analytics queries]
        
        REDIS_CLUSTER[Redis Cluster<br/>- Sharded data<br/>- High availability<br/>- Automatic failover]
    end

    %% Scaling connections
    LB --> DI1
    LB --> DI2
    LB --> DI3
    LB --> NT1
    LB --> NT2
    LB --> NT3

    DI1 --> TS_PRIMARY
    DI2 --> TS_PRIMARY
    DI3 --> TS_PRIMARY

    NT1 --> TS_REPLICA1
    NT2 --> TS_REPLICA2
    NT3 --> TS_REPLICA1

    TS_PRIMARY -.->|replication| TS_REPLICA1
    TS_PRIMARY -.->|replication| TS_REPLICA2

    DI1 --> REDIS_CLUSTER
    NT1 --> REDIS_CLUSTER
```

### 2. Performance Optimization

| Component | Optimization Strategy | Target Metric | Implementation |
|-----------|----------------------|---------------|----------------|
| Data Ingestion | Async processing, connection pooling | < 1s latency | WebSocket with asyncio |
| Neural Processing | Model caching, batch inference | < 500ms prediction | In-memory model loading |
| Database | Hypertables, continuous aggregates | < 100ms queries | TimescaleDB optimization |
| Cache | Redis clustering, read replicas | < 10ms cache hits | Redis Cluster with replicas |
| Network | Connection reuse, compression | < 50ms response time | HTTP/2, gzip compression |

---

*This deployment architecture documentation provides a comprehensive view of how the Neural Trader platform is deployed, secured, monitored, and scaled in production environments. The architecture is designed for high availability, fault tolerance, and horizontal scalability.*