# Memory Intelligence Architecture Research

**Research Date:** 2025-12-20
**Goal:** Accelerate agent development through intelligent, persistent memory systems that learn over time

---

## Executive Summary

This research analyzes three complementary technologies for enhancing AI agent memory and intelligence:

| Technology | Role | Key Capability |
|------------|------|----------------|
| **claude-flow@alpha** | Orchestration | Simple memory, agent coordination, MCP tools |
| **agentdb** | Learning Memory | Vector search, RL algorithms, QUIC sync, skills |
| **ruvector** | Vector Service | HTTP/gRPC server, clustering, embeddings |

**Key Finding:** A layered architecture combining all three tools provides the optimal balance of simplicity, intelligence, and performance.

---

## 1. Technology Analysis

### 1.1 Claude-Flow Memory (Current System)

**Capabilities:**
- Simple key-value storage with namespaces
- Commands: `store`, `query`, `list`, `export`, `import`, `clear`
- TTL (time-to-live) support
- Pattern storage via get-pattern/save-pattern skills

**Current Pattern Namespace:**
```
ndp-patterns
├── architecture/      # ADRs, design patterns
├── data-flow/         # Pipeline patterns
├── development/       # Implementation procedures
├── deployment/        # Operational procedures
├── troubleshooting/   # Checklists, common issues
├── conventions/       # Naming rules, style guides
├── procedures/        # Multi-component changes
└── streams/           # Active data stream docs
```

**Strengths:**
- Fast key lookups (~2-3ms)
- Simple API via MCP tools
- Namespace isolation
- Session persistence in `.swarm/memory.db`

**Limitations:**
- **No semantic search** - Only exact/pattern matching
- **No learning** - Static storage, no improvement over time
- **No vector similarity** - Can't find "similar" patterns
- **Limited context synthesis** - Returns raw patterns, no aggregation

**Assessment:** Excellent for configuration and static documentation, insufficient for intelligent agent memory.

---

### 1.2 AgentDB (Learning Memory Engine)

**Version:** v2.0+ (alpha with frontier features)
**Backend:** RuVector Rust core (61μs latency) or SQLite fallback
**Installation:** `npx agentdb@latest` or `claude mcp add agentdb npx agentdb@latest mcp start`

#### Core Architecture

```
AgentDB Architecture:
├── Vector Database Layer
│   ├── HNSW Indexing (150x-12,500x faster)
│   ├── Quantization (4-32x memory reduction)
│   │   ├── Binary: 32x reduction, ~5% accuracy loss
│   │   ├── Scalar: 4x reduction, ~2% accuracy loss
│   │   └── Product: 8-16x reduction, ~5-7% accuracy loss
│   └── Multiple Distance Metrics (cosine, euclidean, dot)
│
├── Six Cognitive Memory Patterns
│   ├── ReasoningBank: 32.6M pattern ops/sec
│   ├── Reflexion: Episodic replay with self-critique
│   ├── Skill Library: Auto-consolidation from success
│   ├── Causal Memory Graph: Intervention-based causality
│   ├── Causal Recall: Utility-ranked retrieval
│   └── Nightly Learner: Background pattern discovery
│
├── 9 Reinforcement Learning Algorithms
│   ├── Decision Transformer (recommended)
│   ├── Q-Learning
│   ├── SARSA
│   ├── Actor-Critic
│   ├── Active Learning
│   ├── Adversarial Training
│   ├── Curriculum Learning
│   ├── Federated Learning
│   └── Multi-Task Learning
│
├── 4 Reasoning Agents
│   ├── PatternMatcher: HNSW-powered similarity search
│   ├── ContextSynthesizer: Multi-memory aggregation
│   ├── MemoryOptimizer: Consolidation + pruning
│   └── ExperienceCurator: Quality-based filtering
│
└── Distributed Coordination
    └── QUIC Sync: <1ms multi-agent coordination
```

#### Key CLI Commands

