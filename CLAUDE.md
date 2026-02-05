# Claude Code Configuration - Claude Flow V3

## 🚨 AUTOMATIC SWARM ORCHESTRATION

**When starting work on complex tasks, Claude Code MUST automatically:**

1. **Initialize the swarm** using CLI tools via Bash
2. **Spawn concurrent agents** using Claude Code's Task tool
3. **Coordinate via hooks** and memory

### 🚨 CRITICAL: CLI + Task Tool in SAME Message

**When user says "spawn swarm" or requests complex work, Claude Code MUST in ONE message:**
1. Call CLI tools via Bash to initialize coordination
2. **IMMEDIATELY** call Task tool to spawn REAL working agents
3. Both CLI and Task calls must be in the SAME response

**CLI coordinates, Task tool agents do the actual work!**

### 🤖 INTELLIGENT 3-TIER MODEL ROUTING (ADR-026)

**The routing system has 3 tiers for optimal cost/performance:**

| Tier | Handler | Latency | Cost | Use Cases |
|------|---------|---------|------|-----------|
| **1** | Agent Booster | <1ms | $0 | Simple transforms (var→const, add-types, remove-console) |
| **2** | Haiku | ~500ms | $0.0002 | Simple tasks, bug fixes, low complexity |
| **3** | Sonnet/Opus | 2-5s | $0.003-$0.015 | Architecture, security, complex reasoning |

**Before spawning agents, get routing recommendation:**
```bash
claude-flow hooks pre-task --description "[task description]"
```

**When you see these recommendations:**

1. `[AGENT_BOOSTER_AVAILABLE]` → Skip LLM entirely, use Edit tool directly
   - Intent types: `var-to-const`, `add-types`, `add-error-handling`, `async-await`, `add-logging`, `remove-console`

2. `[TASK_MODEL_RECOMMENDATION] Use model="X"` → Use that model in Task tool:
```javascript
Task({
  prompt: "...",
  subagent_type: "coder",
  model: "haiku"  // ← USE THE RECOMMENDED MODEL (haiku/sonnet/opus)
})
```

**Benefits:** 75% cost reduction, 352x faster for Tier 1 tasks

---

### 🛡️ Anti-Drift Config (PREFERRED)

**Use this to prevent agent drift:**
```bash
# Small teams (6-8 agents) - use hierarchical for tight control
claude-flow swarm init --topology hierarchical --max-agents 8 --strategy specialized

# Large teams (10-15 agents) - use hierarchical-mesh for V3 queen + peer communication
claude-flow swarm init --topology hierarchical-mesh --max-agents 15 --strategy specialized
```

**Valid Topologies:**
- `hierarchical` - Queen controls workers directly (anti-drift for small teams)
- `hierarchical-mesh` - V3 queen + peer communication (recommended for 10+ agents)
- `mesh` - Fully connected peer network
- `ring` - Circular communication pattern
- `star` - Central coordinator with spokes
- `hybrid` - Dynamic topology switching

**Anti-Drift Guidelines:**
- **hierarchical**: Coordinator catches divergence
- **max-agents 6-8**: Smaller team = less drift
- **specialized**: Clear roles, no overlap
- **consensus**: raft (leader maintains state)

---

### 🔄 Auto-Start Swarm Protocol (Background Execution)

When the user requests a complex task, **spawn agents in background and WAIT for completion:**

```javascript
// STEP 1: Initialize swarm coordination (anti-drift config)
// Run via Bash tool:
// claude-flow swarm init --topology hierarchical --max-agents 8 --strategy specialized

// STEP 2: Spawn ALL agents IN BACKGROUND in a SINGLE message
// Use run_in_background: true so agents work concurrently
Task({
  prompt: "Research requirements, analyze codebase patterns, store findings in memory",
  subagent_type: "researcher",
  description: "Research phase",
  run_in_background: true  // ← CRITICAL: Run in background
})
Task({
  prompt: "Design architecture based on research. Document decisions.",
  subagent_type: "system-architect",
  description: "Architecture phase",
  run_in_background: true
})
Task({
  prompt: "Implement the solution following the design. Write clean code.",
  subagent_type: "coder",
  description: "Implementation phase",
  run_in_background: true
})
Task({
  prompt: "Write comprehensive tests for the implementation.",
  subagent_type: "tester",
  description: "Testing phase",
  run_in_background: true
})
Task({
  prompt: "Review code quality, security, and best practices.",
  subagent_type: "reviewer",
  description: "Review phase",
  run_in_background: true
})

// STEP 3: WAIT - Tell user agents are working, then STOP
// Say: "I've spawned 5 agents to work on this in parallel. They'll report back when done."
// DO NOT check status repeatedly. Just wait for user or agent responses.
```

