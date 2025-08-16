"""
Dynamic Data Type Discovery System for Neural Trading Platform

This module implements a dynamic data type registry that discovers and registers 
data types at runtime without hardcoded assumptions. The system enables models 
to specify requirements by data characteristics rather than specific type names.

Key Features:
- Runtime data type discovery and registration
- Characteristic-based data matching (frequency, scope, nature, quality)
- Automatic model-data compatibility assessment
- Flexible data ingestion from any Redis channel structure
- Support for multi-scope data routing (symbol, market, sector, geographic)

Author: Data-Pipeline-Dev2
Date: 2025-08-02
"""

import json
import logging
import time
from abc import ABC, abstractmethod
from dataclasses import dataclass, field
from datetime import datetime, timedelta
from enum import Enum
from typing import Dict, List, Optional, Set, Any, Tuple, Callable
import asyncio
import redis.asyncio as redis
import numpy as np
from collections import defaultdict, deque


# =============================================================================
# Core Data Characteristics Framework
# =============================================================================

class DataFrequency(Enum):
    """Data update frequency categories"""
    REAL_TIME = "1s"      # Sub-second to 1 second updates
    MINUTE = "1m"         # 1 minute intervals  
    FIVE_MINUTE = "5m"    # 5 minute intervals
    HOUR = "1h"           # Hourly updates
    DAILY = "1d"          # Daily updates
    WEEKLY = "1w"         # Weekly updates
    MONTHLY = "1M"        # Monthly updates
    QUARTERLY = "1Q"      # Quarterly updates
    
    @property
    def seconds(self) -> int:
        """Convert frequency to seconds"""
        mapping = {
            "1s": 1,
            "1m": 60,
            "5m": 300,
            "1h": 3600,
            "1d": 86400,
            "1w": 604800,
            "1M": 2592000,  # Approximate
            "1Q": 7776000   # Approximate
        }
        return mapping[self.value]


class DataScope(Enum):
    """Data scope and aggregation level"""
    SYMBOL = "symbol"           # Individual symbol/ticker
    SECTOR = "sector"          # Sector-wide data
    MARKET = "market"          # Market-wide indicators
    GEOGRAPHIC = "geographic"   # Geographic/regional data
    GLOBAL = "global"          # Global economic indicators


class DataNature(Enum):
    """Nature/type of the data content"""
    PRICE = "price"                    # OHLCV price data
    VOLUME = "volume"                  # Trading volume data
    SENTIMENT = "sentiment"            # News/social sentiment
    ECONOMIC = "economic"              # Economic indicators
    FUNDAMENTAL = "fundamental"        # Company fundamentals
    TECHNICAL = "technical"            # Technical indicators
    ALTERNATIVE = "alternative"        # Alternative data sources
    MICROSTRUCTURE = "microstructure"  # Market microstructure
    DERIVATIVES = "derivatives"        # Options/futures data
    CORRELATION = "correlation"        # Cross-asset correlations


class DataQuality(Enum):
    """Data quality/importance level for model requirements"""
    REQUIRED = "required"      # Absolutely required for model
    PREFERRED = "preferred"    # Strongly preferred but not required
    OPTIONAL = "optional"      # Nice to have but not critical
    DEPRECATED = "deprecated"  # Legacy data, use if available


