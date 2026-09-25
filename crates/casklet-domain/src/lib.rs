//! Platform-neutral Cask rules.

use uuid::Uuid;
pub mod resource;

pub const CRATE_BOUNDARY: &str = "domain";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cask {
    pub id: String,
    pub name: String,
    pub storage_key: String,
    pub managed_storage_root: String,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
    pub revision: i64,
    pub state: CaskState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaskState {
    Creating,
    Active,
    Archived,
    Destroying,
    PartiallyDestroyed,
    Destroyed,
    RecoveryRequired,
}

impl CaskState {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Creating => "CREATING",
            Self::Active => "ACTIVE",
            Self::Archived => "ARCHIVED",
            Self::Destroying => "DESTROYING",
            Self::PartiallyDestroyed => "PARTIALLY_DESTROYED",
            Self::Destroyed => "DESTROYED",
            Self::RecoveryRequired => "RECOVERY_REQUIRED",
        }
    }
    #[must_use]
    pub fn from_code(code: &str) -> Option<Self> {
        Some(match code {
            "CREATING" => Self::Creating,
            "ACTIVE" => Self::Active,
            "ARCHIVED" => Self::Archived,
            "DESTROYING" => Self::Destroying,
            "PARTIALLY_DESTROYED" => Self::PartiallyDestroyed,
            "DESTROYED" => Self::Destroyed,
            "RECOVERY_REQUIRED" => Self::RecoveryRequired,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomainError {
    InvalidName,
}

impl Cask {
    /// Names are labels, not identities. Duplicate names are valid.
    ///
    /// # Errors
    /// Returns `InvalidName` for empty, overlong, or control-character names.
    pub fn new(name: &str, managed_storage_root: String, now_ms: i64) -> Result<Self, DomainError> {
        let name = name.trim();
        if name.is_empty() || name.chars().count() > 120 || name.chars().any(char::is_control) {
            return Err(DomainError::InvalidName);
        }
        let id = Uuid::new_v4().to_string();
        Ok(Self {
            id: id.clone(),
            name: name.to_owned(),
            storage_key: id,
            managed_storage_root,
            created_at_ms: now_ms,
            updated_at_ms: now_ms,
            revision: 0,
            state: CaskState::Active,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn name_policy() {
        assert!(Cask::new("   ", String::new(), 0).is_err());
        assert!(Cask::new("a\0b", String::new(), 0).is_err());
        assert!(Cask::new(&"x".repeat(121), String::new(), 0).is_err());
        let a = Cask::new(" Same ", String::new(), 0).unwrap();
        let b = Cask::new("Same", String::new(), 0).unwrap();
        assert_eq!(a.name, b.name);
        assert_ne!(a.id, b.id);
    }
}
