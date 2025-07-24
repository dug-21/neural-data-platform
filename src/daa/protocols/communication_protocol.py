"""
DAA Communication Protocol Implementation

This module defines the inter-agent communication protocols for the
Decentralized Autonomous Trading System.
"""

from abc import ABC, abstractmethod
from dataclasses import dataclass, field
from typing import List, Dict, Any, Optional, Callable
from enum import Enum
import asyncio
import json
import time
import hashlib
import hmac
from datetime import datetime
import uuid


class MessageType(Enum):
    """Types of messages in the DAA system"""
    SIGNAL = "signal"           # Trading signals
    QUERY = "query"             # Information requests
    RESPONSE = "response"       # Query responses
    BROADCAST = "broadcast"     # System-wide announcements
    CONSENSUS = "consensus"     # Consensus proposals
    HEARTBEAT = "heartbeat"     # Health checks
    KNOWLEDGE = "knowledge"     # Knowledge sharing
    ALERT = "alert"            # Risk/system alerts


class Priority(Enum):
    """Message priority levels"""
    CRITICAL = 4
    HIGH = 3
    MEDIUM = 2
    LOW = 1


@dataclass
class Message:
    """Standard message format for inter-agent communication"""
    id: str = field(default_factory=lambda: str(uuid.uuid4()))
    timestamp: str = field(default_factory=lambda: datetime.utcnow().isoformat())
    from_agent: str = ""
    to_agents: List[str] = field(default_factory=list)
    message_type: MessageType = MessageType.SIGNAL
    priority: Priority = Priority.MEDIUM
    payload: Dict[str, Any] = field(default_factory=dict)
    signature: Optional[str] = None
    correlation_id: Optional[str] = None
    ttl: int = 300  # Time to live in seconds
    
    def to_json(self) -> str:
        """Serialize message to JSON"""
        data = {
            "id": self.id,
            "timestamp": self.timestamp,
            "from": self.from_agent,
            "to": self.to_agents,
            "type": self.message_type.value,
            "priority": self.priority.value,
            "payload": self.payload,
            "signature": self.signature,
            "correlation_id": self.correlation_id,
            "ttl": self.ttl
        }
        return json.dumps(data)
    
    @classmethod
    def from_json(cls, json_str: str) -> 'Message':
        """Deserialize message from JSON"""
        data = json.loads(json_str)
        return cls(
            id=data["id"],
            timestamp=data["timestamp"],
            from_agent=data["from"],
            to_agents=data["to"],
            message_type=MessageType(data["type"]),
            priority=Priority(data["priority"]),
            payload=data["payload"],
            signature=data.get("signature"),
            correlation_id=data.get("correlation_id"),
            ttl=data.get("ttl", 300)
        )
    
    def sign(self, secret_key: str) -> None:
        """Sign the message with HMAC"""
        message_bytes = self.to_json().encode()
        self.signature = hmac.new(
            secret_key.encode(),
            message_bytes,
            hashlib.sha256
        ).hexdigest()
    
    def verify_signature(self, secret_key: str) -> bool:
        """Verify message signature"""
        if not self.signature:
            return False
            
        temp_sig = self.signature
        self.signature = None
        message_bytes = self.to_json().encode()
        expected_sig = hmac.new(
            secret_key.encode(),
            message_bytes,
            hashlib.sha256
        ).hexdigest()
        self.signature = temp_sig
        
        return hmac.compare_digest(self.signature, expected_sig)
    
    def is_expired(self) -> bool:
        """Check if message has expired"""
        created_time = datetime.fromisoformat(self.timestamp)
        current_time = datetime.utcnow()
        age_seconds = (current_time - created_time).total_seconds()
        return age_seconds > self.ttl


class CommunicationProtocol(ABC):
    """Abstract base class for communication protocols"""
    
    @abstractmethod
    async def send(self, message: Message) -> bool:
        """Send a message"""
        pass
    
    @abstractmethod
    async def receive(self) -> Optional[Message]:
        """Receive a message"""
        pass
    
    @abstractmethod
    async def subscribe(self, topic: str, callback: Callable) -> None:
        """Subscribe to a topic"""
        pass
    
    @abstractmethod
    async def unsubscribe(self, topic: str) -> None:
        """Unsubscribe from a topic"""
        pass


