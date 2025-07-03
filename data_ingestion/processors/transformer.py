"""Data transformation utilities for market data."""
import pandas as pd
import numpy as np
from typing import List, Dict, Any, Optional, Union
from datetime import datetime
import ta  # Technical Analysis library

from ..utils.logging import get_logger
from ..utils.metrics import metrics


logger = get_logger(__name__)


class DataTransformer:
    """Transform market data for analysis and storage."""
    
    def __init__(self):
        self.resample_rules = {
            '1min': '1T',
            '5min': '5T',
            '15min': '15T',
            '30min': '30T',
            '1hour': '1H',
            '4hour': '4H',
            '1day': '1D',
            '1week': '1W',
            '1month': '1M'
        }
    
    def transform_to_ohlcv(self, tick_data: List[Dict[str, Any]], interval: str = '1min') -> pd.DataFrame:
        """Transform tick data to OHLCV bars."""
        if not tick_data:
            return pd.DataFrame()
        
        # Convert to DataFrame
        df = pd.DataFrame(tick_data)
        
        # Ensure time column is datetime
        df['time'] = pd.to_datetime(df['time'])
        
        # Set time as index
        df.set_index('time', inplace=True)
        
        # Group by symbol if multiple symbols
        if 'symbol' in df.columns and df['symbol'].nunique() > 1:
            ohlcv_dfs = []
            
            for symbol, group in df.groupby('symbol'):
                ohlcv = self._aggregate_to_ohlcv(group, interval)
                ohlcv['symbol'] = symbol
                ohlcv_dfs.append(ohlcv)
            
            return pd.concat(ohlcv_dfs) if ohlcv_dfs else pd.DataFrame()
        else:
            return self._aggregate_to_ohlcv(df, interval)
    
    def _aggregate_to_ohlcv(self, df: pd.DataFrame, interval: str) -> pd.DataFrame:
        """Aggregate tick data to OHLCV format."""
        rule = self.resample_rules.get(interval, '1T')
        
        # Resample to create OHLCV bars
        ohlcv = df['price'].resample(rule).agg({
            'open': 'first',
            'high': 'max',
            'low': 'min',
            'close': 'last'
        })
        
        # Add volume if available
        if 'size' in df.columns:
            ohlcv['volume'] = df['size'].resample(rule).sum()
        else:
            ohlcv['volume'] = df['price'].resample(rule).count()
        
        # Remove empty bars
        ohlcv = ohlcv.dropna()
        
        # Reset index to have time as column
        ohlcv.reset_index(inplace=True)
        
        return ohlcv
    
    def resample_ohlcv(self, df: pd.DataFrame, target_interval: str) -> pd.DataFrame:
        """Resample OHLCV data to different time interval."""
        if df.empty:
            return df
        
        rule = self.resample_rules.get(target_interval, '1T')
        
        # Ensure time index
        if 'time' in df.columns:
            df = df.set_index('time')
        
        # Group by symbol if needed
        if 'symbol' in df.columns and df['symbol'].nunique() > 1:
            resampled_dfs = []
            
            for symbol, group in df.groupby('symbol'):
                resampled = self._resample_group(group, rule)
                resampled['symbol'] = symbol
                resampled_dfs.append(resampled)
            
            return pd.concat(resampled_dfs) if resampled_dfs else pd.DataFrame()
        else:
            return self._resample_group(df, rule)
    
    def _resample_group(self, df: pd.DataFrame, rule: str) -> pd.DataFrame:
        """Resample a single group of OHLCV data."""
        agg_dict = {
            'open': 'first',
            'high': 'max',
            'low': 'min',
            'close': 'last',
            'volume': 'sum'
        }
        
        # Only aggregate columns that exist
        agg_dict = {k: v for k, v in agg_dict.items() if k in df.columns}
        
        resampled = df.resample(rule).agg(agg_dict)
        resampled = resampled.dropna()
        resampled.reset_index(inplace=True)
        
        return resampled
    
    def add_technical_indicators(self, df: pd.DataFrame) -> pd.DataFrame:
        """Add technical indicators to OHLCV data."""
        if df.empty or len(df) < 20:  # Need minimum data for indicators
            return df
        
        # Work with copy to avoid modifying original
        df = df.copy()
        
        # Ensure we have required columns
        required_cols = ['open', 'high', 'low', 'close', 'volume']
        if not all(col in df.columns for col in required_cols):
            logger.warning("Missing required columns for technical indicators")
            return df
        
        try:
            # Trend Indicators
            df['sma_20'] = ta.trend.sma_indicator(df['close'], window=20)
            df['sma_50'] = ta.trend.sma_indicator(df['close'], window=50)
            df['ema_12'] = ta.trend.ema_indicator(df['close'], window=12)
            df['ema_26'] = ta.trend.ema_indicator(df['close'], window=26)
            
            # MACD
            macd = ta.trend.MACD(df['close'])
            df['macd'] = macd.macd()
            df['macd_signal'] = macd.macd_signal()
            df['macd_diff'] = macd.macd_diff()
            
            # RSI
            df['rsi'] = ta.momentum.RSIIndicator(df['close']).rsi()
            
            # Bollinger Bands
            bb = ta.volatility.BollingerBands(df['close'])
            df['bb_upper'] = bb.bollinger_hband()
            df['bb_middle'] = bb.bollinger_mavg()
            df['bb_lower'] = bb.bollinger_lband()
            df['bb_width'] = bb.bollinger_wband()
            
            # ATR (Average True Range)
            df['atr'] = ta.volatility.average_true_range(df['high'], df['low'], df['close'])
            
            # Volume indicators
            df['volume_sma'] = ta.volume.volume_weighted_average_price(
                df['high'], df['low'], df['close'], df['volume']
            )
            
            # OBV (On Balance Volume)
            df['obv'] = ta.volume.on_balance_volume(df['close'], df['volume'])
            
            # Stochastic Oscillator
            stoch = ta.momentum.StochasticOscillator(df['high'], df['low'], df['close'])
            df['stoch_k'] = stoch.stoch()
            df['stoch_d'] = stoch.stoch_signal()
            
            logger.info(f"Added technical indicators to {len(df)} records")
            
        except Exception as e:
            logger.error(f"Failed to add technical indicators: {e}")
        
        return df
    
    def normalize_data(self, df: pd.DataFrame, method: str = 'minmax') -> pd.DataFrame:
        """Normalize numerical data."""
        if df.empty:
            return df
        
        df = df.copy()
        
        # Identify numerical columns (exclude time and symbol)
        numeric_cols = df.select_dtypes(include=[np.number]).columns.tolist()
        exclude_cols = ['time', 'symbol']
        numeric_cols = [col for col in numeric_cols if col not in exclude_cols]
        
        if not numeric_cols:
            return df
        
        if method == 'minmax':
            # Min-Max normalization (0-1 range)
            for col in numeric_cols:
                min_val = df[col].min()
                max_val = df[col].max()
                if max_val > min_val:
                    df[f'{col}_norm'] = (df[col] - min_val) / (max_val - min_val)
        
        elif method == 'zscore':
            # Z-score normalization
            for col in numeric_cols:
                mean_val = df[col].mean()
                std_val = df[col].std()
                if std_val > 0:
                    df[f'{col}_zscore'] = (df[col] - mean_val) / std_val
        
        elif method == 'log':
            # Log transformation (for positive values)
            for col in numeric_cols:
                if df[col].min() > 0:  # Only for positive values
                    df[f'{col}_log'] = np.log(df[col])
        
        return df
    
    def calculate_returns(self, df: pd.DataFrame) -> pd.DataFrame:
        """Calculate various return metrics."""
        if df.empty or 'close' not in df.columns:
            return df
        
        df = df.copy()
        
        # Sort by time to ensure correct calculation
        if 'time' in df.columns:
            df = df.sort_values('time')
        
        # Simple returns
        df['returns'] = df['close'].pct_change()
        
        # Log returns
        df['log_returns'] = np.log(df['close'] / df['close'].shift(1))
        
        # Multi-period returns
        for period in [5, 10, 20]:  # 5, 10, 20 periods
            if len(df) > period:
                df[f'returns_{period}p'] = df['close'].pct_change(periods=period)
        
        # Cumulative returns
        df['cumulative_returns'] = (1 + df['returns']).cumprod() - 1
        
        # Rolling volatility (20-period)
        if len(df) > 20:
            df['volatility_20'] = df['returns'].rolling(window=20).std() * np.sqrt(252)  # Annualized
        
        return df
    
    def create_features(self, df: pd.DataFrame) -> pd.DataFrame:
        """Create additional features for ML models."""
        if df.empty:
            return df
        
        df = df.copy()
        
        # Price-based features
        if 'close' in df.columns:
            # Price ratios
            df['high_low_ratio'] = df['high'] / df['low']
            df['close_open_ratio'] = df['close'] / df['open']
            
            # Price position in range
            df['price_position'] = (df['close'] - df['low']) / (df['high'] - df['low'])
            
            # Gap features
            df['gap_open'] = df['open'] - df['close'].shift(1)
            df['gap_percent'] = df['gap_open'] / df['close'].shift(1) * 100
        
        # Volume features
        if 'volume' in df.columns:
            df['volume_ratio'] = df['volume'] / df['volume'].rolling(window=20).mean()
            df['dollar_volume'] = df['close'] * df['volume']
        
        # Time-based features
        if 'time' in df.columns:
            df['hour'] = pd.to_datetime(df['time']).dt.hour
            df['day_of_week'] = pd.to_datetime(df['time']).dt.dayofweek
            df['day_of_month'] = pd.to_datetime(df['time']).dt.day
            df['month'] = pd.to_datetime(df['time']).dt.month
            
            # Trading session features
            df['is_market_hours'] = df['hour'].between(9, 16)
            df['is_morning'] = df['hour'].between(9, 12)
            df['is_afternoon'] = df['hour'].between(12, 16)
        
        # Lag features
        for lag in [1, 2, 3, 5, 10]:
            if len(df) > lag:
                df[f'close_lag_{lag}'] = df['close'].shift(lag)
                df[f'volume_lag_{lag}'] = df['volume'].shift(lag)
        
        # Rolling statistics
        for window in [5, 10, 20]:
            if len(df) > window:
                df[f'close_mean_{window}'] = df['close'].rolling(window=window).mean()
                df[f'close_std_{window}'] = df['close'].rolling(window=window).std()
                df[f'volume_mean_{window}'] = df['volume'].rolling(window=window).mean()
        
        return df
    
    def prepare_for_storage(self, data: Union[List[Dict], pd.DataFrame]) -> List[Dict[str, Any]]:
        """Prepare data for database storage."""
        if isinstance(data, pd.DataFrame):
            # Convert DataFrame to list of dicts
            df = data.copy()
            
            # Ensure time is in correct format
            if 'time' in df.columns:
                df['time'] = pd.to_datetime(df['time'])
            
            # Replace NaN/inf with None for database compatibility
            df = df.replace([np.inf, -np.inf], np.nan)
            records = df.where(pd.notnull(df), None).to_dict('records')
        else:
            records = data
        
        # Ensure all records have required fields
        cleaned_records = []
        for record in records:
            # Convert numpy types to Python types
            cleaned_record = {}
            for key, value in record.items():
                if isinstance(value, np.integer):
                    cleaned_record[key] = int(value)
                elif isinstance(value, np.floating):
                    cleaned_record[key] = float(value)
                elif isinstance(value, np.bool_):
                    cleaned_record[key] = bool(value)
                elif pd.isna(value):
                    cleaned_record[key] = None
                else:
                    cleaned_record[key] = value
            
            cleaned_records.append(cleaned_record)
        
        return cleaned_records