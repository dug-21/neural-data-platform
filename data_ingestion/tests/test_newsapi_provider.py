"""Tests for NewsAPI provider."""
import pytest
import asyncio
from datetime import datetime, timedelta
from unittest.mock import AsyncMock, MagicMock, patch
import aiohttp

from data_ingestion.providers.newsapi import NewsAPIProvider, NewsArticle


@pytest.fixture
def mock_settings():
    """Mock settings with API key."""
    settings = MagicMock()
    settings.newsapi_key = "test_api_key"
    settings.max_concurrent_requests = 10
    settings.max_requests_per_minute = 100
    return settings


@pytest.fixture
async def provider(mock_settings):
    """Create NewsAPI provider instance."""
    with patch('data_ingestion.providers.base.get_settings', return_value=mock_settings):
        provider = NewsAPIProvider()
        yield provider
        if provider._connected:
            await provider.disconnect()


@pytest.fixture
def sample_article():
    """Sample article response from NewsAPI."""
    return {
        "source": {"id": "bloomberg", "name": "Bloomberg"},
        "author": "John Doe",
        "title": "Tech Stocks Surge on Strong Earnings",
        "description": "Technology stocks rallied after major companies beat earnings expectations",
        "url": "https://example.com/article",
        "urlToImage": "https://example.com/image.jpg",
        "publishedAt": "2024-01-15T10:00:00Z",
        "content": "Full article content here..."
    }


@pytest.fixture
def sample_negative_article():
    """Sample negative sentiment article."""
    return {
        "source": {"id": "reuters", "name": "Reuters"},
        "author": "Jane Smith",
        "title": "Markets Crash as Concerns Rise",
        "description": "Stock markets plunge amid growing economic concerns and weak outlook",
        "url": "https://example.com/article2",
        "urlToImage": None,
        "publishedAt": "2024-01-15T12:00:00Z",
        "content": "Markets fall sharply with losses across all sectors..."
    }


