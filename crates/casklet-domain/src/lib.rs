//! Platform-neutral Cask rules.

use uuid::Uuid;
pub mod operation;
pub mod portable;
pub mod removal;
pub mod resource;

pub const CRATE_BOUNDARY: &str = "domain";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cask {
    pub id: String,
    pub name: String,
    pub description: String,
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
    InvalidDescription,
    InvalidState,
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
            description: String::new(),
            storage_key: id,
            managed_storage_root,
            created_at_ms: now_ms,
            updated_at_ms: now_ms,
            revision: 0,
            state: CaskState::Active,
        })
    }

    /// Changes metadata only. Archiving preserves all resources and installations.
    /// # Errors
    /// Rejects invalid text and lifecycle states that require recovery or cleanup.
    pub fn update_metadata(
        &mut self,
        name: &str,
        description: &str,
        archived: bool,
        now_ms: i64,
    ) -> Result<(), DomainError> {
        let name = name.trim();
        if name.is_empty() || name.chars().count() > 120 || name.chars().any(char::is_control) {
            return Err(DomainError::InvalidName);
        }
        if description.chars().count() > 500 || description.chars().any(char::is_control) {
            return Err(DomainError::InvalidDescription);
        }
        if !matches!(self.state, CaskState::Active | CaskState::Archived) {
            return Err(DomainError::InvalidState);
        }
        let next_revision = self
            .revision
            .checked_add(1)
            .ok_or(DomainError::InvalidState)?;
        name.clone_into(&mut self.name);
        description.trim().clone_into(&mut self.description);
        self.state = if archived {
            CaskState::Archived
        } else {
            CaskState::Active
        };
        self.updated_at_ms = now_ms;
        self.revision = next_revision;
        Ok(())
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
    #[test]
    fn metadata_and_archive_preserve_identity_and_storage() {
        let mut cask = Cask::new("Work", "C:/managed".into(), 1).unwrap();
        let id = cask.id.clone();
        cask.update_metadata("Renamed", "Project resources", true, 2)
            .unwrap();
        assert_eq!(cask.id, id);
        assert_eq!(cask.managed_storage_root, "C:/managed");
        assert_eq!(cask.state, CaskState::Archived);
        cask.update_metadata("Renamed", "Project resources", false, 3)
            .unwrap();
        assert_eq!(cask.state, CaskState::Active);
        assert!(cask.update_metadata("", "", false, 4).is_err());
        assert!(
            cask.update_metadata("Work", &"x".repeat(501), false, 4)
                .is_err()
        );
        cask.state = CaskState::RecoveryRequired;
        assert!(cask.update_metadata("Work", "", false, 4).is_err());
    }
}
