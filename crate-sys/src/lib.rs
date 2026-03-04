#![allow(non_camel_case_types, non_snake_case, unused_imports)]

// Re-export the compat crate at `crate::compat` so generated code paths
// like `crate::compat::DateTime` resolve correctly.
pub use space_engineers_compat as compat;

// Re-export math types at `crate::math` so generated code paths
// like `crate::math::Vector3F` resolve correctly.
pub mod math {
    pub use space_engineers_compat::math::*;
}

pub mod types;
