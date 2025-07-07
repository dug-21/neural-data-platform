# NewsAPI Provider

The NewsAPI provider integrates with [NewsAPI.org](https://newsapi.org) to fetch news articles and perform sentiment analysis for market intelligence.

## Features

- **Top Headlines**: Get breaking news and top stories
- **Everything Search**: Search millions of articles from over 80,000 sources
- **News Sources**: Discover available news sources
- **Market News**: Get news specific to stock symbols
- **Sentiment Analysis**: Automatic sentiment scoring of articles
- **Sentiment Summary**: Aggregate sentiment analysis for symbols

## Configuration

Add your NewsAPI key to your settings:

```python
# In your .env file or settings
NEWSAPI_KEY=your_api_key_here
```

## Usage

### Basic Usage

```python
from data_ingestion.providers.newsapi import NewsAPIProvider

async def get_news():
    async with NewsAPIProvider() as provider:
        # Get top business headlines
        async for article in provider.get_top_headlines(category="business"):
            print(f"{article.title} - Sentiment: {article.sentiment_label}")
```

### Search for Specific Topics

```python
# Search for articles about a company
async for article in provider.get_everything(
    q="Tesla earnings",
    from_date=datetime.now() - timedelta(days=7),
    sort_by="relevancy"
):
    print(f"{article.title}")
    print(f"Sentiment Score: {article.sentiment_score}")
```

### Get Market News for Symbols

```python
# Get news for multiple stock symbols
symbols = ["AAPL", "GOOGL", "MSFT"]
async for article in provider.get_market_news(symbols):
    print(f"{article.source}: {article.title}")
    print(f"Sentiment: {article.sentiment_label}")
```

### Sentiment Analysis Summary

```python
# Get sentiment summary for stocks over past week
summary = await provider.get_sentiment_summary(["AAPL", "TSLA"], days=7)

for symbol, data in summary.items():
    print(f"{symbol}:")
    print(f"  Overall: {data['overall_sentiment']}")
    print(f"  Average Score: {data['average_sentiment']:.3f}")
    print(f"  Positive Articles: {data['positive_articles']}")
    print(f"  Negative Articles: {data['negative_articles']}")
```

## Article Data Structure

Each article returned includes:

- `time`: Publication timestamp
- `source`: News source name
- `author`: Article author (if available)
- `title`: Article headline
- `description`: Brief summary
- `url`: Link to full article
- `image_url`: Featured image (if available)
- `content`: Article preview content
- `sentiment_score`: Calculated sentiment (-1 to 1)
- `sentiment_label`: "positive", "negative", or "neutral"

## Sentiment Analysis

The provider includes basic sentiment analysis that:

1. Analyzes article title, description, and content
2. Identifies positive/negative keywords related to markets
3. Calculates a sentiment score from -1 (very negative) to 1 (very positive)
4. Assigns labels: positive (>0.3), negative (<-0.3), or neutral

### Sentiment Keywords

**Positive**: gain, surge, rise, rally, boost, growth, profit, beat, bullish, etc.

**Negative**: loss, drop, fall, decline, crash, weak, risk, warning, bearish, etc.

## API Limits

- Free tier: 100 requests per day
- Rate limiting is automatically handled by the provider
- Page size is limited to 100 articles per request

## Available Parameters

### Top Headlines
- `category`: business, entertainment, general, health, science, sports, technology
- `country`: 2-letter ISO country code (e.g., "us", "gb")
- `q`: Keywords to search for
- `page_size`: Number of results (max 100)

### Everything Search
- `q`: Search query (required)
- `from_date`: Oldest article date
- `to_date`: Newest article date
- `sort_by`: relevancy, popularity, or publishedAt
- `language`: 2-letter ISO language code
- `page_size`: Number of results (max 100)

### Sources
- `category`: Category to filter by
- `language`: Language to filter by
- `country`: Country to filter by

## Error Handling

The provider includes:
- Automatic retry on network errors (3 attempts)
- Rate limiting to respect API limits
- Detailed error messages for debugging
- Graceful handling of missing article fields

## Testing

Run tests with:

```bash
pytest data_ingestion/tests/test_newsapi_provider.py -v
```

## Example

See `data_ingestion/examples/newsapi_example.py` for a complete working example.