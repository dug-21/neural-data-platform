"""
Unit tests for BackfillJob functionality
"""
import pytest
from datetime import datetime, timedelta
from unittest.mock import Mock, patch
import hashlib

from data_ingestion.providers.historical_backfill import (
    BackfillJob, BackfillPriority, DataGranularity, BackfillStatus
)


class TestBackfillJob:
    """Test suite for BackfillJob class"""
    
    def test_job_id_generation(self):
        """Test unique job ID generation based on job parameters"""
        # Test auto-generation
        job1 = BackfillJob(
            job_id="",
            symbol="AAPL",
            provider="yahoo",
            start_date=datetime(2023, 1, 1),
            end_date=datetime(2023, 12, 31),
            priority=BackfillPriority.HIGH,
            granularity=DataGranularity.DAY
        )
        
        # Verify ID was generated
        assert job1.job_id != ""
        assert len(job1.job_id) == 12  # MD5 hash truncated to 12 chars
        
        # Test deterministic generation - same params = same ID
        job2 = BackfillJob(
            job_id="",
            symbol="AAPL",
            provider="yahoo", 
            start_date=datetime(2023, 1, 1),
            end_date=datetime(2023, 12, 31),
            priority=BackfillPriority.HIGH,
            granularity=DataGranularity.DAY
        )
        
        assert job1.job_id == job2.job_id
        
        # Test different params = different ID
        job3 = BackfillJob(
            job_id="",
            symbol="GOOGL",  # Different symbol
            provider="yahoo",
            start_date=datetime(2023, 1, 1),
            end_date=datetime(2023, 12, 31),
            priority=BackfillPriority.HIGH,
            granularity=DataGranularity.DAY
        )
        
        assert job1.job_id != job3.job_id
        
    def test_progress_calculation(self):
        """Test progress percentage calculation"""
        job = BackfillJob(
            job_id="test-123",
            symbol="AAPL",
            provider="yahoo",
            start_date=datetime(2023, 1, 1),
            end_date=datetime(2023, 1, 31),
            priority=BackfillPriority.MEDIUM,
            granularity=DataGranularity.DAY
        )
        
        # Test with known total points
        job.total_points_expected = 1000
        job.points_stored = 250
        job.update_progress()
        assert job.progress == 25.0
        
        # Test completion
        job.points_stored = 1000
        job.update_progress()
        assert job.progress == 100.0
        
        # Test with zero expected points (time-based)
        job.total_points_expected = 0
        job.start_time = datetime.now() - timedelta(hours=1)
        job.update_progress()
        assert 0 <= job.progress <= 100
        
    def test_retry_logic(self):
        """Test retry count and eligibility"""
        job = BackfillJob(
            job_id="test-retry",
            symbol="AAPL",
            provider="alpaca",
            start_date=datetime(2023, 1, 1),
            end_date=datetime(2023, 1, 31),
            priority=BackfillPriority.HIGH,
            granularity=DataGranularity.MINUTE
        )
        
        # Initial state - can retry
        assert job.retry_count == 0
        assert job.can_retry() is True
        
        # After retries
        job.retry_count = 1
        assert job.can_retry() is True
        
        job.retry_count = 2
        assert job.can_retry() is True
        
        # Max retries reached
        job.retry_count = 3
        assert job.can_retry() is False
        
        # Test custom max retries
        job.max_retries = 5
        job.retry_count = 4
        assert job.can_retry() is True
        
        job.retry_count = 5
        assert job.can_retry() is False
        
    def test_completion_time_estimation(self):
        """Test ETA calculation accuracy"""
        job = BackfillJob(
            job_id="test-eta",
            symbol="TSLA",
            provider="polygon",
            start_date=datetime.now() - timedelta(days=30),
            end_date=datetime.now(),
            priority=BackfillPriority.CRITICAL,
            granularity=DataGranularity.TICK
        )
        
        # No progress yet
        eta = job.estimate_completion_time()
        assert eta is None
        
        # With progress
        job.start_time = datetime.now() - timedelta(minutes=10)
        job.progress = 25.0  # 25% done in 10 minutes
        
        eta = job.estimate_completion_time()
        assert eta is not None
        
        # Should complete in ~30 more minutes (40 total - 10 elapsed)
        remaining_minutes = (eta - datetime.now()).total_seconds() / 60
        assert 25 <= remaining_minutes <= 35  # Allow some tolerance
        
    def test_serialization_deserialization(self):
        """Test job state persistence"""
        from data_ingestion.providers.historical_backfill import HistoricalBackfillCoordinator
        
        coordinator = HistoricalBackfillCoordinator()
        
        # Create job with all fields populated
        job = BackfillJob(
            job_id="test-serial",
            symbol="NVDA",
            provider="iex",
            start_date=datetime(2023, 1, 1),
            end_date=datetime(2023, 12, 31),
            priority=BackfillPriority.LOW,
            granularity=DataGranularity.HOUR,
            status=BackfillStatus.RUNNING,
            progress=45.5,
            total_points_expected=10000,
            points_loaded=4550,
            points_validated=4500,
            points_stored=4500,
            data_quality_score=0.98,
            gaps_found=5,
            gaps_filled=3,
            invalid_points=50,
            start_time=datetime.now() - timedelta(hours=2),
            fetch_duration_ms=120000,
            validation_duration_ms=30000,
            storage_duration_ms=45000,
            error="Temporary network issue",
            error_count=2,
            last_error_time=datetime.now() - timedelta(minutes=30),
            retry_count=1,
            last_checkpoint=datetime.now() - timedelta(minutes=5),
            checkpoint_data={"last_processed": "2023-06-15"}
        )
        
        # Serialize
        serialized = coordinator._serialize_job(job)
        
        # Verify serialization
        assert isinstance(serialized, dict)
        assert serialized['job_id'] == 'test-serial'
        assert serialized['symbol'] == 'NVDA'
        assert serialized['priority'] == BackfillPriority.LOW.value
        assert serialized['granularity'] == DataGranularity.HOUR.value
        assert serialized['status'] == BackfillStatus.RUNNING.value
        assert serialized['progress'] == 45.5
        
        # Deserialize
        deserialized = coordinator._deserialize_job(serialized)
        
        # Verify deserialization
        assert isinstance(deserialized, BackfillJob)
        assert deserialized.job_id == job.job_id
        assert deserialized.symbol == job.symbol
        assert deserialized.priority == job.priority
        assert deserialized.granularity == job.granularity
        assert deserialized.status == job.status
        assert deserialized.progress == job.progress
        assert deserialized.checkpoint_data == job.checkpoint_data
        
    def test_job_status_transitions(self):
        """Test valid status transitions"""
        job = BackfillJob(
            job_id="test-status",
            symbol="AMD",
            provider="yahoo",
            start_date=datetime(2023, 1, 1),
            end_date=datetime(2023, 1, 31),
            priority=BackfillPriority.MEDIUM,
            granularity=DataGranularity.DAY
        )
        
        # Initial status
        assert job.status == BackfillStatus.PENDING
        
        # Valid transitions
        job.status = BackfillStatus.RUNNING
        assert job.status == BackfillStatus.RUNNING
        
        job.status = BackfillStatus.COMPLETED
        assert job.status == BackfillStatus.COMPLETED
        
        # Test failure and retry
        job.status = BackfillStatus.FAILED
        job.error = "API rate limit exceeded"
        job.error_count = 1
        
        assert job.status == BackfillStatus.FAILED
        assert job.can_retry() is True
        
        job.status = BackfillStatus.RETRYING
        assert job.status == BackfillStatus.RETRYING
        
    def test_performance_metrics(self):
        """Test performance metric tracking"""
        job = BackfillJob(
            job_id="test-perf",
            symbol="MSFT",
            provider="alpaca",
            start_date=datetime(2023, 1, 1),
            end_date=datetime(2023, 1, 7),
            priority=BackfillPriority.HIGH,
            granularity=DataGranularity.MINUTE
        )
        
        # Simulate job execution
        job.start_time = datetime.now()
        job.status = BackfillStatus.RUNNING
        
        # Update metrics
        job.fetch_duration_ms = 5000  # 5 seconds
        job.validation_duration_ms = 1000  # 1 second
        job.storage_duration_ms = 2000  # 2 seconds
        
        total_duration = (job.fetch_duration_ms + 
                         job.validation_duration_ms + 
                         job.storage_duration_ms)
        
        assert total_duration == 8000  # 8 seconds total
        
        # Calculate throughput
        job.points_stored = 2340  # 6 days * 390 points/day
        throughput = job.points_stored / (total_duration / 1000)  # points per second
        
        assert throughput == 292.5  # 2340 points / 8 seconds


if __name__ == "__main__":
    pytest.main([__file__, "-v"])