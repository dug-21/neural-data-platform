# ADR-005: MCP Response Format

## Status

Accepted

## Date

2026-01-03

## Context

The dp-005 Bronze MCP Server must return tool results in a format that:

1. Conforms to MCP specification
2. Enables Claude (and other AI agents) to understand results
3. Clearly distinguishes success from error
4. Provides structured data for programmatic use
5. Includes human-readable context

### MCP Specification Requirements

From the MCP specification, tool call results must return:

```json
{
  "content": [
    {
      "type": "text",
      "text": "The actual content as a string"
    }
  ],
  "isError": true  // Optional, present only on errors
}
```

The `content` array can contain multiple items of different types (text, image, etc.), but for this server we only use text.

### Requirements

| Requirement | Priority | Notes |
|-------------|----------|-------|
| MCP spec compliance | Must | `content[]` array structure |
| JSON in text content | Must | Structured, parseable results |
| Clear success/error | Must | Distinguishable at a glance |
| Actionable errors | Should | What went wrong, how to fix |
| Consistent structure | Should | Same patterns across all tools |

## Decision

**Use JSON with a `success` flag inside the text content, plus `isError` at the MCP level for error responses.**

### Response Structure

#### Success Response

```json
{
  "content": [
    {
      "type": "text",
      "text": "{\"success\": true, \"data\": {<tool-specific-result>}}"
    }
  ]
}
```

#### Error Response

```json
{
  "content": [
    {
      "type": "text",
      "text": "{\"success\": false, \"error\": \"<error-message>\", \"details\": {<optional-context>}}"
    }
  ],
  "isError": true
}
```

### Standard Response Types

```rust
use serde::{Deserialize, Serialize};

/// Wrapper for all tool responses
#[derive(Serialize)]
#[serde(untagged)]
pub enum ToolResponse {
    Success(SuccessResponse),
    Error(ErrorResponse),
}

#[derive(Serialize)]
pub struct SuccessResponse {
    pub success: bool,  // Always true
    #[serde(flatten)]
    pub data: serde_json::Value,
}

#[derive(Serialize)]
pub struct ErrorResponse {
    pub success: bool,  // Always false
    pub error: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

impl ToolResponse {
    pub fn success(data: impl Serialize) -> Self {
        ToolResponse::Success(SuccessResponse {
            success: true,
            data: serde_json::to_value(data).unwrap(),
        })
    }

    pub fn error(message: impl Into<String>) -> Self {
        ToolResponse::Error(ErrorResponse {
            success: false,
            error: message.into(),
            details: None,
        })
    }

    pub fn error_with_details(message: impl Into<String>, details: impl Serialize) -> Self {
        ToolResponse::Error(ErrorResponse {
            success: false,
            error: message.into(),
            details: Some(serde_json::to_value(details).unwrap()),
        })
    }
}
```

### MCP Response Builder

```rust
use serde::Serialize;

#[derive(Serialize)]
pub struct McpContent {
    #[serde(rename = "type")]
    pub content_type: String,
    pub text: String,
}

#[derive(Serialize)]
pub struct McpResponse {
    pub content: Vec<McpContent>,
    #[serde(rename = "isError", skip_serializing_if = "std::ops::Not::not")]
    pub is_error: bool,
}

impl McpResponse {
    pub fn from_tool_response(response: ToolResponse) -> Self {
        let is_error = matches!(response, ToolResponse::Error(_));
        let text = serde_json::to_string(&response).unwrap();

        McpResponse {
            content: vec![McpContent {
                content_type: "text".to_string(),
                text,
            }],
            is_error,
        }
    }
}
```

### Tool-Specific Response Examples

#### list_streams

