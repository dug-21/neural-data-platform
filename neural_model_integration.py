"""
Neural Model Integration for Dynamic Data Type Discovery

This module provides integration between the dynamic data type discovery system
and existing neural model configurations, enabling automatic model activation
based on available data characteristics.

Author: Data-Pipeline-Dev2
Date: 2025-08-02
"""

import json
import logging
from dataclasses import dataclass, field
from typing import Dict, List, Optional, Set, Any, Tuple
from datetime import datetime
import asyncio
from pathlib import Path

from phase3_data_pipeline_type_discovery import (
    DataFrequency, DataScope, DataNature, DataQuality,
    DataCharacteristics, ModelDataRequirement,
    DynamicDataTypeRegistry, ModelDataMatcher
)


# =============================================================================
# Neural Model Configuration Integration
# =============================================================================

@dataclass
class NeuralModelConfig:
    """Configuration for a neural model with dynamic data requirements"""
    model_id: str
    model_type: str  # e.g., "LSTM", "TCN", "DeepAR", "NBEATS"
    architecture_params: Dict[str, Any]
    
    # Dynamic data requirements
    data_requirements: ModelDataRequirement
    
    # Model metadata
    sector: Optional[str] = None
    symbols: List[str] = field(default_factory=list)
    
    # Performance requirements
    max_inference_latency_ms: int = 100
    min_accuracy_threshold: float = 0.7
    memory_limit_mb: int = 512
    
    # Training configuration
    training_enabled: bool = True
    retraining_frequency_hours: int = 24
    adaptation_enabled: bool = True
    
    # Status tracking
    is_active: bool = False
    activation_timestamp: Optional[datetime] = None
    last_training: Optional[datetime] = None
    performance_metrics: Dict[str, float] = field(default_factory=dict)


