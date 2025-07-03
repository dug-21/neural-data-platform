"""
Trading API Usage Examples for Day Trading
Demonstrates practical usage of Alpha Vantage and Polygon.io APIs
"""

import os
import asyncio
from datetime import datetime, timedelta
from typing import List, Dict, Optional
import pandas as pd

# API Keys (use environment variables in production)
ALPHA_VANTAGE_KEY = os.getenv('ALPHA_VANTAGE_KEY', 'E81QRCDSNSIUUCI4')
POLYGON_KEY = os.getenv('POLYGON_KEY', 'mp3IJaWLRs2dKooPNlmAiZ6c1p8_Ez0V')

# ============================================================================
# ALPHA VANTAGE EXAMPLES
# ============================================================================

def alpha_vantage_examples():
    """Examples using Alpha Vantage API - suitable for backtesting/EOD analysis"""
    
    from alpha_vantage.timeseries import TimeSeries
    from alpha_vantage.techindicators import TechIndicators
    from alpha_vantage.fundamentaldata import FundamentalData
    
    # Initialize clients
    ts = TimeSeries(key=ALPHA_VANTAGE_KEY, output_format='pandas')
    ti = TechIndicators(key=ALPHA_VANTAGE_KEY, output_format='pandas')
    fd = FundamentalData(key=ALPHA_VANTAGE_KEY, output_format='pandas')
    
    # Example 1: Get intraday data (EOD updated only on free tier)
    print("1. Fetching intraday data for AAPL...")
    try:
        data, meta = ts.get_intraday(
            symbol='AAPL',
            interval='5min',  # 1min, 5min, 15min, 30min, 60min
            outputsize='compact'  # 'compact' = 100 points, 'full' = all available
        )
        print(f"Latest 5 data points:\n{data.head()}")
        print(f"Data shape: {data.shape}")
    except Exception as e:
        print(f"Error: {e}")
    
    # Example 2: Get daily data with adjustments
    print("\n2. Fetching daily adjusted data...")
    try:
        data, meta = ts.get_daily_adjusted(
            symbol='AAPL',
            outputsize='compact'
        )
        # Calculate simple returns
        data['returns'] = data['5. adjusted close'].pct_change()
        print(f"Latest adjusted prices:\n{data[['5. adjusted close', 'returns']].head()}")
    except Exception as e:
        print(f"Error: {e}")
    
    # Example 3: Get technical indicators
    print("\n3. Fetching RSI indicator...")
    try:
        data, meta = ti.get_rsi(
            symbol='AAPL',
            interval='daily',
            time_period=14,
            series_type='close'
        )
        print(f"Latest RSI values:\n{data.head()}")
    except Exception as e:
        print(f"Error: {e}")
    
    # Example 4: Batch quote endpoint (more efficient for multiple symbols)
    print("\n4. Getting quotes for multiple symbols...")
    symbols = ['AAPL', 'MSFT', 'GOOGL', 'TSLA']
    quotes = {}
    
    for symbol in symbols:
        try:
            data, _ = ts.get_quote_endpoint(symbol)
            quotes[symbol] = {
                'price': data['05. price'],
                'volume': data['06. volume'],
                'change': data['09. change'],
                'change_pct': data['10. change percent']
            }
            # Sleep to respect rate limits (5 per minute)
            import time
            time.sleep(12)  # 60 seconds / 5 requests = 12 seconds between requests
        except Exception as e:
            print(f"Error fetching {symbol}: {e}")
    
    print("Current quotes:")
    for symbol, quote in quotes.items():
        print(f"{symbol}: ${quote.get('price', 'N/A')} ({quote.get('change_pct', 'N/A')})")

# ============================================================================
# POLYGON.IO EXAMPLES
# ============================================================================

