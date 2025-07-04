//! I/O and serialization module for rUv-FANN

mod error;
mod fann_format;
mod training_data;
mod dot_export;
#[cfg(feature = "serde")]
mod json;
#[cfg(feature = "binary")]
mod binary;
#[cfg(feature = "compression")]
mod compression;
mod streaming;

// Re-export types
pub use error::{IoError, IoResult};
pub use fann_format::{FannReader, FannWriter};
pub use training_data::{TrainingDataReader, TrainingDataWriter, TrainingDataStreamReader};
pub use dot_export::DotExporter;

#[cfg(feature = "serde")]
pub use json::{read_json, write_json};

#[cfg(feature = "binary")]
pub use binary::{read_binary, write_binary};

#[cfg(feature = "compression")]
pub use compression::{compress_data, decompress_data};

/// Supported file formats
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileFormat {
    /// Native FANN format (text-based)
    Fann,
    /// JSON format
    Json,
    /// Binary format (using bincode)
    Binary,
    /// DOT format for visualization
    Dot,
    /// Compressed FANN format
    CompressedFann,
    /// Compressed binary format
    CompressedBinary,
}