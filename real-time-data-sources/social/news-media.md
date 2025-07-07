# Social Media & News Real-Time Data Sources

## News APIs ⭐⭐⭐⭐

### NewsAPI.org
**Most Popular Free News Aggregator**

#### Overview
- **URL**: https://newsapi.org/
- **Documentation**: https://newsapi.org/docs
- **Update Frequency**: 15 minutes
- **Coverage**: 80,000+ news sources

#### Free Tier Details
- 100 requests per day
- 500 requests per day (with attribution)
- Access to headlines and articles
- 1 month historical data
- No commercial use

#### Implementation
```python
import requests
from datetime import datetime, timedelta

class NewsAPIClient:
    def __init__(self, api_key):
        self.api_key = api_key
        self.base_url = 'https://newsapi.org/v2'
    
    def get_top_headlines(self, country='us', category=None):
        """Get top headlines"""
        url = f'{self.base_url}/top-headlines'
        params = {
            'apiKey': self.api_key,
            'country': country,
            'pageSize': 100
        }
        
        if category:
            params['category'] = category
        
        response = requests.get(url, params=params)
        return response.json()
    
    def search_everything(self, query, from_date=None, sort_by='publishedAt'):
        """Search all articles"""
        url = f'{self.base_url}/everything'
        
        if not from_date:
            from_date = (datetime.now() - timedelta(days=7)).strftime('%Y-%m-%d')
        
        params = {
            'apiKey': self.api_key,
            'q': query,
            'from': from_date,
            'sortBy': sort_by,
            'pageSize': 100
        }
        
        response = requests.get(url, params=params)
        return response.json()
    
    def get_sources(self, category=None, language='en'):
        """Get news sources"""
        url = f'{self.base_url}/top-headlines/sources'
        params = {
            'apiKey': self.api_key,
            'language': language
        }
        
        if category:
            params['category'] = category
        
        response = requests.get(url, params=params)
        return response.json()

# Real-time monitoring
class NewsMonitor:
    def __init__(self, api_key, keywords):
        self.client = NewsAPIClient(api_key)
        self.keywords = keywords
        self.seen_articles = set()
        
    def check_news(self):
        for keyword in self.keywords:
            articles = self.client.search_everything(
                query=keyword,
                from_date=(datetime.now() - timedelta(hours=1)).strftime('%Y-%m-%d')
            )
            
            if articles['status'] == 'ok':
                for article in articles['articles']:
                    article_id = article['url']
                    if article_id not in self.seen_articles:
                        self.seen_articles.add(article_id)
                        self.on_new_article(keyword, article)
    
    def on_new_article(self, keyword, article):
        print(f"\n📰 NEW: {keyword}")
        print(f"Title: {article['title']}")
        print(f"Source: {article['source']['name']}")
        print(f"Time: {article['publishedAt']}")
        print(f"URL: {article['url']}")
```

---

### MediaStack ⭐⭐⭐
**Global News Coverage**

#### Overview
- **URL**: https://mediastack.com/
- **Update Frequency**: Near real-time
- **Coverage**: 7,500+ news sources
- **Languages**: 13 languages

#### Free Tier
- 500 requests per month
- 1 month historical data
- Basic endpoints only

```javascript
class MediaStackClient {
    constructor(apiKey) {
        this.apiKey = apiKey;
        this.baseUrl = 'http://api.mediastack.com/v1';
    }
    
    async getLiveNews(params = {}) {
        const url = `${this.baseUrl}/news`;
        const defaultParams = {
            access_key: this.apiKey,
            limit: 100,
            languages: 'en',
            sort: 'published_desc'
        };
        
        const queryParams = { ...defaultParams, ...params };
        const response = await fetch(`${url}?${new URLSearchParams(queryParams)}`);
        return response.json();
    }
    
    async searchNews(keywords, categories = null) {
        const params = {
            keywords: keywords,
            limit: 100
        };
        
        if (categories) {
            params.categories = categories; // general,business,technology,etc
        }
        
        return this.getLiveNews(params);
    }
    
    async getNewsBySources(sources) {
        return this.getLiveNews({
            sources: sources.join(',')
        });
    }
}
```

