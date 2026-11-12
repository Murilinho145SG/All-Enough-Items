//! Local crate root for AEI's browser engine, tests, and editor support.
//!
//! Horizon's native mod loads the separate engine crate through a `horizon-mixins`
//! bundle dependency. This package exists for local Cargo tooling and rust-analyzer.

pub use aei_engine::*;
