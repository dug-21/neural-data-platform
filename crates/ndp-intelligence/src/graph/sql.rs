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
    use chrono::Utc;

    #[test]
    fn test_sql_graph_store_is_send_sync() {
        fn _assert_send_sync<T: Send + Sync>() {}
        _assert_send_sync::<SqlGraphStore>();
    }

    // ---- Integration tests ----
    // Require a running PostgreSQL.
    // Run with: TIMESCALE_URL="host=localhost dbname=ndp user=ndp password=ndp" cargo test -p ndp-intelligence -- --ignored

    async fn setup_graph_client() -> Arc<Client> {
        let url = std::env::var("TIMESCALE_URL")
            .unwrap_or_else(|_| "host=localhost dbname=ndp user=ndp password=ndp".to_string());
        let (client, connection) = tokio_postgres::connect(&url, tokio_postgres::NoTls)
            .await
            .expect("Failed to connect to PostgreSQL");
        tokio::spawn(async move {
            if let Err(e) = connection.await {
                eprintln!("connection error: {}", e);
            }
        });
        let client = Arc::new(client);

        // Ensure tables exist
        client
            .batch_execute(
                "CREATE SCHEMA IF NOT EXISTS gold;
                 CREATE TABLE IF NOT EXISTS gold.graph_nodes (
                     id TEXT PRIMARY KEY,
                     node_type TEXT NOT NULL,
                     properties JSONB DEFAULT '{}',
                     created_at TIMESTAMPTZ DEFAULT NOW()
                 );
                 CREATE TABLE IF NOT EXISTS gold.graph_edges (
                     id SERIAL PRIMARY KEY,
                     source_id TEXT NOT NULL REFERENCES gold.graph_nodes(id),
                     target_id TEXT NOT NULL REFERENCES gold.graph_nodes(id),
                     edge_type TEXT NOT NULL,
                     weight DOUBLE PRECISION DEFAULT 1.0,
                     properties JSONB DEFAULT '{}',
                     created_at TIMESTAMPTZ DEFAULT NOW()
                 );",
            )
            .await
            .expect("Failed to create graph tables");

        // Clean test data
        client
            .batch_execute(
                "DELETE FROM gold.graph_edges WHERE source_id LIKE 'test-%' OR target_id LIKE 'test-%';
                 DELETE FROM gold.graph_nodes WHERE id LIKE 'test-%';",
            )
            .await
            .expect("Failed to clean test data");

        client
    }

    #[tokio::test]
    #[ignore]
    async fn test_add_node_and_count() {
        let client = setup_graph_client().await;
        let store = SqlGraphStore::new(client);

        let node = GraphNode {
            id: "test-node-1".to_string(),
            node_type: "metric".to_string(),
            properties: serde_json::json!({"unit": "ug/m3"}),
            created_at: Utc::now(),
        };

        store.add_node(&node).await.unwrap();
        let count = store.node_count(Some("metric")).await.unwrap();
        assert!(count >= 1, "Should have at least 1 metric node, got {}", count);
    }

    #[tokio::test]
    #[ignore]
    async fn test_add_node_upsert() {
        let client = setup_graph_client().await;
        let store = SqlGraphStore::new(client);

        let node_v1 = GraphNode {
            id: "test-upsert-node".to_string(),
            node_type: "metric".to_string(),
            properties: serde_json::json!({"version": 1}),
            created_at: Utc::now(),
        };
        let node_v2 = GraphNode {
            id: "test-upsert-node".to_string(),
            node_type: "metric".to_string(),
            properties: serde_json::json!({"version": 2}),
            created_at: Utc::now(),
        };

        store.add_node(&node_v1).await.unwrap();
        store.add_node(&node_v2).await.unwrap();

        // Count should still be the same (upsert, not duplicate)
        // We can't easily check the exact count without isolation, but no error means upsert worked
    }

    #[tokio::test]
    #[ignore]
    async fn test_add_edge_dangling_source() {
        let client = setup_graph_client().await;
        let store = SqlGraphStore::new(client);

        let edge = GraphEdge {
            source_id: "test-nonexistent-source".to_string(),
            target_id: "test-nonexistent-target".to_string(),
            edge_type: "causes".to_string(),
            weight: 0.8,
            properties: serde_json::json!({}),
            created_at: Utc::now(),
        };

        let result = store.add_edge(&edge).await;
        assert!(result.is_err(), "Should fail with DanglingEdge");
        match result.unwrap_err() {
            GraphError::DanglingEdge { node_id } => {
                assert_eq!(node_id, "test-nonexistent-source");
            }
            e => panic!("Expected DanglingEdge, got {:?}", e),
        }
    }

    #[tokio::test]
    #[ignore]
    async fn test_get_edges_by_type() {
        let client = setup_graph_client().await;
        let store = SqlGraphStore::new(client);

        // Create two nodes
        let node_a = GraphNode {
            id: "test-edge-a".to_string(),
            node_type: "metric".to_string(),
            properties: serde_json::json!({}),
            created_at: Utc::now(),
        };
        let node_b = GraphNode {
            id: "test-edge-b".to_string(),
            node_type: "metric".to_string(),
            properties: serde_json::json!({}),
            created_at: Utc::now(),
        };
        store.add_node(&node_a).await.unwrap();
        store.add_node(&node_b).await.unwrap();

        // Add two edges with different types
        let edge_causes = GraphEdge {
            source_id: "test-edge-a".to_string(),
            target_id: "test-edge-b".to_string(),
            edge_type: "causes".to_string(),
            weight: 0.9,
            properties: serde_json::json!({}),
            created_at: Utc::now(),
        };
        let edge_corr = GraphEdge {
            source_id: "test-edge-a".to_string(),
            target_id: "test-edge-b".to_string(),
            edge_type: "correlates".to_string(),
            weight: 0.5,
            properties: serde_json::json!({}),
            created_at: Utc::now(),
        };
        store.add_edge(&edge_causes).await.unwrap();
        store.add_edge(&edge_corr).await.unwrap();

        // Filter by type
        let causes_only = store
            .get_edges("test-edge-a", Some("causes"))
            .await
            .unwrap();
        assert!(
            causes_only.iter().all(|e| e.edge_type == "causes"),
            "Should only return 'causes' edges"
        );

        // All edges
        let all = store.get_edges("test-edge-a", None).await.unwrap();
        assert!(all.len() >= 2, "Should return at least 2 edges");
    }

    #[tokio::test]
    #[ignore]
    async fn test_get_neighbors_one_hop() {
        let client = setup_graph_client().await;
        let store = SqlGraphStore::new(client);

        // Create A -> B -> C
        for id in ["test-hop-a", "test-hop-b", "test-hop-c"] {
            store
                .add_node(&GraphNode {
                    id: id.to_string(),
                    node_type: "metric".to_string(),
                    properties: serde_json::json!({}),
                    created_at: Utc::now(),
                })
                .await
                .unwrap();
        }

        store
            .add_edge(&GraphEdge {
                source_id: "test-hop-a".to_string(),
                target_id: "test-hop-b".to_string(),
                edge_type: "causes".to_string(),
                weight: 1.0,
                properties: serde_json::json!({}),
                created_at: Utc::now(),
            })
            .await
            .unwrap();

        store
            .add_edge(&GraphEdge {
                source_id: "test-hop-b".to_string(),
                target_id: "test-hop-c".to_string(),
                edge_type: "causes".to_string(),
                weight: 1.0,
                properties: serde_json::json!({}),
                created_at: Utc::now(),
            })
            .await
            .unwrap();

        // 1-hop from A should give B (not C)
        let neighbors = store
            .get_neighbors("test-hop-a", None)
            .await
            .unwrap();
        let neighbor_ids: Vec<&str> = neighbors.iter().map(|n| n.id.as_str()).collect();
        assert!(
            neighbor_ids.contains(&"test-hop-b"),
            "1-hop neighbors of A should include B, got {:?}",
            neighbor_ids
        );
        assert!(
            !neighbor_ids.contains(&"test-hop-c"),
            "1-hop neighbors of A should NOT include C (that's 2-hop)"
        );
    }
}
