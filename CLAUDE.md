# Claude Code Configuration - Neural Data Platform

## 🎯 NDP Project Team (USE THESE AGENTS)

**ALWAYS use NDP agents instead of generic agents for this project.**

| Instead of | Use | Why |
|------------|-----|-----|
| `coder` | `ndp-rust-dev` | Knows Rust patterns, project structure |
| `system-architect` | `ndp-architect` | Knows Domain Adapter pattern, ADRs |
| `tester` | `ndp-tester` | Knows test patterns, mocking approach |
| `planner` | `ndp-scrum-master` | Knows feature lifecycle, SPARC phases |

### NDP Agent Roster

| Agent | Scope | When to Use |
|-------|-------|-------------|
| `ndp-scrum-master` | Broad | Feature lifecycle, SPARC phases, STATUS.md, GitHub workflow |
| `ndp-architect` | Broad | Architecture decisions, ADRs, cross-cutting concerns |
| `ndp-rust-dev` | General | Any Rust development following NDP patterns |
| `ndp-tester` | Specialized | Testing strategy, integration tests, mocking |
| `ndp-parquet-dev` | Narrow | Bronze layer, Parquet storage, WAL |
| `ndp-timescale-dev` | Narrow | Silver layer, TimescaleDB, continuous aggregates |
| `ndp-feature-engineer` | Narrow | Time-series features, windowing, aggregations |
| `ndp-ml-engineer` | Narrow | ruv-FANN models, training, inference |
| `ndp-grafana-dev` | Narrow | Grafana dashboards, panels, data sources |
| `ndp-alert-engineer` | Narrow | Rust-based triggers, thresholds, notifications |

See: `.claude/agents/ndp/README.md` for full documentation.

---

## 🔧 Required Skills for NDP

**ALL agents MUST use these skills:**

| Skill | When | Purpose |
|-------|------|---------|
| `get-pattern` | Before implementation | Search for "How do I" questions |
| `save-pattern` | After discoveries | Answers "How Agents should do x" questions |
| `reflexion` | After using get-pattern | Evaluate if retrieved patterns helped |
| `ndp-github-workflow` | ALL git operations | Branch naming, commits, PRs |

---

## 🧠 Pattern Memory for Project Knowledge

**Pattern skills store APPLICATION knowledge - not swarm/transient state.**

### What Patterns Are For

This is intended to be a definitive source to maintain consistency of development across many different agents over developing many different features.
Patterns capture **permanent, reusable project knowledge**:
- Architecture decisions and ADRs
- Implementation procedures ("how to add a stream")
- Naming conventions and code organization
- Troubleshooting guides and checklists
- Data flow and pipeline designs

### What Patterns Are NOT For

Do NOT use pattern skills for:
- Swarm coordination state (use claude-flow memory tools)
- Agent task progress (use claude-flow task tools)
- Session-specific working memory (use MCP memory with TTL)
- Inter-agent communication (use claude-flow DAA tools)

### The Pattern Workflow

```
BEFORE work:  get-pattern   → Research existing project approaches
DURING work:  Apply patterns, note gaps or new discoveries
AFTER work:   reflexion     → Rate if get-pattern results helped
              save-pattern  → Store NEW reusable knowledge (if any)
```

### Why This Matters

1. **Consistency** - Patterns ensure all agents follow established project conventions
2. **Learning** - Feedback via reflexion improves future pattern recommendations
3. **Knowledge Capture** - New discoveries become available to future sessions
4. **Project Memory** - The codebase evolves; patterns document the "why" and "how"

**See skill files in `.claude/skills/` for usage details.**

---

## 📁 NDP Project Structure

**NEVER save to root folder. Use these directories:**

```
/core                    - Rust library (neural-core)
/apps                    - Application binaries (air-quality-app)
/config                  - Stream configurations (base/streams/)
/config-client           - etcd configuration client
/deploy                  - Docker and Pi deployment
/docs                    - Architecture and procedures
/product/features/{feature name}        - SPARC documentation per feature
/.claude/agents/ndp      - NDP agent definitions
/.claude/skills          - Project skills
```

---

## 🚀 Feature Development Workflow

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

---

## 🚨 CRITICAL: CONCURRENT EXECUTION & FILE MANAGEMENT

**ABSOLUTE RULES**:
1. ALL operations MUST be concurrent/parallel in a single message
2. **NEVER save working files, text/mds and tests to the root folder**
3. ALWAYS organize files in appropriate subdirectories
4. **USE CLAUDE CODE'S TASK TOOL** for spawning agents concurrently

### ⚡ GOLDEN RULE: "1 MESSAGE = ALL RELATED OPERATIONS"

**MANDATORY PATTERNS:**
- **TodoWrite**: ALWAYS batch ALL todos in ONE call (5-10+ todos minimum)
- **Task tool (Claude Code)**: ALWAYS spawn ALL agents in ONE message with full instructions
- **File operations**: ALWAYS batch ALL reads/writes/edits in ONE message
- **Bash commands**: ALWAYS batch ALL terminal operations in ONE message
- **Memory operations**: ALWAYS batch ALL memory store/retrieve in ONE message

---

