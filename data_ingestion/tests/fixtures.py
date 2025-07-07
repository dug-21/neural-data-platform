"""Mock data fixtures for testing data providers."""
from datetime import datetime
from typing import Dict, Any, List


class MockDataFixtures:
    """Collection of mock data for different providers."""
    
    @staticmethod
    def get_iex_cloud_response() -> Dict[str, Any]:
        """Mock IEX Cloud API response."""
        return {
            "chart": [
                {
                    "date": "2024-01-01",
                    "open": 100.0,
                    "high": 105.0,
                    "low": 99.0,
                    "close": 104.0,
                    "volume": 1000000,
                    "symbol": "AAPL",
                    "changePercent": 0.04
                },
                {
                    "date": "2024-01-02",
                    "open": 104.0,
                    "high": 107.0,
                    "low": 103.0,
                    "close": 106.0,
                    "volume": 1200000,
                    "symbol": "AAPL",
                    "changePercent": 0.019
                }
            ]
        }
    
    @staticmethod
    def get_alpha_vantage_response() -> Dict[str, Any]:
        """Mock Alpha Vantage API response."""
        return {
            "Meta Data": {
                "1. Information": "Intraday (1min)",
                "2. Symbol": "AAPL",
                "3. Last Refreshed": "2024-01-02 16:00:00",
                "4. Interval": "1min",
                "5. Output Size": "Compact",
                "6. Time Zone": "US/Eastern"
            },
            "Time Series (1min)": {
                "2024-01-01 09:30:00": {
                    "1. open": "100.00",
                    "2. high": "100.50",
                    "3. low": "99.80",
                    "4. close": "100.20",
                    "5. volume": "50000"
                },
                "2024-01-01 09:31:00": {
                    "1. open": "100.20",
                    "2. high": "100.80",
                    "3. low": "100.10",
                    "4. close": "100.60",
                    "5. volume": "45000"
                }
            }
        }
    
    @staticmethod
    def get_polygon_response() -> Dict[str, Any]:
        """Mock Polygon.io API response."""
        return {
            "ticker": "AAPL",
            "status": "OK",
            "from": "2024-01-01",
            "to": "2024-01-02",
            "results": [
                {
                    "v": 1000000,  # volume
                    "vw": 102.5,   # volume weighted avg price
                    "o": 100.0,    # open
                    "c": 104.0,    # close
                    "h": 105.0,    # high
                    "l": 99.0,     # low
                    "t": 1704123600000,  # timestamp
                    "n": 1000      # number of transactions
                },
                {
                    "v": 1200000,
                    "vw": 105.0,
                    "o": 104.0,
                    "c": 106.0,
                    "h": 107.0,
                    "l": 103.0,
                    "t": 1704210000000,
                    "n": 1100
                }
            ]
        }
    
    @staticmethod
    def get_yahoo_finance_response() -> Dict[str, Any]:
        """Mock Yahoo Finance API response."""
        return {
            "chart": {
                "result": [{
                    "meta": {
                        "currency": "USD",
                        "symbol": "AAPL",
                        "exchangeName": "NMS",
                        "instrumentType": "EQUITY",
                        "regularMarketPrice": 106.0,
                        "regularMarketTime": 1704210000
                    },
                    "timestamp": [1704123600, 1704127200, 1704130800],
                    "indicators": {
                        "quote": [{
                            "open": [100.0, 101.0, 102.0],
                            "high": [105.0, 105.5, 106.0],
                            "low": [99.0, 100.5, 101.5],
                            "close": [104.0, 105.0, 106.0],
                            "volume": [1000000, 1100000, 1200000]
                        }],
                        "adjclose": [{
                            "adjclose": [104.0, 105.0, 106.0]
                        }]
                    }
                }],
                "error": None
            }
        }
    
    @staticmethod
    def get_finnhub_response() -> Dict[str, Any]:
        """Mock Finnhub API response."""
        return {
            "s": "ok",  # status
            "o": [100.0, 104.0],  # open prices
            "h": [105.0, 107.0],  # high prices
            "l": [99.0, 103.0],   # low prices
            "c": [104.0, 106.0],  # close prices
            "v": [1000000, 1200000],  # volumes
            "t": [1704123600, 1704210000]  # timestamps
        }
    
    @staticmethod
    def get_fred_response() -> Dict[str, Any]:
        """Mock FRED API response."""
        return {
            "realtime_start": "2024-01-01",
            "realtime_end": "2024-01-02",
            "observation_start": "2024-01-01",
            "observation_end": "2024-01-02",
            "units": "Percent",
            "output_type": 1,
            "file_type": "json",
            "order_by": "observation_date",
            "sort_order": "asc",
            "count": 2,
            "offset": 0,
            "limit": 100000,
            "observations": [
                {
                    "realtime_start": "2024-01-01",
                    "realtime_end": "2024-01-02",
                    "date": "2024-01-01",
                    "value": "3.5"
                },
                {
                    "realtime_start": "2024-01-01",
                    "realtime_end": "2024-01-02",
                    "date": "2024-01-02",
                    "value": "3.4"
                }
            ]
        }
    
    @staticmethod
    def get_reddit_response() -> Dict[str, Any]:
        """Mock Reddit API response."""
        return {
            "kind": "Listing",
            "data": {
                "after": "t3_xyz789",
                "dist": 25,
                "modhash": "",
                "geo_filter": None,
                "children": [
                    {
                        "kind": "t3",
                        "data": {
                            "subreddit": "wallstreetbets",
                            "selftext": "AAPL earnings were amazing! To the moon! 🚀",
                            "author_fullname": "t2_testuser",
                            "title": "AAPL DD: Why I'm going all in",
                            "subreddit_name_prefixed": "r/wallstreetbets",
                            "name": "t3_abc123",
                            "score": 1500,
                            "thumbnail": "self",
                            "created": 1704123600.0,
                            "created_utc": 1704123600.0,
                            "num_comments": 250,
                            "upvote_ratio": 0.95,
                            "over_18": False,
                            "spoiler": False,
                            "locked": False,
                            "id": "abc123",
                            "author": "testuser",
                            "permalink": "/r/wallstreetbets/comments/abc123/"
                        }
                    },
                    {
                        "kind": "t3",
                        "data": {
                            "subreddit": "stocks",
                            "selftext": "Technical analysis shows strong support at $100",
                            "author_fullname": "t2_analyst",
                            "title": "AAPL Technical Analysis - Bullish Pattern",
                            "subreddit_name_prefixed": "r/stocks",
                            "name": "t3_def456",
                            "score": 500,
                            "thumbnail": "self",
                            "created": 1704127200.0,
                            "created_utc": 1704127200.0,
                            "num_comments": 75,
                            "upvote_ratio": 0.89,
                            "over_18": False,
                            "spoiler": False,
                            "locked": False,
                            "id": "def456",
                            "author": "analyst",
                            "permalink": "/r/stocks/comments/def456/"
                        }
                    }
                ],
                "before": None
            }
        }
    
    @staticmethod
    def get_nasdaq_response() -> Dict[str, Any]:
        """Mock NASDAQ Data Link API response."""
        return {
            "data": {
                "tradesTable": {
                    "headers": ["Date", "Open", "High", "Low", "Close", "Volume"],
                    "rows": [
                        {
                            "date": "2024-01-01",
                            "open": "$100.00",
                            "high": "$105.00",
                            "low": "$99.00",
                            "close": "$104.00",
                            "volume": "1,000,000"
                        },
                        {
                            "date": "2024-01-02",
                            "open": "$104.00",
                            "high": "$107.00",
                            "low": "$103.00",
                            "close": "$106.00",
                            "volume": "1,200,000"
                        }
                    ]
                }
            }
        }
    
    @staticmethod
    def get_error_responses() -> Dict[str, Dict[str, Any]]:
        """Mock error responses for different providers."""
        return {
            "rate_limit": {
                "error": "Rate limit exceeded",
                "message": "You have exceeded your rate limit. Please try again later.",
                "status": 429
            },
            "invalid_api_key": {
                "error": "Unauthorized",
                "message": "Invalid API key provided",
                "status": 401
            },
            "symbol_not_found": {
                "error": "Not Found",
                "message": "Symbol not found",
                "status": 404
            },
            "server_error": {
                "error": "Internal Server Error",
                "message": "An unexpected error occurred",
                "status": 500
            },
            "invalid_parameters": {
                "error": "Bad Request",
                "message": "Invalid parameters provided",
                "status": 400
            }
        }
    
    @staticmethod
    def get_streaming_data() -> List[Dict[str, Any]]:
        """Mock streaming data updates."""
        base_price = 100.0
        updates = []
        
        for i in range(10):
            price_change = (i % 3 - 1) * 0.5  # Oscillate price
            updates.append({
                "type": "trade",
                "symbol": "AAPL",
                "price": base_price + price_change,
                "size": 100 + i * 10,
                "timestamp": 1704123600 + i * 60,
                "conditions": ["regular"]
            })
        
        return updates
    
    @staticmethod
    def get_order_book_snapshot() -> Dict[str, Any]:
        """Mock order book snapshot."""
        return {
            "symbol": "AAPL",
            "timestamp": 1704123600,
            "bids": [
                {"price": 99.95, "size": 500},
                {"price": 99.90, "size": 1000},
                {"price": 99.85, "size": 1500},
                {"price": 99.80, "size": 2000},
                {"price": 99.75, "size": 2500}
            ],
            "asks": [
                {"price": 100.05, "size": 500},
                {"price": 100.10, "size": 1000},
                {"price": 100.15, "size": 1500},
                {"price": 100.20, "size": 2000},
                {"price": 100.25, "size": 2500}
            ]
        }
    
    @staticmethod
    def get_aggregated_data() -> Dict[str, Any]:
        """Mock aggregated data from multiple providers."""
        return {
            "symbol": "AAPL",
            "timestamp": 1704123600,
            "consensus": {
                "open": 100.05,  # Average from multiple sources
                "high": 105.02,
                "low": 99.03,
                "close": 104.01,
                "volume": 1025000
            },
            "sources": [
                {
                    "provider": "iex_cloud",
                    "open": 100.0,
                    "close": 104.0,
                    "confidence": 0.95
                },
                {
                    "provider": "yahoo_finance",
                    "open": 100.1,
                    "close": 104.0,
                    "confidence": 0.92
                },
                {
                    "provider": "polygon",
                    "open": 100.05,
                    "close": 104.05,
                    "confidence": 0.94
                }
            ],
            "quality_score": 0.93,
            "discrepancy_flag": False
        }


