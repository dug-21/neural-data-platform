"""Data aggregation utilities for combining multiple data sources."""
import pandas as pd
import numpy as np
from typing import List, Dict, Any, Optional, Tuple
from datetime import datetime
import asyncio

from utils.logging import get_logger
from utils.metrics import metrics


logger = get_logger(__name__)


class DataAggregator:
    """Aggregate data from multiple providers and sources."""
    
    def __init__(self):
        self.priority_order = [
            'polygon',  # Highest quality
            'iex_cloud',
            'finnhub',
            'alpha_vantage',
            'yahoo_finance'  # Free fallback
        ]
    
    def merge_market_data(
        self,
        data_sources: Dict[str, pd.DataFrame],
        method: str = 'priority'
    ) -> pd.DataFrame:
        """
        Merge market data from multiple sources.
        
        Args:
            data_sources: Dict mapping provider name to DataFrame
            method: Merge method - 'priority', 'average', 'consensus'
        
        Returns:
            Merged DataFrame
        """
        if not data_sources:
            return pd.DataFrame()
        
        # Filter out empty DataFrames
        valid_sources = {k: v for k, v in data_sources.items() if not v.empty}
        
        if not valid_sources:
            return pd.DataFrame()
        
        if len(valid_sources) == 1:
            # Only one source, return it
            df = list(valid_sources.values())[0].copy()
            df['provider'] = list(valid_sources.keys())[0]
            return df
        
        if method == 'priority':
            return self._merge_by_priority(valid_sources)
        elif method == 'average':
            return self._merge_by_average(valid_sources)
        elif method == 'consensus':
            return self._merge_by_consensus(valid_sources)
        else:
            raise ValueError(f"Unknown merge method: {method}")
    
    def _merge_by_priority(self, data_sources: Dict[str, pd.DataFrame]) -> pd.DataFrame:
        """Merge using provider priority order."""
        # Sort sources by priority
        sorted_sources = sorted(
            data_sources.items(),
            key=lambda x: self.priority_order.index(x[0]) if x[0] in self.priority_order else 999
        )
        
        # Start with highest priority source
        merged = sorted_sources[0][1].copy()
        merged['provider'] = sorted_sources[0][0]
        
        # Fill missing data from lower priority sources
        for provider, df in sorted_sources[1:]:
            # Align on time and symbol
            df_aligned = df.set_index(['time', 'symbol'])
            merged_aligned = merged.set_index(['time', 'symbol'])
            
            # Fill missing values
            for col in ['open', 'high', 'low', 'close', 'volume']:
                if col in df_aligned.columns and col in merged_aligned.columns:
                    merged_aligned[col] = merged_aligned[col].fillna(df_aligned[col])
            
            merged = merged_aligned.reset_index()
        
        return merged
    
    def _merge_by_average(self, data_sources: Dict[str, pd.DataFrame]) -> pd.DataFrame:
        """Merge by averaging values from all sources."""
        # Align all DataFrames on time and symbol
        aligned_dfs = []
        
        for provider, df in data_sources.items():
            df_copy = df.copy()
            df_copy['provider'] = provider
            aligned_dfs.append(df_copy.set_index(['time', 'symbol']))
        
        # Concatenate all sources
        combined = pd.concat(aligned_dfs)
        
        # Group by time and symbol, then average
        price_cols = ['open', 'high', 'low', 'close']
        agg_dict = {col: 'mean' for col in price_cols if col in combined.columns}
        agg_dict['volume'] = 'sum'  # Sum volumes
        agg_dict['provider'] = lambda x: ','.join(sorted(set(x)))  # List all providers
        
        merged = combined.groupby(['time', 'symbol']).agg(agg_dict)
        
        # Add statistics about the merge
        for col in price_cols:
            if col in combined.columns:
                merged[f'{col}_std'] = combined.groupby(['time', 'symbol'])[col].std()
                merged[f'{col}_count'] = combined.groupby(['time', 'symbol'])[col].count()
        
        return merged.reset_index()
    
    def _merge_by_consensus(self, data_sources: Dict[str, pd.DataFrame]) -> pd.DataFrame:
        """Merge using consensus (median) values with outlier detection."""
        aligned_dfs = []
        
        for provider, df in data_sources.items():
            df_copy = df.copy()
            df_copy['provider'] = provider
            aligned_dfs.append(df_copy.set_index(['time', 'symbol']))
        
        combined = pd.concat(aligned_dfs)
        
        # Calculate median and detect outliers
        price_cols = ['open', 'high', 'low', 'close']
        
        def consensus_agg(series):
            """Calculate consensus value excluding outliers."""
            if len(series) < 3:
                return series.mean()
            
            # Use IQR method for outlier detection
            q1 = series.quantile(0.25)
            q3 = series.quantile(0.75)
            iqr = q3 - q1
            lower_bound = q1 - 1.5 * iqr
            upper_bound = q3 + 1.5 * iqr
            
            # Filter outliers
            filtered = series[(series >= lower_bound) & (series <= upper_bound)]
            
            if len(filtered) > 0:
                return filtered.median()
            else:
                return series.median()
        
        # Apply consensus aggregation
        agg_dict = {col: consensus_agg for col in price_cols if col in combined.columns}
        agg_dict['volume'] = 'median'
        agg_dict['provider'] = lambda x: ','.join(sorted(set(x)))
        
        merged = combined.groupby(['time', 'symbol']).agg(agg_dict)
        
        # Add confidence scores
        for col in price_cols:
            if col in combined.columns:
                # Coefficient of variation as confidence measure
                merged[f'{col}_confidence'] = 1 - (
                    combined.groupby(['time', 'symbol'])[col].std() /
                    combined.groupby(['time', 'symbol'])[col].mean()
                ).fillna(0).clip(0, 1)
        
        return merged.reset_index()
    
    def reconcile_timestamps(
        self,
        dfs: List[pd.DataFrame],
        tolerance: str = '1min'
    ) -> List[pd.DataFrame]:
        """Reconcile timestamps across multiple DataFrames."""
        if not dfs or all(df.empty for df in dfs):
            return dfs
        
        # Convert tolerance to timedelta
        tolerance_td = pd.Timedelta(tolerance)
        
        reconciled = []
        
        for df in dfs:
            if df.empty or 'time' not in df.columns:
                reconciled.append(df)
                continue
            
            df_copy = df.copy()
            
            # Round timestamps to nearest interval
            df_copy['time'] = pd.to_datetime(df_copy['time'])
            df_copy['time'] = df_copy['time'].dt.round(tolerance)
            
            reconciled.append(df_copy)
        
        return reconciled
    
    def detect_arbitrage_opportunities(
        self,
        data_sources: Dict[str, pd.DataFrame],
        min_spread_pct: float = 0.1
    ) -> pd.DataFrame:
        """Detect price differences between providers that could indicate arbitrage."""
        opportunities = []
        
        # Get unique timestamps and symbols
        all_times = set()
        all_symbols = set()
        
        for df in data_sources.values():
            if not df.empty and 'time' in df.columns and 'symbol' in df.columns:
                all_times.update(df['time'].unique())
                all_symbols.update(df['symbol'].unique())
        
        # Check each time/symbol combination
        for time in all_times:
            for symbol in all_symbols:
                prices = {}
                
                # Collect prices from each provider
                for provider, df in data_sources.items():
                    mask = (df['time'] == time) & (df['symbol'] == symbol)
                    if mask.any():
                        close_price = df.loc[mask, 'close'].iloc[0]
                        if pd.notna(close_price) and close_price > 0:
                            prices[provider] = close_price
                
                # Check for arbitrage if we have multiple prices
                if len(prices) > 1:
                    min_price = min(prices.values())
                    max_price = max(prices.values())
                    spread_pct = (max_price - min_price) / min_price * 100
                    
                    if spread_pct >= min_spread_pct:
                        min_provider = min(prices, key=prices.get)
                        max_provider = max(prices, key=prices.get)
                        
                        opportunities.append({
                            'time': time,
                            'symbol': symbol,
                            'min_price': min_price,
                            'max_price': max_price,
                            'spread_pct': spread_pct,
                            'min_provider': min_provider,
                            'max_provider': max_provider,
                            'potential_profit': max_price - min_price
                        })
        
        if opportunities:
            logger.info(f"Found {len(opportunities)} potential arbitrage opportunities")
            metrics.arbitrage_opportunities.inc(len(opportunities))
        
        return pd.DataFrame(opportunities)
    
    def calculate_composite_price(
        self,
        prices: Dict[str, float],
        weights: Optional[Dict[str, float]] = None
    ) -> float:
        """Calculate weighted composite price from multiple sources."""
        if not prices:
            return 0.0
        
        if weights is None:
            # Default weights based on provider quality
            weights = {
                'polygon': 0.3,
                'iex_cloud': 0.25,
                'finnhub': 0.2,
                'alpha_vantage': 0.15,
                'yahoo_finance': 0.1
            }
        
        total_weight = 0
        weighted_sum = 0
        
        for provider, price in prices.items():
            weight = weights.get(provider, 0.1)  # Default weight
            weighted_sum += price * weight
            total_weight += weight
        
        return weighted_sum / total_weight if total_weight > 0 else 0.0
    
    def create_consensus_dataset(
        self,
        data_sources: Dict[str, pd.DataFrame],
        min_sources: int = 2
    ) -> pd.DataFrame:
        """Create a consensus dataset requiring minimum number of sources."""
        if not data_sources:
            return pd.DataFrame()
        
        # Combine all sources
        combined_dfs = []
        for provider, df in data_sources.items():
            if not df.empty:
                df_copy = df.copy()
                df_copy['provider'] = provider
                combined_dfs.append(df_copy)
        
        if not combined_dfs:
            return pd.DataFrame()
        
        combined = pd.concat(combined_dfs, ignore_index=True)
        
        # Count sources per time/symbol
        source_counts = combined.groupby(['time', 'symbol'])['provider'].nunique()
        
        # Filter for minimum sources
        valid_combinations = source_counts[source_counts >= min_sources].index
        
        # Create consensus data
        consensus_data = []
        
        for time, symbol in valid_combinations:
            mask = (combined['time'] == time) & (combined['symbol'] == symbol)
            group = combined[mask]
            
            consensus_record = {
                'time': time,
                'symbol': symbol,
                'open': group['open'].median(),
                'high': group['high'].max(),  # Use max for high
                'low': group['low'].min(),    # Use min for low
                'close': group['close'].median(),
                'volume': group['volume'].sum(),
                'source_count': len(group),
                'providers': ','.join(sorted(group['provider'].unique()))
            }
            
            # Add price agreement score
            close_prices = group['close'].dropna()
            if len(close_prices) > 1:
                consensus_record['price_agreement'] = 1 - (close_prices.std() / close_prices.mean())
            else:
                consensus_record['price_agreement'] = 1.0
            
            consensus_data.append(consensus_record)
        
        consensus_df = pd.DataFrame(consensus_data)
        
        logger.info(
            f"Created consensus dataset with {len(consensus_df)} records "
            f"from {len(data_sources)} sources"
        )
        
        return consensus_df