### ⏸️ CRITICAL: Spawn and Wait Pattern

**After spawning background agents:**

1. **TELL USER** - "I've spawned X agents working in parallel on: [list tasks]"
2. **STOP** - Do not continue with more tool calls
3. **WAIT** - Let the background agents complete their work
4. **RESPOND** - When agents return results, review and synthesize

**Example response after spawning:**
```
I've launched 5 concurrent agents to work on this:
- 🔍 Researcher: Analyzing requirements and codebase
- 🏗️ Architect: Designing the implementation approach
- 💻 Coder: Implementing the solution
- 🧪 Tester: Writing tests
- 👀 Reviewer: Code review and security check

They're working in parallel. I'll synthesize their results when they complete.
```

### 🚫 DO NOT:
- Continuously check swarm status
- Poll TaskOutput repeatedly
- Add more tool calls after spawning
- Ask "should I check on the agents?"

### ✅ DO:
- Spawn all agents in ONE message
- Tell user what's happening
- Wait for agent results to arrive
- Synthesize results when they return

## 🧠 AUTO-LEARNING PROTOCOL
Your #1 job is to help human advance the system capabilities.
Your #2 job is to help future team members how to operate on this platform successfully.

### Before Starting Any Task
```bash
# 1. Use get-pattern skill relevant architectural and procedural guidance
# 2. Load learned optimizations
```

### After Completing Any Task Successfully
```bash
# 1. Record pattern feedback via reflexion skill (REQUIRED)
# 2. Store successful pattern for future reference
# 3. If you discovered something NEW, use /save-pattern skill
```

### Two Memory Systems

| System | Purpose | Persistence | Use For |
|--------|---------|-------------|---------|
| **get-pattern/save-pattern/reflexion Skills** | Application knowledge | Permanent | Patterns, procedures, architecture |
| **Claude-Flow Memory** | Swarm/session state | Transient | Coordination, task progress, working memory |

### AgentDB Skills (Persistent Patterns)

**Use these skills for permanent, reusable project knowledge:**

| Skill | When | What |
|-------|------|------|
| `get-pattern` | BEFORE work | Search existing patterns and approaches |
| `save-pattern` | AFTER discoveries | Store NEW reusable knowledge |
| `reflexion` | AFTER work | Record if patterns helped (required) |
| `learner` | Post-feature | Auto-discover patterns from episodes |

**Patterns capture:**
- Architecture decisions and ADRs
- Implementation procedures ("how to add a stream")
- Naming conventions and code organization
- Troubleshooting guides and checklists

### Claude-Flow Memory (Transient State)

**Use claude-flow memory for swarm coordination and session state:**
```bash
claude-flow memory store "<key>" "<value>" --namespace <ns>
claude-flow memory query "<pattern>" --namespace <ns>
```

**Transient memory is for:**
- Swarm coordination state
- Agent task progress
- Session-specific working memory
- Inter-agent communication

### The Pattern Workflow

```
BEFORE work:  /get-pattern  → Research existing project approaches
DURING work:  Apply patterns, note gaps or new discoveries
AFTER work:   /reflexion    → Rate if get-pattern results helped (required)
              /save-pattern → Store NEW reusable knowledge (if any)
              /learner      → Auto-discover patterns (periodic)
```

### Why This Matters

1. **Consistency** - Patterns ensure all agents follow established project conventions
2. **Learning** - Feedback via reflexion improves future pattern recommendations
3. **Knowledge Capture** - New discoveries become available to future sessions
4. **Project Memory** - The codebase evolves; patterns document the "why" and "how"

**See skill files in `.claude/skills/` for full documentation.**


### Continuous Improvement Triggers

| Trigger | Worker | When to Use |
|---------|--------|-------------|
| After major refactor | `optimize` | Performance optimization |
| After adding features | `testgaps` | Find missing test coverage |
| After security changes | `audit` | Security analysis |
| After API changes | `document` | Update documentation |
| Every 5+ file changes | `map` | Update codebase map |
| Complex debugging | `deepdive` | Deep code analysis |

### Memory-Enhanced Development

**ALWAYS check memory before: (get-pattern skill)**
- Starting a new feature (search for similar implementations)
- Debugging an issue (search for past solutions)
- Refactoring code (search for learned patterns)
- Performance work (search for optimization strategies)

**ALWAYS store in memory after: (save-pattern skill)**
- Solving a tricky bug (store the solution pattern)
- Completing a feature (store the approach)
- Finding a performance fix (store the optimization)
- Discovering a security issue (store the vulnerability pattern)

### 📋 Agent Routing (Anti-Drift)

