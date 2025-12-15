# Swarm Coordination Protocols
## Hierarchical Command Structure - Operations Manual

---

## Quick Reference

### For Workers: How to Communicate with Coordinator

**Starting a Task**:
```bash
# 1. Restore session
npx claude-flow@alpha hooks session-restore --session-id "swarm-[agent-id]"

# 2. Signal task start
npx claude-flow@alpha hooks pre-task --description "[task-description]"

# 3. Notify coordinator
npx claude-flow@alpha hooks notify --message "Agent [id]: Started [task]"
```

**During Task Execution**:
```bash
# Update progress every 5 minutes
npx claude-flow@alpha hooks notify --message "Agent [id]: Progress [X%] on [task]"

# Report blockers immediately
npx claude-flow@alpha hooks notify --message "Agent [id]: BLOCKED - [reason]"

# Store intermediate results
npx claude-flow@alpha hooks post-edit --file "[file]" --memory-key "swarm/workers/[agent-id]/[task-id]"
```

**Completing a Task**:
```bash
# 1. Signal completion
npx claude-flow@alpha hooks post-task --task-id "[task-id]"

# 2. Report results
npx claude-flow@alpha hooks notify --message "Agent [id]: COMPLETED [task] - [summary]"

# 3. End session
npx claude-flow@alpha hooks session-end --export-metrics true
```

---

## For Coordinator: Managing the Swarm

### Task Intake Process

**Step 1: Receive & Analyze**
```markdown
1. Parse incoming task requirements
2. Identify deliverables and constraints
3. Classify task type (research/code/analysis/test)
4. Estimate complexity and duration
5. Identify dependencies and prerequisites
```

**Step 2: Plan & Decompose**
```markdown
1. Break down into subtasks
2. Define acceptance criteria for each
3. Sequence tasks based on dependencies
4. Assign priority levels
5. Estimate resource requirements
```

**Step 3: Allocate & Assign**
```markdown
1. Match capabilities to task requirements
2. Check agent availability and workload
3. Select optimal agent(s)
4. Spawn new agents if needed
5. Assign tasks with clear specifications
```

### Agent Management

**Spawning Strategy**:
```bash
# Research-heavy tasks
mcp__claude-flow__agent_spawn researcher --capabilities="research,analysis"

# Implementation tasks
mcp__claude-flow__agent_spawn coder --capabilities="rust,testing,optimization"

# Data/metrics tasks
mcp__claude-flow__agent_spawn analyst --capabilities="data_analysis,reporting"

# Quality validation
mcp__claude-flow__agent_spawn tester --capabilities="testing,validation,qa"
```

**Load Balancing**:
- Target: 70-80% utilization per agent
- Max concurrent tasks per agent: 2-3
- Redistribute if agent >85% utilized
- Spawn additional agents if all >80% utilized

**Performance Monitoring**:
```bash
# Check swarm health every 5 minutes
npx claude-flow@alpha hooks notify --message "Coordinator: Status check - [X] active tasks, [Y] agents"

# Track metrics
mcp__claude-flow__agent_metrics --agent-id "[id]"

# Monitor bottlenecks
mcp__claude-flow__swarm_monitor --interval=300000
```

---

## Communication Templates

### Task Assignment Message
```markdown
## TASK ASSIGNMENT: [TASK-ID]

**Assigned To**: [Agent Type] Agent [ID]
**Priority**: [CRITICAL|HIGH|MEDIUM|LOW]
**Estimated Duration**: [X] hours
**Dependencies**: [List or "None"]

### Objective
[Clear, concise description of what needs to be accomplished]

### Acceptance Criteria
- [ ] [Criterion 1]
- [ ] [Criterion 2]
- [ ] [Criterion 3]

### Context
[Relevant background information, file locations, constraints]

### Resources
- Files: [List relevant files]
- Documentation: [Links to docs]
- Previous Work: [References]

### Reporting
- Progress updates every 5 minutes
- Notify immediately if blocked
- Signal completion with summary

**Start Time**: [Timestamp]
**Target Completion**: [Timestamp]
```

### Status Update Message (Worker → Coordinator)
```markdown
## STATUS UPDATE: [TASK-ID]

**Agent**: [Type] [ID]
**Timestamp**: [ISO8601]
**Progress**: [X]%

### Current Status
- **State**: [IN_PROGRESS|BLOCKED|REVIEW|COMPLETED]
- **Current Step**: [What is being worked on now]
- **Completed**: [List of completed items]
- **Remaining**: [List of pending items]

### Blockers
[List blockers or "None"]

### Next Steps
[What will be done next]

### ETA
[Estimated completion time]
```

