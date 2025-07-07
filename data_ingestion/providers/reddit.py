"""Reddit sentiment analysis provider implementation."""
import asyncpraw
import asyncio
import re
from datetime import datetime, timedelta
from typing import List, Dict, Any, AsyncIterator, Optional, Set
from dataclasses import dataclass
import logging

from .base import BaseProvider
from ..config import get_settings
from ..utils.retry import with_retry


@dataclass
class RedditSentiment:
    """Reddit sentiment data structure."""
    ticker: str
    subreddit: str
    score: int
    num_comments: int
    sentiment: float
    title: str
    text: str
    created_utc: datetime
    author: str
    url: str
    mention_count: int = 1


class RedditProvider(BaseProvider):
    """Provider for Reddit sentiment data."""
    
    def __init__(self):
        super().__init__("Reddit")
        self.client_id = self.settings.reddit_client_id
        self.client_secret = self.settings.reddit_client_secret
        self.user_agent = self.settings.reddit_user_agent
        
        if not all([self.client_id, self.client_secret, self.user_agent]):
            raise ValueError("Reddit API credentials not found in settings")
        
        self._reddit: Optional[asyncpraw.Reddit] = None
        self.subreddits = ["wallstreetbets", "stocks", "investing", "options"]
        self.ticker_blacklist = {"DD", "CEO", "IPO", "FDA", "EPS", "PE", "SEC", "ETF"}
        
        # Sentiment keywords and scores
        self.bullish_keywords = {
            "moon": 2.0, "rocket": 2.0, "🚀": 2.0, "bull": 1.5, "bullish": 1.5,
            "call": 1.0, "calls": 1.0, "buy": 1.0, "long": 1.0, "pump": 1.0,
            "squeeze": 1.5, "gamma": 1.5, "breakout": 1.0, "mooning": 2.0,
            "tendies": 1.5, "gains": 1.0, "yolo": 1.5, "diamond hands": 2.0,
            "💎": 2.0, "🙌": 1.5, "printing": 1.5
        }
        
        self.bearish_keywords = {
            "put": -1.0, "puts": -1.0, "bear": -1.5, "bearish": -1.5,
            "short": -1.0, "sell": -1.0, "dump": -1.5, "crash": -2.0,
            "drill": -1.5, "tank": -1.5, "red": -0.5, "loss": -1.0,
            "bag": -1.0, "bagholder": -1.5, "guh": -2.0, "rip": -1.5,
            "📉": -1.5, "🐻": -1.5, "bleeding": -1.5
        }
    
    async def connect(self):
        """Initialize connection to Reddit API."""
        if self._connected:
            return
            
        self._reddit = asyncpraw.Reddit(
            client_id=self.client_id,
            client_secret=self.client_secret,
            user_agent=self.user_agent
        )
        
        # Verify connection
        try:
            user = await self._reddit.user.me()
            self.logger.info(f"Connected to Reddit API as {user.name if user else 'read-only'}")
        except Exception as e:
            self.logger.warning(f"Connected in read-only mode: {e}")
        
        self._connected = True
    
    async def disconnect(self):
        """Close connection to Reddit API."""
        if self._reddit:
            await self._reddit.close()
            self._reddit = None
        self._connected = False
        self.logger.info("Disconnected from Reddit API")
    
    def _check_connection(self):
        """Ensure provider is connected."""
        if not self._connected:
            raise RuntimeError("Not connected to Reddit API. Call connect() first.")
    
    def _extract_tickers(self, text: str) -> List[str]:
        """Extract ticker symbols from text."""
        # Combined pattern for $TICKER and plain TICKER
        ticker_pattern = r'\$([A-Z]{1,5})\b|(?:^|\s)([A-Z]{2,5})(?:\s|$|\.|\,)'
        
        all_text = text.upper()
        matches = re.findall(ticker_pattern, all_text)
        
        tickers = []
        seen = set()
        
        for match in matches:
            # match is a tuple, take the non-empty group
            ticker = match[0] if match[0] else match[1]
            
            # Filter criteria
            if (ticker and 
                2 <= len(ticker) <= 4 and  # Valid ticker length
                ticker not in self.ticker_blacklist and  # Not blacklisted
                ticker not in seen):  # Not duplicate
                
                tickers.append(ticker)
                seen.add(ticker)
        
        return tickers
    
    def _analyze_sentiment(self, post) -> float:
        """Analyze sentiment of a Reddit post."""
        # Combine title and text for analysis
        full_text = f"{post.title} {post.selftext}".lower()
        
        sentiment_score = 0.0
        word_count = 0
        
        # Check for bullish keywords
        for keyword, score in self.bullish_keywords.items():
            count = full_text.count(keyword.lower())
            if count > 0:
                sentiment_score += score * count
                word_count += count
        
        # Check for bearish keywords
        for keyword, score in self.bearish_keywords.items():
            count = full_text.count(keyword.lower())
            if count > 0:
                sentiment_score += score * count  # score is already negative
                word_count += count
        
        # Weight by post score (upvotes)
        if post.score > 100:
            sentiment_score *= 1.5
        elif post.score > 500:
            sentiment_score *= 2.0
        
        # Normalize sentiment to [-1, 1] range
        if word_count > 0:
            sentiment_score = max(-1.0, min(1.0, sentiment_score / (word_count * 2)))
        
        return sentiment_score
    
    @with_retry(max_attempts=3, max_delay=60)
    async def get_ticker_mentions(
        self,
        subreddit: str,
        limit: int = 100,
        time_filter: str = "day"
    ) -> AsyncIterator[RedditSentiment]:
        """
        Get ticker mentions from a subreddit.
        
        Args:
            subreddit: Subreddit name to search
            limit: Maximum number of posts to fetch
            time_filter: Time filter (hour, day, week, month, year, all)
            
        Yields:
            RedditSentiment objects for each ticker mention
        """
        self._check_connection()
        await self._rate_limit()
        
        try:
            sub = await self._reddit.subreddit(subreddit)
            
            # Get posts based on different sorting methods
            async for post in sub.new(limit=limit):
                # Extract tickers from title and text
                all_text = f"{post.title} {post.selftext}"
                tickers = self._extract_tickers(all_text)
                
                if tickers:
                    sentiment = self._analyze_sentiment(post)
                    
                    for ticker in tickers:
                        # Count mentions in the post
                        mention_count = all_text.upper().count(ticker)
                        
                        yield RedditSentiment(
                            ticker=ticker,
                            subreddit=subreddit,
                            score=post.score,
                            num_comments=post.num_comments,
                            sentiment=sentiment,
                            title=post.title,
                            text=post.selftext[:500],  # Truncate for storage
                            created_utc=datetime.fromtimestamp(post.created_utc),
                            author=post.author.name if post.author else "[deleted]",
                            url=f"https://reddit.com{post.permalink}",
                            mention_count=mention_count
                        )
                        
        except Exception as e:
            self.logger.error(f"Error fetching from r/{subreddit}: {e}")
            raise
    
    async def get_aggregated_sentiment(
        self,
        subreddits: Optional[List[str]] = None,
        time_window_hours: int = 24,
        min_mentions: int = 2
    ) -> Dict[str, Dict[str, Any]]:
        """
        Get aggregated sentiment for tickers across subreddits.
        
        Args:
            subreddits: List of subreddits (uses default if None)
            time_window_hours: Hours to look back
            min_mentions: Minimum mentions to include ticker
            
        Returns:
            Dictionary of ticker -> sentiment data
        """
        if subreddits is None:
            subreddits = self.subreddits
        
        ticker_data: Dict[str, List[RedditSentiment]] = {}
        
        # Collect mentions from all subreddits
        for subreddit in subreddits:
            try:
                async for mention in self.get_ticker_mentions(subreddit, limit=100):
                    if mention.ticker not in ticker_data:
                        ticker_data[mention.ticker] = []
                    ticker_data[mention.ticker].append(mention)
            except Exception as e:
                self.logger.warning(f"Error processing r/{subreddit}: {e}")
                continue
        
        # Aggregate sentiment by ticker
        aggregated = {}
        for ticker, mentions in ticker_data.items():
            if len(mentions) >= min_mentions:
                total_score = sum(m.score for m in mentions)
                total_comments = sum(m.num_comments for m in mentions)
                avg_sentiment = sum(m.sentiment for m in mentions) / len(mentions)
                
                aggregated[ticker] = {
                    "mention_count": len(mentions),
                    "total_score": total_score,
                    "total_comments": total_comments,
                    "avg_sentiment": avg_sentiment,
                    "sentiment_label": self._get_sentiment_label(avg_sentiment),
                    "subreddits": list(set(m.subreddit for m in mentions)),
                    "latest_mention": max(m.created_utc for m in mentions)
                }
        
        return aggregated
    
    def _get_sentiment_label(self, sentiment: float) -> str:
        """Convert sentiment score to label."""
        if sentiment > 0.5:
            return "very_bullish"
        elif sentiment > 0.2:
            return "bullish"
        elif sentiment < -0.5:
            return "very_bearish"
        elif sentiment < -0.2:
            return "bearish"
        else:
            return "neutral"
    
    async def search_ticker_posts(
        self,
        ticker: str,
        limit: int = 25,
        time_filter: str = "week"
    ) -> AsyncIterator[Dict[str, Any]]:
        """
        Search for posts mentioning a specific ticker.
        
        Args:
            ticker: Ticker symbol to search for
            limit: Maximum number of results
            time_filter: Time filter for search
            
        Yields:
            Post data dictionaries
        """
        self._check_connection()
        await self._rate_limit()
        
        search_query = f"${ticker} OR {ticker}"
        
        # Search across all configured subreddits
        subreddit_str = "+".join(self.subreddits)
        sub = await self._reddit.subreddit(subreddit_str)
        
        async for post in sub.search(search_query, limit=limit, time_filter=time_filter):
            yield {
                "title": post.title,
                "text": post.selftext[:1000],  # Truncate
                "score": post.score,
                "num_comments": post.num_comments,
                "created_utc": datetime.fromtimestamp(post.created_utc),
                "author": post.author.name if post.author else "[deleted]",
                "subreddit": post.subreddit.display_name,
                "url": f"https://reddit.com{post.permalink}",
                "sentiment": self._analyze_sentiment(post)
            }
    
    async def get_trending_tickers(
        self,
        limit: int = 10,
        time_window: str = "day"
    ) -> List[Dict[str, Any]]:
        """
        Get trending tickers based on mention frequency and engagement.
        
        Args:
            limit: Number of top tickers to return
            time_window: Time window for trending (hour, day, week)
            
        Returns:
            List of trending ticker data
        """
        ticker_stats: Dict[str, Dict[str, Any]] = {}
        
        # Collect data from hot posts across subreddits
        for subreddit in self.subreddits:
            try:
                sub = await self._reddit.subreddit(subreddit)
                
                # Get hot posts
                async for post in sub.hot(limit=50):
                    all_text = f"{post.title} {post.selftext}"
                    tickers = self._extract_tickers(all_text)
                    
                    for ticker in tickers:
                        if ticker not in ticker_stats:
                            ticker_stats[ticker] = {
                                "ticker": ticker,
                                "total_mentions": 0,
                                "total_score": 0,
                                "total_comments": 0,
                                "posts": 0,
                                "subreddits": set()
                            }
                        
                        # Count all occurrences in the post
                        mention_count = all_text.upper().count(ticker)
                        
                        ticker_stats[ticker]["total_mentions"] += mention_count
                        ticker_stats[ticker]["total_score"] += post.score
                        ticker_stats[ticker]["total_comments"] += post.num_comments
                        ticker_stats[ticker]["posts"] += 1
                        ticker_stats[ticker]["subreddits"].add(subreddit)
                        
            except Exception as e:
                self.logger.warning(f"Error processing trending from r/{subreddit}: {e}")
                continue
        
        # Convert sets to lists for serialization
        for stats in ticker_stats.values():
            stats["subreddits"] = list(stats["subreddits"])
        
        # Sort by engagement (combination of mentions, score, and comments)
        trending = sorted(
            ticker_stats.values(),
            key=lambda x: (
                x["total_mentions"] * 10 +  # Weight mentions heavily
                x["total_score"] +
                x["total_comments"] * 2
            ),
            reverse=True
        )
        
        return trending[:limit]
    
    # Required abstract methods (not fully implemented for Reddit)
    async def get_market_data(
        self,
        symbols: List[str],
        start_time: datetime,
        end_time: datetime,
        interval: str = "1day"
    ) -> AsyncIterator[None]:
        """Reddit doesn't provide market data."""
        raise NotImplementedError("Reddit provider does not support market data")
    
    async def stream_market_data(
        self,
        symbols: List[str]
    ) -> AsyncIterator[None]:
        """Reddit doesn't provide streaming market data."""
        raise NotImplementedError("Reddit provider does not support streaming market data")