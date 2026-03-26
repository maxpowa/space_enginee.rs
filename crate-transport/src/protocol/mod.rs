//! Stable type and event identities for Space Engineers.
//!
//! These enums use FNV-1a hashes as stable identifiers that don't change
//! between game versions. Use `Version` to convert to/from
//! version-specific indices at runtime.

#![allow(non_camel_case_types, clippy::all)]

mod replicated_types;
mod static_events;
mod instance_events;
pub mod version;

pub use replicated_types::ReplicatedType;

// Re-export all static event types, payloads, and visitor
pub use static_events::*;

// Re-export all instance event types and parsers
pub use instance_events::*;

// Re-export version schema types
pub use version::{InstanceEventSchema, SchemaError, Version};
