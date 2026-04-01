//! Space Engineers compatibility types.
//!
//! This crate provides Rust equivalents for C# types used in Space Engineers,
//! with support for serde (XML/JSON), protobuf (proto_rs), and Deku (bit-level)
//! serialization.
//!
//! # Module Organization
//!
//! - [`bcl`] - C# Base Class Library types (DateTime, TimeSpan, Guid, Decimal)
//! - [`bitfield`] - BitField wrapper for enumflags2::BitFlags
//! - [`nullable`] - Nullable wrapper for C# nullable value types
//! - [`collections`] - VarMap and Tuple types
//! - [`xml`] - XML serialization helpers
//! - [`deku`] - Bit-stream serialization types (Varint, BitAligned, VarBytes, VarString)
//! - [`direction`] - Base6Directions.Direction enum
//! - [`math`] - Vector, Matrix, Quaternion types

// Core modules
mod bcl;
mod bitfield;
mod collections;
mod deku;
mod nullable;
mod xml;

// Game-specific modules  
pub mod compression;
pub mod direction;
pub mod math;

// Re-export all public types at crate root for convenience
pub use bcl::{DateTime, DateTimeKind, Decimal, Guid, TimeSpan, TimeSpanScale};
pub use bitfield::{BitField, BitFieldShadow};
pub use collections::{VarMap, Tuple};
pub use deku::{
    BitAligned, BitAlignedShadow, BitBool, BitBoolShadow, VarBytes, VarBytesShadow, VarString, VarStringShadow,
    VarVec, Varint, VarintShadow,
};
pub use nullable::{Nullable, NullableShadow};
pub use xml::{xml_vec, PacketCompressedXmlObject};
pub use compression::{compress, decompress};
