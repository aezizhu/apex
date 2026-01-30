//! Event Sourcing System
//!
//! This module provides the full event sourcing infrastructure for Apex:
//!
//! - **`event`**: Domain events, event envelope/metadata types, and the persistent `EventStore`.
//! - **`aggregate`**: The `Aggregate` trait for state reconstruction, with implementations
//!   for Task, Agent, and DAG aggregates.
//! - **`crdt`**: Conflict-free Replicated Data Types (LWWRegister, GSet, ORSet, GCounter,
//!   MergeableState) for deterministic merging of parallel agent outputs.

pub mod aggregate;
pub mod crdt;
pub mod event;

pub use aggregate::*;
pub use crdt::*;
pub use event::*;