| Code | Task | Agents |
|------|------|--------|
| 1 | Bug Fix | coordinator, researcher, coder, tester |
| 3 | Feature | ndp-scrum-master, ndp-architect, ndp-coder, ndp-tester, reviewer |
| 5 | Refactor | coordinator, ndp-architect, ndp-coder, reviewer |
| 7 | Performance | coordinator, perf-engineer, coder |
| 9 | Security | coordinator, security-architect, auditor |
| 11 | Docs | researcher, api-docs |

| Initiative Type | Core Team | Domain Specialists | When to Use |
|-----------------|-----------|-------------------|-------------|
| **Schema/ETL Work** | `ndp-architect`, `ndp-timescale-dev`, `ndp-dq-engineer` | `ndp-meteorologist` or `ndp-air-quality-specialist` | Silver layer design, Bronze→Silver ETL |
| **Analytics/Dashboards** | `ndp-analytics-engineer`, `ndp-grafana-dev` | Domain specialist for metrics | Forecast accuracy views, AQI dashboards |
| **New Data Source** | `ndp-architect`, `ndp-rust-dev`, `ndp-parquet-dev` | Domain specialist for validation | Adding new streams to Bronze |
| **ML/Predictions** | `ndp-feature-engineer`, `ndp-ml-engineer` | Domain specialist for feature logic | Feature engineering, model training |
| **Alerts/Triggers** | `ndp-alert-engineer`, `ndp-rust-dev` | `ndp-air-quality-specialist` for thresholds | Health-based alerting |
| **Research/Exploration** | Domain specialists + `ndp-analytics-engineer` | Primary focus | Domain modeling, DuckDB exploration |

**Team Formation Rules:**
1. **Always include domain specialist** when working with data domain areas
2. **Always include `ndp-dq-engineer`** when schema or ETL changes affect data quality
3. **Always include `ndp-architect`** for cross-cutting or schema changes
4. **Consult domain specialists first** before implementing domain logic in code



**Codes 1-9: hierarchical/specialized (anti-drift). Code 11: mesh/balanced**

### 🎯 Task Complexity Detection

**AUTO-INVOKE SWARM when task involves:**
- Multiple files (3+)
- New feature implementation
- Refactoring across modules
- API changes with tests
- Security-related changes
- Performance optimization
- Database schema changes

**SKIP SWARM for:**
- Single file edits
- Simple bug fixes (1-2 lines)
- Documentation updates
- Configuration changes
- Quick questions/exploration

## 🚨 CRITICAL: CONCURRENT EXECUTION & FILE MANAGEMENT

**ABSOLUTE RULES**:
1. ALL operations MUST be concurrent/parallel in a single message
2. **NEVER save working files, text/mds and tests to the root folder**
3. ALWAYS organize files in appropriate subdirectories
4. **USE CLAUDE CODE'S TASK TOOL** for spawning agents concurrently, not just MCP

### ⚡ GOLDEN RULE: "1 MESSAGE = ALL RELATED OPERATIONS"

**MANDATORY PATTERNS:**
- **TodoWrite**: ALWAYS batch ALL todos in ONE call (5-10+ todos minimum)
- **Task tool (Claude Code)**: ALWAYS spawn ALL agents in ONE message with full instructions
- **File operations**: ALWAYS batch ALL reads/writes/edits in ONE message
- **Bash commands**: ALWAYS batch ALL terminal operations in ONE message
- **Memory operations**: ALWAYS batch ALL memory store/retrieve in ONE message

### 📁 File Organization Rules

**NEVER save to root folder. Use these directories:**
- `/src` - Source code files
- `/tests` - Test files
- `/docs` - Documentation and markdown files
- `/config` - Configuration files
- `/scripts` - Utility scripts
- `/examples` - Example code
- `/product/features/{feature name}` - feature specifications
- `/product/research` - research and analysis

### Feature Naming Convention

Features follow `{phase}-{NNN}` pattern in `product/features/`:

| Phase | Prefix | Focus |
|-------|--------|-------|
| Air Quality | `air` | Foundation, sensors, external data (COMPLETE) |
| Data Platform | `dp` | Silver layer, TimescaleDB, ETL |
| Feature Engineering | `fe` | ML features, aggregations |
| Dashboards | `db` | Grafana, visualization |
| Predictions | `ml` | ruv-FANN, forecasting |
| Alerts | `al` | Triggers, notifications |

### Feature Directory Structure