@dataclass
class DataCharacteristics:
    """Complete characterization of a data type"""
    frequency: DataFrequency
    scope: DataScope
    nature: DataNature
    quality: DataQuality
    
    # Additional metadata
    feature_count: Optional[int] = None
    latency_ms: Optional[int] = None
    reliability_score: float = 1.0
    coverage_ratio: float = 1.0  # What percentage of symbols/markets covered
    
    # Data freshness tracking
    max_staleness_seconds: Optional[int] = None
    typical_delay_seconds: int = 0
    
    # Source metadata
    source_channels: Set[str] = field(default_factory=set)
    data_format: Optional[str] = None
    validation_rules: Dict[str, Any] = field(default_factory=dict)
    
    def matches_requirements(self, required_chars: 'DataCharacteristics') -> Tuple[bool, float]:
        """
        Check if this data type matches model requirements.
        Returns (matches, compatibility_score)
        """
        # Calculate compatibility score
        score = 0.0
        
        # Nature must match exactly for any compatibility
        if self.nature != required_chars.nature:
            return False, 0.0
        
        score += 0.4  # Base score for matching nature
        
        # Frequency compatibility (higher frequency can substitute lower)
        if self.frequency.seconds <= required_chars.frequency.seconds:
            score += 0.25
        elif self.frequency.seconds <= required_chars.frequency.seconds * 2:
            score += 0.15  # Close enough
        elif self.frequency.seconds <= required_chars.frequency.seconds * 5:
            score += 0.05  # Acceptable for some use cases
        
        # Scope compatibility
        if self.scope == required_chars.scope:
            score += 0.25
        elif self._scope_compatible(required_chars.scope):
            score += 0.15
        
        # Quality compatibility
        quality_scores = {
            DataQuality.REQUIRED: 0.1,
            DataQuality.PREFERRED: 0.08,
            DataQuality.OPTIONAL: 0.05,
            DataQuality.DEPRECATED: 0.02
        }
        
        # If model requires REQUIRED quality, we must be at least PREFERRED
        if required_chars.quality == DataQuality.REQUIRED:
            if self.quality in [DataQuality.REQUIRED, DataQuality.PREFERRED]:
                score += quality_scores.get(self.quality, 0.0)
            else:
                score *= 0.5  # Penalty for lower quality
        else:
            score += quality_scores.get(self.quality, 0.0)
        
        # Reliability and coverage bonuses
        score += 0.05 * self.reliability_score
        score += 0.05 * self.coverage_ratio
        
        # Lower threshold for matching - 0.4 is more reasonable
        return score >= 0.4, min(score, 1.0)
    
    def _scope_compatible(self, required_scope: DataScope) -> bool:
        """Check if scopes are compatible (e.g., symbol data can work for sector models)"""
        # This scope can satisfy the required scope
        compatibility_map = {
            DataScope.SYMBOL: {DataScope.SYMBOL, DataScope.SECTOR, DataScope.MARKET, DataScope.GEOGRAPHIC, DataScope.GLOBAL},
            DataScope.SECTOR: {DataScope.SECTOR, DataScope.MARKET, DataScope.GEOGRAPHIC, DataScope.GLOBAL},
            DataScope.MARKET: {DataScope.MARKET, DataScope.GEOGRAPHIC, DataScope.GLOBAL},
            DataScope.GEOGRAPHIC: {DataScope.GEOGRAPHIC, DataScope.GLOBAL},
            DataScope.GLOBAL: {DataScope.GLOBAL}
        }
        return required_scope in compatibility_map.get(self.scope, set())


# =============================================================================
# Dynamic Data Type Registry
# =============================================================================

@dataclass
class DiscoveredDataType:
    """A dynamically discovered data type"""
    type_id: str
    characteristics: DataCharacteristics
    sample_data: Optional[Dict[str, Any]] = None
    discovery_timestamp: datetime = field(default_factory=datetime.utcnow)
    last_seen: datetime = field(default_factory=datetime.utcnow)
    seen_count: int = 1
    
    # Schema information
    schema: Dict[str, str] = field(default_factory=dict)  # field_name -> data_type
    required_fields: Set[str] = field(default_factory=set)
    optional_fields: Set[str] = field(default_factory=set)
    
    # Performance tracking
    average_size_bytes: float = 0.0
    processing_time_ms: float = 0.0
    error_rate: float = 0.0


@dataclass
class ModelDataRequirement:
    """Model's data requirements specification"""
    model_id: str
    required_data: List[DataCharacteristics]
    optional_data: List[DataCharacteristics] = field(default_factory=list)
    
    # Constraints
    min_feature_count: int = 1
    max_latency_ms: int = 1000
    min_reliability: float = 0.8
    min_coverage: float = 0.9
    
    # Compatibility preferences
    frequency_tolerance: int = 2  # How many frequency levels can be substituted
    scope_flexibility: bool = True
    quality_threshold: float = 0.7


class DataTypeDiscoveryStrategy(ABC):
    """Abstract base class for data type discovery strategies"""
    
    @abstractmethod
    async def discover_from_sample(self, data: Dict[str, Any], 
                                 channel: str, metadata: Dict[str, Any]) -> Optional[DataCharacteristics]:
        """Discover data characteristics from a sample"""
        pass


