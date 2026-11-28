//! Durable operation states. Uncertain external work never becomes success by inference.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationState {
    Running,
    Succeeded,
    RecoveryRequired,
    Cancelled,
    ClosedPreserved,
}

impl OperationState {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Running => "RUNNING",
            Self::Succeeded => "SUCCEEDED",
            Self::RecoveryRequired => "RECOVERY_REQUIRED",
            Self::Cancelled => "CANCELLED",
            Self::ClosedPreserved => "CLOSED_PRESERVED",
        }
    }

    #[must_use]
    pub const fn can_transition_to(self, next: Self) -> bool {
        matches!(
            (self, next),
            (
                Self::Running,
                Self::Succeeded | Self::RecoveryRequired | Self::Cancelled
            ) | (
                Self::RecoveryRequired,
                Self::Succeeded | Self::ClosedPreserved
            )
        )
    }
}

#[cfg(test)]
mod tests {
    use super::OperationState as S;

    #[test]
    fn uncertain_work_cannot_be_silently_reset() {
        assert!(!S::RecoveryRequired.can_transition_to(S::Running));
        assert!(!S::Succeeded.can_transition_to(S::Running));
        assert!(S::Running.can_transition_to(S::RecoveryRequired));
    }
}
