#!/usr/bin/env python3
"""
Backfill CLI - Command-line interface for data backfill operations.

This module provides a comprehensive CLI for managing historical data imports
from various sources including Polygon S3 and local files.
"""

import os
import sys
import json
import asyncio
import argparse
import logging
from pathlib import Path
from datetime import datetime
from typing import List, Optional, Dict, Any

# Add parent directory to path for imports
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from providers.file_provider import FileProvider
from utils.file_backfill import FileBackfillHandler
from utils.logging import get_logger
from utils.metrics import metrics
from storage.timescale import TimescaleDB
from storage.redis_store import RedisStore

logger = get_logger(__name__)


class BackfillCLI:
    """Main CLI handler for backfill operations."""
    
    def __init__(self):
        """Initialize the CLI."""
        self.parser = self._create_parser()
        self.config = self._load_config()
        
    def _create_parser(self) -> argparse.ArgumentParser:
        """Create the argument parser with all commands."""
        parser = argparse.ArgumentParser(
            prog='backfill',
            description='Neural Trader Data Backfill CLI',
            formatter_class=argparse.RawDescriptionHelpFormatter,
            epilog="""
Examples:
  # Download from S3
  %(prog)s s3 --profile polygon-s3 --symbols AAPL,MSFT --start-date 2023-01-01 --end-date 2023-12-31
  
  # Import from files
  %(prog)s file --path /mnt/data --format csv --recursive
  
  # Check status
  %(prog)s status
  
  # Resume operation
  %(prog)s resume --operation-id op_123abc
            """
        )
        
        # Global options
        parser.add_argument(
            '--config',
            help='Configuration file path',
            default=os.environ.get('NEURAL_TRADER_CONFIG', '~/.neural_trader/backfill.yaml')
        )
        parser.add_argument(
            '--log-level',
            choices=['DEBUG', 'INFO', 'WARNING', 'ERROR'],
            default='INFO',
            help='Logging level'
        )
        parser.add_argument(
            '--metrics-port',
            type=int,
            help='Port for Prometheus metrics'
        )
        
        # Create subparsers for commands
        subparsers = parser.add_subparsers(dest='command', help='Available commands')
        
        # S3 command
        self._add_s3_parser(subparsers)
        
        # File command
        self._add_file_parser(subparsers)
        
        # Status command
        self._add_status_parser(subparsers)
        
        # Resume command
        self._add_resume_parser(subparsers)
        
        # Validate command
        self._add_validate_parser(subparsers)
        
        # Diagnose command
        self._add_diagnose_parser(subparsers)
        
        return parser
    
    def _add_s3_parser(self, subparsers):
        """Add S3 download command parser."""
        parser = subparsers.add_parser('s3', help='Download data from Polygon S3')
        
        parser.add_argument(
            '--profile',
            required=True,
            help='AWS profile name'
        )
        parser.add_argument(
            '--symbols',
            required=True,
            help='Comma-separated list of symbols'
        )
        parser.add_argument(
            '--start-date',
            required=True,
            type=lambda s: datetime.strptime(s, '%Y-%m-%d'),
            help='Start date (YYYY-MM-DD)'
        )
        parser.add_argument(
            '--end-date',
            required=True,
            type=lambda s: datetime.strptime(s, '%Y-%m-%d'),
            help='End date (YYYY-MM-DD)'
        )
        parser.add_argument(
            '--data-type',
            default='day_aggs_v1',
            choices=['day_aggs_v1', 'trades_v1', 'quotes_v1'],
            help='Type of data to download'
        )
        parser.add_argument(
            '--destination',
            default='/mnt/data',
            help='Local storage path'
        )
        parser.add_argument(
            '--max-workers',
            type=int,
            default=10,
            help='Number of parallel download workers'
        )
        parser.add_argument(
            '--batch-size',
            type=int,
            default=10000,
            help='Records per batch'
        )
        parser.add_argument(
            '--checkpoint',
            action='store_true',
            default=True,
            help='Enable checkpointing'
        )
        parser.add_argument(
            '--dry-run',
            action='store_true',
            help='Preview without downloading'
        )
    
    def _add_file_parser(self, subparsers):
        """Add file import command parser."""
        parser = subparsers.add_parser('file', help='Import data from local files')
        
        parser.add_argument(
            '--path',
            required=True,
            type=Path,
            help='File or directory path'
        )
        parser.add_argument(
            '--format',
            default='csv',
            choices=['csv', 'json', 'parquet'],
            help='File format'
        )
        parser.add_argument(
            '--symbols',
            help='Filter by symbols (comma-separated)'
        )
        parser.add_argument(
            '--symbols-file',
            help='File containing symbols (one per line)'
        )
        parser.add_argument(
            '--start-date',
            type=lambda s: datetime.strptime(s, '%Y-%m-%d'),
            help='Start date filter'
        )
        parser.add_argument(
            '--end-date',
            type=lambda s: datetime.strptime(s, '%Y-%m-%d'),
            help='End date filter'
        )
        parser.add_argument(
            '--recursive',
            action='store_true',
            help='Search subdirectories'
        )
        parser.add_argument(
            '--pattern',
            help='File name pattern'
        )
        parser.add_argument(
            '--batch-size',
            type=int,
            default=10000,
            help='Records per batch'
        )
        parser.add_argument(
            '--workers',
            type=int,
            default=5,
            help='Number of parallel workers'
        )
        parser.add_argument(
            '--checkpoint',
            action='store_true',
            default=True,
            help='Enable checkpointing'
        )
        parser.add_argument(
            '--dry-run',
            action='store_true',
            help='Preview without importing'
        )
        parser.add_argument(
            '--skip-errors',
            action='store_true',
            help='Skip files with errors'
        )
    
    def _add_status_parser(self, subparsers):
        """Add status command parser."""
        parser = subparsers.add_parser('status', help='Check backfill operation status')
        
        parser.add_argument(
            '--operation-id',
            help='Specific operation ID'
        )
        parser.add_argument(
            '--detailed',
            action='store_true',
            help='Show detailed progress'
        )
        parser.add_argument(
            '--format',
            choices=['table', 'json'],
            default='table',
            help='Output format'
        )
        parser.add_argument(
            '--show-checkpoints',
            action='store_true',
            help='Show available checkpoints'
        )
    
    def _add_resume_parser(self, subparsers):
        """Add resume command parser."""
        parser = subparsers.add_parser('resume', help='Resume interrupted operation')
        
        parser.add_argument(
            '--operation-id',
            required=True,
            help='Operation ID to resume'
        )
        parser.add_argument(
            '--force',
            action='store_true',
            help='Force resume despite warnings'
        )
        parser.add_argument(
            '--skip-validation',
            action='store_true',
            help='Skip checkpoint validation'
        )
    
    def _add_validate_parser(self, subparsers):
        """Add validate command parser."""
        parser = subparsers.add_parser('validate', help='Validate imported data')
        
        parser.add_argument(
            '--symbols',
            required=True,
            help='Symbols to validate'
        )
        parser.add_argument(
            '--start-date',
            required=True,
            type=lambda s: datetime.strptime(s, '%Y-%m-%d'),
            help='Start date'
        )
        parser.add_argument(
            '--end-date',
            required=True,
            type=lambda s: datetime.strptime(s, '%Y-%m-%d'),
            help='End date'
        )
        parser.add_argument(
            '--checks',
            default='all',
            help='Validation checks to run'
        )
        parser.add_argument(
            '--fix',
            action='store_true',
            help='Attempt to fix issues'
        )
        parser.add_argument(
            '--report',
            help='Generate validation report'
        )
    
    def _add_diagnose_parser(self, subparsers):
        """Add diagnose command parser."""
        parser = subparsers.add_parser('diagnose', help='Run system diagnostics')
        
        parser.add_argument(
            '--export',
            help='Export diagnostics to file'
        )
        parser.add_argument(
            '--alert-on-issues',
            action='store_true',
            help='Send alerts if issues found'
        )
    
    def _load_config(self) -> Dict[str, Any]:
        """Load configuration from file and environment."""
        config = {}
        
        # Try to load from file
        config_path = os.path.expanduser(
            os.environ.get('NEURAL_TRADER_CONFIG', '~/.neural_trader/backfill.yaml')
        )
        
        if os.path.exists(config_path):
            try:
                import yaml
                with open(config_path, 'r') as f:
                    config = yaml.safe_load(f)
            except Exception as e:
                logger.warning(f"Failed to load config from {config_path}: {e}")
        
        # Override with environment variables
        env_mappings = {
            'BACKFILL_WORKERS': ('backfill', 'defaults', 'workers'),
            'BACKFILL_BATCH_SIZE': ('backfill', 'defaults', 'batch_size'),
            'DB_HOST': ('database', 'host'),
            'DB_PORT': ('database', 'port'),
            'DB_NAME': ('database', 'name'),
            'DB_USER': ('database', 'username'),
            'DB_PASSWORD': ('database', 'password'),
            'REDIS_HOST': ('redis', 'host'),
            'REDIS_PORT': ('redis', 'port'),
        }
        
        for env_var, config_path in env_mappings.items():
            if env_var in os.environ:
                # Navigate nested config
                current = config
                for key in config_path[:-1]:
                    if key not in current:
                        current[key] = {}
                    current = current[key]
                
                # Set value
                value = os.environ[env_var]
                # Try to convert to appropriate type
                if value.isdigit():
                    value = int(value)
                current[config_path[-1]] = value
        
        return config
    
    async def run_s3_command(self, args):
        """Execute S3 download command."""
        # Import here to avoid circular imports
        from scripts.download_polygon_s3 import PolygonS3Downloader
        
        logger.info("Starting S3 download")
        
        # Parse symbols
        symbols = [s.strip() for s in args.symbols.split(',')]
        
        # Initialize downloader
        downloader = PolygonS3Downloader(
            aws_profile=args.profile,
            external_drive_path=args.destination,
            bucket_name=self.config.get('s3', {}).get('bucket', 'flatfiles'),
            region=self.config.get('s3', {}).get('region', 'us-east-1')
        )
        
        # Build S3 prefix based on data type
        prefix = f"us_stocks_sip/{args.data_type}/"
        
        # Execute download
        if args.dry_run:
            logger.info("[DRY RUN] Would download:")
            logger.info(f"  Symbols: {symbols}")
            logger.info(f"  Date range: {args.start_date} to {args.end_date}")
            logger.info(f"  Destination: {args.destination}")
        else:
            downloader.download_batch(
                prefix=prefix,
                start_date=args.start_date,
                end_date=args.end_date,
                file_pattern=None,  # Will filter by symbol in processing
                max_files=None
            )
    
    async def run_file_command(self, args):
        """Execute file import command."""
        logger.info("Starting file import")
        
        # Parse symbols
        symbols = None
        if args.symbols:
            symbols = [s.strip() for s in args.symbols.split(',')]
        elif args.symbols_file:
            with open(args.symbols_file, 'r') as f:
                symbols = [line.strip() for line in f if line.strip()]
        
        # Initialize handler
        handler = FileBackfillHandler(
            path=args.path,
            format=args.format,
            symbols=symbols,
            start_date=args.start_date,
            end_date=args.end_date,
            batch_size=args.batch_size,
            use_checkpoint=args.checkpoint,
            dry_run=args.dry_run
        )
        
        # Run backfill
        await handler.run()
    
    async def run_status_command(self, args):
        """Execute status command."""
        logger.info("Checking backfill status")
        
        # Get status from checkpoint system
        redis_store = RedisStore()
        try:
            await redis_store.connect()
            
            if args.operation_id:
                # Get specific operation
                status = await redis_store.cache_get(f"operation:{args.operation_id}")
                if status:
                    if args.format == 'json':
                        print(json.dumps(status, indent=2))
                    else:
                        self._print_status_table(status)
                else:
                    logger.error(f"Operation {args.operation_id} not found")
            else:
                # List all operations
                operations = []
                async for key in redis_store.cache_scan("operation:*"):
                    op_data = await redis_store.cache_get(key)
                    if op_data:
                        operations.append(op_data)
                
                if args.format == 'json':
                    print(json.dumps(operations, indent=2))
                else:
                    self._print_operations_table(operations)
                    
        finally:
            await redis_store.disconnect()
    
    async def run_validate_command(self, args):
        """Execute validate command."""
        logger.info("Running data validation")
        
        # Import validation module
        from validation.data_quality import DataQualityValidator
        
        # Parse symbols
        symbols = [s.strip() for s in args.symbols.split(',')]
        
        # Initialize validator
        validator = DataQualityValidator()
        
        # Connect to database
        storage = TimescaleDB()
        await storage.connect()
        
        try:
            # Run validation
            results = await validator.validate_date_range(
                storage=storage,
                symbols=symbols,
                start_date=args.start_date,
                end_date=args.end_date,
                checks=args.checks.split(',') if args.checks != 'all' else None
            )
            
            # Display or save results
            if args.report:
                with open(args.report, 'w') as f:
                    json.dump(results, f, indent=2, default=str)
                logger.info(f"Validation report saved to {args.report}")
            else:
                print(json.dumps(results, indent=2, default=str))
                
        finally:
            await storage.disconnect()
    
    async def run_diagnose_command(self, args):
        """Execute diagnose command."""
        logger.info("Running system diagnostics")
        
        diagnostics = {
            'timestamp': datetime.utcnow().isoformat(),
            'system': {},
            'services': {},
            'configuration': {},
            'issues': []
        }
        
        # Check AWS credentials
        try:
            import boto3
            session = boto3.Session()
            credentials = session.get_credentials()
            diagnostics['services']['aws'] = {
                'status': 'OK' if credentials else 'ERROR',
                'profile': os.environ.get('AWS_PROFILE', 'default')
            }
        except Exception as e:
            diagnostics['services']['aws'] = {
                'status': 'ERROR',
                'error': str(e)
            }
            diagnostics['issues'].append('AWS credentials not configured')
        
        # Check database connection
        try:
            storage = TimescaleDB()
            await storage.connect()
            diagnostics['services']['database'] = {
                'status': 'OK',
                'host': self.config.get('database', {}).get('host', 'localhost')
            }
            await storage.disconnect()
        except Exception as e:
            diagnostics['services']['database'] = {
                'status': 'ERROR',
                'error': str(e)
            }
            diagnostics['issues'].append('Database connection failed')
        
        # Check Redis connection
        try:
            redis_store = RedisStore()
            await redis_store.connect()
            diagnostics['services']['redis'] = {
                'status': 'OK',
                'host': self.config.get('redis', {}).get('host', 'localhost')
            }
            await redis_store.disconnect()
        except Exception as e:
            diagnostics['services']['redis'] = {
                'status': 'WARNING',
                'error': str(e)
            }
            diagnostics['issues'].append('Redis connection failed (checkpoints disabled)')
        
        # Check disk space
        import shutil
        usage = shutil.disk_usage('/')
        diagnostics['system']['disk'] = {
            'total_gb': usage.total / (1024**3),
            'free_gb': usage.free / (1024**3),
            'used_percent': (usage.used / usage.total) * 100
        }
        
        if diagnostics['system']['disk']['used_percent'] > 90:
            diagnostics['issues'].append('Low disk space')
        
        # Check memory
        import psutil
        memory = psutil.virtual_memory()
        diagnostics['system']['memory'] = {
            'total_gb': memory.total / (1024**3),
            'available_gb': memory.available / (1024**3),
            'used_percent': memory.percent
        }
        
        if memory.percent > 90:
            diagnostics['issues'].append('Low memory')
        
        # Output results
        if args.export:
            with open(args.export, 'w') as f:
                json.dump(diagnostics, f, indent=2)
            logger.info(f"Diagnostics exported to {args.export}")
        else:
            # Print summary
            print("\n=== System Diagnostics ===")
            print(f"Timestamp: {diagnostics['timestamp']}")
            print("\nServices:")
            for service, status in diagnostics['services'].items():
                icon = "✅" if status['status'] == 'OK' else "❌"
                print(f"  {icon} {service}: {status['status']}")
            
            print("\nSystem Resources:")
            print(f"  Disk: {diagnostics['system']['disk']['free_gb']:.1f} GB free "
                  f"({diagnostics['system']['disk']['used_percent']:.1f}% used)")
            print(f"  Memory: {diagnostics['system']['memory']['available_gb']:.1f} GB available "
                  f"({diagnostics['system']['memory']['used_percent']:.1f}% used)")
            
            if diagnostics['issues']:
                print("\n⚠️  Issues Found:")
                for issue in diagnostics['issues']:
                    print(f"  - {issue}")
            else:
                print("\n✅ No issues found")
        
        # Send alerts if requested
        if args.alert_on_issues and diagnostics['issues']:
            logger.warning(f"System issues detected: {diagnostics['issues']}")
            # TODO: Implement alert sending
    
    def _print_status_table(self, status: Dict[str, Any]):
        """Print operation status in table format."""
        print(f"\nOperation: {status.get('id', 'Unknown')}")
        print(f"Status: {status.get('status', 'Unknown')}")
        print(f"Type: {status.get('type', 'Unknown')}")
        print(f"Started: {status.get('started_at', 'Unknown')}")
        
        if 'progress' in status:
            progress = status['progress']
            print(f"\nProgress: {progress.get('percentage', 0):.1f}%")
            print(f"Files: {progress.get('files_completed', 0)} / {progress.get('files_total', 0)}")
            print(f"Records: {progress.get('records_processed', 0):,}")
            print(f"Speed: {progress.get('records_per_second', 0):,} records/sec")
            
        if 'eta' in status:
            print(f"\nETA: {status['eta']}")
    
    def _print_operations_table(self, operations: List[Dict[str, Any]]):
        """Print operations list in table format."""
        if not operations:
            print("No operations found")
            return
            
        print("\nBackfill Operations:")
        print("-" * 80)
        print(f"{'ID':<20} {'Status':<12} {'Type':<15} {'Progress':<10} {'Started':<20}")
        print("-" * 80)
        
        for op in operations:
            progress = op.get('progress', {}).get('percentage', 0)
            print(f"{op.get('id', ''):<20} "
                  f"{op.get('status', ''):<12} "
                  f"{op.get('type', ''):<15} "
                  f"{progress:>9.1f}% "
                  f"{op.get('started_at', ''):<20}")
    
    def run(self):
        """Main entry point for the CLI."""
        args = self.parser.parse_args()
        
        # Configure logging
        logging.basicConfig(
            level=getattr(logging, args.log_level),
            format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
        )
        
        # Start metrics server if requested
        if args.metrics_port:
            from prometheus_client import start_http_server
            start_http_server(args.metrics_port)
            logger.info(f"Metrics server started on port {args.metrics_port}")
        
        # Execute command
        if not args.command:
            self.parser.print_help()
            return
        
        # Map commands to handlers
        command_handlers = {
            's3': self.run_s3_command,
            'file': self.run_file_command,
            'status': self.run_status_command,
            'resume': self.run_resume_command,
            'validate': self.run_validate_command,
            'diagnose': self.run_diagnose_command,
        }
        
        handler = command_handlers.get(args.command)
        if handler:
            try:
                asyncio.run(handler(args))
            except KeyboardInterrupt:
                logger.info("Operation interrupted by user")
            except Exception as e:
                logger.error(f"Command failed: {e}")
                if args.log_level == 'DEBUG':
                    import traceback
                    traceback.print_exc()
                sys.exit(1)
        else:
            logger.error(f"Unknown command: {args.command}")
            self.parser.print_help()
            sys.exit(1)


def main():
    """Main entry point."""
    cli = BackfillCLI()
    cli.run()


if __name__ == '__main__':
    main()