class HeuristicDiscoveryStrategy(DataTypeDiscoveryStrategy):
    """Heuristic-based data type discovery using field analysis and patterns"""
    
    def __init__(self):
        self.pattern_rules = self._initialize_pattern_rules()
    
    def _initialize_pattern_rules(self) -> Dict[str, Callable]:
        """Initialize pattern recognition rules"""
        return {
            'price_patterns': self._detect_price_data,
            'volume_patterns': self._detect_volume_data,
            'sentiment_patterns': self._detect_sentiment_data,
            'economic_patterns': self._detect_economic_data,
            'technical_patterns': self._detect_technical_data,
            'fundamental_patterns': self._detect_fundamental_data
        }
    
    async def discover_from_sample(self, data: Dict[str, Any], 
                                 channel: str, metadata: Dict[str, Any]) -> Optional[DataCharacteristics]:
        """Discover data characteristics using heuristic analysis"""
        
        # Analyze field names and values
        fields = set(data.keys())
        
        # Detect data nature
        nature = self._detect_data_nature(fields, data)
        if not nature:
            return None
        
        # Detect scope from channel name and content
        scope = self._detect_data_scope(channel, fields, data)
        
        # Detect frequency from metadata and patterns
        frequency = self._detect_data_frequency(channel, metadata, data)
        
        # Determine quality based on completeness and reliability indicators
        quality = self._determine_data_quality(fields, data, metadata)
        
        # Extract additional characteristics
        feature_count = len([k for k, v in data.items() if isinstance(v, (int, float))])
        reliability_score = self._calculate_reliability_score(data, metadata)
        
        return DataCharacteristics(
            frequency=frequency,
            scope=scope,
            nature=nature,
            quality=quality,
            feature_count=feature_count,
            reliability_score=reliability_score,
            source_channels={channel},
            data_format='json'
        )
    
    def _detect_data_nature(self, fields: Set[str], data: Dict[str, Any]) -> Optional[DataNature]:
        """Detect the nature of data based on field analysis"""
        field_lower = {f.lower() for f in fields}
        
        # Price data patterns
        if self._detect_price_data(field_lower, data):
            return DataNature.PRICE
        
        # Volume data patterns
        if self._detect_volume_data(field_lower, data):
            return DataNature.VOLUME
        
        # Sentiment data patterns
        if self._detect_sentiment_data(field_lower, data):
            return DataNature.SENTIMENT
        
        # Economic indicators
        if self._detect_economic_data(field_lower, data):
            return DataNature.ECONOMIC
        
        # Technical indicators
        if self._detect_technical_data(field_lower, data):
            return DataNature.TECHNICAL
        
        # Fundamental data
        if self._detect_fundamental_data(field_lower, data):
            return DataNature.FUNDAMENTAL
        
        return None
    
    def _detect_price_data(self, fields: Set[str], data: Dict[str, Any]) -> bool:
        """Detect OHLCV price data"""
        price_indicators = {'open', 'high', 'low', 'close', 'ohlc', 'price', 'bid', 'ask'}
        volume_indicators = {'volume', 'vol', 'trade_volume'}
        
        has_price = any(indicator in fields for indicator in price_indicators)
        has_ohlc = len(price_indicators.intersection(fields)) >= 2
        
        return has_price or has_ohlc
    
    def _detect_volume_data(self, fields: Set[str], data: Dict[str, Any]) -> bool:
        """Detect volume-focused data"""
        volume_indicators = {'volume', 'vol', 'trade_volume', 'share_volume', 'dollar_volume'}
        # Only return True if volume is the primary data (not just present with price data)
        has_volume = any(indicator in fields for indicator in volume_indicators)
        has_price = any(price in fields for price in {'open', 'high', 'low', 'close', 'price'})
        
        # If both volume and price are present, prefer price data type
        if has_volume and has_price:
            return False
        
        return has_volume
    
    def _detect_sentiment_data(self, fields: Set[str], data: Dict[str, Any]) -> bool:
        """Detect sentiment data"""
        sentiment_indicators = {
            'sentiment', 'news_sentiment', 'social_sentiment', 'analyst_sentiment',
            'bullish', 'bearish', 'positive', 'negative', 'confidence'
        }
        return any(indicator in fields for indicator in sentiment_indicators)
    
    def _detect_economic_data(self, fields: Set[str], data: Dict[str, Any]) -> bool:
        """Detect economic indicators"""
        economic_indicators = {
            'gdp', 'inflation', 'unemployment', 'interest_rate', 'pmi', 'cpi',
            'economic', 'macro', 'fed', 'central_bank', 'yield', 'treasury'
        }
        return any(indicator in fields for indicator in economic_indicators)
    
    def _detect_technical_data(self, fields: Set[str], data: Dict[str, Any]) -> bool:
        """Detect technical indicators"""
        technical_indicators = {
            'sma', 'ema', 'rsi', 'macd', 'bollinger', 'atr', 'stochastic',
            'moving_average', 'oscillator', 'momentum', 'volatility'
        }
        return any(indicator in fields for indicator in technical_indicators)
    
    def _detect_fundamental_data(self, fields: Set[str], data: Dict[str, Any]) -> bool:
        """Detect fundamental data"""
        fundamental_indicators = {
            'pe_ratio', 'pb_ratio', 'debt_equity', 'roe', 'roa', 'revenue',
            'earnings', 'cash_flow', 'dividend', 'market_cap', 'fundamental'
        }
        return any(indicator in fields for indicator in fundamental_indicators)
    
    def _detect_data_scope(self, channel: str, fields: Set[str], data: Dict[str, Any]) -> DataScope:
        """Detect data scope from channel name and content"""
        channel_lower = channel.lower()
        field_lower = {f.lower() for f in fields}
        
        # Global indicators
        if any(global_ind in channel_lower for global_ind in ['global', 'world', 'international']):
            return DataScope.GLOBAL
        
        # Geographic indicators
        if any(geo in channel_lower for geo in ['region', 'country', 'geographic', 'us', 'eu', 'asia']):
            return DataScope.GEOGRAPHIC
        
        # Market-wide indicators  
        if any(market in channel_lower for market in ['market_wide', 'index', 'spy', 'qqq', 'broad']):
            return DataScope.MARKET
        
        # Sector indicators
        if any(sector in channel_lower for sector in ['sector', 'industry', 'technology', 'financial', 'healthcare', 'energy', 'xlf', 'xlt', 'xlk']):
            return DataScope.SECTOR
        
        # Symbol-specific - check both data content and channel
        if ('symbol' in data or 'ticker' in data or 
            any(field in field_lower for field in {'symbol', 'ticker', 'instrument'}) or
            'symbol' in channel_lower):
            return DataScope.SYMBOL
        
        # Economic data typically regional
        if any(econ in field_lower for econ in {'gdp', 'inflation', 'unemployment', 'interest_rate'}):
            return DataScope.GEOGRAPHIC
        
        return DataScope.SYMBOL  # Default assumption for financial data
    
    def _detect_data_frequency(self, channel: str, metadata: Dict[str, Any], 
                             data: Dict[str, Any]) -> DataFrequency:
        """Detect update frequency from various signals"""
        channel_lower = channel.lower()
        
        # Real-time indicators
        if any(rt in channel_lower for rt in ['realtime', 'live', 'streaming', 'tick']):
            return DataFrequency.REAL_TIME
        
        # Minute data indicators
        if any(min_ind in channel_lower for min_ind in ['1min', 'minute', '1m']):
            return DataFrequency.MINUTE
        
        # 5-minute indicators
        if any(min5_ind in channel_lower for min5_ind in ['5min', '5m']):
            return DataFrequency.FIVE_MINUTE
        
        # Hourly indicators
        if any(hour_ind in channel_lower for hour_ind in ['hourly', '1h', 'hour']):
            return DataFrequency.HOUR
        
        # Daily indicators
        if any(daily_ind in channel_lower for daily_ind in ['daily', '1d', 'day', 'eod']):
            return DataFrequency.DAILY
        
        # Weekly indicators
        if any(weekly_ind in channel_lower for weekly_ind in ['weekly', '1w', 'week']):
            return DataFrequency.WEEKLY
        
        # Monthly/quarterly for fundamentals and economic data
        if any(period in channel_lower for period in ['monthly', 'quarterly', 'annual']):
            if 'economic' in channel_lower or 'fundamental' in channel_lower:
                return DataFrequency.QUARTERLY
            return DataFrequency.MONTHLY
        
        # Default based on data nature
        if any(nature in channel_lower for nature in ['sentiment', 'news']):
            return DataFrequency.HOUR
        
        return DataFrequency.MINUTE  # Conservative default
    
    def _determine_data_quality(self, fields: Set[str], data: Dict[str, Any], 
                              metadata: Dict[str, Any]) -> DataQuality:
        """Determine data quality level"""
        # Check for quality indicators in metadata
        if metadata.get('quality') == 'premium':
            return DataQuality.REQUIRED
        
        # Check data completeness
        non_null_ratio = len([v for v in data.values() if v is not None]) / len(data)
        
        if non_null_ratio >= 0.95:
            return DataQuality.PREFERRED
        elif non_null_ratio >= 0.8:
            return DataQuality.OPTIONAL
        else:
            return DataQuality.DEPRECATED
    
    def _calculate_reliability_score(self, data: Dict[str, Any], metadata: Dict[str, Any]) -> float:
        """Calculate reliability score based on data quality indicators"""
        score = 0.8  # Base score
        
        # Adjust based on data completeness
        non_null_count = len([v for v in data.values() if v is not None])
        completeness = non_null_count / len(data)
        score *= completeness
        
        # Adjust based on metadata quality indicators
        if metadata.get('source_reliability'):
            score *= metadata['source_reliability']
        
        # Adjust based on timestamp freshness
        if 'timestamp' in data:
            try:
                timestamp = datetime.fromisoformat(str(data['timestamp']).replace('Z', '+00:00'))
                age_seconds = (datetime.utcnow().replace(tzinfo=timestamp.tzinfo) - timestamp).total_seconds()
                if age_seconds < 60:  # Very fresh data
                    score *= 1.1
                elif age_seconds > 3600:  # Stale data
                    score *= 0.9
            except:
                pass
        
        return min(score, 1.0)