```bash
# Initialize database
npx agentdb@latest init ./memory.db --dimension 1536 --preset medium

# Store experiences with self-critique (Reflexion)
npx agentdb@latest reflexion store <session-id> <task> <reward> <success> [critique]

# Semantic query with context synthesis
npx agentdb@latest reflexion retrieve <task> --k 10 --synthesize-context

# Train learning models
npx agentdb@latest train --domain "code-edits" --epochs 50 --batch-size 32

# Consolidate successful patterns into skills
npx agentdb@latest skill consolidate 3 0.7 7 true

# Start QUIC sync server for multi-agent coordination
npx agentdb@latest sync start-server --port 4433 --auth-token secret123

# MCP server for Claude Code integration
npx agentdb@latest mcp start
```

#### Performance Benchmarks

| Operation | Traditional | AgentDB | Improvement |
|-----------|-------------|---------|-------------|
| Pattern Search | 15ms | 100μs | **150x** |
| Large-scale Query (1M) | 100s | 8ms | **12,500x** |
| Batch Insert (100) | 1s | 2ms | **500x** |
| Memory (1M vectors) | 3GB | 96MB | **32x** (binary) |

#### Unique Value for Agent Memory

1. **Reflexion Pattern**: Agents critique their own failures and store refined strategies
2. **Skill Consolidation**: Automatically extracts reusable patterns from 3+ successful episodes
3. **Causal Learning**: Understands "why" something works, not just "what" worked
4. **QUIC Sync**: <1ms knowledge sharing between agents in a swarm
5. **Semantic Search**: Finds relevant context even with different wording

---

### 1.3 RuVector (Vector Service)

**Version:** 0.1.35
**Architecture:** Rust core with SIMD acceleration
**Modes:** CLI, HTTP server, gRPC server, Cluster

#### Server Capabilities

```bash
# Start HTTP/gRPC server (persistent service)
ruvector server --port 8080 --grpc-port 50051 --data-dir ./ruvector-data --cors

# Cluster operations
ruvector cluster --status       # Show cluster status
ruvector cluster --join <addr>  # Join existing cluster
ruvector cluster --nodes        # List cluster nodes
ruvector cluster --leader       # Show current leader
```

#### Key Features

| Feature | Description |
|---------|-------------|
| **HTTP/gRPC Server** | RESTful API + streaming support |
| **Distributed Cluster** | Raft consensus, multi-master replication |
| **Embedding Generation** | Built-in models (all-minilm-l6-v2) |
| **Semantic Router** | Intent classification for query routing |
| **Graph Neural Networks** | 8-head attention for relationship queries |
| **Performance** | <0.5ms latency, SIMD acceleration |

#### Integration Patterns

```bash
# Generate embeddings locally
ruvector embed --text "How do I add a new stream?" --model all-minilm-l6-v2

# Semantic routing for query classification
ruvector router --route "authentication error" --intents intents.json

# Export/Import for data migration
ruvector export ./db --output backup.json
ruvector import backup.json --target ./new-db
```

#### Cluster Architecture

```
RuVector Cluster Topology:
                    ┌─────────────┐
                    │   Leader    │
                    │  (writes)   │
                    └──────┬──────┘
                           │ Raft Consensus
           ┌───────────────┼───────────────┐
           │               │               │
    ┌──────▼──────┐ ┌──────▼──────┐ ┌──────▼──────┐
    │  Follower 1 │ │  Follower 2 │ │  Follower 3 │
    │   (reads)   │ │   (reads)   │ │   (reads)   │
    └─────────────┘ └─────────────┘ └─────────────┘
```

**Advantages of Central Service:**
- Single source of truth for embeddings
- High-availability via cluster mode
- Centralized embedding model (consistent vectors)
- gRPC for high-performance streaming
- Horizontal scaling via auto-sharding

---

## 2. Integration Architecture Options

### Option A: RuVector as Central Service + AgentDB for Local Learning