```json
{
  "content": [{
    "type": "text",
    "text": "{\"success\":true,\"streams\":[{\"stream_id\":\"air-quality\",\"description\":\"Indoor air quality from AirGradient\",\"enabled\":true,\"version\":\"1.0.0\",\"sources\":[\"mqtt\"],\"storage\":{\"latest_partition\":\"year=2026/month=01/day=03\",\"file_size_bytes\":7310,\"file_modified\":\"2026-01-03T14:54:00Z\"}},{\"stream_id\":\"outdoor-weather\",\"description\":\"Weather from OpenWeatherMap\",\"enabled\":true,\"version\":\"1.0.0\",\"sources\":[\"http_poll\"],\"storage\":{\"latest_partition\":\"year=2026/month=01/day=03\",\"file_size_bytes\":12450,\"file_modified\":\"2026-01-03T15:00:00Z\"}}]}"
  }]
}
```

#### describe_schema (success)

```json
{
  "content": [{
    "type": "text",
    "text": "{\"success\":true,\"stream_id\":\"outdoor-weather\",\"mode\":\"all\",\"source\":{\"raw_payload_structure\":{\"keys\":[\"base\",\"clouds\",\"main\",\"wind\"],\"nested\":{\"main\":[\"temp\",\"humidity\",\"pressure\"],\"wind\":[\"speed\",\"deg\"]}},\"field_mappings\":[{\"source_path\":\"main.temp\",\"target_field\":\"temperature\",\"unit\":\"celsius\"}]},\"target\":{\"entity_schema\":\"nws-weather\",\"attributes\":[{\"name\":\"temperature\",\"type\":\"float\",\"unit\":\"celsius\"}]},\"gap_analysis\":{\"unmapped_source_fields\":[\"base\",\"clouds\"],\"target_fields_without_mapping\":[\"rain_1h\"]}}"
  }]
}
```

#### sample_data (success)

```json
{
  "content": [{
    "type": "text",
    "text": "{\"success\":true,\"stream_id\":\"outdoor-weather\",\"row_count\":3,\"rows\":[{\"timestamp\":1767452639760716,\"source_id\":\"outdoor-weather-Http\",\"ndp_id\":\"weather-owm-002\",\"context\":{\"location\":{\"coordinates\":[29.95838,-81.30878]}},\"raw_payload\":{\"main\":{\"temp\":19.72,\"humidity\":76}}}],\"source_file\":\"/data/raw/outdoor-weather/year=2026/month=01/day=03/data.parquet\"}"
  }]
}
```

#### validate_config (mismatch found)

```json
{
  "content": [{
    "type": "text",
    "text": "{\"success\":true,\"stream_id\":\"outdoor-weather\",\"entity_schema\":\"nws-weather\",\"validation\":{\"status\":\"mismatch\",\"config_fields\":[\"temperature\",\"humidity\",\"rain_1h\"],\"raw_payload_fields\":[\"main\",\"wind\",\"clouds\"],\"analysis\":{\"in_config_not_in_payload\":[\"rain_1h\"],\"in_payload_not_in_config\":[\"clouds\"],\"matching\":[\"temperature\",\"humidity\"]},\"notes\":\"Config uses flattened names; raw_payload has nested structure. Mapping happens in Silver layer.\"}}"
  }]
}
```

#### Error Response Example

```json
{
  "content": [{
    "type": "text",
    "text": "{\"success\":false,\"error\":\"Stream 'nonexistent' not found\",\"details\":{\"available_streams\":[\"air-quality\",\"outdoor-weather\",\"outdoor-air-quality\"],\"suggestion\":\"Use list_streams to see available streams\"}}"
  }],
  "isError": true
}
```

### Error Categories

| Error Type | Error Message Pattern | Details Included |
|------------|----------------------|------------------|
| Stream not found | "Stream '{id}' not found" | available_streams, suggestion |
| No data available | "Stream '{id}' has no data" | suggestion to check ingestion |
| Config unavailable | "Configuration unavailable: {reason}" | etcd_status |
| Storage error | "Storage error: {reason}" | path, error_code |
| Invalid parameter | "Invalid parameter '{name}': {reason}" | expected_type, received |

