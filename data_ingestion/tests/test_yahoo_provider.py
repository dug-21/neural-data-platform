"""
Tests for Yahoo Finance Data Provider

This module contains comprehensive tests for the Yahoo Finance provider,
including market data, company information, options, and recommendations.
"""

import pytest
import pandas as pd
from datetime import datetime, timedelta
from unittest.mock import Mock, patch, MagicMock, AsyncMock
import yfinance as yf
import asyncio

from ..providers.yahoo_finance import YahooFinanceProvider
from ..providers.base import MarketData


class TestYahooFinanceProvider:
    """Test suite for Yahoo Finance provider."""
    
    @pytest.fixture
    def provider(self):
        """Create a Yahoo Finance provider instance."""
        return YahooFinanceProvider()
    
    @pytest.fixture
    def mock_ticker_info(self):
        """Mock ticker info data."""
        return {
            'symbol': 'AAPL',
            'regularMarketPrice': 150.00,
            'regularMarketOpen': 149.00,
            'regularMarketDayHigh': 151.00,
            'regularMarketDayLow': 148.00,
            'regularMarketVolume': 50000000,
            'previousClose': 148.50,
            'marketCap': 2500000000000,
            'trailingPE': 25.5,
            'dividendYield': 0.005,
            'fiftyTwoWeekHigh': 180.00,
            'fiftyTwoWeekLow': 120.00,
            'averageVolume': 45000000,
            'beta': 1.2,
            'currency': 'USD',
            'longName': 'Apple Inc.',
            'shortName': 'Apple',
            'sector': 'Technology',
            'industry': 'Consumer Electronics',
            'country': 'United States',
            'website': 'https://www.apple.com',
            'longBusinessSummary': 'Apple Inc. designs, manufactures, and markets smartphones...',
            'fullTimeEmployees': 150000,
            'exchange': 'NMS'
        }
    
    @pytest.fixture
    def mock_historical_data(self):
        """Mock historical data DataFrame."""
        dates = pd.date_range(end=datetime.now(), periods=5, freq='D')
        return pd.DataFrame({
            'Open': [148, 149, 150, 151, 152],
            'High': [149, 150, 151, 152, 153],
            'Low': [147, 148, 149, 150, 151],
            'Close': [148.5, 149.5, 150.5, 151.5, 152.5],
            'Volume': [50000000, 51000000, 52000000, 53000000, 54000000]
        }, index=dates)
    
    def test_initialization(self, provider):
        """Test provider initialization."""
        assert provider.name == "yahoo_finance"
        assert provider._executor is not None
        assert not provider._connected
        assert provider.session is None
    
    @pytest.mark.asyncio
    async def test_connect_disconnect(self, provider):
        """Test connection and disconnection."""
        # Test connection
        await provider.connect()
        assert provider._connected
        assert provider.session is not None
        
        # Test disconnection
        await provider.disconnect()
        assert not provider._connected
    
    @pytest.mark.asyncio
    @patch('yfinance.Ticker')
    async def test_get_market_data(self, mock_ticker_class, provider, mock_historical_data):
        """Test market data retrieval."""
        # Setup mock
        mock_ticker = Mock()
        mock_ticker.history.return_value = mock_historical_data
        mock_ticker_class.return_value = mock_ticker
        
        # Test data retrieval
        symbols = ['AAPL']
        start_time = datetime.now() - timedelta(days=5)
        end_time = datetime.now()
        
        market_data = []
        async for data in provider.get_market_data(symbols, start_time, end_time):
            market_data.append(data)
        
        # Assertions
        assert len(market_data) == 5  # 5 days of data
        assert all(isinstance(data, MarketData) for data in market_data)
        assert all(data.symbol == 'AAPL' for data in market_data)
        assert all(data.provider == 'yahoo_finance' for data in market_data)
        assert market_data[0].close == 148.5
        assert market_data[-1].close == 152.5
    
    @pytest.mark.asyncio
    @patch('yfinance.Ticker')
    async def test_get_company_info(self, mock_ticker_class, provider, mock_ticker_info):
        """Test company information retrieval."""
        # Setup mock
        mock_ticker = Mock()
        mock_ticker.info = mock_ticker_info
        mock_ticker_class.return_value = mock_ticker
        
        # Test company info
        info = await provider.get_company_info('AAPL')
        
        # Assertions
        assert info['symbol'] == 'AAPL'
        assert info['name'] == 'Apple Inc.'
        assert info['sector'] == 'Technology'
        assert info['industry'] == 'Consumer Electronics'
        assert info['country'] == 'United States'
        assert info['currency'] == 'USD'
    
    @pytest.mark.asyncio
    @patch('yfinance.Ticker')
    async def test_get_options_chain(self, mock_ticker_class, provider):
        """Test options chain retrieval."""
        # Create mock options data
        mock_calls = pd.DataFrame({
            'strike': [140, 145, 150, 155, 160],
            'lastPrice': [12.5, 8.2, 4.5, 1.8, 0.5],
            'volume': [1000, 2000, 5000, 3000, 1500],
            'openInterest': [5000, 8000, 15000, 10000, 6000]
        })
        mock_puts = pd.DataFrame({
            'strike': [140, 145, 150, 155, 160],
            'lastPrice': [0.5, 1.2, 3.5, 6.8, 11.5],
            'volume': [500, 1500, 4000, 2500, 1000],
            'openInterest': [3000, 6000, 12000, 8000, 4000]
        })
        
        # Setup mock
        mock_ticker = Mock()
        mock_ticker.options = ['2024-01-19', '2024-01-26', '2024-02-02']
        mock_option_chain = Mock()
        mock_option_chain.calls = mock_calls
        mock_option_chain.puts = mock_puts
        mock_ticker.option_chain.return_value = mock_option_chain
        mock_ticker_class.return_value = mock_ticker
        
        # Test options chain
        options = await provider.get_options_chain('AAPL')
        
        # Assertions
        assert 'symbol' in options
        assert 'expirations' in options
        assert 'calls' in options
        assert 'puts' in options
        assert len(options['expirations']) == 3
        assert len(options['calls']) == 5
        assert len(options['puts']) == 5
    
    @pytest.mark.asyncio
    @patch('yfinance.Ticker')
    async def test_get_recommendations(self, mock_ticker_class, provider):
        """Test recommendations retrieval."""
        # Create mock recommendations
        dates = pd.date_range(end=datetime.now(), periods=5, freq='M')
        mock_recommendations = pd.DataFrame({
            'Firm': ['Morgan Stanley', 'Goldman Sachs', 'JP Morgan', 'Bank of America', 'Citigroup'],
            'To Grade': ['Buy', 'Buy', 'Hold', 'Buy', 'Buy'],
            'From Grade': ['Hold', 'Buy', 'Hold', 'Hold', 'Buy'],
            'Action': ['up', 'main', 'main', 'up', 'main']
        }, index=dates)
        
        # Setup mock
        mock_ticker = Mock()
        mock_ticker.recommendations = mock_recommendations
        mock_ticker_class.return_value = mock_ticker
        
        # Test recommendations
        recommendations = await provider.get_recommendations('AAPL')
        
        # Assertions
        assert isinstance(recommendations, list)
        assert len(recommendations) == 5
        assert all('Firm' in rec for rec in recommendations)
        assert all('To Grade' in rec for rec in recommendations)
    
    @pytest.mark.asyncio
    @patch('yfinance.Ticker')
    async def test_get_institutional_holders(self, mock_ticker_class, provider):
        """Test institutional holders retrieval."""
        # Create mock institutional holders
        mock_holders = pd.DataFrame({
            'Holder': ['Vanguard Group', 'BlackRock', 'Berkshire Hathaway', 'State Street', 'Fidelity'],
            'Shares': [1300000000, 1100000000, 900000000, 600000000, 500000000],
            'Date Reported': pd.date_range(end=datetime.now(), periods=5, freq='Q'),
            '% Out': [8.2, 6.9, 5.7, 3.8, 3.1],
            'Value': [195000000000, 165000000000, 135000000000, 90000000000, 75000000000]
        })
        
        # Setup mock
        mock_ticker = Mock()
        mock_ticker.institutional_holders = mock_holders
        mock_ticker_class.return_value = mock_ticker
        
        # Test institutional holders
        holders = await provider.get_institutional_holders('AAPL')
        
        # Assertions
        assert isinstance(holders, list)
        assert len(holders) == 5
        assert all('Holder' in holder for holder in holders)
        assert all('Shares' in holder for holder in holders)
    
    @pytest.mark.asyncio
    @patch('yfinance.Ticker')
    async def test_get_earnings(self, mock_ticker_class, provider):
        """Test earnings data retrieval."""
        # Create mock earnings data
        mock_annual_earnings = pd.DataFrame({
            'Revenue': [365000000000, 380000000000, 395000000000, 410000000000],
            'Earnings': [95000000000, 99000000000, 103000000000, 107000000000]
        }, index=pd.date_range(end=datetime.now(), periods=4, freq='Y'))
        
        mock_quarterly_earnings = pd.DataFrame({
            'Revenue': [90000000000, 95000000000, 100000000000, 105000000000],
            'Earnings': [25000000000, 27000000000, 28000000000, 30000000000]
        }, index=pd.date_range(end=datetime.now(), periods=4, freq='Q'))
        
        # Setup mock
        mock_ticker = Mock()
        mock_ticker.earnings = mock_annual_earnings
        mock_ticker.quarterly_earnings = mock_quarterly_earnings
        mock_ticker_class.return_value = mock_ticker
        
        # Test earnings
        earnings = await provider.get_earnings('AAPL')
        
        # Assertions
        assert 'symbol' in earnings
        assert 'annual' in earnings
        assert 'quarterly' in earnings
        assert len(earnings['annual']) == 4
        assert len(earnings['quarterly']) == 4
    
    @pytest.mark.asyncio
    @patch('yfinance.Ticker')
    async def test_stream_market_data_single_iteration(self, mock_ticker_class, provider, mock_ticker_info):
        """Test streaming market data (single iteration)."""
        # Setup mock
        mock_ticker = Mock()
        mock_ticker.info = mock_ticker_info
        mock_ticker_class.return_value = mock_ticker
        
        # Test streaming (just one iteration)
        symbols = ['AAPL']
        stream = provider.stream_market_data(symbols)
        
        # Get first data point
        data = await stream.__anext__()
        
        # Assertions
        assert isinstance(data, MarketData)
        assert data.symbol == 'AAPL'
        assert data.provider == 'yahoo_finance'
        assert data.close == 150.00
        assert data.volume == 50000000
    
    @pytest.mark.asyncio
    @patch('yfinance.Ticker')
    async def test_parse_quote_data(self, mock_ticker_class, provider, mock_ticker_info):
        """Test quote data parsing."""
        # Test parsing
        parsed_data = provider._parse_quote_data(mock_ticker_info, 'AAPL')
        
        # Assertions
        assert isinstance(parsed_data, MarketData)
        assert parsed_data.symbol == 'AAPL'
        assert parsed_data.open == 149.00
        assert parsed_data.high == 151.00
        assert parsed_data.low == 148.00
        assert parsed_data.close == 150.00
        assert parsed_data.volume == 50000000
        assert parsed_data.provider == 'yahoo_finance'
        assert parsed_data.metadata['market_cap'] == 2500000000000
        assert parsed_data.metadata['pe_ratio'] == 25.5
        assert parsed_data.metadata['currency'] == 'USD'
    
    @pytest.mark.asyncio
    @patch('yfinance.Ticker')
    async def test_error_handling(self, mock_ticker_class, provider):
        """Test error handling."""
        # Setup mock to raise exception
        mock_ticker_class.side_effect = Exception("API Error")
        
        # Test company info error handling
        info = await provider.get_company_info('INVALID')
        assert info == {}
        
        # Test options chain error handling
        options = await provider.get_options_chain('INVALID')
        assert options == {}
        
        # Test recommendations error handling
        recommendations = await provider.get_recommendations('INVALID')
        assert recommendations == []
    
    @pytest.mark.asyncio
    @patch('yfinance.Ticker')
    async def test_empty_data_handling(self, mock_ticker_class, provider):
        """Test handling of empty data responses."""
        # Setup mock with empty data
        mock_ticker = Mock()
        mock_ticker.history.return_value = pd.DataFrame()
        mock_ticker.recommendations = None
        mock_ticker.institutional_holders = pd.DataFrame()
        mock_ticker.earnings = pd.DataFrame()
        mock_ticker.quarterly_earnings = pd.DataFrame()
        mock_ticker_class.return_value = mock_ticker
        
        # Test market data with empty response
        symbols = ['INVALID']
        start_time = datetime.now() - timedelta(days=5)
        end_time = datetime.now()
        
        market_data = []
        async for data in provider.get_market_data(symbols, start_time, end_time):
            market_data.append(data)
        
        assert len(market_data) == 0
        
        # Test recommendations with None response
        recommendations = await provider.get_recommendations('INVALID')
        assert recommendations == []
        
        # Test institutional holders with empty DataFrame
        holders = await provider.get_institutional_holders('INVALID')
        assert holders == []
        
        # Test earnings with empty DataFrames
        earnings = await provider.get_earnings('INVALID')
        assert earnings == {'symbol': 'INVALID'}
    
    def test_interval_mapping(self, provider):
        """Test interval mapping."""
        assert provider.INTERVAL_MAP['1min'] == '1m'
        assert provider.INTERVAL_MAP['5min'] == '5m'
        assert provider.INTERVAL_MAP['1hour'] == '60m'
        assert provider.INTERVAL_MAP['1day'] == '1d'
        assert provider.INTERVAL_MAP['1week'] == '1wk'
        assert provider.INTERVAL_MAP['1month'] == '1mo'
    
    def test_fetch_data_sync_intraday_limit(self, provider):
        """Test intraday data date limit."""
        # Test with date more than 60 days ago
        start_date = datetime.now() - timedelta(days=90)
        end_date = datetime.now()
        
        with patch('yfinance.Ticker') as mock_ticker_class:
            mock_ticker = Mock()
            mock_ticker.history.return_value = pd.DataFrame()
            mock_ticker_class.return_value = mock_ticker
            
            # Test intraday interval
            provider._fetch_data_sync('AAPL', start_date, end_date, '1m')
            
            # Check that start date was adjusted
            call_args = mock_ticker.history.call_args
            adjusted_start = call_args[1]['start']
            assert (datetime.now() - adjusted_start).days <= 60


