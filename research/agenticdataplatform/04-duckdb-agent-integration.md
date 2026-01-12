# DuckDB + AI Agent Integration Patterns

**Research Date**: 2026-01-03
**Researcher**: ndp-rust-dev (Hive-Mind Swarm)
**Context**: Agentic capabilities for accelerating data exploration in NDP Silver layer

---

## Executive Summary

This research explores integration patterns for exposing DuckDB to AI agents in the Neural Data Platform. The NDP uses DuckDB for Silver layer queries over Parquet Bronze files, making agentic access to DuckDB essential for autonomous data exploration, self-correcting SQL workflows, and natural language analytics.

**Key Findings:**

| Pattern | Maturity | NDP Fit | Implementation Effort |
|---------|----------|---------|----------------------|
| MotherDuck MCP Server | Production | Excellent | Low (1-2 weeks) |
| smolagents + DuckDB | Production | Good | Medium (2-3 weeks) |
| DuckDB-NSQL Model | Production | Good | Medium (2-4 weeks) |
| Self-Correcting SQL | Emerging | Excellent | Medium (3-4 weeks) |
| SQL Pattern Learning | Research | Good | High (4-6 weeks) |

**Recommendation**: Implement MotherDuck MCP Server for immediate agent access, then layer smolagents for custom tool workflows with self-correcting SQL patterns for reliability.

---

## 1. MCP (Model Context Protocol) for DuckDB

### 1.1 Overview

The Model Context Protocol (MCP) is an open standard that enables AI assistants to interact with external data sources and tools. MotherDuck's official MCP server implements this protocol to allow AI assistants (Claude, Cursor, etc.) to directly interact with local DuckDB or MotherDuck cloud databases.

**Key Characteristics:**
- Standardized protocol (like "USB-C for AI applications")
- Python-based server implementation
- 62K+ downloads, 371+ GitHub stars
- Supports local DuckDB, MotherDuck cloud, and S3-hosted databases

### 1.2 Architecture

```
+------------------+       +-------------------+       +----------------+
|  AI Assistant    |       |   MCP Server      |       |    DuckDB      |
|  (Claude/Cursor) | <---> | (mcp-server-      | <---> |  (Local/Cloud) |
|                  |  MCP  |  motherduck)      |  SQL  |                |
+------------------+       +-------------------+       +----------------+
                                    |
                                    v
                           +-------------------+
                           | Result Limiting   |
                           | - 1024 rows max   |
                           | - 50K chars max   |
                           +-------------------+
```

### 1.3 Schema Introspection

The MCP server exposes a single primary tool (`query`) that executes SQL queries. Schema introspection happens through standard SQL:

```sql
-- Describe table structure
DESCRIBE SELECT * FROM read_parquet('/bronze/air-quality/*.parquet');

-- List all tables
SELECT * FROM duckdb_tables();

-- Get column metadata
SELECT * FROM information_schema.columns
WHERE table_name = 'sensor_readings';

-- Get table statistics
SELECT * FROM duckdb_tables()
WHERE table_name = 'sensor_readings';
```

**MCP Workflow for Schema Discovery:**

```python
# Pseudo-code for MCP-based schema discovery
async def discover_schema(mcp_client, parquet_path):
    # Step 1: Describe the Parquet schema
    schema_result = await mcp_client.call_tool(
        "query",
        {"sql": f"DESCRIBE SELECT * FROM read_parquet('{parquet_path}')"}
    )

    # Step 2: Get sample data
    sample_result = await mcp_client.call_tool(
        "query",
        {"sql": f"SELECT * FROM read_parquet('{parquet_path}') LIMIT 5"}
    )

    return {"schema": schema_result, "sample": sample_result}
```

### 1.4 Self-Correcting SQL Workflows

The key innovation is creating a feedback loop where the AI writes SQL, executes it, observes errors, and corrects autonomously:

```yaml
self_correcting_workflow:
  step_1_schema_context:
    action: "Provide schema as XML in prompt context"
    rationale: "XML format yields best LLM results per MotherDuck research"

  step_2_local_execution:
    action: "Execute SQL against local DuckDB replica"
    command: "duckdb local.db -f query.sql"

  step_3_error_feedback:
    action: "Feed errors back to LLM"
    prompt: |
      The following SQL query failed:
      ```sql
      {query}
      ```
      Error: {error_message}
      Schema: {schema_xml}

      Please fix the query.

  step_4_iteration:
    action: "Repeat until success or max attempts (3)"
```

**MotherDuck's FixIt Pattern:**
- Generates only the line number and fixed line (not entire query)
- Reduces latency from 10-20 seconds to 1-3 seconds
- LLM receives: error, query, schema context

### 1.5 Installation and Configuration

```bash
# Install with uv (recommended)
pip install uv
uvx mcp-server-motherduck --db-path :memory:

# For MotherDuck cloud
uvx mcp-server-motherduck --db-path md: --motherduck-token $MD_TOKEN

# For local DuckDB file
uvx mcp-server-motherduck --db-path /path/to/database.db --read-only

# For S3-hosted DuckDB
uvx mcp-server-motherduck --db-path s3://bucket/database.db
```

