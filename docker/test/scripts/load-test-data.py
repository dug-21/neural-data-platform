#!/usr/bin/env python3
"""
Test data loading script for TimescaleDB initialization.
This script is run during database initialization to load test fixtures.
"""
import os
import sys
import json
import logging
from datetime import datetime, timedelta

import psycopg2
from psycopg2.extras import RealDictCursor

# Setup logging
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger(__name__)


def connect_db():
    """Connect to PostgreSQL database."""
    # Try to connect using standard PostgreSQL environment variables
    try:
        conn = psycopg2.connect(
            host=os.getenv('POSTGRES_HOST', 'localhost'),
            port=os.getenv('POSTGRES_PORT', '5432'),
            database=os.getenv('POSTGRES_DB', 'neural_trader_test'),
            user=os.getenv('POSTGRES_USER', 'test_user'),
            password=os.getenv('POSTGRES_PASSWORD', 'test_password_123')
        )
        return conn
    except Exception as e:
        logger.error(f"Database connection failed: {e}")
        return None


def load_test_fixtures():
    """Load test data fixtures into database."""
    logger.info("Loading test fixtures...")
    
    conn = connect_db()
    if not conn:
        logger.error("Could not connect to database, skipping fixture loading")
        return
    
    try:
        with conn.cursor(cursor_factory=RealDictCursor) as cur:
            # Load market data fixtures
            test_fixtures_path = '/test-data'
            if os.path.exists(test_fixtures_path):
                logger.info(f"Loading fixtures from {test_fixtures_path}")
                
                # Look for JSON fixture files
                for filename in os.listdir(test_fixtures_path):
                    if filename.endswith('.json'):
                        filepath = os.path.join(test_fixtures_path, filename)
                        try:
                            with open(filepath, 'r') as f:
                                fixture_data = json.load(f)
                            logger.info(f"Loaded fixture: {filename}")
                            
                            # Process different types of fixtures
                            if 'market_data' in fixture_data:
                                load_market_data_fixture(cur, fixture_data['market_data'])
                            
                            if 'features' in fixture_data:
                                load_features_fixture(cur, fixture_data['features'])
                                
                        except Exception as e:
                            logger.warning(f"Could not load fixture {filename}: {e}")
                            continue
            else:
                logger.info("No test fixtures directory found, creating sample data")
                create_sample_test_data(cur)
            
            conn.commit()
            logger.info("Test fixtures loaded successfully")
            
    except Exception as e:
        logger.error(f"Error loading test fixtures: {e}")
        if conn:
            conn.rollback()
    finally:
        if conn:
            conn.close()


def load_market_data_fixture(cursor, market_data):
    """Load market data from fixture."""
    logger.info(f"Loading {len(market_data)} market data records")
    
    insert_query = """
    INSERT INTO market_data (time, symbol, price, volume, open_price, 
                           high_price, low_price, close_price, provider, data_quality_score)
    VALUES (%(time)s, %(symbol)s, %(price)s, %(volume)s, %(open_price)s,
            %(high_price)s, %(low_price)s, %(close_price)s, %(provider)s, %(data_quality_score)s)
    ON CONFLICT (time, symbol) DO NOTHING
    """
    
    # Prepare data
    prepared_data = []
    for record in market_data:
        prepared_data.append({
            'time': record.get('timestamp', record.get('time')),
            'symbol': record['symbol'],
            'price': float(record.get('price', record.get('close', 100.0))),
            'volume': int(record.get('volume', 1000)),
            'open_price': float(record.get('open', record.get('open_price', 100.0))),
            'high_price': float(record.get('high', record.get('high_price', 101.0))),
            'low_price': float(record.get('low', record.get('low_price', 99.0))),
            'close_price': float(record.get('close', record.get('close_price', 100.0))),
            'provider': record.get('provider', 'test_fixture'),
            'data_quality_score': float(record.get('data_quality_score', 1.0))
        })
    
    cursor.executemany(insert_query, prepared_data)
    logger.info(f"Inserted {len(prepared_data)} market data records")


def load_features_fixture(cursor, features):
    """Load features from fixture."""
    logger.info(f"Loading {len(features)} feature records")
    
    insert_query = """
    INSERT INTO features (symbol, time, feature_name, feature_value, 
                        feature_type, calculation_method)
    VALUES (%(symbol)s, %(time)s, %(feature_name)s, %(feature_value)s,
            %(feature_type)s, %(calculation_method)s)
    """
    
    cursor.executemany(insert_query, features)
    logger.info(f"Inserted {len(features)} feature records")


