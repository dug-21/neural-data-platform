# smolagents + MCP Integration Research

**Date**: 2026-01-03
**Research Type**: Tool Integration Analysis
**Relevance to NDP**: High - Direct path to agentic data exploration

---

## 1. smolagents Overview

**Source**: [Hugging Face smolagents](https://huggingface.co/docs/smolagents/en/index)

smolagents is a minimalist AI agent library from Hugging Face (~1,000 lines of code) that emphasizes simplicity and code-first execution.

### Key Characteristics

| Feature | Description | NDP Relevance |
|---------|-------------|---------------|
| **Code Agents** | Writes Python code to execute actions | DuckDB queries |
| **Tool-Agnostic** | Works with MCP, LangChain, HF Spaces | Flexible integration |
| **LLM-Agnostic** | Any LLM via LiteLLM integration | Use Claude, local models |
| **Sandboxed Execution** | E2B, Docker, Pyodide+WASM | Edge-safe execution |
| **Multimodal** | Text, images, video, audio | Time-series viz |

### Why smolagents for NDP?

```
┌─────────────────────────────────────────────────────────────────┐
│                    USER QUESTION                                │
│   "What caused the PM2.5 spike on December 15th?"              │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                    smolagents CodeAgent                         │
│                                                                  │
│   1. Introspect DuckDB schema via MCP                          │
│   2. Generate SQL: SELECT * FROM silver_indoor_air              │
│                    WHERE date = '2025-12-15'                    │
│   3. Execute query via DuckDB MCP tool                         │
│   4. Analyze results in Python                                  │
│   5. Correlate with outdoor_weather data                       │
│   6. Return natural language explanation                        │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                    INSIGHT                                       │
│   "The PM2.5 spike coincided with outdoor AQI reaching 125      │
│    (Unhealthy for Sensitive Groups). Wind was from the south    │
│    at 15 mph, likely carrying pollutants from the highway."     │
└─────────────────────────────────────────────────────────────────┘
```

---

## 2. Model Context Protocol (MCP)

**Source**: [Model Context Protocol Specification](https://modelcontextprotocol.io/specification/2025-11-25)

MCP is an open standard from Anthropic for connecting LLMs to external tools and data sources. It became the de-facto standard in 2025.

### MCP Architecture

```
┌──────────────────┐     ┌──────────────────┐     ┌──────────────────┐
│   LLM Client     │     │   MCP Server     │     │   Data Source    │
│  (Claude Code,   │────►│  (runs locally)  │────►│  (DuckDB, APIs)  │
│   smolagents)    │◄────│                  │◄────│                  │
└──────────────────┘     └──────────────────┘     └──────────────────┘

Protocol: JSON-RPC 2.0 over stdio/HTTP
Operations: resources.list, resources.read, tools.call
```

### Key MCP Servers for NDP

| Server | Purpose | Source |
|--------|---------|--------|
| **MotherDuck/DuckDB** | SQL queries, schema introspection | [MotherDuck Blog](https://motherduck.com/blog/faster-data-pipelines-with-mcp-duckdb-ai/) |
| **File System** | Read Parquet files directly | Official |
| **SQLite** | Pattern storage, agent memory | Official |
| **Google Toolbox** | BigQuery, AlloyDB | [Google Cloud Blog](https://cloud.google.com/blog/products/ai-machine-learning/mcp-toolbox-for-databases-now-supports-model-context-protocol) |

---

## 3. DuckDB MCP Integration for NDP

### Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                      DEV CONTAINER                               │
│                                                                  │
│  ┌──────────────────┐      ┌──────────────────┐                │
│  │   Claude Code    │      │   DuckDB MCP     │                │
│  │   (smolagents)   │◄────►│     Server       │                │
│  └──────────────────┘      └────────┬─────────┘                │
│                                     │                           │
│                                     │ HTTP (local) or           │
│                                     │ remote via SSH tunnel     │
└─────────────────────────────────────┼───────────────────────────┘
                                      │
                                      ▼
┌─────────────────────────────────────────────────────────────────┐
│                      PRODUCTION PI                               │
│                                                                  │
│  ┌──────────────────┐      ┌──────────────────┐                │
│  │  DuckDB HTTP API │◄────►│  Bronze Parquet  │                │
│  │  (port 9090)     │      │     /data/*      │                │
│  └──────────────────┘      └──────────────────┘                │
│                                                                  │
│  Silver Views: silver_indoor_air, silver_outdoor_weather, etc.  │
└─────────────────────────────────────────────────────────────────┘
```

### MCP Server Configuration

```json
{
  "mcpServers": {
    "duckdb-ndp": {
      "command": "npx",
      "args": ["mcp-server-duckdb", "--remote", "http://pi-host:9090"],
      "env": {}
    }
  }
}
```

### MCP Tools Exposed

```python
# Schema introspection
@tool
def describe_tables() -> str:
    """List all Silver layer views with column definitions"""
    return duckdb.sql("SHOW ALL TABLES")

# Query execution
@tool
def execute_query(sql: str) -> pd.DataFrame:
    """Execute read-only SQL against DuckDB Silver layer"""
    # Validate SQL is SELECT-only
    if not sql.strip().upper().startswith("SELECT"):
        raise ValueError("Only SELECT queries allowed")
    return duckdb.sql(sql).df()

# Pattern-specific tools
@tool
def find_pm25_spikes(threshold: float = 35.0, days: int = 7) -> pd.DataFrame:
    """Find PM2.5 readings above EPA threshold in recent history"""
    sql = f"""
    SELECT timestamp, pm25, co2, temperature
    FROM silver_indoor_air
    WHERE pm25 > {threshold}
      AND timestamp > now() - interval '{days} days'
    ORDER BY pm25 DESC
    """
    return duckdb.sql(sql).df()
```

---

## 4. Self-Correcting SQL Workflows

**Source**: [AI-Driven Analytics with MCP](https://skywork.ai/skypage/en/ai-driven-analytics-motherduck-duckdb/1977950223667744768)

A key pattern for agentic data exploration is self-correcting SQL:

### Flow

```
┌────────────────────────────────────────────────────────────────┐
│                    SELF-CORRECTING LOOP                         │
│                                                                 │
│   1. User asks: "Show humidity trends last week"                │
│                         │                                       │
│                         ▼                                       │
│   2. Agent generates SQL:                                       │
│      SELECT date, humidity FROM silver_indoor_air               │
│                         │                                       │
│                         ▼                                       │
│   3. Execute query → ERROR: column 'date' not found            │
│                         │                                       │
│                         ▼                                       │
│   4. Agent introspects schema:                                  │
│      DESCRIBE silver_indoor_air → timestamp, humidity...        │
│                         │                                       │
│                         ▼                                       │
│   5. Agent corrects SQL:                                        │
│      SELECT date_trunc('day', to_timestamp(timestamp))          │
│             AS date, avg(humidity) AS avg_humidity              │
│      FROM silver_indoor_air                                     │
│      WHERE timestamp > now() - interval '7 days'                │
│      GROUP BY 1                                                 │
│                         │                                       │
│                         ▼                                       │
│   6. Success → Return results + store pattern                   │
└────────────────────────────────────────────────────────────────┘
```

### Implementation Pattern

```python
from smolagents import CodeAgent, tool
import duckdb

class SQLAgent(CodeAgent):
    def __init__(self, duckdb_conn, **kwargs):
        self.db = duckdb_conn
        self.schema_cache = {}
        super().__init__(**kwargs)

    @tool
    def execute_with_retry(self, sql: str, max_retries: int = 3) -> str:
        """Execute SQL with self-correction on error"""
        for attempt in range(max_retries):
            try:
                result = self.db.sql(sql).df()
                # Store successful pattern
                self.store_pattern(sql, success=True)
                return result.to_markdown()
            except Exception as e:
                error_msg = str(e)
                # Get schema context
                schema = self.get_schema_context()
                # Let LLM correct the query
                sql = self.correct_query(sql, error_msg, schema)

        return f"Failed after {max_retries} attempts"

    def get_schema_context(self) -> str:
        """Get schema for all Silver tables"""
        if not self.schema_cache:
            tables = self.db.sql("SHOW TABLES").df()
            for table in tables['name']:
                cols = self.db.sql(f"DESCRIBE {table}").df()
                self.schema_cache[table] = cols
        return str(self.schema_cache)
```

---

## 5. Pattern Learning with Vector Storage

**Integration with ruvector**

```
┌─────────────────────────────────────────────────────────────────┐
│                   SQL PATTERN LEARNING                           │
│                                                                  │
│  ┌──────────────────┐     ┌──────────────────┐                 │
│  │ Successful Query │────►│   Embed Query    │                 │
│  │     + Context    │     │ (text → vector)  │                 │
│  └──────────────────┘     └────────┬─────────┘                 │
│                                    │                            │
│                                    ▼                            │
│                      ┌──────────────────────┐                   │
│                      │    rvLite/ruvector   │                   │
│                      │                       │                   │
│                      │   Pattern Storage:    │                   │
│                      │   - NL description    │                   │
│                      │   - SQL query         │                   │
│                      │   - Success count     │                   │
│                      │   - Context (tables)  │                   │
│                      └──────────────────────┘                   │
│                                    │                            │
│                                    ▼                            │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │ Next time user asks similar question:                    │  │
│  │                                                          │  │
│  │ User: "humidity patterns for this month"                 │  │
│  │                    ↓                                     │  │
│  │ Semantic search: "humidity trends last week" (85% sim)   │  │
│  │                    ↓                                     │  │
│  │ Retrieve SQL pattern, adapt dates, execute               │  │
│  └──────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
```

---

## 6. Security Model

### Read-Only Access

```python
# DuckDB connection with strict read-only mode
conn = duckdb.connect(database=':memory:', read_only=True)

# Attach Parquet files as external tables
conn.execute("""
    CREATE VIEW bronze_air AS
    SELECT * FROM read_parquet('/data/air-quality/*.parquet')
""")

# Use SQL injection prevention
from sqlglot import parse, transpile

def safe_execute(sql: str):
    # Parse SQL to AST
    parsed = parse(sql)

    # Verify only SELECT statements
    for stmt in parsed:
        if stmt.key != 'select':
            raise SecurityError("Only SELECT allowed")

    # Check for dangerous functions
    dangerous = ['write_parquet', 'copy', 'attach', 'export']
    sql_lower = sql.lower()
    for func in dangerous:
        if func in sql_lower:
            raise SecurityError(f"Function {func} not allowed")

    return conn.execute(sql)
```

### Query Sandboxing

```
┌─────────────────────────────────────────────────────────────────┐
│                    SANDBOXED EXECUTION                           │
│                                                                  │
│  Option 1: Pyodide + WASM (in-browser)                          │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │ Browser sandbox → Cannot access filesystem              │   │
│  │ DuckDB-WASM → Read-only, limited memory                 │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                  │
│  Option 2: E2B Sandbox                                          │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │ Ephemeral container → Destroyed after query             │   │
│  │ Network isolated → Cannot exfiltrate data               │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                  │
│  Option 3: Docker with --read-only                              │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │ Read-only filesystem → Cannot write anywhere            │   │
│  │ Resource limits → Memory and CPU caps                   │   │
│  └─────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
```

---

## 7. Integration Architecture for NDP

### Recommended Stack

```
┌─────────────────────────────────────────────────────────────────┐
│                    AGENTIC EXPLORATION STACK                     │
│                                                                  │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │                    USER INTERFACE                        │   │
│  │            Claude Code / CLI / Jupyter                   │   │
│  └─────────────────────────────────────────────────────────┘   │
│                              │                                  │
│                              ▼                                  │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │                    smolagents                            │   │
│  │                                                          │   │
│  │  CodeAgent:                                              │   │
│  │  - Natural language → Python/SQL                         │   │
│  │  - Multi-step reasoning                                  │   │
│  │  - Tool orchestration                                    │   │
│  │                                                          │   │
│  │  Tools:                                                  │   │
│  │  - @tool describe_schema()                               │   │
│  │  - @tool execute_query(sql)                              │   │
│  │  - @tool find_patterns(metric, timerange)                │   │
│  │  - @tool correlate_streams(stream1, stream2)             │   │
│  │  - @tool visualize(data, chart_type)                     │   │
│  └───────────────────────┬─────────────────────────────────┘   │
│                          │                                      │
│           ┌──────────────┼──────────────┐                      │
│           ▼              ▼              ▼                      │
│  ┌────────────┐  ┌────────────┐  ┌────────────┐               │
│  │  DuckDB    │  │  rvLite    │  │ Grafana    │               │
│  │  MCP       │  │  Patterns  │  │ API        │               │
│  │  Server    │  │  Memory    │  │            │               │
│  └────────────┘  └────────────┘  └────────────┘               │
│        │               │               │                       │
│        └───────────────┴───────────────┘                       │
│                        │                                        │
│                        ▼                                        │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │                   NDP DATA LAYER                         │   │
│  │                                                          │   │
│  │   Bronze (Parquet) ─► Silver (DuckDB) ─► Gold (TBD)     │   │
│  └─────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
```

---

## 8. Memory Budget

| Component | Memory Est. | Purpose |
|-----------|-------------|---------|
| smolagents runtime | 50-100MB | Agent framework |
| DuckDB MCP Server | 50MB | Protocol bridge |
| rvLite pattern storage | 50-200MB | SQL pattern memory |
| LLM inference | 0 (API) or 2-4GB (local) | Query understanding |
| **Total (API LLM)** | **~350MB** | Edge-compatible |
| **Total (Local LLM)** | **~4GB** | Requires more RAM |

---

## 9. Implementation Roadmap

### Phase 1: MCP Foundation (1-2 days)
1. Set up DuckDB MCP server pointing to production Pi
2. Configure Claude Code with MCP connection
3. Test basic query execution

### Phase 2: smolagents Integration (2-3 days)
1. Install smolagents in dev container
2. Create NDP-specific tools (schema, query, patterns)
3. Implement self-correcting SQL loop

### Phase 3: Pattern Learning (1 week)
1. Deploy rvLite for pattern storage
2. Embed successful queries
3. Implement semantic retrieval

### Phase 4: Advanced Features (2 weeks)
1. Multi-agent coordination
2. Dashboard auto-generation
3. Anomaly detection agents

---

## 10. References

- [smolagents Documentation](https://huggingface.co/docs/smolagents/en/index)
- [smolagents GitHub](https://github.com/huggingface/smolagents)
- [MCP Specification](https://modelcontextprotocol.io/specification/2025-11-25)
- [MCP Servers Repository](https://github.com/modelcontextprotocol/servers)
- [MotherDuck MCP Integration](https://motherduck.com/blog/faster-data-pipelines-with-mcp-duckdb-ai/)
- [Google MCP Toolbox](https://cloud.google.com/blog/products/ai-machine-learning/mcp-toolbox-for-databases-now-supports-model-context-protocol)
- [Text-to-SQL with MCP](https://medium.com/@sanjeebmeister/model-context-protocol-mcp-in-ai-agent-development-text-to-sql-af66b2e4a52c)

---

*Research conducted as part of Hive Mind Research Swarm*