# Integration tests (marked for skipping in CI)
@pytest.mark.integration
class TestYahooFinanceProviderIntegration:
    """Integration tests that make real API calls."""
    
    @pytest.fixture
    def provider(self):
        """Create a Yahoo Finance provider instance for integration tests."""
        return YahooFinanceProvider()
    
    @pytest.mark.asyncio
    async def test_real_market_data_retrieval(self, provider):
        """Test real API call for market data."""
        await provider.connect()
        
        try:
            symbols = ['AAPL']
            start_time = datetime.now() - timedelta(days=5)
            end_time = datetime.now()
            
            market_data = []
            async for data in provider.get_market_data(symbols, start_time, end_time):
                market_data.append(data)
                if len(market_data) >= 3:  # Just test a few data points
                    break
            
            assert len(market_data) >= 3
            assert all(isinstance(data, MarketData) for data in market_data)
            assert all(data.symbol == 'AAPL' for data in market_data)
            assert all(data.close > 0 for data in market_data)
        finally:
            await provider.disconnect()
    
    @pytest.mark.asyncio
    async def test_real_company_info(self, provider):
        """Test real API call for company information."""
        await provider.connect()
        
        try:
            info = await provider.get_company_info('AAPL')
            
            assert info['symbol'] == 'AAPL'
            assert 'Apple' in info.get('name', '')
            assert info.get('sector') == 'Technology'
            assert info.get('country') == 'United States'
        finally:
            await provider.disconnect()
    
    @pytest.mark.asyncio
    async def test_real_options_chain(self, provider):
        """Test real API call for options chain."""
        await provider.connect()
        
        try:
            options = await provider.get_options_chain('AAPL')
            
            if options:  # Options might not be available for all symbols
                assert 'symbol' in options
                assert 'expirations' in options
                assert 'calls' in options
                assert 'puts' in options
        finally:
            await provider.disconnect()