"""
CLI Integration Tests for Backfill Commands

This module tests the command-line interface for backfill operations,
including argument parsing, command execution, and error handling.
"""

import pytest
import asyncio
import tempfile
import shutil
import json
import gzip
from pathlib import Path
from datetime import datetime, timedelta
from unittest.mock import AsyncMock, Mock, patch, MagicMock
from io import StringIO
import sys

# System imports
import os
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from cli.backfill import BackfillCLI, main
from utils.file_backfill import FileBackfillHandler
from storage.timescale import TimescaleDB
from storage.redis_store import RedisStore


class TestBackfillCLIArguments:
    """Test CLI argument parsing and validation"""
    
    @pytest.fixture
    def cli(self):
        """Create CLI instance for testing."""
        return BackfillCLI()
    
    def test_help_display(self, cli, capsys):
        """Test help message display."""
        # Test main help
        with pytest.raises(SystemExit):
            cli.parser.parse_args(['--help'])
        
        captured = capsys.readouterr()
        assert 'Neural Trader Data Backfill CLI' in captured.out
        assert 'Examples:' in captured.out
    
    def test_s3_command_parsing(self, cli):
        """Test S3 command argument parsing."""
        args = cli.parser.parse_args([
            's3',
            '--profile', 'test-profile',
            '--symbols', 'AAPL,MSFT,GOOGL',
            '--start-date', '2023-01-01',
            '--end-date', '2023-12-31',
            '--data-type', 'day_aggs_v1',
            '--destination', '/tmp/test-data',
            '--max-workers', '5',
            '--batch-size', '5000',
            '--dry-run'
        ])
        
        assert args.command == 's3'
        assert args.profile == 'test-profile'
        assert args.symbols == 'AAPL,MSFT,GOOGL'
        assert args.start_date.year == 2023
        assert args.start_date.month == 1
        assert args.start_date.day == 1
        assert args.end_date.year == 2023
        assert args.end_date.month == 12
        assert args.end_date.day == 31
        assert args.data_type == 'day_aggs_v1'
        assert args.destination == '/tmp/test-data'
        assert args.max_workers == 5
        assert args.batch_size == 5000
        assert args.dry_run is True
    
    def test_file_command_parsing(self, cli):
        """Test file command argument parsing."""
        args = cli.parser.parse_args([
            'file',
            '--path', '/data/test.csv',
            '--format', 'csv',
            '--symbols', 'AAPL,MSFT',
            '--start-date', '2023-06-01',
            '--end-date', '2023-12-31',
            '--recursive',
            '--pattern', '*.csv',
            '--batch-size', '10000',
            '--workers', '3',
            '--checkpoint',
            '--skip-errors'
        ])
        
        assert args.command == 'file'
        assert str(args.path) == '/data/test.csv'
        assert args.format == 'csv'
        assert args.symbols == 'AAPL,MSFT'
        assert args.start_date.year == 2023
        assert args.start_date.month == 6
        assert args.recursive is True
        assert args.pattern == '*.csv'
        assert args.batch_size == 10000
        assert args.workers == 3
        assert args.checkpoint is True
        assert args.skip_errors is True
    
    def test_status_command_parsing(self, cli):
        """Test status command argument parsing."""
        args = cli.parser.parse_args([
            'status',
            '--operation-id', 'op_123abc',
            '--detailed',
            '--format', 'json',
            '--show-checkpoints'
        ])
        
        assert args.command == 'status'
        assert args.operation_id == 'op_123abc'
        assert args.detailed is True
        assert args.format == 'json'
        assert args.show_checkpoints is True
    
    def test_resume_command_parsing(self, cli):
        """Test resume command argument parsing."""
        args = cli.parser.parse_args([
            'resume',
            '--operation-id', 'op_456def',
            '--force',
            '--skip-validation'
        ])
        
        assert args.command == 'resume'
        assert args.operation_id == 'op_456def'
        assert args.force is True
        assert args.skip_validation is True
    
    def test_validate_command_parsing(self, cli):
        """Test validate command argument parsing."""
        args = cli.parser.parse_args([
            'validate',
            '--symbols', 'AAPL,MSFT',
            '--start-date', '2023-01-01',
            '--end-date', '2023-12-31',
            '--checks', 'consistency,completeness',
            '--fix',
            '--report', '/tmp/validation_report.json'
        ])
        
        assert args.command == 'validate'
        assert args.symbols == 'AAPL,MSFT'
        assert args.checks == 'consistency,completeness'
        assert args.fix is True
        assert args.report == '/tmp/validation_report.json'
    
    def test_diagnose_command_parsing(self, cli):
        """Test diagnose command argument parsing."""
        args = cli.parser.parse_args([
            'diagnose',
            '--export', '/tmp/diagnostics.json',
            '--alert-on-issues'
        ])
        
        assert args.command == 'diagnose'
        assert args.export == '/tmp/diagnostics.json'
        assert args.alert_on_issues is True
    
    def test_invalid_date_format(self, cli):
        """Test handling of invalid date formats."""
        with pytest.raises(SystemExit):
            cli.parser.parse_args([
                'file',
                '--path', '/data/test.csv',
                '--start-date', 'invalid-date'
            ])
    
    def test_missing_required_arguments(self, cli):
        """Test handling of missing required arguments."""
        # S3 command missing required arguments
        with pytest.raises(SystemExit):
            cli.parser.parse_args(['s3'])
        
        # File command missing required arguments
        with pytest.raises(SystemExit):
            cli.parser.parse_args(['file'])
        
        # Resume command missing required arguments
        with pytest.raises(SystemExit):
            cli.parser.parse_args(['resume'])