def polygon_rest_examples():
    """Examples using Polygon.io REST API"""
    
    from polygon import RESTClient
    
    # Initialize client
    client = RESTClient(POLYGON_KEY)
    
    # Example 1: Get real-time quote (snapshot)
    print("1. Fetching real-time snapshot for AAPL...")
    try:
        ticker = client.get_snapshot_ticker("AAPL")
        print(f"Symbol: {ticker.ticker}")
        print(f"Day Open: ${ticker.day.open}")
        print(f"Day High: ${ticker.day.high}")
        print(f"Day Low: ${ticker.day.low}")
        print(f"Last Price: ${ticker.last_quote.ask_price}")
        print(f"Volume: {ticker.day.volume:,}")
    except Exception as e:
        print(f"Error: {e}")
    
    # Example 2: Get aggregates (bars)
    print("\n2. Fetching 5-minute bars...")
    try:
        # Get last 10 5-minute bars
        aggs = client.get_aggs(
            ticker="AAPL",
            multiplier=5,
            timespan="minute",
            from_=(datetime.now() - timedelta(hours=2)).strftime("%Y-%m-%d"),
            to=datetime.now().strftime("%Y-%m-%d"),
            limit=10
        )
        
        for agg in aggs:
            print(f"Time: {datetime.fromtimestamp(agg.timestamp/1000)}, "
                  f"O: ${agg.open}, H: ${agg.high}, L: ${agg.low}, C: ${agg.close}, "
                  f"V: {agg.volume:,}")
    except Exception as e:
        print(f"Error: {e}")
    
    # Example 3: Get trades
    print("\n3. Fetching recent trades...")
    try:
        trades = client.list_trades(
            ticker="AAPL",
            timestamp_gte=datetime.now() - timedelta(minutes=5),
            limit=5
        )
        
        for trade in trades:
            print(f"Time: {datetime.fromtimestamp(trade.sip_timestamp/1000000000)}, "
                  f"Price: ${trade.price}, Size: {trade.size}")
    except Exception as e:
        print(f"Error: {e}")
    
    # Example 4: Get market status
    print("\n4. Checking market status...")
    try:
        status = client.get_market_status()
        print(f"Market is: {status.market}")
        print(f"Server time: {status.server_time}")
    except Exception as e:
        print(f"Error: {e}")

def polygon_websocket_example():
    """Example using Polygon.io WebSocket for real-time data"""
    
    from polygon import WebSocketClient
    from polygon.websocket.models import WebSocketMessage
    from typing import List
    import json
    
    # Track received messages
    message_count = 0
    start_time = datetime.now()
    
    def handle_msg(msgs: List[WebSocketMessage]):
        """Handle incoming WebSocket messages"""
        nonlocal message_count
        
        for msg in msgs:
            message_count += 1
            
            # Parse message based on type
            if hasattr(msg, 'event_type'):
                if msg.event_type == 'T':  # Trade
                    print(f"Trade: {msg.symbol} @ ${msg.price} x {msg.size}")
                elif msg.event_type == 'Q':  # Quote
                    print(f"Quote: {msg.symbol} Bid: ${msg.bid_price} Ask: ${msg.ask_price}")
                elif msg.event_type == 'A':  # Aggregate
                    print(f"Agg: {msg.symbol} OHLC: ${msg.open}/{msg.high}/{msg.low}/{msg.close}")
            
            # Stop after 20 messages for demo
            if message_count >= 20:
                elapsed = (datetime.now() - start_time).total_seconds()
                print(f"\nReceived {message_count} messages in {elapsed:.2f} seconds")
                print(f"Rate: {message_count/elapsed:.2f} messages/second")
                return False  # Stop the client
    
    # Create WebSocket client
    print("Starting WebSocket stream for AAPL trades...")
    print("(Note: This will show delayed data on free tier)")
    
    try:
        # Subscribe to trades for AAPL
        # T.* = all trades, Q.* = all quotes, A.* = all aggregates
        client = WebSocketClient(
            api_key=POLYGON_KEY,
            feed='delayed.polygon.io',  # Use delayed feed for free tier
            subscriptions=["T.AAPL", "Q.AAPL"],  # Trades and quotes for AAPL
        )
        
        # Run the client
        client.run(handle_msg)
        
    except Exception as e:
        print(f"WebSocket error: {e}")

# ============================================================================
# DAY TRADING STRATEGY EXAMPLE
# ============================================================================

