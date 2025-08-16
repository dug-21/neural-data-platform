"""Test suite for channel validation and circuit breaker functionality."""
import pytest
import asyncio
from unittest.mock import Mock, AsyncMock, patch
from data_ingestion.utils.channel_validator import ChannelValidator, CircuitBreaker, CircuitBreakerOpenError


class TestChannelValidator:
    """Test channel validation per INTERFACE_CONTRACT requirements."""
    
    def test_validate_channel_name_valid(self):
        """Test valid channel names pass validation."""
        valid_channels = [
            "market:AAPL",
            "market:NVDA", 
            "market:MSFT",
            "market:GOOGL",
            "market:TSLA",
            "market:META",
            "market:A",      # Single character
            "market:ABCDE"   # Five characters
        ]
        
        for channel in valid_channels:
            assert ChannelValidator.validate_channel_name(channel), f"Should be valid: {channel}"
    
    def test_validate_channel_name_invalid(self):
        """Test invalid channel names fail validation."""
        invalid_channels = [
            "market:aapl",      # Lowercase
            "market:AAPL.US",   # Special characters
            "market:AAPL-USD",  # Dashes
            "market:123",       # Numbers
            "market:ABCDEF",    # Too long (6 chars)
            "market:",          # Empty symbol
            "MARKET:AAPL",      # Wrong prefix case
            "markets:AAPL",     # Wrong prefix
            "market:AA PL",     # Space in symbol
        ]
        
        for channel in invalid_channels:
            assert not ChannelValidator.validate_channel_name(channel), f"Should be invalid: {channel}"
    
    def test_validate_symbol_valid(self):
        """Test valid symbols are normalized correctly."""
        test_cases = [
            ("AAPL", "AAPL"),
            ("aapl", "AAPL"),
            (" NVDA ", "NVDA"),
            ("msft", "MSFT"),
            ("A", "A"),
            ("ABCDE", "ABCDE"),
        ]
        
        for input_symbol, expected in test_cases:
            result = ChannelValidator.validate_symbol(input_symbol)
            assert result == expected, f"Expected {expected}, got {result}"
    
    def test_validate_symbol_invalid(self):
        """Test invalid symbols return None."""
        invalid_symbols = [
            "",
            None,
            "AAPL.US",
            "AAPL-USD", 
            "123",
            "ABCDEF",    # Too long
            "AA PL",     # Space
        ]
        
        for symbol in invalid_symbols:
            result = ChannelValidator.validate_symbol(symbol)
            assert result is None, f"Should be None for: {symbol}"
    
    def test_create_symbol_channel_valid(self):
        """Test creating valid symbol channels."""
        test_cases = [
            ("AAPL", "market:AAPL"),
            ("aapl", "market:AAPL"),
            (" nvda ", "market:NVDA"),
        ]
        
        for symbol, expected_channel in test_cases:
            result = ChannelValidator.create_symbol_channel(symbol)
            assert result == expected_channel, f"Expected {expected_channel}, got {result}"
    
    def test_create_symbol_channel_invalid(self):
        """Test creating channels for invalid symbols returns None."""
        invalid_symbols = ["", "AAPL.US", "123", "ABCDEF"]
        
        for symbol in invalid_symbols:
            result = ChannelValidator.create_symbol_channel(symbol)
            assert result is None, f"Should be None for: {symbol}"


class TestCircuitBreaker:
    """Test circuit breaker functionality."""
    
    def test_circuit_breaker_initial_state(self):
        """Test circuit breaker starts in CLOSED state."""
        cb = CircuitBreaker()
        assert cb.state == "CLOSED"
        assert cb.allow_request("test_channel") is True
    
    def test_circuit_breaker_success_recording(self):
        """Test success recording keeps circuit closed."""
        cb = CircuitBreaker(failure_threshold=3)
        
        # Record some failures (but under threshold)
        cb.record_failure("test_channel")
        cb.record_failure("test_channel")
        assert cb.state == "CLOSED"
        
        # Record success - should reset failure count
        cb.record_success("test_channel")
        assert cb.failure_count == 0
        assert cb.state == "CLOSED"
    
    def test_circuit_breaker_opens_on_threshold(self):
        """Test circuit breaker opens when failure threshold reached."""
        cb = CircuitBreaker(failure_threshold=3)
        
        # Record failures up to threshold
        cb.record_failure("test_channel")
        cb.record_failure("test_channel")
        assert cb.state == "CLOSED"
        
        # This should open the circuit
        cb.record_failure("test_channel")
        assert cb.state == "OPEN"
        assert cb.allow_request("test_channel") is False
    
    def test_circuit_breaker_half_open_transition(self):
        """Test circuit breaker transitions to HALF_OPEN after timeout."""
        cb = CircuitBreaker(failure_threshold=2, recovery_timeout=0.1)
        
        # Open the circuit
        cb.record_failure("test_channel")
        cb.record_failure("test_channel")
        assert cb.state == "OPEN"
        
        # Should still be blocked immediately
        assert cb.allow_request("test_channel") is False
        
        # Wait for recovery timeout
        import time
        time.sleep(0.15)
        
        # Should transition to HALF_OPEN and allow request
        assert cb.allow_request("test_channel") is True
        assert cb.state == "HALF_OPEN"
    
    def test_circuit_breaker_half_open_recovery(self):
        """Test circuit breaker closes from HALF_OPEN on success."""
        cb = CircuitBreaker(failure_threshold=2)
        
        # Open circuit and force to HALF_OPEN
        cb.record_failure("test_channel")
        cb.record_failure("test_channel")
        cb.state = "HALF_OPEN"  # Force state for testing
        
        # Record success should close circuit
        cb.record_success("test_channel")
        assert cb.state == "CLOSED"
        assert cb.failure_count == 0


@pytest.mark.asyncio
class TestRealtimeCoordinatorChannelIntegration:
    """Integration tests for channel validation in RealtimeCoordinator."""
    
    async def test_channel_validation_integration(self):
        """Test channel validation is properly integrated."""
        # This would require a full RealtimeCoordinator setup
        # For now, test the components in isolation
        validator = ChannelValidator()
        
        # Test valid symbol processing
        valid_channel = validator.create_symbol_channel("AAPL")
        assert valid_channel == "market:AAPL"
        
        # Test invalid symbol processing
        invalid_channel = validator.create_symbol_channel("INVALID.SYMBOL")
        assert invalid_channel is None


# Performance tests for SUCCESS_CRITERIA validation
class TestPerformanceRequirements:
    """Test performance requirements from SUCCESS_CRITERIA."""
    
    def test_channel_validation_performance(self):
        """Test channel validation meets performance requirements."""
        import time
        validator = ChannelValidator()
        
        # Test batch validation performance
        symbols = ["AAPL", "NVDA", "MSFT", "GOOGL", "TSLA"] * 200  # 1000 symbols
        
        start_time = time.time()
        for symbol in symbols:
            validator.create_symbol_channel(symbol)
        duration = time.time() - start_time
        
        # Should process 1000 symbols in well under 1 second
        assert duration < 0.1, f"Channel validation too slow: {duration}s for 1000 symbols"
    
    def test_circuit_breaker_performance(self):
        """Test circuit breaker doesn't add significant overhead."""
        import time
        cb = CircuitBreaker()
        
        # Test performance of allow_request calls
        start_time = time.time()
        for _ in range(10000):
            cb.allow_request("test_channel")
        duration = time.time() - start_time
        
        # Should handle 10k requests in under 0.1 seconds
        assert duration < 0.1, f"Circuit breaker too slow: {duration}s for 10k requests"


if __name__ == "__main__":
    pytest.main([__file__, "-v"])