---

### NewsData.io ⭐⭐⭐
**Real-time News with Sentiment**

#### Overview
- **URL**: https://newsdata.io/
- **Update Frequency**: Real-time
- **Coverage**: 150,000+ sources
- **Features**: Sentiment analysis, entity extraction

#### Free Tier
- 200 requests per day
- Real-time news
- 48 hours historical
- 10 results per request

```python
class NewsDataClient:
    def __init__(self, api_key):
        self.api_key = api_key
        self.base_url = 'https://newsdata.io/api/1'
    
    def get_latest_news(self, country=None, category=None, language='en'):
        """Get latest news"""
        url = f'{self.base_url}/news'
        params = {
            'apikey': self.api_key,
            'language': language
        }
        
        if country:
            params['country'] = country
        if category:
            params['category'] = category
        
        response = requests.get(url, params=params)
        return response.json()
    
    def search_news(self, query, from_date=None, to_date=None):
        """Search news articles"""
        url = f'{self.base_url}/news'
        params = {
            'apikey': self.api_key,
            'q': query,
            'language': 'en'
        }
        
        if from_date:
            params['from_date'] = from_date
        if to_date:
            params['to_date'] = to_date
        
        response = requests.get(url, params=params)
        return response.json()
    
    def get_news_with_sentiment(self, query):
        """Get news with sentiment analysis"""
        url = f'{self.base_url}/news'
        params = {
            'apikey': self.api_key,
            'q': query,
            'sentiment': 'positive,negative,neutral'
        }
        
        response = requests.get(url, params=params)
        return response.json()
```

---

## Reddit API ⭐⭐⭐⭐
**Real-time Social Discussions**

### Overview
- **Documentation**: https://www.reddit.com/dev/api/
- **Authentication**: OAuth2 required
- **Rate Limit**: 60 requests/minute
- **Update Frequency**: Real-time

### Implementation
```python
import praw
import time
from datetime import datetime

class RedditMonitor:
    def __init__(self, client_id, client_secret, user_agent):
        self.reddit = praw.Reddit(
            client_id=client_id,
            client_secret=client_secret,
            user_agent=user_agent
        )
        self.monitored_subreddits = []
    
    def monitor_subreddit(self, subreddit_name, keywords=None):
        """Monitor new posts in subreddit"""
        subreddit = self.reddit.subreddit(subreddit_name)
        
        # Get initial posts
        seen_posts = set()
        for post in subreddit.new(limit=100):
            seen_posts.add(post.id)
        
        print(f"Monitoring r/{subreddit_name}...")
        
        while True:
            try:
                for post in subreddit.new(limit=25):
                    if post.id not in seen_posts:
                        seen_posts.add(post.id)
                        
                        # Check keywords if specified
                        if keywords:
                            text = f"{post.title} {post.selftext}".lower()
                            if any(keyword.lower() in text for keyword in keywords):
                                self.on_matching_post(post, keywords)
                        else:
                            self.on_new_post(post)
                
                time.sleep(30)  # Check every 30 seconds
                
            except Exception as e:
                print(f"Error: {e}")
                time.sleep(60)
    
    def on_new_post(self, post):
        print(f"\n🔴 New post in r/{post.subreddit}:")
        print(f"Title: {post.title}")
        print(f"Author: {post.author}")
        print(f"Score: {post.score}")
        print(f"URL: https://reddit.com{post.permalink}")
    
    def stream_comments(self, subreddit_name):
        """Stream all comments in real-time"""
        subreddit = self.reddit.subreddit(subreddit_name)
        
        for comment in subreddit.stream.comments(skip_existing=True):
            self.on_new_comment(comment)
    
    def on_new_comment(self, comment):
        print(f"\n💬 New comment by {comment.author}:")
        print(f"Post: {comment.submission.title}")
        print(f"Comment: {comment.body[:200]}...")
    
    def get_trending_topics(self, subreddit_name, time_filter='hour'):
        """Get trending topics"""
        subreddit = self.reddit.subreddit(subreddit_name)
        trending = []
        
        for post in subreddit.top(time_filter=time_filter, limit=10):
            trending.append({
                'title': post.title,
                'score': post.score,
                'comments': post.num_comments,
                'created': datetime.fromtimestamp(post.created_utc),
                'url': f"https://reddit.com{post.permalink}"
            })
        
        return trending
```