```
product/features/{phase}-{NNN}/
├── SCOPE.md                    # Initial scope (human writes)
├── STATUS.md                   # Live status (agent maintains)
├── specification/              # SPARC S
├── pseudocode/                 # SPARC P
├── architecture/               # SPARC A
├── refinement/                 # SPARC R
├── completion/                 # SPARC C
├── bugs/                       # BUG-{NNN}-{slug}.md
└── reports/                    # Swarm/coordination reports
```

### SPARC Workflow Phases

1. **Specification** - Requirements analysis, acceptance criteria
2. **Pseudocode** - Algorithm design
3. **Architecture** - System design, ADRs
4. **Refinement** - TDD implementation
5. **Completion** - Integration, deployment verification

## Project Config (Anti-Drift Defaults)

- **Topology**: hierarchical (prevents drift)
- **Max Agents**: 8 (smaller = less drift)
- **Strategy**: specialized (clear roles)
- **Consensus**: raft
- **Memory**: hybrid
- **HNSW**: Enabled
- **Neural**: Enabled

---

## 📦 NDP Project Context

This CLAUDE.md is for the **Neural Data Platform** - a time-series data platform for Raspberry Pi.

### Architecture Overview
- **Product Vision**: Configuration driven generic data platform that leverages declarative deployment and neural capabilities to self learn causal relationships enabling predictive actions to be triggered that can operate on the edge.  (In development phase).  

- **Data Lake**: Bronze → Silver → Gold architecture
- **Bronze Layer**: Parquet files with WAL (Write-Ahead Log)
- **Silver Layer**: TimescaleDB with hypertables and continuous aggregates
- **Gold Layer**: /*In development*/
- **Hexagonal Architecture**: Domain adapters with Source/Sink traits

### NDP-Specific Agents (USE THESE)

| Instead of | Use | Why |
|------------|-----|-----|
| `coder` | `ndp-rust-dev` | Knows Rust patterns, project structure |
| `system-architect` | `ndp-architect` | Knows Domain Adapter pattern, ADRs |
| `tester` | `ndp-tester` | Knows test patterns, mocking approach |
| `planner` | `ndp-scrum-master` | Knows feature lifecycle, SPARC phases |

### Additional NDP Agents

| Agent | Scope | When to Use |
|-------|-------|-------------|
| `ndp-parquet-dev` | Narrow | Bronze layer, Parquet storage, WAL |
| `ndp-timescale-dev` | Narrow | Silver layer, TimescaleDB, continuous aggregates |
| `ndp-dq-engineer` | Specialized | Layered DQ strategy, transparency tables |
| `ndp-analytics-engineer` | Specialized | Silver→Gold transforms, domain logic |
| `ndp-feature-engineer` | Narrow | Time-series features, windowing |
| `ndp-ml-engineer` | Narrow | ruv-FANN models, training, inference |
| `ndp-grafana-dev` | Narrow | Grafana dashboards, panels |
| `ndp-meteorologist` | Specialized | NWS data, forecast evaluation, weather domain |
| `ndp-air-quality-specialist` | Specialized | AQI calculations, EPA standards |

### Deprecated Approaches (DO NOT USE)

- **DuckDB as ETL engine** - Use tokio-postgres/TimescaleDB directly
- **Polars with streaming** - Use TimescaleDB continuous aggregates
- **DuckDB for Gold layer** - Architecture eliminated DuckDB entirely

### NDP Project Structure

```
/core                    - Rust library (neural-core)
/apps                    - Application binaries (air-quality-app)
/config                  - Stream configurations (base/streams/)
/config-client           - etcd configuration client
/deploy                  - Docker and Pi deployment
/docs                    - Architecture and procedures
/product/features/       - SPARC documentation per feature
/.claude/agents/ndp      - NDP agent definitions
/.claude/skills          - Project skills
```

---

## 📦 Release Methodology (REQUIRED)

All releases MUST follow this methodology. See `docs/procedures/RELEASE-POLICY.md` for full details.

### Semantic Versioning

NDP uses **Semantic Versioning 2.0.0**:

```
MAJOR.MINOR.PATCH

MAJOR - Breaking changes (schema migration, API breaking change)
MINOR - New features (new stream, new Silver table, new MCP tool)
PATCH - Bug fixes (config corrections, DQ rule adjustments)
```

### Release Artifacts (3 Required)

Every release consists of:

| Artifact | Location | Description |
|----------|----------|-------------|
| **Manifest** | `.deploy/releases/vX.Y.Z.manifest.json` | Declares what changed |
| **Git Tag** | `vX.Y.Z` (annotated) | Version control marker |
| **Changelog** | `CHANGELOG.md` | Human-readable description |

### Quick Release Workflow

NDP has created custom deployment approach, declarative in nature.  **YOU MUST FOLLOW [RELEASE POLICY](docs/procedures/RELEASE-POLICY.md)**