async def simple_momentum_strategy(symbols: List[str], lookback_minutes: int = 30):
    """
    Simple momentum strategy using Polygon.io
    Identifies stocks with strong recent momentum
    """
    from polygon import RESTClient
    
    client = RESTClient(POLYGON_KEY)
    momentum_scores = {}
    
    for symbol in symbols:
        try:
            # Get recent aggregates
            aggs = client.get_aggs(
                ticker=symbol,
                multiplier=1,
                timespan="minute",
                from_=(datetime.now() - timedelta(minutes=lookback_minutes)).strftime("%Y-%m-%d %H:%M:%S"),
                to=datetime.now().strftime("%Y-%m-%d %H:%M:%S"),
                limit=lookback_minutes
            )
            
            if len(aggs) > 0:
                # Convert to pandas for easier analysis
                df = pd.DataFrame([{
                    'timestamp': agg.timestamp,
                    'close': agg.close,
                    'volume': agg.volume
                } for agg in aggs])
                
                # Calculate momentum metrics
                returns = (df['close'].iloc[-1] / df['close'].iloc[0] - 1) * 100
                avg_volume = df['volume'].mean()
                volume_spike = df['volume'].iloc[-1] / avg_volume if avg_volume > 0 else 0
                
                momentum_scores[symbol] = {
                    'returns': returns,
                    'volume_spike': volume_spike,
                    'last_price': df['close'].iloc[-1],
                    'score': returns * volume_spike  # Simple momentum score
                }
                
            # Respect rate limits
            await asyncio.sleep(0.2)  # 5 requests per second max
            
        except Exception as e:
            print(f"Error analyzing {symbol}: {e}")
    
    # Sort by momentum score
    sorted_momentum = sorted(
        momentum_scores.items(),
        key=lambda x: x[1]['score'],
        reverse=True
    )
    
    print("\nMomentum Analysis Results:")
    print("-" * 60)
    print(f"{'Symbol':<10} {'Return %':<10} {'Vol Spike':<10} {'Price':<10} {'Score':<10}")
    print("-" * 60)
    
    for symbol, metrics in sorted_momentum[:5]:  # Top 5
        print(f"{symbol:<10} {metrics['returns']:<10.2f} {metrics['volume_spike']:<10.2f} "
              f"${metrics['last_price']:<9.2f} {metrics['score']:<10.2f}")
    
    return sorted_momentum

# ============================================================================
# MAIN EXECUTION
# ============================================================================

if __name__ == "__main__":
    print("=" * 80)
    print("TRADING API EXAMPLES FOR DAY TRADING")
    print("=" * 80)
    
    # Choose which examples to run
    print("\nSelect examples to run:")
    print("1. Alpha Vantage REST API examples")
    print("2. Polygon.io REST API examples")
    print("3. Polygon.io WebSocket streaming")
    print("4. Simple momentum strategy")
    print("5. Run all examples")
    
    choice = input("\nEnter choice (1-5): ").strip()
    
    if choice == '1' or choice == '5':
        print("\n" + "="*80)
        print("ALPHA VANTAGE EXAMPLES (EOD Data Only on Free Tier)")
        print("="*80)
        alpha_vantage_examples()
    
    if choice == '2' or choice == '5':
        print("\n" + "="*80)
        print("POLYGON.IO REST API EXAMPLES")
        print("="*80)
        polygon_rest_examples()
    
    if choice == '3' or choice == '5':
        print("\n" + "="*80)
        print("POLYGON.IO WEBSOCKET STREAMING")
        print("="*80)
        polygon_websocket_example()
    
    if choice == '4' or choice == '5':
        print("\n" + "="*80)
        print("MOMENTUM STRATEGY EXAMPLE")
        print("="*80)
        symbols = ['AAPL', 'MSFT', 'GOOGL', 'TSLA', 'NVDA']
        asyncio.run(simple_momentum_strategy(symbols, lookback_minutes=30))
    
    print("\n" + "="*80)
    print("IMPORTANT NOTES FOR DAY TRADING:")
    print("="*80)
    print("1. Alpha Vantage free tier has NO real-time data - only EOD updates")
    print("2. Polygon.io free tier provides delayed data (typically 15 minutes)")
    print("3. For active day trading, you need premium subscriptions")
    print("4. Always implement proper error handling and respect rate limits")
    print("5. Consider using WebSockets for real-time data instead of polling REST APIs")