```
Architecture A: Centralized Vector Service

┌─────────────────────────────────────────────────────────┐
│                    RuVector Cluster                     │
│  ┌──────────┐    ┌──────────┐    ┌──────────┐          │
│  │  Leader  │◄──►│ Follower │◄──►│ Follower │          │
│  └────┬─────┘    └──────────┘    └──────────┘          │
│       │ HTTP/gRPC API                                   │
└───────┼─────────────────────────────────────────────────┘
        │
        ▼
┌───────────────────────────────────────────────────────┐
│                   Agent Layer                          │
│  ┌───────────────┐   ┌───────────────┐                │
│  │   Agent A     │   │   Agent B     │                │
│  │  ┌─────────┐  │   │  ┌─────────┐  │                │
│  │  │ AgentDB │  │   │  │ AgentDB │  │                │
│  │  │ (local) │  │   │  │ (local) │  │                │
│  │  └─────────┘  │   │  └─────────┘  │                │
│  └───────────────┘   └───────────────┘                │
│                                                        │
│  claude-flow orchestrates + coordinates                │
└───────────────────────────────────────────────────────┘
```

**Data Flow:**
1. Agent generates query → sends to RuVector for embedding
2. RuVector returns embedding → Agent queries local AgentDB
3. AgentDB returns semantically similar experiences
4. Agent stores new experience in local AgentDB
5. Periodic sync: AgentDB QUIC pushes learnings to peers

**Pros:**
- Consistent embeddings across all agents
- RuVector handles embedding generation (offloads from agents)
- Local AgentDB enables offline operation
- QUIC sync for multi-agent learning

**Cons:**
- Network dependency for embeddings
- Two systems to maintain
- Additional latency for embedding round-trip

---

### Option B: AgentDB with QUIC Sync (Peer-to-Peer)

```
Architecture B: Distributed AgentDB Mesh

┌───────────────────────────────────────────────────────┐
│                   QUIC Mesh Network                    │
│                     (<1ms sync)                        │
│                                                        │
│  ┌───────────┐         ┌───────────┐                  │
│  │ Agent A   │◄───────►│ Agent B   │                  │
│  │ ┌───────┐ │         │ ┌───────┐ │                  │
│  │ │AgentDB│ │         │ │AgentDB│ │                  │
│  │ │(full) │ │         │ │(full) │ │                  │
│  │ └───────┘ │         │ └───────┘ │                  │
│  └─────┬─────┘         └─────┬─────┘                  │
│        │                     │                         │
│        └─────────┬───────────┘                         │
│                  │                                     │
│           ┌──────▼──────┐                              │
│           │  Agent C    │                              │
│           │  ┌───────┐  │                              │
│           │  │AgentDB│  │                              │
│           │  │(full) │  │                              │
│           │  └───────┘  │                              │
│           └─────────────┘                              │
│                                                        │
│   claude-flow coordinates task distribution            │
└───────────────────────────────────────────────────────┘
```

**Data Flow:**
1. Agent learns pattern → stores in local AgentDB
2. AgentDB QUIC broadcasts to all peers (<1ms)
3. All agents update their local vector DBs
4. Future queries find shared learnings locally

**Pros:**
- No central point of failure
- <1ms knowledge sharing
- Each agent has full memory locally
- Works offline after initial sync

**Cons:**
- Each agent stores full database (memory overhead)
- Eventual consistency (conflicts possible)
- No centralized embedding service

---

### Option C: Layered Memory Architecture (Recommended)

```
Architecture C: Tiered Memory System

┌─────────────────────────────────────────────────────────────┐
│                     Layer 1: Quick Access                    │
│  ┌─────────────────────────────────────────────────────────┐│
│  │              claude-flow Memory                          ││
│  │  • Key-value patterns (ndp-patterns namespace)          ││
│  │  • Configuration, conventions, procedures                ││
│  │  • <3ms access                                           ││
│  └─────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                   Layer 2: Semantic Search                   │
│  ┌─────────────────────────────────────────────────────────┐│
│  │                      AgentDB                             ││
│  │  • Vector similarity search (<100μs)                     ││
│  │  • ReasoningBank for pattern matching                    ││
│  │  • Reflexion for self-improvement                        ││
│  │  • Skills consolidation                                  ││
│  │  • RL algorithms for learning                            ││
│  └─────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│              Layer 3: Heavy Compute (Optional)               │
│  ┌─────────────────────────────────────────────────────────┐│
│  │              RuVector Service (Cluster)                  ││
│  │  • Centralized embedding generation                      ││
│  │  • GNN for complex relationship queries                  ││
│  │  • Semantic router for query classification              ││
│  │  • Cross-project knowledge base                          ││
│  └─────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────┘
```