**Configuration for Claude Desktop (mcp.json):**

```json
{
  "mcpServers": {
    "ndp-duckdb": {
      "command": "uvx",
      "args": [
        "mcp-server-motherduck",
        "--db-path", "/workspaces/neural-data-platform/bronze/duckdb/analytics.db",
        "--read-only",
        "--max-rows", "1024",
        "--max-chars", "50000"
      ]
    }
  }
}
```

### 1.6 Security Configuration

**Three-Tier Security Approach:**

| Mode | Use Case | Configuration |
|------|----------|---------------|
| Read-Only | Exploration | `--read-only` |
| SaaS Mode | Multi-user | `--saas-mode` (restricts file access) |
| Read Scaling Tokens | Production | MotherDuck tokens with 4 read replicas |

**For NDP (Recommended):**

```bash
uvx mcp-server-motherduck \
  --db-path /bronze/duckdb/analytics.db \
  --read-only \
  --max-rows 500 \
  --max-chars 25000
```

---

## 2. smolagents + DuckDB Pattern

### 2.1 Overview

smolagents is Hugging Face's lightweight agent framework (~1000 lines of code) that enables LLM agents to write and execute Python code. The key insight: agents write Python code that calls tools, enabling natural composability with DuckDB.

**Key Characteristics:**
- CodeAgent writes actions in Python (not just function calls)
- Model-agnostic: supports any LLM (local, API, Anthropic, OpenAI)
- Secure sandboxed execution (Modal, E2B, Docker)
- ~30 lines of code to build an agent

### 2.2 Architecture

```
+------------------+       +-------------------+       +----------------+
|  User Query      |       |  CodeAgent        |       |  @tool         |
|  "What's avg     | ----> |  (smolagents)     | ----> |  decorated     |
|   PM2.5?"        |       |                   |       |  functions     |
+------------------+       +-------------------+       +----------------+
                                    |                         |
                                    v                         v
                           +-------------------+       +----------------+
                           | Python Code Gen   |       |  DuckDB        |
                           | (think-act loop)  |       |  Execution     |
                           +-------------------+       +----------------+
```

### 2.3 Tool Decorators for DuckDB

```python
from smolagents import tool, CodeAgent, HfApiModel
import duckdb

# Initialize DuckDB connection
conn = duckdb.connect('/bronze/duckdb/analytics.db', read_only=True)

@tool
def query_duckdb(sql: str) -> str:
    """
    Execute a SQL query against the NDP DuckDB database.

    Args:
        sql: A valid DuckDB SQL query. Must be read-only.

    Returns:
        Query results as a formatted string.

    Example:
        query_duckdb("SELECT AVG(pm2_5) FROM air_quality_hourly WHERE location = 'Seattle'")
    """
    # Validate read-only
    if any(kw in sql.upper() for kw in ['INSERT', 'UPDATE', 'DELETE', 'DROP', 'CREATE', 'ALTER']):
        return "Error: Only read-only queries are allowed."

    try:
        result = conn.execute(sql).fetchdf()
        if len(result) > 100:
            result = result.head(100)
            return f"Results (showing first 100 of {len(result)} rows):\n{result.to_string()}"
        return result.to_string()
    except Exception as e:
        return f"Query error: {str(e)}"

@tool
def describe_table(table_name: str) -> str:
    """
    Get schema information for a table in the NDP database.

    Args:
        table_name: Name of the table to describe.

    Returns:
        Column names, types, and sample values.
    """
    try:
        schema = conn.execute(f"DESCRIBE {table_name}").fetchdf()
        sample = conn.execute(f"SELECT * FROM {table_name} LIMIT 3").fetchdf()
        return f"Schema:\n{schema.to_string()}\n\nSample data:\n{sample.to_string()}"
    except Exception as e:
        return f"Error describing table: {str(e)}"

@tool
def list_parquet_files(path_pattern: str) -> str:
    """
    List available Parquet files matching a pattern.

    Args:
        path_pattern: Glob pattern for Parquet files (e.g., '/bronze/air-quality/*.parquet')

    Returns:
        List of matching file paths with row counts.
    """
    try:
        result = conn.execute(f"""
            SELECT
                filename,
                COUNT(*) as row_count
            FROM read_parquet('{path_pattern}', filename=true)
            GROUP BY filename
        """).fetchdf()
        return result.to_string()
    except Exception as e:
        return f"Error listing files: {str(e)}"

@tool
def get_time_range(table_name: str, timestamp_column: str = "timestamp") -> str:
    """
    Get the time range of data in a table.

    Args:
        table_name: Name of the table.
        timestamp_column: Name of the timestamp column (default: 'timestamp').

    Returns:
        Minimum and maximum timestamps in the table.
    """
    try:
        result = conn.execute(f"""
            SELECT
                MIN({timestamp_column}) as earliest,
                MAX({timestamp_column}) as latest,
                COUNT(*) as total_rows
            FROM {table_name}
        """).fetchdf()
        return result.to_string()
    except Exception as e:
        return f"Error getting time range: {str(e)}"
```