def create_sample_test_data(cursor):
    """Create minimal sample test data when no fixtures are available."""
    logger.info("Creating minimal sample test data")
    
    symbols = ['AAPL', 'MSFT', 'GOOGL']
    base_prices = {'AAPL': 150.0, 'MSFT': 280.0, 'GOOGL': 2150.0}
    
    # Create sample market data for the last hour
    current_time = datetime.now()
    sample_data = []
    
    for symbol in symbols:
        base_price = base_prices[symbol]
        
        for i in range(10):  # 10 data points
            timestamp = current_time - timedelta(minutes=i * 6)  # Every 6 minutes
            price = base_price * (1 + (i % 3 - 1) * 0.01)  # Small variations
            
            sample_data.append({
                'time': timestamp,
                'symbol': symbol,
                'price': price,
                'volume': 100000 + i * 10000,
                'open_price': price * 0.999,
                'high_price': price * 1.002,
                'low_price': price * 0.998,
                'close_price': price,
                'provider': 'sample',
                'data_quality_score': 1.0
            })
    
    # Insert sample data
    insert_query = """
    INSERT INTO market_data (time, symbol, price, volume, open_price, 
                           high_price, low_price, close_price, provider, data_quality_score)
    VALUES (%(time)s, %(symbol)s, %(price)s, %(volume)s, %(open_price)s,
            %(high_price)s, %(low_price)s, %(close_price)s, %(provider)s, %(data_quality_score)s)
    ON CONFLICT (time, symbol) DO NOTHING
    """
    
    cursor.executemany(insert_query, sample_data)
    logger.info(f"Created {len(sample_data)} sample market data records")
    
    # Create sample features
    sample_features = []
    for symbol in symbols:
        sample_features.extend([
            {
                'symbol': symbol,
                'time': current_time - timedelta(minutes=5),
                'feature_name': 'sma_5',
                'feature_value': base_prices[symbol] * 1.001,
                'feature_type': 'technical',
                'calculation_method': 'simple_moving_average'
            },
            {
                'symbol': symbol,
                'time': current_time - timedelta(minutes=5),
                'feature_name': 'rsi_14',
                'feature_value': 65.5,
                'feature_type': 'momentum',
                'calculation_method': 'relative_strength_index'
            }
        ])
    
    # Insert sample features
    feature_query = """
    INSERT INTO features (symbol, time, feature_name, feature_value, 
                        feature_type, calculation_method)
    VALUES (%(symbol)s, %(time)s, %(feature_name)s, %(feature_value)s,
            %(feature_type)s, %(calculation_method)s)
    """
    
    cursor.executemany(feature_query, sample_features)
    logger.info(f"Created {len(sample_features)} sample feature records")


def verify_test_data():
    """Verify that test data was loaded correctly."""
    logger.info("Verifying test data...")
    
    conn = connect_db()
    if not conn:
        logger.error("Could not connect to database for verification")
        return False
    
    try:
        with conn.cursor(cursor_factory=RealDictCursor) as cur:
            # Check market data
            cur.execute("SELECT COUNT(*) as count, COUNT(DISTINCT symbol) as symbols FROM market_data")
            market_stats = cur.fetchone()
            logger.info(f"Market data: {market_stats['count']} records, {market_stats['symbols']} symbols")
            
            # Check features
            cur.execute("SELECT COUNT(*) as count, COUNT(DISTINCT feature_name) as features FROM features")
            feature_stats = cur.fetchone()
            logger.info(f"Features: {feature_stats['count']} records, {feature_stats['features']} feature types")
            
            # Check recent data
            cur.execute("""
                SELECT symbol, MAX(time) as latest_time 
                FROM market_data 
                GROUP BY symbol 
                ORDER BY symbol
            """)
            recent_data = cur.fetchall()
            
            logger.info("Latest data per symbol:")
            for row in recent_data:
                logger.info(f"  {row['symbol']}: {row['latest_time']}")
            
            return True
            
    except Exception as e:
        logger.error(f"Error during verification: {e}")
        return False
    finally:
        if conn:
            conn.close()


def main():
    """Main function."""
    logger.info("Starting test data loading process...")
    
    # Wait a moment for database to be fully ready
    import time
    time.sleep(2)
    
    load_test_fixtures()
    
    if verify_test_data():
        logger.info("Test data loading completed successfully!")
        sys.exit(0)
    else:
        logger.error("Test data loading failed verification")
        sys.exit(1)


if __name__ == '__main__':
    main()