**Query Routing Logic:**

```javascript
async function intelligentQuery(query) {
  // Layer 1: Check for exact pattern match
  const pattern = await claudeFlow.memory.query(query, {
    namespace: 'ndp-patterns'
  });
  if (pattern && pattern.confidence > 0.9) {
    return { source: 'claude-flow', data: pattern };
  }

  // Layer 2: Semantic search for similar experiences
  const similar = await agentDB.retrieveWithReasoning(query, {
    domain: getCurrentDomain(),
    k: 5,
    synthesizeContext: true
  });
  if (similar.memories.length > 0 && similar.memories[0].similarity > 0.7) {
    return { source: 'agentdb', data: similar };
  }

  // Layer 3: Complex query to central service (optional)
  if (ruvectorEnabled) {
    const embedding = await ruvector.embed(query);
    const graphResults = await ruvector.gnnQuery(embedding);
    return { source: 'ruvector', data: graphResults };
  }

  // No match - agent must explore
  return { source: 'exploration', data: null };
}
```

**Pros:**
- Best of all three systems
- Progressive complexity (simple → semantic → GNN)
- claude-flow for fast lookups
- AgentDB for learning and semantic search
- RuVector for heavy compute (optional)
- Each layer handles what it does best

**Cons:**
- Most complex setup
- Three systems to coordinate
- Query routing logic required

---

### Option D: Hybrid Intelligent Routing

```
Architecture D: Router-Based Memory Orchestration

┌────────────────────────────────────────────────────────────┐
│                   RuVector Semantic Router                  │
│  ┌────────────────────────────────────────────────────────┐│
│  │   Query: "How do I add a new data stream?"             ││
│  │   Route: → development/procedures                       ││
│  │                                                         ││
│  │   Query: "Why did my test fail with async error?"      ││
│  │   Route: → agentdb/reflexion                            ││
│  │                                                         ││
│  │   Query: "What's the naming convention for streams?"   ││
│  │   Route: → claude-flow/conventions                      ││
│  └────────────────────────────────────────────────────────┘│
└──────────────────────────┬─────────────────────────────────┘
                           │ Routes to appropriate tier
           ┌───────────────┼───────────────┐
           ▼               ▼               ▼
    ┌─────────────┐ ┌─────────────┐ ┌─────────────┐
    │claude-flow  │ │  AgentDB    │ │  RuVector   │
    │   memory    │ │  Learning   │ │   Graph     │
    │  (static)   │ │  (dynamic)  │ │  (complex)  │
    └─────────────┘ └─────────────┘ └─────────────┘
```

**Routing Categories:**

| Query Type | Route To | Rationale |
|------------|----------|-----------|
| "How do I..." (procedures) | claude-flow | Static documentation |
| "What's the convention..." | claude-flow | Configuration |
| "Why did X fail?" | AgentDB Reflexion | Self-critique memory |
| "Similar to this bug..." | AgentDB Vector | Semantic similarity |
| "What patterns suggest..." | AgentDB Skills | Consolidated knowledge |
| "Relationships between..." | RuVector GNN | Graph queries |
| "Find all related..." | RuVector | Complex traversals |

**Pros:**
- Automatic query classification
- Optimal routing to best-fit system
- Single entry point for all memory queries
- Self-learning router (improves over time)

**Cons:**
- Requires training router with example queries
- Additional latency for routing decision
- Router is another component to maintain

---

## 3. Recommended Architecture

### Primary Recommendation: Option C (Layered Memory Architecture)

**Rationale:**
1. **Progressive Complexity**: Start simple, add layers as needed
2. **Preserves Existing Patterns**: claude-flow memory continues as-is
3. **AgentDB Proven Value**: 150x-12,500x performance gains
4. **RuVector Optional**: Can add later for cross-project knowledge

