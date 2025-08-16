# MCP Versioning Strategy and Backward Compatibility

## Overview

This document defines the comprehensive versioning strategy for the Neural Trader MCP architecture, ensuring backward compatibility, smooth migrations, and independent evolution of system components while maintaining stability and reliability.

## Versioning Philosophy

### Core Principles

1. **Semantic Versioning**: All MCP contracts follow semantic versioning (SemVer) principles
2. **Backward Compatibility**: Maintain compatibility within major versions
3. **Graceful Degradation**: Systems continue operating with reduced functionality when versions mismatch
4. **Independent Evolution**: Layers can evolve independently without breaking other components
5. **Clear Migration Paths**: Well-defined upgrade and rollback procedures

## Versioning Scheme

### Version Format

```
v{MAJOR}.{MINOR}.{PATCH}[-{PRERELEASE}][+{BUILD}]
```

- **MAJOR**: Breaking changes, incompatible API changes
- **MINOR**: New features, backward compatible
- **PATCH**: Bug fixes, backward compatible
- **PRERELEASE**: alpha, beta, rc.1, etc.
- **BUILD**: Build metadata, commit hash

### Examples
```
v1.0.0          # Initial stable release
v1.1.0          # New features added
v1.1.1          # Bug fixes
v2.0.0          # Breaking changes
v2.1.0-beta.1   # Beta release with new features
v2.1.0+build.123 # Release with build metadata
```

## Component Versioning Matrix

### Layer-Specific Versioning

```json
{
  "version_matrix": {
    "data_ingestion": {
      "current_version": "v1.2.3",
      "supported_versions": ["v1.0.x", "v1.1.x", "v1.2.x"],
      "deprecated_versions": [],
      "sunset_date": null
    },
    "discovery": {
      "current_version": "v1.1.0",
      "supported_versions": ["v1.0.x", "v1.1.x"],
      "deprecated_versions": [],
      "sunset_date": null
    },
    "analysis": {
      "current_version": "v2.0.0",
      "supported_versions": ["v1.3.x", "v2.0.x"],
      "deprecated_versions": ["v1.0.x", "v1.1.x", "v1.2.x"],
      "sunset_date": "2024-12-31"
    },
    "execution": {
      "current_version": "v1.4.2",
      "supported_versions": ["v1.3.x", "v1.4.x"],
      "deprecated_versions": ["v1.0.x", "v1.1.x", "v1.2.x"],
      "sunset_date": "2024-09-30"
    },
    "storage": {
      "current_version": "v1.0.5",
      "supported_versions": ["v1.0.x"],
      "deprecated_versions": [],
      "sunset_date": null
    }
  }
}
```

### Compatibility Matrix

```json
{
  "compatibility_matrix": {
    "data_ingestion_v1.2.x": {
      "discovery_v1.0.x": "✅ Full compatibility",
      "discovery_v1.1.x": "✅ Full compatibility",
      "analysis_v1.3.x": "✅ Full compatibility",
      "analysis_v2.0.x": "⚠️ Limited compatibility (adapter required)",
      "execution_v1.4.x": "✅ Full compatibility",
      "storage_v1.0.x": "✅ Full compatibility"
    },
    "analysis_v2.0.x": {
      "data_ingestion_v1.0.x": "❌ Incompatible",
      "data_ingestion_v1.1.x": "⚠️ Limited compatibility",
      "data_ingestion_v1.2.x": "✅ Full compatibility",
      "discovery_v1.1.x": "✅ Full compatibility",
      "execution_v1.4.x": "✅ Full compatibility",
      "storage_v1.0.x": "✅ Full compatibility"
    }
  }
}
```

## Contract Evolution Patterns

### Additive Changes (Minor Version)

#### Adding New Optional Fields
```json
{
  "before_v1.1.0": {
    "event_type": "data.ingestion.stream.data_received",
    "payload_schema": {
      "type": "object",
      "properties": {
        "symbol": {"type": "string"},
        "data": {"type": "object"},
        "timestamp": {"type": "string"}
      },
      "required": ["symbol", "data", "timestamp"]
    }
  },
  "after_v1.2.0": {
    "event_type": "data.ingestion.stream.data_received",
    "payload_schema": {
      "type": "object",
      "properties": {
        "symbol": {"type": "string"},
        "data": {"type": "object"},
        "timestamp": {"type": "string"},
        "quality_score": {"type": "number"},
        "latency_ms": {"type": "number"}
      },
      "required": ["symbol", "data", "timestamp"]
    }
  },
  "compatibility": "✅ Backward compatible - old clients ignore new fields"
}
```

