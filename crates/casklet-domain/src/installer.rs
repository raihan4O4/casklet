//! Ownership policy for supported installer results. Observation is not authority.
#[derive(Debug, Clone, Copy)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "These are independent verified facts, not mutable workflow state"
)]
pub struct InstallerOwnershipEvidence {
    pub supported_format: bool,
    pub successful_launch: bool,
    pub verified_new_product: bool,
    pub single_exact_candidate: bool,
    pub current_identity: bool,
    pub supported_registration: bool,
    pub active_cask: bool,
    pub unshared_and_unlocked: bool,
}
impl InstallerOwnershipEvidence {
    #[must_use]
    pub const fn permits_ownership(self) -> bool {
        self.supported_format
            && self.successful_launch
            && self.verified_new_product
            && self.single_exact_candidate
            && self.current_identity
            && self.supported_registration
            && self.active_cask
            && self.unshared_and_unlocked
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_ownership_fact_is_required() {
        for bits in 0_u16..256 {
            let evidence = InstallerOwnershipEvidence {
                supported_format: bits & 1 != 0,
                successful_launch: bits & 2 != 0,
                verified_new_product: bits & 4 != 0,
                single_exact_candidate: bits & 8 != 0,
                current_identity: bits & 16 != 0,
                supported_registration: bits & 32 != 0,
                active_cask: bits & 64 != 0,
                unshared_and_unlocked: bits & 128 != 0,
            };
            assert_eq!(evidence.permits_ownership(), bits == 255);
        }
    }
}
