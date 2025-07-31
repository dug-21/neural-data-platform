"""Comprehensive tests for file provider implementation."""
import pytest
import asyncio
import csv
import json
import tempfile
from pathlib import Path
from datetime import datetime, timedelta
import pandas as pd
from unittest.mock import Mock, AsyncMock, patch

from data_ingestion.providers.file_provider import FileProvider, CheckpointManager, FileMetadata
from data_ingestion.providers.base import MarketData


class TestCheckpointManager:
    """Test checkpoint manager functionality."""
    
    @pytest.fixture
    def temp_checkpoint_dir(self):
        """Create temporary checkpoint directory."""
        with tempfile.TemporaryDirectory() as tmpdir:
            yield tmpdir
            
    @pytest.fixture
    def checkpoint_manager(self, temp_checkpoint_dir):
        """Create checkpoint manager with temp directory."""
        return CheckpointManager(checkpoint_dir=temp_checkpoint_dir)
        
    def test_checkpoint_path_generation(self, checkpoint_manager):
        """Test checkpoint path is consistent for same file."""
        path1 = checkpoint_manager._get_checkpoint_path("/data/test.csv")
        path2 = checkpoint_manager._get_checkpoint_path("/data/test.csv")
        assert path1 == path2
        
        path3 = checkpoint_manager._get_checkpoint_path("/data/other.csv")
        assert path1 != path3
        
    def test_checkpoint_lifecycle(self, checkpoint_manager):
        """Test checkpoint create, read, update, clear."""
        filepath = "/data/test.csv"
        
        # Initially no checkpoint
        assert checkpoint_manager.get_checkpoint(filepath) == 0
        
        # Create checkpoint
        checkpoint_manager.update_checkpoint(filepath, 100, {'test': 'data'})
        assert checkpoint_manager.get_checkpoint(filepath) == 100
        
        # Update checkpoint
        checkpoint_manager.update_checkpoint(filepath, 200)
        assert checkpoint_manager.get_checkpoint(filepath) == 200
        
        # Clear checkpoint
        checkpoint_manager.clear_checkpoint(filepath)
        assert checkpoint_manager.get_checkpoint(filepath) == 0
        
    def test_checkpoint_persistence(self, temp_checkpoint_dir):
        """Test checkpoint persists across manager instances."""
        filepath = "/data/test.csv"
        
        # Create checkpoint with first manager
        manager1 = CheckpointManager(checkpoint_dir=temp_checkpoint_dir)
        manager1.update_checkpoint(filepath, 500)
        
        # Read with new manager instance
        manager2 = CheckpointManager(checkpoint_dir=temp_checkpoint_dir)
        assert manager2.get_checkpoint(filepath) == 500


