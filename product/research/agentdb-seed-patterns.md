# AgentDB Seed Patterns — Consolidated

**Generated:** 2026-02-19
**Source:** Deduplicated from 98 raw patterns across 4 research files
**Final count:** 75 unique patterns (after merging overlaps)

---

## Pattern Index by Category

### Architecture — Core Platform (19)
| # | taskType | Name |
|---|----------|------|
| 1 | architecture:hexagonal-ports-adapters | Hexagonal Architecture |
| 2 | architecture:source-sink-traits | Source and Sink Trait Pattern |
| 3 | architecture:data-lake-pipeline | Bronze-Silver-Gold Data Lake |
| 4 | architecture:bronze-raw-json-schema | Bronze Raw JSON Envelope |
| 5 | architecture:parquet-sidecar-files | Per-Flush Sidecar Parquet Files |
| 6 | architecture:wal-ownership | WAL Owned by BronzeSubscriber |
| 7 | coding:bronze-accumulator | In-Memory Accumulator HashMap |
| 8 | architecture:multi-stream-tables | Multi-Stream Independent Tables |
| 9 | architecture:silver-gold-separation | Silver = Facts, Gold = Computed |
| 10 | architecture:silver-event-log-schema | Silver Event Log Schema |
| 11 | architecture:timescaledb-hypertable-schema | TimescaleDB Hypertable Standard |
| 12 | architecture:silver-etl-field-mapping | Silver ETL Field Mapping Config |
| 13 | architecture:event-bus-silver-subscriber | Unified Event Bus |
| 14 | architecture:silver-self-healing | Self-Healing Silver ETL |
| 15 | architecture:silver-etl-config-in-etcd | Silver ETL Config in etcd |
| 16 | architecture:dq-transparency | Data Quality Transparency |
| 17 | architecture:etl-run-statistics | ETL Run Statistics |
| 18 | architecture:data-dictionary | Data Dictionary Bronze+Silver |
| 19 | architecture:mcp-server-design | MCP Server + Storage Traits |

### Architecture — Gold Layer (7)
| 20 | architecture:gold-ddl-generation | Gold DDL Generation in Rust |
| 21 | architecture:gold-continuous-aggregates | Config-Driven Continuous Aggregates |
| 22 | architecture:gold-aligned-view | Cross-Stream Aligned View |
| 23 | architecture:gold-text-view | Gold Text View Pattern |
| 24 | architecture:gold-config-driven-views | Config-Driven Gold Views |
| 25 | architecture:unified-events | Unified Event Abstraction |
| 26 | architecture:threshold-crossings | Threshold Crossing Detection |

### Architecture — Intelligence/ML (11)
| 27 | architecture:intelligence-tiers | Tiered Intelligence (NN-SONA-LLM) |
| 28 | architecture:crate-boundary-features-vs-intelligence | Feature Engineering Crate Separation |
| 29 | architecture:ewma-normalization | EWMA Online Normalization |
| 30 | architecture:feature-vector-assembly | Feature Vector Assembly |
| 31 | architecture:online-mlp-ewc | Online MLP with EWC |
| 32 | architecture:knn-baseline | K-NN as Bootstrap/Baseline |
| 33 | architecture:sona-meta-learning | SONA Meta-Learning |
| 34 | architecture:text-embeddings-onnx | Text Embeddings via ONNX |
| 35 | architecture:composite-embedding | Composite Embedding (PCA) |
| 36 | architecture:granger-causality | Granger Causality Feature Mask |
| 37 | architecture:intelligence-cycle-trigger | PG NOTIFY Intelligence Cycle |

### Architecture — Config (6)
| 38 | architecture:stream-config-json-v2 | Config-Driven Stream Definition |
| 39 | architecture:config-schema-versioning | Config Schema Versioning |
| 40 | conventions:ndp-id-source-identity | ndp_id Stable Source Identity |
| 41 | conventions:config-directory-lifecycle | Config Directory Semantics |
| 42 | architecture:config-driven-lifecycle | Config-Driven Platform Lifecycle |
| 43 | architecture:hot-reload-sources | Hot-Reload for Sources |