### Key Documentation

| Document | Purpose |
|----------|---------|
| `docs/procedures/RELEASE-POLICY.md` | Full versioning standard and release workflow |
| `docs/procedures/DEPLOYMENT-DECLARATIVES.md` | Manifest format and all declaration types |
| `deploy/pi/README.md` | Pi deployment commands |
| `.deploy/releases/TEMPLATE.manifest.json` | Template for new releases |

### Version Bump Decision Guide

| Change | Bump | Example |
|--------|------|---------|
| New stream | MINOR | 1.0.0 → 1.1.0 |
| New Silver table | MINOR | 1.1.0 → 1.2.0 |
| Bug fix in config | PATCH | 1.2.0 → 1.2.1 |
| Remove deprecated field | MAJOR | 1.2.1 → 2.0.0 |
| API breaking change | MAJOR | 2.0.0 → 3.0.0 |

### Current Version

**v1.0.0** - Initial stable release with declarative deployment (2026-02-02)

---

## 🚀 V3 CLI Commands (26 Commands, 140+ Subcommands)

### Core Commands

| Command | Subcommands | Description |
|---------|-------------|-------------|
| `init` | 4 | Project initialization with wizard, presets, skills, hooks |
| `agent` | 8 | Agent lifecycle (spawn, list, status, stop, metrics, pool, health, logs) |
| `swarm` | 6 | Multi-agent swarm coordination and orchestration |
| `memory` | 11 | AgentDB memory with vector search (150x-12,500x faster) |
| `mcp` | 9 | MCP server management and tool execution |
| `task` | 6 | Task creation, assignment, and lifecycle |
| `session` | 7 | Session state management and persistence |
| `config` | 7 | Configuration management and provider setup |
| `status` | 3 | System status monitoring with watch mode |
| `workflow` | 6 | Workflow execution and template management |
| `hooks` | 17 | Self-learning hooks + 12 background workers |
| `hive-mind` | 6 | Queen-led Byzantine fault-tolerant consensus |

### Advanced Commands

| Command | Subcommands | Description |
|---------|-------------|-------------|
| `daemon` | 5 | Background worker daemon (start, stop, status, trigger, enable) |
| `neural` | 5 | Neural pattern training (train, status, patterns, predict, optimize) |
| `security` | 6 | Security scanning (scan, audit, cve, threats, validate, report) |
| `performance` | 5 | Performance profiling (benchmark, profile, metrics, optimize, report) |
| `providers` | 5 | AI providers (list, add, remove, test, configure) |
| `plugins` | 5 | Plugin management (list, install, uninstall, enable, disable) |
| `deployment` | 5 | Deployment management (deploy, rollback, status, environments, release) |
| `embeddings` | 4 | Vector embeddings (embed, batch, search, init) - 75x faster with agentic-flow |
| `claims` | 4 | Claims-based authorization (check, grant, revoke, list) |
| `migrate` | 5 | V2 to V3 migration with rollback support |
| `doctor` | 1 | System diagnostics with health checks |
| `completions` | 4 | Shell completions (bash, zsh, fish, powershell) |

### Quick CLI Examples

```bash
# Initialize project
claude-flow init --wizard

# Start daemon with background workers
claude-flow daemon start

# Spawn an agent
claude-flow agent spawn -t coder --name my-coder

# Initialize swarm
claude-flow swarm init --v3-mode

# Search memory (HNSW-indexed)
claude-flow memory search --query "authentication patterns"

# System diagnostics
claude-flow doctor --fix

# Security scan
claude-flow security scan --depth full

# Performance benchmark
claude-flow performance benchmark --suite all
```

## 🚀 Available Agents (60+ Types)

### Core Development
`coder`, `reviewer`, `tester`, `planner`, `researcher`

### V3 Specialized Agents
`security-architect`, `security-auditor`, `memory-specialist`, `performance-engineer`

### 🔐 @claude-flow/security
CVE remediation, input validation, path security:
- `InputValidator` - Zod validation
- `PathValidator` - Traversal prevention
- `SafeExecutor` - Injection protection

### Swarm Coordination
`hierarchical-coordinator`, `mesh-coordinator`, `adaptive-coordinator`, `collective-intelligence-coordinator`, `swarm-memory-manager`

### Consensus & Distributed
`byzantine-coordinator`, `raft-manager`, `gossip-coordinator`, `consensus-builder`, `crdt-synchronizer`, `quorum-manager`, `security-manager`

### Performance & Optimization
`perf-analyzer`, `performance-benchmarker`, `task-orchestrator`, `memory-coordinator`, `smart-agent`

