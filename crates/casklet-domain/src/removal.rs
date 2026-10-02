//! Conservative removal eligibility. Association and detection never grant deletion authority.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProposedAction {
    Trash,
    Detach,
    Preserve,
}

impl ProposedAction {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Trash => "TRASH",
            Self::Detach => "DETACH",
            Self::Preserve => "PRESERVE",
        }
    }
}

#[derive(Debug, Clone, Copy)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "independent safety evidence is intentionally explicit and exhaustively tested"
)]
pub struct RemovalEvidence {
    pub managed: bool,
    pub internal: bool,
    pub explicitly_adopted: bool,
    pub created_by_casklet: bool,
    pub owned: bool,
    pub delete_policy: bool,
    pub has_other_association: bool,
    pub present: bool,
    pub verified: bool,
    pub broad: bool,
    pub unknown_entries: bool,
}

#[must_use]
pub const fn propose(e: RemovalEvidence) -> (ProposedAction, &'static str) {
    if !e.owned
        || !e.managed
        || !e.delete_policy
        || !(e.internal && e.created_by_casklet || !e.internal && e.explicitly_adopted)
    {
        return (ProposedAction::Detach, "NO_DELETE_AUTHORITY");
    }
    if e.has_other_association {
        return (ProposedAction::Detach, "SHARED");
    }
    if !e.present || !e.verified || e.broad || e.unknown_entries {
        return (ProposedAction::Preserve, "UNVERIFIED");
    }
    (ProposedAction::Trash, "ELIGIBLE_MANAGED")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_evidence_combinations_never_delete_without_every_required_fact() {
        for bits in 0_u16..2048 {
            let e = RemovalEvidence {
                managed: bits & 1 != 0,
                internal: bits & 2 != 0,
                explicitly_adopted: bits & 1024 != 0,
                created_by_casklet: bits & 4 != 0,
                owned: bits & 8 != 0,
                delete_policy: bits & 16 != 0,
                has_other_association: bits & 32 != 0,
                present: bits & 64 != 0,
                verified: bits & 128 != 0,
                broad: bits & 256 != 0,
                unknown_entries: bits & 512 != 0,
            };
            let can_trash = e.managed
                && ((e.internal && e.created_by_casklet) || (!e.internal && e.explicitly_adopted))
                && e.owned
                && e.delete_policy
                && !e.has_other_association
                && e.present
                && e.verified
                && !e.broad
                && !e.unknown_entries;
            assert_eq!(propose(e).0 == ProposedAction::Trash, can_trash);
        }
    }
}
