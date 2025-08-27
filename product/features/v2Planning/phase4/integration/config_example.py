"""
Configuration Examples for EventBus Bridge Integration

This module provides example configurations for different deployment scenarios
and environments (development, staging, production).
"""

import os
from python_eventbus_bridge import EventBusConfig
from data_ingestion_integration import FeatureFlags, RedisConfig


# Development Configuration
def get_development_config():
    """Configuration for local development environment."""
    return {
        "eventbus_config": EventBusConfig(
            host="localhost",
            port=8080,
            max_retries=3,
            retry_backoff_base=2.0,
            retry_backoff_max=10.0,  # Shorter max for dev
            connection_timeout=5.0,
            request_timeout=15.0,  # Shorter timeout for dev
            max_connections=20,  # Fewer connections for dev
            circuit_breaker_threshold=3,  # Lower threshold for dev
            circuit_breaker_timeout=30.0,  # Shorter timeout for dev
            enable_metrics=True,
            metrics_prefix="dev_eventbus"
        ),
        "redis_config": RedisConfig(
            host="localhost",
            port=6379,
            db=0,
            max_connections=10,  # Fewer connections for dev
            socket_timeout=3.0,
            socket_connect_timeout=3.0
        ),
        "feature_flags": FeatureFlags(
            enable_eventbus=True,
            enable_dual_publish=True,  # Always dual publish in dev
            eventbus_percentage=1.0,  # 100% EventBus in dev
            enable_benchmarking=True,
            enable_detailed_logging=True,
            fallback_to_redis=True,
            max_eventbus_failures=2,  # Lower threshold for dev
            circuit_breaker_timeout=20.0  # Shorter timeout for dev
        )
    }


# Staging Configuration
def get_staging_config():
    """Configuration for staging environment."""
    return {
        "eventbus_config": EventBusConfig(
            host=os.getenv("EVENTBUS_HOST", "eventbus-staging"),
            port=int(os.getenv("EVENTBUS_PORT", "8080")),
            max_retries=5,
            retry_backoff_base=2.0,
            retry_backoff_max=30.0,
            connection_timeout=5.0,
            request_timeout=30.0,
            max_connections=50,
            circuit_breaker_threshold=5,
            circuit_breaker_timeout=60.0,
            enable_metrics=True,
            metrics_prefix="staging_eventbus"
        ),
        "redis_config": RedisConfig(
            host=os.getenv("REDIS_HOST", "redis-staging"),
            port=int(os.getenv("REDIS_PORT", "6379")),
            db=int(os.getenv("REDIS_DB", "0")),
            password=os.getenv("REDIS_PASSWORD"),
            max_connections=30
        ),
        "feature_flags": FeatureFlags(
            enable_eventbus=True,
            enable_dual_publish=True,
            eventbus_percentage=0.8,  # 80% EventBus in staging
            enable_benchmarking=True,
            enable_detailed_logging=False,  # Less verbose in staging
            fallback_to_redis=True,
            max_eventbus_failures=5,
            circuit_breaker_timeout=60.0
        )
    }


# Production Configuration
def get_production_config():
    """Configuration for production environment."""
    return {
        "eventbus_config": EventBusConfig(
            host=os.getenv("EVENTBUS_HOST", "eventbus-prod"),
            port=int(os.getenv("EVENTBUS_PORT", "8080")),
            max_retries=5,
            retry_backoff_base=2.0,
            retry_backoff_max=60.0,
            connection_timeout=5.0,
            request_timeout=30.0,
            max_connections=100,
            circuit_breaker_threshold=10,  # Higher threshold for prod
            circuit_breaker_timeout=300.0,  # Longer timeout for prod
            enable_metrics=True,
            metrics_prefix="prod_eventbus"
        ),
        "redis_config": RedisConfig(
            host=os.getenv("REDIS_HOST", "redis-prod"),
            port=int(os.getenv("REDIS_PORT", "6379")),
            db=int(os.getenv("REDIS_DB", "0")),
            password=os.getenv("REDIS_PASSWORD"),
            max_connections=100
        ),
        "feature_flags": FeatureFlags(
            enable_eventbus=os.getenv("ENABLE_EVENTBUS", "false").lower() == "true",
            enable_dual_publish=os.getenv("ENABLE_DUAL_PUBLISH", "false").lower() == "true",
            eventbus_percentage=float(os.getenv("EVENTBUS_PERCENTAGE", "0.1")),  # Start with 10%
            enable_benchmarking=True,
            enable_detailed_logging=False,  # Minimal logging in prod
            fallback_to_redis=True,
            max_eventbus_failures=10,  # Higher threshold for prod
            circuit_breaker_timeout=300.0  # Longer timeout for prod
        )
    }


