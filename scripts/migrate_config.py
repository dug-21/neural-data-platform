#!/usr/bin/env python3
"""
Config Store Migration Script for Neural Trader
Migrates existing configuration from environment variables and files to config-store
"""

import json
import os
import sys
import asyncio
import logging
import yaml
import toml
from pathlib import Path
from typing import Dict, Any, List, Optional
from datetime import datetime
import redis
import argparse

logging.basicConfig(level=logging.INFO, format='%(asctime)s - %(levelname)s - %(message)s')
logger = logging.getLogger(__name__)

class ConfigMigrator:
    """Handles migration of configuration from various sources to config-store"""
    
    def __init__(self, redis_url: str = "redis://localhost:6379", dry_run: bool = False):
        self.redis_url = redis_url
        self.dry_run = dry_run
        self.redis_client = None
        self.migration_report = {
            'timestamp': datetime.utcnow().isoformat(),
            'migrated_configs': {},
            'skipped_configs': {},
            'errors': []
        }
        
    def connect_redis(self):
        """Connect to Redis for config storage"""
        try:
            self.redis_client = redis.from_url(self.redis_url, decode_responses=True)
            self.redis_client.ping()
            logger.info(f"Connected to Redis at {self.redis_url}")
        except Exception as e:
            logger.error(f"Failed to connect to Redis: {e}")
            sys.exit(1)
    
    def load_env_config(self) -> Dict[str, Any]:
        """Load configuration from environment variables"""
        logger.info("Loading configuration from environment variables")
        
        # Separate secrets from regular config
        config = {}
        secrets = {}
        
        # Define what should be treated as secrets
        secret_patterns = [
            'PASSWORD', 'SECRET', 'KEY', 'TOKEN', 'PASS',
            'CLIENT_ID', 'CLIENT_SECRET', 'PRIVATE'
        ]
        
        for key, value in os.environ.items():
            if key.startswith(('NEURAL_TRADER_', 'ALPACA_', 'POLYGON_', 'FINNHUB_', 
                             'REDDIT_', 'ALPHA_VANTAGE_', 'IEX_', 'QUANDL_', 'FRED_', 
                             'NASDAQ_', 'NEWSAPI_', 'POSTGRES_', 'GRAFANA_')):
                
                # Check if it's a secret
                is_secret = any(pattern in key.upper() for pattern in secret_patterns)
                
                if is_secret:
                    secrets[key] = value
                    logger.info(f"Identified secret: {key}")
                else:
                    config[key] = self._convert_value(value)
                    logger.info(f"Loaded config: {key}")
        
        return config, secrets
    
    def load_file_configs(self) -> Dict[str, Any]:
        """Load configuration from existing config files"""
        logger.info("Loading configuration from files")
        
        config_files = [
            'config/sector_models.toml',
            'config/autonomous_training.toml', 
            'config/data_requirements.toml',
            'config/v2-platform',
            '.env.example'
        ]
        
        configs = {}
        
        for config_file in config_files:
            file_path = Path(config_file)
            if file_path.exists():
                try:
                    configs[config_file] = self._load_file_content(file_path)
                    logger.info(f"Loaded config from {config_file}")
                except Exception as e:
                    logger.error(f"Failed to load {config_file}: {e}")
        
        return configs
    
    def _load_file_content(self, file_path: Path) -> Any:
        """Load content from various file formats"""
        suffix = file_path.suffix.lower()
        
        with open(file_path, 'r') as f:
            if suffix in ['.json']:
                return json.load(f)
            elif suffix in ['.yaml', '.yml']:
                return yaml.safe_load(f)
            elif suffix in ['.toml']:
                return toml.load(f)
            else:
                return f.read()
    
    def _convert_value(self, value: str) -> Any:
        """Convert string values to appropriate types"""
        if value.lower() in ('true', 'false'):
            return value.lower() == 'true'
        
        try:
            if '.' in value:
                return float(value)
            else:
                return int(value)
        except ValueError:
            return value
    
    def create_namespace_structure(self) -> Dict[str, Dict[str, Any]]:
        """Create hierarchical namespace structure as defined in specification"""
        logger.info("Creating namespace structure")
        
        namespaces = {
            # Neural Platform Shared Configurations
            'neural-platform/shared/eventbus': {
                'connection': 'redis://redis:6379',
                'consumer_groups': [
                    'data-ingestion-group',
                    'model-execution-group', 
                    'action-execution-group'
                ],
                'streams': {
                    'market_data': 'trading:market-data',
                    'system_events': 'trading:system',
                    'ml_events': 'trading:ml-events'
                },
                'dead_letter_queue': 'trading:dlq',
                'message_ttl_seconds': 86400
            },
            
            'neural-platform/shared/ml-ops': {
                'model_registry': '/opt/models',
                'training_schedule': '0 2 * * *',
                'performance_thresholds': {
                    'accuracy': 0.85,
                    'latency_ms': 100,
                    'throughput_rps': 1000
                },
                'auto_retrain_threshold': 0.8,
                'model_versioning': True,
                'backup_retention_days': 30
            },
            
            'neural-platform/shared/monitoring': {
                'prometheus_url': 'http://prometheus:9090',
                'grafana_url': 'http://grafana:3000',
                'log_level': 'info',
                'metrics_retention_days': 90,
                'alert_manager_url': 'http://alertmanager:9093',
                'jaeger_url': 'http://jaeger:14268'
            },
            
            # Neural Trading Domain Configurations
            'neural-trading/data-ingestion': {
                'sources': {
                    'primary': {
                        'provider': 'alpaca',
                        'api_url': '${ALPACA_API_URL}',
                        'websocket_url': '${ALPACA_WS_URL}',
                        'symbols': ['AAPL', 'GOOGL', 'MSFT', 'AMZN', 'NVDA'],
                        'rate_limits': {
                            'requests_per_minute': 200,
                            'websocket_connections': 5,
                            'burst_limit': 50
                        },
                        'retry_policy': {
                            'max_attempts': 3,
                            'backoff_multiplier': 2.0,
                            'initial_delay_ms': 1000,
                            'max_delay_ms': 30000
                        }
                    },
                    'fallback': {
                        'provider': 'polygon',
                        'api_url': '${POLYGON_API_URL}',
                        'websocket_url': '${POLYGON_WS_URL}',
                        'symbols': ['AAPL', 'GOOGL', 'MSFT', 'AMZN', 'NVDA']
                    }
                },
                'validation': {
                    'price_range': {
                        'min_price': 0.01,
                        'max_price': 10000.0
                    },
                    'timestamp_tolerance_ms': 300000,
                    'required_fields': ['symbol', 'price', 'timestamp', 'volume'],
                    'data_quality_threshold': 0.95
                },
                'processing': {
                    'batch_size': 1000,
                    'flush_interval_ms': 5000,
                    'enable_compression': True,
                    'parallelism': 4
                }
            },
            
            'neural-trading/model-execution': {
                'models': {
                    'trading_mlp': {
                        'input_size': 20,
                        'hidden_layers': [64, 32, 16],
                        'output_size': 3,
                        'learning_rate': 0.001,
                        'batch_size': 32,
                        'dropout_rate': 0.2,
                        'activation': 'relu'
                    },
                    'lstm_predictor': {
                        'sequence_length': 60,
                        'hidden_size': 128,
                        'num_layers': 2,
                        'learning_rate': 0.0001,
                        'batch_size': 64
                    }
                },
                'inference': {
                    'max_concurrent_requests': 100,
                    'timeout_ms': 5000,
                    'enable_gpu': True,
                    'model_cache_size': 10
                },
                'training': {
                    'enable_autonomous': False,
                    'sample_threshold': 1000,
                    'validation_split': 0.2,
                    'early_stopping_patience': 10
                }
            },
            
            'neural-trading/action-layer': {
                'risk_controls': {
                    'max_position_size': 0.05,
                    'max_daily_loss': 0.02,
                    'stop_loss_percentage': 0.05,
                    'max_drawdown': 0.15,
                    'position_concentration_limit': 0.25
                },
                'execution': {
                    'order_timeout_seconds': 30,
                    'retry_attempts': 3,
                    'slippage_tolerance': 0.001,
                    'enable_smart_routing': True
                },
                'portfolio': {
                    'rebalancing_frequency': 'daily',
                    'min_cash_reserve': 0.1,
                    'max_sectors_exposure': 0.4
                }
            }
        }
        
        return namespaces
    
    def migrate_to_config_store(self, namespaces: Dict[str, Dict[str, Any]]):
        """Migrate configuration to Redis-based config store"""
        logger.info("Starting migration to config-store")
        
        if not self.redis_client:
            self.connect_redis()
        
        for namespace, config in namespaces.items():
            try:
                config_key = f"config::{namespace}"
                config_json = json.dumps(config, indent=2)
                
                if not self.dry_run:
                    # Store configuration
                    self.redis_client.hset(config_key, mapping={
                        'data': config_json,
                        'version': '1.0.0',
                        'created_at': datetime.utcnow().isoformat(),
                        'updated_at': datetime.utcnow().isoformat(),
                        'schema_version': 'v1'
                    })
                    
                    # Set expiration for cache (optional)
                    self.redis_client.expire(config_key, 86400 * 7)  # 7 days
                
                self.migration_report['migrated_configs'][namespace] = len(str(config_json))
                logger.info(f"Migrated namespace: {namespace} ({len(str(config_json))} bytes)")
                
            except Exception as e:
                error_msg = f"Failed to migrate namespace {namespace}: {e}"
                logger.error(error_msg)
                self.migration_report['errors'].append(error_msg)
    
    def create_seed_data(self, output_file: str):
        """Create seed data JSON file for initial population"""
        logger.info(f"Creating seed data file: {output_file}")
        
        env_config, secrets = self.load_env_config()
        file_configs = self.load_file_configs()
        namespaces = self.create_namespace_structure()
        
        seed_data = {
            'version': '1.0.0',
            'created_at': datetime.utcnow().isoformat(),
            'environment_configs': env_config,
            'file_configs': file_configs,
            'namespaces': namespaces,
            'secrets_found': list(secrets.keys()),  # Don't include actual secrets
            'migration_instructions': {
                'secrets': 'Secrets should be provided via environment variables',
                'variable_substitution': 'Variables in ${VAR} format will be replaced at runtime',
                'namespace_access': 'Use ConfigStore::get_namespace() to access configurations'
            }
        }
        
        os.makedirs(os.path.dirname(output_file), exist_ok=True)
        with open(output_file, 'w') as f:
            json.dump(seed_data, f, indent=2, sort_keys=True)
        
        logger.info(f"Seed data written to {output_file}")
    
    def validate_migration(self) -> bool:
        """Validate that migration was successful"""
        logger.info("Validating migration")
        
        if not self.redis_client:
            return False
        
        try:
            namespaces = self.create_namespace_structure()
            for namespace in namespaces.keys():
                config_key = f"config::{namespace}"
                if not self.redis_client.hexists(config_key, 'data'):
                    logger.error(f"Missing configuration for namespace: {namespace}")
                    return False
            
            logger.info("Migration validation successful")
            return True
            
        except Exception as e:
            logger.error(f"Migration validation failed: {e}")
            return False
    
    def generate_migration_report(self, output_file: str):
        """Generate migration report"""
        self.migration_report['validation_passed'] = self.validate_migration()
        self.migration_report['total_namespaces'] = len(self.migration_report['migrated_configs'])
        
        os.makedirs(os.path.dirname(output_file), exist_ok=True)
        with open(output_file, 'w') as f:
            json.dump(self.migration_report, f, indent=2, sort_keys=True)
        
        logger.info(f"Migration report written to {output_file}")
    
    async def run_migration(self, seed_file: str, report_file: str):
        """Run complete migration process"""
        logger.info("Starting configuration migration")
        
        try:
            # Create seed data
            self.create_seed_data(seed_file)
            
            # Run actual migration if not dry run
            if not self.dry_run:
                namespaces = self.create_namespace_structure()
                self.migrate_to_config_store(namespaces)
                
                # Validate migration
                if self.validate_migration():
                    logger.info("Migration completed successfully")
                else:
                    logger.error("Migration validation failed")
                    return False
            else:
                logger.info("Dry run completed - no data was migrated")
            
            # Generate report
            self.generate_migration_report(report_file)
            return True
            
        except Exception as e:
            logger.error(f"Migration failed: {e}")
            self.migration_report['errors'].append(str(e))
            return False

def main():
    parser = argparse.ArgumentParser(description='Neural Trader Config Store Migration')
    parser.add_argument('--redis-url', default='redis://localhost:6379',
                      help='Redis connection URL')
    parser.add_argument('--dry-run', action='store_true',
                      help='Run without making changes')
    parser.add_argument('--seed-file', default='/workspaces/neural-trader/config/config_store_seed.json',
                      help='Output file for seed data')
    parser.add_argument('--report-file', default='/workspaces/neural-trader/scripts/migration_report.json',
                      help='Output file for migration report')
    
    args = parser.parse_args()
    
    migrator = ConfigMigrator(redis_url=args.redis_url, dry_run=args.dry_run)
    
    success = asyncio.run(migrator.run_migration(args.seed_file, args.report_file))
    
    if success:
        print(f"\nMigration completed successfully!")
        print(f"Seed data: {args.seed_file}")
        print(f"Report: {args.report_file}")
        
        if not args.dry_run:
            print(f"\nConfiguration store populated at: {args.redis_url}")
            print("Services can now use ConfigStore to access configuration")
    else:
        print("\nMigration failed. Check logs for details.")
        sys.exit(1)

if __name__ == '__main__':
    main()