### Implementation Phases

#### Phase 1: AgentDB Integration (Week 1-2)

```bash
# Install AgentDB MCP server
claude mcp add agentdb npx agentdb@latest mcp start

# Initialize database for NDP project
npx agentdb@latest init .agentdb/ndp.db --dimension 1536 --preset medium

# Create initial patterns from existing memory
# Export claude-flow patterns → Import as AgentDB vectors
```

**Tasks:**
1. Add AgentDB MCP server to Claude Code
2. Initialize vector database with HNSW indexing
3. Migrate high-value ndp-patterns to AgentDB
4. Create hooks for storing agent experiences

#### Phase 2: Reflexion + Skills (Week 2-3)

```bash
# Store experiences during tasks
npx agentdb@latest reflexion store \
  --session "session-id" \
  --task "add-stream-solar" \
  --reward 0.95 \
  --success true \
  --critique "Used correct procedure but forgot to run deploy.sh sync"

# Consolidate successful patterns into reusable skills
npx agentdb@latest skill consolidate 3 0.7 7 true
```

**Tasks:**
1. Add post-task hooks to store experiences
2. Enable Reflexion self-critique on failures
3. Schedule skill consolidation (daily/weekly)
4. Test semantic retrieval for similar tasks

#### Phase 3: Multi-Agent QUIC Sync (Week 3-4)

```bash
# Start QUIC sync server
npx agentdb@latest sync start-server --port 4433 --auth-token $QUIC_TOKEN

# Configure agents to sync
export AGENTDB_QUIC_SYNC=true
export AGENTDB_QUIC_PEERS=localhost:4433
```

**Tasks:**
1. Configure QUIC sync for swarm agents
2. Enable federated learning across agents
3. Test knowledge sharing between researcher/coder/tester
4. Measure improvement in swarm coordination

#### Phase 4: RuVector Service (Optional, Week 4+)

```bash
# Start RuVector server as persistent service
docker run -d --name ruvector \
  -p 8080:8080 -p 50051:50051 \
  -v ruvector-data:/data \
  ruvector/server:latest

# Configure agents to use centralized embeddings
export RUVECTOR_URL=http://localhost:8080
```

**Tasks:**
1. Deploy RuVector server (Docker or bare metal)
2. Configure cluster for high availability
3. Migrate embedding generation to RuVector
4. Enable GNN queries for complex relationships

---

## 4. Memory Patterns for Smarter Agents

### 4.1 What Agents Should Remember

| Category | Store In | TTL | Example |
|----------|----------|-----|---------|
| **Project Patterns** | claude-flow | Permanent | Architecture decisions, conventions |
| **Task Experiences** | AgentDB | 90 days | Successful/failed attempts + critique |
| **Consolidated Skills** | AgentDB | Permanent | Reusable procedures (3+ successes) |
| **Causal Relationships** | AgentDB | 180 days | "X causes Y with 85% confidence" |
| **Session Context** | AgentDB | 7 days | Current task state, dependencies |

### 4.2 What Agents Should NOT Remember

| Category | Reason | Alternative |
|----------|--------|-------------|
| Raw code snippets | Bloats memory, stale quickly | Store patterns, not code |
| Full file contents | Too large for vector storage | Store file references |
| Low-confidence experiences | Noise reduces retrieval quality | Prune < 0.5 confidence |
| Duplicate patterns | Wastes memory | Enable auto-consolidation |
| Personal/sensitive data | Security risk | Never store credentials |

### 4.3 Memory Lifecycle

