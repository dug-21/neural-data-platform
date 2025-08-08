"""
Phase 2 Channel Validation Tests
Tests for INTERFACE_CONTRACT compliance and dual publishing functionality
"""
import pytest
import json
import asyncio
from unittest.mock import AsyncMock, MagicMock, patch
from typing import Dict, Any

# Import test subject
from utils.channel_validator import ChannelValidator, CircuitBreaker
from config import Settings, get_settings


class TestChannelValidation:
    """Test channel naming validation per INTERFACE_CONTRACT."""
    
    def test_validate_channel_name_valid_symbols(self):
        """Test valid channel names pass validation."""
        valid_channels = [
            "market:AAPL",
            "market:MSFT", 
            "market:GOOGL",
            "market:NVDA",
            "market:TSLA",
            "market:META",
            "market:AMZN",
            "market:A",     # Single letter
            "market:ABCDE", # Five letters
        ]
        
        for channel in valid_channels:
            assert ChannelValidator.validate_channel_name(channel), f"Should be valid: {channel}"
    
    def test_validate_channel_name_invalid_symbols(self):
        """Test invalid channel names fail validation."""
        invalid_channels = [
            "market:aapl",      # Lowercase
            "market:ABCDEF",    # Too long (6 letters)
            "market:",          # Empty symbol
            "market:AAPL.US",   # Special characters
            "market:MSFT-USD",  # Dash
            "wrong:AAPL",       # Wrong prefix
            "AAPL",             # No prefix
            "market:123",       # Numbers
            "market:A1",        # Mixed alphanumeric
        ]
        
        for channel in invalid_channels:
            assert not ChannelValidator.validate_channel_name(channel), f"Should be invalid: {channel}"
    
    def test_validate_symbol_normalization(self):
        """Test symbol validation and normalization."""
        test_cases = [
            ("AAPL", "AAPL"),    # Already uppercase
            ("aapl", "AAPL"),    # Lowercase to uppercase
            (" MSFT ", "MSFT"),  # Whitespace trimmed
            ("", None),          # Empty string
            ("123", None),       # Numbers only
            ("ABCDEF", None),    # Too long
            ("A@#$", None),      # Special characters
        ]
        
        for input_symbol, expected in test_cases:
            result = ChannelValidator.validate_symbol(input_symbol)
            assert result == expected, f"Input: {input_symbol}, Expected: {expected}, Got: {result}"
    
    def test_create_symbol_channel(self):
        """Test symbol channel creation."""
        test_cases = [
            ("AAPL", "market:AAPL"),
            ("aapl", "market:AAPL"),  # Normalization
            (" tsla ", "market:TSLA"),  # Trimmed and normalized
            ("", None),               # Invalid symbol
            ("ABCDEF", None),         # Too long
        ]
        
        for input_symbol, expected in test_cases:
            result = ChannelValidator.create_symbol_channel(input_symbol)
            assert result == expected, f"Input: {input_symbol}, Expected: {expected}, Got: {result}"


class TestCircuitBreaker:
    """Test circuit breaker functionality."""
    
    def test_circuit_breaker_closed_state(self):
        """Test circuit breaker allows requests when closed."""
        cb = CircuitBreaker(failure_threshold=5, recovery_timeout=30)
        assert cb.state == "CLOSED"
        assert cb.allow_request("test_channel") == True
    
    def test_circuit_breaker_failure_tracking(self):
        """Test circuit breaker tracks failures correctly."""
        cb = CircuitBreaker(failure_threshold=3, recovery_timeout=30)
        
        # Record failures
        cb.record_failure("test_channel")
        assert cb.failure_count == 1
        assert cb.state == "CLOSED"
        
        cb.record_failure("test_channel")
        assert cb.failure_count == 2
        assert cb.state == "CLOSED"
        
        cb.record_failure("test_channel")
        assert cb.failure_count == 3
        assert cb.state == "OPEN"
    
    def test_circuit_breaker_success_reset(self):
        """Test circuit breaker resets on success."""
        cb = CircuitBreaker(failure_threshold=5, recovery_timeout=30)
        
        # Record some failures
        cb.record_failure("test_channel")
        cb.record_failure("test_channel")
        assert cb.failure_count == 2
        
        # Success should reset
        cb.record_success("test_channel")
        assert cb.failure_count == 0
        assert cb.state == "CLOSED"
    
    def test_circuit_breaker_half_open_transition(self):
        """Test circuit breaker half-open state transition."""
        cb = CircuitBreaker(failure_threshold=2, recovery_timeout=0.1)  # Short timeout for testing
        
        # Trip the circuit breaker
        cb.record_failure("test_channel")
        cb.record_failure("test_channel")
        assert cb.state == "OPEN"
        assert cb.allow_request("test_channel") == False
        
        # Wait for recovery timeout
        import time
        time.sleep(0.2)
        
        # Should transition to half-open
        assert cb.allow_request("test_channel") == True
        assert cb.state == "HALF_OPEN"
        
        # Success in half-open should close circuit
        cb.record_success("test_channel")
        assert cb.state == "CLOSED"


