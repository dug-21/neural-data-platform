//! DP-021: Stream management handlers
//!
//! Provides HTTP endpoints for stream hot-reload and management.

use crate::coordinator::SourceManager;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

/// State for stream management endpoints
pub type StreamManagerState = Arc<RwLock<SourceManager>>;

/// Response for stream reload
#[derive(Debug, Serialize)]
pub struct ReloadResponse {
    pub success: bool,
    pub stream_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sources_started: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sources_stopped: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Error response
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub stream_id: String,
}

/// POST /api/v1/streams/{stream_id}/reload
///
/// Triggers a hot-reload for the specified stream. This will:
/// 1. Clear the config cache for the stream
/// 2. Reload the configuration from etcd
/// 3. Stop existing sources for the stream
/// 4. Start new sources with the updated configuration
///
/// # Path Parameters
/// - `stream_id`: The stream identifier (e.g., "air-quality", "outdoor-weather")
///
/// # Returns
/// - 200 OK with reload results on success
/// - 500 Internal Server Error if reload fails
pub async fn reload_stream_handler(
    Path(stream_id): Path<String>,
    State(source_manager): State<StreamManagerState>,
) -> impl IntoResponse {
    tracing::info!(stream_id = %stream_id, "HTTP reload request received");

    let mut manager = source_manager.write().await;
    let result = manager.trigger_reload(&stream_id).await;

    if result.success {
        tracing::info!(
            stream_id = %stream_id,
            sources_started = ?result.sources_started,
            sources_stopped = ?result.sources_stopped,
            duration_ms = result.duration_ms,
            "HTTP reload completed successfully"
        );

        (
            StatusCode::OK,
            Json(ReloadResponse {
                success: true,
                stream_id,
                sources_started: Some(result.sources_started),
                sources_stopped: Some(result.sources_stopped),
                duration_ms: Some(result.duration_ms),
                error: None,
            }),
        )
            .into_response()
    } else {
        let error_msg = result.error.unwrap_or_else(|| "Unknown error".to_string());
        tracing::error!(
            stream_id = %stream_id,
            error = %error_msg,
            "HTTP reload failed"
        );

        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ReloadResponse {
                success: false,
                stream_id,
                sources_started: None,
                sources_stopped: None,
                duration_ms: Some(result.duration_ms),
                error: Some(error_msg),
            }),
        )
            .into_response()
    }
}

/// GET /api/v1/streams/{stream_id}/health
///
/// Gets health status for all sources of a specific stream.
///
/// # Path Parameters
/// - `stream_id`: The stream identifier
///
/// # Returns
/// - 200 OK with health status
#[derive(Debug, Serialize)]
pub struct StreamHealthResponse {
    pub stream_id: String,
    pub sources: Vec<SourceHealthInfo>,
}

#[derive(Debug, Serialize)]
pub struct SourceHealthInfo {
    pub source_id: String,
    pub health: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

pub async fn stream_health_handler(
    Path(stream_id): Path<String>,
    State(source_manager): State<StreamManagerState>,
) -> impl IntoResponse {
    let manager = source_manager.read().await;
    let all_health = manager.get_all_health().await;

    // Filter health for this stream's sources
    let sources: Vec<SourceHealthInfo> = all_health
        .into_iter()
        .filter(|(source_id, _)| source_id.starts_with(&format!("{}-", stream_id)))
        .map(|(source_id, health)| {
            let (status, reason) = match health {
                crate::coordinator::SourceHealth::Healthy => ("healthy".to_string(), None),
                crate::coordinator::SourceHealth::Degraded { reason } => {
                    ("degraded".to_string(), Some(reason))
                }
                crate::coordinator::SourceHealth::Unhealthy { reason } => {
                    ("unhealthy".to_string(), Some(reason))
                }
                crate::coordinator::SourceHealth::Unknown => ("unknown".to_string(), None),
            };
            SourceHealthInfo {
                source_id,
                health: status,
                reason,
            }
        })
        .collect();

    (
        StatusCode::OK,
        Json(StreamHealthResponse { stream_id, sources }),
    )
}

/// GET /api/v1/streams
///
/// Lists all streams with their source counts.
#[derive(Debug, Serialize)]
pub struct StreamListResponse {
    pub streams: Vec<StreamInfo>,
}

#[derive(Debug, Serialize)]
pub struct StreamInfo {
    pub stream_id: String,
    pub source_count: usize,
    pub healthy_count: usize,
}

pub async fn list_streams_handler(
    State(source_manager): State<StreamManagerState>,
) -> impl IntoResponse {
    let manager = source_manager.read().await;
    let all_health = manager.get_all_health().await;

    // Group by stream_id prefix
    let mut stream_map: std::collections::HashMap<String, (usize, usize)> =
        std::collections::HashMap::new();

    for (source_id, health) in all_health {
        // Extract stream_id from source_id (format: "{stream_id}-{source_type}-{index}")
        if let Some(stream_id) = source_id.split('-').next() {
            let entry = stream_map.entry(stream_id.to_string()).or_insert((0, 0));
            entry.0 += 1; // total count
            if matches!(health, crate::coordinator::SourceHealth::Healthy) {
                entry.1 += 1; // healthy count
            }
        }
    }

    let streams: Vec<StreamInfo> = stream_map
        .into_iter()
        .map(|(stream_id, (source_count, healthy_count))| StreamInfo {
            stream_id,
            source_count,
            healthy_count,
        })
        .collect();

    (StatusCode::OK, Json(StreamListResponse { streams }))
}
