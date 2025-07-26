#!/usr/bin/env python3
"""
Load testing script for data backfill with 600+ symbols.

This script tests the backfill system's ability to handle large-scale
data imports with many symbols concurrently.
"""

import os
import sys
import time
import json
import asyncio
import argparse
import random
from datetime import datetime, timedelta
from typing import List, Dict, Any
import multiprocessing as mp
from concurrent.futures import ProcessPoolExecutor, ThreadPoolExecutor

# Add parent to path
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from providers.file_provider import FileProvider
from utils.file_backfill import FileBackfillHandler
from utils.logging import get_logger
from utils.metrics import metrics
from storage.timescale import TimescaleDB

logger = get_logger(__name__)


class LoadTester:
    """Load testing coordinator for backfill operations."""
    
    # S&P 500 + additional symbols for 600+ total
    SP500_SYMBOLS = [
        'AAPL', 'MSFT', 'AMZN', 'GOOGL', 'META', 'TSLA', 'BRK.B', 'UNH', 'JNJ', 'JPM',
        'V', 'PG', 'XOM', 'NVDA', 'HD', 'CVX', 'MA', 'BAC', 'ABBV', 'PFE',
        'AVGO', 'COST', 'DIS', 'KO', 'PEP', 'WMT', 'CSCO', 'MRK', 'VZ', 'CMCSA',
        'ADBE', 'TMO', 'CRM', 'ABT', 'NKE', 'NFLX', 'ACN', 'ORCL', 'MCD', 'PM',
        'NEE', 'DHR', 'TXN', 'LIN', 'WFC', 'UPS', 'BMY', 'RTX', 'AMD', 'QCOM',
        'T', 'LOW', 'SBUX', 'AMT', 'HON', 'INTU', 'CVS', 'COP', 'UNP', 'IBM',
        'GS', 'LMT', 'ELV', 'CAT', 'BA', 'SCHW', 'SPGI', 'MDT', 'BLK', 'GILD',
        'AXP', 'DE', 'C', 'ISRG', 'MO', 'ADI', 'PLD', 'CI', 'MDLZ', 'TMUS',
        'PYPL', 'CB', 'TJX', 'ZTS', 'MMC', 'REGN', 'SYK', 'AMGN', 'BDX', 'VRTX',
        'EOG', 'SO', 'NOW', 'DUK', 'PGR', 'SLB', 'NOC', 'CSX', 'BSX', 'ITW',
        # Add more symbols to reach 600+
    ]
    
    def __init__(self, config: Dict[str, Any]):
        """Initialize load tester with configuration."""
        self.config = config
        self.symbols = self._generate_symbol_list(config['num_symbols'])
        self.start_date = datetime.strptime(config['start_date'], '%Y-%m-%d')
        self.end_date = datetime.strptime(config['end_date'], '%Y-%m-%d')
        self.num_workers = config.get('num_workers', mp.cpu_count())
        self.batch_size = config.get('batch_size', 10000)
        self.results = {
            'start_time': None,
            'end_time': None,
            'total_records': 0,
            'total_errors': 0,
            'symbol_stats': {},
            'performance_metrics': {}
        }
    
    def _generate_symbol_list(self, count: int) -> List[str]:
        """Generate list of symbols for testing."""
        # Start with real S&P 500 symbols
        symbols = list(self.SP500_SYMBOLS)
        
        # Add synthetic symbols to reach desired count
        while len(symbols) < count:
            # Generate realistic-looking symbols
            prefix = random.choice(['A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J'])
            suffix = ''.join(random.choices('ABCDEFGHIJKLMNOPQRSTUVWXYZ', k=random.randint(2, 4)))
            symbol = f"{prefix}{suffix}"
            if symbol not in symbols:
                symbols.append(symbol)
        
        return symbols[:count]
    
    async def generate_test_data(self, output_dir: str):
        """Generate test data files for load testing."""
        logger.info(f"Generating test data for {len(self.symbols)} symbols")
        
        os.makedirs(output_dir, exist_ok=True)
        
        # Generate data for each day in range
        current_date = self.start_date
        files_created = 0
        
        while current_date <= self.end_date:
            # Skip weekends
            if current_date.weekday() >= 5:
                current_date += timedelta(days=1)
                continue
            
            # Create directory structure
            date_dir = os.path.join(
                output_dir,
                str(current_date.year),
                f"{current_date.month:02d}",
                f"{current_date.day:02d}"
            )
            os.makedirs(date_dir, exist_ok=True)
            
            # Generate CSV file with all symbols
            file_path = os.path.join(date_dir, f"market_data_{current_date.strftime('%Y%m%d')}.csv")
            
            with open(file_path, 'w') as f:
                # Write header
                f.write("timestamp,symbol,open,high,low,close,volume\n")
                
                # Generate data for each symbol
                for symbol in self.symbols:
                    # Generate minute-level data (390 minutes per trading day)
                    base_price = random.uniform(10, 500)
                    
                    for minute in range(390):  # 9:30 AM to 4:00 PM
                        timestamp = current_date.replace(hour=9, minute=30) + timedelta(minutes=minute)
                        
                        # Generate realistic OHLC data
                        open_price = base_price * random.uniform(0.98, 1.02)
                        high_price = open_price * random.uniform(1.0, 1.01)
                        low_price = open_price * random.uniform(0.99, 1.0)
                        close_price = random.uniform(low_price, high_price)
                        volume = random.randint(1000, 1000000)
                        
                        f.write(f"{timestamp.isoformat()},{symbol},{open_price:.2f},"
                               f"{high_price:.2f},{low_price:.2f},{close_price:.2f},{volume}\n")
                        
                        base_price = close_price
            
            files_created += 1
            logger.info(f"Created test file: {file_path}")
            
            # Compress file
            os.system(f"gzip {file_path}")
            
            current_date += timedelta(days=1)
        
        logger.info(f"Generated {files_created} test data files")
    
    async def run_load_test(self, data_dir: str):
        """Run the load test with generated data."""
        logger.info(f"Starting load test with {len(self.symbols)} symbols")
        
        self.results['start_time'] = datetime.utcnow()
        
        # Initialize components
        provider = FileProvider(
            base_path=data_dir,
            checkpoint_dir="/tmp/load_test_checkpoints"
        )
        
        storage = TimescaleDB()
        
        try:
            await provider.connect()
            await storage.connect()
            
            # Divide symbols among workers
            symbol_chunks = [
                self.symbols[i:i + len(self.symbols) // self.num_workers]
                for i in range(0, len(self.symbols), len(self.symbols) // self.num_workers)
            ]
            
            # Create tasks for parallel processing
            tasks = []
            for i, chunk in enumerate(symbol_chunks):
                task = asyncio.create_task(
                    self._process_symbol_chunk(
                        worker_id=i,
                        symbols=chunk,
                        provider=provider,
                        storage=storage
                    )
                )
                tasks.append(task)
            
            # Monitor progress
            monitor_task = asyncio.create_task(self._monitor_progress())
            
            # Wait for all tasks
            results = await asyncio.gather(*tasks, return_exceptions=True)
            
            # Cancel monitor
            monitor_task.cancel()
            
            # Process results
            for i, result in enumerate(results):
                if isinstance(result, Exception):
                    logger.error(f"Worker {i} failed: {result}")
                    self.results['total_errors'] += 1
            
        finally:
            await provider.disconnect()
            await storage.disconnect()
        
        self.results['end_time'] = datetime.utcnow()
        
        # Calculate final metrics
        duration = (self.results['end_time'] - self.results['start_time']).total_seconds()
        self.results['performance_metrics'] = {
            'total_duration_seconds': duration,
            'avg_records_per_second': self.results['total_records'] / duration if duration > 0 else 0,
            'symbols_processed': len(self.symbols),
            'error_rate': self.results['total_errors'] / self.results['total_records'] if self.results['total_records'] > 0 else 0
        }
        
        return self.results
    
    async def _process_symbol_chunk(
        self,
        worker_id: int,
        symbols: List[str],
        provider: FileProvider,
        storage: TimescaleDB
    ):
        """Process a chunk of symbols in parallel."""
        logger.info(f"Worker {worker_id} processing {len(symbols)} symbols")
        
        worker_stats = {
            'records': 0,
            'errors': 0,
            'start_time': datetime.utcnow()
        }
        
        try:
            # Process data for each symbol
            batch = []
            async for data in provider.get_market_data(
                symbols=symbols,
                start_time=self.start_date,
                end_time=self.end_date
            ):
                batch.append({
                    'time': data.time,
                    'symbol': data.symbol,
                    'open': data.open,
                    'high': data.high,
                    'low': data.low,
                    'close': data.close,
                    'volume': data.volume,
                    'provider': 'load_test'
                })
                
                # Insert batch when full
                if len(batch) >= self.batch_size:
                    try:
                        await storage.insert_market_data(batch)
                        worker_stats['records'] += len(batch)
                        batch = []
                    except Exception as e:
                        logger.error(f"Worker {worker_id} insert error: {e}")
                        worker_stats['errors'] += 1
            
            # Insert remaining records
            if batch:
                await storage.insert_market_data(batch)
                worker_stats['records'] += len(batch)
                
        except Exception as e:
            logger.error(f"Worker {worker_id} failed: {e}")
            worker_stats['errors'] += 1
        
        # Update global results
        self.results['total_records'] += worker_stats['records']
        self.results['total_errors'] += worker_stats['errors']
        
        worker_stats['end_time'] = datetime.utcnow()
        worker_stats['duration'] = (worker_stats['end_time'] - worker_stats['start_time']).total_seconds()
        
        logger.info(f"Worker {worker_id} completed: {worker_stats['records']} records in {worker_stats['duration']:.2f}s")
        
        return worker_stats
    
    async def _monitor_progress(self):
        """Monitor and report progress during load test."""
        while True:
            try:
                await asyncio.sleep(10)  # Report every 10 seconds
                
                if self.results['start_time']:
                    elapsed = (datetime.utcnow() - self.results['start_time']).total_seconds()
                    rate = self.results['total_records'] / elapsed if elapsed > 0 else 0
                    
                    logger.info(
                        f"Progress: {self.results['total_records']:,} records processed "
                        f"({rate:,.0f} records/sec) - "
                        f"Errors: {self.results['total_errors']}"
                    )
                    
                    # Check system resources
                    import psutil
                    memory = psutil.virtual_memory()
                    cpu = psutil.cpu_percent(interval=1)
                    
                    logger.info(
                        f"System: CPU {cpu}%, Memory {memory.percent}% "
                        f"({memory.used / (1024**3):.1f} GB used)"
                    )
                    
            except asyncio.CancelledError:
                break
            except Exception as e:
                logger.error(f"Monitor error: {e}")
    
    def generate_report(self, output_file: str):
        """Generate detailed load test report."""
        report = {
            'test_configuration': self.config,
            'results': self.results,
            'recommendations': []
        }
        
        # Add recommendations based on results
        if self.results['performance_metrics']['avg_records_per_second'] < 10000:
            report['recommendations'].append(
                "Performance below target. Consider increasing batch size or workers."
            )
        
        if self.results['performance_metrics']['error_rate'] > 0.01:
            report['recommendations'].append(
                "High error rate detected. Review error logs for patterns."
            )
        
        # Save report
        with open(output_file, 'w') as f:
            json.dump(report, f, indent=2, default=str)
        
        logger.info(f"Load test report saved to {output_file}")
        
        # Print summary
        print("\n" + "=" * 80)
        print("LOAD TEST SUMMARY")
        print("=" * 80)
        print(f"Symbols tested: {len(self.symbols)}")
        print(f"Date range: {self.config['start_date']} to {self.config['end_date']}")
        print(f"Total records: {self.results['total_records']:,}")
        print(f"Total errors: {self.results['total_errors']:,}")
        print(f"Duration: {self.results['performance_metrics']['total_duration_seconds']:.2f} seconds")
        print(f"Average rate: {self.results['performance_metrics']['avg_records_per_second']:,.0f} records/sec")
        print(f"Error rate: {self.results['performance_metrics']['error_rate']:.4%}")
        print("=" * 80)


async def main():
    """Main entry point for load testing."""
    parser = argparse.ArgumentParser(
        description='Load test the data backfill system with 600+ symbols'
    )
    
    parser.add_argument(
        '--symbols',
        type=int,
        default=600,
        help='Number of symbols to test (default: 600)'
    )
    parser.add_argument(
        '--start-date',
        default='2023-01-01',
        help='Start date (YYYY-MM-DD)'
    )
    parser.add_argument(
        '--end-date',
        default='2023-01-31',
        help='End date (YYYY-MM-DD)'
    )
    parser.add_argument(
        '--workers',
        type=int,
        default=mp.cpu_count(),
        help='Number of parallel workers'
    )
    parser.add_argument(
        '--batch-size',
        type=int,
        default=10000,
        help='Records per batch'
    )
    parser.add_argument(
        '--data-dir',
        default='/tmp/load_test_data',
        help='Directory for test data'
    )
    parser.add_argument(
        '--generate-data',
        action='store_true',
        help='Generate test data files'
    )
    parser.add_argument(
        '--report',
        default='load_test_report.json',
        help='Output report file'
    )
    
    args = parser.parse_args()
    
    # Configure logging
    logging.basicConfig(
        level=logging.INFO,
        format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
    )
    
    # Create configuration
    config = {
        'num_symbols': args.symbols,
        'start_date': args.start_date,
        'end_date': args.end_date,
        'num_workers': args.workers,
        'batch_size': args.batch_size
    }
    
    # Initialize load tester
    tester = LoadTester(config)
    
    # Generate test data if requested
    if args.generate_data:
        await tester.generate_test_data(args.data_dir)
    
    # Run load test
    results = await tester.run_load_test(args.data_dir)
    
    # Generate report
    tester.generate_report(args.report)


if __name__ == '__main__':
    asyncio.run(main())