class TestFileProvider:
    """Test file provider functionality."""
    
    @pytest.fixture
    def file_provider(self):
        """Create file provider instance."""
        return FileProvider({'batch_size': 10})
        
    @pytest.fixture
    def temp_csv_file(self):
        """Create temporary CSV file with test data."""
        with tempfile.NamedTemporaryFile(mode='w', suffix='.csv', delete=False) as f:
            writer = csv.DictWriter(f, fieldnames=['timestamp', 'symbol', 'open', 'high', 'low', 'close', 'volume'])
            writer.writeheader()
            
            base_time = datetime.now()
            for i in range(100):
                writer.writerow({
                    'timestamp': (base_time + timedelta(minutes=i)).strftime('%Y-%m-%d %H:%M:%S'),
                    'symbol': 'TEST',
                    'open': 100 + i * 0.1,
                    'high': 100.5 + i * 0.1,
                    'low': 99.5 + i * 0.1,
                    'close': 100.2 + i * 0.1,
                    'volume': 1000000 + i * 1000
                })
                
            f.flush()
            yield f.name
            
        # Cleanup
        Path(f.name).unlink(missing_ok=True)
        
    @pytest.fixture
    def temp_json_file(self):
        """Create temporary JSON file with test data."""
        with tempfile.NamedTemporaryFile(mode='w', suffix='.json', delete=False) as f:
            data = []
            base_time = datetime.now()
            
            for i in range(50):
                data.append({
                    'timestamp': (base_time + timedelta(minutes=i)).isoformat(),
                    'symbol': 'JSON_TEST',
                    'open': 200 + i * 0.2,
                    'high': 200.5 + i * 0.2,
                    'low': 199.5 + i * 0.2,
                    'close': 200.2 + i * 0.2,
                    'volume': 2000000 + i * 2000
                })
                
            json.dump(data, f)
            f.flush()
            yield f.name
            
        Path(f.name).unlink(missing_ok=True)
        
    @pytest.mark.asyncio
    async def test_connect_disconnect(self, file_provider):
        """Test connect and disconnect (no-op for file provider)."""
        await file_provider.connect()
        assert file_provider._connected
        
        await file_provider.disconnect()
        assert not file_provider._connected
        
    @pytest.mark.asyncio
    async def test_load_csv_file(self, file_provider, temp_csv_file):
        """Test loading CSV file."""
        await file_provider.connect()
        
        count = 0
        async for market_data in file_provider.load_from_file(temp_csv_file, format='csv'):
            assert isinstance(market_data, MarketData)
            assert market_data.symbol == 'TEST'
            assert market_data.provider == 'file'
            count += 1
            
        assert count == 100
        
    @pytest.mark.asyncio
    async def test_load_json_file(self, file_provider, temp_json_file):
        """Test loading JSON file."""
        await file_provider.connect()
        
        count = 0
        async for market_data in file_provider.load_from_file(temp_json_file, format='json'):
            assert isinstance(market_data, MarketData)
            assert market_data.symbol == 'JSON_TEST'
            count += 1
            
        assert count == 50
        
    @pytest.mark.asyncio
    async def test_checkpoint_recovery(self, file_provider, temp_csv_file):
        """Test checkpoint recovery after interruption."""
        await file_provider.connect()
        
        # First load - interrupt at row 30
        count1 = 0
        async for market_data in file_provider.load_from_file(temp_csv_file, format='csv'):
            count1 += 1
            if count1 >= 30:
                break
                
        assert count1 == 30
        
        # Second load - should resume from checkpoint
        count2 = 0
        async for market_data in file_provider.load_from_file(temp_csv_file, format='csv'):
            count2 += 1
            
        # Should have processed remaining 70 rows
        assert count2 == 70
        
    @pytest.mark.asyncio
    async def test_symbol_override(self, file_provider, temp_csv_file):
        """Test symbol override functionality."""
        await file_provider.connect()
        
        async for market_data in file_provider.load_from_file(
            temp_csv_file, 
            format='csv', 
            symbol='OVERRIDE'
        ):
            assert market_data.symbol == 'OVERRIDE'
            break
            
    @pytest.mark.asyncio
    async def test_unsupported_format(self, file_provider):
        """Test error on unsupported format."""
        with pytest.raises(ValueError, match="Unsupported format"):
            async for _ in file_provider.load_from_file("test.xyz", format='xyz'):
                pass
                
    @pytest.mark.asyncio
    async def test_file_not_found(self, file_provider):
        """Test error on missing file."""
        with pytest.raises(FileNotFoundError):
            async for _ in file_provider.load_from_file("/nonexistent/file.csv"):
                pass
                
    @pytest.mark.asyncio
    async def test_parse_market_data(self, file_provider):
        """Test market data parsing logic."""
        metadata = FileMetadata(filepath="test.csv", format="csv")
        
        # Test with complete data
        row_data = {
            'timestamp': '2024-01-01 10:00:00',
            'symbol': 'AAPL',
            'open': '150.0',
            'high': '151.0',
            'low': '149.0',
            'close': '150.5',
            'volume': '1000000'
        }
        
        market_data = file_provider._parse_market_data(row_data, metadata)
        assert market_data.symbol == 'AAPL'
        assert market_data.open == 150.0
        assert market_data.high == 151.0
        assert market_data.low == 149.0
        assert market_data.close == 150.5
        assert market_data.volume == 1000000
        
    @pytest.mark.asyncio
    async def test_batch_processing(self, file_provider, temp_csv_file):
        """Test batch processing works correctly."""
        file_provider.batch_size = 25  # Set specific batch size
        await file_provider.connect()
        
        batch_counts = []
        current_batch = 0
        
        # Mock checkpoint update to track batches
        original_update = file_provider.checkpoint_manager.update_checkpoint
        
        def track_batch(*args, **kwargs):
            nonlocal current_batch
            current_batch += 1
            batch_counts.append(current_batch)
            return original_update(*args, **kwargs)
            
        file_provider.checkpoint_manager.update_checkpoint = track_batch
        
        count = 0
        async for _ in file_provider.load_from_file(temp_csv_file, format='csv'):
            count += 1
            
        # Should have processed 100 rows in 4 batches of 25
        assert count == 100
        assert len(batch_counts) >= 3  # At least 3 checkpoint updates
        
    def test_field_mapping_flexibility(self, file_provider):
        """Test field mapping handles various column names."""
        metadata = FileMetadata(filepath="test.csv", format="csv")
        
        # Test alternative field names
        row_data = {
            'time': '2024-01-01T10:00:00',
            'ticker': 'MSFT',
            'o': '300',
            'h': '301',
            'l': '299',
            'c': '300.5',
            'vol': '500000'
        }
        
        market_data = file_provider._parse_market_data(row_data, metadata)
        assert market_data.symbol == 'MSFT'
        assert market_data.open == 300.0
        assert market_data.volume == 500000
        
    @pytest.mark.asyncio
    async def test_error_handling_in_rows(self, file_provider):
        """Test provider continues on row parsing errors."""
        with tempfile.NamedTemporaryFile(mode='w', suffix='.csv', delete=False) as f:
            writer = csv.DictWriter(f, fieldnames=['timestamp', 'symbol', 'open', 'high', 'low', 'close', 'volume'])
            writer.writeheader()
            
            # Write some good rows and some bad rows
            writer.writerow({'timestamp': '2024-01-01', 'symbol': 'GOOD', 'open': '100', 'high': '101', 'low': '99', 'close': '100', 'volume': '1000'})
            writer.writerow({'timestamp': 'bad_date', 'symbol': 'BAD', 'open': 'not_a_number', 'high': '101', 'low': '99', 'close': '100', 'volume': '1000'})
            writer.writerow({'timestamp': '2024-01-02', 'symbol': 'GOOD2', 'open': '102', 'high': '103', 'low': '101', 'close': '102', 'volume': '2000'})
            
            f.flush()
            filepath = f.name
            
        try:
            await file_provider.connect()
            
            count = 0
            symbols = []
            async for market_data in file_provider.load_from_file(filepath, format='csv'):
                count += 1
                symbols.append(market_data.symbol)
                
            # Should process all rows despite errors
            assert count == 3
            assert 'GOOD' in symbols
            assert 'GOOD2' in symbols
            
        finally:
            Path(filepath).unlink(missing_ok=True)