```
Experience Lifecycle:

 ┌─────────────────────────────────────────────────────────┐
 │  STORE: Agent completes task                            │
 │  → Store experience with embedding + metadata           │
 │  → Record success/failure + reward                      │
 └─────────────────────────────────────────────────────────┘
                              │
                              ▼ (If failed)
 ┌─────────────────────────────────────────────────────────┐
 │  REFLECT: Analyze failure                               │
 │  → Generate self-critique with LLM                      │
 │  → Store refined strategy as new experience             │
 └─────────────────────────────────────────────────────────┘
                              │
                              ▼ (After 3+ successes)
 ┌─────────────────────────────────────────────────────────┐
 │  CONSOLIDATE: Extract skill                             │
 │  → Cluster similar successful experiences               │
 │  → Extract common patterns                              │
 │  → Create reusable skill template                       │
 └─────────────────────────────────────────────────────────┘
                              │
                              ▼ (Periodically)
 ┌─────────────────────────────────────────────────────────┐
 │  PRUNE: Optimize memory                                 │
 │  → Remove low-confidence (<0.5)                         │
 │  → Remove stale (>90 days unused)                       │
 │  → Merge duplicates                                     │
 └─────────────────────────────────────────────────────────┘
```

---

## 5. Configuration Recommendations

### 5.1 AgentDB Configuration

```typescript
// Recommended configuration for NDP project
const config = {
  dbPath: '.agentdb/ndp.db',
  dimension: 1536,           // OpenAI ada-002 compatible
  quantizationType: 'scalar', // 4x memory reduction, 98% accuracy
  cacheSize: 1000,           // 1000 pattern cache
  enableLearning: true,      // Enable RL algorithms
  enableReasoning: true,     // Enable reasoning agents
  hnswM: 16,                 // Balanced HNSW connections
  hnswEfSearch: 100,         // Balanced search quality
};
```

### 5.2 Memory Namespace Strategy

```yaml
Namespace Organization:
  claude-flow (key-value):
    ndp-patterns:
      - architecture/*      # Static design decisions
      - conventions/*       # Naming rules, style guides
      - procedures/*        # Step-by-step instructions

  agentdb (vector):
    ndp-experiences:
      - tasks/*             # Task execution history
      - errors/*            # Error resolution patterns
      - workflows/*         # Multi-step workflows

    ndp-skills:
      - code-patterns/*     # Consolidated coding skills
      - debugging/*         # Debugging strategies
      - testing/*           # Testing approaches

    ndp-causal:
      - cause-effect/*      # Causal relationships
      - interventions/*     # What changes cause what
```

### 5.3 Hook Integration

```bash
# Post-task hook: Store experience in AgentDB
npx claude-flow@alpha hooks post-task \
  --store-experience \
  --agentdb-domain "ndp-experiences/tasks"

# Pre-task hook: Retrieve similar experiences
npx claude-flow@alpha hooks pre-task \
  --query-agentdb \
  --top-k 5 \
  --synthesize-context

# Session end hook: Trigger skill consolidation
npx claude-flow@alpha hooks session-end \
  --consolidate-skills \
  --min-success-rate 0.7
```

---

## 6. Expected Outcomes

### 6.1 Performance Improvements

| Metric | Current | With AgentDB | Improvement |
|--------|---------|--------------|-------------|
| Pattern lookup | 2-3ms | <100μs | **20x** |
| Context retrieval | Manual | Automatic semantic | **N/A** (new) |
| Memory usage | ~100MB | ~25MB (scalar) | **4x reduction** |
| Cross-agent sync | Manual | <1ms QUIC | **Real-time** |

### 6.2 Agent Intelligence Improvements

| Capability | Before | After |
|------------|--------|-------|
| Learn from failures | No | Yes (Reflexion) |
| Find similar experiences | No | Yes (vector search) |
| Consolidate successful patterns | Manual | Automatic (Skills) |
| Share knowledge between agents | No | Yes (QUIC sync) |
| Understand causality | No | Yes (Causal learning) |
| Self-improve over time | No | Yes (RL algorithms) |

### 6.3 Token Efficiency

| Scenario | Without Memory | With AgentDB | Savings |
|----------|----------------|--------------|---------|
| Repeat task | Full re-learn | Retrieve skill | ~80% |
| Similar error | Full debug | Retrieve fix | ~70% |
| Swarm coordination | Redundant work | Shared knowledge | ~75% |

---

## 7. Next Steps

### Immediate Actions

1. **Install AgentDB MCP server**
   ```bash
   claude mcp add agentdb npx agentdb@latest mcp start
   ```