### GitHub & Repository
`github-modes`, `pr-manager`, `code-review-swarm`, `issue-tracker`, `release-manager`, `workflow-automation`, `project-board-sync`, `repo-architect`, `multi-repo-swarm`

### SPARC Methodology
`sparc-coord`, `sparc-coder`, `specification`, `pseudocode`, `architecture`, `refinement`

### Specialized Development
`backend-dev`, `mobile-dev`, `ml-developer`, `cicd-engineer`, `api-docs`, `system-architect`, `code-analyzer`, `base-template-generator`

### Testing & Validation
`tdd-london-swarm`, `production-validator`

### NDP Agent Roster

| Agent | Scope | When to Use |
|-------|-------|-------------|
| `ndp-scrum-master` | Broad | Feature lifecycle, SPARC phases, STATUS.md, GitHub workflow |
| `ndp-architect` | Broad | Architecture decisions, ADRs, cross-cutting concerns |
| `ndp-rust-dev` | General | Any Rust development following NDP patterns |
| `ndp-tester` | Specialized | Testing strategy, integration tests, mocking |
| `ndp-meteorologist` | Specialized | NWS data interpretation, forecast evaluation, weather domain |
| `ndp-air-quality-specialist` | Specialized | AQI calculations, sensor calibration, EPA standards |
| `ndp-parquet-dev` | Narrow | Bronze layer, Parquet storage, WAL |
| `ndp-timescale-dev` | Narrow | Silver layer, TimescaleDB, continuous aggregates |
| `ndp-dq-engineer` | Specialized | Layered DQ strategy, transparency tables, quality monitoring |
| `ndp-analytics-engineer` | Specialized | Silver→Gold transforms, domain logic in SQL |
| `ndp-feature-engineer` | Narrow | Time-series features, windowing, aggregations |
| `ndp-ml-engineer` | Narrow | ruv-FANN models, training, inference |
| `ndp-grafana-dev` | Narrow | Grafana dashboards, panels, data sources |
| `ndp-alert-engineer` | Narrow | Rust-based triggers, thresholds, notifications |

See: `.claude/agents/ndp/README.md` for full documentation.


## 🪝 V3 Hooks System (27 Hooks + 12 Workers)

### All Available Hooks

| Hook | Description | Key Options |
|------|-------------|-------------|
| `pre-edit` | Get context before editing files | `--file`, `--operation` |
| `post-edit` | Record editing outcome for learning | `--file`, `--success`, `--train-neural` |
| `pre-command` | Assess risk before commands | `--command`, `--validate-safety` |
| `post-command` | Record command execution outcome | `--command`, `--track-metrics` |
| `pre-task` | Record task start, get agent suggestions | `--description`, `--coordinate-swarm` |
| `post-task` | Record task completion for learning | `--task-id`, `--success`, `--store-results` |
| `session-start` | Start/restore session (v2 compat) | `--session-id`, `--auto-configure` |
| `session-end` | End session and persist state | `--generate-summary`, `--export-metrics` |
| `session-restore` | Restore a previous session | `--session-id`, `--latest` |
| `route` | Route task to optimal agent | `--task`, `--context`, `--top-k` |
| `route-task` | (v2 compat) Alias for route | `--task`, `--auto-swarm` |
| `explain` | Explain routing decision | `--topic`, `--detailed` |
| `pretrain` | Bootstrap intelligence from repo | `--model-type`, `--epochs` |
| `build-agents` | Generate optimized agent configs | `--agent-types`, `--focus` |
| `metrics` | View learning metrics dashboard | `--v3-dashboard`, `--format` |
| `transfer` | Transfer patterns via IPFS registry | `store`, `from-project` |
| `list` | List all registered hooks | `--format` |
| `intelligence` | RuVector intelligence system | `trajectory-*`, `pattern-*`, `stats` |
| `worker` | Background worker management | `list`, `dispatch`, `status`, `detect` |
| `progress` | Check V3 implementation progress | `--detailed`, `--format` |
| `statusline` | Generate dynamic statusline | `--json`, `--compact`, `--no-color` |
| `coverage-route` | Route based on test coverage gaps | `--task`, `--path` |
| `coverage-suggest` | Suggest coverage improvements | `--path` |
| `coverage-gaps` | List coverage gaps with priorities | `--format`, `--limit` |
| `pre-bash` | (v2 compat) Alias for pre-command | Same as pre-command |
| `post-bash` | (v2 compat) Alias for post-command | Same as post-command |

### 12 Background Workers

