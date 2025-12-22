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
| `get-pattern` | Before implementation | Retrieve project patterns from memory |
| `save-pattern` | After discoveries | Store new reusable patterns |
| `reflexion` | After using get-pattern | Evaluate if retrieved patterns helped |
| `ndp-github-workflow` | ALL git operations | Branch naming, commits, PRs |

---

## 🧠 Pattern Memory for Project Knowledge

**Pattern skills store APPLICATION knowledge - not swarm/transient state.**

### What Patterns Are For

Patterns capture **permanent, reusable project knowledge**:
- Architecture decisions and ADRs
- Implementation procedures ("how to add a stream")
- Naming conventions and code organization
- Troubleshooting guides and checklists
- Data flow and pipeline designs

### What Patterns Are NOT For

Do NOT use pattern skills for:
- Swarm coordination state (use MCP memory tools)
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
/product/features        - SPARC documentation per feature
/.claude/agents/ndp      - NDP agent definitions
/.claude/skills          - Project skills
/.claude/patterns        - Pattern index
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

## 🎯 Claude Code Task Tool for Agent Execution

**Claude Code's Task tool is the PRIMARY way to spawn agents:**

```javascript
// ✅ CORRECT: Use NDP agents via Task tool for parallel execution
[Single Message]:
  Task("Design Silver layer schema", "Create TimescaleDB hypertables...", "ndp-timescale-dev")
  Task("Build Parquet ETL", "Implement Bronze to Silver ETL...", "ndp-parquet-dev")
  Task("Create dashboard queries", "Design Grafana panel queries...", "ndp-grafana-dev")
  Task("Write integration tests", "Test full pipeline flow...", "ndp-tester")
```

**MCP tools are ONLY for coordination setup:**
- `mcp__claude-flow__swarm_init` - Initialize coordination topology
- `mcp__claude-flow__agent_spawn` - Define agent types for coordination
- `mcp__claude-flow__task_orchestrate` - Orchestrate high-level workflows

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

### Claude Code Handles ALL EXECUTION:
- **Task tool**: Spawn NDP agents concurrently for actual work
- File operations (Read, Write, Edit, Glob, Grep)
- Code generation and Rust implementation
- Bash commands and cargo operations
- Git operations (using `ndp-github-workflow` skill)
- Testing and debugging

### MCP Tools ONLY COORDINATE:
- Swarm initialization (topology setup)
- Agent type definitions (coordination patterns)
- Task orchestration (high-level planning)
- Memory management (pattern storage/retrieval)
- Performance tracking

**KEY**: MCP coordinates the strategy, Claude Code's Task tool executes with NDP agents.

---

## 🚀 Quick Setup

```bash
# Add MCP server (required)
claude mcp add claude-flow claude-flow mcp start
```

## MCP Tool Categories

### Coordination
`swarm_init`, `agent_spawn`, `task_orchestrate`

### Monitoring
`swarm_status`, `agent_list`, `agent_metrics`, `task_status`, `task_results`

### Memory & Neural
`memory_usage`, `memory_search`, `neural_status`, `neural_train`, `neural_patterns`

### System
`benchmark_run`, `features_detect`, `swarm_monitor`

---

## 🚀 NDP Agent Execution Examples

### Example: Silver Layer Development

```javascript
// Single message with NDP agents via Task tool
[Single Message - Parallel Agent Execution]:
  Task("Design TimescaleDB schema", "Create hypertables and continuous aggregates for air quality data. Use get-pattern skill first.", "ndp-timescale-dev")
  Task("Build ETL pipeline", "Implement Parquet to TimescaleDB ETL. Check architecture patterns.", "ndp-parquet-dev")
  Task("Create feature aggregations", "Design hourly/daily rollups for ML features.", "ndp-feature-engineer")
  Task("Write integration tests", "Test ETL pipeline with mock data.", "ndp-tester")

  // Batch ALL todos in ONE call
  TodoWrite { todos: [
    {content: "Design TimescaleDB schema", status: "in_progress", activeForm: "Designing TimescaleDB schema"},
    {content: "Create continuous aggregates", status: "pending", activeForm: "Creating continuous aggregates"},
    {content: "Implement ETL from Parquet", status: "pending", activeForm: "Implementing ETL"},
    {content: "Add retention policies", status: "pending", activeForm: "Adding retention policies"},
    {content: "Write integration tests", status: "pending", activeForm: "Writing integration tests"},
    {content: "Update architecture docs", status: "pending", activeForm: "Updating architecture docs"}
  ]}
```

### Example: New Feature Kickoff

```javascript
// Initialize feature with scrum master
[Single Message]:
  Task("Initialize dp-001 feature", "Create feature directory structure, STATUS.md, coordinate SPARC phases.", "ndp-scrum-master")
  Task("Design architecture", "Create Silver layer architecture for dp-001. Document ADRs.", "ndp-architect")

  // Create feature branch
  Bash "git checkout -b feature/dp-001"
```

---

## 📋 Agent Coordination Protocol

### Every NDP Agent MUST:

**1️⃣ BEFORE Work:**
```bash
# Use get-pattern skill to retrieve relevant patterns
claude-flow memory query "architecture" --namespace ndp-patterns
claude-flow hooks pre-task --description "[task]"
```

**2️⃣ DURING Work:**
```bash
claude-flow hooks post-edit --file "[file]" --memory-key "swarm/[agent]/[step]"
claude-flow hooks notify --message "[what was done]"
```

**3️⃣ AFTER Work:**
```bash
# Use save-pattern skill if discovered reusable pattern
claude-flow memory store "category:pattern-name" "description" --namespace ndp-patterns
claude-flow hooks post-task --task-id "[task]"
```

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