# Migration Phases Configuration
def get_migration_phase_config(phase: int):
    """
    Get configuration for different migration phases.
    
    Phase 0: Redis only (baseline)
    Phase 1: EventBus 10% with dual publish
    Phase 2: EventBus 50% with dual publish
    Phase 3: EventBus 90% with dual publish
    Phase 4: EventBus only
    """
    base_config = get_production_config()
    
    phase_configs = {
        0: {  # Baseline - Redis only
            "enable_eventbus": False,
            "enable_dual_publish": False,
            "eventbus_percentage": 0.0
        },
        1: {  # Initial rollout
            "enable_eventbus": True,
            "enable_dual_publish": True,
            "eventbus_percentage": 0.1
        },
        2: {  # Partial rollout
            "enable_eventbus": True,
            "enable_dual_publish": True,
            "eventbus_percentage": 0.5
        },
        3: {  # Near complete rollout
            "enable_eventbus": True,
            "enable_dual_publish": True,
            "eventbus_percentage": 0.9
        },
        4: {  # Complete migration
            "enable_eventbus": True,
            "enable_dual_publish": False,
            "eventbus_percentage": 1.0
        }
    }
    
    if phase not in phase_configs:
        raise ValueError(f"Invalid migration phase: {phase}")
    
    phase_config = phase_configs[phase]
    base_config["feature_flags"].enable_eventbus = phase_config["enable_eventbus"]
    base_config["feature_flags"].enable_dual_publish = phase_config["enable_dual_publish"]
    base_config["feature_flags"].eventbus_percentage = phase_config["eventbus_percentage"]
    
    return base_config


# Environment Detection
def get_environment_config():
    """Automatically detect environment and return appropriate configuration."""
    env = os.getenv("ENVIRONMENT", "development").lower()
    
    if env == "production":
        return get_production_config()
    elif env == "staging":
        return get_staging_config()
    else:
        return get_development_config()


# Docker Configuration
def get_docker_config():
    """Configuration for Docker deployment."""
    return {
        "eventbus_config": EventBusConfig(
            host=os.getenv("EVENTBUS_HOST", "eventbus"),
            port=int(os.getenv("EVENTBUS_PORT", "8080")),
            max_retries=5,
            retry_backoff_base=2.0,
            retry_backoff_max=60.0,
            connection_timeout=10.0,  # Longer for container startup
            request_timeout=30.0,
            max_connections=50,
            circuit_breaker_threshold=5,
            circuit_breaker_timeout=120.0,
            enable_metrics=True,
            metrics_prefix="docker_eventbus"
        ),
        "redis_config": RedisConfig(
            host=os.getenv("REDIS_HOST", "redis"),
            port=int(os.getenv("REDIS_PORT", "6379")),
            db=int(os.getenv("REDIS_DB", "0")),
            password=os.getenv("REDIS_PASSWORD"),
            max_connections=50,
            socket_timeout=10.0,
            socket_connect_timeout=10.0
        ),
        "feature_flags": FeatureFlags.from_env()
    }