class TestBackfillCLIExecution:
    """Test CLI command execution"""
    
    @pytest.fixture
    def cli(self):
        """Create CLI instance for testing."""
        return BackfillCLI()
    
    @pytest.fixture
    def temp_dir(self):
        """Create temporary directory for test files."""
        temp_dir = tempfile.mkdtemp()
        yield Path(temp_dir)
        shutil.rmtree(temp_dir)
    
    @pytest.fixture
    def sample_csv_file(self, temp_dir):
        """Create sample CSV file for testing."""
        csv_file = temp_dir / "sample_data.csv"
        with open(csv_file, 'w') as f:
            f.write('timestamp,symbol,open,high,low,close,volume\n')
            f.write('2024-01-01 09:30:00,AAPL,150.0,152.0,149.0,151.0,1000000\n')
            f.write('2024-01-01 09:31:00,AAPL,151.0,153.0,150.0,152.0,1100000\n')
            f.write('2024-01-01 09:32:00,AAPL,152.0,154.0,151.0,153.0,1200000\n')
        return csv_file
    
    @pytest.mark.asyncio
    async def test_file_command_execution(self, cli, sample_csv_file):
        """Test file command execution."""
        # Mock the FileBackfillHandler
        with patch('cli.backfill.FileBackfillHandler') as mock_handler_class:
            mock_handler = AsyncMock()
            mock_handler_class.return_value = mock_handler
            
            # Create mock arguments
            args = Mock()
            args.path = sample_csv_file
            args.format = 'csv'
            args.symbols = 'AAPL'
            args.symbols_file = None
            args.start_date = None
            args.end_date = None
            args.batch_size = 1000
            args.checkpoint = True
            args.dry_run = False
            
            # Execute the command
            await cli.run_file_command(args)
            
            # Verify handler was created and run
            mock_handler_class.assert_called_once()
            mock_handler.run.assert_called_once()
    
    @pytest.mark.asyncio
    async def test_file_command_with_symbols_file(self, cli, temp_dir, sample_csv_file):
        """Test file command with symbols file."""
        # Create symbols file
        symbols_file = temp_dir / "symbols.txt"
        with open(symbols_file, 'w') as f:
            f.write('AAPL\n')
            f.write('MSFT\n')
            f.write('GOOGL\n')
        
        with patch('cli.backfill.FileBackfillHandler') as mock_handler_class:
            mock_handler = AsyncMock()
            mock_handler_class.return_value = mock_handler
            
            args = Mock()
            args.path = sample_csv_file
            args.format = 'csv'
            args.symbols = None
            args.symbols_file = symbols_file
            args.start_date = None
            args.end_date = None
            args.batch_size = 1000
            args.checkpoint = True
            args.dry_run = False
            
            await cli.run_file_command(args)
            
            # Verify handler was created with symbols from file
            mock_handler_class.assert_called_once()
            call_kwargs = mock_handler_class.call_args[1]
            assert call_kwargs['symbols'] == ['AAPL', 'MSFT', 'GOOGL']
    
    @pytest.mark.asyncio
    async def test_status_command_execution(self, cli):
        """Test status command execution."""
        with patch('cli.backfill.RedisStore') as mock_redis_class:
            mock_redis = AsyncMock()
            mock_redis_class.return_value = mock_redis
            
            # Mock operation data
            mock_operation = {
                'id': 'op_123abc',
                'status': 'running',
                'type': 'file_backfill',
                'started_at': '2024-01-01T09:00:00Z',
                'progress': {
                    'percentage': 45.5,
                    'files_completed': 2,
                    'files_total': 5,
                    'records_processed': 12500,
                    'records_per_second': 150
                }
            }
            mock_redis.cache_get.return_value = mock_operation
            
            args = Mock()
            args.operation_id = 'op_123abc'
            args.format = 'table'
            args.detailed = True
            args.show_checkpoints = False
            
            # Capture output
            with patch('builtins.print') as mock_print:
                await cli.run_status_command(args)
            
            # Verify Redis was called
            mock_redis.connect.assert_called_once()
            mock_redis.cache_get.assert_called_with('operation:op_123abc')
            mock_redis.disconnect.assert_called_once()
            
            # Verify output was printed
            mock_print.assert_called()
    
    @pytest.mark.asyncio
    async def test_status_command_json_output(self, cli):
        """Test status command with JSON output."""
        with patch('cli.backfill.RedisStore') as mock_redis_class:
            mock_redis = AsyncMock()
            mock_redis_class.return_value = mock_redis
            
            mock_operation = {
                'id': 'op_123abc',
                'status': 'completed',
                'type': 'file_backfill'
            }
            mock_redis.cache_get.return_value = mock_operation
            
            args = Mock()
            args.operation_id = 'op_123abc'
            args.format = 'json'
            args.detailed = False
            args.show_checkpoints = False
            
            with patch('builtins.print') as mock_print:
                await cli.run_status_command(args)
            
            # Verify JSON output
            printed_args = mock_print.call_args[0]
            json_output = printed_args[0]
            parsed = json.loads(json_output)
            assert parsed['id'] == 'op_123abc'
            assert parsed['status'] == 'completed'
    
    @pytest.mark.asyncio
    async def test_resume_command_execution(self, cli):
        """Test resume command execution."""
        with patch('cli.backfill.RedisStore') as mock_redis_class, \
             patch('cli.backfill.FileBackfillHandler') as mock_handler_class:
            
            mock_redis = AsyncMock()
            mock_redis_class.return_value = mock_redis
            
            mock_handler = AsyncMock()
            mock_handler_class.return_value = mock_handler
            
            # Mock operation data
            operation_data = {
                'id': 'op_123abc',
                'type': 'file_backfill',
                'config': {
                    'path': '/data/test.csv',
                    'format': 'csv',
                    'symbols': ['AAPL'],
                    'batch_size': 1000,
                    'dry_run': False
                }
            }
            mock_redis.cache_get.return_value = operation_data
            
            args = Mock()
            args.operation_id = 'op_123abc'
            args.force = False
            args.skip_validation = False
            
            await cli.run_resume_command(args)
            
            # Verify operation was retrieved and handler was created
            mock_redis.cache_get.assert_called_with('operation:op_123abc')
            mock_handler_class.assert_called_once()
            mock_handler.run.assert_called_once()
    
    @pytest.mark.asyncio
    async def test_validate_command_execution(self, cli):
        """Test validate command execution."""
        with patch('cli.backfill.DataQualityValidator') as mock_validator_class, \
             patch('cli.backfill.TimescaleDB') as mock_db_class:
            
            mock_validator = Mock()
            mock_validator_class.return_value = mock_validator
            
            mock_db = AsyncMock()
            mock_db_class.return_value = mock_db
            
            # Mock validation results
            validation_results = {
                'symbols': ['AAPL'],
                'date_range': ['2023-01-01', '2023-12-31'],
                'checks_passed': 8,
                'checks_failed': 2,
                'quality_score': 0.85,
                'issues': ['Missing data on 2023-02-15', 'Duplicate record at 2023-03-20 10:30:00']
            }
            mock_validator.validate_date_range.return_value = validation_results
            
            args = Mock()
            args.symbols = 'AAPL'
            args.start_date = datetime(2023, 1, 1)
            args.end_date = datetime(2023, 12, 31)
            args.checks = 'all'
            args.fix = False
            args.report = None
            
            with patch('builtins.print') as mock_print:
                await cli.run_validate_command(args)
            
            # Verify validation was performed
            mock_validator.validate_date_range.assert_called_once()
            mock_print.assert_called()
    
    @pytest.mark.asyncio
    async def test_diagnose_command_execution(self, cli):
        """Test diagnose command execution."""
        with patch('cli.backfill.TimescaleDB') as mock_db_class, \
             patch('cli.backfill.RedisStore') as mock_redis_class, \
             patch('boto3.Session') as mock_boto_session:
            
            mock_db = AsyncMock()
            mock_db_class.return_value = mock_db
            
            mock_redis = AsyncMock()
            mock_redis_class.return_value = mock_redis
            
            # Mock AWS session
            mock_session = Mock()
            mock_credentials = Mock()
            mock_session.get_credentials.return_value = mock_credentials
            mock_boto_session.return_value = mock_session
            
            args = Mock()
            args.export = None
            args.alert_on_issues = False
            
            with patch('builtins.print') as mock_print:
                await cli.run_diagnose_command(args)
            
            # Verify connections were tested
            mock_db.connect.assert_called_once()
            mock_db.disconnect.assert_called_once()
            mock_redis.connect.assert_called_once()
            mock_redis.disconnect.assert_called_once()
            
            # Verify output was printed
            mock_print.assert_called()


