#![doc = "Infrastructure adapters. Concrete persistence and platform behavior begin in later milestones."]

/// Marker proving the infrastructure crate is linked without adding an adapter.
pub const CRATE_BOUNDARY: &str = "infrastructure";