class DynamicDataTypeRegistry:
    """
    Central registry for dynamic data type discovery and management.
    
    Key capabilities:
    - Runtime discovery and registration of new data types
    - Characteristic-based matching between models and available data
    - Automatic model activation when data requirements are met
    - Performance tracking and optimization recommendations
    """
    
    def __init__(self, redis_client: Optional[redis.Redis] = None):
        self.logger = logging.getLogger(__name__)
        self.redis_client = redis_client
        
        # Registry storage
        self.discovered_types: Dict[str, DiscoveredDataType] = {}
        self.model_requirements: Dict[str, ModelDataRequirement] = {}
        
        # Discovery strategies
        self.discovery_strategies: List[DataTypeDiscoveryStrategy] = [
            HeuristicDiscoveryStrategy()
        ]
        
        # Performance tracking
        self.discovery_stats = {
            'total_discoveries': 0,
            'successful_matches': 0,
            'failed_matches': 0,
            'average_discovery_time_ms': 0.0
        }
        
        # Channel monitoring
        self.monitored_channels: Set[str] = set()
        self.channel_data_history: Dict[str, deque] = defaultdict(lambda: deque(maxlen=100))
        
        # Cache for frequent lookups
        self._compatibility_cache: Dict[Tuple[str, str], Tuple[bool, float]] = {}
        self._last_cache_cleanup = time.time()
    
    async def register_type(self, data: Dict[str, Any], channel: str, 
                          metadata: Optional[Dict[str, Any]] = None) -> Optional[str]:
        """
        Discover and register a new data type from sample data.
        
        Args:
            data: Sample data to analyze
            channel: Redis channel or data source identifier
            metadata: Additional metadata about the data source
            
        Returns:
            Type ID if successfully discovered and registered, None otherwise
        """
        start_time = time.time()
        metadata = metadata or {}
        
        try:
            # Attempt discovery with available strategies
            characteristics = None
            for strategy in self.discovery_strategies:
                characteristics = await strategy.discover_from_sample(data, channel, metadata)
                if characteristics:
                    break
            
            if not characteristics:
                self.logger.warning(f"Could not discover data type for channel {channel}")
                return None
            
            # Generate unique type ID
            type_id = self._generate_type_id(characteristics, channel)
            
            # Check if we already have this type
            if type_id in self.discovered_types:
                # Update existing type
                existing = self.discovered_types[type_id]
                existing.last_seen = datetime.utcnow()
                existing.seen_count += 1
                existing.characteristics.source_channels.add(channel)
                self._update_schema(existing, data)
                self.logger.debug(f"Updated existing data type {type_id}")
                return type_id
            
            # Create new discovered type
            discovered_type = DiscoveredDataType(
                type_id=type_id,
                characteristics=characteristics,
                sample_data=data.copy(),
                schema=self._infer_schema(data),
                required_fields=self._identify_required_fields(data),
                optional_fields=self._identify_optional_fields(data),
                average_size_bytes=len(json.dumps(data).encode('utf-8'))
            )
            
            # Register the type
            self.discovered_types[type_id] = discovered_type
            
            # Update monitoring
            self.monitored_channels.add(channel)
            self.channel_data_history[channel].append({
                'timestamp': datetime.utcnow(),
                'type_id': type_id,
                'data_size': len(json.dumps(data))
            })
            
            # Update statistics
            discovery_time_ms = (time.time() - start_time) * 1000
            self.discovery_stats['total_discoveries'] += 1
            self.discovery_stats['average_discovery_time_ms'] = (
                (self.discovery_stats['average_discovery_time_ms'] * 
                 (self.discovery_stats['total_discoveries'] - 1) + discovery_time_ms) /
                self.discovery_stats['total_discoveries']
            )
            
            self.logger.info(f"Registered new data type {type_id} for channel {channel}")
            
            # Trigger model matching for this new type
            await self._check_model_activations(type_id)
            
            return type_id
            
        except Exception as e:
            self.logger.error(f"Error registering data type for channel {channel}: {e}")
            return None
    
    async def discover_type(self, data: Dict[str, Any], channel: str) -> Optional[DataCharacteristics]:
        """
        Discover data type characteristics without registering.
        
        Args:
            data: Sample data to analyze
            channel: Data source channel
            
        Returns:
            Discovered characteristics or None
        """
        for strategy in self.discovery_strategies:
            characteristics = await strategy.discover_from_sample(data, channel, {})
            if characteristics:
                return characteristics
        return None
    
    async def match_available(self, model_id: str) -> List[Tuple[str, float]]:
        """
        Find available data types that match a model's requirements.
        
        Args:
            model_id: Model identifier
            
        Returns:
            List of (type_id, compatibility_score) tuples, sorted by score
        """
        if model_id not in self.model_requirements:
            self.logger.warning(f"No requirements found for model {model_id}")
            return []
        
        requirements = self.model_requirements[model_id]
        matches = []
        
        for type_id, discovered_type in self.discovered_types.items():
            # Check cache first
            cache_key = (model_id, type_id)
            if cache_key in self._compatibility_cache:
                is_compatible, score = self._compatibility_cache[cache_key]
                if is_compatible:
                    matches.append((type_id, score))
                continue
            
            # Check compatibility with required data
            best_score = 0.0
            is_compatible = False
            
            for required_char in requirements.required_data:
                compatible, score = discovered_type.characteristics.matches_requirements(required_char)
                if compatible:
                    is_compatible = True
                    best_score = max(best_score, score)
            
            # Check optional data for additional scoring
            if is_compatible:
                for optional_char in requirements.optional_data:
                    compatible, score = discovered_type.characteristics.matches_requirements(optional_char)
                    if compatible:
                        best_score += score * 0.3  # Weight optional data less
            
            # Apply model-specific constraints
            if is_compatible:
                is_compatible = self._check_model_constraints(discovered_type, requirements)
                if is_compatible:
                    best_score *= self._calculate_quality_multiplier(discovered_type, requirements)
            
            # Cache result
            self._compatibility_cache[cache_key] = (is_compatible, best_score)
            
            if is_compatible:
                matches.append((type_id, best_score))
        
        # Sort by compatibility score (descending)
        matches.sort(key=lambda x: x[1], reverse=True)
        
        # Update statistics
        if matches:
            self.discovery_stats['successful_matches'] += 1
        else:
            self.discovery_stats['failed_matches'] += 1
        
        # Clean cache periodically
        if time.time() - self._last_cache_cleanup > 3600:  # Every hour
            await self._cleanup_cache()
        
        return matches
    
    def register_model_requirements(self, model_id: str, requirements: ModelDataRequirement):
        """Register data requirements for a model"""
        self.model_requirements[model_id] = requirements
        self.logger.info(f"Registered requirements for model {model_id}")
    
    def get_available_types(self) -> Dict[str, DiscoveredDataType]:
        """Get all discovered data types"""
        return self.discovered_types.copy()
    
    def get_type_statistics(self) -> Dict[str, Any]:
        """Get registry statistics"""
        stats = self.discovery_stats.copy()
        stats.update({
            'total_types': len(self.discovered_types),
            'monitored_channels': len(self.monitored_channels),
            'cache_size': len(self._compatibility_cache),
            'model_count': len(self.model_requirements)
        })
        
        # Add per-nature statistics
        nature_counts = defaultdict(int)
        for discovered_type in self.discovered_types.values():
            nature_counts[discovered_type.characteristics.nature.value] += 1
        stats['types_by_nature'] = dict(nature_counts)
        
        return stats
    
    async def optimize_registry(self) -> Dict[str, Any]:
        """
        Analyze and optimize the registry for better performance.
        
        Returns:
            Optimization recommendations
        """
        recommendations = {
            'actions_taken': [],
            'suggestions': [],
            'performance_improvements': {}
        }
        
        # Remove stale types
        cutoff_time = datetime.utcnow() - timedelta(days=7)
        stale_types = [
            type_id for type_id, discovered_type in self.discovered_types.items()
            if discovered_type.last_seen < cutoff_time and discovered_type.seen_count < 5
        ]
        
        for type_id in stale_types:
            del self.discovered_types[type_id]
            recommendations['actions_taken'].append(f"Removed stale type {type_id}")
        
        # Identify redundant types
        redundant_pairs = self._find_redundant_types()
        for type1, type2 in redundant_pairs:
            recommendations['suggestions'].append(
                f"Consider merging similar types {type1} and {type2}"
            )
        
        # Cache optimization
        await self._cleanup_cache()
        recommendations['actions_taken'].append("Cleaned compatibility cache")
        
        return recommendations
    
    # Private helper methods
    
    def _generate_type_id(self, characteristics: DataCharacteristics, channel: str) -> str:
        """Generate unique type ID based on characteristics"""
        components = [
            characteristics.nature.value,
            characteristics.scope.value,
            characteristics.frequency.value,
            characteristics.quality.value
        ]
        base_id = "_".join(components)
        
        # Add channel hash for uniqueness
        channel_hash = hash(channel) % 10000
        return f"{base_id}_{channel_hash}"
    
    def _infer_schema(self, data: Dict[str, Any]) -> Dict[str, str]:
        """Infer data schema from sample"""
        schema = {}
        for key, value in data.items():
            if isinstance(value, bool):
                schema[key] = 'boolean'
            elif isinstance(value, int):
                schema[key] = 'integer'
            elif isinstance(value, float):
                schema[key] = 'float'
            elif isinstance(value, str):
                schema[key] = 'string'
            elif isinstance(value, list):
                schema[key] = 'array'
            elif isinstance(value, dict):
                schema[key] = 'object'
            else:
                schema[key] = 'unknown'
        return schema
    
    def _identify_required_fields(self, data: Dict[str, Any]) -> Set[str]:
        """Identify fields that appear to be required"""
        required = set()
        for key, value in data.items():
            if value is not None and key.lower() in {
                'timestamp', 'symbol', 'price', 'close', 'volume'
            }:
                required.add(key)
        return required
    
    def _identify_optional_fields(self, data: Dict[str, Any]) -> Set[str]:
        """Identify optional fields"""
        required = self._identify_required_fields(data)
        return set(data.keys()) - required
    
    def _update_schema(self, discovered_type: DiscoveredDataType, new_data: Dict[str, Any]):
        """Update schema with new data sample"""
        new_schema = self._infer_schema(new_data)
        
        # Merge schemas
        for field, field_type in new_schema.items():
            if field not in discovered_type.schema:
                discovered_type.schema[field] = field_type
                discovered_type.optional_fields.add(field)
            elif discovered_type.schema[field] != field_type:
                # Type conflict - mark as string (most flexible)
                discovered_type.schema[field] = 'string'
    
    def _check_model_constraints(self, discovered_type: DiscoveredDataType, 
                               requirements: ModelDataRequirement) -> bool:
        """Check if discovered type meets model constraints"""
        chars = discovered_type.characteristics
        
        # Check feature count
        if chars.feature_count and chars.feature_count < requirements.min_feature_count:
            return False
        
        # Check latency
        if chars.latency_ms and chars.latency_ms > requirements.max_latency_ms:
            return False
        
        # Check reliability
        if chars.reliability_score < requirements.min_reliability:
            return False
        
        # Check coverage
        if chars.coverage_ratio < requirements.min_coverage:
            return False
        
        return True
    
    def _calculate_quality_multiplier(self, discovered_type: DiscoveredDataType,
                                    requirements: ModelDataRequirement) -> float:
        """Calculate quality multiplier based on how well the type exceeds requirements"""
        multiplier = 1.0
        chars = discovered_type.characteristics
        
        # Reliability bonus
        if chars.reliability_score > requirements.min_reliability:
            multiplier += (chars.reliability_score - requirements.min_reliability) * 0.5
        
        # Coverage bonus
        if chars.coverage_ratio > requirements.min_coverage:
            multiplier += (chars.coverage_ratio - requirements.min_coverage) * 0.3
        
        # Freshness bonus (based on seen count and recency)
        freshness_score = min(discovered_type.seen_count / 100, 1.0)
        age_hours = (datetime.utcnow() - discovered_type.last_seen).total_seconds() / 3600
        if age_hours < 1:
            freshness_score += 0.2
        
        multiplier += freshness_score * 0.2
        
        return min(multiplier, 2.0)  # Cap at 2x multiplier
    
    async def _check_model_activations(self, new_type_id: str):
        """Check if any models can be activated with the new data type"""
        for model_id in self.model_requirements:
            matches = await self.match_available(model_id)
            if matches and any(type_id == new_type_id for type_id, _ in matches):
                self.logger.info(f"Model {model_id} can now be activated with data type {new_type_id}")
                # Here you would trigger actual model activation
                # await self.model_activator.activate_model(model_id, matches)
    
    def _find_redundant_types(self) -> List[Tuple[str, str]]:
        """Find pairs of types that might be redundant"""
        redundant_pairs = []
        type_list = list(self.discovered_types.items())
        
        for i, (type1_id, type1) in enumerate(type_list):
            for type2_id, type2 in type_list[i+1:]:
                if self._types_similar(type1.characteristics, type2.characteristics):
                    redundant_pairs.append((type1_id, type2_id))
        
        return redundant_pairs
    
    def _types_similar(self, chars1: DataCharacteristics, chars2: DataCharacteristics) -> bool:
        """Check if two data types are very similar"""
        return (chars1.nature == chars2.nature and
                chars1.scope == chars2.scope and
                abs(chars1.frequency.seconds - chars2.frequency.seconds) <= 300 and  # 5 min tolerance
                chars1.quality == chars2.quality)
    
    async def _cleanup_cache(self):
        """Clean up compatibility cache"""
        # Remove entries for deleted types
        valid_keys = []
        for model_id, type_id in self._compatibility_cache.keys():
            if (model_id in self.model_requirements and 
                type_id in self.discovered_types):
                valid_keys.append((model_id, type_id))
        
        # Rebuild cache with only valid entries
        new_cache = {k: self._compatibility_cache[k] for k in valid_keys}
        self._compatibility_cache = new_cache
        self._last_cache_cleanup = time.time()