## 🎯 When to Use Swarms vs Task Tool

### Use MCP Swarms When:
- Tasks need **shared memory/state** between agents
- You need **monitored, trackable progress**
- Work requires **coordinated handoffs** between agents
- You want **persistent task history** and results
- Complex multi-step workflows with dependencies

### Use Task Tool When:
- Tasks are **independent** (no shared state needed)
- Simple parallel execution is sufficient
- You don't need coordination tracking
- Quick one-off agent invocations

---

## 🔄 Swarm Execution Workflow (CRITICAL)

**When using swarms, follow this COMPLETE workflow:**

```
1. swarm_init        → Create swarm with topology
2. agent_spawn       → Add agents TO the swarm (NOT Task tool!)
3. task_orchestrate  → Assign work TO swarm agents
4. task_status       → Monitor progress
5. task_results      → Retrieve completed work
```

**⚠️ WRONG Pattern (causes empty swarms):**
```javascript
// ❌ WRONG: Creates swarm then ignores it
mcp__claude-flow__swarm_init({ topology: "hierarchical" })
Task("Do work", "...", "ndp-rust-dev")  // Work happens OUTSIDE swarm!
```

**✅ CORRECT Pattern (work happens IN swarm):**
```javascript
// ✅ RIGHT: Full swarm lifecycle
mcp__claude-flow__swarm_init({ topology: "hierarchical", maxAgents: 5 })

// Spawn agents INTO the swarm
mcp__claude-flow__agent_spawn({ type: "coordinator", name: "lead" })
mcp__claude-flow__agent_spawn({ type: "coder", name: "rust-dev" })
mcp__claude-flow__agent_spawn({ type: "analyst", name: "reviewer" })

// Orchestrate work TO the swarm agents
mcp__claude-flow__task_orchestrate({
  task: "Implement TimescaleDB ETL pipeline with tests",
  strategy: "adaptive",
  priority: "high"
})

// Monitor and retrieve results
mcp__claude-flow__task_status({ taskId: "..." })
mcp__claude-flow__task_results({ taskId: "...", format: "detailed" })
```

---

## 🎯 Swarm Agents vs Task Agents

| Aspect | MCP Swarm Agents | Task Tool Agents |
|--------|------------------|------------------|
| **Coordination** | Shared memory, tracked state | Independent, isolated |
| **Monitoring** | `swarm_status`, `task_status` | No built-in monitoring |
| **Results** | `task_results` retrieves | Returned directly in response |
| **Memory** | Persistent across tasks | Session only |
| **Use Case** | Complex coordinated workflows | Simple parallel tasks |

---

## 🛠️ Build Commands

### Rust/Cargo
- `cargo build` - Build project
- `cargo test` - Run tests
- `cargo clippy` - Linting
- `cargo fmt --check` - Format check
- `cargo doc --open` - Generate documentation

### Deployment
- `./deploy/pi/deploy.sh start` - Start services
- `./deploy/pi/deploy.sh stop` - Stop services
- `./deploy/pi/deploy.sh logs` - View logs
- `./deploy/pi/deploy.sh sync` - Sync config to etcd
- `./deploy/pi/deploy.sh status` - Check health

### SPARC Commands
- `npx claude-flow sparc modes` - List available modes
- `npx claude-flow sparc run <mode> "<task>"` - Execute specific mode
- `npx claude-flow sparc tdd "<feature>"` - Run complete TDD workflow

---

## 🎯 Claude Code vs MCP Tools

### Claude Code Direct Tools:
- File operations (Read, Write, Edit, Glob, Grep)
- Code generation and Rust implementation
- Bash commands and cargo operations
- Git operations (using `ndp-github-workflow` skill)
- Testing and debugging
- **Task tool**: For independent parallel agents (no coordination needed)

### MCP Swarm Tools (Full Execution Cycle):
- **Swarm lifecycle**: `swarm_init` → `agent_spawn` → `task_orchestrate` → `task_results`
- **Agents do actual work** when tasks are orchestrated to them
- Memory management and shared state across agents
- Performance tracking and monitoring

### AgentDB Tools (Pattern Memory):
- `npx agentdb query` - Retrieve patterns
- `npx agentdb store-pattern` - Save patterns
- `npx agentdb reflexion` - Record feedback

**KEY**: If you initialize a swarm, USE IT - orchestrate tasks to swarm agents, don't spawn separate Task agents.

---


## MCP Tool Categories

### Coordination
`swarm_init`, `agent_spawn`, `task_orchestrate`

### Monitoring
`swarm_status`, `agent_list`, `agent_metrics`, `task_status`, `task_results`

### Claude-flow Memory & Neural
`memory_usage`, `memory_search`, `neural_status`, `neural_train`, `neural_patterns`

### System
`benchmark_run`, `features_detect`, `swarm_monitor`

---

## 🚀 NDP Execution Examples

### Example 1: Swarm-Coordinated Development (Complex Workflows)

Use swarms when you need coordination, shared state, and tracked progress:

```javascript
// Step 1: Initialize swarm
mcp__claude-flow__swarm_init({ topology: "hierarchical", maxAgents: 6, strategy: "specialized" })

// Step 2: Spawn specialized agents INTO the swarm
mcp__claude-flow__agent_spawn({ type: "coordinator", name: "dp-lead" })
mcp__claude-flow__agent_spawn({ type: "coder", name: "timescale-dev" })
mcp__claude-flow__agent_spawn({ type: "coder", name: "parquet-dev" })
mcp__claude-flow__agent_spawn({ type: "analyst", name: "feature-eng" })
mcp__claude-flow__agent_spawn({ type: "tester", name: "qa" })

// Step 3: Orchestrate the work TO the swarm
mcp__claude-flow__task_orchestrate({
  task: "Implement Silver Layer: 1) Design TimescaleDB schema with hypertables, 2) Build Parquet ETL, 3) Create feature aggregations, 4) Write integration tests",
  strategy: "adaptive",
  priority: "high",
  maxAgents: 4
})

// Step 4: Monitor progress
mcp__claude-flow__swarm_status({})
mcp__claude-flow__task_status({ detailed: true })

// Step 5: Retrieve results when complete
mcp__claude-flow__task_results({ taskId: "...", format: "detailed" })
```

### Example 2: Task Tool for Independent Work (Simple Parallel)

Use Task tool when agents don't need to coordinate:

```javascript
// Independent parallel agents - no shared state needed
[Single Message]:
  Task("Review PR #42", "Check code quality and tests.", "ndp-tester")
  Task("Update docs", "Add API documentation for new endpoints.", "ndp-architect")
  Task("Fix linting", "Run cargo clippy and fix warnings.", "ndp-rust-dev")
```

### Example 3: New Feature Kickoff (Swarm + Branch)

```javascript
// Create feature branch first
Bash "git checkout -b feature/dp-001"

// Initialize coordinated swarm for feature
mcp__claude-flow__swarm_init({ topology: "hierarchical", maxAgents: 4 })
mcp__claude-flow__agent_spawn({ type: "coordinator", name: "scrum-master" })
mcp__claude-flow__agent_spawn({ type: "architect", name: "designer" })
mcp__claude-flow__agent_spawn({ type: "coder", name: "implementer" })

// Orchestrate SPARC workflow
mcp__claude-flow__task_orchestrate({
  task: "Initialize dp-001: Create feature directory, STATUS.md, specification docs, architecture ADRs",
  strategy: "sequential",
  priority: "high"
})
```

---

## 📋 Agent Protocols
---

## 🔀 Git Workflow (REQUIRED)

**ALL git operations MUST follow `ndp-github-workflow` skill:**

### Branch Naming
```
feature/{phase}-{NNN}           # feature/dp-001
feature/{phase}-{NNN}/bug-{nnn} # feature/dp-001/bug-001
```

### Commit Format
```
{type}({scope}): {description}

feat(dp-001): add timescaledb schema migrations
fix(dp-001): correct continuous aggregate refresh
docs(dp-001): update architecture documentation
```

### PR Template
```markdown
## Feature
{phase}-{NNN}: {title}

## Summary
{description}

## Checklist
- [ ] SPARC documentation updated
- [ ] Tests passing
- [ ] STATUS.md updated
```

---

## 📊 Key Project Patterns

NDP agents should know these patterns (use `get-pattern` for details):

| Pattern | Description |
|---------|-------------|
| `architecture:domain-adapter-pattern` | Hexagonal architecture with Source/Store traits |
| `architecture:data-layers` | Bronze (Parquet) → Silver (TimescaleDB) → Gold (Features) |
| `architecture:channel-ownership-adr-001` | IngestionCoordinator owns mpsc channel |
| `data-flow:ingestion-pipeline` | Source → Channel → Router → Storage |
| `deployment:docker-minimal-changes` | Extend Docker without restructuring |
| `conventions:naming` | Stream IDs (kebab-case), fields (snake_case) |

---

## Hooks Integration

### Pre-Operation
- Auto-assign agents by file type
- Validate commands for safety
- Prepare resources automatically
- Optimize topology by complexity

### Post-Operation
- Auto-format code
- Train neural patterns
- Update memory
- Analyze performance

### Session Management
- Generate summaries
- Persist state
- Track metrics
- Restore context

---

## Code Style & Best Practices

- **Modular Design**: Files under 500 lines
- **Environment Safety**: Never hardcode secrets (use .env)
- **Test-First**: Write tests before implementation
- **Clean Architecture**: Domain Adapter pattern
- **Documentation**: Update SPARC docs with implementation
- **Patterns**: Use `get-pattern` before implementing, `save-pattern` after discovering

---

## Support

- Claude-Flow Documentation: https://github.com/ruvnet/claude-flow
- Claude-Flow Issues: https://github.com/ruvnet/claude-flow/issues

---

Remember: **Use NDP agents, check patterns first, follow the workflow!**

# important-instruction-reminders
Do what has been asked; nothing more, nothing less.
NEVER create files unless they're absolutely necessary for achieving your goal.
ALWAYS prefer editing an existing file to creating a new one.
NEVER proactively create documentation files (*.md) or README files. Only create documentation files if explicitly requested by the User.
Never save working files, text/mds and tests to the root folder.
