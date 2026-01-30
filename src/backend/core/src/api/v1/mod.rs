//! V1 API module for Apex Core.
//!
//! This module contains the stable V1 API endpoints for:
//! - Task management
//! - DAG operations
//! - Agent management
//! - Contract management
//! - System statistics
//!
//! V1 is the current stable API version.

pub mod routes;

pub use routes::{v1_router, V1_PREFIX};
