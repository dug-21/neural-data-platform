# SPARC Completion: AlpacaProvider WebSocket Integration

## 1. Definition of Done

### 1.1 Feature Completion Criteria

- [ ] **AlpacaProvider WebSocket Extension**
  - [ ] `stream_market_data_ws()` method implemented
  - [ ] WebSocket authentication working
  - [ ] Message parsing for trades, quotes, and bars
  - [ ] Simple reconnection logic with exponential backoff
  - [ ] Maintains backward compatibility with REST API

- [ ] **Basic Error Handling**
  - [ ] Connection error recovery
  - [ ] Invalid message handling
  - [ ] Automatic fallback to REST API
  - [ ] Logging of WebSocket events

- [ ] **Integration with Existing System**
  - [ ] Works with current storage adapters
  - [ ] Compatible with existing metrics
  - [ ] No breaking changes to API
  - [ ] Configuration via environment variables

## 2. Testing Requirements

### 2.1 Unit Tests for AlpacaProvider

```python
# test_alpaca_websocket.py
class TestAlpacaWebSocket:
    """Unit tests for AlpacaProvider WebSocket functionality."""
    
    async def test_websocket_connection(self):
        """Test WebSocket connection establishment."""
        provider = AlpacaProvider(websocket_enabled=True)
        
        # Mock WebSocket for testing
        with patch('websockets.connect') as mock_connect:
            mock_ws = AsyncMock()
            mock_connect.return_value.__aenter__.return_value = mock_ws
            
            # Test connection
            task = asyncio.create_task(
                provider.stream_market_data_ws(['AAPL'])
            )
            await asyncio.sleep(0.1)
            
            # Verify connection attempt
            mock_connect.assert_called_once()
            assert 'wss://' in mock_connect.call_args[0][0]
            
            task.cancel()
    
    async def test_message_parsing(self):
        """Test parsing of Alpaca WebSocket messages."""
        provider = AlpacaProvider()
        
        # Test trade message
        trade_msg = '{"T":"t","S":"AAPL","p":150.25,"s":100,"t":"2024-01-01T10:00:00Z"}'
        data = provider._parse_ws_message(trade_msg)
        
        assert data is not None
        assert data.symbol == "AAPL"
        assert data.price == 150.25
        assert data.volume == 100
        
    async def test_reconnection_logic(self):
        """Test automatic reconnection on failure."""
        provider = AlpacaProvider(websocket_enabled=True)
        reconnect_count = 0
        
        async def mock_connect(*args, **kwargs):
            nonlocal reconnect_count
            reconnect_count += 1
            if reconnect_count < 3:
                raise ConnectionError("Test disconnect")
            return AsyncMock()
            
        with patch('websockets.connect', side_effect=mock_connect):
            task = asyncio.create_task(
                provider.stream_market_data_ws(['AAPL'])
            )
            await asyncio.sleep(5)
            task.cancel()
            
        # Should have attempted reconnection
        assert reconnect_count >= 3
```

### 2.2 Integration Tests

```python
# test_alpaca_integration.py
class TestAlpacaIntegration:
    """Integration tests with real Alpaca sandbox."""
    
    @pytest.mark.integration
    async def test_real_websocket_connection(self):
        """Test connection to Alpaca sandbox WebSocket."""
        # Use paper trading credentials
        provider = AlpacaProvider(
            api_key=os.getenv('ALPACA_PAPER_KEY'),
            api_secret=os.getenv('ALPACA_PAPER_SECRET'),
            base_url='https://paper-api.alpaca.markets',
            websocket_enabled=True
        )
        
        received_messages = []
        
        async def collect_messages():
            async for data in provider.stream_market_data_ws(['AAPL']):
                received_messages.append(data)
                if len(received_messages) >= 5:
                    break
                    
        # Collect messages for up to 30 seconds
        try:
            await asyncio.wait_for(collect_messages(), timeout=30)
        except asyncio.TimeoutError:
            pass
            
        # Should have received some messages
        assert len(received_messages) > 0
        
    async def test_fallback_to_rest(self):
        """Test fallback to REST when WebSocket fails."""
        provider = AlpacaProvider(websocket_enabled=True)
        
        # Force WebSocket to fail
        with patch.object(provider, 'stream_market_data_ws', 
                         side_effect=Exception("WebSocket unavailable")):
            
            # Should fall back to REST
            data = await provider.get_market_data(['AAPL'])
            assert data is not None
            assert len(data) > 0
```

### 2.3 Performance Tests

