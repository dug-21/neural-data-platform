"""NewsAPI data provider implementation with sentiment analysis."""
import asyncio
import aiohttp
from typing import List, AsyncIterator, Optional, Dict, Any
from datetime import datetime, timedelta
from dataclasses import dataclass
import re
from urllib.parse import quote

from .base import BaseProvider, DataType
from ..utils.retry import with_retry


@dataclass
class NewsArticle:
    """News article data structure."""
    time: datetime
    source: str
    author: Optional[str]
    title: str
    description: Optional[str]
    url: str
    image_url: Optional[str]
    content: Optional[str]
    sentiment_score: Optional[float] = None
    sentiment_label: Optional[str] = None
    provider: str = "newsapi"
    

class NewsAPIProvider(BaseProvider):
    """NewsAPI provider for news data with sentiment analysis."""
    
    BASE_URL = "https://newsapi.org/v2"
    
    # Sentiment keywords for basic analysis
    POSITIVE_KEYWORDS = [
        'gain', 'gains', 'surge', 'surges', 'rise', 'rises', 'rally', 'rallies',
        'boost', 'boosts', 'jump', 'jumps', 'advance', 'advances', 'strong',
        'growth', 'profit', 'profits', 'beat', 'beats', 'outperform', 'positive',
        'upgrade', 'upgrades', 'bullish', 'optimistic', 'record', 'high', 'success'
    ]
    
    NEGATIVE_KEYWORDS = [
        'loss', 'losses', 'drop', 'drops', 'fall', 'falls', 'decline', 'declines',
        'plunge', 'plunges', 'crash', 'crashes', 'slump', 'slumps', 'weak',
        'concern', 'concerns', 'risk', 'risks', 'warning', 'warnings', 'negative',
        'downgrade', 'downgrades', 'bearish', 'pessimistic', 'low', 'fail', 'fails'
    ]
    
    def __init__(self):
        super().__init__("newsapi")
        self.api_key = self.settings.newsapi_key
        self.session: Optional[aiohttp.ClientSession] = None
        
    async def connect(self):
        """Initialize HTTP session."""
        if not self.api_key:
            raise ValueError("NewsAPI key not configured")
            
        self.session = aiohttp.ClientSession(
            timeout=aiohttp.ClientTimeout(total=30)
        )
        self._connected = True
        self.logger.info("Connected to NewsAPI")
        
    async def disconnect(self):
        """Close HTTP session."""
        if self.session:
            await self.session.close()
        self._connected = False
        self.logger.info("Disconnected from NewsAPI")
    
    @with_retry(max_attempts=3, exceptions=(aiohttp.ClientError,))
    async def _request(self, endpoint: str, params: Optional[Dict[str, Any]] = None) -> Any:
        """Make request to NewsAPI."""
        await self._rate_limit()
        
        url = f"{self.BASE_URL}/{endpoint}"
        headers = {"X-Api-Key": self.api_key}
        
        try:
            async with self.session.get(url, headers=headers, params=params) as response:
                response.raise_for_status()
                data = await response.json()
                
                # Check for API errors
                if data.get("status") == "error":
                    raise ValueError(f"API Error: {data.get('message', 'Unknown error')}")
                    
                return data
        except aiohttp.ClientError as e:
            self.logger.error(f"API request failed: {endpoint}", error=str(e))
            raise
    
    def _calculate_sentiment(self, text: str) -> tuple[float, str]:
        """Calculate basic sentiment score from text."""
        if not text:
            return 0.0, "neutral"
            
        text_lower = text.lower()
        
        # Count positive and negative keywords
        positive_count = sum(1 for word in self.POSITIVE_KEYWORDS if word in text_lower)
        negative_count = sum(1 for word in self.NEGATIVE_KEYWORDS if word in text_lower)
        
        # Calculate sentiment score (-1 to 1)
        total_count = positive_count + negative_count
        if total_count == 0:
            score = 0.0
        else:
            score = (positive_count - negative_count) / total_count
            
        # Determine label
        if score >= 0.3:
            label = "positive"
        elif score <= -0.3:
            label = "negative"
        else:
            label = "neutral"
            
        return score, label
    
    def _parse_article(self, article: Dict[str, Any]) -> NewsArticle:
        """Parse article data into NewsArticle object."""
        # Parse published time
        published_at = article.get("publishedAt", "")
        if published_at:
            time = datetime.fromisoformat(published_at.replace("Z", "+00:00"))
        else:
            time = datetime.now()
            
        # Extract content for sentiment analysis
        title = article.get("title", "")
        description = article.get("description", "")
        content = article.get("content", "")
        
        # Calculate sentiment
        full_text = f"{title} {description} {content}"
        sentiment_score, sentiment_label = self._calculate_sentiment(full_text)
        
        return NewsArticle(
            time=time,
            source=article.get("source", {}).get("name", "Unknown"),
            author=article.get("author"),
            title=title,
            description=description,
            url=article.get("url", ""),
            image_url=article.get("urlToImage"),
            content=content,
            sentiment_score=sentiment_score,
            sentiment_label=sentiment_label,
            provider=self.name
        )
    
    async def get_top_headlines(
        self,
        category: Optional[str] = "business",
        country: Optional[str] = "us",
        q: Optional[str] = None,
        page_size: int = 100
    ) -> AsyncIterator[NewsArticle]:
        """Get top headlines."""
        params = {
            "pageSize": min(page_size, 100),  # NewsAPI max is 100
        }
        
        if category:
            params["category"] = category
        if country:
            params["country"] = country
        if q:
            params["q"] = q
            
        try:
            data = await self._request("top-headlines", params)
            
            for article in data.get("articles", []):
                yield self._parse_article(article)
                
        except Exception as e:
            self.logger.error(f"Failed to get top headlines", error=str(e))
            raise
    
    async def get_everything(
        self,
        q: str,
        from_date: Optional[datetime] = None,
        to_date: Optional[datetime] = None,
        sort_by: str = "relevancy",
        language: str = "en",
        page_size: int = 100
    ) -> AsyncIterator[NewsArticle]:
        """Search all articles."""
        params = {
            "q": q,
            "sortBy": sort_by,
            "language": language,
            "pageSize": min(page_size, 100)
        }
        
        # Add date range if specified
        if from_date:
            params["from"] = from_date.strftime("%Y-%m-%d")
        if to_date:
            params["to"] = to_date.strftime("%Y-%m-%d")
            
        try:
            data = await self._request("everything", params)
            
            for article in data.get("articles", []):
                yield self._parse_article(article)
                
        except Exception as e:
            self.logger.error(f"Failed to search articles for: {q}", error=str(e))
            raise
    
    async def get_sources(
        self,
        category: Optional[str] = None,
        language: Optional[str] = "en",
        country: Optional[str] = None
    ) -> List[Dict[str, Any]]:
        """Get available news sources."""
        params = {}
        
        if category:
            params["category"] = category
        if language:
            params["language"] = language
        if country:
            params["country"] = country
            
        try:
            data = await self._request("sources", params)
            return data.get("sources", [])
            
        except Exception as e:
            self.logger.error("Failed to get sources", error=str(e))
            raise
    
    async def get_market_news(
        self,
        symbols: List[str],
        from_date: Optional[datetime] = None,
        to_date: Optional[datetime] = None
    ) -> AsyncIterator[NewsArticle]:
        """Get news for specific stock symbols."""
        # Default to last 7 days if not specified
        if not from_date:
            from_date = datetime.now() - timedelta(days=7)
        if not to_date:
            to_date = datetime.now()
            
        # Search for each symbol
        for symbol in symbols:
            # Build search query
            query = f"{symbol} stock OR {symbol} shares OR {symbol} earnings"
            
            async for article in self.get_everything(
                q=query,
                from_date=from_date,
                to_date=to_date,
                sort_by="relevancy"
            ):
                yield article
    
    async def get_sentiment_summary(
        self,
        symbols: List[str],
        days: int = 7
    ) -> Dict[str, Dict[str, Any]]:
        """Get sentiment summary for symbols."""
        from_date = datetime.now() - timedelta(days=days)
        results = {}
        
        for symbol in symbols:
            articles = []
            sentiment_scores = []
            
            # Collect articles
            async for article in self.get_market_news([symbol], from_date=from_date):
                articles.append(article)
                if article.sentiment_score is not None:
                    sentiment_scores.append(article.sentiment_score)
            
            # Calculate summary
            if sentiment_scores:
                avg_sentiment = sum(sentiment_scores) / len(sentiment_scores)
                positive_count = sum(1 for s in sentiment_scores if s > 0.3)
                negative_count = sum(1 for s in sentiment_scores if s < -0.3)
                neutral_count = len(sentiment_scores) - positive_count - negative_count
                
                # Determine overall sentiment
                if avg_sentiment >= 0.2:
                    overall = "positive"
                elif avg_sentiment <= -0.2:
                    overall = "negative"
                else:
                    overall = "neutral"
                    
                results[symbol] = {
                    "average_sentiment": avg_sentiment,
                    "overall_sentiment": overall,
                    "article_count": len(articles),
                    "positive_articles": positive_count,
                    "negative_articles": negative_count,
                    "neutral_articles": neutral_count,
                    "latest_articles": articles[:5]  # Top 5 most recent
                }
            else:
                results[symbol] = {
                    "average_sentiment": 0.0,
                    "overall_sentiment": "neutral",
                    "article_count": len(articles),
                    "positive_articles": 0,
                    "negative_articles": 0,
                    "neutral_articles": 0,
                    "latest_articles": articles[:5]
                }
                
        return results
    
    # Required BaseProvider methods (not fully implemented for news)
    async def get_market_data(
        self,
        symbols: List[str],
        start_time: datetime,
        end_time: datetime,
        interval: str = "1min"
    ) -> AsyncIterator[Any]:
        """Not implemented for news provider."""
        raise NotImplementedError("NewsAPI does not provide market data")
        
    async def stream_market_data(
        self,
        symbols: List[str]
    ) -> AsyncIterator[Any]:
        """Not implemented for news provider."""
        raise NotImplementedError("NewsAPI does not provide streaming data")