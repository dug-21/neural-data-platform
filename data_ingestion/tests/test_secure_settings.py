"""Tests for secure settings that prevent secrets from being loaded from files"""
import pytest
import os
import tempfile
from unittest.mock import patch, MagicMock
import warnings

# These imports will fail initially (TDD)
from data_ingestion.config.settings import SecureSettings


class TestSecureSettings:
    """Test secure settings implementation"""
    
    @pytest.fixture
    def temp_env_file(self):
        """Create a temporary .env file for testing"""
        with tempfile.NamedTemporaryFile(mode='w', suffix='.env', delete=False) as f:
            # Write both secrets and non-secrets
            f.write("# Configuration values (should load)\n")
            f.write("LOG_LEVEL=DEBUG\n")
            f.write("MAX_REQUESTS_PER_MINUTE=100\n")
            f.write("PROMETHEUS_PORT=9090\n")
            f.write("BATCH_SIZE=2000\n")
            f.write("\n# Secrets (should NOT load from file)\n")
            f.write("ALPHA_VANTAGE_API_KEY=secret_key_from_file\n")
            f.write("REDDIT_CLIENT_SECRET=reddit_secret_from_file\n")
            f.write("TIMESCALE_PASSWORD=db_password_from_file\n")
            f.write("REDIS_PASSWORD=redis_password_from_file\n")
            temp_path = f.name
        
        yield temp_path
        
        # Cleanup
        os.unlink(temp_path)
    
    def test_non_secrets_loaded_from_env_file(self, temp_env_file):
        """Test that non-secret configs are still loaded from .env file"""
        # Clear any existing env vars
        env_backup = dict(os.environ)
        for key in ['LOG_LEVEL', 'MAX_REQUESTS_PER_MINUTE', 'PROMETHEUS_PORT', 'BATCH_SIZE']:
            os.environ.pop(key, None)
        
        try:
            settings = SecureSettings(_env_file=temp_env_file)
            
            # Non-secrets should be loaded
            assert settings.log_level == "DEBUG"
            assert settings.max_requests_per_minute == 100
            assert settings.prometheus_port == 9090
            assert settings.batch_size == 2000
        finally:
            # Restore environment
            os.environ.clear()
            os.environ.update(env_backup)
    
    def test_secrets_not_loaded_from_env_file(self, temp_env_file, capfd):
        """Test that secrets are NOT loaded from .env file"""
        # Clear any existing env vars
        env_backup = dict(os.environ)
        secret_keys = [
            'ALPHA_VANTAGE_API_KEY', 'REDDIT_CLIENT_SECRET',
            'TIMESCALE_PASSWORD', 'REDIS_PASSWORD'
        ]
        for key in secret_keys:
            os.environ.pop(key, None)
        
        try:
            settings = SecureSettings(_env_file=temp_env_file)
            
            # Secrets should NOT be loaded from file
            assert settings.alpha_vantage_api_key is None
            assert settings.reddit_client_secret is None
            assert settings.timescale_password == ""  # Has default
            assert settings.redis_password is None
            
            # Should have warnings about ignored secrets
            captured = capfd.readouterr()
            assert "WARNING" in captured.out
            assert "ALPHA_VANTAGE_API_KEY" in captured.out
            assert "found in .env file - ignoring for security" in captured.out
        finally:
            # Restore environment
            os.environ.clear()
            os.environ.update(env_backup)
    
    def test_secrets_loaded_from_environment(self, temp_env_file):
        """Test that secrets ARE loaded from environment variables"""
        # Set secrets in environment
        env_secrets = {
            'ALPHA_VANTAGE_API_KEY': 'env_alpha_key',
            'REDDIT_CLIENT_SECRET': 'env_reddit_secret',
            'TIMESCALE_PASSWORD': 'env_db_password',
            'REDIS_PASSWORD': 'env_redis_password'
        }
        
        with patch.dict(os.environ, env_secrets):
            settings = SecureSettings(_env_file=temp_env_file)
            
            # Secrets should be loaded from environment, not file
            assert settings.alpha_vantage_api_key == 'env_alpha_key'
            assert settings.reddit_client_secret == 'env_reddit_secret'
            assert settings.timescale_password == 'env_db_password'
            assert settings.redis_password == 'env_redis_password'
    
    def test_all_secret_fields_identified(self):
        """Test that all secret fields are properly identified"""
        expected_secrets = {
            'iex_cloud_api_key', 'alpha_vantage_api_key', 'polygon_api_key',
            'finnhub_api_key', 'fred_api_key', 'reddit_client_id',
            'reddit_client_secret', 'quandl_api_key', 'newsapi_key',
            'yahoo_api_key', 'timescale_password', 'redis_password'
        }
        
        # Create an instance to check if the secret fields are used correctly
        # We'll verify the filtering logic works for all expected secrets
        settings = SecureSettings()
        
        # Check that the class has the _secret_fields defined
        # Note: This is checking the internal implementation, which ensures
        # our security filtering is properly configured
        assert hasattr(SecureSettings, '_secret_fields')
        assert SecureSettings._secret_fields.default == expected_secrets
    
    def test_environment_overrides_file_for_all(self, temp_env_file):
        """Test that environment variables override file values for all settings"""
        env_overrides = {
            'LOG_LEVEL': 'ERROR',  # Non-secret override
            'ALPHA_VANTAGE_API_KEY': 'env_override_key'  # Secret override
        }
        
        with patch.dict(os.environ, env_overrides):
            settings = SecureSettings(_env_file=temp_env_file)
            
            # Both should use environment values
            assert settings.log_level == 'ERROR'  # Not 'DEBUG' from file
            assert settings.alpha_vantage_api_key == 'env_override_key'
    
    def test_no_env_file_still_works(self):
        """Test that settings work without .env file"""
        # Use non-existent file
        settings = SecureSettings(_env_file='non_existent.env')
        
        # Should still have defaults
        assert settings.log_level == "INFO"  # Default
        assert settings.max_requests_per_minute == 60  # Default
    
    def test_reddit_client_id_special_case(self, temp_env_file):
        """Test that reddit_client_id is treated as a secret"""
        # Add reddit_client_id to temp file
        with open(temp_env_file, 'a') as f:
            f.write("REDDIT_CLIENT_ID=client_id_from_file\n")
        
        # Clear env
        os.environ.pop('REDDIT_CLIENT_ID', None)
        
        settings = SecureSettings(_env_file=temp_env_file)
        
        # Should not load from file
        assert settings.reddit_client_id is None
    
    def test_connection_urls_use_secure_passwords(self, temp_env_file):
        """Test that connection URLs use passwords from environment only"""
        # Set password in environment
        with patch.dict(os.environ, {
            'TIMESCALE_PASSWORD': 'secure_env_password',
            'REDIS_PASSWORD': 'secure_redis_password'
        }):
            settings = SecureSettings(_env_file=temp_env_file)
            
            # URLs should use environment passwords, not file
            assert 'secure_env_password' in settings.timescale_url
            assert 'secure_redis_password' in settings.redis_url
            
            # Should not contain file passwords
            assert 'db_password_from_file' not in settings.timescale_url
            assert 'redis_password_from_file' not in settings.redis_url
    
    @pytest.mark.parametrize("secret_field,env_var", [
        ("iex_cloud_api_key", "IEX_CLOUD_API_KEY"),
        ("alpha_vantage_api_key", "ALPHA_VANTAGE_API_KEY"),
        ("polygon_api_key", "POLYGON_API_KEY"),
        ("finnhub_api_key", "FINNHUB_API_KEY"),
        ("fred_api_key", "FRED_API_KEY"),
        ("reddit_client_secret", "REDDIT_CLIENT_SECRET"),
        ("quandl_api_key", "QUANDL_API_KEY"),
        ("newsapi_key", "NEWSAPI_KEY"),
        ("yahoo_api_key", "YAHOO_API_KEY"),
    ])
    def test_each_api_key_secure(self, secret_field, env_var, temp_env_file):
        """Test each API key is properly secured"""
        # Add to temp file
        with open(temp_env_file, 'a') as f:
            f.write(f"{env_var}=file_secret_value\n")
        
        # Clear from environment
        os.environ.pop(env_var, None)
        
        settings = SecureSettings(_env_file=temp_env_file)
        
        # Should not load from file
        assert getattr(settings, secret_field) is None
        
        # But should load from environment
        with patch.dict(os.environ, {env_var: 'env_secret_value'}):
            settings = SecureSettings(_env_file=temp_env_file)
            assert getattr(settings, secret_field) == 'env_secret_value'