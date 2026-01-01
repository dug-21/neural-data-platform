/// Configuration validation module
/// Provides schema-based validation for configurations

pub mod schema;

#[cfg(test)]
mod schema_test;

pub use schema::SchemaValidator;