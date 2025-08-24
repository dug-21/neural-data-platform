"""
ArchitectureReader Interface for Phase 3 orchestrator.
Defines the contract for reading and parsing architecture documents.
"""

from abc import ABC, abstractmethod
from typing import Dict, List, Any


class ArchitectureReaderInterface(ABC):
    """Interface for reading architecture documents"""
    
    @abstractmethod
    def read_mvp_architecture(self) -> Dict[str, Any]:
        """
        Read and parse MVP architecture documents.
        
        Returns:
            Dict with parsed MVP architecture including:
            - services: List of MVP services
            - interfaces: Dict of interface definitions (grpc, redis, etc.)
            - ruv_fann: Configuration for RUV-FANN integration
            - dependencies: Service dependencies
        """
        pass
    
    @abstractmethod
    def read_phase3_plans(self) -> Dict[str, Any]:
        """
        Read and parse Phase 3 integration plans.
        
        Returns:
            Dict with Phase 3 plans including:
            - integration_phases: Number of integration phases
            - timeline_weeks: Estimated timeline in weeks
            - components: List of components to integrate
            - milestones: Key integration milestones
        """
        pass
    
    @abstractmethod
    def validate_document_integrity(self, document_path: str) -> bool:
        """
        Validate that architecture document is complete and well-formed.
        
        Args:
            document_path: Path to architecture document
            
        Returns:
            True if document is valid, False otherwise
        """
        pass
    
    @abstractmethod
    def extract_integration_requirements(self) -> Dict[str, List[str]]:
        """
        Extract integration requirements from architecture documents.
        
        Returns:
            Dict mapping integration points to their requirements
        """
        pass