#### Adding New MCP Tools
```json
{
  "new_tool_v1.3.0": {
    "name": "data_ingestion_quality_metrics",
    "description": "Get data quality metrics for ingested streams",
    "version": "v1.3.0",
    "inputSchema": {
      "type": "object",
      "properties": {
        "symbol": {"type": "string"},
        "timeframe": {"type": "string", "default": "1h"}
      },
      "required": ["symbol"]
    }
  },
  "compatibility": "✅ Backward compatible - new functionality for clients that support it"
}
```

### Breaking Changes (Major Version)

#### Removing Required Fields
```json
{
  "breaking_change_v2.0.0": {
    "change_type": "remove_required_field",
    "field": "legacy_format",
    "migration_path": {
      "deprecation_notice": "v1.5.0",
      "removal_version": "v2.0.0",
      "adapter_available": true,
      "migration_guide": "/docs/migration/v1-to-v2.md"
    }
  }
}
```

#### Changing Field Types
```json
{
  "breaking_change_v2.0.0": {
    "change_type": "field_type_change",
    "field": "timestamp",
    "old_type": "string",
    "new_type": "integer",
    "reason": "Unix timestamp for better performance",
    "migration_path": {
      "converter_function": "convert_iso_to_unix",
      "backward_compatibility_period": "6_months"
    }
  }
}
```

## Version Negotiation

### Client-Server Negotiation

```json
{
  "version_negotiation": {
    "handshake_protocol": {
      "client_request": {
        "supported_versions": ["v1.2.0", "v1.1.0", "v1.0.0"],
        "preferred_version": "v1.2.0",
        "capabilities": ["compression", "batch_operations"]
      },
      "server_response": {
        "selected_version": "v1.2.0",
        "server_version": "v1.2.3",
        "features_enabled": ["compression", "batch_operations"],
        "deprecation_warnings": [
          {
            "feature": "legacy_auth",
            "sunset_date": "2024-12-31",
            "replacement": "oauth2_auth"
          }
        ]
      }
    }
  }
}
```

### Version Headers

```json
{
  "version_headers": {
    "request_headers": {
      "MCP-Version": "v1.2.0",
      "MCP-Client-Version": "neural-trader-client/1.2.3",
      "MCP-Capabilities": "compression,batch_operations,streaming"
    },
    "response_headers": {
      "MCP-Version": "v1.2.0",
      "MCP-Server-Version": "neural-trader-server/1.2.5",
      "MCP-Deprecation": "legacy_auth; sunset=\"2024-12-31\"",
      "MCP-Supported-Versions": "v1.0.0,v1.1.0,v1.2.0"
    }
  }
}
```

## Migration Strategies

### Blue-Green Deployment

```json
{
  "blue_green_deployment": {
    "strategy": "parallel_versions",
    "phases": [
      {
        "phase": "preparation",
        "duration": "1_week",
        "activities": [
          "deploy_new_version_alongside_old",
          "run_compatibility_tests",
          "train_operations_team"
        ]
      },
      {
        "phase": "gradual_migration",
        "duration": "2_weeks",
        "activities": [
          "route_5%_traffic_to_new_version",
          "monitor_metrics_and_errors",
          "gradually_increase_traffic"
        ]
      },
      {
        "phase": "full_migration",
        "duration": "1_week",
        "activities": [
          "route_100%_traffic_to_new_version",
          "monitor_system_stability",
          "prepare_rollback_if_needed"
        ]
      },
      {
        "phase": "cleanup",
        "duration": "1_week",
        "activities": [
          "decommission_old_version",
          "update_documentation",
          "conduct_post_migration_review"
        ]
      }
    ]
  }
}
```

### Canary Releases

