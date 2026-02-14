//! Graph storage trait and types
//!
//! Defines the `GraphStore` trait for causal relationship graphs.
//! Phase 1 provides SqlGraphStore (always compiled) and optional
//! RuvectorGraphStore (feature-gated).

pub mod sql;

#[cfg(feature = "ruvector-graph-backend")]
pub mod ruvector;

use chrono::{DateTime, Utc};

/// A node in the causal graph.
#[derive(Debug, Clone)]
pub struct GraphNode {
    /// Unique node identifier
    pub id: String,
    /// Node type (e.g., "metric", "stream", "event")
    pub node_type: String,
    /// Arbitrary properties
    pub properties: serde_json::Value,
    /// When this node was created
    pub created_at: DateTime<Utc>,
}

/// An edge in the causal graph.
#[derive(Debug, Clone)]
pub struct GraphEdge {
    /// Source node ID
    pub source_id: String,
    /// Target node ID
    pub target_id: String,
    /// Edge type (e.g., "causes", "correlates_with", "predicts")
    pub edge_type: String,
    /// Edge weight (strength of relationship)
    pub weight: f64,
    /// Arbitrary properties
    pub properties: serde_json::Value,
    /// When this edge was created
    pub created_at: DateTime<Utc>,
}

/// Errors from graph operations.
#[derive(Debug, thiserror::Error)]
pub enum GraphError {
    /// Referenced node does not exist
    #[error("Node not found: {id}")]
    NodeNotFound { id: String },

    /// Edge references a node that doesn't exist
    #[error("Edge references non-existent node: {node_id}")]
    DanglingEdge { node_id: String },

    /// Backend-specific error
    #[error("Backend error: {0}")]
    Backend(String),
}

/// Trait for graph storage backends.
#[async_trait::async_trait]
pub trait GraphStore: Send + Sync {
    /// Add or upsert a node.
    async fn add_node(&self, node: &GraphNode) -> std::result::Result<(), GraphError>;

    /// Add an edge between two existing nodes.
    async fn add_edge(&self, edge: &GraphEdge) -> std::result::Result<(), GraphError>;

    /// Get edges originating from a node, optionally filtered by type.
    async fn get_edges(
        &self,
        node_id: &str,
        edge_type: Option<&str>,
    ) -> std::result::Result<Vec<GraphEdge>, GraphError>;

    /// Get 1-hop neighbor nodes, optionally filtered by edge type.
    async fn get_neighbors(
        &self,
        node_id: &str,
        edge_type: Option<&str>,
    ) -> std::result::Result<Vec<GraphNode>, GraphError>;

    /// Count nodes, optionally filtered by type.
    async fn node_count(
        &self,
        node_type: Option<&str>,
    ) -> std::result::Result<usize, GraphError>;

    /// Count edges, optionally filtered by type.
    async fn edge_count(
        &self,
        edge_type: Option<&str>,
    ) -> std::result::Result<usize, GraphError>;
}

/// Create the default graph store backend.
///
/// Returns SqlGraphStore when no feature gates are active.
/// When `ruvector-graph-backend` is enabled, could return RuvectorGraphStore
/// based on configuration (Phase 2).
pub fn default_graph_store(
    client: std::sync::Arc<tokio_postgres::Client>,
) -> sql::SqlGraphStore {
    sql::SqlGraphStore::new(client)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_graph_node_construction() {
        let node = GraphNode {
            id: "metric:pm25".to_string(),
            node_type: "metric".to_string(),
            properties: serde_json::json!({"unit": "ug/m3"}),
            created_at: Utc::now(),
        };
        assert_eq!(node.id, "metric:pm25");
        assert_eq!(node.node_type, "metric");
    }

    #[test]
    fn test_graph_edge_construction() {
        let edge = GraphEdge {
            source_id: "metric:pm25".to_string(),
            target_id: "metric:co2".to_string(),
            edge_type: "correlates_with".to_string(),
            weight: 0.85,
            properties: serde_json::json!({}),
            created_at: Utc::now(),
        };
        assert_eq!(edge.edge_type, "correlates_with");
        assert!((edge.weight - 0.85).abs() < f64::EPSILON);
    }

    #[test]
    fn test_graph_error_display() {
        let err = GraphError::NodeNotFound {
            id: "n1".to_string(),
        };
        assert_eq!(err.to_string(), "Node not found: n1");

        let err = GraphError::DanglingEdge {
            node_id: "n2".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "Edge references non-existent node: n2"
        );

        let err = GraphError::Backend("timeout".to_string());
        assert_eq!(err.to_string(), "Backend error: timeout");
    }

    #[test]
    fn test_graph_store_is_object_safe() {
        fn _accept_store(_store: &dyn GraphStore) {}
    }
}