class TestIntegration:
    """Integration tests for file provider."""
    
    @pytest.mark.asyncio
    async def test_large_file_processing(self):
        """Test processing a larger file with checkpoints."""
        # Create a larger test file
        with tempfile.NamedTemporaryFile(mode='w', suffix='.csv', delete=False) as f:
            writer = csv.DictWriter(f, fieldnames=['timestamp', 'symbol', 'open', 'high', 'low', 'close', 'volume'])
            writer.writeheader()
            
            base_time = datetime.now()
            for i in range(10000):  # 10k rows
                writer.writerow({
                    'timestamp': (base_time + timedelta(minutes=i)).strftime('%Y-%m-%d %H:%M:%S'),
                    'symbol': f'SYM{i % 10}',  # 10 different symbols
                    'open': 100 + (i % 100) * 0.1,
                    'high': 100.5 + (i % 100) * 0.1,
                    'low': 99.5 + (i % 100) * 0.1,
                    'close': 100.2 + (i % 100) * 0.1,
                    'volume': 1000000 + i * 1000
                })
                
            f.flush()
            filepath = f.name
            
        try:
            provider = FileProvider({'batch_size': 500})
            await provider.connect()
            
            # Process file
            count = 0
            symbols = set()
            
            async for market_data in provider.load_from_file(filepath, format='csv'):
                count += 1
                symbols.add(market_data.symbol)
                
            assert count == 10000
            assert len(symbols) == 10
            
        finally:
            Path(filepath).unlink(missing_ok=True)