### Architecture — Parsing/Ingestion (5)
| 44 | architecture:http-polling-config-driven | Generic HTTP Polling |
| 45 | coding:column-oriented-parser | Column-Oriented Parser (NWS) |
| 46 | architecture:mqtt-multi-subscription | MQTT Multi-Subscription |
| 47 | architecture:pre-transform-parser | Pre-Transform Parser |
| 48 | architecture:stream-type-classification | Stream Type Classification |

### Tooling and Operations (12)
| 49 | architecture:no-polars-in-bronze | arrow-rs for Bronze (No Polars) |
| 50 | architecture:pi-deployment-constraints | Pi 5 Deployment Constraints |
| 51 | architecture:ndp-lib-library-first | Library-First CLI (ndp-lib) |
| 52 | architecture:ndp-cli-entity-verb | ndp CLI Entity/Verb Structure |
| 53 | architecture:deploy-sh-rust-migration | Deploy.sh Bash-to-Rust Migration |
| 54 | architecture:two-layer-config-validation | Two-Layer Config Validation |
| 55 | architecture:database-bootstrap | Database Bootstrap Two-Layer |
| 56 | testing:integration-testbed | Integration Testbed Framework |
| 57 | architecture:validation-trust-pipeline | Validation + Trust Pipeline |
| 58 | architecture:grafana-silver-dashboards | Grafana Dashboard Patterns |
| 59 | procedure:silver-table-ddl-generation | Silver Table DDL Generation |
| 60 | procedure:dimension-tables | Dimension Tables Config and Load |

### Procedures (3)
| 61 | procedure:release-methodology | Release Methodology |
| 62 | procedure:declarative-deployment | Declarative Deployment |
| 63 | procedure:rollback | Rollback Procedure |

### Conventions (6)
| 64 | convention:swarm-coordination | Swarm Protocol |
| 65 | convention:agent-selection | Agent Routing and SPARC Planning |
| 66 | convention:pattern-workflow | Pattern Workflow (mandatory) |
| 67 | testing:conventions | Testing Conventions |
| 68 | testing:london-school-tdd | London School TDD for Domains |
| 69 | convention:tracking | Feature Tracking via GitHub Issues |

### Deprecated (6)
| 70 | deprecated:duckdb | DuckDB Eliminated |
| 71 | deprecated:polars-bronze | Polars in Bronze Write Path |
| 72 | deprecated:polars-streaming | Polars Streaming ETL |
| 73 | deprecated:response-parser-hardcoded | ResponseParser Trait |
| 74 | deprecated:parsers-in-ingestion | Parsers in HTTP Ingestion |
| 75 | deprecated:silver-etl-batch-app | Batch Silver ETL App |

---

## Deduplication Log

98 raw patterns reduced to 75 via these merges:
- DuckDB deprecated (air+dp+fe-gold-ops+procedures) -> single #70
- Polars streaming deprecated (air+dp+procedures) -> single #72
- Tiered intelligence (fe-gold-ops+procedures) -> #27
- Feature engineering separation (fe-gold-ops+procedures) -> #28
- EWMA normalization (fe-gold-ops+procedures) -> #29
- Declarative deployment (dp+procedures) -> #62 (includes execution order + declaration types)
- Release methodology (dp+procedures) -> #61 (includes artifacts + semver + checklist)
- Integration testbed (fe-gold-ops+procedures) -> #56 (includes assertions + feature testbed)
- Stream config format: air YAML superseded by dp JSON v2 -> #38
- Config source of truth: dp etcd merged into #38
- Two-layer validation (fe-gold-ops+dp) -> #54
- Stream type (fe-gold-ops+dp) -> #48
- Pi constraints (air+procedures) -> #50
- MCP server (dp server+traits) -> #19

## Source Research Files

- `pattern-seed-air.md` — 22 patterns from air-001 to air-018
- `pattern-seed-dp.md` — 28 patterns from dp-001 to dp-023
- `pattern-seed-fe-gold-ops.md` — 25 patterns from fe/gold/ops series
- `pattern-seed-procedures.md` — 23 patterns from cross-cutting docs