class NeuralModelManager:
    """
    Manages neural models and their dynamic activation based on data availability.
    
    Integrates with the DynamicDataTypeRegistry to automatically activate models
    when their data requirements are satisfied.
    """
    
    def __init__(self, registry: DynamicDataTypeRegistry, config_path: Optional[str] = None):
        self.registry = registry
        self.matcher = ModelDataMatcher(registry)
        self.logger = logging.getLogger(__name__)
        
        # Model storage
        self.models: Dict[str, NeuralModelConfig] = {}
        self.active_models: Set[str] = set()
        
        # Configuration
        self.config_path = config_path or "models/configurations/"
        self.auto_activation = True
        self.activation_threshold = 0.7  # Minimum compatibility score for activation
        
        # Performance tracking
        self.activation_history: List[Dict[str, Any]] = []
        self.performance_history: Dict[str, List[Dict[str, Any]]] = {}
    
    def register_model(self, config: NeuralModelConfig) -> bool:
        """
        Register a neural model configuration.
        
        Args:
            config: Neural model configuration
            
        Returns:
            True if successfully registered
        """
        try:
            # Validate configuration
            if not self._validate_config(config):
                self.logger.error(f"Invalid configuration for model {config.model_id}")
                return False
            
            # Store model configuration
            self.models[config.model_id] = config
            
            # Register data requirements with the registry
            self.registry.register_model_requirements(config.model_id, config.data_requirements)
            
            self.logger.info(f"Registered neural model {config.model_id}")
            
            # Check if model can be activated immediately
            if self.auto_activation:
                asyncio.create_task(self._check_activation(config.model_id))
            
            return True
            
        except Exception as e:
            self.logger.error(f"Error registering model {config.model_id}: {e}")
            return False
    
    async def check_all_activations(self) -> Dict[str, bool]:
        """
        Check activation status for all registered models.
        
        Returns:
            Dict mapping model_id to activation success
        """
        results = {}
        
        for model_id in self.models:
            if model_id not in self.active_models:
                results[model_id] = await self._check_activation(model_id)
            else:
                results[model_id] = True  # Already active
        
        return results
    
    async def get_activation_recommendations(self) -> List[Dict[str, Any]]:
        """
        Get recommendations for model activations based on available data.
        
        Returns:
            List of activation recommendations
        """
        recommendations = []
        
        for model_id, config in self.models.items():
            if model_id in self.active_models:
                continue
            
            # Get optimal data configuration
            data_config = await self.matcher.find_optimal_configuration(model_id)
            
            if data_config and data_config['completeness'] >= 0.5:
                recommendation = {
                    'model_id': model_id,
                    'model_type': config.model_type,
                    'sector': config.sector,
                    'data_completeness': data_config['completeness'],
                    'compatibility_score': data_config['total_score'],
                    'available_data_types': len(data_config['primary_data'] + data_config['secondary_data']),
                    'can_activate': data_config['total_score'] >= self.activation_threshold,
                    'missing_requirements': self._identify_missing_requirements(config, data_config)
                }
                recommendations.append(recommendation)
        
        # Sort by compatibility score
        recommendations.sort(key=lambda x: x['compatibility_score'], reverse=True)
        
        return recommendations
    
    async def activate_model(self, model_id: str, force: bool = False) -> bool:
        """
        Manually activate a specific model.
        
        Args:
            model_id: Model to activate
            force: Force activation even if data requirements aren't fully met
            
        Returns:
            True if activation successful
        """
        if model_id not in self.models:
            self.logger.error(f"Model {model_id} not found")
            return False
        
        if model_id in self.active_models:
            self.logger.info(f"Model {model_id} already active")
            return True
        
        config = self.models[model_id]
        
        # Check data availability
        data_config = await self.matcher.find_optimal_configuration(model_id)
        
        if not force and (not data_config or data_config['total_score'] < self.activation_threshold):
            self.logger.warning(f"Insufficient data for model {model_id} activation")
            return False
        
        # Perform activation
        success = await self._perform_activation(config, data_config)
        
        if success:
            self.active_models.add(model_id)
            config.is_active = True
            config.activation_timestamp = datetime.utcnow()
            
            # Record activation
            self.activation_history.append({
                'model_id': model_id,
                'timestamp': datetime.utcnow(),
                'data_config': data_config,
                'forced': force
            })
            
            self.logger.info(f"Successfully activated model {model_id}")
        
        return success
    
    async def deactivate_model(self, model_id: str, reason: str = "manual") -> bool:
        """
        Deactivate a model.
        
        Args:
            model_id: Model to deactivate
            reason: Reason for deactivation
            
        Returns:
            True if deactivation successful
        """
        if model_id not in self.active_models:
            self.logger.warning(f"Model {model_id} is not active")
            return False
        
        config = self.models[model_id]
        
        # Perform deactivation
        success = await self._perform_deactivation(config, reason)
        
        if success:
            self.active_models.remove(model_id)
            config.is_active = False
            
            self.logger.info(f"Deactivated model {model_id}: {reason}")
        
        return success
    
    def get_model_status(self, model_id: str) -> Optional[Dict[str, Any]]:
        """Get detailed status for a specific model"""
        if model_id not in self.models:
            return None
        
        config = self.models[model_id]
        
        status = {
            'model_id': model_id,
            'model_type': config.model_type,
            'is_active': config.is_active,
            'sector': config.sector,
            'symbols': config.symbols,
            'activation_timestamp': config.activation_timestamp,
            'last_training': config.last_training,
            'performance_metrics': config.performance_metrics.copy()
        }
        
        # Add data availability information
        if model_id in self.active_models:
            # Get current data configuration
            data_config = asyncio.run(self.matcher.find_optimal_configuration(model_id))
            if data_config:
                status['data_completeness'] = data_config['completeness']
                status['data_types_count'] = len(data_config['primary_data'] + data_config['secondary_data'])
        
        return status
    
    def get_all_models_status(self) -> List[Dict[str, Any]]:
        """Get status for all registered models"""
        return [self.get_model_status(model_id) for model_id in self.models]
    
    async def update_performance_metrics(self, model_id: str, metrics: Dict[str, float]):
        """Update performance metrics for a model"""
        if model_id not in self.models:
            self.logger.error(f"Model {model_id} not found")
            return
        
        config = self.models[model_id]
        config.performance_metrics.update(metrics)
        
        # Store in history
        if model_id not in self.performance_history:
            self.performance_history[model_id] = []
        
        self.performance_history[model_id].append({
            'timestamp': datetime.utcnow(),
            'metrics': metrics.copy()
        })
        
        # Check if model should be deactivated due to poor performance
        if config.is_active and 'accuracy' in metrics:
            if metrics['accuracy'] < config.min_accuracy_threshold:
                await self.deactivate_model(model_id, "poor_performance")
    
    async def optimize_model_portfolio(self) -> Dict[str, Any]:
        """
        Optimize the portfolio of active models based on performance and resource usage.
        
        Returns:
            Optimization recommendations
        """
        recommendations = {
            'activate': [],
            'deactivate': [],
            'retrain': [],
            'resource_usage': {},
            'performance_summary': {}
        }
        
        # Calculate resource usage
        total_memory_mb = sum(
            self.models[model_id].memory_limit_mb 
            for model_id in self.active_models
        )
        recommendations['resource_usage']['total_memory_mb'] = total_memory_mb
        
        # Identify underperforming models
        for model_id in self.active_models:
            config = self.models[model_id]
            if 'accuracy' in config.performance_metrics:
                accuracy = config.performance_metrics['accuracy']
                if accuracy < config.min_accuracy_threshold:
                    recommendations['deactivate'].append({
                        'model_id': model_id,
                        'reason': 'poor_performance',
                        'accuracy': accuracy,
                        'threshold': config.min_accuracy_threshold
                    })
        
        # Identify models that could be activated
        activation_recs = await self.get_activation_recommendations()
        for rec in activation_recs[:5]:  # Top 5 candidates
            if rec['can_activate']:
                recommendations['activate'].append(rec)
        
        # Identify models needing retraining
        now = datetime.utcnow()
        for model_id in self.active_models:
            config = self.models[model_id]
            if config.training_enabled and config.last_training:
                hours_since_training = (now - config.last_training).total_seconds() / 3600
                if hours_since_training >= config.retraining_frequency_hours:
                    recommendations['retrain'].append({
                        'model_id': model_id,
                        'hours_since_training': hours_since_training,
                        'scheduled_frequency': config.retraining_frequency_hours
                    })
        
        # Performance summary
        active_performances = []
        for model_id in self.active_models:
            metrics = self.models[model_id].performance_metrics
            if 'accuracy' in metrics:
                active_performances.append(metrics['accuracy'])
        
        if active_performances:
            recommendations['performance_summary'] = {
                'average_accuracy': sum(active_performances) / len(active_performances),
                'min_accuracy': min(active_performances),
                'max_accuracy': max(active_performances),
                'active_models_count': len(self.active_models)
            }
        
        return recommendations
    
    # Private helper methods
    
    def _validate_config(self, config: NeuralModelConfig) -> bool:
        """Validate model configuration"""
        if not config.model_id or not config.model_type:
            return False
        
        if not config.data_requirements:
            return False
        
        if config.max_inference_latency_ms <= 0:
            return False
        
        if not 0.0 <= config.min_accuracy_threshold <= 1.0:
            return False
        
        return True
    
    async def _check_activation(self, model_id: str) -> bool:
        """Check if a model can be activated"""
        data_config = await self.matcher.find_optimal_configuration(model_id)
        
        if data_config and data_config['total_score'] >= self.activation_threshold:
            return await self.activate_model(model_id)
        
        return False
    
    def _identify_missing_requirements(self, config: NeuralModelConfig, 
                                     data_config: Optional[Dict[str, Any]]) -> List[str]:
        """Identify missing data requirements"""
        if not data_config:
            return ["No compatible data types available"]
        
        missing = []
        
        # Check required data types
        required_natures = {req.nature.value for req in config.data_requirements.required_data}
        available_natures = {item['nature'] for item in data_config['primary_data']}
        
        missing_natures = required_natures - available_natures
        for nature in missing_natures:
            missing.append(f"Missing required data type: {nature}")
        
        # Check data completeness
        if data_config['completeness'] < 0.8:
            missing.append(f"Low data completeness: {data_config['completeness']:.2f}")
        
        # Check feature count
        total_features = sum(
            self.registry.discovered_types[item['type_id']].characteristics.feature_count or 0
            for item in data_config['primary_data'] + data_config['secondary_data']
        )
        
        if total_features < config.data_requirements.min_feature_count:
            missing.append(f"Insufficient features: {total_features} < {config.data_requirements.min_feature_count}")
        
        return missing
    
    async def _perform_activation(self, config: NeuralModelConfig, 
                                data_config: Optional[Dict[str, Any]]) -> bool:
        """Perform the actual model activation"""
        try:
            # Here you would integrate with the actual neural model system
            # For now, we'll simulate the activation
            
            self.logger.info(f"Activating model {config.model_id} with data config: {data_config}")
            
            # Simulate model loading and initialization
            await asyncio.sleep(0.1)  # Simulate loading time
            
            # Validate that all required data types are available
            if data_config:
                for data_item in data_config['primary_data']:
                    type_id = data_item['type_id']
                    if type_id not in self.registry.discovered_types:
                        raise ValueError(f"Data type {type_id} not available")
            
            return True
            
        except Exception as e:
            self.logger.error(f"Failed to activate model {config.model_id}: {e}")
            return False
    
    async def _perform_deactivation(self, config: NeuralModelConfig, reason: str) -> bool:
        """Perform the actual model deactivation"""
        try:
            self.logger.info(f"Deactivating model {config.model_id}: {reason}")
            
            # Simulate model cleanup
            await asyncio.sleep(0.05)
            
            return True
            
        except Exception as e:
            self.logger.error(f"Failed to deactivate model {config.model_id}: {e}")
            return False


