//! Re-export shim. The canonical home for these types is now
//! `aether_xr_hal::tracking` (P1-A migration). Kept here so existing
//! `use aether_input::openxr_tracking::*` import paths continue to compile.
//!
//! TODO(P9): delete this module and migrate consumers to import directly from
//! `aether_xr_hal::tracking`.

pub use aether_xr_hal::tracking::*;
