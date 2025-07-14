#!/usr/bin/env python3
"""Query AAPL records from TimescaleDB"""
import asyncio
from storage.timescale import TimescaleDB
from datetime import datetime

async def query_aapl_records():
    # Initialize TimescaleDB connection
    db = TimescaleDB()
    await db.connect()
    
    try:
        # Query all AAPL records
        query = """
        SELECT timestamp, symbol, open, high, low, close, volume
        FROM market_data
        WHERE symbol = 'AAPL'
        ORDER BY timestamp DESC
        LIMIT 100
        """
        
        async with db.pool.acquire() as conn:
            rows = await conn.fetch(query)
            
        print(f'Found {len(rows)} AAPL records in the database')
        print('\nLatest 10 records:')
        print('-' * 80)
        print(f"{'Timestamp':<20} {'Symbol':<6} {'Open':>8} {'High':>8} {'Low':>8} {'Close':>8} {'Volume':>12}")
        print('-' * 80)
        
        for row in rows[:10]:
            print(f"{str(row['timestamp']):<20} {row['symbol']:<6} {row['open']:>8.2f} {row['high']:>8.2f} {row['low']:>8.2f} {row['close']:>8.2f} {row['volume']:>12,}")
            
        # Get summary statistics
        if rows:
            query_stats = """
            SELECT 
                COUNT(*) as total_records,
                MIN(timestamp) as earliest_date,
                MAX(timestamp) as latest_date,
                AVG(close) as avg_close,
                MIN(close) as min_close,
                MAX(close) as max_close
            FROM market_data
            WHERE symbol = 'AAPL'
            """
            
            stats = await conn.fetchrow(query_stats)
            
            print('\nAAPL Summary Statistics:')
            print('-' * 50)
            print(f"Total Records: {stats['total_records']:,}")
            print(f"Date Range: {stats['earliest_date']} to {stats['latest_date']}")
            print(f"Average Close: ${stats['avg_close']:.2f}")
            print(f"Price Range: ${stats['min_close']:.2f} - ${stats['max_close']:.2f}")
            
    except Exception as e:
        print(f"Error querying database: {e}")
        print("\nTrying alternative query without table...")
        
        # Try to check if database/table exists
        try:
            async with db.pool.acquire() as conn:
                # Check tables
                tables_query = """
                SELECT table_name 
                FROM information_schema.tables 
                WHERE table_schema = 'public'
                """
                tables = await conn.fetch(tables_query)
                print("\nAvailable tables:")
                for table in tables:
                    print(f"  - {table['table_name']}")
        except Exception as e2:
            print(f"Could not list tables: {e2}")
            
    finally:
        await db.disconnect()

if __name__ == "__main__":
    asyncio.run(query_aapl_records())