### Reddit Streaming with PRAW
```python
# Real-time streaming
class RedditStreamMonitor:
    def __init__(self, reddit_instance):
        self.reddit = reddit_instance
    
    def stream_multiple_subreddits(self, subreddits, callback):
        """Stream from multiple subreddits"""
        combined = '+'.join(subreddits)
        subreddit = self.reddit.subreddit(combined)
        
        # Stream submissions
        for submission in subreddit.stream.submissions(skip_existing=True):
            callback('submission', submission)
        
    def stream_all_reddit(self, keywords):
        """Stream all of Reddit for keywords"""
        all_reddit = self.reddit.subreddit('all')
        
        for submission in all_reddit.stream.submissions(skip_existing=True):
            title_lower = submission.title.lower()
            if any(keyword.lower() in title_lower for keyword in keywords):
                self.on_keyword_match(submission, keywords)
    
    def on_keyword_match(self, submission, keywords):
        matched = [kw for kw in keywords if kw.lower() in submission.title.lower()]
        print(f"\n🎯 Keyword match: {', '.join(matched)}")
        print(f"Subreddit: r/{submission.subreddit}")
        print(f"Title: {submission.title}")
        print(f"URL: https://reddit.com{submission.permalink}")
```

---

## Discord Integration ⭐⭐⭐
**Real-time Chat Monitoring**

### Overview
- **API**: Requires bot creation
- **WebSocket**: Real-time gateway
- **Rate Limits**: Complex, bucket-based
- **Update Frequency**: Real-time

### Bot Implementation
```python
import discord
from discord.ext import commands, tasks
import asyncio

class DiscordMonitor(discord.Client):
    def __init__(self, keywords=None):
        super().__init__(intents=discord.Intents.all())
        self.keywords = keywords or []
        self.message_cache = []
    
    async def on_ready(self):
        print(f'Logged in as {self.user}')
        self.monitor_messages.start()
    
    async def on_message(self, message):
        # Don't respond to ourselves
        if message.author == self.user:
            return
        
        # Check for keywords
        if self.keywords:
            content_lower = message.content.lower()
            for keyword in self.keywords:
                if keyword.lower() in content_lower:
                    await self.on_keyword_found(message, keyword)
        
        # Cache message
        self.message_cache.append({
            'timestamp': message.created_at,
            'author': str(message.author),
            'channel': str(message.channel),
            'content': message.content,
            'guild': str(message.guild) if message.guild else 'DM'
        })
        
        # Keep cache size limited
        if len(self.message_cache) > 10000:
            self.message_cache = self.message_cache[-5000:]
    
    async def on_keyword_found(self, message, keyword):
        print(f"\n🔍 Keyword '{keyword}' found!")
        print(f"Server: {message.guild}")
        print(f"Channel: {message.channel}")
        print(f"Author: {message.author}")
        print(f"Message: {message.content}")
    
    @tasks.loop(minutes=5)
    async def monitor_messages(self):
        """Periodic analysis of message cache"""
        if not self.message_cache:
            return
        
        # Analyze trends, sentiment, etc.
        from collections import Counter
        
        # Word frequency
        words = []
        for msg in self.message_cache:
            words.extend(msg['content'].lower().split())
        
        common_words = Counter(words).most_common(20)
        print(f"\nTop words in last {len(self.message_cache)} messages:")
        for word, count in common_words:
            if len(word) > 3:  # Filter short words
                print(f"  {word}: {count}")

# Usage
client = DiscordMonitor(keywords=['bitcoin', 'stocks', 'trading'])
client.run('YOUR_BOT_TOKEN')
```

---

## Twitter/X Alternatives ⭐⭐