### Escalation Message (Worker → Coordinator)
```markdown
## ESCALATION: [TASK-ID]

**Agent**: [Type] [ID]
**Severity**: [CRITICAL|HIGH|MEDIUM]
**Timestamp**: [ISO8601]

### Issue
[Clear description of the problem]

### Impact
[How this affects task completion]

### Attempted Solutions
1. [What was tried]
2. [What was tried]

### Required Support
[Specific help needed from coordinator]

### Recommended Action
[Agent's suggestion for resolution]
```

---

## Workflow Examples

### Example 1: Feature Implementation Task

**Coordinator Analysis**:
```markdown
Task: "Implement authentication middleware for air quality API"

Classification: CODE
Priority: HIGH
Estimated Duration: 4 hours
Dependencies: None

Decomposition:
1. Research authentication patterns in Rust (1 hour)
2. Implement middleware module (2 hours)
3. Write unit tests (0.5 hours)
4. Integration testing (0.5 hours)

Agent Assignment:
- Research Worker: Task 1
- Code Worker: Task 2, 3
- Test Worker: Task 4
```

**Execution Flow**:
```bash
# Coordinator spawns agents
mcp__claude-flow__agent_spawn researcher
mcp__claude-flow__agent_spawn coder
mcp__claude-flow__agent_spawn tester

# Assigns sequential tasks
Task("Research Worker: Analyze authentication patterns...")
Task("Code Worker: Implement middleware based on research...")
Task("Test Worker: Validate implementation...")
```

### Example 2: Bug Investigation & Fix

**Coordinator Analysis**:
```markdown
Task: "Investigate and fix etcd configuration loading issue"

Classification: CODE + ANALYSIS
Priority: CRITICAL
Estimated Duration: 3 hours
Dependencies: Access to logs, running system

Decomposition:
1. Analyze logs and reproduce issue (1 hour)
2. Identify root cause (0.5 hours)
3. Implement fix (1 hour)
4. Validate fix and regression test (0.5 hours)

Agent Assignment:
- Analyst Worker: Task 1
- Code Worker: Task 2, 3
- Test Worker: Task 4
```

---

## Best Practices

### For Coordinators
1. **Always analyze before assigning** - Understand the full scope
2. **Be explicit** - Leave no room for ambiguity in assignments
3. **Monitor actively** - Don't wait for problems to be reported
4. **Adjust dynamically** - Reallocate resources based on progress
5. **Document decisions** - Record rationale for strategic choices

### For Workers
1. **Acknowledge assignments** - Confirm receipt and understanding
2. **Report proactively** - Don't wait to be asked for status
3. **Escalate early** - Flag blockers as soon as identified
4. **Store context** - Use memory namespaces for continuity
5. **Signal completion** - Clear handoff when task is done

### For Everyone
1. **Use structured formats** - Consistent communication patterns
2. **Be specific** - Vague updates create confusion
3. **Track time** - Know if estimates align with actuals
4. **Share knowledge** - Store learnings in shared namespaces
5. **Improve continuously** - Learn from each coordination cycle

---

## Troubleshooting

### Agent Not Responding
```bash
# Check agent status
mcp__claude-flow__agent_list

# Check for errors
mcp__claude-flow__agent_metrics --agent-id "[id]"

# Reassign task if needed
mcp__claude-flow__task_orchestrate "Reassign task [id] from [agent-1] to [agent-2]"
```

### Task Taking Too Long
```bash
# Analyze bottleneck
mcp__claude-flow__swarm_monitor

# Check dependencies
mcp__claude-flow__task_status --task-id "[id]"

# Consider parallel execution or additional resources
```

### Quality Issues
```bash
# Assign reviewer
mcp__claude-flow__agent_spawn reviewer

# Request rework with specific criteria
Task("Reviewer: Validate [deliverable] against [criteria]")
```

---

## Integration Checklist

Before starting coordination:
- [ ] Session initialized with unique ID
- [ ] Memory namespaces created
- [ ] Task tracking system ready
- [ ] Agent capability matrix defined
- [ ] Communication protocols understood
- [ ] Monitoring systems active
- [ ] Escalation procedures clear

During coordination:
- [ ] All tasks logged and tracked
- [ ] Status updates received regularly
- [ ] Metrics collected and analyzed
- [ ] Decisions documented
- [ ] Blockers resolved promptly
- [ ] Quality gates enforced
- [ ] Knowledge captured

After coordination:
- [ ] All tasks completed or handed off
- [ ] Session state persisted
- [ ] Metrics exported
- [ ] Lessons learned documented
- [ ] Artifacts archived
- [ ] Final report generated

---

*Coordination Protocols v1.0.0*
*Swarm: swarm-hierarchical-coordinator-001*
*Last Updated: 2025-12-15*
