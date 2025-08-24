"""
Error handling component for Phase 3 orchestrator.
Provides centralized error handling, logging, and recovery coordination.
"""

import logging
import traceback
from typing import Any, Dict, Optional
from datetime import datetime
from enum import Enum


class ErrorSeverity(Enum):
    """Error severity levels"""
    LOW = "low"
    MEDIUM = "medium"  
    HIGH = "high"
    CRITICAL = "critical"


class ErrorCategory(Enum):
    """Error categories for classification"""
    SWARM_MANAGEMENT = "swarm_management"
    ARCHITECTURE_PARSING = "architecture_parsing"
    VALIDATION = "validation"
    AGENT_COMMUNICATION = "agent_communication"
    RESOURCE_MANAGEMENT = "resource_management"
    INTEGRATION = "integration"


class ErrorHandler:
    """Centralized error handling for orchestrator operations"""
    
    def __init__(self):
        self.logger = logging.getLogger(__name__)
        self.error_history: Dict[str, Dict[str, Any]] = {}
        self.error_counts: Dict[str, int] = {}
    
    def handle_error(
        self, 
        operation: str, 
        error: Exception, 
        severity: ErrorSeverity = ErrorSeverity.MEDIUM,
        category: ErrorCategory = ErrorCategory.SWARM_MANAGEMENT,
        context: Optional[Dict[str, Any]] = None
    ) -> str:
        """
        Handle error with appropriate logging and tracking.
        
        Args:
            operation: Name of operation that failed
            error: Exception that occurred
            severity: Error severity level
            category: Error category for classification
            context: Additional context information
            
        Returns:
            str: Error ID for tracking
        """
        error_id = f"{operation}_{datetime.utcnow().strftime('%Y%m%d_%H%M%S')}"
        
        # Track error frequency
        self.error_counts[operation] = self.error_counts.get(operation, 0) + 1
        
        # Build error record
        error_record = {
            'error_id': error_id,
            'operation': operation,
            'error_type': type(error).__name__,
            'error_message': str(error),
            'severity': severity.value,
            'category': category.value,
            'context': context or {},
            'traceback': traceback.format_exc(),
            'timestamp': datetime.utcnow().isoformat(),
            'count': self.error_counts[operation]
        }
        
        # Store error history
        self.error_history[error_id] = error_record
        
        # Log based on severity
        log_message = f"Operation '{operation}' failed: {str(error)}"
        
        if severity == ErrorSeverity.CRITICAL:
            self.logger.critical(log_message, extra={'error_record': error_record})
        elif severity == ErrorSeverity.HIGH:
            self.logger.error(log_message, extra={'error_record': error_record})
        elif severity == ErrorSeverity.MEDIUM:
            self.logger.warning(log_message, extra={'error_record': error_record})
        else:
            self.logger.info(log_message, extra={'error_record': error_record})
        
        # Check for error patterns that require immediate attention
        if self.error_counts[operation] >= 3:
            self.logger.critical(
                f"Operation '{operation}' has failed {self.error_counts[operation]} times - immediate attention required"
            )
        
        return error_id
    
    def get_error_history(self, operation: Optional[str] = None) -> Dict[str, Dict[str, Any]]:
        """
        Get error history, optionally filtered by operation.
        
        Args:
            operation: Optional operation name to filter by
            
        Returns:
            Dict of error records
        """
        if operation:
            return {k: v for k, v in self.error_history.items() if v['operation'] == operation}
        return self.error_history.copy()
    
    def get_error_statistics(self) -> Dict[str, Any]:
        """
        Get error statistics and patterns.
        
        Returns:
            Dict with error statistics
        """
        total_errors = len(self.error_history)
        
        # Count by severity
        severity_counts = {}
        category_counts = {}
        
        for error_record in self.error_history.values():
            severity = error_record['severity']
            category = error_record['category']
            
            severity_counts[severity] = severity_counts.get(severity, 0) + 1
            category_counts[category] = category_counts.get(category, 0) + 1
        
        # Find most problematic operations
        top_failing_operations = sorted(
            self.error_counts.items(),
            key=lambda x: x[1],
            reverse=True
        )[:5]
        
        return {
            'total_errors': total_errors,
            'severity_breakdown': severity_counts,
            'category_breakdown': category_counts,
            'top_failing_operations': top_failing_operations,
            'unique_operations_failed': len(self.error_counts)
        }
    
    def should_trigger_rollback(self, operation: str) -> bool:
        """
        Determine if error pattern warrants system rollback.
        
        Args:
            operation: Operation name to check
            
        Returns:
            bool: True if rollback should be triggered
        """
        operation_failures = self.error_counts.get(operation, 0)
        
        # Rollback triggers
        if operation_failures >= 5:  # Too many failures of same operation
            return True
        
        recent_critical_errors = sum(
            1 for error in self.error_history.values()
            if error['severity'] == ErrorSeverity.CRITICAL.value and
            (datetime.utcnow() - datetime.fromisoformat(error['timestamp'])).seconds < 300  # Last 5 minutes
        )
        
        if recent_critical_errors >= 2:  # Multiple critical errors recently
            return True
        
        return False
    
    def clear_error_history(self, operation: Optional[str] = None):
        """
        Clear error history, optionally for specific operation.
        
        Args:
            operation: Optional operation name to clear errors for
        """
        if operation:
            # Clear errors for specific operation
            self.error_history = {
                k: v for k, v in self.error_history.items() 
                if v['operation'] != operation
            }
            if operation in self.error_counts:
                del self.error_counts[operation]
        else:
            # Clear all error history
            self.error_history.clear()
            self.error_counts.clear()
            
        self.logger.info(f"Cleared error history{' for ' + operation if operation else ''}")