# =============================================================================
# Integration Helper Classes
# =============================================================================

class ModelDataMatcher:
    """Helper class for matching models with available data types"""
    
    def __init__(self, registry: DynamicDataTypeRegistry):
        self.registry = registry
        self.logger = logging.getLogger(__name__)
    
    async def find_optimal_configuration(self, model_id: str) -> Optional[Dict[str, Any]]:
        """
        Find the optimal data configuration for a model.
        
        Returns:
            Configuration dict with matched data types and their roles
        """
        matches = await self.registry.match_available(model_id)
        if not matches:
            return None
        
        # Group matches by data nature
        by_nature = defaultdict(list)
        for type_id, score in matches:
            discovered_type = self.registry.discovered_types[type_id]
            by_nature[discovered_type.characteristics.nature].append((type_id, score))
        
        # Select best type for each nature
        configuration = {
            'model_id': model_id,
            'primary_data': [],
            'secondary_data': [],
            'total_score': 0.0,
            'completeness': 0.0
        }
        
        requirements = self.registry.model_requirements.get(model_id)
        if not requirements:
            return None
        
        # Match required data
        required_natures = {req.nature for req in requirements.required_data}
        for nature in required_natures:
            if nature in by_nature:
                best_type, score = max(by_nature[nature], key=lambda x: x[1])
                configuration['primary_data'].append({
                    'type_id': best_type,
                    'nature': nature.value,
                    'score': score,
                    'role': 'required'
                })
                configuration['total_score'] += score
        
        # Match optional data
        optional_natures = {req.nature for req in requirements.optional_data}
        for nature in optional_natures:
            if nature in by_nature:
                best_type, score = max(by_nature[nature], key=lambda x: x[1])
                configuration['secondary_data'].append({
                    'type_id': best_type,
                    'nature': nature.value,
                    'score': score,
                    'role': 'optional'
                })
                configuration['total_score'] += score * 0.5  # Weight optional data less
        
        # Calculate completeness
        total_possible = len(required_natures) + len(optional_natures)
        total_matched = len(configuration['primary_data']) + len(configuration['secondary_data'])
        configuration['completeness'] = total_matched / total_possible if total_possible > 0 else 0.0
        
        # Return None if no data at all, otherwise return configuration
        if total_matched == 0 and total_possible > 0:
            return None
        
        return configuration