class MockWebSocketMessages:
    """Mock WebSocket messages for real-time testing."""
    
    @staticmethod
    def get_connect_message() -> Dict[str, Any]:
        """WebSocket connection acknowledgment."""
        return {
            "type": "connection",
            "status": "connected",
            "message": "Successfully connected to market data stream"
        }
    
    @staticmethod
    def get_subscribe_message() -> Dict[str, Any]:
        """Subscription confirmation."""
        return {
            "type": "subscription",
            "status": "subscribed",
            "symbols": ["AAPL", "GOOGL", "MSFT"],
            "channels": ["trades", "quotes"]
        }
    
    @staticmethod
    def get_trade_message() -> Dict[str, Any]:
        """Real-time trade update."""
        return {
            "type": "trade",
            "symbol": "AAPL",
            "price": 104.25,
            "size": 300,
            "timestamp": 1704123660,
            "exchange": "NASDAQ",
            "conditions": ["regular", "intermarket_sweep"]
        }
    
    @staticmethod
    def get_quote_message() -> Dict[str, Any]:
        """Real-time quote update."""
        return {
            "type": "quote",
            "symbol": "AAPL",
            "bid": 104.20,
            "bid_size": 500,
            "ask": 104.25,
            "ask_size": 300,
            "timestamp": 1704123661,
            "exchange": "NASDAQ"
        }
    
    @staticmethod
    def get_error_message() -> Dict[str, Any]:
        """WebSocket error message."""
        return {
            "type": "error",
            "code": "SYMBOL_NOT_FOUND",
            "message": "Symbol XYZ is not available for streaming",
            "timestamp": 1704123662
        }