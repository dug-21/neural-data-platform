"""Secure settings implementation that prevents secrets from being loaded from files"""
import os
import json
from typing import Optional, Dict, Any, Tuple, Type
from functools import lru_cache
from pydantic import BaseModel, Field, field_validator
from pydantic_settings import BaseSettings, SettingsConfigDict


class RateLimitConfig(BaseModel):
    """Rate limit configuration for an API"""
    calls_per_minute: Optional[int] = Field(None, ge=1)
    calls_per_day: Optional[int] = Field(None, ge=1)
    burst_size: Optional[int] = Field(None, ge=1)
    
    @field_validator('calls_per_minute', 'calls_per_day', 'burst_size')
    def validate_positive(cls, v):
        if v is not None and v <= 0:
            raise ValueError("Rate limit values must be positive")
        return v


class SecureSettings(BaseSettings):
    """Application settings with secure handling of secrets"""
    
    model_config = SettingsConfigDict(
        env_file='.env',
        env_file_encoding='utf-8',
        case_sensitive=False,
        env_nested_delimiter='__',
        extra='allow',  # Allow extra environment variables
    )
    
    # API Keys (SECRETS - will not load from .env)
    iex_cloud_api_key: Optional[str] = Field(None, alias="IEX_CLOUD_API_KEY")
    alpha_vantage_api_key: Optional[str] = Field(None, alias="ALPHA_VANTAGE_API_KEY")
    polygon_api_key: Optional[str] = Field(None, alias="POLYGON_API_KEY")
    finnhub_api_key: Optional[str] = Field(None, alias="FINNHUB_API_KEY")
    fred_api_key: Optional[str] = Field(None, alias="FRED_API_KEY")
    reddit_client_id: Optional[str] = Field(None, alias="REDDIT_CLIENT_ID")
    reddit_client_secret: Optional[str] = Field(None, alias="REDDIT_CLIENT_SECRET")
    reddit_user_agent: Optional[str] = Field("neural-trader/1.0", alias="REDDIT_USER_AGENT")
    quandl_api_key: Optional[str] = Field(None, alias="QUANDL_API_KEY")
    newsapi_key: Optional[str] = Field(None, alias="NEWSAPI_KEY")
    yahoo_api_key: Optional[str] = Field(None, alias="YAHOO_API_KEY")
    alpaca_api_key: Optional[str] = Field(None, alias="ALPACA_API_KEY")
    alpaca_api_secret: Optional[str] = Field(None, alias="ALPACA_API_SECRET")
    alpaca_subscription_level: Optional[str] = Field("basic", alias="ALPACA_SUBSCRIPTION_LEVEL")
    alpaca_ws_enabled: bool = Field(False, alias="ALPACA_WS_ENABLED")
    
    # Database Configuration
    timescale_host: str = Field("localhost", alias="TIMESCALE_HOST")
    timescale_port: int = Field(5432, alias="TIMESCALE_PORT")
    timescale_database: str = Field("neural_trader", alias="TIMESCALE_DATABASE")
    timescale_user: str = Field("trader", alias="TIMESCALE_USER")
    timescale_password: str = Field("", alias="TIMESCALE_PASSWORD")  # SECRET
    
    # Redis Configuration
    redis_host: str = Field("localhost", alias="REDIS_HOST")
    redis_port: int = Field(6379, alias="REDIS_PORT")
    redis_password: Optional[str] = Field(None, alias="REDIS_PASSWORD")  # SECRET
    redis_db: int = Field(0, alias="REDIS_DB")
    
    # Rate Limiting Configuration
    max_requests_per_minute: int = Field(60, alias="MAX_REQUESTS_PER_MINUTE")
    max_concurrent_requests: int = Field(10, alias="MAX_CONCURRENT_REQUESTS")
    
    # Per-API Rate Limits
    rate_limits: Dict[str, RateLimitConfig] = Field(
        default_factory=lambda: {
            "alpha_vantage": RateLimitConfig(calls_per_minute=5, calls_per_day=500),
            "polygon": RateLimitConfig(calls_per_minute=5),
            "finnhub": RateLimitConfig(calls_per_minute=60),
            "newsapi": RateLimitConfig(calls_per_minute=None, calls_per_day=100),
            "fred": RateLimitConfig(calls_per_minute=120),  # 120/min = 2/sec
            "reddit": RateLimitConfig(calls_per_minute=60),
            "nasdaq": RateLimitConfig(calls_per_minute=None, calls_per_day=50000),
            "yahoo_finance": RateLimitConfig(calls_per_minute=None, calls_per_day=200),
            "alpaca": RateLimitConfig(calls_per_minute=200),  # Basic plan default
        }
    )
    
    # Logging
    log_level: str = Field("INFO", alias="LOG_LEVEL")
    log_format: str = Field("json", alias="LOG_FORMAT")
    
    # Monitoring
    prometheus_enabled: bool = Field(True, alias="PROMETHEUS_ENABLED")
    prometheus_port: int = Field(9090, alias="PROMETHEUS_PORT")
    
    # Data Processing
    batch_size: int = Field(1000, alias="BATCH_SIZE")
    processing_interval_seconds: int = Field(60, alias="PROCESSING_INTERVAL_SECONDS")
    
    # Phase 2: Channel migration settings (INTERFACE_CONTRACT compliance)
    enable_legacy_channel: bool = Field(True, alias="ENABLE_LEGACY_CHANNEL")
    redis_channel_prefix: str = Field("market", alias="REDIS_CHANNEL_PREFIX")
    redis_dual_publish: bool = Field(True, alias="REDIS_DUAL_PUBLISH")
    
    # Redis connection and performance settings
    redis_max_connections: int = Field(50, alias="REDIS_MAX_CONNECTIONS")
    redis_publish_timeout: int = Field(5, alias="REDIS_PUBLISH_TIMEOUT")
    redis_decode_responses: bool = Field(True, alias="REDIS_DECODE_RESPONSES")
    
    # Define which fields are secrets
    _secret_fields = {
        'iex_cloud_api_key', 'alpha_vantage_api_key', 'polygon_api_key',
        'finnhub_api_key', 'fred_api_key', 'reddit_client_id',
        'reddit_client_secret', 'quandl_api_key', 'newsapi_key',
        'yahoo_api_key', 'alpaca_api_key', 'alpaca_api_secret',
        'timescale_password', 'redis_password'
    }
    
    @classmethod
    def settings_customise_sources(
        cls,
        settings_cls: Type[BaseSettings],
        init_settings,
        env_settings,
        dotenv_settings,
        file_secret_settings,
    ) -> Tuple[Any, ...]:
        """Customize settings sources to filter secrets from dotenv"""
        
        # Create a wrapper for env settings that handles rate limits
        class EnhancedEnvSettings:
            def __init__(self, original_env):
                self.original_env = original_env
                
            def __call__(self) -> Dict[str, Any]:
                # Get all values from environment
                env_values = self.original_env() if self.original_env else {}
                
                # Handle rate limits from environment
                rate_limit_updates = {}
                for key, value in os.environ.items():
                    if key.startswith('RATE_LIMIT_') and key.endswith(('_CALLS_PER_MINUTE', '_CALLS_PER_DAY', '_BURST_SIZE')):
                        # Parse rate limit env vars
                        parts = key.split('_')
                        if len(parts) >= 4:
                            api_name = '_'.join(parts[2:-3]).lower()
                            metric = '_'.join(parts[-3:]).lower()
                            
                            if api_name not in rate_limit_updates:
                                rate_limit_updates[api_name] = {}
                            rate_limit_updates[api_name][metric] = int(value)
                
                # Update rate_limits if we have updates
                if rate_limit_updates:
                    # Get default rate limits
                    default_factory = cls.model_fields['rate_limits'].default_factory
                    current_rate_limits = default_factory() if default_factory else {}
                    
                    # Apply updates
                    for api_name, config in rate_limit_updates.items():
                        if api_name not in current_rate_limits:
                            current_rate_limits[api_name] = RateLimitConfig()
                        
                        # Update the config
                        for metric, value in config.items():
                            setattr(current_rate_limits[api_name], metric, value)
                    
                    env_values['rate_limits'] = current_rate_limits
                
                # Handle RATE_LIMITS_JSON
                if 'RATE_LIMITS_JSON' in os.environ:
                    try:
                        rate_limits_from_json = json.loads(os.environ['RATE_LIMITS_JSON'])
                        current = env_values.get('rate_limits', {})
                        
                        for api_name, config_dict in rate_limits_from_json.items():
                            if api_name not in current:
                                current[api_name] = RateLimitConfig(**config_dict)
                            else:
                                # Update existing
                                for k, v in config_dict.items():
                                    setattr(current[api_name], k, v)
                        
                        env_values['rate_limits'] = current
                    except Exception as e:
                        print(f"Error parsing RATE_LIMITS_JSON: {e}")
                
                return env_values
        
        # Create a wrapper for dotenv settings that filters secrets
        class FilteredDotEnvSettings:
            def __init__(self, original_dotenv):
                self.original_dotenv = original_dotenv
                
            def __call__(self) -> Dict[str, Any]:
                # Get all values from dotenv
                dotenv_values = {}
                if self.original_dotenv:
                    try:
                        dotenv_values = self.original_dotenv()
                    except:
                        # If dotenv fails, return empty dict
                        return {}
                
                # Filter out secrets
                filtered = {}
                for key, value in dotenv_values.items():
                    # Check if this is a secret field
                    if key in [
                        'IEX_CLOUD_API_KEY', 'ALPHA_VANTAGE_API_KEY', 'POLYGON_API_KEY',
                        'FINNHUB_API_KEY', 'FRED_API_KEY', 'REDDIT_CLIENT_ID',
                        'REDDIT_CLIENT_SECRET', 'QUANDL_API_KEY', 'NEWSAPI_KEY',
                        'YAHOO_API_KEY', 'ALPACA_API_KEY', 'ALPACA_API_SECRET',
                        'TIMESCALE_PASSWORD', 'REDIS_PASSWORD'
                    ]:
                        print(f"WARNING: Secret '{key}' found in .env file - ignoring for security")
                    else:
                        filtered[key] = value
                
                return filtered
        
        # Return customized sources
        wrapped_env = EnhancedEnvSettings(env_settings)
        wrapped_dotenv = FilteredDotEnvSettings(dotenv_settings)
        
        return (
            init_settings,
            wrapped_env,
            wrapped_dotenv,
            file_secret_settings,
        )
    
    @property
    def timescale_url(self) -> str:
        """Get TimescaleDB connection URL"""
        return (
            f"postgresql://{self.timescale_user}:{self.timescale_password}"
            f"@{self.timescale_host}:{self.timescale_port}/{self.timescale_database}"
        )
    
    @property
    def redis_url(self) -> str:
        """Get Redis connection URL"""
        if self.redis_password:
            return f"redis://:{self.redis_password}@{self.redis_host}:{self.redis_port}/{self.redis_db}"
        return f"redis://{self.redis_host}:{self.redis_port}/{self.redis_db}"


# Maintain backward compatibility
Settings = SecureSettings


@lru_cache()
def get_settings() -> Settings:
    """Get cached settings instance"""
    return Settings()