# Data Platform Component Inventory
**Neural Data Platform - Silver Platform Research**
*Research Date: 2025-12-23*

## Executive Summary

This document provides a comprehensive inventory of components that can be independently built for the Neural Data Platform's evolution beyond the bronze layer (Parquet storage). The analysis is based on modern data platform architectures, microservices patterns, and best practices from industry leaders.

The platform follows a **modular, microservices-based architecture** where each component can be developed, deployed, and scaled independently while maintaining loose coupling through well-defined interfaces.

---

## 1. Architectural Foundation

### Modern Data Platform Layers

Based on [Modern Data Platform Architecture (Domo)](https://www.domo.com/learn/article/how-to-architect-a-modern-data-platform-in-2025), data platforms consist of five core layers:

1. **Data Ingestion Layer** - EXISTING (Bronze Layer with 5 streams)
2. **Data Storage Layer** - EXISTING (Parquet files)
3. **Data Processing/Transformation Layer** - TO BUILD (Silver Layer)
4. **Analytics & Intelligence Layer** - TO BUILD (Gold Layer)
5. **Governance Layer** - TO BUILD (Cross-cutting)

### Microservices Design Principles

Following [microservices patterns](https://microservices.io/patterns/microservices.html), the platform should adopt:

- **Database per Service**: Each component owns its data store ([microservices database pattern](https://microservices.io/patterns/data/database-per-service.html))
- **API Gateway**: Single entry point for client access
- **Event-Driven Architecture**: Loose coupling through events ([EDA patterns](https://microservices.io/patterns/data/event-driven-architecture.html))
- **Circuit Breaker**: Fault isolation to prevent cascading failures
- **CQRS**: Separate read/write models for optimization

---

## 2. Component Inventory

### 2.1 Silver Layer - Transformation Engine

**Purpose**: Transform raw bronze data into cleaned, structured, queryable datasets.

**Responsibilities**:
- ETL/ELT processing from Parquet to structured storage
- Data cleaning, validation, and normalization
- Schema enforcement and evolution
- Incremental processing and change data capture
- Time-series aggregations and rollups

**Dependencies**:
- Bronze Layer (Parquet files) - READ ONLY
- Metadata Catalog - for schema registry
- Orchestration - for job scheduling

**Technology Options**:

| Option | Pros | Cons | Build vs Buy |
|--------|------|------|--------------|
| **TimescaleDB** | Native time-series support, PostgreSQL compatibility, continuous aggregates | Single database limitation | Build (OSS) |
| **Apache Spark** | Massive scale, distributed processing | High operational overhead | Build (OSS) |
| **dbt** | SQL-based, excellent lineage, testing framework | Requires warehouse | Build (OSS) |
| **Custom Rust ETL** | Performance, control, existing codebase fit | Development time | Build |

**Recommendation**: **TimescaleDB + Custom Rust ETL**
- Rust for high-performance Parquet reading
- TimescaleDB for structured storage with time-series optimizations
- Use continuous aggregates for real-time rollups

**References**:
- [Data Platform Architecture (Acceldata)](https://www.acceldata.io/article/what-is-a-data-platform-architecture)
- [Modern Data Platform (Folio3)](https://data.folio3.com/blog/modern-data-platform/)

---

### 2.2 Gold Layer - Feature Store

**Purpose**: Serve pre-computed ML features with low latency for training and inference.

**Responsibilities**:
- Feature computation from silver layer
- Dual storage: offline (historical) + online (real-time)
- Feature versioning and lineage tracking
- Point-in-time correctness for training
- Feature serving API for inference

**Dependencies**:
- Silver Layer - feature source data
- Metadata Catalog - feature registry
- ML Training Pipeline - feature consumption

**Architecture** ([Feature Store 101](https://aerospike.com/blog/feature-store/)):

```
┌─────────────────┐
│ Feature Pipeline│
│   (Compute)     │
└────────┬────────┘
         │
    ┌────▼─────┐
    │ Registry │◄──── Feature definitions, versions, lineage
    └────┬─────┘
         │
    ┌────▼────────────────────┐
    │                         │
┌───▼────┐            ┌──────▼────┐
│Offline │            │  Online   │
│ Store  │            │  Store    │
│(S3/DW) │            │(Redis/DDB)│
└───┬────┘            └──────┬────┘
    │                        │
    │                        │
┌───▼────┐            ┌──────▼────┐
│Training│            │ Inference │
│Pipeline│            │  Service  │
└────────┘            └───────────┘
```

**Technology Options**:

| Option | Pros | Cons | Build vs Buy |
|--------|------|------|--------------|
| **Feast** | Minimal, flexible, OSS | Limited feature engineering | Buy (OSS) |
| **Tecton** | Production-ready, managed | Commercial, vendor lock-in | Buy (SaaS) |
| **Custom** | Full control, NDP integration | High development effort | Build |
| **Hopsworks** | Full-featured OSS | Complex deployment | Buy (OSS) |

**Recommendation**: **Custom Feature Store on TimescaleDB + Redis**
- Offline store: TimescaleDB (already in silver layer)
- Online store: Redis for low-latency serving
- Feature registry: PostgreSQL table with versioning
- Tight integration with existing NDP architecture

**References**:
- [What is a Feature Store (Hopsworks)](https://www.hopsworks.ai/dictionary/feature-store)
- [Feature Store Architecture (Qwak)](https://www.qwak.com/post/feature-store-architecture)
- [Building ML Systems with Feature Stores (Neptune)](https://neptune.ai/blog/building-ml-systems-with-feature-store)

---

### 2.3 Data Quality Engine

**Purpose**: Validate, profile, and monitor data quality across all layers.

**Responsibilities**:
- Schema validation and constraint checking
- Data profiling and statistical analysis
- Anomaly detection and alerting
- Data quality metrics and SLAs
- Automated test generation from profiles

**Dependencies**:
- All data layers (Bronze, Silver, Gold)
- Metadata Catalog - quality metadata storage
- Orchestration - scheduled quality checks

**Technology Options**:

| Tool | Language | Spark Native | Auto-Profiling | Best For |
|------|----------|--------------|----------------|----------|
| **Great Expectations** | Python | Partial | Yes | Python workflows, flexibility |
| **Deequ** | Scala/Python | Yes | Yes | Large-scale Spark workloads |
| **Soda** | Python | No | Yes | SQL-based checks, simplicity |
| **dbt Tests** | SQL | No | No | Warehouse-centric, simple tests |

**Comparison** ([Great Expectations vs Deequ vs Soda](https://branchboston.com/great-expectations-vs-deequ-vs-soda-data-quality-testing-tools-compared/)):

- **Great Expectations**: Declarative framework, excellent for Python-based pipelines, strong documentation generation
- **Deequ**: Built on Spark, optimized for distributed computation, automatic constraint suggestion
- **Soda**: SQL-first, easy to learn, good for warehouse-centric architectures

**Recommendation**: **Great Expectations + Custom Rust Validators**
- Great Expectations for Python-based validation logic
- Custom Rust validators for high-performance Parquet validation
- Integration with existing NDP testing framework
- Store validation results in metadata catalog

**References**:
- [Great Expectations](https://greatexpectations.io/)
- [Deequ and Great Expectations Comparison (Medium)](https://medium.com/@sriramjcet/deequ-and-great-expectations-data-quality-assessment-frameworks-for-modern-data-platforms-on-c6df718dba25)
- [Data Quality Frameworks Comparison](https://nurbolsakenov.com/data-quality-frameworks-comparison/)

---

### 2.4 Metadata Catalog & Data Discovery

**Purpose**: Centralized repository for all metadata, enabling data discovery and governance.

**Responsibilities**:
- Store descriptive, administrative, and structural metadata
- Data lineage tracking (source → bronze → silver → gold)
- Search and discovery interface
- Schema registry and versioning
- Tag-based organization and filtering

**Dependencies**:
- All components (metadata producer)
- Independent storage (no dependencies)

**Architecture Types** ([Metadata Architecture](https://www.cockroachlabs.com/blog/metadata-reference-architecture/)):

1. **Centralized**: Single metadata database
   - Pros: Simple, consistent
   - Cons: Single point of failure, scalability limits

2. **Federated**: Multiple systems with shared framework
   - Pros: Domain ownership, flexibility
   - Cons: Synchronization complexity

3. **Distributed**: Fully independent systems
   - Pros: No coordination needed
   - Cons: Inconsistency risk, difficult discovery

**Technology Options**:

| Tool | Origin | Best For | Key Features |
|------|--------|----------|--------------|
| **OpenMetadata** | OSS | All-in-one solution | 100+ connectors, data quality, governance |
| **DataHub** | LinkedIn | Strong community, flexible | Real-time updates, rich lineage |
| **Amundsen** | Lyft | Search-first discovery | Simple setup, productivity focus |
| **Apache Atlas** | Hortonworks | Hadoop ecosystems | Classification, governance |
| **Marquez** | WeWork | Lineage visualization | dbt/Airflow integration |

**Comparison** ([Open Source Data Catalog Comparison](https://atlan.com/open-source-data-catalog-tools/)):

- **OpenMetadata**: All-in-one platform with the most features, 100+ integrations
- **DataHub**: Strong LinkedIn backing, excellent community, good for LinkedIn-style workflows
- **Amundsen**: Simple, focused on discovery and search
- **Apache Atlas**: Best for Hadoop/Hive-heavy environments
- **Marquez**: Focused on lineage, good for Airflow users

**Recommendation**: **OpenMetadata**
- Most comprehensive feature set
- 100+ data service connectors
- Built-in data quality and lineage
- Active community and frequent releases
- Can start with core features and expand

**References**:
- [OpenMetadata](https://open-metadata.org/)
- [Top 7 Open-Source Data Catalogs (OvalEdge)](https://www.ovaledge.com/blog/ai-powered-open-source-data-catalogs)
- [Metadata Management Architecture (Astera)](https://www.astera.com/type/blog/introduction-to-metadata-architecture/)

---

### 2.5 Orchestration & Scheduling

**Purpose**: Coordinate execution of data pipelines, manage dependencies, and handle scheduling.

**Responsibilities**:
- DAG-based workflow definition
- Dependency management
- Job scheduling (cron, event-driven)
- Retry logic and error handling
- Resource allocation and scaling
- Monitoring and alerting

**Dependencies**:
- All pipeline components (orchestration target)
- Metadata Catalog - for lineage tracking

**Technology Options**:

| Tool | Release | Philosophy | Best For |
|------|---------|------------|----------|
| **Apache Airflow** | 2014 | Task-centric DAGs | Industry standard, vast ecosystem |
| **Dagster** | 2019 | Asset-centric (SDA) | Data-first teams, dbt users |
| **Prefect** | 2018 | Dynamic workflows | Cloud-native, error handling |
| **Mage** | 2022 | Notebook-style | Data scientists, rapid prototyping |

**Detailed Comparison** ([Airflow vs Dagster vs Prefect](https://risingwave.com/blog/airflow-vs-dagster-vs-prefect-a-detailed-comparison/)):

**Apache Airflow**:
- Pros: De facto standard, massive provider ecosystem, proven at scale
- Cons: Operational overhead, dated UI (improving in v3), steep learning curve
- When to use: Established teams, need for extensive integrations, proven track record

**Dagster**:
- Pros: Software-defined assets, excellent dbt integration, developer experience
- Cons: More opinionated, steeper initial setup
- When to use: Data-first teams, heavy dbt usage, focus on lineage and testing

**Prefect**:
- Pros: Pure Python, minimal boilerplate, excellent error handling, event-driven
- Cons: Smaller ecosystem, less mature
- When to use: Cloud-native deployments, dynamic workflows, rapid iteration

**Recommendation**: **Dagster**
- Asset-centric model aligns with NDP's data layers (Bronze/Silver/Gold as assets)
- Excellent for time-series data and continuous computation
- Strong developer experience with testing and local development
- Good fit for Rust-based pipelines (can wrap Rust binaries as ops)
- Software-defined assets provide clear lineage

**Alternative**: **Airflow** if extensive provider integrations are needed

**References**:
- [Dagster vs Airflow (Dagster)](https://dagster.io/blog/dagster-airflow)
- [Airflow vs Dagster vs Prefect (RisingWave)](https://risingwave.com/blog/airflow-vs-dagster-vs-prefect-a-detailed-comparison/)
- [Decoding Data Orchestration Tools (FreeAgent)](https://engineering.freeagent.com/2025/05/29/decoding-data-orchestration-tools-comparing-prefect-dagster-airflow-and-mage/)

---

### 2.6 Monitoring & Observability

**Purpose**: Track system health, performance, and data pipeline execution.

**Responsibilities**:
- Pipeline execution monitoring
- System metrics (CPU, memory, disk)
- Data metrics (volume, latency, quality)
- Alerting and incident management
- Distributed tracing for debugging
- Log aggregation and analysis

**Dependencies**:
- All components (monitoring target)
- Independent alerting service

**Architecture** ([Modern Data Platform](https://www.matillion.com/learn/blog/modern-data-platform)):

```
┌──────────────────────────────────────┐
│         Observability Layer          │
├──────────────────────────────────────┤
│                                      │
│  ┌────────────┐  ┌───────────────┐ │
│  │  Metrics   │  │     Logs      │ │
│  │(Prometheus)│  │ (Loki/Elastic)│ │
│  └─────┬──────┘  └───────┬───────┘ │
│        │                 │          │
│  ┌─────▼─────────────────▼──────┐  │
│  │      Visualization Layer     │  │
│  │         (Grafana)            │  │
│  └──────────────────────────────┘  │
│                                      │
│  ┌──────────────────────────────┐  │
│  │    Alerting (AlertManager)   │  │
│  └──────────────────────────────┘  │
└──────────────────────────────────────┘
```

**Technology Stack**:

| Component | Tool | Purpose |
|-----------|------|---------|
| **Metrics** | Prometheus | Time-series metrics, scraping, PromQL |
| **Logs** | Loki or ELK | Log aggregation, search, analysis |
| **Traces** | Jaeger/Tempo | Distributed tracing, request flows |
| **Visualization** | Grafana | Dashboards, alerting, exploration |
| **Alerting** | AlertManager | Alert routing, grouping, silencing |

**Recommendation**: **Prometheus + Loki + Grafana Stack**
- Prometheus for metrics (already has Rust client libraries)
- Loki for log aggregation (lightweight, integrates with Grafana)
- Grafana for visualization (already planned in NDP roadmap)
- AlertManager for alerting
- All OSS, proven stack, excellent Rust support

**References**:
- [Scalable Data Platform Architecture (Acceldata)](https://www.acceldata.io/blog/designing-a-future-ready-data-platform-architecture)
- [DQOps Data Platform Architecture](https://dqops.com/data-platform-architecture/)

---

### 2.7 Query Engine & API Layer

**Purpose**: Provide unified query interface across all data layers.

**Responsibilities**:
- SQL query processing across federated sources
- REST/GraphQL API for programmatic access
- Query optimization and caching
- Access control and rate limiting
- Response formatting and serialization

**Dependencies**:
- Silver Layer (primary query target)
- Gold Layer (feature serving)
- Metadata Catalog (schema information)

**Architecture Patterns**:

1. **Query Federation**: Single query interface, multiple backends
2. **API Gateway**: Unified entry point with routing
3. **GraphQL Federation**: Schema stitching across domains

**Technology Options**:

| Option | Type | Best For |
|--------|------|----------|
| **Trino/Presto** | Distributed SQL | Querying across multiple sources |
| **GraphQL Server** | API | Flexible client queries |
| **PostgREST** | REST | Auto-generate REST from PostgreSQL |
| **Custom Rust API** | Custom | Full control, performance |

**Recommendation**: **Custom Rust REST API + PostgREST**
- Custom Rust API for high-performance feature serving
- PostgREST for quick TimescaleDB access during development
- GraphQL layer if complex client requirements emerge
- API Gateway (e.g., Kong, Traefik) for routing and rate limiting

**References**:
- [API Gateway Pattern (Microservices.io)](https://microservices.io/patterns/apigateway.html)
- [Modern Data Architecture (Aezion)](https://www.aezion.com/blogs/modern-data-architecture/)

---

### 2.8 Stream Processing Engine (Real-Time Layer)

**Purpose**: Process data in real-time for low-latency use cases.

**Responsibilities**:
- Windowed aggregations on streaming data
- Real-time feature computation
- Event pattern detection
- Stateful stream processing
- Exactly-once semantics

**Dependencies**:
- Bronze Layer (stream input)
- Gold Layer (feature output)
- Orchestration (deployment)

**Technology Options**:

| Tool | Language | Maturity | Best For |
|------|----------|----------|----------|
| **Apache Flink** | Java/Scala | Very High | Complex stateful processing |
| **Apache Spark Streaming** | Scala/Python | High | Micro-batch processing |
| **Kafka Streams** | Java | High | Kafka-centric architectures |
| **Custom Rust** | Rust | N/A | Control, performance |

**Recommendation**: **Defer or Custom Rust**
- Current NDP ingestion already handles real-time via channels
- If complex windowing needed: Consider Flink
- For simple aggregations: Extend existing Rust ingestion coordinator
- Evaluate necessity based on actual latency requirements

**References**:
- [Event-Driven Architecture Patterns (ByteByteGo)](https://blog.bytebytego.com/p/event-driven-architectural-patterns)
- [Event-Driven Architecture (Microsoft)](https://learn.microsoft.com/en-us/azure/architecture/guide/architecture-styles/event-driven)

---

### 2.9 ML Training & Inference Platform

**Purpose**: Train models, serve predictions, manage model lifecycle.

**Responsibilities**:
- Model training orchestration
- Hyperparameter tuning
- Model versioning and registry
- Model serving and inference
- A/B testing and experimentation
- Model monitoring and drift detection

**Dependencies**:
- Gold Layer (feature store)
- Orchestration (training jobs)
- Metadata Catalog (model registry)

**Architecture**:

```
┌──────────────────────────────────────┐
│        ML Platform                   │
├──────────────────────────────────────┤
│                                      │
│  ┌────────────────────────────────┐ │
│  │   Training Pipeline           │ │
│  │  - Experiment tracking        │ │
│  │  - Hyperparameter tuning      │ │
│  │  - Model versioning           │ │
│  └────────────┬───────────────────┘ │
│               │                      │
│  ┌────────────▼───────────────────┐ │
│  │      Model Registry           │ │
│  │  - Version management         │ │
│  │  - Metadata & lineage         │ │
│  └────────────┬───────────────────┘ │
│               │                      │
│  ┌────────────▼───────────────────┐ │
│  │   Inference Service           │ │
│  │  - Online serving             │ │
│  │  - Batch predictions          │ │
│  │  - A/B testing                │ │
│  └────────────────────────────────┘ │
└──────────────────────────────────────┘
```

**Technology Options**:

| Component | Tool | Purpose |
|-----------|------|---------|
| **Training** | MLflow, Kubeflow | Experiment tracking, orchestration |
| **Registry** | MLflow Model Registry | Model versioning, staging |
| **Serving** | Seldon, KServe, BentoML | Model serving, scaling |
| **Monitoring** | Evidently, WhyLabs | Drift detection, performance |

**Recommendation**: **MLflow + Custom Rust Serving**
- MLflow for experiment tracking and model registry
- Custom Rust inference service for high-performance serving
- Integration with existing ruv-FANN neural network library
- Deploy models as Rust binaries for minimal latency

**References**:
- [Building ML Systems with Feature Stores (Neptune)](https://neptune.ai/blog/building-ml-systems-with-feature-store)
- [Feature Store For ML](https://www.featurestore.org/)

---

### 2.10 Data Governance & Security

**Purpose**: Enforce data policies, access control, privacy, and compliance.

**Responsibilities**:
- Role-based access control (RBAC)
- Data masking and anonymization
- Audit logging
- Policy enforcement
- Compliance reporting (GDPR, CCPA)
- Data retention and lifecycle management

**Dependencies**:
- All components (governance enforcement)
- Metadata Catalog (policy storage)

**Architecture** ([Data Platform Governance](https://learn.microsoft.com/en-us/azure/cloud-adoption-framework/scenarios/cloud-scale-analytics/govern-metadata-standards)):

```
┌──────────────────────────────────────┐
│      Governance Layer                │
├──────────────────────────────────────┤
│                                      │
│  ┌────────────┐  ┌────────────────┐ │
│  │   Policy   │  │    Access      │ │
│  │  Registry  │  │   Control      │ │
│  └─────┬──────┘  └────────┬───────┘ │
│        │                  │          │
│  ┌─────▼──────────────────▼──────┐  │
│  │   Enforcement Engine          │  │
│  │  - RBAC/ABAC                  │  │
│  │  - Data masking               │  │
│  │  - Encryption                 │  │
│  └───────────────────────────────┘  │
│                                      │
│  ┌───────────────────────────────┐  │
│  │    Audit & Compliance         │  │
│  │  - Activity logging           │  │
│  │  - Compliance reports         │  │
│  └───────────────────────────────┘  │
└──────────────────────────────────────┘
```

**Technology Options**:

| Tool | Focus | Best For |
|------|-------|----------|
| **Apache Ranger** | Fine-grained access control | Hadoop ecosystems |
| **OPA (Open Policy Agent)** | Policy as code | Kubernetes, microservices |
| **Custom RBAC** | Application-specific | NDP-specific requirements |

**Recommendation**: **Custom RBAC + OPA**
- Custom RBAC in Rust for NDP-specific policies
- OPA for declarative policy definition
- Integrate with existing authentication system
- Store policies in Metadata Catalog

**References**:
- [Metadata Standards (Microsoft)](https://learn.microsoft.com/en-us/azure/cloud-adoption-framework/scenarios/cloud-scale-analytics/govern-metadata-standards)
- [Active Metadata Management](https://www.informatica.com/products/informatica-platform/metadata-management.html)

---

## 3. Component Dependency Matrix

| Component | Depends On | Provides To |
|-----------|------------|-------------|
| **Bronze Layer** | - | Silver, Quality, Catalog |
| **Silver Layer** | Bronze, Catalog, Orchestration | Gold, Query, Quality |
| **Gold Layer** | Silver, Catalog | ML, API, Query |
| **Quality Engine** | All layers, Catalog | Orchestration, Monitoring |
| **Metadata Catalog** | - | All components |
| **Orchestration** | All pipelines | Monitoring |
| **Monitoring** | All components | Alerting |
| **Query Engine** | Silver, Gold, Catalog | API Layer, Clients |
| **API Layer** | Query Engine, Gold | External consumers |
| **Stream Processing** | Bronze | Gold, Silver |
| **ML Platform** | Gold, Orchestration | API, Monitoring |
| **Governance** | - | All components |

---

## 4. Build vs Buy Analysis

### Recommended Build (Custom Development)

1. **Silver Layer ETL** - Core differentiation, Rust expertise
2. **Gold Layer Feature Store** - Tight integration with ML pipeline
3. **API Layer** - NDP-specific requirements
4. **Governance** - Custom security needs
5. **Quality Validators (Rust)** - Performance-critical, Parquet-specific

### Recommended Buy/Adopt (Open Source)

1. **Metadata Catalog** - OpenMetadata (comprehensive OSS solution)
2. **Orchestration** - Dagster (asset-centric fits NDP model)
3. **Monitoring** - Prometheus/Grafana (standard, proven stack)
4. **Query Engine** - TimescaleDB + PostgREST (PostgreSQL-based)
5. **Data Quality** - Great Expectations (flexible, Python-based)
6. **ML Tracking** - MLflow (industry standard)

### Defer/Evaluate Later

1. **Stream Processing** - Current ingestion may suffice
2. **Advanced ML Serving** - Start simple, evaluate Seldon/KServe if needed
3. **Data Governance Tools** - Build RBAC first, add tooling as needed

---

## 5. Implementation Phases

### Phase 1: Foundation (Silver Layer)
- Silver Layer ETL (Parquet → TimescaleDB)
- Metadata Catalog (OpenMetadata)
- Basic Orchestration (Dagster)
- Monitoring (Prometheus + Grafana)

### Phase 2: Quality & Discovery
- Data Quality Engine (Great Expectations)
- Enhanced Metadata (lineage, profiling)
- API Layer (Rust REST)
- Basic Governance (RBAC)

### Phase 3: ML Platform
- Gold Layer Feature Store
- ML Training Pipeline
- Model Registry (MLflow)
- Inference Service

### Phase 4: Advanced Capabilities
- Stream Processing (if needed)
- Advanced Governance
- A/B Testing Platform
- Cost Optimization

---

## 6. Key Design Principles

### 1. Modularity ([Modern Data Architecture](https://www.aezion.com/blogs/modern-data-architecture/))
> "The future favors modularity over monolithic systems. Companies are building modern data platforms from interoperable, open-source components."

- Each component should be independently deployable
- Use well-defined interfaces (APIs, events)
- Avoid tight coupling between components

### 2. Event-Driven Architecture ([EDA Patterns](https://solace.com/event-driven-architecture-patterns/))
> "Event-driven systems leverage eventual consistency, employing techniques like event sourcing."

- Components communicate via events
- Asynchronous processing for scalability
- Loose coupling through pub/sub patterns

### 3. Database per Service ([Microservices Pattern](https://microservices.io/patterns/data/database-per-service.html))
> "Each microservice owns its database, ensuring loose coupling and high cohesion."

- Silver Layer: TimescaleDB
- Metadata: PostgreSQL
- Cache: Redis
- Feature Store: TimescaleDB (offline) + Redis (online)

### 4. Data Lakehouse Architecture
> "The data lakehouse concept emerged, combining the flexibility of data lakes with the structure of warehouses."

- Bronze: Data lake (Parquet)
- Silver: Structured warehouse (TimescaleDB)
- Gold: Feature store (specialized)

### 5. Composable Architecture
> "Composable systems allow teams to swap components without breaking core functionality."

- Standard interfaces between components
- Plugin architecture where appropriate
- Technology agnostic where possible

---

## 7. Technology Stack Summary

| Layer | Component | Technology | Rationale |
|-------|-----------|------------|-----------|
| **Bronze** | Storage | Parquet | EXISTING - lightweight, columnar |
| **Silver** | Database | TimescaleDB | Time-series optimized PostgreSQL |
| **Silver** | ETL | Rust | Performance, existing expertise |
| **Gold** | Feature Store | Custom (TimescaleDB + Redis) | Tight integration, control |
| **Catalog** | Metadata | OpenMetadata | Comprehensive OSS, 100+ connectors |
| **Orchestration** | Workflow | Dagster | Asset-centric, developer experience |
| **Quality** | Validation | Great Expectations + Rust | Flexibility + performance |
| **Monitoring** | Metrics | Prometheus | Standard, Rust support |
| **Monitoring** | Logs | Loki | Lightweight, Grafana integration |
| **Monitoring** | Viz | Grafana | Industry standard, rich features |
| **API** | REST | Rust (Axum/Actix) | Performance, type safety |
| **Query** | SQL | TimescaleDB + PostgREST | PostgreSQL-based |
| **ML** | Tracking | MLflow | Experiment tracking, registry |
| **ML** | Serving | Custom Rust | Low latency, ruv-FANN integration |
| **Governance** | RBAC | Custom Rust + OPA | Security, policy as code |

---

## 8. Source References

### Data Platform Architecture
- [Modern Data Platform Architecture (Domo)](https://www.domo.com/learn/article/how-to-architect-a-modern-data-platform-in-2025)
- [Modern Data Platform (Folio3)](https://data.folio3.com/blog/modern-data-platform/)
- [Complete Guide to Modern Data Platform (Matillion)](https://www.matillion.com/learn/blog/modern-data-platform)
- [Data Platform Architecture (Acceldata)](https://www.acceldata.io/article/what-is-a-data-platform-architecture)
- [What is Data Platform Architecture (DQOps)](https://dqops.com/data-platform-architecture/)

### Microservices Patterns
- [Microservices Pattern (Microservices.io)](https://microservices.io/patterns/microservices.html)
- [Database per Service Pattern](https://microservices.io/patterns/data/database-per-service.html)
- [5 Essential Microservices Design Patterns (OsoHQ)](https://www.osohq.com/learn/microservices-design-patterns)
- [Top 10 Microservices Design Patterns (Codefresh)](https://codefresh.io/learn/microservices/top-10-microservices-design-patterns-and-how-to-choose/)

### Metadata Management
- [OpenMetadata](https://open-metadata.org/)
- [Metadata Management Quick Guide (CockroachDB)](https://www.cockroachlabs.com/blog/metadata-reference-architecture/)
- [Introduction to Metadata Architecture (Astera)](https://www.astera.com/type/blog/introduction-to-metadata-architecture/)
- [Metadata-Driven Architecture Guide (Medium)](https://medium.com/@er.shrivastav/metadata-driven-architecture-a-comprehensive-guide-to-metadata-driven-architecture-39f04c5107ad)

### Data Catalog Tools
- [Open Source Data Catalog 2025 (Atlan)](https://atlan.com/open-source-data-catalog-tools/)
- [Top 7 Open-Source Data Catalogs (OvalEdge)](https://www.ovaledge.com/blog/ai-powered-open-source-data-catalogs)
- [Awesome Data Catalogs (GitHub)](https://github.com/opendatadiscovery/awesome-data-catalogs)
- [Best Data Catalog Tools 2025 (Hevo)](https://hevodata.com/learn/data-catalog-tools/)

### Data Orchestration
- [Dagster vs Airflow (Dagster)](https://dagster.io/blog/dagster-airflow)
- [Airflow vs Dagster vs Prefect (RisingWave)](https://risingwave.com/blog/airflow-vs-dagster-vs-prefect-a-detailed-comparison/)
- [Decoding Data Orchestration Tools (FreeAgent)](https://engineering.freeagent.com/2025/05/29/decoding-data-orchestration-tools-comparing-prefect-dagster-airflow-and-mage/)
- [Dagster vs Airflow (DataCamp)](https://www.datacamp.com/blog/dagster-vs-airflow)

### Event-Driven Architecture
- [Event-Driven Architecture Pattern (Microservices.io)](https://microservices.io/patterns/data/event-driven-architecture.html)
- [Event-Driven Architectural Patterns (ByteByteGo)](https://blog.bytebytego.com/p/event-driven-architectural-patterns)
- [Ultimate Guide to EDA Patterns (Solace)](https://solace.com/event-driven-architecture-patterns/)
- [Event-Driven Architecture (Microsoft)](https://learn.microsoft.com/en-us/azure/architecture/guide/architecture-styles/event-driven)

### Feature Store
- [Feature Store 101 (Aerospike)](https://aerospike.com/blog/feature-store/)
- [Feature Store For ML](https://www.featurestore.org/)
- [Building ML Systems with Feature Store (Neptune)](https://neptune.ai/blog/building-ml-systems-with-feature-store)
- [What is a Feature Store (Hopsworks)](https://www.hopsworks.ai/dictionary/feature-store)
- [Feature Store Architecture (Qwak)](https://www.qwak.com/post/feature-store-architecture)

### Data Quality
- [Great Expectations](https://greatexpectations.io/)
- [Deequ and Great Expectations (Medium)](https://medium.com/@sriramjcet/deequ-and-great-expectations-data-quality-assessment-frameworks-for-modern-data-platforms-on-c6df718dba25)
- [Data Quality Frameworks Comparison](https://nurbolsakenov.com/data-quality-frameworks-comparison/)
- [Great Expectations vs Deequ vs Soda (Branch Boston)](https://branchboston.com/great-expectations-vs-deequ-vs-soda-data-quality-testing-tools-compared/)

---

## 9. Next Steps

1. **Review Component Inventory**: Validate components with NDP team
2. **Prioritize Components**: Determine build order based on dependencies
3. **Create Technical ADRs**: Document key technology decisions
4. **Design Component Interfaces**: Define APIs and event schemas
5. **Prototype Silver Layer**: Build MVP with Rust + TimescaleDB
6. **Evaluate Orchestration**: Set up Dagster POC
7. **Deploy Metadata Catalog**: Install and configure OpenMetadata

---

*Document created by: component-mapper analyst*
*Research completed: 2025-12-23*
*Total sources referenced: 50+*
