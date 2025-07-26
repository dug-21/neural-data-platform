# REST API Endpoints

## Overview

This document describes the REST API endpoints for monitoring and controlling backfill operations. The API is exposed when running the backfill service with the `--api` flag.

## Base URL

```
http://localhost:8080/api/v1
```

## Authentication

All endpoints require API key authentication:

```http
Authorization: Bearer <API_KEY>
```

## Endpoints

### Health Check

#### `GET /health`

Check service health status.

**Response:**
```json
{
  "status": "healthy",
  "version": "1.0.0",
  "uptime": 3600,
  "timestamp": "2023-07-24T10:00:00Z"
}
```

### Operations

#### `GET /operations`

List all backfill operations.

**Query Parameters:**
- `status` (string): Filter by status (pending, running, completed, failed)
- `symbol` (string): Filter by symbol
- `limit` (integer): Max results (default: 100)
- `offset` (integer): Pagination offset

**Response:**
```json
{
  "operations": [
    {
      "id": "op_123abc",
      "status": "running",
      "type": "s3_download",
      "symbols": ["AAPL", "MSFT"],
      "start_date": "2023-01-01",
      "end_date": "2023-12-31",
      "created_at": "2023-07-24T09:00:00Z",
      "started_at": "2023-07-24T09:01:00Z",
      "progress": {
        "percentage": 45.5,
        "files_completed": 165,
        "files_total": 365,
        "records_processed": 1234567
      }
    }
  ],
  "total": 42,
  "limit": 100,
  "offset": 0
}
```

#### `POST /operations`

Create a new backfill operation.

**Request Body:**
```json
{
  "type": "s3_download",
  "source": {
    "profile": "polygon-s3",
    "prefix": "us_stocks_sip/day_aggs_v1/"
  },
  "symbols": ["AAPL", "MSFT", "GOOGL"],
  "date_range": {
    "start": "2023-01-01",
    "end": "2023-12-31"
  },
  "options": {
    "batch_size": 10000,
    "max_workers": 10,
    "checkpoint": true
  }
}
```

**Response:**
```json
{
  "id": "op_456def",
  "status": "pending",
  "created_at": "2023-07-24T10:00:00Z",
  "estimated_duration": 7200,
  "estimated_size_gb": 125.5
}
```

#### `GET /operations/{id}`

Get operation details.

**Response:**
```json
{
  "id": "op_123abc",
  "status": "running",
  "type": "s3_download",
  "symbols": ["AAPL", "MSFT"],
  "start_date": "2023-01-01",
  "end_date": "2023-12-31",
  "created_at": "2023-07-24T09:00:00Z",
  "started_at": "2023-07-24T09:01:00Z",
  "progress": {
    "percentage": 45.5,
    "files_completed": 165,
    "files_total": 365,
    "records_processed": 1234567,
    "bytes_downloaded": 53687091200,
    "current_file": "2023-06-15.csv.gz",
    "errors": 2,
    "warnings": 5
  },
  "performance": {
    "download_speed_mbps": 85.5,
    "processing_rate_rps": 11234,
    "memory_usage_mb": 1823,
    "cpu_usage_percent": 67.5
  },
  "eta": "2023-07-24T11:30:00Z"
}
```

#### `PUT /operations/{id}/control`

Control operation execution.

**Request Body:**
```json
{
  "action": "pause"  // pause, resume, cancel
}
```

**Response:**
```json
{
  "id": "op_123abc",
  "status": "paused",
  "message": "Operation paused successfully"
}
```

#### `DELETE /operations/{id}`

Cancel and delete an operation.

**Response:**
```json
{
  "message": "Operation cancelled and deleted"
}
```

### Progress

#### `GET /operations/{id}/progress`

Get real-time progress updates.

