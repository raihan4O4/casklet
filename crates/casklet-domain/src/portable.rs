//! Portable installation identity and provider capabilities.
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallationState {
    Installed,
    Missing,
    Changed,
    Removed,
    RecoveryRequired,
}
impl InstallationState {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Installed => "INSTALLED",
            Self::Missing => "MISSING",
            Self::Changed => "CHANGED",
            Self::Removed => "REMOVED",
            Self::RecoveryRequired => "RECOVERY_REQUIRED",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortableIdentity {
    pub application_id: String,
    pub installation_id: String,
    pub cask_installation_id: String,
}
impl PortableIdentity {
    #[must_use]
    pub fn new() -> Self {
        Self {
            application_id: Uuid::new_v4().to_string(),
            installation_id: Uuid::new_v4().to_string(),
            cask_installation_id: Uuid::new_v4().to_string(),
        }
    }
}
impl Default for PortableIdentity {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn application_installation_and_association_have_distinct_ids() {
        let ids = PortableIdentity::new();
        assert_ne!(ids.application_id, ids.installation_id);
        assert_ne!(ids.installation_id, ids.cask_installation_id);
    }
}
