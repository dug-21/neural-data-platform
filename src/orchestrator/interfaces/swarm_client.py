"""
SwarmClient Interface for Phase 3 orchestrator.
Defines the contract for swarm management operations.
"""

from abc import ABC, abstractmethod
from typing import Dict, List, Any, Optional


class SwarmClientInterface(ABC):
    """Interface for swarm management operations"""
    
    @abstractmethod
    def init_swarm(self, topology: str, max_agents: int, strategy: str) -> Dict[str, Any]:
        """
        Initialize swarm with specified topology and configuration.
        
        Args:
            topology: Swarm topology ('hierarchical', 'mesh', 'ring', 'star')
            max_agents: Maximum number of agents in swarm
            strategy: Agent distribution strategy
            
        Returns:
            Dict with swarm initialization results including swarm_id
        """
        pass
    
    @abstractmethod 
    def spawn_agent(self, type: str, name: str, capabilities: List[str]) -> Dict[str, Any]:
        """
        Spawn a new agent with specified type and capabilities.
        
        Args:
            type: Agent type ('coordinator', 'architect', 'analyst', 'optimizer')
            name: Unique agent name
            capabilities: List of agent capabilities
            
        Returns:
            Dict with agent spawn results including agent_id
        """
        pass
    
    @abstractmethod
    def send_task(self, task: Dict[str, Any]) -> Dict[str, Any]:
        """
        Send task to appropriate agent in swarm.
        
        Args:
            task: Task definition with type, target, and parameters
            
        Returns:
            Dict with task execution results including task_id
        """
        pass
    
    @abstractmethod
    def get_agent_status(self, agent_id: str) -> Dict[str, Any]:
        """
        Get current status of specified agent.
        
        Args:
            agent_id: ID of agent to check
            
        Returns:
            Dict with agent status information
        """
        pass
    
    @abstractmethod
    def get_swarm_status(self) -> Dict[str, Any]:
        """
        Get overall swarm status and health metrics.
        
        Returns:
            Dict with swarm status including agent counts and task metrics
        """
        pass
    
    @abstractmethod
    def destroy_swarm(self, swarm_id: str) -> Dict[str, Any]:
        """
        Destroy swarm and clean up resources.
        
        Args:
            swarm_id: ID of swarm to destroy
            
        Returns:
            Dict with destruction results
        """
        pass