### 2.4 CodeAgent for SQL Generation

```python
from smolagents import CodeAgent, HfApiModel
from anthropic import Anthropic

# Use Claude for complex reasoning
model = HfApiModel(model_id="anthropic/claude-sonnet-4-20250514")

# Alternative: Use local model via Ollama
# model = HfApiModel(model_id="ollama/duckdb-nsql")

# Create agent with DuckDB tools
agent = CodeAgent(
    tools=[query_duckdb, describe_table, list_parquet_files, get_time_range],
    model=model,
    max_steps=5,
    system_prompt="""You are an NDP Data Analyst agent. You help users explore
    air quality and weather data stored in DuckDB/Parquet files.

    Available data:
    - Bronze layer: Raw Parquet files in /bronze/
    - Air quality: PM2.5, PM10, AQI, temperature, humidity
    - Weather: NWS forecasts, observations
    - Locations: Seattle, Portland

    Always:
    1. First describe tables to understand schema
    2. Use time_bucket() for time-series aggregations
    3. Limit results to prevent context overflow
    4. Explain your SQL queries
    """
)

# Run agent
result = agent.run("What was the average PM2.5 in Seattle last week compared to Portland?")
print(result)
```

### 2.5 Agent Loop Pattern

```
User Query: "Compare air quality between Seattle and Portland last week"
    |
    v
+---------------------------+
| THINK: Need to find tables|
| and understand schema     |
+---------------------------+
    |
    v
+---------------------------+
| ACT: describe_table(      |
|   "air_quality_hourly")   |
+---------------------------+
    |
    v
+---------------------------+
| OBSERVE: Schema shows     |
| timestamp, location_id,   |
| pm2_5, aqi columns        |
+---------------------------+
    |
    v
+---------------------------+
| THINK: Now I can write    |
| the comparison query      |
+---------------------------+
    |
    v
+---------------------------+
| ACT: query_duckdb(        |
|   "SELECT location,       |
|    AVG(pm2_5) ...")       |
+---------------------------+
    |
    v
+---------------------------+
| OBSERVE: Seattle: 42.3    |
| Portland: 38.7            |
+---------------------------+
    |
    v
+---------------------------+
| FINAL ANSWER: Seattle had |
| slightly worse air quality|
| (42.3 vs 38.7 PM2.5)      |
+---------------------------+
```

---

## 3. SQL Pattern Learning

### 3.1 Vector Database for Query Patterns

Store successful queries in a vector database for semantic retrieval:

```python
import duckdb
from sentence_transformers import SentenceTransformer
import numpy as np

# Initialize embedding model
embedder = SentenceTransformer('all-MiniLM-L6-v2')

# Create vector store in DuckDB
conn = duckdb.connect('query_patterns.db')
conn.execute("""
    CREATE TABLE IF NOT EXISTS query_patterns (
        id INTEGER PRIMARY KEY,
        natural_language TEXT,
        sql_query TEXT,
        embedding FLOAT[384],
        success_count INTEGER DEFAULT 1,
        last_used TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
        execution_time_ms FLOAT,
        schema_context TEXT
    )
""")

def store_successful_query(nl_query: str, sql_query: str, exec_time_ms: float, schema: str):
    """Store a successful query pattern for future retrieval."""
    embedding = embedder.encode(nl_query).tolist()

    conn.execute("""
        INSERT INTO query_patterns (natural_language, sql_query, embedding,
                                     execution_time_ms, schema_context)
        VALUES (?, ?, ?, ?, ?)
    """, [nl_query, sql_query, embedding, exec_time_ms, schema])

def find_similar_queries(nl_query: str, top_k: int = 5) -> list:
    """Find similar past queries using vector search."""
    query_embedding = embedder.encode(nl_query).tolist()

    results = conn.execute("""
        SELECT
            natural_language,
            sql_query,
            success_count,
            execution_time_ms,
            array_cosine_similarity(embedding, ?::FLOAT[384]) as similarity
        FROM query_patterns
        WHERE similarity > 0.7
        ORDER BY similarity DESC
        LIMIT ?
    """, [query_embedding, top_k]).fetchall()

    return [
        {
            "nl": row[0],
            "sql": row[1],
            "success_count": row[2],
            "exec_time": row[3],
            "similarity": row[4]
        }
        for row in results
    ]
```

### 3.2 RAG-Enhanced SQL Generation

