//! Embedding populator module
//!
//! Contains the EmbeddingWriter that bridges the Embedder (ndp-lib)
//! with the StorageBackend (ndp-intelligence).

pub mod embedding_writer;

pub use embedding_writer::EmbeddingWriter;