class TestPhase2Configuration:
    """Test Phase 2 configuration settings."""
    
    def test_default_phase2_settings(self):
        """Test Phase 2 settings have correct defaults."""
        settings = Settings()
        
        assert settings.enable_legacy_channel == True
        assert settings.redis_channel_prefix == "market"
        assert settings.redis_dual_publish == True
        assert settings.redis_max_connections == 50
        assert settings.redis_publish_timeout == 5
        assert settings.redis_decode_responses == True
    
    def test_phase2_environment_override(self):
        """Test Phase 2 settings can be overridden by environment."""
        import os
        original_env = {}
        
        try:
            # Set test environment variables
            test_env = {
                "ENABLE_LEGACY_CHANNEL": "false",
                "REDIS_CHANNEL_PREFIX": "test_market",
                "REDIS_DUAL_PUBLISH": "false",
                "REDIS_MAX_CONNECTIONS": "100",
                "REDIS_PUBLISH_TIMEOUT": "10",
            }
            
            for key, value in test_env.items():
                original_env[key] = os.environ.get(key)
                os.environ[key] = value
            
            # Create new settings instance
            settings = Settings()
            
            assert settings.enable_legacy_channel == False
            assert settings.redis_channel_prefix == "test_market"
            assert settings.redis_dual_publish == False
            assert settings.redis_max_connections == 100
            assert settings.redis_publish_timeout == 10
            
        finally:
            # Restore original environment
            for key, value in original_env.items():
                if value is None:
                    os.environ.pop(key, None)
                else:
                    os.environ[key] = value


@pytest.mark.asyncio
class TestDualPublishing:
    """Test dual publishing functionality."""
    
    async def test_dual_publishing_both_channels(self):
        """Test that dual publishing works for both channels."""
        from schedulers.realtime_coordinator import RealtimeCoordinator
        
        # Mock dependencies
        coordinator = RealtimeCoordinator()
        coordinator.redis = AsyncMock()
        coordinator.timescale = AsyncMock()
        coordinator.settings = Settings(enable_legacy_channel=True)
        
        # Mock validator and circuit breaker
        coordinator.channel_validator = MagicMock()
        coordinator.channel_validator.validate_channel_name.return_value = True
        coordinator.circuit_breaker = MagicMock()
        coordinator.circuit_breaker.allow_request.return_value = True
        
        # Test data
        test_data = {
            'symbol': 'AAPL',
            'price': 150.0,
            'volume': 1000,
            'time': '2025-08-08T15:30:00Z'
        }
        
        # Process data
        await coordinator._process_market_data(test_data, 'test_provider')
        
        # Verify dual publishing
        publish_calls = coordinator.redis.publish.call_args_list
        
        # Should have 3 calls: market_data:AAPL, market:AAPL, market:updates
        assert len(publish_calls) == 3
        
        # Check specific channels
        channels_called = [call[0][0] for call in publish_calls]
        assert "market_data:AAPL" in channels_called
        assert "market:AAPL" in channels_called
        assert "market:updates" in channels_called
    
    async def test_legacy_channel_disabled(self):
        """Test that legacy channel is skipped when disabled."""
        from schedulers.realtime_coordinator import RealtimeCoordinator
        
        # Mock dependencies
        coordinator = RealtimeCoordinator()
        coordinator.redis = AsyncMock()
        coordinator.timescale = AsyncMock()
        coordinator.settings = Settings(enable_legacy_channel=False)
        
        # Mock validator and circuit breaker
        coordinator.channel_validator = MagicMock()
        coordinator.channel_validator.validate_channel_name.return_value = True
        coordinator.circuit_breaker = MagicMock()
        coordinator.circuit_breaker.allow_request.return_value = True
        
        # Test data
        test_data = {
            'symbol': 'NVDA',
            'price': 450.0,
            'volume': 2000,
            'time': '2025-08-08T15:30:00Z'
        }
        
        # Process data
        await coordinator._process_market_data(test_data, 'test_provider')
        
        # Verify publishing
        publish_calls = coordinator.redis.publish.call_args_list
        
        # Should have 2 calls: market_data:NVDA, market:NVDA (no legacy channel)
        assert len(publish_calls) == 2
        
        # Check specific channels
        channels_called = [call[0][0] for call in publish_calls]
        assert "market_data:NVDA" in channels_called
        assert "market:NVDA" in channels_called
        assert "market:updates" not in channels_called
    
    async def test_circuit_breaker_blocks_publishing(self):
        """Test that circuit breaker blocks publishing when open."""
        from schedulers.realtime_coordinator import RealtimeCoordinator
        
        # Mock dependencies
        coordinator = RealtimeCoordinator()
        coordinator.redis = AsyncMock()
        coordinator.timescale = AsyncMock()
        coordinator.settings = Settings(enable_legacy_channel=True)
        
        # Mock validator and circuit breaker (circuit open)
        coordinator.channel_validator = MagicMock()
        coordinator.channel_validator.validate_channel_name.return_value = True
        coordinator.circuit_breaker = MagicMock()
        coordinator.circuit_breaker.allow_request.return_value = False  # Circuit open
        
        # Test data
        test_data = {
            'symbol': 'TSLA',
            'price': 250.0,
            'volume': 1500,
            'time': '2025-08-08T15:30:00Z'
        }
        
        # Process data
        await coordinator._process_market_data(test_data, 'test_provider')
        
        # Verify publishing
        publish_calls = coordinator.redis.publish.call_args_list
        
        # Should only have 1 call: market_data:TSLA (no market:TSLA or market:updates due to circuit breaker)
        assert len(publish_calls) == 1
        
        # Check specific channel
        channels_called = [call[0][0] for call in publish_calls]
        assert "market_data:TSLA" in channels_called
        assert "market:TSLA" not in channels_called
        assert "market:updates" not in channels_called


if __name__ == "__main__":
    pytest.main([__file__])