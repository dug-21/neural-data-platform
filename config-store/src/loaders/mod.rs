/// Configuration loaders module
/// Provides different ways to load configurations into the store

pub mod gitops;

#[cfg(test)]
mod gitops_test;

pub use gitops::GitOpsLoader;