2. **Initialize NDP database**
   ```bash
   npx agentdb@latest init .agentdb/ndp.db --dimension 1536 --preset medium
   ```

3. **Create integration hooks**
   - Post-task: Store experiences
   - Pre-task: Retrieve context
   - Session-end: Consolidate skills

### Future Considerations

- **RuVector Cluster**: Deploy when cross-project knowledge needed
- **Semantic Router**: Add when query classification valuable
- **GNN Queries**: Enable for complex relationship exploration
- **Federated Learning**: Implement for privacy-sensitive domains

---

## 8. Tool Reference

### AgentDB CLI Cheatsheet

```bash
# Database
npx agentdb@latest init <db-path> --dimension 1536 --preset medium
npx agentdb@latest stats <db-path>
npx agentdb@latest export <db-path> ./backup.json
npx agentdb@latest import ./backup.json

# Vector Search
npx agentdb@latest query --query "<text>" --k 10 --synthesize-context
npx agentdb@latest vector-search <db-path> "[0.1,0.2,...]" -k 10 -m cosine

# Reflexion (Self-Critique)
npx agentdb@latest reflexion store <session> <task> <reward> <success> [critique]
npx agentdb@latest reflexion retrieve <task> --k 10 --synthesize-context

# Skills
npx agentdb@latest skill create <name> <description> [code]
npx agentdb@latest skill search <query> [k]
npx agentdb@latest skill consolidate [min-attempts] [min-reward] [time-window-days]

# Training
npx agentdb@latest train --domain <domain> --epochs 50 --batch-size 32

# QUIC Sync
npx agentdb@latest sync start-server --port 4433 --auth-token <token>
npx agentdb@latest sync connect <host> <port> --auth-token <token>
npx agentdb@latest sync push --server <host:port> --incremental
npx agentdb@latest sync pull --server <host:port> --incremental

# MCP Server
npx agentdb@latest mcp start
```

### RuVector CLI Cheatsheet

```bash
# Server
ruvector server --port 8080 --grpc-port 50051 --data-dir ./data --cors

# Cluster
ruvector cluster --status
ruvector cluster --join <address>
ruvector cluster --nodes
ruvector cluster --leader

# Embeddings
ruvector embed --text "query text" --model all-minilm-l6-v2
ruvector embed --file inputs.txt --output embeddings.json

# Router
ruvector router --route "query" --intents intents.json
ruvector router --add-intent <name> --examples '["example1", "example2"]'

# Database
ruvector create ./db.vec --dimensions 768
ruvector search ./db.vec --query "[0.1, 0.2, ...]" --top-k 10
ruvector export ./db.vec --output backup.json
ruvector import backup.json --target ./db.vec
```

### claude-flow Memory Cheatsheet

```bash
# Store/Retrieve
claude-flow memory store "<key>" "<value>" --namespace ndp-patterns
claude-flow memory query "<pattern>" --namespace ndp-patterns

# Management
claude-flow memory list --namespace ndp-patterns
claude-flow memory export backup.json --namespace ndp-patterns
claude-flow memory import backup.json --namespace ndp-patterns
claude-flow memory clear --namespace ndp-patterns
```

---

## Appendix A: GitHub Repositories

- **AgentDB**: https://github.com/ruvnet/agentic-flow/tree/main/packages/agentdb
- **RuVector**: https://github.com/ruvnet/ruvector
- **Claude Flow**: https://github.com/ruvnet/claude-flow

## Appendix B: Existing Skills

| Skill | Purpose |
|-------|---------|
| `agentdb-learning` | RL algorithm integration |
| `agentdb-memory-patterns` | Memory pattern implementation |
| `agentdb-vector-search` | Semantic search |
| `agentdb-optimization` | Performance tuning |
| `agentdb-advanced` | QUIC sync, hybrid search |
| `reasoningbank-agentdb` | Legacy migration, adaptive learning |
| `reasoningbank-intelligence` | Pattern recognition, strategy optimization |
| `get-pattern` | Retrieve project patterns |
| `save-pattern` | Store project patterns |