```json
{
  "canary_release": {
    "strategy": "feature_flags",
    "rollout_plan": {
      "week_1": {
        "percentage": 1,
        "target_users": "internal_testing"
      },
      "week_2": {
        "percentage": 5,
        "target_users": "beta_customers"
      },
      "week_3": {
        "percentage": 25,
        "target_users": "early_adopters"
      },
      "week_4": {
        "percentage": 100,
        "target_users": "all_users"
      }
    },
    "rollback_criteria": {
      "error_rate_increase": "> 10%",
      "performance_degradation": "> 20%",
      "user_complaints": "> 5_per_day"
    }
  }
}
```

## Compatibility Adapters

### Protocol Adapters

```json
{
  "protocol_adapters": {
    "v1_to_v2_adapter": {
      "source_version": "v1.x.x",
      "target_version": "v2.x.x",
      "transformations": [
        {
          "field": "timestamp",
          "transformation": "iso_string_to_unix_timestamp"
        },
        {
          "field": "price_data.currency",
          "transformation": "add_default_usd"
        },
        {
          "event": "data.received",
          "transformation": "split_to_data.ingestion.received"
        }
      ],
      "performance_impact": {
        "latency_overhead": "< 5ms",
        "memory_overhead": "< 10MB",
        "cpu_overhead": "< 5%"
      }
    }
  }
}
```

### Schema Evolution Adapters

```python
# Example adapter implementation
class SchemaAdapter:
    def __init__(self, source_version: str, target_version: str):
        self.source_version = source_version
        self.target_version = target_version
        self.transformations = self._load_transformations()
    
    def transform_request(self, request: dict) -> dict:
        """Transform request from source to target schema"""
        transformed = request.copy()
        
        for transformation in self.transformations:
            transformed = self._apply_transformation(transformed, transformation)
        
        return transformed
    
    def transform_response(self, response: dict) -> dict:
        """Transform response from target back to source schema"""
        # Reverse transformations for backward compatibility
        pass
```

## Deprecation Policy

### Deprecation Timeline

```json
{
  "deprecation_policy": {
    "deprecation_notice_period": "6_months",
    "support_period_after_deprecation": "12_months",
    "total_lifecycle": "18_months",
    "notification_channels": [
      "api_headers",
      "documentation",
      "email_notifications",
      "slack_alerts"
    ]
  }
}
```

### Deprecation Stages

```json
{
  "deprecation_stages": {
    "stage_1_announcement": {
      "timeline": "T-18_months",
      "actions": [
        "announce_deprecation_plan",
        "publish_migration_guide",
        "add_deprecation_headers"
      ]
    },
    "stage_2_warning": {
      "timeline": "T-12_months",
      "actions": [
        "add_warning_logs",
        "update_documentation",
        "start_client_outreach"
      ]
    },
    "stage_3_limited_support": {
      "timeline": "T-6_months",
      "actions": [
        "stop_new_feature_development",
        "security_fixes_only",
        "increased_migration_support"
      ]
    },
    "stage_4_sunset": {
      "timeline": "T-0",
      "actions": [
        "disable_deprecated_endpoints",
        "return_410_gone_status",
        "provide_migration_assistance"
      ]
    }
  }
}
```

## Version Discovery

### Service Registry

```json
{
  "service_registry": {
    "data_ingestion_service": {
      "endpoint": "https://api.neural-trader.com/ingestion",
      "versions": [
        {
          "version": "v1.2.3",
          "status": "stable",
          "endpoints": ["/stream", "/health", "/metrics"],
          "schema_url": "https://schemas.neural-trader.com/ingestion/v1.2.3"
        },
        {
          "version": "v1.1.5",
          "status": "deprecated",
          "sunset_date": "2024-09-30",
          "endpoints": ["/stream", "/health"]
        }
      ]
    }
  }
}
```

### Schema Registry

```json
{
  "schema_registry": {
    "base_url": "https://schemas.neural-trader.com",
    "schemas": [
      {
        "id": "data_ingestion_stream_event",
        "version": "v1.2.0",
        "url": "/schemas/data-ingestion/stream-event/v1.2.0.json",
        "checksum": "sha256:abc123...",
        "compatible_versions": ["v1.0.0", "v1.1.0"]
      }
    ],
    "validation": {
      "enabled": true,
      "strict_mode": false,
      "unknown_fields": "ignore"
    }
  }
}
```