| Worker | Priority | Description |
|--------|----------|-------------|
| `ultralearn` | normal | Deep knowledge acquisition |
| `optimize` | high | Performance optimization |
| `consolidate` | low | Memory consolidation |
| `predict` | normal | Predictive preloading |
| `audit` | critical | Security analysis |
| `map` | normal | Codebase mapping |
| `preload` | low | Resource preloading |
| `deepdive` | normal | Deep code analysis |
| `document` | normal | Auto-documentation |
| `refactor` | normal | Refactoring suggestions |
| `benchmark` | normal | Performance benchmarking |
| `testgaps` | normal | Test coverage analysis |

### Essential Hook Commands

```bash
# Core hooks
claude-flow hooks pre-task --description "[task]"
claude-flow hooks post-task --task-id "[id]" --success true
claude-flow hooks post-edit --file "[file]" --train-neural true

# Session management
claude-flow hooks session-start --session-id "[id]"
claude-flow hooks session-end --export-metrics true
claude-flow hooks session-restore --session-id "[id]"

# Intelligence routing
claude-flow hooks route --task "[task]"
claude-flow hooks explain --topic "[topic]"

# Neural learning
claude-flow hooks pretrain --model-type moe --epochs 10
claude-flow hooks build-agents --agent-types coder,tester

# Background workers
claude-flow hooks worker list
claude-flow hooks worker dispatch --trigger audit
claude-flow hooks worker status

# Coverage-aware routing
claude-flow hooks coverage-gaps --format table
claude-flow hooks coverage-route --task "[task]"

# Statusline (for Claude Code integration)
claude-flow hooks statusline
claude-flow hooks statusline --json
```

## 🧠 Intelligence System (RuVector)

V3 includes the RuVector Intelligence System:
- **SONA**: Self-Optimizing Neural Architecture (<0.05ms adaptation)
- **MoE**: Mixture of Experts for specialized routing
- **HNSW**: 150x-12,500x faster pattern search
- **EWC++**: Elastic Weight Consolidation (prevents forgetting)
- **Flash Attention**: 2.49x-7.47x speedup

The 4-step intelligence pipeline:
1. **RETRIEVE** - Fetch relevant patterns via HNSW
2. **JUDGE** - Evaluate with verdicts (success/failure)
3. **DISTILL** - Extract key learnings via LoRA
4. **CONSOLIDATE** - Prevent catastrophic forgetting via EWC++

## 📦 Embeddings Package (v3.0.0-alpha.12)

Features:
- **sql.js**: Cross-platform SQLite persistent cache (WASM, no native compilation)
- **Document chunking**: Configurable overlap and size
- **Normalization**: L2, L1, min-max, z-score
- **Hyperbolic embeddings**: Poincaré ball model for hierarchical data
- **75x faster**: With agentic-flow ONNX integration
- **Neural substrate**: Integration with RuVector

## 🐝 Hive-Mind Consensus

### Topologies
- `hierarchical` - Queen controls workers directly
- `mesh` - Fully connected peer network
- `hierarchical-mesh` - Hybrid (recommended)
- `adaptive` - Dynamic based on load

### Consensus Strategies
- `byzantine` - BFT (tolerates f < n/3 faulty)
- `raft` - Leader-based (tolerates f < n/2)
- `gossip` - Epidemic for eventual consistency
- `crdt` - Conflict-free replicated data types
- `quorum` - Configurable quorum-based

## V3 Performance Targets

| Metric | Target |
|--------|--------|
| Flash Attention | 2.49x-7.47x speedup |
| HNSW Search | 150x-12,500x faster |
| Memory Reduction | 50-75% with quantization |
| MCP Response | <100ms |
| CLI Startup | <500ms |
| SONA Adaptation | <0.05ms |

## 📊 Performance Optimization Protocol

### Automatic Performance Tracking
```bash
# After any significant operation, track metrics
claude-flow hooks post-command --command '[operation]' --track-metrics true

# Periodically run benchmarks (every major feature)
claude-flow performance benchmark --suite all

# Analyze bottlenecks when performance degrades
claude-flow performance profile --target '[component]'
```

### Session Persistence (Cross-Conversation Learning)
```bash
# At session start - restore previous context
claude-flow session restore --latest

# At session end - persist learned patterns
claude-flow hooks session-end --generate-summary true --persist-state true --export-metrics true
```

### Neural Pattern Training
```bash
# Train on successful code patterns
claude-flow neural train --pattern-type coordination --epochs 10

# Predict optimal approach for new tasks
claude-flow neural predict --input '[task description]'

# View learned patterns
claude-flow neural patterns --list
```

## 🔧 Environment Variables