# =============================================================================
# Configuration Loader
# =============================================================================

class ModelConfigLoader:
    """Loads neural model configurations from various sources"""
    
    def __init__(self, manager: NeuralModelManager):
        self.manager = manager
        self.logger = logging.getLogger(__name__)
    
    def load_from_toml(self, file_path: str) -> bool:
        """Load model configurations from TOML file"""
        try:
            import toml
            
            with open(file_path, 'r') as f:
                config_data = toml.load(f)
            
            for model_id, model_config in config_data.get('models', {}).items():
                neural_config = self._parse_toml_config(model_id, model_config)
                if neural_config:
                    self.manager.register_model(neural_config)
            
            self.logger.info(f"Loaded configurations from {file_path}")
            return True
            
        except Exception as e:
            self.logger.error(f"Failed to load TOML config from {file_path}: {e}")
            return False
    
    def load_from_json(self, file_path: str) -> bool:
        """Load model configurations from JSON file"""
        try:
            with open(file_path, 'r') as f:
                config_data = json.load(f)
            
            for model_config in config_data.get('models', []):
                neural_config = self._parse_json_config(model_config)
                if neural_config:
                    self.manager.register_model(neural_config)
            
            self.logger.info(f"Loaded configurations from {file_path}")
            return True
            
        except Exception as e:
            self.logger.error(f"Failed to load JSON config from {file_path}: {e}")
            return False
    
    def create_sector_models(self, sectors: List[str]) -> List[NeuralModelConfig]:
        """Create default model configurations for sectors"""
        configs = []
        
        for sector in sectors:
            # LSTM model for each sector
            lstm_config = NeuralModelConfig(
                model_id=f"lstm_{sector.lower()}",
                model_type="LSTM",
                architecture_params={
                    "hidden_size": 128,
                    "num_layers": 2,
                    "dropout": 0.2,
                    "lookback_window": 60
                },
                data_requirements=ModelDataRequirement(
                    model_id=f"lstm_{sector.lower()}",
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
                        ),
                        DataCharacteristics(
                            frequency=DataFrequency.MINUTE,
                            scope=DataScope.SYMBOL,
                            nature=DataNature.VOLUME,
                            quality=DataQuality.PREFERRED,
                            feature_count=2
                        )
                    ]
                ),
                sector=sector,
                max_inference_latency_ms=50,
                min_accuracy_threshold=0.65,
                memory_limit_mb=256
            )
            
            configs.append(lstm_config)
            
            # TCN model for each sector
            tcn_config = NeuralModelConfig(
                model_id=f"tcn_{sector.lower()}",
                model_type="TCN",
                architecture_params={
                    "num_channels": [64, 64, 128],
                    "kernel_size": 3,
                    "dropout": 0.1,
                    "lookback_window": 120
                },
                data_requirements=ModelDataRequirement(
                    model_id=f"tcn_{sector.lower()}",
                    required_data=[
                        DataCharacteristics(
                            frequency=DataFrequency.MINUTE,
                            scope=DataScope.SYMBOL,
                            nature=DataNature.PRICE,
                            quality=DataQuality.REQUIRED,
                            feature_count=8
                        ),
                        DataCharacteristics(
                            frequency=DataFrequency.MINUTE,
                            scope=DataScope.SYMBOL,
                            nature=DataNature.TECHNICAL,
                            quality=DataQuality.REQUIRED,
                            feature_count=10
                        )
                    ],
                    optional_data=[
                        DataCharacteristics(
                            frequency=DataFrequency.FIVE_MINUTE,
                            scope=DataScope.SECTOR,
                            nature=DataNature.SENTIMENT,
                            quality=DataQuality.OPTIONAL,
                            feature_count=5
                        )
                    ]
                ),
                sector=sector,
                max_inference_latency_ms=75,
                min_accuracy_threshold=0.70,
                memory_limit_mb=384
            )
            
            configs.append(tcn_config)
        
        return configs
    
    def _parse_toml_config(self, model_id: str, config: Dict[str, Any]) -> Optional[NeuralModelConfig]:
        """Parse TOML configuration into NeuralModelConfig"""
        try:
            # Parse data requirements
            required_data = []
            for req_config in config.get('required_data', []):
                chars = DataCharacteristics(
                    frequency=DataFrequency(req_config['frequency']),
                    scope=DataScope(req_config['scope']),
                    nature=DataNature(req_config['nature']),
                    quality=DataQuality(req_config['quality']),
                    feature_count=req_config.get('feature_count')
                )
                required_data.append(chars)
            
            optional_data = []
            for opt_config in config.get('optional_data', []):
                chars = DataCharacteristics(
                    frequency=DataFrequency(opt_config['frequency']),
                    scope=DataScope(opt_config['scope']),
                    nature=DataNature(opt_config['nature']),
                    quality=DataQuality(opt_config['quality']),
                    feature_count=opt_config.get('feature_count')
                )
                optional_data.append(chars)
            
            data_requirements = ModelDataRequirement(
                model_id=model_id,
                required_data=required_data,
                optional_data=optional_data,
                min_feature_count=config.get('min_feature_count', 1),
                max_latency_ms=config.get('max_latency_ms', 1000),
                min_reliability=config.get('min_reliability', 0.8)
            )
            
            return NeuralModelConfig(
                model_id=model_id,
                model_type=config['model_type'],
                architecture_params=config.get('architecture_params', {}),
                data_requirements=data_requirements,
                sector=config.get('sector'),
                symbols=config.get('symbols', []),
                max_inference_latency_ms=config.get('max_inference_latency_ms', 100),
                min_accuracy_threshold=config.get('min_accuracy_threshold', 0.7),
                memory_limit_mb=config.get('memory_limit_mb', 512)
            )
            
        except Exception as e:
            self.logger.error(f"Failed to parse TOML config for {model_id}: {e}")
            return None
    
    def _parse_json_config(self, config: Dict[str, Any]) -> Optional[NeuralModelConfig]:
        """Parse JSON configuration into NeuralModelConfig"""
        # Similar to TOML parsing but for JSON format
        # Implementation would be similar to _parse_toml_config
        pass


