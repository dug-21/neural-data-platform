#!/usr/bin/env python3
"""
Extract specific symbols from downloaded Polygon flatfiles.
Processes compressed CSV files and outputs filtered data for selected symbols.
"""

import os
import sys
import gzip
import argparse
import pandas as pd
from pathlib import Path
from typing import List, Set
from datetime import datetime
import logging

# Setup logging
logging.basicConfig(level=logging.INFO, format='%(asctime)s - %(levelname)s - %(message)s')
logger = logging.getLogger(__name__)


def extract_symbols_from_file(file_path: Path, symbols: Set[str], output_dir: Path) -> int:
    """
    Extract specific symbols from a single compressed CSV file.
    
    Args:
        file_path: Path to the compressed CSV file
        symbols: Set of symbols to extract
        output_dir: Directory to save filtered output
        
    Returns:
        Number of records extracted
    """
    try:
        logger.info(f"Processing: {file_path}")
        
        # Read compressed CSV
        with gzip.open(file_path, 'rt') as f:
            df = pd.read_csv(f)
        
        # Filter for specific symbols
        filtered_df = df[df['ticker'].isin(symbols)]
        
        if filtered_df.empty:
            logger.debug(f"No data found for symbols {symbols} in {file_path}")
            return 0
        
        # Create output filename based on input file
        date_str = file_path.stem.replace('.csv', '')  # Remove .csv from .csv.gz
        output_file = output_dir / f"{date_str}_filtered.csv"
        
        # Save filtered data
        filtered_df.to_csv(output_file, index=False)
        
        record_count = len(filtered_df)
        logger.info(f"Extracted {record_count} records to {output_file}")
        
        return record_count
        
    except Exception as e:
        logger.error(f"Error processing {file_path}: {e}")
        return 0


def extract_symbols_from_directory(
    input_dir: Path, 
    symbols: List[str], 
    output_dir: Path,
    start_date: str = None,
    end_date: str = None
) -> None:
    """
    Extract symbols from all CSV.gz files in a directory tree.
    
    Args:
        input_dir: Directory containing polygon_data
        symbols: List of symbols to extract
        output_dir: Directory to save filtered files
        start_date: Optional start date (YYYY-MM-DD)
        end_date: Optional end date (YYYY-MM-DD)
    """
    symbols_set = set(symbols)
    total_records = 0
    processed_files = 0
    
    # Create output directory
    output_dir.mkdir(parents=True, exist_ok=True)
    
    # Find all .csv.gz files recursively
    csv_files = list(input_dir.rglob("*.csv.gz"))
    
    # Filter by date range if specified
    if start_date or end_date:
        filtered_files = []
        for file_path in csv_files:
            # Extract date from filename (YYYY-MM-DD.csv.gz)
            try:
                date_str = file_path.stem.replace('.csv', '')
                file_date = datetime.strptime(date_str, '%Y-%m-%d')
                
                if start_date:
                    start_dt = datetime.strptime(start_date, '%Y-%m-%d')
                    if file_date < start_dt:
                        continue
                
                if end_date:
                    end_dt = datetime.strptime(end_date, '%Y-%m-%d')
                    if file_date > end_dt:
                        continue
                
                filtered_files.append(file_path)
                
            except ValueError:
                logger.warning(f"Could not parse date from filename: {file_path}")
                # Include file anyway if date parsing fails
                filtered_files.append(file_path)
        
        csv_files = filtered_files
    
    logger.info(f"Found {len(csv_files)} files to process")
    logger.info(f"Extracting symbols: {', '.join(symbols)}")
    
    # Process each file
    for file_path in sorted(csv_files):
        records = extract_symbols_from_file(file_path, symbols_set, output_dir)
        total_records += records
        processed_files += 1
        
        # Progress update every 50 files
        if processed_files % 50 == 0:
            logger.info(f"Progress: {processed_files}/{len(csv_files)} files processed, {total_records:,} total records")
    
    logger.info(f"Extraction complete!")
    logger.info(f"Files processed: {processed_files}")
    logger.info(f"Total records extracted: {total_records:,}")
    logger.info(f"Output directory: {output_dir}")