```bash
# Configuration
CLAUDE_FLOW_CONFIG=./claude-flow.config.json
CLAUDE_FLOW_LOG_LEVEL=info

# Provider API Keys
ANTHROPIC_API_KEY=sk-ant-...
OPENAI_API_KEY=sk-...
GOOGLE_API_KEY=...

# MCP Server
CLAUDE_FLOW_MCP_PORT=3000
CLAUDE_FLOW_MCP_HOST=localhost
CLAUDE_FLOW_MCP_TRANSPORT=stdio

# Memory
CLAUDE_FLOW_MEMORY_BACKEND=hybrid
CLAUDE_FLOW_MEMORY_PATH=./data/memory
```

## 🔍 Doctor Health Checks

Run `claude-flow doctor` to check:
- Node.js version (20+)
- npm version (9+)
- Git installation
- Config file validity
- Daemon status
- Memory database
- API keys
- MCP servers
- Disk space
- TypeScript installation

## 🚀 Quick Setup

```bash
# Add MCP servers (auto-detects MCP mode when stdin is piped)
claude mcp add claude-flow -- npx -y @claude-flow/cli@latest
claude mcp add ruv-swarm -- npx -y ruv-swarm mcp start  # Optional
claude mcp add flow-nexus -- npx -y flow-nexus@latest mcp start  # Optional

# Start daemon
claude-flow daemon start

# Run doctor
claude-flow doctor --fix
```

## 🎯 Claude Code vs CLI Tools

### Claude Code Handles ALL EXECUTION:
- **Task tool**: Spawn and run agents concurrently
- File operations (Read, Write, Edit, MultiEdit, Glob, Grep)
- Code generation and programming
- Bash commands and system operations
- TodoWrite and task management
- Git operations

### CLI Tools Handle Coordination (via Bash):
- **Swarm init**: `claude-flow swarm init --topology <type>`
- **Swarm status**: `claude-flow swarm status`
- **Agent spawn**: `claude-flow agent spawn -t <type> --name <name>`
- **Memory store**: `claude-flow memory store --key "mykey" --value "myvalue" --namespace patterns`
- **Memory search**: `claude-flow memory search --query "search terms"`
- **Memory list**: `claude-flow memory list --namespace patterns`
- **Memory retrieve**: `claude-flow memory retrieve --key "mykey" --namespace patterns`
- **Hooks**: `claude-flow hooks <hook-name> [options]`

## 📝 Memory Commands Reference (IMPORTANT)

### Store Data (ALL options shown)
```bash
# REQUIRED: --key and --value
# OPTIONAL: --namespace (default: "default"), --ttl, --tags
claude-flow memory store --key "pattern-auth" --value "JWT with refresh tokens" --namespace patterns
claude-flow memory store --key "bug-fix-123" --value "Fixed null check" --namespace solutions --tags "bugfix,auth"
```

### Search Data (semantic vector search)
```bash
# REQUIRED: --query (full flag, not -q)
# OPTIONAL: --namespace, --limit, --threshold
claude-flow memory search --query "authentication patterns"
claude-flow memory search --query "error handling" --namespace patterns --limit 5
```

### List Entries
```bash
# OPTIONAL: --namespace, --limit
claude-flow memory list
claude-flow memory list --namespace patterns --limit 10
```

### Retrieve Specific Entry
```bash
# REQUIRED: --key
# OPTIONAL: --namespace (default: "default")
claude-flow memory retrieve --key "pattern-auth"
claude-flow memory retrieve --key "pattern-auth" --namespace patterns
```

### Initialize Memory Database
```bash
claude-flow memory init --force --verbose
```

**KEY**: CLI coordinates the strategy via Bash, Claude Code's Task tool executes with real agents.

## 📚 Full Capabilities Reference

For a comprehensive overview of all Claude Flow V3 features, agents, commands, and integrations, see:

**`.claude-flow/CAPABILITIES.md`** - Complete reference generated during init

This includes:
- All 60+ agent types with routing recommendations
- All 26 CLI commands with 140+ subcommands
- All 27 hooks + 12 background workers
- RuVector intelligence system details
- Hive-Mind consensus mechanisms
- Integration ecosystem (agentic-flow, agentdb, ruv-swarm, flow-nexus, agentic-jujutsu)
- Performance targets and status

## Support

- Documentation: https://github.com/ruvnet/claude-flow
- Issues: https://github.com/ruvnet/claude-flow/issues

---

Remember: **Claude Flow CLI coordinates, Claude Code Task tool creates!**

# important-instruction-reminders
Do what has been asked; nothing more, nothing less.
NEVER create files unless they're absolutely necessary for achieving your goal.
ALWAYS prefer editing an existing file to creating a new one.
NEVER proactively create documentation files (*.md) or README files. Only create documentation files if explicitly requested by the User.
Never save working files, text/mds and tests to the root folder.
