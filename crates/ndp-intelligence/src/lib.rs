//! NDP Intelligence Foundation
//!
//! This crate provides the intelligence layer for the Neural Data Platform:
//! - Vector similarity search (trait + backends)
//! - Graph storage (SQL adjacency + optional ruvector-graph)
//! - Embedding storage (PostgreSQL + pgvector)
//! - Embedding population (EmbeddingWriter)
//!
//! ## Architecture
//!
//! All backends are trait-based for testability:
//! - [`storage::StorageBackend`] — embedding and prediction persistence
//! - [`graph::GraphStore`] — causal relationship graph
//! - [`similarity::SimilarityEngine`] — vector similarity search (Phase 2)
//!
//! ## Feature Gates
//!
//! - `ruvector` — enables ruvector-core for HNSW similarity search
//! - `ruvector-graph-backend` — enables ruvector-graph for graph operations

pub mod error;
pub mod graph;
pub mod populator;
pub mod similarity;
pub mod storage;

pub use error::{IntelligenceError, Result};