### Mastodon (Federated) ⭐⭐⭐⭐
**Open Source Twitter Alternative**

```python
from mastodon import Mastodon, StreamListener

class MastodonMonitor(StreamListener):
    def __init__(self, api_base_url, access_token):
        self.mastodon = Mastodon(
            access_token=access_token,
            api_base_url=api_base_url
        )
        self.keywords = []
    
    def on_update(self, status):
        """Called when a new status arrives"""
        content = status['content']
        
        # Check for keywords
        for keyword in self.keywords:
            if keyword.lower() in content.lower():
                self.on_keyword_match(status, keyword)
    
    def on_keyword_match(self, status, keyword):
        print(f"\n🐘 Mastodon keyword match: {keyword}")
        print(f"User: @{status['account']['username']}")
        print(f"Content: {status['content']}")
        print(f"URL: {status['url']}")
    
    def stream_public(self):
        """Stream public timeline"""
        self.mastodon.stream_public(self)
    
    def stream_hashtag(self, hashtag):
        """Stream specific hashtag"""
        self.mastodon.stream_hashtag(hashtag, self)
    
    def get_trending_tags(self):
        """Get trending hashtags"""
        return self.mastodon.trending_tags()
```

### Bluesky (AT Protocol) ⭐⭐⭐
```javascript
// Bluesky firehose streaming
import { WebSocket } from 'ws';

class BlueskyFirehose {
    constructor() {
        this.ws = null;
        this.handlers = [];
    }
    
    connect() {
        this.ws = new WebSocket('wss://bsky.social/xrpc/com.atproto.sync.subscribeRepos');
        
        this.ws.on('open', () => {
            console.log('Connected to Bluesky firehose');
        });
        
        this.ws.on('message', (data) => {
            // Bluesky uses CAR format
            this.decodeAndProcess(data);
        });
        
        this.ws.on('error', (error) => {
            console.error('WebSocket error:', error);
        });
        
        this.ws.on('close', () => {
            console.log('Disconnected, reconnecting...');
            setTimeout(() => this.connect(), 5000);
        });
    }
    
    decodeAndProcess(data) {
        // CAR file decoding logic
        // Process AT Protocol events
        try {
            // Simplified - actual implementation needs CAR decoder
            const event = this.decodeCAR(data);
            
            if (event.type === 'post') {
                this.onPost(event);
            }
        } catch (error) {
            console.error('Decode error:', error);
        }
    }
    
    onPost(post) {
        console.log('New post:', post.text);
        // Process post
    }
}
```

---

## RSS Feed Aggregation ⭐⭐⭐⭐
**Traditional but Reliable**

```python
import feedparser
import asyncio
from datetime import datetime

class RSSAggregator:
    def __init__(self):
        self.feeds = {
            'BBC': 'http://feeds.bbci.co.uk/news/rss.xml',
            'CNN': 'http://rss.cnn.com/rss/cnn_topstories.rss',
            'Reuters': 'http://feeds.reuters.com/reuters/topNews',
            'TechCrunch': 'http://feeds.feedburner.com/TechCrunch/',
            'HackerNews': 'https://news.ycombinator.com/rss',
            'ArsTechnica': 'http://feeds.arstechnica.com/arstechnica/index',
            'TheVerge': 'https://www.theverge.com/rss/index.xml'
        }
        self.seen_entries = set()
    
    async def check_feed(self, name, url):
        """Check single feed for updates"""
        try:
            feed = feedparser.parse(url)
            
            for entry in feed.entries:
                entry_id = entry.get('id', entry.get('link'))
                
                if entry_id not in self.seen_entries:
                    self.seen_entries.add(entry_id)
                    
                    # Parse publication date
                    published = entry.get('published_parsed')
                    if published:
                        pub_date = datetime(*published[:6])
                        # Only show recent entries
                        if (datetime.now() - pub_date).days < 1:
                            self.on_new_entry(name, entry)
                    
        except Exception as e:
            print(f"Error checking {name}: {e}")
    
    def on_new_entry(self, source, entry):
        print(f"\n📡 {source}:")
        print(f"Title: {entry.title}")
        print(f"Link: {entry.link}")
        if 'summary' in entry:
            print(f"Summary: {entry.summary[:200]}...")
    
    async def monitor_all_feeds(self, interval=300):
        """Monitor all feeds"""
        while True:
            tasks = [
                self.check_feed(name, url)
                for name, url in self.feeds.items()
            ]
            
            await asyncio.gather(*tasks)
            await asyncio.sleep(interval)
```