```python
from smolagents import tool

@tool
def generate_sql_with_examples(user_query: str, schema: str) -> str:
    """
    Generate SQL using RAG with similar past queries.

    Args:
        user_query: Natural language query from user.
        schema: Current database schema context.

    Returns:
        Generated SQL query with confidence score.
    """
    # Find similar successful queries
    similar = find_similar_queries(user_query)

    # Build few-shot prompt
    examples = "\n".join([
        f"Question: {q['nl']}\nSQL: {q['sql']}\n"
        for q in similar[:3]
    ])

    prompt = f"""
    Given this database schema:
    {schema}

    Here are some similar successful queries:
    {examples}

    Now generate SQL for:
    Question: {user_query}

    SQL:
    """

    # Generate with LLM (using smolagents model)
    return generate_sql(prompt)
```

### 3.3 Continuous Learning Pipeline

```yaml
sql_pattern_learning:
  capture_phase:
    trigger: "Successful query execution"
    actions:
      - Extract natural language intent
      - Store SQL with execution metrics
      - Update embedding index

  learning_phase:
    trigger: "Weekly batch job"
    actions:
      - Cluster similar queries
      - Identify common patterns
      - Generate template queries
      - Prune low-success patterns

  retrieval_phase:
    trigger: "New user query"
    actions:
      - Embed user query
      - Vector search for similar patterns
      - Rank by similarity + success rate
      - Provide as few-shot examples

  feedback_phase:
    trigger: "Query success/failure"
    actions:
      - Update success_count for pattern
      - Log execution time
      - Flag problematic patterns
```

### 3.4 DuckDB VSS Extension for Pattern Storage

```sql
-- Install and load VSS extension
INSTALL vss;
LOAD vss;

-- Create HNSW index for fast similarity search
CREATE INDEX query_embedding_idx ON query_patterns
USING HNSW (embedding)
WITH (metric = 'cosine');

-- Fast similarity search with index
SELECT
    natural_language,
    sql_query,
    success_count
FROM query_patterns
ORDER BY array_cosine_distance(embedding, ?::FLOAT[384])
LIMIT 5;
```

---

## 4. Implementation Sketch for NDP

### 4.1 Architecture Overview

```
+------------------------------------------------------------------+
|                        NDP Agent Layer                            |
+------------------------------------------------------------------+
|                                                                   |
|  +----------------+  +------------------+  +-------------------+  |
|  | MCP Server     |  | smolagents       |  | Pattern Learning  |  |
|  | (MotherDuck)   |  | (CodeAgent)      |  | (Vector DB)       |  |
|  +-------+--------+  +--------+---------+  +---------+---------+  |
|          |                    |                      |            |
|          v                    v                      v            |
|  +-------+--------------------+----------------------+---------+  |
|  |                    Query Router                              |  |
|  |  - Schema discovery                                          |  |
|  |  - Query validation                                          |  |
|  |  - Security checks                                           |  |
|  |  - Result pagination                                         |  |
|  +--------------------------------------------------------------+  |
|                              |                                    |
+------------------------------------------------------------------+
                               |
                               v
+------------------------------------------------------------------+
|                        DuckDB Layer                               |
+------------------------------------------------------------------+
|                                                                   |
|  +-----------------+  +------------------+  +-----------------+   |
|  | Bronze Reader   |  | Silver Views     |  | Query Cache     |   |
|  | (Parquet)       |  | (Aggregates)     |  | (LRU)           |   |
|  +-----------------+  +------------------+  +-----------------+   |
|                                                                   |
+------------------------------------------------------------------+
```

### 4.2 Rust Implementation: Query Router