class TestNewsAPIProvider:
    """Test NewsAPI provider functionality."""
    
    @pytest.mark.asyncio
    async def test_connect(self, provider):
        """Test provider connection."""
        await provider.connect()
        assert provider._connected is True
        assert provider.session is not None
        
    @pytest.mark.asyncio
    async def test_connect_no_api_key(self, mock_settings):
        """Test connection fails without API key."""
        mock_settings.newsapi_key = None
        with patch('data_ingestion.providers.base.get_settings', return_value=mock_settings):
            provider = NewsAPIProvider()
            with pytest.raises(ValueError, match="NewsAPI key not configured"):
                await provider.connect()
    
    @pytest.mark.asyncio
    async def test_disconnect(self, provider):
        """Test provider disconnection."""
        await provider.connect()
        await provider.disconnect()
        assert provider._connected is False
        
    def test_calculate_sentiment_positive(self, provider):
        """Test positive sentiment calculation."""
        text = "Stock surges on strong earnings beat, rallies to record high"
        score, label = provider._calculate_sentiment(text)
        assert score > 0.3
        assert label == "positive"
        
    def test_calculate_sentiment_negative(self, provider):
        """Test negative sentiment calculation."""
        text = "Markets crash as losses mount, concerns grow over weak outlook"
        score, label = provider._calculate_sentiment(text)
        assert score < -0.3
        assert label == "negative"
        
    def test_calculate_sentiment_neutral(self, provider):
        """Test neutral sentiment calculation."""
        text = "Company announces quarterly results"
        score, label = provider._calculate_sentiment(text)
        assert -0.3 <= score <= 0.3
        assert label == "neutral"
        
    def test_calculate_sentiment_empty(self, provider):
        """Test sentiment calculation with empty text."""
        score, label = provider._calculate_sentiment("")
        assert score == 0.0
        assert label == "neutral"
        
    def test_parse_article(self, provider, sample_article):
        """Test article parsing."""
        article = provider._parse_article(sample_article)
        
        assert isinstance(article, NewsArticle)
        assert article.title == "Tech Stocks Surge on Strong Earnings"
        assert article.source == "Bloomberg"
        assert article.author == "John Doe"
        assert article.sentiment_label == "positive"
        assert article.sentiment_score > 0
        assert article.time.year == 2024
        
    def test_parse_article_negative(self, provider, sample_negative_article):
        """Test parsing article with negative sentiment."""
        article = provider._parse_article(sample_negative_article)
        
        assert article.sentiment_label == "negative"
        assert article.sentiment_score < 0
        assert article.image_url is None
        
    @pytest.mark.asyncio
    async def test_get_top_headlines(self, provider, sample_article):
        """Test getting top headlines."""
        await provider.connect()
        
        mock_response = {
            "status": "ok",
            "articles": [sample_article]
        }
        
        with patch.object(provider, '_request', return_value=mock_response) as mock_request:
            articles = []
            async for article in provider.get_top_headlines(category="business"):
                articles.append(article)
                
            assert len(articles) == 1
            assert articles[0].title == "Tech Stocks Surge on Strong Earnings"
            
            # Verify request parameters
            mock_request.assert_called_once()
            call_args = mock_request.call_args
            assert call_args[0][0] == "top-headlines"
            assert call_args[1]['params']['category'] == "business"
            
    @pytest.mark.asyncio
    async def test_get_everything(self, provider, sample_article):
        """Test searching all articles."""
        await provider.connect()
        
        mock_response = {
            "status": "ok",
            "articles": [sample_article]
        }
        
        from_date = datetime.now() - timedelta(days=7)
        to_date = datetime.now()
        
        with patch.object(provider, '_request', return_value=mock_response) as mock_request:
            articles = []
            async for article in provider.get_everything(
                q="AAPL",
                from_date=from_date,
                to_date=to_date
            ):
                articles.append(article)
                
            assert len(articles) == 1
            
            # Verify request parameters
            call_args = mock_request.call_args
            params = call_args[1]['params']
            assert params['q'] == "AAPL"
            assert 'from' in params
            assert 'to' in params
            
    @pytest.mark.asyncio
    async def test_get_sources(self, provider):
        """Test getting news sources."""
        await provider.connect()
        
        mock_response = {
            "status": "ok",
            "sources": [
                {"id": "bloomberg", "name": "Bloomberg"},
                {"id": "reuters", "name": "Reuters"}
            ]
        }
        
        with patch.object(provider, '_request', return_value=mock_response):
            sources = await provider.get_sources(category="business")
            
            assert len(sources) == 2
            assert sources[0]['id'] == "bloomberg"
            assert sources[1]['id'] == "reuters"
            
    @pytest.mark.asyncio
    async def test_get_market_news(self, provider, sample_article):
        """Test getting market news for symbols."""
        await provider.connect()
        
        mock_response = {
            "status": "ok",
            "articles": [sample_article]
        }
        
        with patch.object(provider, '_request', return_value=mock_response):
            articles = []
            async for article in provider.get_market_news(["AAPL", "GOOGL"]):
                articles.append(article)
                
            # Should make requests for each symbol
            assert len(articles) == 2  # One article per symbol
            
    @pytest.mark.asyncio
    async def test_get_sentiment_summary(self, provider, sample_article, sample_negative_article):
        """Test sentiment summary calculation."""
        await provider.connect()
        
        # Mock responses for different sentiment articles
        positive_response = {"status": "ok", "articles": [sample_article]}
        negative_response = {"status": "ok", "articles": [sample_negative_article]}
        
        # Alternate between positive and negative responses
        responses = [positive_response, negative_response]
        
        with patch.object(provider, '_request', side_effect=responses):
            summary = await provider.get_sentiment_summary(["AAPL"], days=7)
            
            assert "AAPL" in summary
            aapl_summary = summary["AAPL"]
            
            # Should have both positive and negative articles
            assert aapl_summary["article_count"] == 2
            assert aapl_summary["positive_articles"] == 1
            assert aapl_summary["negative_articles"] == 1
            assert aapl_summary["neutral_articles"] == 0
            
    @pytest.mark.asyncio
    async def test_api_error_handling(self, provider):
        """Test API error handling."""
        await provider.connect()
        
        error_response = {
            "status": "error",
            "message": "Invalid API key"
        }
        
        with patch.object(provider, '_request', return_value=error_response):
            with pytest.raises(ValueError, match="API Error: Invalid API key"):
                articles = []
                async for article in provider.get_top_headlines():
                    articles.append(article)
                    
    @pytest.mark.asyncio
    async def test_request_retry(self, provider):
        """Test request retry on failure."""
        await provider.connect()
        
        # Mock session that fails once then succeeds
        mock_response = AsyncMock()
        mock_response.raise_for_status = MagicMock()
        mock_response.json = AsyncMock(return_value={"status": "ok", "articles": []})
        
        provider.session.get = AsyncMock()
        provider.session.get.side_effect = [
            aiohttp.ClientError("Connection error"),
            AsyncMock(__aenter__=AsyncMock(return_value=mock_response))
        ]
        
        # Should retry and succeed
        data = await provider._request("top-headlines")
        assert data["status"] == "ok"
        
    @pytest.mark.asyncio
    async def test_not_implemented_methods(self, provider):
        """Test that market data methods raise NotImplementedError."""
        with pytest.raises(NotImplementedError):
            async for _ in provider.get_market_data(["AAPL"], datetime.now(), datetime.now()):
                pass
                
        with pytest.raises(NotImplementedError):
            async for _ in provider.stream_market_data(["AAPL"]):
                pass


@pytest.mark.asyncio
async def test_context_manager(mock_settings):
    """Test using provider as async context manager."""
    with patch('data_ingestion.providers.base.get_settings', return_value=mock_settings):
        async with NewsAPIProvider() as provider:
            assert provider._connected is True
            
        assert provider._connected is False