**Response (Server-Sent Events):**
```
event: progress
data: {"percentage": 45.5, "current_file": "2023-06-15.csv.gz", "rate": 11234}

event: progress
data: {"percentage": 45.6, "current_file": "2023-06-15.csv.gz", "rate": 11456}

event: error
data: {"file": "2023-06-16.csv.gz", "error": "Checksum mismatch"}

event: complete
data: {"total_records": 2456789, "duration": 7234}
```

#### `GET /operations/{id}/logs`

Get operation logs.

**Query Parameters:**
- `level` (string): Log level filter (debug, info, warning, error)
- `limit` (integer): Max lines (default: 1000)
- `follow` (boolean): Stream logs in real-time

**Response:**
```json
{
  "logs": [
    {
      "timestamp": "2023-07-24T10:00:00.123Z",
      "level": "INFO",
      "message": "Starting download of 2023-06-15.csv.gz",
      "context": {
        "file": "2023-06-15.csv.gz",
        "size_bytes": 145678234
      }
    }
  ]
}
```

### Validation

#### `POST /validation/check`

Run validation on imported data.

**Request Body:**
```json
{
  "symbols": ["AAPL", "MSFT"],
  "date_range": {
    "start": "2023-01-01",
    "end": "2023-01-31"
  },
  "checks": [
    "completeness",
    "consistency",
    "duplicates",
    "gaps"
  ]
}
```

**Response:**
```json
{
  "validation_id": "val_789ghi",
  "status": "completed",
  "summary": {
    "total_records": 123456,
    "valid_records": 123400,
    "invalid_records": 56,
    "warnings": 12
  },
  "issues": [
    {
      "type": "gap",
      "severity": "warning",
      "symbol": "AAPL",
      "date": "2023-01-15",
      "description": "Missing data between 14:30 and 14:35"
    }
  ]
}
```

### Metrics

#### `GET /metrics`

Get system metrics in Prometheus format.

**Response:**
```
# HELP backfill_operations_total Total number of backfill operations
# TYPE backfill_operations_total counter
backfill_operations_total{status="completed"} 42
backfill_operations_total{status="failed"} 3

# HELP backfill_records_processed_total Total records processed
# TYPE backfill_records_processed_total counter
backfill_records_processed_total{symbol="AAPL"} 1234567
backfill_records_processed_total{symbol="MSFT"} 1123456

# HELP backfill_processing_rate_rps Current processing rate
# TYPE backfill_processing_rate_rps gauge
backfill_processing_rate_rps 11234

# HELP backfill_active_operations Number of active operations
# TYPE backfill_active_operations gauge
backfill_active_operations 2
```

#### `GET /metrics/summary`

Get human-readable metrics summary.

**Response:**
```json
{
  "operations": {
    "total": 45,
    "completed": 42,
    "failed": 3,
    "active": 2
  },
  "performance": {
    "avg_processing_rate_rps": 10500,
    "peak_processing_rate_rps": 15234,
    "avg_download_speed_mbps": 75.5
  },
  "data": {
    "total_records": 245678901,
    "total_size_gb": 1234.5,
    "symbols_processed": 150
  },
  "uptime": {
    "started_at": "2023-07-24T00:00:00Z",
    "uptime_seconds": 36000
  }
}
```

### Configuration

#### `GET /config`

Get current configuration.

**Response:**
```json
{
  "version": "1.0.0",
  "features": {
    "checkpoint": true,
    "validation": true,
    "metrics": true
  },
  "limits": {
    "max_workers": 20,
    "max_batch_size": 50000,
    "max_memory_mb": 4096
  },
  "defaults": {
    "batch_size": 10000,
    "workers": 10,
    "checkpoint_interval": 60
  }
}
```

#### `PUT /config`

Update configuration (requires admin).

**Request Body:**
```json
{
  "defaults": {
    "batch_size": 20000,
    "workers": 15
  }
}
```

**Response:**
```json
{
  "message": "Configuration updated",
  "restart_required": false
}
```

## WebSocket API

### Real-time Updates

Connect to WebSocket for real-time updates:

```
ws://localhost:8080/api/v1/ws
```

**Subscribe to operation:**
```json
{
  "action": "subscribe",
  "operation_id": "op_123abc"
}
```

**Receive updates:**
```json
{
  "type": "progress",
  "operation_id": "op_123abc",
  "data": {
    "percentage": 45.5,
    "rate": 11234,
    "eta": "2023-07-24T11:30:00Z"
  }
}
```

## Error Responses

All errors follow standard format:

```json
{
  "error": {
    "code": "INVALID_REQUEST",
    "message": "Invalid symbol format",
    "details": {
      "field": "symbols",
      "value": "INVALID SYMBOL",
      "constraint": "Must be alphanumeric"
    }
  },
  "request_id": "req_abc123",
  "timestamp": "2023-07-24T10:00:00Z"
}
```

### Error Codes

| Code | HTTP Status | Description |
|------|-------------|-------------|
| `INVALID_REQUEST` | 400 | Invalid request parameters |
| `UNAUTHORIZED` | 401 | Missing or invalid API key |
| `FORBIDDEN` | 403 | Insufficient permissions |
| `NOT_FOUND` | 404 | Resource not found |
| `CONFLICT` | 409 | Operation conflict |
| `RATE_LIMITED` | 429 | Too many requests |
| `SERVER_ERROR` | 500 | Internal server error |

## Rate Limiting

API requests are rate limited:

- **Default**: 1000 requests per hour
- **Authenticated**: 5000 requests per hour
- **Admin**: Unlimited

Rate limit headers:
```http
X-RateLimit-Limit: 5000
X-RateLimit-Remaining: 4995
X-RateLimit-Reset: 1690195200
```

## Pagination

List endpoints support pagination:

```http
GET /api/v1/operations?limit=50&offset=100

Link: </api/v1/operations?limit=50&offset=150>; rel="next",
      </api/v1/operations?limit=50&offset=50>; rel="prev",
      </api/v1/operations?limit=50&offset=0>; rel="first",
      </api/v1/operations?limit=50&offset=500>; rel="last"
```

## Examples

### Start New Backfill

```bash
curl -X POST http://localhost:8080/api/v1/operations \
  -H "Authorization: Bearer $API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "type": "s3_download",
    "source": {
      "profile": "polygon-s3",
      "prefix": "us_stocks_sip/day_aggs_v1/"
    },
    "symbols": ["AAPL"],
    "date_range": {
      "start": "2023-01-01",
      "end": "2023-01-31"
    }
  }'
```

### Monitor Progress

```bash
# Get current status
curl http://localhost:8080/api/v1/operations/op_123abc \
  -H "Authorization: Bearer $API_KEY"

# Stream progress updates
curl http://localhost:8080/api/v1/operations/op_123abc/progress \
  -H "Authorization: Bearer $API_KEY" \
  -H "Accept: text/event-stream"
```

### Pause and Resume

```bash
# Pause operation
curl -X PUT http://localhost:8080/api/v1/operations/op_123abc/control \
  -H "Authorization: Bearer $API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"action": "pause"}'

# Resume operation
curl -X PUT http://localhost:8080/api/v1/operations/op_123abc/control \
  -H "Authorization: Bearer $API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"action": "resume"}'
```

## SDK Support

Official SDKs available:

- **Python**: `pip install neural-trader-sdk`
- **JavaScript**: `npm install @neural-trader/sdk`
- **Go**: `go get github.com/neural-trader/sdk-go`

Example using Python SDK:
```python
from neural_trader import BackfillClient

client = BackfillClient(
    base_url="http://localhost:8080",
    api_key="your-api-key"
)

# Start backfill
operation = client.create_operation(
    type="s3_download",
    symbols=["AAPL", "MSFT"],
    start_date="2023-01-01",
    end_date="2023-12-31"
)

# Monitor progress
for update in operation.stream_progress():
    print(f"Progress: {update.percentage}%")
```