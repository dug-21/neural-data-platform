# Team Builder Guide

How to create a specialized Claude agent team for any project using Ruven Cohens [Claude-Flow](github.com/ruvnet/claude-flow).

This document explains how we designed the NDP agent team and provides templates to create similar teams for other projects. Copy this file to another repository and ask Claude to create agents following this guide.

---

## Table of Contents

1. [Philosophy](#philosophy)
2. [Team Design Process](#team-design-process)
3. [Directory Structure](#directory-structure)
4. [Core Skills (Copy These First)](#core-skills)
5. [Agent Templates](#agent-templates)
6. [Pattern System](#pattern-system)
7. [GitHub Workflow Skill Template](#github-workflow-skill-template)
8. [Bootstrapping Patterns](#bootstrapping-patterns)
9. [Quick Start Checklist](#quick-start-checklist)

---

## Philosophy

### Why Specialized Agents?

Generic agents (`coder`, `tester`, `architect`) don't know your project. They:
- Reinvent patterns that already exist
- Create inconsistent code styles
- Miss project-specific conventions
- Don't know your architecture decisions
- Hardcoded in Claude and may or may not pickup any Claude-flow adjustments

**Project-specific agents** solve this by embedding:
- Architecture knowledge (ADRs, design patterns)
- Coding conventions (naming, error handling, file organization)
- Development procedures (how to add features, deploy, test)
- Technology stack awareness (your specific tools and frameworks)

### The Three-Skill Foundation

Every project team needs these three skills.  Feel free to add more, but name them in ways your agent will look for them:

| Skill | Purpose |
|-------|---------|
| `get-pattern` | Retrieve established patterns before implementing |
| `save-pattern` | Store new patterns after discovering them |
| `{project}-github-workflow` | Enforce consistent git conventions |

These skills ensure agents:
1. Check existing patterns before writing code
2. Document new patterns for future agents
3. Follow consistent commit/branch/PR conventions

---

## Team Design Process

### Step 1: Analyze Your Project

Understand your project's key areas:

```
Questions to answer:
├── What technology stack? (Language, frameworks, databases)
├── What are the main components? (Backend, frontend, ML, data, etc.)
├── What are the critical integration points?
├── What conventions exist? (Naming, file organization, error handling)
├── What's the deployment model? (Cloud, edge, containers, etc.)
└── What documentation exists? (ADRs, architecture docs, runbooks)
```

### Step 2: Define Agent Scopes

Map agents to your project areas:

| Scope | Agent Type | Responsibility |
|-------|------------|----------------|
| **Broad** | Architect, Scrum Master | Cross-cutting concerns, coordination |
| **General** | Core Developer | Main technology (Rust, Python, TypeScript, etc.) |
| **Specialized** | Tester, DevOps | Specific discipline |
| **Narrow** | Domain Specialists | Specific component or layer |

### Step 3: Create Agent Team Map

Example (NDP Project):

```
{project}-team/
├── Coordination
│   └── {project}-scrum-master     # Feature lifecycle, SPARC, status tracking
├── Core Team
│   ├── {project}-architect        # Design decisions, ADRs, patterns
│   ├── {project}-{lang}-dev       # Main language development
│   └── {project}-tester           # Testing strategy, integration tests
├── Domain Specialists
│   ├── {project}-{layer1}-dev     # First layer/component
│   ├── {project}-{layer2}-dev     # Second layer/component
│   └── ...                        # Additional specialists as needed
└── Feature Specialists
    ├── {project}-{feature1}-dev   # Specific feature area
    └── ...
```

### Step 4: Create Skills First

1. Create `get-pattern` skill (adapted from template below)
2. Create `save-pattern` skill (adapted from template below)
3. Create `{project}-github-workflow` skill with your conventions

### Step 5: Create Agents

For each agent:
1. Define frontmatter (name, type, scope, capabilities)
2. Add "MANDATORY: Before Any Work" section with pattern retrieval
3. Include project-specific knowledge
4. Reference required skills
5. Link related agents

---

## Directory Structure

```
.claude/
├── agents/
│   └── {project}/                    # Project-specific agents
│       ├── README.md                 # Agent roster and usage
│       ├── {project}-scrum-master.md # Coordination
│       ├── {project}-architect.md    # Architecture
│       ├── {project}-{lang}-dev.md   # Core development
│       ├── {project}-tester.md       # Testing
│       └── {project}-{domain}-dev.md # Domain specialists
├── skills/
│   ├── get-pattern/
│   │   └── SKILL.md                  # Pattern retrieval
│   ├── save-pattern/
│   │   └── SKILL.md                  # Pattern storage
│   └── {project}-github-workflow/
│       └── SKILL.md                  # Git conventions
└── patterns/
    └── INDEX.yaml                    # Pattern reference index
```

---

## Core Skills

### get-pattern Skill Template

Create `.claude/skills/get-pattern/SKILL.md`:

```markdown
---
name: "get-pattern"
description: "Retrieve established project patterns, conventions, architecture decisions, and reference documents. Use when you need to understand how something is done in this project before implementing."
---

# Get Pattern

## What This Skill Does

Retrieves established patterns, conventions, and architecture documentation for {PROJECT_NAME}. Use this **before implementing anything** to ensure you follow project standards.

## When to Use

- "How do I add a new [component type]?"
- "What's the architecture of this system?"
- "What are the naming conventions?"
- "Where should I put this file?"
- "How does [process] work?"

## Quick Reference

### Pattern Hierarchy ({project}-patterns namespace)

```
{project}-patterns
├── architecture/      # ADRs, design patterns, schemas, component relationships
├── data-flow/         # Pipeline patterns, data transformation approaches
├── development/       # How to add features, components, modules
├── deployment/        # Docker, cloud, infrastructure patterns
├── troubleshooting/   # Checklists, common issues and fixes
├── conventions/       # Naming rules, error handling, organization
├── procedures/        # Step-by-step guides for common tasks
└── {domain}/          # Domain-specific patterns
```

## Usage

### Method 1: CLI Commands

```bash
# Search for patterns by keyword
claude-flow memory query "<search-term>" --namespace {project}-patterns

# List all patterns in namespace
claude-flow memory list --namespace {project}-patterns

# Get specific pattern
claude-flow memory query "<category>:<pattern-name>" --namespace {project}-patterns
```

### Method 2: MCP Tools (Preferred for Agents)

```javascript
// Search patterns
mcp__claude-flow__memory_search({
  pattern: "<search-term>",
  namespace: "{project}-patterns",
  limit: 5
})

// Retrieve specific pattern
mcp__claude-flow__memory_usage({
  action: "retrieve",
  key: "<category>:<pattern-name>",
  namespace: "{project}-patterns"
})
```

## Key Documents (Update These Paths)

| Document | Path | Contains |
|----------|------|----------|
| Architecture Overview | `docs/architecture/OVERVIEW.md` | System design |
| Component Map | `docs/architecture/COMPONENTS.md` | How parts relate |
| ADR Directory | `docs/architecture/adr/` | Design decisions |

## Common Pattern Lookups

### Adding a New [Component Type]

```bash
claude-flow memory query "add-[component]" --namespace {project}-patterns
```

### Understanding the Architecture

```bash
claude-flow memory query "architecture" --namespace {project}-patterns
```

### Naming Conventions

```bash
claude-flow memory query "naming" --namespace {project}-patterns
```

## If Pattern Not Found

1. Check the pattern index: `.claude/patterns/INDEX.yaml`
2. Search documentation: `docs/`
3. If you discover a new pattern, use `save-pattern` skill to store it

## Related Skills

- `save-pattern` - Store new patterns you discover
- `{project}-github-workflow` - Git conventions
```

---

### save-pattern Skill Template

Create `.claude/skills/save-pattern/SKILL.md`:

```markdown
---
name: "save-pattern"
description: "Manage project patterns: create, update, deprecate, or delete patterns in the knowledge base. Use after discovering reusable approaches or when patterns become stale."
---

# Save Pattern

## What This Skill Does

Manages the full lifecycle of project patterns in memory:
- **Store** - Create new patterns
- **Update** - Replace existing patterns with new content
- **Deprecate** - Mark patterns as outdated with migration guidance
- **Delete** - Remove patterns entirely

## When to Use

| Situation | Operation |
|-----------|-----------|
| Discovered a reusable approach | Store |
| Defined a new process or procedure | Store |
| Pattern procedure has changed | Update |
| Pattern replaced by better approach | Deprecate |
| Pattern is wrong/dangerous/obsolete | Delete |

## Pattern Hierarchy ({project}-patterns namespace)

```
{project}-patterns
├── architecture/      # ADRs, design patterns, schemas
├── data-flow/         # Pipeline patterns, data transformation
├── development/       # Implementation procedures
├── deployment/        # Operational procedures
├── troubleshooting/   # Checklists, common issues
├── conventions/       # Naming rules, style guides
├── procedures/        # Step-by-step guides
└── {domain}/          # Domain-specific patterns
```

## Operations

### 1. Store (Create New Pattern)

```bash
claude-flow memory store "<category>:<pattern-name>" "<pattern-content>" --namespace {project}-patterns
```

Or via MCP:

```javascript
mcp__claude-flow__memory_usage({
  action: "store",
  key: "<category>:<pattern-name>",
  value: "<pattern-content>",
  namespace: "{project}-patterns",
  ttl: 0  // permanent
})
```

### Pattern Content Structure

```
# Pattern Name

## Context
When/why you would use this pattern.

## Problem
What problem does this solve?

## Solution
The actual pattern/procedure.

## Example
Concrete usage example.

## Related
- Related patterns or files
```

### 2. Update (Replace Existing)

Same as Store - memory overwrites existing keys.

### 3. Deprecate (Soft Delete)

```bash
claude-flow memory store "<category>:<old-pattern>" "# DEPRECATED: <Old Pattern>

## Status
DEPRECATED as of <date>

## Replacement
Use <category>:<new-pattern> instead.

## Migration
<Steps to migrate>

## Reason
<Why deprecated>" --namespace {project}-patterns
```

### 4. Delete (Hard Remove)

```javascript
mcp__claude-flow__memory_usage({
  action: "delete",
  key: "<category>:<pattern-name>",
  namespace: "{project}-patterns"
})
```

## Best Practices

1. **Be Specific** - Include concrete examples
2. **Include Context** - Explain when/why to use
3. **Reference Files** - Link to implementation code
4. **Use Consistent Keys** - `category:pattern-name` (kebab-case)
5. **Deprecate Don't Delete** - Preserve knowledge

## Related Skills

- `get-pattern` - Retrieve stored patterns
```

---

## Agent Templates

### Coordinator Agent Template

```markdown
---
name: {project}-scrum-master
type: coordinator
scope: broad
description: Feature lifecycle coordinator managing documentation, status tracking, and workflow
capabilities:
  - feature_lifecycle
  - sparc_coordination
  - status_tracking
  - progress_reporting
---

# {Project} Scrum Master

You are the feature lifecycle coordinator for {Project Name}. You ensure consistent structure, track progress, and coordinate documentation.

## Your Scope

- **Broad**: Cross-cutting coordination
- Feature directory structure enforcement
- STATUS.md maintenance
- SPARC phase progression
- GitHub workflow consistency

## MANDATORY: Before Any Work

### 1. Get Project Patterns

```bash
claude-flow memory query "conventions" --namespace {project}-patterns
claude-flow memory query "procedures" --namespace {project}-patterns
```

Or use MCP:
```javascript
mcp__claude-flow__memory_search({
  pattern: "feature",
  namespace: "{project}-patterns",
  limit: 5
})
```

## Feature Directory Structure

```
product/features/{feature-id}/
├── SCOPE.md                    # Initial scope
├── STATUS.md                   # Live status
├── specification/              # SPARC S
├── pseudocode/                 # SPARC P
├── architecture/               # SPARC A
├── refinement/                 # SPARC R
├── completion/                 # SPARC C
├── bugs/                       # Bug fixes
└── reports/                    # Coordination reports
```

## STATUS.md Template

```markdown
# {Feature ID}: {Title}

## Current Phase
{specification | pseudocode | architecture | refinement | completion | done}

## Progress
- [ ] SCOPE.md created
- [ ] SPARC Specification complete
- [ ] SPARC Pseudocode complete
- [ ] SPARC Architecture complete
- [ ] SPARC Refinement complete
- [ ] SPARC Completion complete
- [ ] All tests passing
- [ ] Documentation updated
- [ ] Deployed

## Active Work
{Current task}

## Last Updated
{YYYY-MM-DD HH:MM} by {agent/human}
```

## Related Agents

- `{project}-architect` - Design decisions
- `{project}-{lang}-dev` - Implementation
- `{project}-tester` - Testing

## Related Skills

- `{project}-github-workflow` - REQUIRED for all git operations
- `get-pattern` - Retrieve project patterns
- `save-pattern` - Store new patterns
```

---

### Architect Agent Template

```markdown
---
name: {project}-architect
type: architect
scope: broad
description: Architecture specialist for design decisions, ADRs, and cross-cutting concerns
capabilities:
  - architecture_design
  - adr_creation
  - pattern_definition
  - technology_selection
---

# {Project} Architect

You are the architecture specialist for {Project Name}. You make design decisions, create ADRs, and ensure architectural consistency.

## Your Scope

- **Broad**: Whole system and component interactions
- Architecture Decision Records (ADRs)
- Technology selection
- Cross-cutting concerns
- Pattern definition

## MANDATORY: Before Any Architecture Work

### 1. Load Existing Architecture Context

```bash
claude-flow memory query "architecture" --namespace {project}-patterns
```

### 2. Read Key Architecture Documents

- `docs/architecture/OVERVIEW.md` - System overview
- `docs/architecture/adr/` - Existing ADRs

## Core Architecture Knowledge

### {Your Primary Pattern Name}

{Document your primary architectural pattern here}

```{language}
// Example code showing the pattern
```

### Existing ADRs

| ADR | Decision |
|-----|----------|
| ADR-001 | {Decision 1} |
| ADR-002 | {Decision 2} |

### Technology Stack

| Layer | Technology | Status |
|-------|------------|--------|
| {Layer} | {Tech} | ✅ Current |

## When Creating ADRs

Store in `docs/architecture/adr/`:

```markdown
# ADR-NNN: Title

## Status
Proposed | Accepted | Deprecated | Superseded

## Context
What is the issue that motivates this decision?

## Decision
What is the change we're proposing?

## Consequences
What becomes easier or harder?

## Alternatives Considered
What other options were evaluated?
```

After creating an ADR:
```bash
claude-flow memory store "architecture:<adr-key>" "<summary>" --namespace {project}-patterns
```

## Related Agents

- `{project}-{lang}-dev` - Implements your designs
- `{project}-tester` - Validates testability
- `{project}-scrum-master` - Coordination

## Related Skills

- `{project}-github-workflow` - REQUIRED for all git operations
- `get-pattern` - Retrieve project patterns
- `save-pattern` - Store new patterns
```

---

### Developer Agent Template

```markdown
---
name: {project}-{lang}-dev
type: developer
scope: general
description: General {language} developer following established patterns and conventions
capabilities:
  - {lang}_development
  - async_programming
  - code_quality
---

# {Project} {Language} Developer

You are a {Language} developer for {Project Name}. You write clean, idiomatic code following the project's established patterns.

## Your Scope

- **General**: Any {Language} development that doesn't need a specialist
- Implementing new features following existing patterns
- Bug fixes and refactoring
- Code quality improvements

## MANDATORY: Before Any Implementation

### 1. Get Relevant Patterns

```bash
claude-flow memory query "<task-keywords>" --namespace {project}-patterns
```

### 2. Check Pattern Index

Review `.claude/patterns/INDEX.yaml` for existing patterns.

## Project Structure

```
{project}/
├── {src-dir}/              # Source code
│   ├── {modules}/          # Core modules
│   └── ...
├── {test-dir}/             # Tests
└── {config-dir}/           # Configuration
```

## Key Patterns to Follow

### 1. {Primary Pattern Name}

```{language}
// Example code
```

### 2. Error Handling

```{language}
// Error handling example
```

### 3. {Additional Pattern}

```{language}
// Additional pattern example
```

## Naming Conventions

| Element | Convention | Example |
|---------|------------|---------|
| Files | {convention} | {example} |
| Functions | {convention} | {example} |
| Classes/Types | {convention} | {example} |

## Code Quality Checklist

- [ ] Code is formatted
- [ ] Linter passes
- [ ] Tests pass
- [ ] Follows existing patterns
- [ ] No hardcoded secrets

## Related Agents

- `{project}-architect` - Design decisions
- `{project}-tester` - Test implementation
- `{project}-scrum-master` - Coordination

## Related Skills

- `{project}-github-workflow` - REQUIRED for all git operations
- `get-pattern` - Retrieve project patterns
- `save-pattern` - Store new patterns
```

---

## GitHub Workflow Skill Template

Create `.claude/skills/{project}-github-workflow/SKILL.md`:

```markdown
---
name: "{project}-github-workflow"
description: "{Project}-specific GitHub workflow conventions for branches, commits, PRs, and releases."
---

# {Project} GitHub Workflow

## Branch Strategy

### Branch Naming Convention

```
main                           # Always deployable, protected
└── feature/{feature-id}       # Feature development
    └── feature/{feature-id}/bug-{nnn}  # Bug fix during feature
```

### Rules

| Branch Type | Pattern | Example |
|-------------|---------|---------|
| Main | `main` | Protected, requires PR |
| Feature | `feature/{feature-id}` | `feature/auth-001` |
| Hotfix | `hotfix/{issue-id}` | `hotfix/login-fix` |
| Release | `release/v{major}.{minor}` | `release/v1.3` |

## Commit Convention

### Format

```
{type}({scope}): {description}

{optional body}

{optional footer}
```

### Types

| Type | Use For |
|------|---------|
| `feat` | New feature |
| `fix` | Bug fix |
| `docs` | Documentation |
| `refactor` | Code change (no feature/fix) |
| `test` | Tests |
| `chore` | Build, CI, tooling |
| `perf` | Performance |

### Scope

| Scope Type | Examples |
|------------|----------|
| Feature ID | `auth-001`, `api-002` |
| Component | `storage`, `router` |
| Layer | `deploy`, `ci` |

### Rules

1. Type and scope are lowercase
2. Description is lowercase, no period
3. Description is imperative mood ("add" not "added")
4. Max 72 characters for first line

### Examples

```bash
feat(auth-001): add oauth2 login flow
fix(api-002): handle null response from external service
docs(architecture): add authentication design doc
chore(ci): add automated testing workflow
```

## Pull Request Convention

### PR Title

Same format as commits:
```
{type}({scope}): {description}
```

### PR Template

```markdown
## Feature
{feature-id}: {feature title}

## Summary
{1-3 sentence description}

## Changes
- {Change 1}
- {Change 2}

## Checklist
- [ ] Tests passing
- [ ] Linting clean
- [ ] Documentation updated

## Testing
{How to test}
```

### Merge Strategy

- **Squash merge** for feature branches
- **Delete branch** after merge

## Enforcement

All agents MUST use these conventions for git operations.
```

---

## Pattern System

### Pattern Index Template

Create `.claude/patterns/INDEX.yaml`:

```yaml
# Pattern Index for {Project}
# This file maps pattern keys to summaries and file references

namespace: {project}-patterns

categories:
  architecture:
    description: "System design decisions and ADRs"
    patterns:
      domain-pattern:
        key: "architecture:domain-pattern"
        summary: "Primary architectural pattern description"
        files:
          - docs/architecture/OVERVIEW.md

  development:
    description: "Implementation procedures and coding patterns"
    patterns:
      add-feature:
        key: "development:add-feature"
        summary: "How to add a new feature"
        files:
          - docs/procedures/ADD_FEATURE.md

  deployment:
    description: "Operational procedures"
    patterns:
      deploy-production:
        key: "deployment:production"
        summary: "Production deployment procedure"
        files:
          - docs/deployment/PRODUCTION.md

  conventions:
    description: "Naming rules and style guides"
    patterns:
      naming:
        key: "conventions:naming"
        summary: "Naming conventions for all code"
```

---

## Bootstrapping Patterns

After creating skills and agents, bootstrap patterns into memory:

```bash
# Initialize patterns from documentation
claude-flow memory store "architecture:overview" "$(cat docs/architecture/OVERVIEW.md)" --namespace {project}-patterns

claude-flow memory store "conventions:naming" "# Naming Conventions

## Files
- {convention}

## Functions
- {convention}

## Types
- {convention}" --namespace {project}-patterns

claude-flow memory store "development:add-feature" "# How to Add a Feature

1. Create feature directory
2. Write SCOPE.md
3. Create branch: feature/{id}
4. Implement following patterns
5. Create PR" --namespace {project}-patterns
```

---

## Quick Start Checklist

When creating a team for a new project:

```
□ 1. Analyze project structure and technology
□ 2. Create .claude/agents/{project}/ directory
□ 3. Create .claude/skills/get-pattern/SKILL.md (adapt template)
□ 4. Create .claude/skills/save-pattern/SKILL.md (adapt template)
□ 5. Create .claude/skills/{project}-github-workflow/SKILL.md
□ 6. Create .claude/patterns/INDEX.yaml
□ 7. Create {project}-scrum-master agent
□ 8. Create {project}-architect agent
□ 9. Create {project}-{lang}-dev agent(s)
□ 10. Create {project}-tester agent
□ 11. Create domain specialist agents as needed
□ 12. Create .claude/agents/{project}/README.md with roster
□ 13. Bootstrap patterns into memory
□ 14. Update CLAUDE.md to reference your team
```

---

## Example: NDP Team

The Neural Data Platform team was built following this guide:

| Agent | Type | Scope | Purpose |
|-------|------|-------|---------|
| `ndp-scrum-master` | coordinator | broad | Feature lifecycle, SPARC |
| `ndp-architect` | architect | broad | ADRs, design decisions |
| `ndp-rust-dev` | developer | general | Rust implementation |
| `ndp-tester` | tester | specialized | Testing strategy |
| `ndp-parquet-dev` | developer | narrow | Bronze/Parquet layer |
| `ndp-timescale-dev` | developer | narrow | Silver/TimescaleDB layer |
| `ndp-feature-engineer` | developer | narrow | ML features |
| `ndp-ml-engineer` | developer | narrow | ruv-FANN models |
| `ndp-grafana-dev` | developer | narrow | Dashboards |
| `ndp-alert-engineer` | developer | narrow | Alerts/triggers |

Skills: `get-pattern`, `save-pattern`, `ndp-github-workflow`

---

## Transferring to Another Project

1. Copy this file to the new repository
2. Ask Claude: "Create a project team following the TEAMBUILDER.md guide"
3. Provide Claude with:
   - Technology stack information
   - Architecture documentation (if exists)
   - Coding conventions
   - Deployment model
4. Claude will analyze and create appropriate agents and skills
5. Review and iterate on the generated team

The key is that Claude can read this guide and adapt the templates to any project's specific needs while maintaining the proven structure and patterns.
