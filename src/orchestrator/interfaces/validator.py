"""
Validator Interface for Phase 3 orchestrator.
Defines the contract for code and architecture validation operations.
"""

from abc import ABC, abstractmethod
from typing import Dict, List, Any, Optional


class ValidatorInterface(ABC):
    """Interface for validation operations"""
    
    @abstractmethod
    def check_no_todos(self, source_path: str) -> Dict[str, Any]:
        """
        Check that source code contains no TODO comments.
        
        Args:
            source_path: Path to source code directory
            
        Returns:
            Dict with validation results:
            - passed: bool indicating if validation passed
            - findings: List of TODO locations found
        """
        pass
    
    @abstractmethod
    def check_no_stubs(self, source_path: str) -> Dict[str, Any]:
        """
        Check that source code contains no stub function implementations.
        
        Args:
            source_path: Path to source code directory
            
        Returns:
            Dict with validation results:
            - passed: bool indicating if validation passed
            - findings: List of stub function locations
        """
        pass
    
    @abstractmethod
    def check_interfaces(self) -> Dict[str, Any]:
        """
        Check that all interface contracts are fully implemented.
        
        Returns:
            Dict with interface validation results:
            - passed: bool indicating if all interfaces implemented
            - complete: bool indicating completeness
            - missing: List of missing interface implementations
        """
        pass
    
    @abstractmethod
    def check_error_handling(self, source_path: str) -> Dict[str, Any]:
        """
        Check that proper error handling is implemented.
        
        Args:
            source_path: Path to source code directory
            
        Returns:
            Dict with error handling validation results:
            - passed: bool indicating if error handling is adequate
            - coverage: Percentage of functions with error handling
        """
        pass
    
    @abstractmethod
    def check_test_coverage(self) -> Dict[str, Any]:
        """
        Check that minimum test coverage requirements are met.
        
        Returns:
            Dict with test coverage results:
            - coverage: Current test coverage percentage
            - minimum_required: Minimum required coverage
            - passed: bool indicating if coverage meets requirements
        """
        pass
    
    @abstractmethod
    def validate_all(self) -> Dict[str, Any]:
        """
        Run all validation checks and return comprehensive results.
        
        Returns:
            Dict with complete validation results:
            - passed: bool indicating if all validations passed
            - results: Dict of individual validation results
            - summary: Summary of validation findings
        """
        pass