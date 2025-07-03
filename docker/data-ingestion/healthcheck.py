#!/usr/bin/env python3
"""
Health check script for data ingestion service
"""
import sys
import requests
import psycopg2
import redis
from urllib.parse import urlparse
import os

def check_service_health():
    """Check if the data ingestion service is healthy"""
    try:
        # Check if the metrics endpoint is responding
        response = requests.get('http://localhost:8000/metrics', timeout=5)
        if response.status_code != 200:
            print(f"Metrics endpoint returned status {response.status_code}")
            return False
            
        # Check Redis connection
        redis_url = os.getenv('REDIS_URL', 'redis://localhost:6379/0')
        r = redis.from_url(redis_url)
        r.ping()
        
        # Check PostgreSQL connection
        db_url = os.getenv('DATABASE_URL', 'postgresql://neural_trader:neural_trader_pass@localhost:5432/neural_trader_db')
        parsed = urlparse(db_url)
        
        conn = psycopg2.connect(
            host=parsed.hostname,
            port=parsed.port or 5432,
            user=parsed.username,
            password=parsed.password,
            database=parsed.path[1:],
            connect_timeout=5
        )
        
        with conn.cursor() as cur:
            cur.execute('SELECT 1')
            
        conn.close()
        
        print("Health check passed")
        return True
        
    except Exception as e:
        print(f"Health check failed: {str(e)}")
        return False

if __name__ == '__main__':
    sys.exit(0 if check_service_health() else 1)