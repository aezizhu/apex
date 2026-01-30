//! Predefined roles with default permission sets.
//!
//! Apex ships with four built-in roles:
//!
//! | Role       | Description                                                  |
//! |------------|--------------------------------------------------------------|
//! | Admin      | Full access to all resources and settings                     |
//! | Operator   | Manage swarms, agents, and tasks; approve actions             |
//! | Developer  | Create and manage swarms and tasks; cannot change settings    |
//! | Viewer     | Read-only access to all resources                            |

use std::collections::HashSet;

use super::models::{Permission, Role, RoleId};

/// Predefined role templates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PredefinedRole {
    Admin,
    Operator,
    Developer,
    Viewer,
}

impl PredefinedRole {
    /// Get the role identifier string.
    pub fn id(&self) -> &'static str {
        match self {
            Self::Admin => "admin",
            Self::Operator => "operator",
            Self::Developer => "developer",
            Self::Viewer => "viewer",
        }
    }

    /// Get the human-readable name.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Admin => "Admin",
            Self::Operator => "Operator",
            Self::Developer => "Developer",
            Self::Viewer => "Viewer",
        }
    }

    /// Get the description.
    pub fn description(&self) -> &'static str {
        match self {
            Self::Admin => "Full access to all resources and organization settings",
            Self::Operator => "Manage swarms, agents, and tasks; approve actions",
            Self::Developer => "Create and manage swarms and tasks",
            Self::Viewer => "Read-only access to all resources",
        }
    }

    /// Return the set of permissions for this predefined role.
    pub fn permissions(&self) -> HashSet<Permission> {
        match self {
            Self::Admin => {
                // Admin gets wildcard: all resources, all actions.
                let mut perms = HashSet::new();
                perms.insert(Permission::new("*", "*"));
                perms
            }
            Self::Operator => {
                let mut perms = HashSet::new();
                perms.insert(Permission::new("swarm", "create"));
                perms.insert(Permission::new("swarm", "read"));
                perms.insert(Permission::new("swarm", "delete"));
                perms.insert(Permission::new("agent", "manage"));
                perms.insert(Permission::new("task", "submit"));
                perms.insert(Permission::new("task", "read"));
                perms.insert(Permission::new("approval", "approve"));
                // Operator cannot manage settings
                perms
            }
            Self::Developer => {
                let mut perms = HashSet::new();
                perms.insert(Permission::new("swarm", "create"));
                perms.insert(Permission::new("swarm", "read"));
                perms.insert(Permission::new("agent", "manage"));
                perms.insert(Permission::new("task", "submit"));
                perms.insert(Permission::new("task", "read"));
                // Developer cannot delete swarms, approve, or manage settings
                perms
            }
            Self::Viewer => {
                let mut perms = HashSet::new();
                perms.insert(Permission::new("swarm", "read"));
                perms.insert(Permission::new("agent", "read"));
                perms.insert(Permission::new("task", "read"));
                perms.insert(Permission::new("approval", "read"));
                perms.insert(Permission::new("settings", "read"));
                perms
            }
        }
    }

    /// Build a full `Role` struct from this predefined role.
    pub fn to_role(&self) -> Role {
        Role::new(
            self.id(),
            self.name(),
            self.description(),
            self.permissions(),
        )
        .system()
    }

    /// Return all predefined roles.
    pub fn all() -> Vec<PredefinedRole> {
        vec![
            Self::Admin,
            Self::Operator,
            Self::Developer,
            Self::Viewer,
        ]
    }

    /// Return all predefined roles as `Role` structs.
    pub fn all_defaults() -> Vec<Role> {
        Self::all().into_iter().map(|r| r.to_role()).collect()
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_admin_has_wildcard() {
        let role = PredefinedRole::Admin.to_role();
        assert!(role.has_permission(&Permission::new("swarm", "create")));
        assert!(role.has_permission(&Permission::new("settings", "manage")));
        assert!(role.has_permission(&Permission::new("anything", "anything")));
    }

    #[test]
    fn test_operator_permissions() {
        let role = PredefinedRole::Operator.to_role();
        assert!(role.has_permission(&Permission::new("swarm", "create")));
        assert!(role.has_permission(&Permission::new("swarm", "delete")));
        assert!(role.has_permission(&Permission::new("approval", "approve")));
        assert!(!role.has_permission(&Permission::new("settings", "manage")));
    }

    #[test]
    fn test_developer_permissions() {
        let role = PredefinedRole::Developer.to_role();
        assert!(role.has_permission(&Permission::new("swarm", "create")));
        assert!(role.has_permission(&Permission::new("task", "submit")));
        assert!(!role.has_permission(&Permission::new("swarm", "delete")));
        assert!(!role.has_permission(&Permission::new("settings", "manage")));
        assert!(!role.has_permission(&Permission::new("approval", "approve")));
    }

    #[test]
    fn test_viewer_read_only() {
        let role = PredefinedRole::Viewer.to_role();
        assert!(role.has_permission(&Permission::new("swarm", "read")));
        assert!(role.has_permission(&Permission::new("agent", "read")));
        assert!(role.has_permission(&Permission::new("task", "read")));
        assert!(!role.has_permission(&Permission::new("swarm", "create")));
        assert!(!role.has_permission(&Permission::new("swarm", "delete")));
        assert!(!role.has_permission(&Permission::new("settings", "manage")));
    }

    #[test]
    fn test_all_defaults() {
        let roles = PredefinedRole::all_defaults();
        assert_eq!(roles.len(), 4);
        assert!(roles.iter().all(|r| r.is_system));
    }

    #[test]
    fn test_role_ids() {
        assert_eq!(PredefinedRole::Admin.id(), "admin");
        assert_eq!(PredefinedRole::Operator.id(), "operator");
        assert_eq!(PredefinedRole::Developer.id(), "developer");
        assert_eq!(PredefinedRole::Viewer.id(), "viewer");
    }
}
