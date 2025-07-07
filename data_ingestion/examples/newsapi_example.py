"""Example usage of NewsAPI provider for market sentiment analysis."""
import asyncio
from datetime import datetime, timedelta
from data_ingestion.providers.newsapi import NewsAPIProvider


async def main():
    """Demonstrate NewsAPI provider capabilities."""
    # Create provider instance
    provider = NewsAPIProvider()
    
    try:
        # Connect to NewsAPI
        await provider.connect()
        print("✅ Connected to NewsAPI")
        
        # 1. Get top business headlines
        print("\n📰 Top Business Headlines:")
        print("-" * 50)
        count = 0
        async for article in provider.get_top_headlines(category="business", page_size=5):
            count += 1
            print(f"\n{count}. {article.title}")
            print(f"   Source: {article.source}")
            print(f"   Sentiment: {article.sentiment_label} ({article.sentiment_score:.2f})")
            print(f"   URL: {article.url}")
            
        # 2. Search for specific stock news
        print("\n\n🔍 Apple (AAPL) News:")
        print("-" * 50)
        count = 0
        async for article in provider.get_everything(
            q="AAPL Apple stock",
            from_date=datetime.now() - timedelta(days=3),
            sort_by="relevancy",
            page_size=5
        ):
            count += 1
            print(f"\n{count}. {article.title}")
            print(f"   Time: {article.time}")
            print(f"   Sentiment: {article.sentiment_label} ({article.sentiment_score:.2f})")
            if article.description:
                print(f"   Summary: {article.description[:100]}...")
                
        # 3. Get market news for multiple stocks
        print("\n\n📊 Market News for Tech Stocks:")
        print("-" * 50)
        symbols = ["AAPL", "GOOGL", "MSFT"]
        
        articles_by_symbol = {}
        async for article in provider.get_market_news(
            symbols, 
            from_date=datetime.now() - timedelta(days=1)
        ):
            # Group by which symbol the article is about
            for symbol in symbols:
                if symbol in article.title or symbol in (article.description or ""):
                    if symbol not in articles_by_symbol:
                        articles_by_symbol[symbol] = []
                    articles_by_symbol[symbol].append(article)
                    break
                    
        for symbol, articles in articles_by_symbol.items():
            print(f"\n{symbol}: {len(articles)} articles")
            for article in articles[:2]:  # Show first 2
                print(f"  - {article.title}")
                print(f"    Sentiment: {article.sentiment_label}")
                
        # 4. Get sentiment summary
        print("\n\n💭 Sentiment Analysis Summary:")
        print("-" * 50)
        
        summary = await provider.get_sentiment_summary(["AAPL", "TSLA"], days=7)
        
        for symbol, data in summary.items():
            print(f"\n{symbol}:")
            print(f"  Overall Sentiment: {data['overall_sentiment'].upper()}")
            print(f"  Average Score: {data['average_sentiment']:.3f}")
            print(f"  Articles Analyzed: {data['article_count']}")
            print(f"  Distribution:")
            print(f"    - Positive: {data['positive_articles']} articles")
            print(f"    - Negative: {data['negative_articles']} articles") 
            print(f"    - Neutral: {data['neutral_articles']} articles")
            
            if data['latest_articles']:
                print(f"  Latest Headlines:")
                for article in data['latest_articles'][:3]:
                    print(f"    • {article.title}")
                    
        # 5. Get available news sources
        print("\n\n📡 Available News Sources:")
        print("-" * 50)
        
        sources = await provider.get_sources(category="business", country="us")
        print(f"Found {len(sources)} business news sources:")
        for source in sources[:10]:  # Show first 10
            print(f"  - {source['name']} ({source['id']})")
            
    except Exception as e:
        print(f"❌ Error: {e}")
        
    finally:
        # Clean up
        await provider.disconnect()
        print("\n\n✅ Disconnected from NewsAPI")


if __name__ == "__main__":
    # Run the example
    asyncio.run(main())