def merge_daily_files(output_dir: Path, symbols: List[str]) -> None:
    """
    Merge all daily filtered files into symbol-specific files.
    
    Args:
        output_dir: Directory containing filtered daily files
        symbols: List of symbols to create merged files for
    """
    logger.info("Merging daily files by symbol...")
    
    # Find all filtered CSV files
    csv_files = list(output_dir.glob("*_filtered.csv"))
    
    if not csv_files:
        logger.warning("No filtered CSV files found to merge")
        return
    
    # Create merged directory
    merged_dir = output_dir / "merged"
    merged_dir.mkdir(exist_ok=True)
    
    for symbol in symbols:
        symbol_data = []
        
        for csv_file in sorted(csv_files):
            try:
                df = pd.read_csv(csv_file)
                symbol_df = df[df['ticker'] == symbol]
                
                if not symbol_df.empty:
                    symbol_data.append(symbol_df)
                    
            except Exception as e:
                logger.error(f"Error reading {csv_file}: {e}")
        
        if symbol_data:
            # Combine all data for this symbol
            combined_df = pd.concat(symbol_data, ignore_index=True)
            
            # Sort by timestamp
            if 'window_start' in combined_df.columns:
                combined_df = combined_df.sort_values('window_start')
            
            # Save to merged file
            output_file = merged_dir / f"{symbol}_5year_minutes.csv"
            combined_df.to_csv(output_file, index=False)
            
            logger.info(f"Created {output_file} with {len(combined_df):,} records")


def main():
    """Main entry point."""
    parser = argparse.ArgumentParser(
        description="Extract specific symbols from Polygon flatfiles",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  # Extract AAPL and MSFT from last 5 years
  python extract_symbols.py \\
    --input /Volumes/OneTouch/trader/polygon_data \\
    --symbols AAPL,MSFT \\
    --output /Volumes/OneTouch/trader/filtered_data
  
  # Extract with date range
  python extract_symbols.py \\
    --input /Volumes/OneTouch/trader/polygon_data \\
    --symbols AAPL,MSFT,GOOGL \\
    --output /Volumes/OneTouch/trader/filtered_data \\
    --start-date 2022-01-01 \\
    --end-date 2024-12-31
  
  # Extract and merge into symbol-specific files
  python extract_symbols.py \\
    --input /Volumes/OneTouch/trader/polygon_data \\
    --symbols AAPL,MSFT \\
    --output /Volumes/OneTouch/trader/filtered_data \\
    --merge
        """
    )
    
    parser.add_argument(
        '--input',
        required=True,
        type=Path,
        help='Input directory containing polygon_data'
    )
    
    parser.add_argument(
        '--symbols',
        required=True,
        help='Comma-separated list of symbols to extract (e.g., AAPL,MSFT,GOOGL)'
    )
    
    parser.add_argument(
        '--output',
        required=True,
        type=Path,
        help='Output directory for filtered data'
    )
    
    parser.add_argument(
        '--start-date',
        help='Start date (YYYY-MM-DD)'
    )
    
    parser.add_argument(
        '--end-date',
        help='End date (YYYY-MM-DD)'
    )
    
    parser.add_argument(
        '--merge',
        action='store_true',
        help='Merge daily files into symbol-specific files'
    )
    
    parser.add_argument(
        '--verbose',
        action='store_true',
        help='Enable verbose logging'
    )
    
    args = parser.parse_args()
    
    if args.verbose:
        logging.getLogger().setLevel(logging.DEBUG)
    
    # Parse symbols
    symbols = [s.strip().upper() for s in args.symbols.split(',')]
    
    # Validate input directory
    if not args.input.exists():
        logger.error(f"Input directory does not exist: {args.input}")
        sys.exit(1)
    
    # Extract symbols
    try:
        extract_symbols_from_directory(
            input_dir=args.input,
            symbols=symbols,
            output_dir=args.output,
            start_date=args.start_date,
            end_date=args.end_date
        )
        
        # Merge files if requested
        if args.merge:
            merge_daily_files(args.output, symbols)
            
    except Exception as e:
        logger.error(f"Extraction failed: {e}")
        sys.exit(1)


if __name__ == '__main__':
    main()