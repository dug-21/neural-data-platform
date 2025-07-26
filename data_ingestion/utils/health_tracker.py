"""Simple health tracking singleton for data ingestion service."""
from datetime import datetime
from typing import Optional

class HealthTracker:
    """Singleton to track health updates across the application."""
    _instance = None
    _handler = None
    
    def __new__(cls):
        if cls._instance is None:
            cls._instance = super().__new__(cls)
        return cls._instance
    
    def set_handler(self, handler):
        """Set the health check handler instance."""
        self._handler = handler
    
    def update_data_timestamp(self, provider: str, symbol: str):
        """Update timestamp for last received data."""
        if self._handler:
            self._handler.update_data_timestamp(provider, symbol)

# Global instance
health_tracker = HealthTracker()