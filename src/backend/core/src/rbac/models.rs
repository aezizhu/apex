//! RBAC data models: Role, Permission, User identity, Organization, and bindings.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fmt;
use uuid::Uuid;

// ═══════════════════════════════════════════════════════════════════════════════
// Identifiers
// ═══════════════════════════════════════════════════════════════════════════════

/// Strongly-typed user identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UserId(pub String);

impl UserId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for UserId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for UserId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for UserId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

/// Strongly-typed role identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RoleId(pub String);

impl RoleId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RoleId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for RoleId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for RoleId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

/// Strongly-typed organization identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct OrganizationId(pub String);

impl OrganizationId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn from_uuid() -> Self {
        Self(Uuid::new_v4().to_string())
    }
}

impl fmt::Display for OrganizationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for OrganizationId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for OrganizationId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Permission
// ═══════════════════════════════════════════════════════════════════════════════

/// A permission represents an action on a resource type.
///
/// Permissions follow the format `resource:action`, for example:
/// - `swarm:create`
/// - `agent:manage`
/// - `task:submit`
/// - `approval:approve`
/// - `settings:manage`
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Permission {
    /// The resource type (e.g., "swarm", "agent", "task").
    pub resource: String,
    /// The action (e.g., "create", "read", "delete", "manage").
    pub action: String,
}

impl Permission {
    /// Create a new permission.
    pub fn new(resource: impl Into<String>, action: impl Into<String>) -> Self {
        Self {
            resource: resource.into(),
            action: action.into(),
        }
    }

    /// Parse a permission from a colon-separated string like `"swarm:create"`.
    pub fn parse(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.splitn(2, ':').collect();
        if parts.len() == 2 {
            Some(Self::new(parts[0], parts[1]))
        } else {
            None
        }
    }

    /// Return the canonical string form `"resource:action"`.
    pub fn as_string(&self) -> String {
        format!("{}:{}", self.resource, self.action)
    }

    /// Check if this permission matches another, supporting wildcards.
    ///
    /// A wildcard `"*"` in either resource or action matches anything.
    pub fn matches(&self, other: &Permission) -> bool {
        let resource_match =
            self.resource == "*" || other.resource == "*" || self.resource == other.resource;
        let action_match =
            self.action == "*" || other.action == "*" || self.action == other.action;
        resource_match && action_match
    }
}

impl fmt::Display for Permission {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.resource, self.action)
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Role
// ═══════════════════════════════════════════════════════════════════════════════

/// A role groups a set of permissions under a named identity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Role {
    /// Unique role identifier.
    pub id: RoleId,
    /// Human-readable name.
    pub name: String,
    /// Description of the role's purpose.
    pub description: String,
    /// Set of permissions granted by this role.
    pub permissions: HashSet<Permission>,
    /// Whether this is a built-in system role (cannot be deleted).
    pub is_system: bool,
    /// Optional organization scope (None = global role).
    pub organization_id: Option<OrganizationId>,
    /// When the role was created.
    pub created_at: DateTime<Utc>,
    /// When the role was last updated.
    pub updated_at: DateTime<Utc>,
}

impl Role {
    /// Create a new role with the given permissions.
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        description: impl Into<String>,
        permissions: HashSet<Permission>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: RoleId::new(id),
            name: name.into(),
            description: description.into(),
            permissions,
            is_system: false,
            organization_id: None,
            created_at: now,
            updated_at: now,
        }
    }

    /// Mark this as a system role.
    pub fn system(mut self) -> Self {
        self.is_system = true;
        self
    }

    /// Scope this role to an organization.
    pub fn with_organization(mut self, org_id: OrganizationId) -> Self {
        self.organization_id = Some(org_id);
        self
    }

    /// Check if this role grants a specific permission.
    pub fn has_permission(&self, permission: &Permission) -> bool {
        self.permissions.iter().any(|p| p.matches(permission))
    }

    /// Add a permission to this role.
    pub fn grant(&mut self, permission: Permission) {
        self.permissions.insert(permission);
        self.updated_at = Utc::now();
    }

    /// Remove a permission from this role.
    pub fn revoke(&mut self, permission: &Permission) -> bool {
        let removed = self.permissions.remove(permission);
        if removed {
            self.updated_at = Utc::now();
        }
        removed
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Role Binding
// ═══════════════════════════════════════════════════════════════════════════════

/// Binds a user to a role within an organization scope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleBinding {
    /// The user this binding applies to.
    pub user_id: UserId,
    /// The role being assigned.
    pub role_id: RoleId,
    /// Organization scope for this binding.
    pub organization_id: OrganizationId,
    /// When the binding was created.
    pub created_at: DateTime<Utc>,
    /// When the binding expires (None = never).
    pub expires_at: Option<DateTime<Utc>>,
    /// Who granted this binding.
    pub granted_by: Option<UserId>,
}

impl RoleBinding {
    /// Create a new role binding.
    pub fn new(
        user_id: UserId,
        role_id: RoleId,
        organization_id: OrganizationId,
    ) -> Self {
        Self {
            user_id,
            role_id,
            organization_id,
            created_at: Utc::now(),
            expires_at: None,
            granted_by: None,
        }
    }

    /// Set expiration.
    pub fn with_expiry(mut self, expires_at: DateTime<Utc>) -> Self {
        self.expires_at = Some(expires_at);
        self
    }

    /// Record who granted this binding.
    pub fn granted_by(mut self, user_id: UserId) -> Self {
        self.granted_by = Some(user_id);
        self
    }