## Consequences

### Positive

1. **MCP compliant**: Follows specification exactly
2. **Parseable**: JSON in text allows structured extraction
3. **Clear errors**: `isError` flag + `success: false` + message
4. **Actionable**: Error details include suggestions
5. **Consistent**: Same pattern across all tools
6. **Forward compatible**: Can add fields without breaking clients

### Negative

1. **Double serialization**: JSON in JSON (MCP requirement)
   - Mitigation: Single serialization, MCP wrapper is thin

2. **Verbose**: More bytes than minimal response
   - Mitigation: Acceptable for tool calls (not streaming data)

3. **Pretty printing trade-off**: Compact JSON harder to read
   - Mitigation: Claude handles JSON well; logs can pretty-print

### Why success Flag Inside JSON

The MCP `isError` flag is at the protocol level, but we also include `success` in the JSON because:

1. **Explicit contract**: Clear boolean for programmatic checks
2. **Context preservation**: Error details stay with the error
3. **Logging**: Single JSON captures full result
4. **Symmetry**: Both success and error have consistent structure

## Alternatives Considered

### Alternative 1: Plain Text Responses

**How it works**: Return human-readable text, not JSON.

```json
{
  "content": [{"type": "text", "text": "Found 3 streams: air-quality, outdoor-weather..."}]
}
```

**Rejected because**:
- Harder to parse programmatically
- Claude can handle JSON
- Less precise for complex data

### Alternative 2: Multiple Content Items

**How it works**: Use multiple content array items for different parts.

```json
{
  "content": [
    {"type": "text", "text": "Success"},
    {"type": "text", "text": "{\"data\": {...}}"}
  ]
}
```

**Rejected because**:
- Splits related information
- More complex to handle
- No clear benefit

### Alternative 3: Rich Content Types

**How it works**: Use markdown, tables, or other formats.

```json
{
  "content": [
    {"type": "text", "text": "| Stream | Status |\n|--------|--------|\n| air | ok |"}
  ]
}
```

**Rejected for MVP because**:
- JSON is more machine-friendly
- Markdown can be added later if needed
- Current tools benefit from structured data

### Alternative 4: No success Flag

**How it works**: Rely only on MCP `isError` flag.

**Rejected because**:
- isError missing on success (less explicit)
- Error details would be unstructured
- Harder to distinguish success patterns

## Implementation Notes

### Handler Pattern

```rust
async fn handle_list_streams(
    storage: &dyn BronzeStorage,
    config: &dyn ConfigStore,
) -> McpResponse {
    match list_streams_impl(storage, config).await {
        Ok(streams) => McpResponse::from_tool_response(
            ToolResponse::success(ListStreamsResult { streams })
        ),
        Err(e) => McpResponse::from_tool_response(
            ToolResponse::error_with_details(
                e.user_message(),
                e.details(),
            )
        ),
    }
}
```

### JSON Formatting

For logging/debugging, pretty-print:

```rust
fn format_for_log(response: &ToolResponse) -> String {
    serde_json::to_string_pretty(response).unwrap()
}
```

For wire format, compact:

```rust
fn format_for_wire(response: &ToolResponse) -> String {
    serde_json::to_string(response).unwrap()
}
```

## Related Decisions

- [ADR-001: MCP Transport](./ADR-001-mcp-transport.md) - How responses are delivered
- [ADR-002: Storage Abstraction](./ADR-002-storage-abstraction.md) - What data is returned
- [ADR-004: Schema Discovery](./ADR-004-schema-discovery.md) - Schema response content

## References

- [MCP Specification - Tool Results](https://modelcontextprotocol.io/specification/2025-11-25#tool-results)
- [JSON-RPC 2.0 Specification](https://www.jsonrpc.org/specification)
- [NDP Error Handling](../../../core/src/error.rs) - CoreError patterns
