"""Test suite for Reddit sentiment provider."""
import pytest
from unittest.mock import Mock, patch, AsyncMock, MagicMock
from datetime import datetime, timedelta
import asyncio
from typing import List, Dict

# Import directly to avoid loading all providers
import sys
import os
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))))

from data_ingestion.providers.reddit import RedditProvider, RedditSentiment


class TestRedditProvider:
    """Test cases for Reddit provider implementation."""
    
    @pytest.fixture
    def mock_settings(self):
        """Mock settings for testing."""
        settings = Mock()
        settings.reddit_client_id = "test_client_id"
        settings.reddit_client_secret = "test_secret"
        settings.reddit_user_agent = "test_agent/1.0"
        settings.max_concurrent_requests = 5
        settings.max_requests_per_minute = 60
        return settings
    
    @pytest.fixture
    def provider(self, mock_settings):
        """Create Reddit provider instance with mocked settings."""
        with patch('data_ingestion.providers.reddit.get_settings', return_value=mock_settings):
            return RedditProvider()
    
    @pytest.mark.asyncio
    async def test_provider_initialization(self, provider):
        """Test provider initializes correctly."""
        assert provider.name == "Reddit"
        assert provider.subreddits == ["wallstreetbets", "stocks", "investing", "options"]
        assert not provider._connected
    
    @pytest.mark.asyncio
    async def test_connect_success(self, provider):
        """Test successful connection to Reddit API."""
        # Mock asyncpraw Reddit instance
        mock_reddit = AsyncMock()
        mock_user = AsyncMock()
        mock_user.me = AsyncMock(return_value=Mock(name="test_user"))
        mock_reddit.user = mock_user
        
        with patch('asyncpraw.Reddit', return_value=mock_reddit):
            await provider.connect()
            
            assert provider._connected is True
            assert provider._reddit == mock_reddit
    
    @pytest.mark.asyncio
    async def test_disconnect(self, provider):
        """Test provider disconnection."""
        # Setup connected provider
        provider._reddit = AsyncMock()
        provider._connected = True
        
        await provider.disconnect()
        
        provider._reddit.close.assert_called_once()
        assert provider._connected is False
    
    @pytest.mark.asyncio
    async def test_get_ticker_mentions(self, provider):
        """Test extracting ticker mentions from Reddit posts."""
        # Mock Reddit submissions
        mock_submissions = [
            Mock(
                title="$AAPL to the moon! 🚀",
                selftext="I think $AAPL and $MSFT are great buys",
                score=100,
                num_comments=50,
                created_utc=1704067200.0
            ),
            Mock(
                title="Why I'm buying $TSLA calls",
                selftext="$TSLA $TSLA $TSLA!!!",
                score=200,
                num_comments=75,
                created_utc=1704067300.0
            )
        ]
        
        # Mock subreddit
        mock_subreddit = AsyncMock()
        mock_subreddit.new = AsyncMock(return_value=AsyncMock(__aiter__=lambda x: iter(mock_submissions)))
        
        provider._reddit = AsyncMock()
        provider._reddit.subreddit = AsyncMock(return_value=mock_subreddit)
        provider._connected = True
        
        # Get mentions
        mentions = []
        async for mention in provider.get_ticker_mentions("wallstreetbets", limit=2):
            mentions.append(mention)
        
        # Verify results
        assert len(mentions) == 2
        
        # Check first post
        assert mentions[0].ticker in ["AAPL", "MSFT"]
        assert mentions[0].score == 100
        assert mentions[0].subreddit == "wallstreetbets"
        
        # Check second post  
        assert mentions[1].ticker == "TSLA"
        assert mentions[1].score == 200
        assert mentions[1].mention_count == 3  # TSLA mentioned 3 times
    
    @pytest.mark.asyncio
    async def test_ticker_extraction_regex(self, provider):
        """Test ticker symbol extraction from text."""
        test_cases = [
            ("Buy $AAPL now!", ["AAPL"]),
            ("$MSFT and $GOOGL are my picks", ["MSFT", "GOOGL"]),
            ("I like TSLA stock", ["TSLA"]),
            ("$SPY puts printing", ["SPY"]),
            ("Random text without tickers", []),
            ("$A $BB $CCC $DDDD $EEEEE", ["BB", "CCC", "DDDD"]),  # Valid lengths only
            ("crypto $BTC is not a stock", [])  # Should filter out crypto
        ]
        
        for text, expected in test_cases:
            tickers = provider._extract_tickers(text)
            assert set(tickers) == set(expected), f"Failed for: {text}"
    
    @pytest.mark.asyncio
    async def test_sentiment_analysis(self, provider):
        """Test sentiment scoring of posts."""
        test_posts = [
            Mock(title="$AAPL calls printing money! 🚀🚀🚀", selftext="To the moon!", score=500),
            Mock(title="$MSFT puts - this is going down", selftext="Sell everything", score=100),
            Mock(title="Neutral post about $GOOGL", selftext="Just sharing info", score=50)
        ]
        
        sentiments = []
        for post in test_posts:
            sentiment = provider._analyze_sentiment(post)
            sentiments.append(sentiment)
        
        # Bullish post should have positive sentiment
        assert sentiments[0] > 0.5
        
        # Bearish post should have negative sentiment  
        assert sentiments[1] < -0.3
        
        # Neutral post should be near zero
        assert -0.2 < sentiments[2] < 0.2
    
    @pytest.mark.asyncio
    async def test_aggregate_sentiment_by_ticker(self, provider):
        """Test aggregating sentiment across multiple mentions."""
        # Mock multiple posts mentioning same tickers
        mock_submissions = [
            Mock(title="$AAPL bullish!", selftext="", score=100, num_comments=10, created_utc=1704067200.0),
            Mock(title="$AAPL bearish", selftext="", score=50, num_comments=5, created_utc=1704067300.0),
            Mock(title="$TSLA rocket", selftext="", score=200, num_comments=20, created_utc=1704067400.0)
        ]
        
        mock_subreddit = AsyncMock()
        mock_subreddit.hot = AsyncMock(return_value=AsyncMock(__aiter__=lambda x: iter(mock_submissions)))
        
        provider._reddit = AsyncMock()
        provider._reddit.subreddit = AsyncMock(return_value=mock_subreddit)
        provider._connected = True
        
        # Get aggregated sentiment
        sentiment_data = await provider.get_aggregated_sentiment(
            subreddits=["wallstreetbets"],
            time_window_hours=24
        )
        
        # Check results
        assert "AAPL" in sentiment_data
        assert "TSLA" in sentiment_data
        
        # AAPL should have mixed sentiment (bullish + bearish)
        assert sentiment_data["AAPL"]["mention_count"] == 2
        assert -0.3 < sentiment_data["AAPL"]["avg_sentiment"] < 0.3
        
        # TSLA should be bullish
        assert sentiment_data["TSLA"]["mention_count"] == 1
        assert sentiment_data["TSLA"]["avg_sentiment"] > 0.5
    
    @pytest.mark.asyncio
    async def test_rate_limiting(self, provider):
        """Test rate limiting for Reddit API."""
        provider._request_count = 59  # Just under limit
        provider._last_request_time = 0
        
        with patch('asyncio.get_event_loop') as mock_loop:
            mock_loop.return_value.time.return_value = 30  # 30 seconds elapsed
            
            await provider._rate_limit()
            assert provider._request_count == 60
            
            # Next request should trigger rate limit
            with patch('asyncio.sleep') as mock_sleep:
                await provider._rate_limit()
                mock_sleep.assert_called_once()
    
    @pytest.mark.asyncio 
    async def test_search_posts(self, provider):
        """Test searching for posts by query."""
        mock_results = [
            Mock(
                title="DD on $AAPL",
                selftext="Analysis here",
                score=1000,
                num_comments=200,
                created_utc=1704067200.0,
                author=Mock(name="test_user"),
                subreddit=Mock(display_name="stocks")
            )
        ]
        
        mock_subreddit = AsyncMock()
        mock_subreddit.search = AsyncMock(return_value=AsyncMock(__aiter__=lambda x: iter(mock_results)))
        
        provider._reddit = AsyncMock()
        provider._reddit.subreddit = AsyncMock(return_value=mock_subreddit)
        provider._connected = True
        
        # Search for AAPL posts
        results = []
        async for post in provider.search_ticker_posts("AAPL", limit=1):
            results.append(post)
        
        assert len(results) == 1
        assert results[0]["title"] == "DD on $AAPL"
        assert results[0]["score"] == 1000
        assert results[0]["author"] == "test_user"
    
    @pytest.mark.asyncio
    async def test_get_trending_tickers(self, provider):
        """Test getting trending tickers from hot posts."""
        # Mock posts with various tickers
        mock_submissions = [
            Mock(title="$GME squeeze!", selftext="$GME $GME", score=5000, num_comments=1000, created_utc=1704067200.0),
            Mock(title="$AMC following $GME", selftext="", score=3000, num_comments=500, created_utc=1704067300.0),
            Mock(title="$AAPL earnings", selftext="", score=1000, num_comments=100, created_utc=1704067400.0),
            Mock(title="$GME update", selftext="", score=2000, num_comments=300, created_utc=1704067500.0)
        ]
        
        mock_subreddit = AsyncMock()
        mock_subreddit.hot = AsyncMock(return_value=AsyncMock(__aiter__=lambda x: iter(mock_submissions)))
        
        provider._reddit = AsyncMock()
        provider._reddit.subreddit = AsyncMock(return_value=mock_subreddit)
        provider._connected = True
        
        # Get trending
        trending = await provider.get_trending_tickers(limit=3)
        
        # GME should be #1 (mentioned 3 times with high scores)
        assert trending[0]["ticker"] == "GME"
        assert trending[0]["total_mentions"] == 3
        
        # AMC should be #2
        assert trending[1]["ticker"] == "AMC"
        
        # AAPL should be #3
        assert trending[2]["ticker"] == "AAPL"
    
    @pytest.mark.asyncio
    async def test_connection_required(self, provider):
        """Test that methods fail when not connected."""
        provider._connected = False
        
        with pytest.raises(RuntimeError) as exc_info:
            async for _ in provider.get_ticker_mentions("wallstreetbets"):
                pass
        
        assert "Not connected" in str(exc_info.value)
    
    @pytest.mark.asyncio
    async def test_filter_blacklisted_tickers(self, provider):
        """Test filtering of blacklisted tickers."""
        # Add common false positives to blacklist
        provider.ticker_blacklist = {"DD", "CEO", "IPO", "FDA", "EPS", "PE"}
        
        text = "DD: Why I think $AAPL will beat EPS estimates. CEO is solid."
        tickers = provider._extract_tickers(text)
        
        # Should only extract AAPL, not DD/CEO/EPS
        assert tickers == ["AAPL"]