class DataIngestionAdapter:
    """Channel-agnostic data consumption from Redis"""
    
    def __init__(self, redis_client: redis.Redis, registry: DynamicDataTypeRegistry):
        self.redis = redis_client
        self.registry = registry
        self.logger = logging.getLogger(__name__)
        
        # Channel monitoring
        self.active_subscriptions: Set[str] = set()
        self.channel_patterns: List[str] = []
        
    async def start_monitoring(self, channel_patterns: List[str]):
        """Start monitoring Redis channels for new data types"""
        self.channel_patterns = channel_patterns
        
        pubsub = self.redis.pubsub()
        
        # Subscribe to patterns
        for pattern in channel_patterns:
            await pubsub.psubscribe(pattern)
            self.logger.info(f"Subscribed to channel pattern: {pattern}")
        
        # Process messages
        async for message in pubsub.listen():
            if message['type'] == 'pmessage':
                await self._process_message(message)
    
    async def _process_message(self, message):
        """Process incoming Redis message"""
        try:
            channel = message['channel'].decode('utf-8')
            data = json.loads(message['data'].decode('utf-8'))
            
            # Register/update data type
            type_id = await self.registry.register_type(
                data=data,
                channel=channel,
                metadata={'source': 'redis', 'timestamp': datetime.utcnow().isoformat()}
            )
            
            if type_id:
                self.logger.debug(f"Processed data from {channel} as type {type_id}")
            
        except Exception as e:
            self.logger.error(f"Error processing message from {message.get('channel', 'unknown')}: {e}")


