//! Opt-in helper utilities for examples, tests, and downstream validation.
//!
//! This module is public behind the `utils` feature, and is not considered part
//! of the public API contract. It is intended for internal use in examples and tests,
//! and for downstream users to copy and adapt as needed. As such, it may change without
//! warning and should not be used directly by downstream users.

pub mod counting;
pub mod file;
pub mod fixture;
pub mod flaky;
pub mod materialization;
pub mod oracle;
pub mod temp;
pub mod tracing;