---

## Hacker News API ⭐⭐⭐⭐⭐
**Tech News & Discussions**

```javascript
class HackerNewsMonitor {
    constructor() {
        this.baseUrl = 'https://hacker-news.firebaseio.com/v0';
        this.seenStories = new Set();
    }
    
    async getTopStories() {
        const response = await fetch(`${this.baseUrl}/topstories.json`);
        return response.json();
    }
    
    async getNewStories() {
        const response = await fetch(`${this.baseUrl}/newstories.json`);
        return response.json();
    }
    
    async getItem(id) {
        const response = await fetch(`${this.baseUrl}/item/${id}.json`);
        return response.json();
    }
    
    async monitorNewStories() {
        const storyIds = await this.getNewStories();
        
        for (const id of storyIds.slice(0, 30)) {
            if (!this.seenStories.has(id)) {
                this.seenStories.add(id);
                
                const story = await this.getItem(id);
                if (story && story.type === 'story') {
                    this.onNewStory(story);
                }
            }
        }
    }
    
    onNewStory(story) {
        console.log(`\n🔶 HN: ${story.title}`);
        console.log(`   Points: ${story.score}, Comments: ${story.descendants || 0}`);
        console.log(`   URL: ${story.url || `https://news.ycombinator.com/item?id=${story.id}`}`);
    }
    
    async streamComments(minScore = 10) {
        // Monitor high-scoring comments
        const updates = await fetch(`${this.baseUrl}/updates.json`);
        const data = await updates.json();
        
        for (const id of data.items || []) {
            const item = await this.getItem(id);
            
            if (item && item.type === 'comment' && item.score >= minScore) {
                console.log(`\n💭 High-score comment (${item.score} points):`);
                console.log(item.text);
            }
        }
    }
    
    start() {
        // Check every minute
        setInterval(() => this.monitorNewStories(), 60000);
        
        // Check top stories every 5 minutes
        setInterval(async () => {
            const top = await this.getTopStories();
            console.log(`\nTop story IDs: ${top.slice(0, 5).join(', ')}`);
        }, 300000);
    }
}
```

---

## News Webhooks & Push Services

### Webhooks.news ⭐⭐⭐
```python
# Example webhook receiver
from flask import Flask, request
import json

app = Flask(__name__)

@app.route('/webhook/news', methods=['POST'])
def receive_news_webhook():
    data = request.json
    
    # Process incoming news
    print(f"New article: {data['title']}")
    print(f"Source: {data['source']}")
    print(f"URL: {data['url']}")
    
    # Trigger your processing pipeline
    process_news_item(data)
    
    return {'status': 'received'}, 200

def process_news_item(news_data):
    # Sentiment analysis, entity extraction, etc.
    pass
```

---

## Integrated Social/News Monitor