# =============================================================================
# Memory Storage for Claude Flow Integration
# =============================================================================

async def store_implementation_in_memory(registry: DynamicDataTypeRegistry):
    """Store the implementation in Claude Flow memory for phase 3"""
    try:
        # This would use the mcp__claude-flow__memory_usage tool
        implementation_data = {
            'component': 'DynamicDataTypeRegistry',
            'version': '1.0.0',
            'timestamp': datetime.utcnow().isoformat(),
            'features': {
                'runtime_discovery': True,
                'characteristic_matching': True,
                'model_compatibility': True,
                'channel_agnostic_ingestion': True,
                'multi_scope_routing': True
            },
            'statistics': registry.get_type_statistics(),
            'discovered_types_count': len(registry.discovered_types),
            'model_requirements_count': len(registry.model_requirements)
        }
        
        # Store in memory namespace
        # await mcp__claude_flow__memory_usage(
        #     action="store",
        #     key="phase3/data_pipeline/type_discovery",
        #     value=json.dumps(implementation_data),
        #     namespace="neural_trader_phase3"
        # )
        
        return implementation_data
        
    except Exception as e:
        logging.error(f"Failed to store implementation in memory: {e}")
        return None


# =============================================================================
# Example Usage and Testing
# =============================================================================

async def example_usage():
    """Example usage of the dynamic data type discovery system"""
    
    # Initialize registry
    registry = DynamicDataTypeRegistry()
    
    # Example: Register model requirements
    model_requirements = ModelDataRequirement(
        model_id="lstm_sector_tech",
        required_data=[
            DataCharacteristics(
                frequency=DataFrequency.MINUTE,
                scope=DataScope.SYMBOL,
                nature=DataNature.PRICE,
                quality=DataQuality.REQUIRED,
                feature_count=5
            )
        ],
        optional_data=[
            DataCharacteristics(
                frequency=DataFrequency.HOUR,
                scope=DataScope.SECTOR,
                nature=DataNature.SENTIMENT,
                quality=DataQuality.OPTIONAL,
                feature_count=3
            )
        ]
    )
    
    registry.register_model_requirements("lstm_sector_tech", model_requirements)
    
    # Example: Register some data types
    sample_price_data = {
        "timestamp": "2025-08-02T13:00:00Z",
        "symbol": "AAPL",
        "open": 150.0,
        "high": 152.0,
        "low": 149.5,
        "close": 151.5,
        "volume": 1000000
    }
    
    type_id = await registry.register_type(
        data=sample_price_data,
        channel="market_data:symbols:1min",
        metadata={"source": "alpaca", "quality": "premium"}
    )
    
    print(f"Registered type: {type_id}")
    
    # Example: Find matches for model
    matches = await registry.match_available("lstm_sector_tech")
    print(f"Model matches: {matches}")
    
    # Example: Get registry statistics
    stats = registry.get_type_statistics()
    print(f"Registry stats: {stats}")
    
    # Store in memory for phase 3
    await store_implementation_in_memory(registry)


if __name__ == "__main__":
    # Configure logging
    logging.basicConfig(level=logging.INFO)
    
    # Run example
    asyncio.run(example_usage())