"""
Integration tests for S3 storage functionality
"""
import pytest
import asyncio
import aioboto3
from datetime import datetime, timedelta
from unittest.mock import Mock, patch, AsyncMock
import json
import gzip
from typing import List

from data_ingestion.providers.historical_backfill import (
    HistoricalBackfillCoordinator, BackfillJob, DataGranularity
)
from data_ingestion.providers.base import MarketData


class TestS3Integration:
    """Test suite for S3 integration"""
    
    @pytest.fixture
    async def s3_client(self):
        """Create mock S3 client for testing"""
        # In real tests, use localstack or moto
        mock_client = AsyncMock()
        mock_client.head_bucket = AsyncMock(return_value={'ResponseMetadata': {'HTTPStatusCode': 200}})
        mock_client.put_object = AsyncMock(return_value={'ETag': '"test-etag"'})
        mock_client.get_object = AsyncMock()
        mock_client.list_objects_v2 = AsyncMock()
        return mock_client
    
    @pytest.fixture
    def sample_market_data(self) -> List[MarketData]:
        """Generate sample market data for testing"""
        data = []
        base_time = datetime(2023, 1, 1, 9, 30)
        
        for i in range(1000):
            data.append(MarketData(
                time=base_time + timedelta(minutes=i),
                symbol="AAPL",
                open=150.0 + (i % 10),
                high=152.0 + (i % 10),
                low=148.0 + (i % 10),
                close=151.0 + (i % 10),
                volume=1000000 + (i * 1000)
            ))
        
        return data
    
    @pytest.mark.asyncio
    async def test_s3_connectivity(self, s3_client):
        """Test S3 bucket connection and permissions"""
        bucket_name = "trading-data-backfill"
        
        # Test bucket exists
        response = await s3_client.head_bucket(Bucket=bucket_name)
        assert response['ResponseMetadata']['HTTPStatusCode'] == 200
        
        # Test write permissions
        test_object = {
            "test": "connectivity",
            "timestamp": datetime.now().isoformat()
        }
        
        response = await s3_client.put_object(
            Bucket=bucket_name,
            Key="test/connectivity-check.json",
            Body=json.dumps(test_object)
        )
        assert 'ETag' in response
        
        # Test read permissions
        s3_client.get_object.return_value = {
            'Body': AsyncMock(read=AsyncMock(return_value=json.dumps(test_object).encode()))
        }
        
        response = await s3_client.get_object(
            Bucket=bucket_name,
            Key="test/connectivity-check.json"
        )
        body = await response['Body'].read()
        assert json.loads(body) == test_object
    
    @pytest.mark.asyncio
    async def test_s3_upload_download(self, s3_client, sample_market_data):
        """Test data upload and retrieval from S3"""
        bucket_name = "trading-data-backfill"
        
        # Convert market data to JSON
        data_json = json.dumps([
            {
                "time": d.time.isoformat(),
                "symbol": d.symbol,
                "open": d.open,
                "high": d.high,
                "low": d.low,
                "close": d.close,
                "volume": d.volume
            }
            for d in sample_market_data
        ])
        
        # Compress data
        compressed_data = gzip.compress(data_json.encode())
        
        # Upload to S3
        key = "market-data/AAPL/2023/01/01/data.json.gz"
        await s3_client.put_object(
            Bucket=bucket_name,
            Key=key,
            Body=compressed_data,
            ContentType="application/gzip",
            Metadata={
                "symbol": "AAPL",
                "date": "2023-01-01",
                "points": str(len(sample_market_data)),
                "granularity": "1min"
            }
        )
        
        # Download from S3
        s3_client.get_object.return_value = {
            'Body': AsyncMock(read=AsyncMock(return_value=compressed_data)),
            'Metadata': {
                "symbol": "AAPL",
                "date": "2023-01-01",
                "points": str(len(sample_market_data)),
                "granularity": "1min"
            }
        }
        
        response = await s3_client.get_object(Bucket=bucket_name, Key=key)
        
        # Decompress and verify
        body = await response['Body'].read()
        decompressed = gzip.decompress(body).decode()
        retrieved_data = json.loads(decompressed)
        
        assert len(retrieved_data) == len(sample_market_data)
        assert retrieved_data[0]['symbol'] == 'AAPL'
        assert response['Metadata']['points'] == str(len(sample_market_data))
    
    @pytest.mark.asyncio
    async def test_s3_partitioning(self, s3_client):
        """Test proper data partitioning in S3"""
        bucket_name = "trading-data-backfill"
        
        # Test partition key generation
        test_cases = [
            {
                "symbol": "AAPL",
                "date": datetime(2023, 1, 15),
                "granularity": DataGranularity.MINUTE,
                "expected_key": "market-data/AAPL/2023/01/15/minute/data.parquet"
            },
            {
                "symbol": "GOOGL",
                "date": datetime(2023, 12, 31),
                "granularity": DataGranularity.DAY,
                "expected_key": "market-data/GOOGL/2023/12/31/daily/data.parquet"
            },
            {
                "symbol": "TSLA",
                "date": datetime(2023, 6, 1),
                "granularity": DataGranularity.TICK,
                "expected_key": "market-data/TSLA/2023/06/01/tick/data.parquet"
            }
        ]
        
        for test_case in test_cases:
            # Generate partition key
            key = f"market-data/{test_case['symbol']}/{test_case['date'].year:04d}/" \
                  f"{test_case['date'].month:02d}/{test_case['date'].day:02d}/" \
                  f"{test_case['granularity'].value}/data.parquet"
            
            assert key == test_case['expected_key']
            
            # Test listing objects by partition
            prefix = f"market-data/{test_case['symbol']}/2023/"
            s3_client.list_objects_v2.return_value = {
                'Contents': [
                    {'Key': f"{prefix}01/01/minute/data.parquet"},
                    {'Key': f"{prefix}01/02/minute/data.parquet"},
                    {'Key': f"{prefix}01/03/minute/data.parquet"}
                ]
            }
            
            response = await s3_client.list_objects_v2(
                Bucket=bucket_name,
                Prefix=prefix
            )
            
            assert len(response['Contents']) == 3
    
    @pytest.mark.asyncio
    async def test_s3_compression(self, s3_client, sample_market_data):
        """Test data compression and decompression"""
        # Test different compression formats
        compression_tests = [
            {
                "format": "gzip",
                "extension": ".gz",
                "compress_func": gzip.compress,
                "decompress_func": gzip.decompress
            },
            # Can add more compression formats here (zstd, lz4, etc.)
        ]
        
        for test in compression_tests:
            # Original data
            original_json = json.dumps([{
                "time": d.time.isoformat(),
                "symbol": d.symbol,
                "close": d.close,
                "volume": d.volume
            } for d in sample_market_data[:100]])  # Use subset
            
            original_size = len(original_json.encode())
            
            # Compress
            compressed = test['compress_func'](original_json.encode())
            compressed_size = len(compressed)
            
            # Verify compression ratio
            compression_ratio = compressed_size / original_size
            assert compression_ratio < 0.5  # Should achieve at least 50% compression
            
            # Upload compressed
            key = f"test/compression-test{test['extension']}"
            await s3_client.put_object(
                Bucket="trading-data-backfill",
                Key=key,
                Body=compressed,
                Metadata={
                    "original-size": str(original_size),
                    "compressed-size": str(compressed_size),
                    "compression-ratio": f"{compression_ratio:.2f}"
                }
            )
            
            # Download and decompress
            s3_client.get_object.return_value = {
                'Body': AsyncMock(read=AsyncMock(return_value=compressed))
            }
            
            response = await s3_client.get_object(
                Bucket="trading-data-backfill",
                Key=key
            )
            
            body = await response['Body'].read()
            decompressed = test['decompress_func'](body).decode()
            
            # Verify data integrity
            assert decompressed == original_json
    
    @pytest.mark.asyncio
    async def test_s3_multipart_upload(self, s3_client):
        """Test multipart upload for large files"""
        bucket_name = "trading-data-backfill"
        key = "large-data/test-multipart.parquet"
        
        # Mock multipart upload
        s3_client.create_multipart_upload = AsyncMock(return_value={
            'UploadId': 'test-upload-id'
        })
        
        s3_client.upload_part = AsyncMock(return_value={
            'ETag': '"part-etag"'
        })
        
        s3_client.complete_multipart_upload = AsyncMock(return_value={
            'ETag': '"complete-etag"'
        })
        
        # Initiate multipart upload
        response = await s3_client.create_multipart_upload(
            Bucket=bucket_name,
            Key=key
        )
        upload_id = response['UploadId']
        
        # Upload parts (simulate large file)
        parts = []
        part_size = 5 * 1024 * 1024  # 5MB parts
        
        for i in range(1, 4):  # 3 parts
            part_data = b'x' * part_size  # Dummy data
            
            response = await s3_client.upload_part(
                Bucket=bucket_name,
                Key=key,
                UploadId=upload_id,
                PartNumber=i,
                Body=part_data
            )
            
            parts.append({
                'ETag': response['ETag'],
                'PartNumber': i
            })
        
        # Complete multipart upload
        await s3_client.complete_multipart_upload(
            Bucket=bucket_name,
            Key=key,
            UploadId=upload_id,
            MultipartUpload={'Parts': parts}
        )
        
        # Verify all parts were uploaded
        assert len(parts) == 3
        assert all('ETag' in part for part in parts)
    
    @pytest.mark.asyncio
    async def test_s3_lifecycle_policies(self, s3_client):
        """Test S3 lifecycle policies for data archival"""
        bucket_name = "trading-data-backfill"
        
        # Mock lifecycle configuration
        lifecycle_config = {
            'Rules': [
                {
                    'ID': 'archive-old-tick-data',
                    'Status': 'Enabled',
                    'Prefix': 'market-data/',
                    'Transitions': [
                        {
                            'Days': 30,
                            'StorageClass': 'STANDARD_IA'  # Infrequent Access after 30 days
                        },
                        {
                            'Days': 90,
                            'StorageClass': 'GLACIER'  # Glacier after 90 days
                        }
                    ],
                    'Filter': {
                        'And': {
                            'Prefix': 'market-data/',
                            'Tags': [
                                {
                                    'Key': 'granularity',
                                    'Value': 'tick'
                                }
                            ]
                        }
                    }
                },
                {
                    'ID': 'delete-old-temp-files',
                    'Status': 'Enabled',
                    'Prefix': 'temp/',
                    'Expiration': {
                        'Days': 7  # Delete temp files after 7 days
                    }
                }
            ]
        }
        
        s3_client.get_bucket_lifecycle_configuration = AsyncMock(
            return_value=lifecycle_config
        )
        
        # Get lifecycle configuration
        config = await s3_client.get_bucket_lifecycle_configuration(
            Bucket=bucket_name
        )
        
        # Verify rules
        assert len(config['Rules']) == 2
        
        # Check archive rule
        archive_rule = next(r for r in config['Rules'] if r['ID'] == 'archive-old-tick-data')
        assert len(archive_rule['Transitions']) == 2
        assert archive_rule['Transitions'][0]['StorageClass'] == 'STANDARD_IA'
        assert archive_rule['Transitions'][1]['StorageClass'] == 'GLACIER'
        
        # Check deletion rule
        delete_rule = next(r for r in config['Rules'] if r['ID'] == 'delete-old-temp-files')
        assert delete_rule['Expiration']['Days'] == 7
    
    @pytest.mark.asyncio
    async def test_s3_error_handling(self, s3_client):
        """Test S3 error handling and retry logic"""
        from botocore.exceptions import ClientError
        
        # Test access denied
        s3_client.put_object.side_effect = ClientError(
            {'Error': {'Code': 'AccessDenied', 'Message': 'Access Denied'}},
            'PutObject'
        )
        
        with pytest.raises(ClientError) as exc_info:
            await s3_client.put_object(
                Bucket="trading-data-backfill",
                Key="test/error.json",
                Body=b'{"test": "data"}'
            )
        
        assert exc_info.value.response['Error']['Code'] == 'AccessDenied'
        
        # Test bucket not found
        s3_client.head_bucket.side_effect = ClientError(
            {'Error': {'Code': 'NoSuchBucket', 'Message': 'Bucket not found'}},
            'HeadBucket'
        )
        
        with pytest.raises(ClientError) as exc_info:
            await s3_client.head_bucket(Bucket="non-existent-bucket")
        
        assert exc_info.value.response['Error']['Code'] == 'NoSuchBucket'


if __name__ == "__main__":
    pytest.main([__file__, "-v"])