"""Configuration management for Neural Trader."""
import os
from typing import Optional, List
from pydantic import Field
from pydantic_settings import BaseSettings


class Settings(BaseSettings):
    """Application settings with environment variable support."""
    
    # Application settings
    app_name: str = Field(default="neural-trader", env="APP_NAME")
    environment: str = Field(default="development", env="ENVIRONMENT")
    log_level: str = Field(default="INFO", env="LOG_LEVEL")
    
    # Data provider settings
    # PRIMARY_PROVIDER takes precedence over DEFAULT_PROVIDER for backward compatibility
    primary_provider: Optional[str] = Field(default=None, env="PRIMARY_PROVIDER")
    default_provider: str = Field(default="polygon", env="DEFAULT_PROVIDER")
    fallback_providers: List[str] = Field(default=["alpaca"], env="FALLBACK_PROVIDERS")
    active_providers: List[str] = Field(default=["polygon", "alpaca"], env="ACTIVE_PROVIDERS")
    
    # Polygon settings
    polygon_api_key: Optional[str] = Field(default=None, env="POLYGON_API_KEY")
    polygon_use_delayed: bool = Field(default=False, env="POLYGON_USE_DELAYED")
    polygon_websocket_enabled: bool = Field(default=True, env="POLYGON_WEBSOCKET_ENABLED")
    polygon_basic_plan: bool = Field(default=True, env="POLYGON_BASIC_PLAN")
    
    # Alpaca settings
    alpaca_api_key: Optional[str] = Field(default=None, env="ALPACA_API_KEY")
    alpaca_api_secret: Optional[str] = Field(default=None, env="ALPACA_API_SECRET")
    alpaca_subscription_level: str = Field(default="basic", env="ALPACA_SUBSCRIPTION_LEVEL")
    alpaca_ws_enabled: bool = Field(default=True, env="ALPACA_WS_ENABLED")
    alpaca_ws_url: str = Field(default="wss://stream.data.alpaca.markets/v2/iex", env="ALPACA_WS_URL")
    alpaca_ws_reconnect_delay: int = Field(default=5, env="ALPACA_WS_RECONNECT_DELAY")
    alpaca_ws_max_reconnect_attempts: int = Field(default=3, env="ALPACA_WS_MAX_RECONNECT_ATTEMPTS")
    
    # IEX Cloud settings
    iex_cloud_api_key: Optional[str] = Field(default=None, env="IEX_CLOUD_API_KEY")
    iex_cloud_version: str = Field(default="stable", env="IEX_CLOUD_VERSION")
    iex_cloud_sandbox: bool = Field(default=False, env="IEX_CLOUD_SANDBOX")
    
    # Alpha Vantage settings
    alpha_vantage_api_key: Optional[str] = Field(default=None, env="ALPHA_VANTAGE_API_KEY")
    
    # Yahoo Finance settings
    yahoo_finance_enabled: bool = Field(default=True, env="YAHOO_FINANCE_ENABLED")
    
    # Database settings
    database_url: str = Field(
        default="postgresql://user:password@localhost/neural_trader",
        env="DATABASE_URL"
    )
    redis_url: str = Field(
        default="redis://localhost:6379",
        env="REDIS_URL"
    )
    
    # TimescaleDB settings
    timescale_host: str = Field(default="localhost", env="TIMESCALE_HOST")
    timescale_port: int = Field(default=5432, env="TIMESCALE_PORT")
    timescale_database: str = Field(default="neural_trader", env="TIMESCALE_DATABASE")
    timescale_user: str = Field(default="postgres", env="TIMESCALE_USER")
    timescale_password: str = Field(default="postgres", env="TIMESCALE_PASSWORD")
    
    # Rate limiting
    rate_limit_requests_per_minute: int = Field(default=60, env="RATE_LIMIT_REQUESTS_PER_MINUTE")
    rate_limit_burst_size: int = Field(default=10, env="RATE_LIMIT_BURST_SIZE")
    
    # Performance settings
    max_concurrent_requests: int = Field(default=10, env="MAX_CONCURRENT_REQUESTS")
    request_timeout: int = Field(default=30, env="REQUEST_TIMEOUT")
    
    # Monitoring settings
    metrics_enabled: bool = Field(default=True, env="METRICS_ENABLED")
    metrics_port: int = Field(default=8000, env="METRICS_PORT")
    
    # Phase 2: Channel migration settings (INTERFACE_CONTRACT compliance)
    enable_legacy_channel: bool = Field(default=True, env="ENABLE_LEGACY_CHANNEL")
    redis_channel_prefix: str = Field(default="market", env="REDIS_CHANNEL_PREFIX")
    redis_dual_publish: bool = Field(default=True, env="REDIS_DUAL_PUBLISH")
    
    # Redis connection and performance settings
    redis_max_connections: int = Field(default=50, env="REDIS_MAX_CONNECTIONS")
    redis_publish_timeout: int = Field(default=5, env="REDIS_PUBLISH_TIMEOUT")
    redis_decode_responses: bool = Field(default=True, env="REDIS_DECODE_RESPONSES")
    
    class Config:
        """Pydantic config."""
        env_file = ".env"
        env_file_encoding = "utf-8"
        case_sensitive = False


# Global settings instance
_settings: Optional[Settings] = None


def get_settings() -> Settings:
    """Get or create settings instance."""
    global _settings
    if _settings is None:
        _settings = Settings()
    return _settings


def reset_settings():
    """Reset settings (mainly for testing)."""
    global _settings
    _settings = None