#![doc = "Use-case orchestration and ports. Product behavior begins in Milestone 1."]

/// Returns linked lower-layer markers for a dependency-direction smoke test.
#[must_use]
pub const fn linked_boundaries() -> (&'static str, &'static str) {
    (
        casklet_domain::CRATE_BOUNDARY,
        casklet_contracts::CRATE_BOUNDARY,
    )
}
