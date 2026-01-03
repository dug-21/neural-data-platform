# MCP Design Patterns - Reference Analysis

**Source**: https://gist.github.com/ruvnet/ea1ec6678b1552c3ff3ae92dc1001d23
**Language**: JavaScript/Node.js
**Framework**: Express.js + @modelcontextprotocol/sdk

---

## Patterns to Adopt

### 1. Tool Definition Structure

```javascript
{
  name: 'tool_name',
  description: 'Clear description of what the tool does',
  inputSchema: {
    type: 'object',
    properties: {
      required_field: { type: 'string', description: '...' },
      optional_field: { type: 'integer', description: '...', default: 10 }
    },
    required: ['required_field']
  }
}
```

**Adopt**: JSON Schema for input validation, clear descriptions, explicit required array.

### 2. Request Handler Pattern

```javascript
server.setRequestHandler('tools/call', async (request) => {
  const { name, arguments: args } = request.params;
  switch (name) {
    case 'tool_name': {
      // Validate required args
      if (!args.required_field) {
        throw new Error('required_field is required');
      }
      // Process and return
      return { content: [{ type: 'text', text: JSON.stringify(result) }] };
    }
    default:
      throw new Error(`Unknown tool: ${name}`);
  }
});
```

**Adopt**: Switch-based routing, early validation, consistent error format.

### 3. Response Format

**Success:**
```json
{
  "content": [{
    "type": "text",
    "text": "{\"success\": true, \"data\": {...}}"
  }]
}
```

**Error:**
```json
{
  "content": [{
    "type": "text",
    "text": "{\"success\": false, \"error\": \"descriptive message\"}"
  }],
  "isError": true
}
```

**Adopt**: Wrap response in JSON with success flag, structured error messages.

### 4. Dual Transport Mode

```javascript
// stdio mode (default)
const transport = new StdioServerTransport();
await server.connect(transport);

// HTTP/SSE mode
const app = express();
app.get('/health', (req, res) => res.json({ status: 'ok', version: '1.0.0' }));
app.get('/sse', async (req, res) => { /* SSE setup */ });
app.post('/message', async (req, res) => { /* handle MCP messages */ });
```

**Adopt**: Health endpoint. **Defer**: SSE mode (HTTP POST sufficient for MVP).

### 5. Configuration via Environment

```javascript
const config = {
  depth: parseInt(process.env.RESEARCH_DEPTH) || 5,
  timeBudget: parseInt(process.env.TIME_BUDGET) || 30,
};
```

**Adopt**: All tunables from environment, sensible defaults.

---

## Patterns to Avoid

### 1. Synchronous Blocking

The reference uses `child_process.spawn()` with detached mode to avoid blocking. In Rust, we'll use `tokio` async throughout.

### 2. Raw Error Exposure

```javascript
// Avoid: exposing internal errors
catch (error) {
  return { error: error.stack }; // Leaks internals
}

// Better: structured error response
catch (error) {
  return {
    content: [{ type: 'text', text: JSON.stringify({
      success: false,
      error: 'Failed to process request',
      code: 'INTERNAL_ERROR'
    })}],
    isError: true
  };
}
```

### 3. Hardcoded Values

Reference has some hardcoded defaults. We'll externalize all configuration.

---

## Rust Translation

### Tool Registry Pattern

```rust
pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn Tool>>,
}

#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn input_schema(&self) -> serde_json::Value;
    async fn execute(&self, args: serde_json::Value) -> Result<ToolResult>;
}

impl ToolRegistry {
    pub fn register<T: Tool + 'static>(&mut self, tool: T) {
        self.tools.insert(tool.name().to_string(), Box::new(tool));
    }

    pub async fn call(&self, name: &str, args: Value) -> Result<ToolResult> {
        let tool = self.tools.get(name)
            .ok_or_else(|| Error::UnknownTool(name.to_string()))?;
        tool.execute(args).await
    }

    pub fn list(&self) -> Vec<ToolDefinition> {
        self.tools.values()
            .map(|t| ToolDefinition {
                name: t.name().to_string(),
                description: t.description().to_string(),
                input_schema: t.input_schema(),
            })
            .collect()
    }
}
```

### MCP Handler Pattern

```rust
async fn handle_mcp_request(
    State(state): State<AppState>,
    Json(request): Json<McpRequest>,
) -> Json<McpResponse> {
    match request.method.as_str() {
        "tools/list" => {
            let tools = state.registry.list();
            Json(McpResponse::success(tools))
        }
        "tools/call" => {
            let name = request.params["name"].as_str().unwrap_or("");
            let args = request.params["arguments"].clone();

            match state.registry.call(name, args).await {
                Ok(result) => Json(McpResponse::success(result)),
                Err(e) => Json(McpResponse::error(e.to_string())),
            }
        }
        _ => Json(McpResponse::error("Unknown method")),
    }
}
```

### Response Types

```rust
#[derive(Serialize)]
pub struct McpResponse {
    pub content: Vec<ContentBlock>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
}

#[derive(Serialize)]
pub struct ContentBlock {
    #[serde(rename = "type")]
    pub content_type: String,
    pub text: String,
}

impl McpResponse {
    pub fn success<T: Serialize>(data: T) -> Self {
        Self {
            content: vec![ContentBlock {
                content_type: "text".to_string(),
                text: serde_json::to_string(&SuccessPayload { success: true, data }).unwrap(),
            }],
            is_error: None,
        }
    }

    pub fn error(message: String) -> Self {
        Self {
            content: vec![ContentBlock {
                content_type: "text".to_string(),
                text: serde_json::to_string(&ErrorPayload { success: false, error: message }).unwrap(),
            }],
            is_error: Some(true),
        }
    }
}
```

---

## Key Takeaways for dp-005

1. **Tool abstraction** - Trait-based tool registry for extensibility
2. **Consistent responses** - Always JSON with success flag
3. **Health endpoint** - Essential for monitoring/load balancers
4. **Async throughout** - tokio for non-blocking I/O
5. **Config from env** - No hardcoded values
6. **HTTP POST for MVP** - SSE optional enhancement later

---

*Analyzed from reference implementation for Rust translation*