# =============================================================================
# Example Usage
# =============================================================================

async def example_integration():
    """Example of neural model integration with dynamic data discovery"""
    
    # Initialize systems
    registry = DynamicDataTypeRegistry()
    manager = NeuralModelManager(registry)
    loader = ModelConfigLoader(manager)
    
    # Create sector models
    sectors = ["Technology", "Financial", "Healthcare", "Energy"]
    sector_configs = loader.create_sector_models(sectors)
    
    # Register all models
    for config in sector_configs:
        manager.register_model(config)
    
    print(f"Registered {len(sector_configs)} neural models")
    
    # Simulate data arrival
    sample_data_types = [
        {
            "data": {
                "timestamp": "2025-08-02T13:00:00Z",
                "symbol": "AAPL",
                "open": 150.0, "high": 152.0, "low": 149.5, "close": 151.5,
                "volume": 1000000
            },
            "channel": "market_data:technology:1min"
        },
        {
            "data": {
                "timestamp": "2025-08-02T13:00:00Z",
                "symbol": "AAPL",
                "sentiment": 0.65, "confidence": 0.85, "volume": 1200
            },
            "channel": "sentiment:technology:hourly"
        }
    ]
    
    # Register data types
    for data_info in sample_data_types:
        await registry.register_type(
            data_info["data"],
            data_info["channel"],
            {"source": "simulation"}
        )
    
    # Check for model activations
    activation_results = await manager.check_all_activations()
    print(f"Activation results: {activation_results}")
    
    # Get activation recommendations
    recommendations = await manager.get_activation_recommendations()
    print(f"Activation recommendations: {len(recommendations)} models ready")
    
    # Get model status
    status = manager.get_all_models_status()
    for model_status in status:
        print(f"Model {model_status['model_id']}: Active = {model_status['is_active']}")
    
    # Optimize model portfolio
    optimization = await manager.optimize_model_portfolio()
    print(f"Optimization recommendations: {optimization}")


if __name__ == "__main__":
    # Configure logging
    logging.basicConfig(level=logging.INFO)
    
    # Run example
    asyncio.run(example_integration())