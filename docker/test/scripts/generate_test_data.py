#!/usr/bin/env python3
"""
Test data generator for Neural Trader testing environment.
Generates realistic market data, features, and predictions for testing.
"""
import asyncio
import json
import os
import sys
from datetime import datetime, timedelta
from decimal import Decimal
import logging

import pandas as pd
import numpy as np
import psycopg2
from psycopg2.extras import RealDictCursor
from faker import Faker

# Setup logging
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger(__name__)

fake = Faker()
fake.seed_instance(42)  # Reproducible test data


class TestDataGenerator:
    """Generates comprehensive test data for Neural Trader."""
    
    def __init__(self, db_url: str):
        self.db_url = db_url
        self.symbols = os.getenv('TEST_SYMBOLS', 'AAPL,MSFT,GOOGL,AMZN,NVDA').split(',')
        self.historical_days = int(os.getenv('HISTORICAL_DAYS', '30'))
        self.data_frequency = os.getenv('DATA_FREQUENCY', '1min')
        
    def connect_db(self):
        """Create database connection."""
        return psycopg2.connect(self.db_url)
    
    def generate_price_series(self, symbol: str, start_time: datetime, 
                            end_time: datetime, freq_minutes: int = 1) -> list:
        """Generate realistic price series using geometric Brownian motion."""
        # Base prices for different symbols (realistic starting points)
        base_prices = {
            'AAPL': 150.0,
            'MSFT': 280.0, 
            'GOOGL': 2150.0,
            'AMZN': 135.0,
            'NVDA': 420.0,
            'TSLA': 250.0,
            'META': 180.0,
            'NFLX': 400.0
        }
        
        base_price = base_prices.get(symbol, 100.0)
        
        # Parameters for geometric Brownian motion
        mu = 0.0001  # Drift (slight upward trend)
        sigma = 0.015  # Volatility (1.5%)
        dt = freq_minutes / (365 * 24 * 60)  # Time step in years
        
        # Generate time series
        current_time = start_time
        prices = []
        current_price = base_price
        
        while current_time <= end_time:
            # Generate random walk
            random_shock = np.random.normal(0, 1)
            price_change = mu * dt + sigma * np.sqrt(dt) * random_shock
            current_price *= (1 + price_change)
            
            # Ensure price stays positive and realistic
            current_price = max(current_price, base_price * 0.1)
            current_price = min(current_price, base_price * 10.0)
            
            # Generate OHLCV data
            open_price = current_price
            close_price = current_price * (1 + np.random.normal(0, 0.005))  # Small close variation
            high_price = max(open_price, close_price) * (1 + abs(np.random.normal(0, 0.003)))
            low_price = min(open_price, close_price) * (1 - abs(np.random.normal(0, 0.003)))
            volume = int(np.random.lognormal(np.log(500000), 0.5))  # Log-normal volume
            
            prices.append({
                'time': current_time,
                'symbol': symbol,
                'open_price': round(open_price, 4),
                'high_price': round(high_price, 4),
                'low_price': round(low_price, 4),
                'close_price': round(close_price, 4),
                'price': round(close_price, 4),  # Use close as current price
                'volume': volume,
                'provider': 'test_generator'
            })
            
            current_time += timedelta(minutes=freq_minutes)
            current_price = close_price
            
        return prices
    
    def calculate_technical_features(self, prices_df: pd.DataFrame, symbol: str) -> list:
        """Calculate technical analysis features."""
        features = []
        
        # Sort by time
        prices_df = prices_df.sort_values('time')
        
        # Simple Moving Averages
        prices_df['sma_5'] = prices_df['price'].rolling(window=5, min_periods=1).mean()
        prices_df['sma_10'] = prices_df['price'].rolling(window=10, min_periods=1).mean()
        prices_df['sma_20'] = prices_df['price'].rolling(window=20, min_periods=1).mean()
        
        # Exponential Moving Averages
        prices_df['ema_5'] = prices_df['price'].ewm(span=5, min_periods=1).mean()
        prices_df['ema_10'] = prices_df['price'].ewm(span=10, min_periods=1).mean()
        
        # RSI (Relative Strength Index)
        def calculate_rsi(prices, window=14):
            delta = prices.diff()
            gain = (delta.where(delta > 0, 0)).rolling(window=window, min_periods=1).mean()
            loss = (-delta.where(delta < 0, 0)).rolling(window=window, min_periods=1).mean()
            rs = gain / loss.replace(0, np.inf)
            rsi = 100 - (100 / (1 + rs))
            return rsi
        
        prices_df['rsi_14'] = calculate_rsi(prices_df['price'])
        
        # Volume indicators
        prices_df['volume_sma_10'] = prices_df['volume'].rolling(window=10, min_periods=1).mean()
        prices_df['volume_ratio'] = prices_df['volume'] / prices_df['volume_sma_10']
        
        # Price momentum
        prices_df['price_momentum'] = prices_df['price'].pct_change(periods=5)
        
        # Volatility (rolling standard deviation)
        prices_df['volatility'] = prices_df['price'].rolling(window=20, min_periods=1).std()
        
        # Convert to features format
        feature_columns = ['sma_5', 'sma_10', 'sma_20', 'ema_5', 'ema_10', 
                          'rsi_14', 'volume_sma_10', 'volume_ratio', 
                          'price_momentum', 'volatility']
        
        for _, row in prices_df.iterrows():
            for col in feature_columns:
                if pd.notna(row[col]):
                    features.append({
                        'symbol': symbol,
                        'time': row['time'],
                        'feature_name': col,
                        'feature_value': float(row[col]),
                        'feature_type': self._get_feature_type(col),
                        'calculation_method': self._get_calculation_method(col)
                    })
        
        return features
    
    def _get_feature_type(self, feature_name: str) -> str:
        """Determine feature type based on name."""
        if 'sma' in feature_name or 'ema' in feature_name:
            return 'technical'
        elif 'rsi' in feature_name or 'momentum' in feature_name:
            return 'momentum'
        elif 'volume' in feature_name:
            return 'volume'
        elif 'volatility' in feature_name:
            return 'volatility'
        else:
            return 'derived'
    
    def _get_calculation_method(self, feature_name: str) -> str:
        """Get calculation method description."""
        methods = {
            'sma_5': 'simple_moving_average_5',
            'sma_10': 'simple_moving_average_10',
            'sma_20': 'simple_moving_average_20',
            'ema_5': 'exponential_moving_average_5',
            'ema_10': 'exponential_moving_average_10',
            'rsi_14': 'relative_strength_index_14',
            'volume_sma_10': 'volume_simple_moving_average_10',
            'volume_ratio': 'current_volume_to_average_ratio',
            'price_momentum': 'price_momentum_5_periods',
            'volatility': 'rolling_standard_deviation_20'
        }
        return methods.get(feature_name, feature_name)
    
    def generate_predictions(self, prices_data: list) -> list:
        """Generate mock predictions for different models."""
        predictions = []
        models = ['NHITS', 'TCN', 'DeepAR', 'MLP']
        
        # Group prices by symbol
        symbol_prices = {}
        for price_data in prices_data:
            symbol = price_data['symbol']
            if symbol not in symbol_prices:
                symbol_prices[symbol] = []
            symbol_prices[symbol].append(price_data)
        
        for symbol, prices in symbol_prices.items():
            # Sort prices by time
            prices.sort(key=lambda x: x['time'])
            
            # Generate predictions for last 10 data points
            for price_data in prices[-10:]:
                current_time = price_data['time']
                current_price = price_data['price']
                
                for model_name in models:
                    # Generate prediction 5 minutes into the future
                    target_time = current_time + timedelta(minutes=5)
                    
                    # Simulate model-specific prediction variations
                    model_bias = {
                        'NHITS': 0.002,   # Slightly optimistic
                        'TCN': -0.001,    # Slightly pessimistic
                        'DeepAR': 0.0005, # Nearly neutral
                        'MLP': 0.001      # Slightly optimistic
                    }
                    
                    # Generate prediction with some noise
                    noise = np.random.normal(0, 0.01)  # 1% noise
                    predicted_price = current_price * (1 + model_bias[model_name] + noise)
                    
                    # Confidence based on recent volatility
                    confidence = max(0.5, min(0.95, 0.8 - abs(noise) * 5))
                    
                    predictions.append({
                        'model_name': model_name,
                        'symbol': symbol,
                        'prediction_time': current_time,
                        'target_time': target_time,
                        'predicted_price': round(predicted_price, 4),
                        'confidence_score': round(confidence, 2),
                        'model_version': 'v1.0',
                        'features_used': json.dumps({
                            'features_count': 10,
                            'lookback_window': 60,
                            'feature_types': ['technical', 'momentum', 'volume']
                        })
                    })
        
        return predictions
    
    def generate_sentiment_data(self, symbols: list, start_time: datetime, 
                              end_time: datetime) -> list:
        """Generate mock sentiment data."""
        sentiment_data = []
        sources = ['twitter', 'reddit', 'news', 'analyst_reports']
        
        current_time = start_time
        while current_time <= end_time:
            for symbol in symbols:
                for source in sources:
                    # Generate sentiment every hour for each source
                    if current_time.minute == 0:
                        sentiment_score = np.random.normal(0.1, 0.3)  # Slightly positive bias
                        sentiment_score = max(-1.0, min(1.0, sentiment_score))  # Clamp to [-1, 1]
                        
                        confidence = np.random.uniform(0.6, 0.95)
                        
                        sentiment_data.append({
                            'time': current_time,
                            'symbol': symbol,
                            'source': source,
                            'sentiment_score': round(sentiment_score, 2),
                            'confidence': round(confidence, 2),
                            'text_content': fake.sentence(nb_words=10),
                            'metadata': json.dumps({
                                'language': 'en',
                                'source_reliability': confidence,
                                'topic_relevance': np.random.uniform(0.7, 1.0)
                            })
                        })
            
            current_time += timedelta(minutes=60)  # Generate sentiment hourly
        
        return sentiment_data
    
    def insert_market_data(self, market_data: list):
        """Insert market data into database."""
        logger.info(f"Inserting {len(market_data)} market data records...")
        
        with self.connect_db() as conn:
            with conn.cursor() as cur:
                # Prepare insert query
                insert_query = """
                INSERT INTO market_data (time, symbol, price, volume, open_price, 
                                       high_price, low_price, close_price, provider, data_quality_score)
                VALUES (%(time)s, %(symbol)s, %(price)s, %(volume)s, %(open_price)s,
                        %(high_price)s, %(low_price)s, %(close_price)s, %(provider)s, 1.0)
                ON CONFLICT (time, symbol) DO UPDATE SET
                    price = EXCLUDED.price,
                    volume = EXCLUDED.volume,
                    open_price = EXCLUDED.open_price,
                    high_price = EXCLUDED.high_price,
                    low_price = EXCLUDED.low_price,
                    close_price = EXCLUDED.close_price
                """
                
                # Insert in batches
                batch_size = 1000
                for i in range(0, len(market_data), batch_size):
                    batch = market_data[i:i + batch_size]
                    cur.executemany(insert_query, batch)
                    logger.info(f"Inserted batch {i//batch_size + 1}")
                
                conn.commit()
        
        logger.info("Market data insertion completed")
    
    def insert_features(self, features: list):
        """Insert features into database."""
        logger.info(f"Inserting {len(features)} feature records...")
        
        with self.connect_db() as conn:
            with conn.cursor() as cur:
                insert_query = """
                INSERT INTO features (symbol, time, feature_name, feature_value, 
                                    feature_type, calculation_method)
                VALUES (%(symbol)s, %(time)s, %(feature_name)s, %(feature_value)s,
                        %(feature_type)s, %(calculation_method)s)
                """
                
                # Insert in batches
                batch_size = 1000
                for i in range(0, len(features), batch_size):
                    batch = features[i:i + batch_size]
                    cur.executemany(insert_query, batch)
                    logger.info(f"Inserted features batch {i//batch_size + 1}")
                
                conn.commit()
        
        logger.info("Features insertion completed")
    
    def insert_predictions(self, predictions: list):
        """Insert predictions into database."""
        logger.info(f"Inserting {len(predictions)} prediction records...")
        
        with self.connect_db() as conn:
            with conn.cursor() as cur:
                insert_query = """
                INSERT INTO predictions (model_name, symbol, prediction_time, target_time,
                                       predicted_price, confidence_score, model_version, features_used)
                VALUES (%(model_name)s, %(symbol)s, %(prediction_time)s, %(target_time)s,
                        %(predicted_price)s, %(confidence_score)s, %(model_version)s, %(features_used)s)
                """
                
                cur.executemany(insert_query, predictions)
                conn.commit()
        
        logger.info("Predictions insertion completed")
    
    def insert_sentiment_data(self, sentiment_data: list):
        """Insert sentiment data into database."""
        logger.info(f"Inserting {len(sentiment_data)} sentiment records...")
        
        with self.connect_db() as conn:
            with conn.cursor() as cur:
                insert_query = """
                INSERT INTO sentiment_data (time, symbol, source, sentiment_score, 
                                          confidence, text_content, metadata)
                VALUES (%(time)s, %(symbol)s, %(source)s, %(sentiment_score)s,
                        %(confidence)s, %(text_content)s, %(metadata)s)
                ON CONFLICT (time, symbol, source) DO UPDATE SET
                    sentiment_score = EXCLUDED.sentiment_score,
                    confidence = EXCLUDED.confidence,
                    text_content = EXCLUDED.text_content,
                    metadata = EXCLUDED.metadata
                """
                
                cur.executemany(insert_query, sentiment_data)
                conn.commit()
        
        logger.info("Sentiment data insertion completed")
    
    def generate_all_test_data(self):
        """Generate all types of test data."""
        logger.info("Starting comprehensive test data generation...")
        
        # Calculate time range
        end_time = datetime.now()
        start_time = end_time - timedelta(days=self.historical_days)
        
        logger.info(f"Generating data for {len(self.symbols)} symbols from {start_time} to {end_time}")
        
        all_market_data = []
        all_features = []
        
        # Generate data for each symbol
        for symbol in self.symbols:
            logger.info(f"Generating data for {symbol}...")
            
            # Generate price series
            prices = self.generate_price_series(symbol, start_time, end_time, freq_minutes=1)
            all_market_data.extend(prices)
            
            # Calculate features
            prices_df = pd.DataFrame(prices)
            features = self.calculate_technical_features(prices_df, symbol)
            all_features.extend(features)
        
        # Generate predictions and sentiment
        predictions = self.generate_predictions(all_market_data)
        sentiment_data = self.generate_sentiment_data(self.symbols, start_time, end_time)
        
        # Insert data into database
        self.insert_market_data(all_market_data)
        self.insert_features(all_features)
        self.insert_predictions(predictions)
        self.insert_sentiment_data(sentiment_data)
        
        # Save summary statistics
        summary = {
            'generation_completed': datetime.now().isoformat(),
            'data_period': {
                'start': start_time.isoformat(),
                'end': end_time.isoformat(),
                'days': self.historical_days
            },
            'symbols': self.symbols,
            'record_counts': {
                'market_data': len(all_market_data),
                'features': len(all_features),
                'predictions': len(predictions),
                'sentiment_data': len(sentiment_data)
            }
        }
        
        # Save summary to fixtures
        fixtures_dir = '/test-fixtures/generated'
        os.makedirs(fixtures_dir, exist_ok=True)
        with open(f'{fixtures_dir}/generation_summary.json', 'w') as f:
            json.dump(summary, f, indent=2)
        
        logger.info("Test data generation completed successfully!")
        logger.info(f"Summary: {json.dumps(summary['record_counts'], indent=2)}")


def main():
    """Main function."""
    # Get database URL from environment
    db_url = os.getenv('DATABASE_URL')
    if not db_url:
        logger.error("DATABASE_URL environment variable not set")
        sys.exit(1)
    
    # Create generator and run
    generator = TestDataGenerator(db_url)
    generator.generate_all_test_data()


if __name__ == '__main__':
    main()