```rust
use duckdb::{Connection, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Security configuration for agent queries
#[derive(Clone, Debug)]
pub struct AgentQueryConfig {
    pub max_rows: usize,
    pub max_execution_time: Duration,
    pub allowed_tables: Vec<String>,
    pub read_only: bool,
}

impl Default for AgentQueryConfig {
    fn default() -> Self {
        Self {
            max_rows: 1000,
            max_execution_time: Duration::from_secs(30),
            allowed_tables: vec![
                "air_quality_hourly".to_string(),
                "nws_forecast".to_string(),
                "sensor_readings".to_string(),
            ],
            read_only: true,
        }
    }
}

/// Query result with metadata
#[derive(Serialize, Deserialize, Debug)]
pub struct AgentQueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<serde_json::Value>>,
    pub row_count: usize,
    pub truncated: bool,
    pub execution_time_ms: u64,
    pub query_hash: String,
}

/// Agent-facing DuckDB query router
pub struct AgentQueryRouter {
    conn: Connection,
    config: AgentQueryConfig,
    query_cache: HashMap<String, AgentQueryResult>,
}

impl AgentQueryRouter {
    pub fn new(db_path: &str, config: AgentQueryConfig) -> Result<Self> {
        let conn = if config.read_only {
            Connection::open_with_flags(db_path, duckdb::Config::default())?
        } else {
            Connection::open(db_path)?
        };

        Ok(Self {
            conn,
            config,
            query_cache: HashMap::new(),
        })
    }

    /// Validate query is safe for agent execution
    fn validate_query(&self, sql: &str) -> std::result::Result<(), String> {
        let sql_upper = sql.to_uppercase();

        // Check for write operations
        if self.config.read_only {
            let forbidden = ["INSERT", "UPDATE", "DELETE", "DROP", "CREATE", "ALTER", "TRUNCATE"];
            for kw in forbidden {
                if sql_upper.contains(kw) {
                    return Err(format!("Write operation '{}' not allowed in read-only mode", kw));
                }
            }
        }

        // Check for dangerous functions
        let dangerous = ["COPY", "EXPORT", "ATTACH", "LOAD"];
        for kw in dangerous {
            if sql_upper.contains(kw) {
                return Err(format!("Operation '{}' not allowed for agent queries", kw));
            }
        }

        Ok(())
    }

    /// Execute a query with agent-safe constraints
    pub fn execute_agent_query(&mut self, sql: &str) -> std::result::Result<AgentQueryResult, String> {
        // Validate query
        self.validate_query(sql)?;

        // Check cache
        let query_hash = format!("{:x}", md5::compute(sql));
        if let Some(cached) = self.query_cache.get(&query_hash) {
            return Ok(cached.clone());
        }

        // Add LIMIT if not present
        let limited_sql = if !sql.to_uppercase().contains("LIMIT") {
            format!("{} LIMIT {}", sql.trim_end_matches(';'), self.config.max_rows)
        } else {
            sql.to_string()
        };

        // Execute with timeout
        let start = Instant::now();
        let mut stmt = self.conn.prepare(&limited_sql)
            .map_err(|e| format!("Query preparation failed: {}", e))?;

        let column_count = stmt.column_count();
        let columns: Vec<String> = (0..column_count)
            .map(|i| stmt.column_name(i).unwrap_or("?").to_string())
            .collect();

        let mut rows: Vec<Vec<serde_json::Value>> = Vec::new();
        let mut row_iter = stmt.query([])
            .map_err(|e| format!("Query execution failed: {}", e))?;

        while let Some(row) = row_iter.next()
            .map_err(|e| format!("Error fetching row: {}", e))?
        {
            if rows.len() >= self.config.max_rows {
                break;
            }

            let mut row_values: Vec<serde_json::Value> = Vec::new();
            for i in 0..column_count {
                let value: duckdb::types::Value = row.get(i)
                    .map_err(|e| format!("Error reading column {}: {}", i, e))?;
                row_values.push(value_to_json(value));
            }
            rows.push(row_values);

            // Check timeout
            if start.elapsed() > self.config.max_execution_time {
                return Err("Query execution timeout".to_string());
            }
        }

        let result = AgentQueryResult {
            columns,
            row_count: rows.len(),
            truncated: rows.len() >= self.config.max_rows,
            rows,
            execution_time_ms: start.elapsed().as_millis() as u64,
            query_hash: query_hash.clone(),
        };

        // Cache result
        self.query_cache.insert(query_hash, result.clone());

        Ok(result)
    }

    /// Get schema information for agent
    pub fn get_schema(&self) -> std::result::Result<String, String> {
        let schema_sql = r#"
            SELECT
                table_name,
                column_name,
                data_type
            FROM information_schema.columns
            WHERE table_schema = 'main'
            ORDER BY table_name, ordinal_position
        "#;

        let mut stmt = self.conn.prepare(schema_sql)
            .map_err(|e| format!("Schema query failed: {}", e))?;

        let mut result = String::new();
        result.push_str("<schema>\n");

        let mut current_table = String::new();
        let mut rows = stmt.query([])
            .map_err(|e| format!("Schema fetch failed: {}", e))?;

        while let Some(row) = rows.next().map_err(|e| e.to_string())? {
            let table: String = row.get(0).unwrap_or_default();
            let column: String = row.get(1).unwrap_or_default();
            let dtype: String = row.get(2).unwrap_or_default();

            if table != current_table {
                if !current_table.is_empty() {
                    result.push_str("  </table>\n");
                }
                result.push_str(&format!("  <table name=\"{}\">\n", table));
                current_table = table;
            }
            result.push_str(&format!("    <column name=\"{}\" type=\"{}\"/>\n", column, dtype));
        }

        if !current_table.is_empty() {
            result.push_str("  </table>\n");
        }
        result.push_str("</schema>");

        Ok(result)
    }
}

fn value_to_json(value: duckdb::types::Value) -> serde_json::Value {
    match value {
        duckdb::types::Value::Null => serde_json::Value::Null,
        duckdb::types::Value::Boolean(b) => serde_json::Value::Bool(b),
        duckdb::types::Value::Int(i) => serde_json::json!(i),
        duckdb::types::Value::BigInt(i) => serde_json::json!(i),
        duckdb::types::Value::Double(f) => serde_json::json!(f),
        duckdb::types::Value::Text(s) => serde_json::Value::String(s),
        _ => serde_json::Value::String(format!("{:?}", value)),
    }
}
```

### 4.3 Python Agent Service

