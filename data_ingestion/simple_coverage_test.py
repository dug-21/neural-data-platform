#!/usr/bin/env python3
"""
Simple isolated test to check Alpaca provider code coverage.
This bypasses the complex import chain and directly tests the provider.
"""

import sys
import os
import unittest
from unittest.mock import Mock, patch
from datetime import datetime, timedelta

# Direct import to test coverage
sys.path.insert(0, '/workspaces/neural-trader/data_ingestion')

def test_alpaca_provider_coverage():
    """Direct test of AlpacaProvider without complex dependencies."""
    
    print("🔄 Testing AlpacaProvider coverage...")
    
    # Mock the settings to avoid import issues
    mock_settings = Mock()
    mock_settings.alpaca_api_key = "test_key"
    mock_settings.alpaca_api_secret = "test_secret"
    mock_settings.alpaca_subscription_level = "basic"
    mock_settings.max_concurrent_requests = 10
    mock_settings.max_requests_per_minute = 200
    
    with patch('config.get_settings', return_value=mock_settings):
        from providers.alpaca import AlpacaProvider
        
        # Test initialization
        provider = AlpacaProvider()
        assert provider.name == "alpaca"
        assert provider.api_key == "test_key"
        assert provider.api_secret == "test_secret"
        assert provider.subscription_level == "basic"
        print("✅ Initialization tests passed")
        
        # Test interval mapping
        intervals = provider.INTERVAL_MAP
        assert "1min" in intervals
        assert "1day" in intervals
        assert len(intervals) >= 9
        print("✅ Interval mapping tests passed")
        
        # Test subscription limits
        basic_limits = provider.SUBSCRIPTION_LIMITS["basic"]
        unlimited_limits = provider.SUBSCRIPTION_LIMITS["unlimited"]
        assert basic_limits["websocket_symbols"] == 30
        assert unlimited_limits["websocket_symbols"] is None
        print("✅ Subscription limits tests passed")
        
        # Test parsing functions with mock data
        from alpaca.data.models import Bar, Trade, Quote
        from alpaca.data.enums import DataFeed
        
        # Test _parse_bar
        mock_bar = Mock()
        mock_bar.timestamp = datetime.now()
        mock_bar.open = 150.0
        mock_bar.high = 151.0
        mock_bar.low = 149.0
        mock_bar.close = 150.5
        mock_bar.volume = 1000000
        mock_bar.trade_count = 500
        mock_bar.vwap = 150.25
        
        market_data = provider._parse_bar(mock_bar, "AAPL")
        assert market_data.symbol == "AAPL"
        assert market_data.close == 150.5
        assert market_data.volume == 1000000
        print("✅ Bar parsing tests passed")
        
        # Test _parse_trade
        mock_trade = Mock()
        mock_trade.timestamp = datetime.now()
        mock_trade.price = 150.05
        mock_trade.size = 100
        mock_trade.exchange = "V"
        mock_trade.conditions = ["@", "I"]
        
        tick_data = provider._parse_trade(mock_trade, "AAPL")
        assert tick_data.symbol == "AAPL"
        assert tick_data.price == 150.05
        assert tick_data.size == 100
        print("✅ Trade parsing tests passed")
        
        # Test _parse_quote
        mock_quote = Mock()
        mock_quote.timestamp = datetime.now()
        mock_quote.bid_price = 150.0
        mock_quote.ask_price = 150.5
        mock_quote.bid_size = 100
        mock_quote.ask_size = 200
        
        order_book = provider._parse_quote(mock_quote, "AAPL")
        assert order_book.symbol == "AAPL"
        assert order_book.bid_price == 150.0
        assert order_book.ask_price == 150.5
        assert order_book.spread == 0.5
        print("✅ Quote parsing tests passed")
        
        print("🎉 All basic tests passed!")
        return True