class PublishSubscribeProtocol(CommunicationProtocol):
    """Publish-Subscribe communication pattern"""
    
    def __init__(self):
        self.subscribers: Dict[str, List[Callable]] = {}
        self.message_queue = asyncio.Queue()
        self.running = False
        
    async def send(self, message: Message) -> bool:
        """Publish a message to subscribers"""
        try:
            # Determine topic from message type and payload
            topic = self._get_topic(message)
            
            # Notify all subscribers
            if topic in self.subscribers:
                for callback in self.subscribers[topic]:
                    asyncio.create_task(callback(message))
                    
            return True
        except Exception as e:
            print(f"Error publishing message: {e}")
            return False
    
    async def receive(self) -> Optional[Message]:
        """Not used in pub-sub pattern"""
        return None
    
    async def subscribe(self, topic: str, callback: Callable) -> None:
        """Subscribe to a topic"""
        if topic not in self.subscribers:
            self.subscribers[topic] = []
        self.subscribers[topic].append(callback)
    
    async def unsubscribe(self, topic: str) -> None:
        """Unsubscribe from a topic"""
        if topic in self.subscribers:
            del self.subscribers[topic]
    
    def _get_topic(self, message: Message) -> str:
        """Extract topic from message"""
        # Topic can be message type or custom topic in payload
        if "topic" in message.payload:
            return message.payload["topic"]
        return message.message_type.value


class RequestResponseProtocol(CommunicationProtocol):
    """Request-Response communication pattern"""
    
    def __init__(self):
        self.pending_requests: Dict[str, asyncio.Future] = {}
        self.request_handlers: Dict[str, Callable] = {}
        self.response_timeout = 30  # seconds
        
    async def send(self, message: Message) -> bool:
        """Send a request and wait for response"""
        try:
            if message.message_type == MessageType.QUERY:
                # Create future for response
                future = asyncio.Future()
                self.pending_requests[message.id] = future
                
                # Send request (would use actual transport here)
                await self._transport_send(message)
                
                # Wait for response with timeout
                try:
                    response = await asyncio.wait_for(
                        future,
                        timeout=self.response_timeout
                    )
                    return True
                except asyncio.TimeoutError:
                    del self.pending_requests[message.id]
                    return False
                    
            elif message.message_type == MessageType.RESPONSE:
                # Send response to waiting request
                if message.correlation_id in self.pending_requests:
                    self.pending_requests[message.correlation_id].set_result(message)
                return True
                
        except Exception as e:
            print(f"Error in request-response: {e}")
            return False
    
    async def receive(self) -> Optional[Message]:
        """Receive and handle requests"""
        # Would implement actual message receiving here
        pass
    
    async def subscribe(self, topic: str, callback: Callable) -> None:
        """Register request handler"""
        self.request_handlers[topic] = callback
    
    async def unsubscribe(self, topic: str) -> None:
        """Unregister request handler"""
        if topic in self.request_handlers:
            del self.request_handlers[topic]
    
    async def _transport_send(self, message: Message) -> None:
        """Actual message transport (to be implemented)"""
        pass