```python
#!/usr/bin/env python3
"""
NDP DuckDB Agent Service

Exposes DuckDB to AI agents via MCP and smolagents.
"""

from fastapi import FastAPI, HTTPException
from pydantic import BaseModel
from smolagents import tool, CodeAgent, HfApiModel
import duckdb
import hashlib
from functools import lru_cache
from typing import Optional
import logging

app = FastAPI(title="NDP DuckDB Agent Service")
logger = logging.getLogger(__name__)

# Configuration
DUCKDB_PATH = "/workspaces/neural-data-platform/bronze/duckdb/analytics.db"
MAX_ROWS = 1000
MAX_EXECUTION_TIME_SECONDS = 30

# Connection pool
@lru_cache(maxsize=1)
def get_connection():
    return duckdb.connect(DUCKDB_PATH, read_only=True)

# Request/Response models
class QueryRequest(BaseModel):
    sql: str
    context: Optional[str] = None

class NLQueryRequest(BaseModel):
    question: str
    include_schema: bool = True

class QueryResponse(BaseModel):
    columns: list[str]
    rows: list[list]
    row_count: int
    truncated: bool
    execution_time_ms: float

# Security validation
FORBIDDEN_KEYWORDS = ['INSERT', 'UPDATE', 'DELETE', 'DROP', 'CREATE',
                       'ALTER', 'TRUNCATE', 'COPY', 'EXPORT', 'ATTACH', 'LOAD']

def validate_query(sql: str) -> None:
    sql_upper = sql.upper()
    for kw in FORBIDDEN_KEYWORDS:
        if kw in sql_upper:
            raise HTTPException(
                status_code=400,
                detail=f"Operation '{kw}' not allowed"
            )

# smolagents tools
@tool
def query_duckdb(sql: str) -> str:
    """
    Execute a read-only SQL query against the NDP DuckDB database.

    Args:
        sql: A valid DuckDB SQL query. Must be read-only (SELECT only).

    Returns:
        Query results as formatted text, limited to 1000 rows.

    Example:
        query_duckdb("SELECT AVG(pm2_5) FROM air_quality_hourly WHERE city = 'Seattle'")
    """
    try:
        validate_query(sql)
        conn = get_connection()

        # Add limit if not present
        if 'LIMIT' not in sql.upper():
            sql = f"{sql.rstrip(';')} LIMIT {MAX_ROWS}"

        result = conn.execute(sql).fetchdf()
        return result.to_string(max_rows=100)
    except Exception as e:
        return f"Query error: {str(e)}"

@tool
def describe_parquet(path: str) -> str:
    """
    Describe schema and sample data from a Parquet file.

    Args:
        path: Path to Parquet file or glob pattern.

    Returns:
        Schema information and sample rows.
    """
    try:
        conn = get_connection()
        schema = conn.execute(f"DESCRIBE SELECT * FROM read_parquet('{path}')").fetchdf()
        sample = conn.execute(f"SELECT * FROM read_parquet('{path}') LIMIT 5").fetchdf()
        return f"Schema:\n{schema.to_string()}\n\nSample:\n{sample.to_string()}"
    except Exception as e:
        return f"Error: {str(e)}"

@tool
def get_available_tables() -> str:
    """
    List all available tables and views in the NDP database.

    Returns:
        Table names with row counts and descriptions.
    """
    try:
        conn = get_connection()
        result = conn.execute("""
            SELECT
                table_name,
                table_type
            FROM information_schema.tables
            WHERE table_schema = 'main'
            ORDER BY table_name
        """).fetchdf()
        return result.to_string()
    except Exception as e:
        return f"Error: {str(e)}"

# Create agent
def create_agent():
    model = HfApiModel(model_id="anthropic/claude-sonnet-4-20250514")
    return CodeAgent(
        tools=[query_duckdb, describe_parquet, get_available_tables],
        model=model,
        max_steps=5,
        system_prompt="""You are an NDP Data Analyst. You help users explore air quality
        and weather data. The data is stored in DuckDB/Parquet format.

        Always:
        1. First check available tables
        2. Describe schema before writing complex queries
        3. Use time_bucket() for time-series aggregations
        4. Explain your findings clearly
        """
    )

# API Endpoints
@app.post("/query", response_model=QueryResponse)
async def execute_query(request: QueryRequest):
    """Execute a SQL query against DuckDB."""
    import time

    validate_query(request.sql)
    conn = get_connection()

    sql = request.sql
    if 'LIMIT' not in sql.upper():
        sql = f"{sql.rstrip(';')} LIMIT {MAX_ROWS}"

    start = time.time()
    result = conn.execute(sql).fetchdf()
    execution_time = (time.time() - start) * 1000

    return QueryResponse(
        columns=list(result.columns),
        rows=result.values.tolist(),
        row_count=len(result),
        truncated=len(result) >= MAX_ROWS,
        execution_time_ms=execution_time
    )

@app.post("/nl-query")
async def natural_language_query(request: NLQueryRequest):
    """Execute a natural language query using the agent."""
    agent = create_agent()
    result = agent.run(request.question)
    return {"answer": result}

@app.get("/schema")
async def get_schema():
    """Get database schema as XML for LLM context."""
    conn = get_connection()
    result = conn.execute("""
        SELECT
            table_name,
            column_name,
            data_type
        FROM information_schema.columns
        WHERE table_schema = 'main'
        ORDER BY table_name, ordinal_position
    """).fetchdf()

    # Convert to XML format (best for LLM consumption)
    xml = "<schema>\n"
    current_table = ""
    for _, row in result.iterrows():
        if row['table_name'] != current_table:
            if current_table:
                xml += "  </table>\n"
            xml += f"  <table name=\"{row['table_name']}\">\n"
            current_table = row['table_name']
        xml += f"    <column name=\"{row['column_name']}\" type=\"{row['data_type']}\"/>\n"
    if current_table:
        xml += "  </table>\n"
    xml += "</schema>"

    return {"schema_xml": xml}

@app.get("/health")
async def health_check():
    """Health check endpoint."""
    try:
        conn = get_connection()
        conn.execute("SELECT 1").fetchone()
        return {"status": "healthy", "database": "connected"}
    except Exception as e:
        return {"status": "unhealthy", "error": str(e)}

if __name__ == "__main__":
    import uvicorn
    uvicorn.run(app, host="0.0.0.0", port=8765)
```