```python
import asyncio
from concurrent.futures import ThreadPoolExecutor
import time

class IntegratedNewsMonitor:
    def __init__(self, config):
        self.config = config
        self.sources = {
            'newsapi': NewsAPIClient(config['newsapi_key']),
            'reddit': RedditMonitor(
                config['reddit_client_id'],
                config['reddit_secret'],
                config['reddit_user_agent']
            ),
            'rss': RSSAggregator(),
            'hn': HackerNewsMonitor()
        }
        
        self.keywords = config.get('keywords', [])
        self.executor = ThreadPoolExecutor(max_workers=10)
    
    async def monitor_all_sources(self):
        """Monitor all configured sources"""
        tasks = []
        
        # News API - every 15 minutes
        tasks.append(self.monitor_news_api())
        
        # Reddit - continuous
        tasks.append(self.monitor_reddit())
        
        # RSS - every 5 minutes
        tasks.append(self.monitor_rss())
        
        # Hacker News - every 2 minutes
        tasks.append(self.monitor_hn())
        
        await asyncio.gather(*tasks)
    
    async def monitor_news_api(self):
        while True:
            try:
                for keyword in self.keywords:
                    articles = self.sources['newsapi'].search_everything(keyword)
                    # Process articles
                    
                await asyncio.sleep(900)  # 15 minutes
            except Exception as e:
                print(f"NewsAPI error: {e}")
                await asyncio.sleep(1800)  # 30 minutes on error
    
    async def monitor_reddit(self):
        # Run Reddit monitoring in thread pool
        loop = asyncio.get_event_loop()
        await loop.run_in_executor(
            self.executor,
            self.sources['reddit'].stream_all_reddit,
            self.keywords
        )
    
    def aggregate_sentiment(self, time_window=3600):
        """Aggregate sentiment across all sources"""
        # Implement cross-source sentiment analysis
        pass
    
    def detect_trending_topics(self):
        """Detect trending topics across sources"""
        # Implement trend detection
        pass
```

---

## Best Practices

### 1. Rate Limit Management
```python
from functools import wraps
import time

def rate_limit(calls, period):
    min_interval = period / calls
    last_called = [0.0]
    
    def decorator(func):
        @wraps(func)
        def wrapper(*args, **kwargs):
            elapsed = time.time() - last_called[0]
            left_to_wait = min_interval - elapsed
            if left_to_wait > 0:
                time.sleep(left_to_wait)
            ret = func(*args, **kwargs)
            last_called[0] = time.time()
            return ret
        return wrapper
    return decorator

# Usage: 60 calls per minute
@rate_limit(60, 60)
def call_api():
    pass
```

### 2. Content Deduplication
```python
import hashlib

class ContentDeduplicator:
    def __init__(self, ttl=86400):
        self.seen_hashes = {}
        self.ttl = ttl
    
    def get_hash(self, content):
        # Normalize content
        normalized = ' '.join(content.lower().split())
        return hashlib.md5(normalized.encode()).hexdigest()
    
    def is_duplicate(self, content):
        content_hash = self.get_hash(content)
        now = time.time()
        
        # Clean old entries
        self.seen_hashes = {
            h: t for h, t in self.seen_hashes.items()
            if now - t < self.ttl
        }
        
        if content_hash in self.seen_hashes:
            return True
        
        self.seen_hashes[content_hash] = now
        return False
```

### 3. Error Recovery
```python
class ResilientMonitor:
    def __init__(self, source, max_retries=3):
        self.source = source
        self.max_retries = max_retries
        self.retry_delay = 60
    
    async def fetch_with_retry(self):
        for attempt in range(self.max_retries):
            try:
                return await self.source.fetch()
            except Exception as e:
                if attempt == self.max_retries - 1:
                    raise
                
                wait_time = self.retry_delay * (2 ** attempt)
                print(f"Retry {attempt + 1} after {wait_time}s: {e}")
                await asyncio.sleep(wait_time)
```

---

## Comparison Matrix

| Source | Type | Update Rate | Free Limit | Auth | Best For |
|--------|------|-------------|------------|------|----------|
| NewsAPI | Aggregator | 15 min | 100/day | API key | Headlines |
| Reddit | Social | Real-time | 60/min | OAuth2 | Discussions |
| MediaStack | News | Real-time | 500/month | API key | Global news |
| Discord | Chat | Real-time | Complex | Bot token | Communities |
| RSS | Various | Site-specific | Unlimited | None | Custom sources |
| HN | Tech | Real-time | Unlimited | None | Tech news |

## Important Notes

1. **Twitter/X**: As of 2024, free API access is extremely limited
2. **Facebook**: No free API for public content
3. **Instagram**: Very restricted API, mainly for business accounts
4. **LinkedIn**: Limited API, mainly for authenticated user's data

Always check current terms of service as social media APIs change frequently.