# Kubernetes Configuration
def get_kubernetes_config():
    """Configuration for Kubernetes deployment."""
    return {
        "eventbus_config": EventBusConfig(
            host=os.getenv("EVENTBUS_SERVICE_HOST", "eventbus-service"),
            port=int(os.getenv("EVENTBUS_SERVICE_PORT", "8080")),
            max_retries=5,
            retry_backoff_base=2.0,
            retry_backoff_max=60.0,
            connection_timeout=10.0,
            request_timeout=30.0,
            max_connections=100,
            circuit_breaker_threshold=10,
            circuit_breaker_timeout=180.0,
            enable_metrics=True,
            metrics_prefix="k8s_eventbus"
        ),
        "redis_config": RedisConfig(
            host=os.getenv("REDIS_SERVICE_HOST", "redis-service"),
            port=int(os.getenv("REDIS_SERVICE_PORT", "6379")),
            db=int(os.getenv("REDIS_DB", "0")),
            password=os.getenv("REDIS_PASSWORD"),
            max_connections=100
        ),
        "feature_flags": FeatureFlags.from_env()
    }


# High Throughput Configuration
def get_high_throughput_config():
    """Configuration optimized for high throughput scenarios."""
    return {
        "eventbus_config": EventBusConfig(
            host=os.getenv("EVENTBUS_HOST", "localhost"),
            port=int(os.getenv("EVENTBUS_PORT", "8080")),
            max_retries=3,  # Fewer retries for speed
            retry_backoff_base=1.5,  # Faster backoff
            retry_backoff_max=10.0,  # Lower max backoff
            connection_timeout=3.0,  # Shorter timeouts
            request_timeout=10.0,
            max_connections=200,  # More connections
            circuit_breaker_threshold=20,  # Higher threshold
            circuit_breaker_timeout=60.0,
            enable_metrics=True,
            metrics_prefix="htp_eventbus"
        ),
        "redis_config": RedisConfig(
            host=os.getenv("REDIS_HOST", "localhost"),
            port=int(os.getenv("REDIS_PORT", "6379")),
            db=int(os.getenv("REDIS_DB", "0")),
            password=os.getenv("REDIS_PASSWORD"),
            max_connections=200,  # More connections
            socket_timeout=2.0,  # Shorter timeouts
            socket_connect_timeout=2.0
        ),
        "feature_flags": FeatureFlags(
            enable_eventbus=True,
            enable_dual_publish=False,  # Single publish for speed
            eventbus_percentage=1.0,
            enable_benchmarking=True,
            enable_detailed_logging=False,  # Less logging for speed
            fallback_to_redis=False,  # No fallback for max speed
            max_eventbus_failures=50,  # Higher threshold
            circuit_breaker_timeout=30.0  # Shorter recovery time
        )
    }


# Example usage
if __name__ == "__main__":
    import json
    
    # Print example configurations
    configs = {
        "development": get_development_config(),
        "staging": get_staging_config(),
        "production": get_production_config(),
        "migration_phase_1": get_migration_phase_config(1),
        "docker": get_docker_config(),
        "kubernetes": get_kubernetes_config(),
        "high_throughput": get_high_throughput_config()
    }
    
    for name, config in configs.items():
        print(f"\n=== {name.upper()} CONFIGURATION ===")
        print(f"EventBus Host: {config['eventbus_config'].host}:{config['eventbus_config'].port}")
        print(f"Redis Host: {config['redis_config'].host}:{config['redis_config'].port}")
        print(f"EventBus Enabled: {config['feature_flags'].enable_eventbus}")
        print(f"Dual Publish: {config['feature_flags'].enable_dual_publish}")
        print(f"EventBus Percentage: {config['feature_flags'].eventbus_percentage*100}%")
        print(f"Max Connections: {config['eventbus_config'].max_connections}")
        print(f"Circuit Breaker Threshold: {config['eventbus_config'].circuit_breaker_threshold}")