### 4.4 Security: Read-Only, Sandboxed Queries

**Security Layers:**

| Layer | Implementation | Protection |
|-------|---------------|------------|
| Query Validation | Keyword blocking | Prevents write operations |
| Connection | `read_only=True` | DuckDB-level enforcement |
| Result Limiting | `LIMIT` injection | Prevents context overflow |
| Timeout | Execution time limit | Prevents DoS |
| File Access | `--saas-mode` | Restricts filesystem |
| Sandboxing | Docker/E2B/Modal | Process isolation |

**Prepared Statements for Safety:**

```python
# SAFE: Parameterized query
def safe_query(location: str, start_date: str) -> pd.DataFrame:
    conn = get_connection()
    return conn.execute("""
        SELECT * FROM air_quality_hourly
        WHERE location = $1
        AND timestamp >= $2::TIMESTAMP
        LIMIT 1000
    """, [location, start_date]).fetchdf()

# UNSAFE: String concatenation (DO NOT USE)
def unsafe_query(location: str):
    # SQL INJECTION RISK!
    return conn.execute(f"SELECT * FROM air_quality WHERE location = '{location}'")
```

### 4.5 Performance: Query Caching and Pagination

```python
from functools import lru_cache
import hashlib
from typing import Optional
import redis

# Redis cache for query results
redis_client = redis.Redis(host='localhost', port=6379, db=0)
CACHE_TTL = 300  # 5 minutes

def cache_key(sql: str) -> str:
    return f"duckdb:query:{hashlib.sha256(sql.encode()).hexdigest()}"

def cached_query(sql: str) -> Optional[dict]:
    """Check cache for query results."""
    key = cache_key(sql)
    cached = redis_client.get(key)
    if cached:
        return json.loads(cached)
    return None

def cache_result(sql: str, result: dict):
    """Store query result in cache."""
    key = cache_key(sql)
    redis_client.setex(key, CACHE_TTL, json.dumps(result))

# Pagination support
class PaginatedQuery:
    def __init__(self, sql: str, page_size: int = 100):
        self.sql = sql
        self.page_size = page_size
        self.current_page = 0

    def next_page(self) -> dict:
        offset = self.current_page * self.page_size
        paginated_sql = f"""
            {self.sql.rstrip(';')}
            LIMIT {self.page_size}
            OFFSET {offset}
        """
        result = execute_query(paginated_sql)
        self.current_page += 1
        return {
            "page": self.current_page,
            "page_size": self.page_size,
            "has_more": len(result["rows"]) == self.page_size,
            "data": result
        }
```

---

## 5. Integration with NDP Architecture

### 5.1 How NDP Exposes DuckDB to Agents

```yaml
ndp_agent_integration:
  bronze_layer:
    access_pattern: "Direct Parquet read via DuckDB"
    use_cases:
      - Raw data exploration
      - Schema discovery
      - Data quality investigation
    example_query: |
      SELECT * FROM read_parquet('/bronze/air-quality/*.parquet')
      WHERE timestamp > NOW() - INTERVAL '24 hours'
      LIMIT 100

  silver_layer:
    access_pattern: "DuckDB views over aggregated data"
    use_cases:
      - Time-series analytics
      - Trend analysis
      - Comparison queries
    example_query: |
      SELECT
        time_bucket('1 hour', timestamp) as hour,
        location,
        AVG(pm2_5) as avg_pm2_5
      FROM air_quality_hourly
      WHERE timestamp > NOW() - INTERVAL '7 days'
      GROUP BY 1, 2
      ORDER BY 1, 2

  agent_interfaces:
    mcp_server:
      port: 8766
      protocol: MCP (stdio/SSE)
      capabilities: [query, schema]

    rest_api:
      port: 8765
      protocol: HTTP/REST
      endpoints:
        - POST /query
        - POST /nl-query
        - GET /schema
        - GET /health

    smolagents:
      integration: "Python library"
      tools: [query_duckdb, describe_parquet, get_available_tables]
```