class ConsensusProtocol(CommunicationProtocol):
    """Byzantine Fault Tolerant Consensus Protocol"""
    
    def __init__(self, agent_id: str, total_agents: int):
        self.agent_id = agent_id
        self.total_agents = total_agents
        self.proposals: Dict[str, Dict[str, Any]] = {}
        self.votes: Dict[str, Dict[str, bool]] = {}
        self.consensus_threshold = (2 * total_agents) // 3 + 1
        
    async def propose(self, proposal_id: str, value: Any) -> None:
        """Propose a value for consensus"""
        proposal = {
            "id": proposal_id,
            "value": value,
            "proposer": self.agent_id,
            "timestamp": datetime.utcnow().isoformat(),
            "votes": {}
        }
        
        self.proposals[proposal_id] = proposal
        
        # Broadcast proposal
        message = Message(
            from_agent=self.agent_id,
            to_agents=["*"],  # Broadcast to all
            message_type=MessageType.CONSENSUS,
            priority=Priority.HIGH,
            payload={
                "action": "propose",
                "proposal": proposal
            }
        )
        
        await self.send(message)
    
    async def vote(self, proposal_id: str, vote: bool) -> None:
        """Vote on a proposal"""
        if proposal_id not in self.votes:
            self.votes[proposal_id] = {}
            
        self.votes[proposal_id][self.agent_id] = vote
        
        # Broadcast vote
        message = Message(
            from_agent=self.agent_id,
            to_agents=["*"],
            message_type=MessageType.CONSENSUS,
            priority=Priority.HIGH,
            payload={
                "action": "vote",
                "proposal_id": proposal_id,
                "vote": vote
            }
        )
        
        await self.send(message)
    
    async def check_consensus(self, proposal_id: str) -> Optional[Any]:
        """Check if consensus has been reached"""
        if proposal_id not in self.votes:
            return None
            
        yes_votes = sum(1 for v in self.votes[proposal_id].values() if v)
        
        if yes_votes >= self.consensus_threshold:
            return self.proposals[proposal_id]["value"]
            
        no_votes = sum(1 for v in self.votes[proposal_id].values() if not v)
        
        if no_votes > self.total_agents - self.consensus_threshold:
            # Proposal rejected
            return None
            
        # Consensus not yet reached
        return None
    
    async def send(self, message: Message) -> bool:
        """Send consensus message"""
        # Would implement actual sending here
        return True
    
    async def receive(self) -> Optional[Message]:
        """Receive consensus messages"""
        # Would implement actual receiving here
        pass
    
    async def subscribe(self, topic: str, callback: Callable) -> None:
        """Subscribe to consensus events"""
        pass
    
    async def unsubscribe(self, topic: str) -> None:
        """Unsubscribe from consensus events"""
        pass


class KnowledgeSharingProtocol:
    """Protocol for sharing knowledge between agents"""
    
    def __init__(self):
        self.knowledge_base: Dict[str, Any] = {}
        self.knowledge_subscribers: List[Callable] = []
        
    async def share_pattern(self, pattern_id: str, pattern_data: Dict[str, Any]) -> None:
        """Share a discovered pattern with other agents"""
        knowledge_item = {
            "id": pattern_id,
            "type": "pattern",
            "data": pattern_data,
            "timestamp": datetime.utcnow().isoformat(),
            "confidence": pattern_data.get("confidence", 0.0)
        }
        
        self.knowledge_base[pattern_id] = knowledge_item
        
        # Notify subscribers
        for subscriber in self.knowledge_subscribers:
            await subscriber(knowledge_item)
    
    async def share_learning(self, learning_id: str, learning_data: Dict[str, Any]) -> None:
        """Share learning outcomes"""
        knowledge_item = {
            "id": learning_id,
            "type": "learning",
            "data": learning_data,
            "timestamp": datetime.utcnow().isoformat(),
            "performance_impact": learning_data.get("performance_impact", 0.0)
        }
        
        self.knowledge_base[learning_id] = knowledge_item
        
        # Notify subscribers
        for subscriber in self.knowledge_subscribers:
            await subscriber(knowledge_item)
    
    def subscribe_to_knowledge(self, callback: Callable) -> None:
        """Subscribe to knowledge updates"""
        self.knowledge_subscribers.append(callback)
    
    def get_knowledge(self, knowledge_type: Optional[str] = None) -> Dict[str, Any]:
        """Retrieve knowledge from the base"""
        if knowledge_type:
            return {
                k: v for k, v in self.knowledge_base.items()
                if v.get("type") == knowledge_type
            }
        return self.knowledge_base


