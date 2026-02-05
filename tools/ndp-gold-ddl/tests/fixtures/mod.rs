//! Test fixtures for Phase C unit tests
//!
//! Provides helper functions for creating test configurations
//! following London TDD patterns. These fixtures enable testing
//! without real infrastructure dependencies.
//!
//! # Usage
//!
//! ```rust
//! use fixtures::{create_three_stream_domain, create_transition_config, create_objective};
//!
//! let domain = create_three_stream_domain();
//! let transition = create_transition_config("home-assistant-state");
//! let objective = create_objective("healthy_co2", "co2", "<", 800.0);
//! ```

pub mod phase_c;

pub use phase_c::*;
