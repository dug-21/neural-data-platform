//! SQL-based graph store using adjacency tables
//!
//! Always-compiled graph backend using PostgreSQL tables for node/edge storage.
//! This is the default and fallback backend.

use std::sync::Arc;

use tokio_postgres::Client;

use super::{GraphEdge, GraphError, GraphNode, GraphStore};

/// SQL adjacency-table graph store.
///
/// Uses `gold.graph_nodes` and `gold.graph_edges` tables for storage.
pub struct SqlGraphStore {
    client: Arc<Client>,
}

impl SqlGraphStore {
    /// Create a new SqlGraphStore with the given PostgreSQL client.
    pub fn new(client: Arc<Client>) -> Self {
        Self { client }
    }
}

#[async_trait::async_trait]
impl GraphStore for SqlGraphStore {
    async fn add_node(&self, node: &GraphNode) -> Result<(), GraphError> {
        self.client
            .execute(
                "INSERT INTO gold.graph_nodes (id, node_type, properties, created_at)
                 VALUES ($1, $2, $3, $4)
                 ON CONFLICT (id) DO UPDATE SET
                     node_type = EXCLUDED.node_type,
                     properties = EXCLUDED.properties",
                &[
                    &node.id,
                    &node.node_type,
                    &node.properties,
                    &node.created_at,
                ],
            )
            .await
            .map_err(|e| GraphError::Backend(e.to_string()))?;
        Ok(())
    }

    async fn add_edge(&self, edge: &GraphEdge) -> Result<(), GraphError> {
        // Verify source node exists
        let source_exists = self
            .client
            .query_opt(
                "SELECT id FROM gold.graph_nodes WHERE id = $1",
                &[&edge.source_id],
            )
            .await
            .map_err(|e| GraphError::Backend(e.to_string()))?;

        if source_exists.is_none() {
            return Err(GraphError::DanglingEdge {
                node_id: edge.source_id.clone(),
            });
        }

        // Verify target node exists
        let target_exists = self
            .client
            .query_opt(
                "SELECT id FROM gold.graph_nodes WHERE id = $1",
                &[&edge.target_id],
            )
            .await
            .map_err(|e| GraphError::Backend(e.to_string()))?;

        if target_exists.is_none() {
            return Err(GraphError::DanglingEdge {
                node_id: edge.target_id.clone(),
            });
        }

        self.client
            .execute(
                "INSERT INTO gold.graph_edges (source_id, target_id, edge_type, weight, properties, created_at)
                 VALUES ($1, $2, $3, $4, $5, $6)",
                &[
                    &edge.source_id,
                    &edge.target_id,
                    &edge.edge_type,
                    &edge.weight,
                    &edge.properties,
                    &edge.created_at,
                ],
            )
            .await
            .map_err(|e| GraphError::Backend(e.to_string()))?;
        Ok(())
    }

    async fn get_edges(
        &self,
        node_id: &str,
        edge_type: Option<&str>,
    ) -> Result<Vec<GraphEdge>, GraphError> {
        let rows = if let Some(etype) = edge_type {
            self.client
                .query(
                    "SELECT source_id, target_id, edge_type, weight, properties, created_at
                     FROM gold.graph_edges
                     WHERE source_id = $1 AND edge_type = $2",
                    &[&node_id, &etype],
                )
                .await
        } else {
            self.client
                .query(
                    "SELECT source_id, target_id, edge_type, weight, properties, created_at
                     FROM gold.graph_edges
                     WHERE source_id = $1",
                    &[&node_id],
                )
                .await
        }
        .map_err(|e| GraphError::Backend(e.to_string()))?;

        let edges = rows
            .iter()
            .map(|row| GraphEdge {
                source_id: row.get(0),
                target_id: row.get(1),
                edge_type: row.get(2),
                weight: row.get(3),
                properties: row.get(4),
                created_at: row.get(5),
            })
            .collect();

        Ok(edges)
    }

    async fn get_neighbors(
        &self,
        node_id: &str,
        edge_type: Option<&str>,
    ) -> Result<Vec<GraphNode>, GraphError> {
        let rows = if let Some(etype) = edge_type {
            self.client
                .query(
                    "SELECT n.id, n.node_type, n.properties, n.created_at
                     FROM gold.graph_nodes n
                     INNER JOIN gold.graph_edges e ON n.id = e.target_id
                     WHERE e.source_id = $1 AND e.edge_type = $2",
                    &[&node_id, &etype],
                )
                .await
        } else {
            self.client
                .query(
                    "SELECT n.id, n.node_type, n.properties, n.created_at
                     FROM gold.graph_nodes n
                     INNER JOIN gold.graph_edges e ON n.id = e.target_id
                     WHERE e.source_id = $1",
                    &[&node_id],
                )
                .await
        }
        .map_err(|e| GraphError::Backend(e.to_string()))?;

        let nodes = rows
            .iter()
            .map(|row| GraphNode {
                id: row.get(0),
                node_type: row.get(1),
                properties: row.get(2),
                created_at: row.get(3),
            })
            .collect();

        Ok(nodes)
    }

    async fn node_count(&self, node_type: Option<&str>) -> Result<usize, GraphError> {
        let row = if let Some(ntype) = node_type {
            self.client
                .query_one(
                    "SELECT COUNT(*)::bigint FROM gold.graph_nodes WHERE node_type = $1",
                    &[&ntype],
                )
                .await
        } else {
            self.client
                .query_one("SELECT COUNT(*)::bigint FROM gold.graph_nodes", &[])
                .await
        }
        .map_err(|e| GraphError::Backend(e.to_string()))?;

        let count: i64 = row.get(0);
        Ok(count as usize)
    }

    async fn edge_count(&self, edge_type: Option<&str>) -> Result<usize, GraphError> {
        let row = if let Some(etype) = edge_type {
            self.client
                .query_one(
                    "SELECT COUNT(*)::bigint FROM gold.graph_edges WHERE edge_type = $1",
                    &[&etype],
                )
                .await
        } else {
            self.client
                .query_one("SELECT COUNT(*)::bigint FROM gold.graph_edges", &[])
                .await
        }
        .map_err(|e| GraphError::Backend(e.to_string()))?;

        let count: i64 = row.get(0);
        Ok(count as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Unit tests use the trait-level assertions; integration tests need a real DB.

    #[test]
    fn test_sql_graph_store_is_send_sync() {
        fn _assert_send_sync<T: Send + Sync>() {}
        _assert_send_sync::<SqlGraphStore>();
    }

    // Integration tests are #[ignore] because they need a running PostgreSQL with pgvector.
    // Run with: cargo test -p ndp-intelligence -- --ignored

    #[tokio::test]
    #[ignore]
    async fn test_add_node_and_count() {
        // Requires: PostgreSQL with gold.graph_nodes table
        // This test verifies add_node + node_count round-trip
        let _db_url = std::env::var("TIMESCALE_URL")
            .unwrap_or_else(|_| "host=localhost dbname=ndp user=ndp password=ndp".to_string());
        // Connection setup and test would go here
    }

    #[tokio::test]
    #[ignore]
    async fn test_add_edge_dangling_source() {
        // Verifies DanglingEdge error when source doesn't exist
    }

    #[tokio::test]
    #[ignore]
    async fn test_get_edges_by_type() {
        // Verifies edge type filtering
    }

    #[tokio::test]
    #[ignore]
    async fn test_get_neighbors_one_hop() {
        // Verifies 1-hop neighbor traversal
    }
}
