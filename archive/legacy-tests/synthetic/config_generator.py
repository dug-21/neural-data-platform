#!/usr/bin/env python3
"""
Configuration Generator for Testing
Generates test configurations for different scenarios
"""

import json
import yaml
import random
from typing import Dict, Any, List
from pathlib import Path


class ConfigGenerator:
    """Generate test configurations for different services and environments"""
    
    def __init__(self):
        self.environments = ["dev", "test", "staging", "prod"]
        self.services = [
            "config-store",
            "data-ingestion", 
            "data-staging",
            "neural-ml-ops",
            "neural-trading"
        ]
        
    def generate_service_config(
        self,
        service: str,
        environment: str = "test"
    ) -> Dict[str, Any]:
        """Generate configuration for a specific service"""
        
        base_config = {
            "service": {
                "name": service,
                "version": "1.0.0",
                "environment": environment
            },
            "logging": {
                "level": "debug" if environment in ["dev", "test"] else "info",
                "format": "json"
            },
            "monitoring": {
                "metrics_enabled": True,
                "port": 9090 + self.services.index(service)
            }
        }
        
        # Service-specific configurations
        if service == "config-store":
            base_config.update({
                "git": {
                    "repo_url": "https://github.com/test/configs.git",
                    "branch": environment,
                    "sync_interval": 60 if environment == "dev" else 300
                },
                "grpc": {
                    "port": 50051,
                    "max_connections": 100
                },
                "redis": {
                    "url": "redis://localhost:6379",
                    "db": 0,
                    "cache_ttl": 3600
                }
            })
            
        elif service == "data-ingestion":
            base_config.update({
                "api": {
                    "port": 8081,
                    "rate_limit": 1000,
                    "timeout": 30
                },
                "sources": {
                    "polygon": {
                        "enabled": environment != "dev",
                        "api_key": "${POLYGON_API_KEY}"
                    },
                    "alpha_vantage": {
                        "enabled": False,
                        "api_key": "${ALPHA_VANTAGE_API_KEY}"
                    }
                },
                "redis": {
                    "url": "redis://localhost:6379",
                    "stream": "market_data_raw",
                    "max_len": 10000
                }
            })
            
        elif service == "data-staging":
            base_config.update({
                "grpc": {
                    "port": 50052,
                    "max_connections": 50
                },
                "processing": {
                    "batch_size": 100,
                    "window_size": 20,
                    "parallel_workers": 4
                },
                "features": {
                    "technical_indicators": ["sma", "ema", "rsi", "macd"],
                    "custom_features": True
                },
                "database": {
                    "url": "postgresql://postgres:postgres@localhost:5432/neural_trader",
                    "pool_size": 10
                }
            })
            
        elif service == "neural-ml-ops":
            base_config.update({
                "grpc": {
                    "port": 50053,
                    "max_connections": 20
                },
                "models": {
                    "path": "/models",
                    "auto_reload": True,
                    "cache_predictions": True
                },
                "training": {
                    "enabled": environment == "dev",
                    "batch_size": 32,
                    "epochs": 10,
                    "learning_rate": 0.001
                },
                "inference": {
                    "batch_size": 1,
                    "timeout": 5,
                    "cache_ttl": 60
                }
            })
            
        elif service == "neural-trading":
            base_config.update({
                "grpc": {
                    "port": 50054,
                    "max_connections": 10
                },
                "websocket": {
                    "port": 8080,
                    "enabled": True
                },
                "trading": {
                    "mode": "paper" if environment != "prod" else "live",
                    "max_position_size": 1000 if environment != "prod" else 10000,
                    "risk_limit": 100 if environment != "prod" else 1000
                },
                "strategies": {
                    "enabled": ["momentum", "mean_reversion", "ml_ensemble"],
                    "risk_management": True
                }
            })
        
        return base_config
    
    def generate_feature_flags(self, environment: str = "test") -> Dict[str, Any]:
        """Generate feature flags configuration"""
        
        flags = {
            "enable_ml_trading": {
                "enabled": environment in ["staging", "prod"],
                "rollout_percentage": 100 if environment == "prod" else 50,
                "description": "Enable ML-based trading signals"
            },
            "enable_paper_trading": {
                "enabled": environment != "prod",
                "rollout_percentage": 100,
                "description": "Enable paper trading mode"
            },
            "enable_risk_management": {
                "enabled": True,
                "rollout_percentage": 100,
                "description": "Enable risk management controls"
            },
            "enable_real_time_data": {
                "enabled": environment in ["staging", "prod"],
                "rollout_percentage": 100,
                "description": "Enable real-time market data"
            },
            "enable_advanced_features": {
                "enabled": environment == "dev",
                "rollout_percentage": 100,
                "description": "Enable experimental features"
            },
            "enable_performance_monitoring": {
                "enabled": True,
                "rollout_percentage": 100,
                "description": "Enable detailed performance monitoring"
            }
        }
        
        return flags
    
    def generate_test_matrix(self) -> List[Dict[str, Any]]:
        """Generate a matrix of test configurations"""
        
        matrix = []
        
        for env in ["dev", "test"]:
            for service in self.services:
                config = {
                    "name": f"{service}_{env}_test",
                    "service": service,
                    "environment": env,
                    "config": self.generate_service_config(service, env),
                    "feature_flags": self.generate_feature_flags(env),
                    "test_data": {
                        "use_synthetic": True,
                        "data_size": "small" if env == "dev" else "medium"
                    }
                }
                matrix.append(config)
        
        return matrix
    
    def generate_schema(self, service: str) -> Dict[str, Any]:
        """Generate JSON schema for service configuration"""
        
        schema = {
            "$schema": "http://json-schema.org/draft-07/schema#",
            "title": f"{service} Configuration Schema",
            "type": "object",
            "required": ["service", "logging"],
            "properties": {
                "service": {
                    "type": "object",
                    "required": ["name", "version"],
                    "properties": {
                        "name": {"type": "string"},
                        "version": {"type": "string", "pattern": "^\\d+\\.\\d+\\.\\d+$"},
                        "environment": {"type": "string", "enum": self.environments}
                    }
                },
                "logging": {
                    "type": "object",
                    "properties": {
                        "level": {"type": "string", "enum": ["trace", "debug", "info", "warn", "error"]},
                        "format": {"type": "string", "enum": ["json", "text"]}
                    }
                }
            }
        }
        
        # Add service-specific schema properties
        if service == "config-store":
            schema["properties"]["git"] = {
                "type": "object",
                "required": ["repo_url", "branch"],
                "properties": {
                    "repo_url": {"type": "string", "format": "uri"},
                    "branch": {"type": "string"},
                    "sync_interval": {"type": "integer", "minimum": 10}
                }
            }
        
        return schema
    
    def save_configs(self, output_dir: str = "/tmp/test-configs"):
        """Save all generated configurations to files"""
        
        output_path = Path(output_dir)
        output_path.mkdir(parents=True, exist_ok=True)
        
        # Generate and save service configs
        for env in self.environments:
            env_path = output_path / env
            env_path.mkdir(exist_ok=True)
            
            for service in self.services:
                service_path = env_path / service
                service_path.mkdir(exist_ok=True)
                
                # Generate configuration
                config = self.generate_service_config(service, env)
                
                # Save as YAML
                yaml_file = service_path / "config.yaml"
                with open(yaml_file, 'w') as f:
                    yaml.dump(config, f, default_flow_style=False)
                
                # Save as JSON
                json_file = service_path / "config.json"
                with open(json_file, 'w') as f:
                    json.dump(config, f, indent=2)
        
        # Save feature flags
        flags_file = output_path / "feature_flags.json"
        with open(flags_file, 'w') as f:
            json.dump(self.generate_feature_flags(), f, indent=2)
        
        # Save test matrix
        matrix_file = output_path / "test_matrix.json"
        with open(matrix_file, 'w') as f:
            json.dump(self.generate_test_matrix(), f, indent=2)
        
        # Save schemas
        schema_path = output_path / "schemas"
        schema_path.mkdir(exist_ok=True)
        
        for service in self.services:
            schema = self.generate_schema(service)
            schema_file = schema_path / f"{service}.schema.json"
            with open(schema_file, 'w') as f:
                json.dump(schema, f, indent=2)
        
        print(f"Configurations saved to {output_path}")
        
        return str(output_path)


def main():
    """Generate test configurations"""
    
    generator = ConfigGenerator()
    
    print("Generating test configurations...")
    
    # Generate and save all configs
    output_dir = generator.save_configs()
    
    print(f"\nGenerated configurations for:")
    print(f"  - {len(generator.services)} services")
    print(f"  - {len(generator.environments)} environments")
    print(f"\nFiles saved to: {output_dir}")
    
    # Generate sample config for display
    sample_config = generator.generate_service_config("config-store", "test")
    print("\nSample configuration (config-store/test):")
    print(json.dumps(sample_config, indent=2))


if __name__ == "__main__":
    main()