def run_coverage_analysis():
    """Run coverage analysis on the simplified test."""
    import subprocess
    
    print("📊 Running coverage analysis...")
    
    # Create a temporary test file
    test_content = '''
import sys
sys.path.insert(0, '/workspaces/neural-trader/data_ingestion')

from unittest.mock import Mock, patch
from datetime import datetime

def test_all_alpaca_functions():
    """Test as many AlpacaProvider functions as possible."""
    
    mock_settings = Mock()
    mock_settings.alpaca_api_key = "test_key"
    mock_settings.alpaca_api_secret = "test_secret"  
    mock_settings.alpaca_subscription_level = "basic"
    mock_settings.max_concurrent_requests = 10
    mock_settings.max_requests_per_minute = 200
    
    with patch('config.get_settings', return_value=mock_settings):
        from providers.alpaca import AlpacaProvider
        
        provider = AlpacaProvider()
        
        # Test all basic properties and methods
        assert provider.name == "alpaca"
        assert provider.api_key == "test_key"
        assert provider.subscription_level == "basic"
        
        # Test interval mapping
        for interval in ["1min", "5min", "1hour", "1day"]:
            assert interval in provider.INTERVAL_MAP
        
        # Test subscription limits
        basic_limits = provider.SUBSCRIPTION_LIMITS["basic"]
        unlimited_limits = provider.SUBSCRIPTION_LIMITS["unlimited"]
        assert basic_limits["websocket_symbols"] == 30
        assert unlimited_limits["websocket_symbols"] is None
        
        # Test parsing functions
        mock_bar = Mock()
        mock_bar.timestamp = datetime.now()
        mock_bar.open = 150.0
        mock_bar.high = 151.0
        mock_bar.low = 149.0
        mock_bar.close = 150.5
        mock_bar.volume = 1000000
        mock_bar.trade_count = 500
        mock_bar.vwap = 150.25
        
        market_data = provider._parse_bar(mock_bar, "AAPL")
        assert market_data.symbol == "AAPL"
        
        mock_trade = Mock()
        mock_trade.timestamp = datetime.now()
        mock_trade.price = 150.05
        mock_trade.size = 100
        mock_trade.exchange = "V"
        mock_trade.conditions = ["@", "I"]
        
        tick_data = provider._parse_trade(mock_trade, "AAPL")
        assert tick_data.symbol == "AAPL"
        
        mock_quote = Mock()
        mock_quote.timestamp = datetime.now()
        mock_quote.bid_price = 150.0
        mock_quote.ask_price = 150.5
        mock_quote.bid_size = 100
        mock_quote.ask_size = 200
        
        order_book = provider._parse_quote(mock_quote, "AAPL")
        assert order_book.symbol == "AAPL"
        
        print("All tests passed!")

if __name__ == "__main__":
    test_all_alpaca_functions()
'''
    
    # Write the test file
    with open('/workspaces/neural-trader/data_ingestion/temp_coverage_test.py', 'w') as f:
        f.write(test_content)
    
    try:
        # Run coverage
        cmd = [
            'python', '-m', 'coverage', 'run', '--source=providers.alpaca',
            'temp_coverage_test.py'
        ]
        
        result = subprocess.run(cmd, cwd='/workspaces/neural-trader/data_ingestion',
                              capture_output=True, text=True)
        
        if result.returncode == 0:
            # Generate coverage report
            report_cmd = ['python', '-m', 'coverage', 'report', '-m']
            report_result = subprocess.run(report_cmd, 
                                         cwd='/workspaces/neural-trader/data_ingestion',
                                         capture_output=True, text=True)
            
            print("📊 COVERAGE REPORT:")
            print("=" * 50)
            print(report_result.stdout)
            print("=" * 50)
            
            return True
        else:
            print(f"❌ Coverage test failed: {result.stderr}")
            return False
            
    except Exception as e:
        print(f"💥 Error running coverage: {e}")
        return False
    finally:
        # Clean up
        try:
            os.remove('/workspaces/neural-trader/data_ingestion/temp_coverage_test.py')
        except:
            pass

if __name__ == "__main__":
    success = test_alpaca_provider_coverage()
    if success:
        print("\n" + "="*50)
        print("🎯 Running detailed coverage analysis...")
        run_coverage_analysis()
    
    print("\n" + "="*50)
    print("📋 COVERAGE ANALYSIS SUMMARY:")
    print("Current coverage is significantly below 85% target.")
    print("Key missing coverage areas identified:")
    print("- Connection/disconnection methods")
    print("- WebSocket streaming functions") 
    print("- Error handling paths")
    print("- Historical vs real-time data detection")
    print("- API client initialization")
    print("\n🔧 RECOMMENDATIONS:")
    print("1. Add mocking for Alpaca SDK clients")
    print("2. Test connection error scenarios")
    print("3. Test data retrieval methods with mock responses")
    print("4. Test streaming data polling loops")
    print("5. Test all error handling branches")
    print("="*50)