## Testing Strategy

### Compatibility Testing

```json
{
  "compatibility_testing": {
    "matrix_testing": {
      "client_versions": ["v1.0.0", "v1.1.0", "v1.2.0"],
      "server_versions": ["v1.0.0", "v1.1.0", "v1.2.0", "v2.0.0"],
      "test_scenarios": [
        "basic_functionality",
        "error_handling",
        "performance_benchmarks",
        "feature_availability"
      ]
    },
    "automated_tests": {
      "contract_tests": "every_build",
      "integration_tests": "nightly",
      "performance_tests": "weekly",
      "compatibility_tests": "on_version_change"
    }
  }
}
```

### Contract Testing

```yaml
# Example contract test specification
contract_tests:
  - name: "data_ingestion_stream_subscription"
    provider: "data_ingestion_service"
    consumer: "analysis_service"
    scenarios:
      - description: "Subscribe to AAPL stream"
        request:
          version: "v1.2.0"
          method: "subscribe_stream"
          params:
            symbol: "AAPL"
            data_types: ["quotes", "trades"]
        response:
          status: "success"
          subscription_id: "string"
          stream_endpoint: "string"
        compatibility:
          v1.0.0: "supported"
          v1.1.0: "supported"
          v1.2.0: "supported"
          v2.0.0: "requires_adapter"
```

## Monitoring and Metrics

### Version Usage Metrics

```json
{
  "version_metrics": {
    "client_version_distribution": {
      "v1.0.0": 5,
      "v1.1.0": 15,
      "v1.2.0": 75,
      "v2.0.0": 5
    },
    "api_version_usage": {
      "v1.0.0": {
        "requests_per_day": 1000,
        "error_rate": 0.02,
        "performance_p95": 150
      },
      "v1.2.0": {
        "requests_per_day": 50000,
        "error_rate": 0.005,
        "performance_p95": 75
      }
    },
    "migration_progress": {
      "clients_migrated": 85,
      "clients_remaining": 15,
      "migration_completion_rate": 0.85
    }
  }
}
```

### Version Health Monitoring

```json
{
  "version_health": {
    "alerts": [
      {
        "condition": "deprecated_version_usage > 0.1",
        "severity": "warning",
        "action": "notify_migration_team"
      },
      {
        "condition": "unsupported_version_requests > 0",
        "severity": "error",
        "action": "investigate_client_issue"
      },
      {
        "condition": "adapter_error_rate > 0.05",
        "severity": "critical",
        "action": "rollback_or_fix_adapter"
      }
    ]
  }
}
```

## Documentation Strategy

### Version-Specific Documentation

```json
{
  "documentation_strategy": {
    "structure": {
      "current_version": "/docs/v1.2/",
      "previous_versions": [
        "/docs/v1.1/",
        "/docs/v1.0/"
      ],
      "migration_guides": [
        "/docs/migration/v1.0-to-v1.1.md",
        "/docs/migration/v1.1-to-v1.2.md",
        "/docs/migration/v1.x-to-v2.0.md"
      ]
    },
    "maintenance": {
      "current_version": "full_updates",
      "previous_major": "security_fixes_only",
      "deprecated_versions": "read_only"
    }
  }
}
```

### Migration Guides

```markdown
# Migration Guide: v1.x to v2.0

## Breaking Changes

1. **Timestamp Format Change**
   - **Before**: ISO 8601 string format
   - **After**: Unix timestamp (integer)
   - **Migration**: Use provided conversion utility

2. **Authentication Method**
   - **Before**: API key in header
   - **After**: OAuth 2.0 bearer token
   - **Migration**: Update to OAuth flow

## Step-by-Step Migration

1. Update client libraries to v2.0 compatible versions
2. Implement OAuth 2.0 authentication
3. Update timestamp handling code
4. Test thoroughly in staging environment
5. Deploy with gradual rollout
```

This comprehensive versioning strategy ensures:
1. **Predictable Evolution**: Clear rules for when and how versions change
2. **Smooth Migrations**: Well-defined migration paths and timelines
3. **Backward Compatibility**: Support for older versions during transition periods
4. **Quality Assurance**: Comprehensive testing of version compatibility
5. **Operational Excellence**: Monitoring and metrics to track migration progress