class MessageRouter:
    """Routes messages between agents based on various protocols"""
    
    def __init__(self):
        self.agents: Dict[str, Any] = {}
        self.protocols: Dict[str, CommunicationProtocol] = {
            "pubsub": PublishSubscribeProtocol(),
            "request_response": RequestResponseProtocol(),
            "consensus": None,  # Initialized per agent
            "knowledge": KnowledgeSharingProtocol()
        }
        self.message_log: List[Message] = []
        
    async def route_message(self, message: Message) -> bool:
        """Route message to appropriate destination(s)"""
        try:
            # Log message
            self.message_log.append(message)
            
            # Route based on message type
            if message.message_type in [MessageType.SIGNAL, MessageType.BROADCAST, MessageType.ALERT]:
                # Use pub-sub for these types
                return await self.protocols["pubsub"].send(message)
                
            elif message.message_type in [MessageType.QUERY, MessageType.RESPONSE]:
                # Use request-response
                return await self.protocols["request_response"].send(message)
                
            elif message.message_type == MessageType.CONSENSUS:
                # Use consensus protocol
                if self.protocols["consensus"]:
                    return await self.protocols["consensus"].send(message)
                    
            elif message.message_type == MessageType.KNOWLEDGE:
                # Handle knowledge sharing
                if "pattern" in message.payload:
                    await self.protocols["knowledge"].share_pattern(
                        message.payload["pattern_id"],
                        message.payload["pattern"]
                    )
                return True
                
            return False
            
        except Exception as e:
            print(f"Error routing message: {e}")
            return False
    
    def register_agent(self, agent_id: str, agent_ref: Any) -> None:
        """Register an agent with the router"""
        self.agents[agent_id] = agent_ref
        
        # Initialize consensus protocol for this agent if needed
        if not self.protocols["consensus"]:
            self.protocols["consensus"] = ConsensusProtocol(
                agent_id,
                len(self.agents)
            )
    
    def get_message_stats(self) -> Dict[str, Any]:
        """Get messaging statistics"""
        stats = {
            "total_messages": len(self.message_log),
            "messages_by_type": {},
            "messages_by_priority": {},
            "average_ttl": 0
        }
        
        for msg in self.message_log:
            # Count by type
            msg_type = msg.message_type.value
            stats["messages_by_type"][msg_type] = stats["messages_by_type"].get(msg_type, 0) + 1
            
            # Count by priority
            priority = msg.priority.name
            stats["messages_by_priority"][priority] = stats["messages_by_priority"].get(priority, 0) + 1
            
            # Calculate average TTL
            stats["average_ttl"] += msg.ttl
            
        if self.message_log:
            stats["average_ttl"] /= len(self.message_log)
            
        return stats


# Example usage
if __name__ == "__main__":
    async def main():
        # Create a message router
        router = MessageRouter()
        
        # Example: Trading signal message
        signal_msg = Message(
            from_agent="strategy_agent_1",
            to_agents=["execution_agent_1", "risk_agent_1"],
            message_type=MessageType.SIGNAL,
            priority=Priority.HIGH,
            payload={
                "action": "BUY",
                "symbol": "AAPL",
                "quantity": 100,
                "confidence": 0.85,
                "reasoning": "Strong momentum detected"
            }
        )
        
        # Sign the message
        signal_msg.sign("secret_key")
        
        # Route the message
        success = await router.route_message(signal_msg)
        print(f"Message routed: {success}")
        
        # Example: Consensus proposal
        consensus_msg = Message(
            from_agent="risk_agent_1",
            to_agents=["*"],
            message_type=MessageType.CONSENSUS,
            priority=Priority.CRITICAL,
            payload={
                "action": "propose",
                "proposal": {
                    "id": "risk_limit_change",
                    "value": {"max_position_size": 10000},
                    "reason": "Market volatility increased"
                }
            }
        )
        
        await router.route_message(consensus_msg)
        
        # Get statistics
        stats = router.get_message_stats()
        print(f"Message statistics: {stats}")
    
    # Run the example
    asyncio.run(main())