### 5.2 Deployment Configuration

```yaml
# docker-compose.yml addition for agent services
services:
  ndp-duckdb-agent:
    build:
      context: ./agents/duckdb
      dockerfile: Dockerfile
    ports:
      - "8765:8765"  # REST API
      - "8766:8766"  # MCP Server
    volumes:
      - ./bronze:/bronze:ro
      - ./silver:/silver:ro
    environment:
      - DUCKDB_PATH=/bronze/duckdb/analytics.db
      - MAX_ROWS=1000
      - MAX_EXECUTION_TIME_SECONDS=30
      - ANTHROPIC_API_KEY=${ANTHROPIC_API_KEY}
    deploy:
      resources:
        limits:
          memory: 2G
          cpus: '2'
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8765/health"]
      interval: 30s
      timeout: 10s
      retries: 3
```

### 5.3 MCP Configuration for Claude Desktop

```json
{
  "mcpServers": {
    "ndp-duckdb": {
      "command": "docker",
      "args": [
        "run", "-i", "--rm",
        "-v", "/workspaces/neural-data-platform/bronze:/bronze:ro",
        "ndp-duckdb-mcp:latest",
        "--db-path", "/bronze/duckdb/analytics.db",
        "--read-only",
        "--max-rows", "500"
      ]
    }
  }
}
```

---

## 6. Comparison with Alternatives

| Feature | MCP Server | smolagents | DuckDB-NSQL | Vanna.ai |
|---------|------------|------------|-------------|----------|
| **Complexity** | Low | Medium | Low | Medium |
| **Self-Hosted** | Yes | Yes | Yes | Yes/No |
| **LLM Agnostic** | No (MCP clients) | Yes | No (specific model) | Partial |
| **Custom Tools** | No | Yes | No | Limited |
| **Self-Correcting** | Limited | Yes | No | Yes |
| **Pattern Learning** | No | Yes (with code) | No | Yes |
| **Security** | Good | Excellent | Limited | Good |
| **NDP Fit** | Excellent | Excellent | Good | Good |

**Recommendation for NDP:**

1. **Phase 1**: Deploy MCP Server for immediate Claude/Cursor integration
2. **Phase 2**: Add smolagents for custom tools and self-correcting workflows
3. **Phase 3**: Implement SQL pattern learning with DuckDB VSS extension
4. **Phase 4**: Fine-tune DuckDB-NSQL for NDP-specific queries

---

## 7. References

### Official Documentation
- [MotherDuck MCP Server GitHub](https://github.com/motherduckdb/mcp-server-motherduck)
- [smolagents Documentation](https://huggingface.co/docs/smolagents/en/index)
- [DuckDB Vector Similarity Search](https://duckdb.org/2024/05/03/vector-similarity-search-vss)
- [DuckDB Information Schema](https://duckdb.org/docs/stable/sql/meta/information_schema)
- [DuckDB Security Guide](https://duckdb.org/docs/stable/operations_manual/securing_duckdb/overview)

### Tutorials and Guides
- [Agentic AI with DuckDB and smolagents](https://buckenhofer.com/2025/11/agentic-ai-with-duckdb-and-smolagents-natural-language-queries-for-analytics/)
- [Close the Loop: Faster Data Pipelines with MCP, DuckDB & AI](https://motherduck.com/blog/faster-data-pipelines-with-mcp-duckdb-ai/)
- [Self-Correcting SQL with Cursor + MotherDuck](https://motherduck.com/blog/vibe-coding-sql-cursor/)
- [Teaching LLMs About DuckDB](https://motherduck.com/blog/fix-outdated-llm-documentation-duckdb/)
- [Search in DuckDB: Embedding Methods](https://motherduck.com/blog/search-using-duckdb-part-3/)

### Models and Tools
- [DuckDB-NSQL Model (Ollama)](https://ollama.com/library/duckdb-nsql)
- [DuckDB-NSQL GitHub](https://github.com/NumbersStationAI/DuckDB-NSQL)
- [MotherDuck FixIt AI SQL Error Fixer](https://motherduck.com/blog/introducing-fixit-ai-sql-error-fixer/)

### Research Papers
- [A Lightweight Local SQL Agent Using LLMs and DuckDB](https://www.techrxiv.org/users/930000/articles/1308180-a-lightweight-local-sql-agent-using-llms-and-duckdb-for-business-analytics)

---

**Research Completed**: 2026-01-03
**Document Version**: 1.0
**Next Steps**: Implement MCP Server integration for NDP Bronze/Silver layer access