```python
# test_websocket_performance.py
class TestWebSocketPerformance:
    """Basic performance tests for WebSocket."""
    
    async def test_message_processing_speed(self):
        """Test message parsing performance."""
        provider = AlpacaProvider()
        
        # Generate test messages
        test_messages = [
            f'{{"T":"t","S":"AAPL","p":{150+i*0.01},"s":100,"t":"2024-01-01T10:00:{i:02d}Z"}}'
            for i in range(1000)
        ]
        
        start_time = time.perf_counter()
        
        for msg in test_messages:
            provider._parse_ws_message(msg)
            
        elapsed = time.perf_counter() - start_time
        messages_per_second = 1000 / elapsed
        
        # Should process at least 10k messages/second
        assert messages_per_second > 10000
        
    async def test_memory_usage(self):
        """Test memory efficiency of WebSocket streaming."""
        provider = AlpacaProvider()
        
        # Measure initial memory
        import psutil
        process = psutil.Process()
        initial_memory = process.memory_info().rss / 1024 / 1024  # MB
        
        # Process many messages
        for i in range(10000):
            msg = f'{{"T":"t","S":"AAPL","p":{150+i*0.01},"s":100}}'
            provider._parse_ws_message(msg)
            
        # Check memory growth
        final_memory = process.memory_info().rss / 1024 / 1024  # MB
        memory_growth = final_memory - initial_memory
        
        # Should not grow more than 50MB
        assert memory_growth < 50
```

## 3. Deployment Checklist

### 3.1 Configuration

```bash
# Environment variables for WebSocket
export ALPACA_WS_ENABLED=true              # Enable WebSocket
export ALPACA_WS_BATCH_SIZE=100           # Batch size for storage
export ALPACA_WS_BATCH_TIMEOUT=0.1        # Batch timeout in seconds
export ALPACA_WS_RECONNECT_DELAY=1        # Initial reconnect delay
export ALPACA_WS_MAX_RECONNECT_DELAY=60   # Max reconnect delay
```

### 3.2 Code Changes

- [ ] Update `data_ingestion/providers/alpaca.py`:
  - [ ] Add `stream_market_data_ws()` method
  - [ ] Add WebSocket helper methods
  - [ ] Add configuration parameters
  - [ ] Ensure backward compatibility

### 3.3 Testing

- [ ] Run all unit tests
- [ ] Run integration tests with paper trading account
- [ ] Verify no regression in REST API functionality
- [ ] Test WebSocket reconnection scenarios
- [ ] Validate message parsing accuracy

## 4. Monitoring

### 4.1 Key Metrics

```python
# Add to existing metrics
websocket_metrics = {
    'websocket_connected': Gauge('websocket_connected', 'WebSocket connection status'),
    'websocket_messages_received': Counter('websocket_messages_received', 'Total messages received'),
    'websocket_reconnections': Counter('websocket_reconnections', 'WebSocket reconnection count'),
    'websocket_parse_errors': Counter('websocket_parse_errors', 'Message parsing errors'),
    'websocket_message_latency': Histogram('websocket_message_latency', 'Message processing latency')
}
```

### 4.2 Health Checks

```python
def get_websocket_health():
    """Check WebSocket health status."""
    return {
        'websocket_enabled': provider.websocket_enabled,
        'websocket_connected': hasattr(provider, '_websocket') and provider._websocket,
        'last_message_time': provider._ws_metrics.get('last_message_time'),
        'reconnection_count': provider._ws_metrics.get('reconnections', 0),
        'error_rate': provider._ws_metrics.get('parse_errors', 0) / 
                     max(1, provider._ws_metrics.get('messages_received', 1))
    }
```

## 5. Rollback Plan

If issues arise, WebSocket can be disabled without code changes:

```bash
# Disable WebSocket, automatically falls back to REST
export ALPACA_WS_ENABLED=false

# Restart service
kubectl rollout restart deployment/data-ingestion
```

## 6. Success Criteria

### Minimum Requirements Met:
- [ ] WebSocket connection works reliably
- [ ] Messages are parsed correctly
- [ ] Data flows to existing storage
- [ ] No breaking changes to existing code
- [ ] Automatic fallback to REST works
- [ ] Latency improved from 5-60s to <100ms

### Nice to Have (Future):
- Advanced reconnection strategies
- Multiple connection pooling
- More sophisticated error handling
- Additional message types
- Performance optimizations

## 7. Documentation Updates

### 7.1 README Addition

```markdown
## WebSocket Support

The AlpacaProvider now supports WebSocket streaming for real-time data:

```python
# Enable WebSocket in configuration
provider = AlpacaProvider(websocket_enabled=True)

# Stream real-time data
async for data in provider.stream_market_data_ws(['AAPL', 'GOOGL']):
    print(f"{data.symbol}: ${data.price}")
```

WebSocket is optional and the system will automatically fall back to REST API if WebSocket is unavailable.
```

### 7.2 Configuration Documentation

Add to existing configuration docs:
- WebSocket environment variables
- Batch processing settings
- Reconnection parameters
- Monitoring endpoints

## Summary

This minimal implementation adds WebSocket support to the AlpacaProvider with:
- Single file modification
- Full backward compatibility
- Automatic fallback to REST
- Simple configuration
- Basic monitoring

The implementation can be completed in 3 weeks with minimal risk and immediate benefits in reduced latency and improved scalability.