class TestBackfillCLIIntegration:
    """Test end-to-end CLI integration scenarios"""
    
    @pytest.fixture
    def temp_dir(self):
        """Create temporary directory for test files."""
        temp_dir = tempfile.mkdtemp()
        yield Path(temp_dir)
        shutil.rmtree(temp_dir)
    
    def test_main_function_execution(self, temp_dir):
        """Test main function execution with various arguments."""
        # Create test CSV file
        csv_file = temp_dir / "test_data.csv"
        with open(csv_file, 'w') as f:
            f.write('timestamp,symbol,open,high,low,close,volume\n')
            f.write('2024-01-01 09:30:00,AAPL,150.0,152.0,149.0,151.0,1000000\n')
        
        # Test help command
        with patch('sys.argv', ['backfill', '--help']):
            with pytest.raises(SystemExit) as exc_info:
                main()
            assert exc_info.value.code == 0  # Help should exit cleanly
    
    def test_config_loading(self, temp_dir):
        """Test configuration file loading."""
        # Create config file
        config_file = temp_dir / "backfill.yaml"
        config_content = """
        backfill:
          defaults:
            workers: 8
            batch_size: 5000
        database:
          host: localhost
          port: 5432
          name: test_db
        redis:
          host: localhost
          port: 6379
        """
        with open(config_file, 'w') as f:
            f.write(config_content)
        
        # Test config loading
        with patch.dict(os.environ, {'NEURAL_TRADER_CONFIG': str(config_file)}):
            cli = BackfillCLI()
            config = cli._load_config()
            
            assert config['backfill']['defaults']['workers'] == 8
            assert config['database']['host'] == 'localhost'
    
    def test_environment_variable_override(self):
        """Test environment variable configuration override."""
        env_vars = {
            'BACKFILL_WORKERS': '12',
            'BACKFILL_BATCH_SIZE': '8000',
            'DB_HOST': 'prod-db.example.com',
            'DB_PORT': '5433',
            'REDIS_HOST': 'cache.example.com'
        }
        
        with patch.dict(os.environ, env_vars):
            cli = BackfillCLI()
            config = cli._load_config()
            
            # Environment variables should override defaults
            assert config['backfill']['defaults']['workers'] == 12
            assert config['backfill']['defaults']['batch_size'] == 8000
            assert config['database']['host'] == 'prod-db.example.com'
            assert config['database']['port'] == 5433
            assert config['redis']['host'] == 'cache.example.com'
    
    @pytest.mark.asyncio
    async def test_error_handling_and_reporting(self, temp_dir):
        """Test error handling and reporting in CLI."""
        # Create invalid CSV file
        invalid_csv = temp_dir / "invalid_data.csv"
        with open(invalid_csv, 'w') as f:
            f.write('invalid,csv,format\n')
            f.write('missing,data\n')  # Inconsistent columns
        
        cli = BackfillCLI()
        
        # Mock to raise exception
        with patch('cli.backfill.FileBackfillHandler') as mock_handler_class:
            mock_handler = AsyncMock()
            mock_handler.run.side_effect = Exception("File processing error")
            mock_handler_class.return_value = mock_handler
            
            args = Mock()
            args.path = invalid_csv
            args.format = 'csv'
            args.symbols = None
            args.symbols_file = None
            args.start_date = None
            args.end_date = None
            args.batch_size = 1000
            args.checkpoint = True
            args.dry_run = False
            
            # Should handle exception gracefully
            with pytest.raises(Exception, match="File processing error"):
                await cli.run_file_command(args)
    
    def test_dry_run_functionality(self, temp_dir):
        """Test dry run functionality across commands."""
        csv_file = temp_dir / "dry_run_test.csv"
        with open(csv_file, 'w') as f:
            f.write('timestamp,symbol,open,high,low,close,volume\n')
            f.write('2024-01-01 09:30:00,AAPL,150.0,152.0,149.0,151.0,1000000\n')
        
        cli = BackfillCLI()
        
        # Test dry run argument parsing
        args = cli.parser.parse_args([
            'file',
            '--path', str(csv_file),
            '--dry-run'
        ])
        
        assert args.dry_run is True
        
        # Test S3 dry run
        s3_args = cli.parser.parse_args([
            's3',
            '--profile', 'test',
            '--symbols', 'AAPL',
            '--start-date', '2023-01-01',
            '--end-date', '2023-12-31',
            '--dry-run'
        ])
        
        assert s3_args.dry_run is True
    
    def test_metrics_server_integration(self):
        """Test metrics server startup integration."""
        with patch('prometheus_client.start_http_server') as mock_start_server:
            cli = BackfillCLI()
            
            # Mock args with metrics port
            args = Mock()
            args.command = None
            args.metrics_port = 8080
            args.log_level = 'INFO'
            
            # Patch parse_args to return our mock
            with patch.object(cli.parser, 'parse_args', return_value=args):
                cli.run()
            
            # Verify metrics server was started
            mock_start_server.assert_called_once_with(8080)
    
    def test_logging_configuration(self):
        """Test logging configuration from CLI arguments."""
        cli = BackfillCLI()
        
        # Test different log levels
        log_levels = ['DEBUG', 'INFO', 'WARNING', 'ERROR']
        
        for level in log_levels:
            args = cli.parser.parse_args([
                'diagnose',
                '--log-level', level
            ])
            
            assert args.log_level == level
    
    @pytest.mark.asyncio
    async def test_keyboard_interrupt_handling(self, temp_dir):
        """Test handling of keyboard interrupts."""
        csv_file = temp_dir / "interrupt_test.csv"
        with open(csv_file, 'w') as f:
            f.write('timestamp,symbol,open,high,low,close,volume\n')
            f.write('2024-01-01 09:30:00,AAPL,150.0,152.0,149.0,151.0,1000000\n')
        
        cli = BackfillCLI()
        
        # Mock to raise KeyboardInterrupt
        with patch('cli.backfill.FileBackfillHandler') as mock_handler_class:
            mock_handler = AsyncMock()
            mock_handler.run.side_effect = KeyboardInterrupt()
            mock_handler_class.return_value = mock_handler
            
            args = Mock()
            args.path = csv_file
            args.format = 'csv'
            args.symbols = None
            args.symbols_file = None
            args.start_date = None
            args.end_date = None
            args.batch_size = 1000
            args.checkpoint = True
            args.dry_run = False
            
            # Should handle KeyboardInterrupt gracefully
            with pytest.raises(KeyboardInterrupt):
                await cli.run_file_command(args)


if __name__ == "__main__":
    pytest.main([__file__, "-v", "--tb=short"])