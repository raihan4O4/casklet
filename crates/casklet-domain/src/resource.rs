//! Resource identity, association, and storage semantics.
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntakeMode {
    Copy,
    Move,
    Reference,
}
impl IntakeMode {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Copy => "COPY",
            Self::Move => "MOVE",
            Self::Reference => "REFERENCE",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceKind {
    File,
    Directory,
}
impl ResourceKind {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::File => "FILE",
            Self::Directory => "DIRECTORY",
        }
    }
    #[must_use]
    pub fn from_code(code: &str) -> Option<Self> {
        match code {
            "FILE" => Some(Self::File),
            "DIRECTORY" => Some(Self::Directory),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Management {
    Managed,
    Referenced,
}
impl Management {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Managed => "MANAGED",
            Self::Referenced => "REFERENCED",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resource {
    pub id: String,
    pub cask_id: String,
    pub name: String,
    pub path: String,
    pub kind: ResourceKind,
    pub management: Management,
    pub category_key: String,
    pub size_bytes: Option<u64>,
    pub size_measurement: &'static str,
    pub unknown_entries: u64,
    pub created_at_ms: i64,
    pub identity_key: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceError {
    EmptyName,
    InvalidCategory,
}

impl Resource {
    /// # Errors
    /// Rejects empty names and categories.
    #[expect(
        clippy::too_many_arguments,
        reason = "resource creation requires all intake facts"
    )]
    pub fn new(
        cask_id: String,
        name: String,
        path: String,
        kind: ResourceKind,
        mode: IntakeMode,
        category_key: String,
        size_bytes: Option<u64>,
        unknown_entries: u64,
        created_at_ms: i64,
    ) -> Result<Self, ResourceError> {
        if name.is_empty() || name.chars().any(char::is_control) {
            return Err(ResourceError::EmptyName);
        }
        if category_key.is_empty() {
            return Err(ResourceError::InvalidCategory);
        }
        let management = if mode == IntakeMode::Reference {
            Management::Referenced
        } else {
            Management::Managed
        };
        let size_measurement = if size_bytes.is_none() {
            "UNKNOWN"
        } else if unknown_entries > 0 {
            "ESTIMATED"
        } else if kind == ResourceKind::Directory {
            "CALCULATED"
        } else {
            "EXACT"
        };
        Ok(Self {
            id: Uuid::new_v4().to_string(),
            cask_id,
            name,
            path,
            kind,
            management,
            category_key,
            size_bytes,
            size_measurement,
            unknown_entries,
            created_at_ms,
            identity_key: None,
        })
    }
    #[must_use]
    pub const fn containment(&self) -> &'static str {
        match self.management {
            Management::Managed => "INTERNAL",
            Management::Referenced => "EXTERNAL",
        }
    }
    #[must_use]
    pub const fn relationship(&self) -> &'static str {
        match self.management {
            Management::Managed => "OWNED",
            Management::Referenced => "REFERENCED",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reference_never_grants_ownership() {
        let resource = Resource::new(
            "c".into(),
            "x".into(),
            "x".into(),
            ResourceKind::File,
            IntakeMode::Reference,
            "other".into(),
            Some(10),
            0,
            0,
        )
        .unwrap();
        assert_eq!(resource.relationship(), "REFERENCED");
        assert_eq!(resource.containment(), "EXTERNAL");
        assert_eq!(resource.management, Management::Referenced);
    }
}