    /// Check if this binding is currently active (not expired).
    pub fn is_active(&self) -> bool {
        self.expires_at.map_or(true, |exp| Utc::now() < exp)
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Organization (Multi-Tenancy)
// ═══════════════════════════════════════════════════════════════════════════════

/// Organization status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrganizationStatus {
    Active,
    Suspended,
    Deactivated,
}

/// An organization (tenant) that owns resources.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Organization {
    pub id: OrganizationId,
    pub name: String,
    pub slug: String,
    pub status: OrganizationStatus,
    pub owner_id: UserId,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub settings: serde_json::Value,
}

impl Organization {
    /// Create a new organization.
    pub fn new(
        name: impl Into<String>,
        slug: impl Into<String>,
        owner_id: UserId,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: OrganizationId::from_uuid(),
            name: name.into(),
            slug: slug.into(),
            status: OrganizationStatus::Active,
            owner_id,
            created_at: now,
            updated_at: now,
            settings: serde_json::json!({}),
        }
    }

    /// Check if the organization is active.
    pub fn is_active(&self) -> bool {
        self.status == OrganizationStatus::Active
    }
}

/// Membership record linking a user to an organization with a role.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrganizationMember {
    pub user_id: UserId,
    pub organization_id: OrganizationId,
    pub role: MemberRole,
    pub joined_at: DateTime<Utc>,
}

/// Simple organization-level role (separate from RBAC roles for membership).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemberRole {
    Owner,
    Admin,
    Member,
}

// ═══════════════════════════════════════════════════════════════════════════════
// Resource Scope (tenant isolation)
// ═══════════════════════════════════════════════════════════════════════════════

/// Describes the scope of a resource for tenant isolation.
///
/// Every query against a tenant-scoped resource must include an
/// `organization_id` filter to prevent cross-tenant data leakage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceScope {
    /// The organization that owns this resource.
    pub organization_id: OrganizationId,
    /// Optional sub-scope (e.g., project, team).
    pub project_id: Option<String>,
}

impl ResourceScope {
    pub fn org(organization_id: OrganizationId) -> Self {
        Self {
            organization_id,
            project_id: None,
        }
    }

    pub fn project(organization_id: OrganizationId, project_id: impl Into<String>) -> Self {
        Self {
            organization_id,
            project_id: Some(project_id.into()),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_permission_parse() {
        let perm = Permission::parse("swarm:create").unwrap();
        assert_eq!(perm.resource, "swarm");
        assert_eq!(perm.action, "create");
        assert_eq!(perm.as_string(), "swarm:create");

        assert!(Permission::parse("invalid").is_none());
    }

    #[test]
    fn test_permission_matches_exact() {
        let a = Permission::new("swarm", "create");
        let b = Permission::new("swarm", "create");
        assert!(a.matches(&b));
    }

    #[test]
    fn test_permission_matches_wildcard() {
        let wildcard = Permission::new("*", "*");
        let specific = Permission::new("swarm", "create");
        assert!(wildcard.matches(&specific));

        let resource_wild = Permission::new("*", "create");
        assert!(resource_wild.matches(&specific));

        let action_wild = Permission::new("swarm", "*");
        assert!(action_wild.matches(&specific));
    }

    #[test]
    fn test_permission_no_match() {
        let a = Permission::new("swarm", "create");
        let b = Permission::new("agent", "manage");
        assert!(!a.matches(&b));
    }

    #[test]
    fn test_role_has_permission() {
        let mut perms = HashSet::new();
        perms.insert(Permission::new("swarm", "create"));
        perms.insert(Permission::new("swarm", "read"));

        let role = Role::new("editor", "Editor", "Can manage swarms", perms);

        assert!(role.has_permission(&Permission::new("swarm", "create")));
        assert!(role.has_permission(&Permission::new("swarm", "read")));
        assert!(!role.has_permission(&Permission::new("swarm", "delete")));
    }

    #[test]
    fn test_role_wildcard_permission() {
        let mut perms = HashSet::new();
        perms.insert(Permission::new("*", "*"));

        let role = Role::new("superadmin", "Super Admin", "All access", perms);

        assert!(role.has_permission(&Permission::new("swarm", "create")));
        assert!(role.has_permission(&Permission::new("agent", "manage")));
        assert!(role.has_permission(&Permission::new("settings", "manage")));
    }

    #[test]
    fn test_role_grant_revoke() {
        let mut role = Role::new("custom", "Custom", "Custom role", HashSet::new());

        assert!(!role.has_permission(&Permission::new("swarm", "create")));

        role.grant(Permission::new("swarm", "create"));
        assert!(role.has_permission(&Permission::new("swarm", "create")));

        let removed = role.revoke(&Permission::new("swarm", "create"));
        assert!(removed);
        assert!(!role.has_permission(&Permission::new("swarm", "create")));
    }

    #[test]
    fn test_role_binding_active() {
        let binding = RoleBinding::new(
            UserId::new("user1"),
            RoleId::new("admin"),
            OrganizationId::new("org1"),
        );
        assert!(binding.is_active());

        let expired = RoleBinding::new(
            UserId::new("user2"),
            RoleId::new("viewer"),
            OrganizationId::new("org1"),
        )
        .with_expiry(Utc::now() - chrono::Duration::hours(1));
        assert!(!expired.is_active());
    }

    #[test]
    fn test_organization_creation() {
        let org = Organization::new("Acme Corp", "acme-corp", UserId::new("user1"));
        assert!(org.is_active());
        assert_eq!(org.name, "Acme Corp");
        assert_eq!(org.slug, "acme-corp");
    }

    #[test]
    fn test_resource_scope() {
        let scope = ResourceScope::org(OrganizationId::new("org1"));
        assert_eq!(scope.organization_id.as_str(), "org1");
        assert!(scope.project_id.is_none());

        let scoped = ResourceScope::project(OrganizationId::new("org1"), "proj-a");
        assert_eq!(scoped.project_id, Some("proj-a".to_string()));
    }
}
