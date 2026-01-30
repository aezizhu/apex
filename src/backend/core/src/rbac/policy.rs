//! Policy engine for evaluating authorization decisions.
//!
//! The policy engine answers the question:
//! "Can user X perform action Y on resource Z within organization O?"

use dashmap::DashMap;
use std::collections::HashSet;
use std::sync::Arc;
use thiserror::Error;
use tracing::{debug, warn};

use super::models::{
    OrganizationId, Permission, Role, RoleBinding, RoleId, UserId,
};

// ═══════════════════════════════════════════════════════════════════════════════
// Errors
// ═══════════════════════════════════════════════════════════════════════════════

/// Errors from the policy engine.
#[derive(Debug, Error)]
pub enum PolicyError {
    #[error("Role not found: {0}")]
    RoleNotFound(String),

    #[error("Organization not found: {0}")]
    OrganizationNotFound(String),

    #[error("User has no bindings in organization: user={user}, org={org}")]
    NoBindings { user: String, org: String },

    #[error("Permission denied: {0}")]
    PermissionDenied(String),
}

// ═══════════════════════════════════════════════════════════════════════════════
// Decision
// ═══════════════════════════════════════════════════════════════════════════════

/// Result of a policy evaluation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyDecision {
    /// The action is allowed.
    Allow,
    /// The action is denied, with a reason.
    Deny(String),
}

impl PolicyDecision {
    pub fn is_allowed(&self) -> bool {
        matches!(self, Self::Allow)
    }

    pub fn is_denied(&self) -> bool {
        matches!(self, Self::Deny(_))
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Policy Engine
// ═══════════════════════════════════════════════════════════════════════════════

/// The central policy engine that stores roles and bindings and evaluates
/// authorization checks.
///
/// Thread-safe via `DashMap`.
#[derive(Debug, Clone)]
pub struct PolicyEngine {
    /// Roles indexed by role id.
    roles: Arc<DashMap<RoleId, Role>>,

    /// User role bindings: key = (UserId, OrganizationId), value = set of RoleIds.
    bindings: Arc<DashMap<(UserId, OrganizationId), HashSet<RoleId>>>,
}

impl PolicyEngine {
    /// Create an empty policy engine.
    pub fn new() -> Self {
        Self {
            roles: Arc::new(DashMap::new()),
            bindings: Arc::new(DashMap::new()),
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Role management
    // ─────────────────────────────────────────────────────────────────────────

    /// Register a role.
    pub fn add_role(&self, role: Role) {
        debug!(role_id = %role.id, "Adding role to policy engine");
        self.roles.insert(role.id.clone(), role);
    }

    /// Load multiple roles (e.g., predefined defaults).
    pub fn load_roles(&self, roles: Vec<Role>) {
        for role in roles {
            self.add_role(role);
        }
    }

    /// Get a role by id.
    pub fn get_role(&self, role_id: &RoleId) -> Option<Role> {
        self.roles.get(role_id).map(|r| r.clone())
    }

    /// Remove a role. Returns `false` if the role is a system role or not found.
    pub fn remove_role(&self, role_id: &RoleId) -> bool {
        if let Some(role) = self.roles.get(role_id) {
            if role.is_system {
                warn!(role_id = %role_id, "Cannot remove system role");
                return false;
            }
        }
        self.roles.remove(role_id).is_some()
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Binding management
    // ─────────────────────────────────────────────────────────────────────────

    /// Bind a user to a role within an organization.
    pub fn bind_role(&self, binding: RoleBinding) {
        if !binding.is_active() {
            debug!(
                user_id = %binding.user_id,
                role_id = %binding.role_id,
                "Skipping expired binding"
            );
            return;
        }

        let key = (binding.user_id.clone(), binding.organization_id.clone());
        self.bindings
            .entry(key)
            .or_insert_with(HashSet::new)
            .insert(binding.role_id);
    }

    /// Remove a user's binding to a role in an organization.
    pub fn unbind_role(
        &self,
        user_id: &UserId,
        role_id: &RoleId,
        organization_id: &OrganizationId,
    ) -> bool {
        let key = (user_id.clone(), organization_id.clone());
        if let Some(mut role_ids) = self.bindings.get_mut(&key) {
            role_ids.remove(role_id)
        } else {
            false
        }
    }

    /// Get all role IDs bound to a user in a specific organization.
    pub fn user_roles(
        &self,
        user_id: &UserId,
        organization_id: &OrganizationId,
    ) -> HashSet<RoleId> {
        let key = (user_id.clone(), organization_id.clone());
        self.bindings
            .get(&key)
            .map(|r| r.clone())
            .unwrap_or_default()
    }

    /// Get all effective permissions for a user in an organization.
    pub fn effective_permissions(
        &self,
        user_id: &UserId,
        organization_id: &OrganizationId,
    ) -> HashSet<Permission> {
        let role_ids = self.user_roles(user_id, organization_id);
        let mut perms = HashSet::new();

        for role_id in &role_ids {
            if let Some(role) = self.roles.get(role_id) {
                perms.extend(role.permissions.iter().cloned());
            }
        }

        perms
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Authorization checks
    // ─────────────────────────────────────────────────────────────────────────

    /// Check if a user has a specific permission within an organization.
    ///
    /// Returns a `PolicyDecision` indicating allow or deny.
    pub fn check(
        &self,
        user_id: &UserId,
        permission: &Permission,
        organization_id: &OrganizationId,
    ) -> PolicyDecision {
        let role_ids = self.user_roles(user_id, organization_id);

        if role_ids.is_empty() {
            return PolicyDecision::Deny(format!(
                "User {} has no roles in organization {}",
                user_id, organization_id
            ));
        }

        for role_id in &role_ids {
            if let Some(role) = self.roles.get(role_id) {
                if role.has_permission(permission) {
                    debug!(
                        user_id = %user_id,
                        permission = %permission,
                        role = %role_id,
                        "Permission granted"
                    );
                    return PolicyDecision::Allow;
                }
            }
        }

        PolicyDecision::Deny(format!(
            "User {} does not have permission {} in organization {}",
            user_id, permission, organization_id
        ))
    }

    /// Convenience: returns `Ok(())` if allowed, `Err(PolicyError)` if denied.
    pub fn enforce(
        &self,
        user_id: &UserId,
        permission: &Permission,
        organization_id: &OrganizationId,
    ) -> Result<(), PolicyError> {
        match self.check(user_id, permission, organization_id) {
            PolicyDecision::Allow => Ok(()),
            PolicyDecision::Deny(reason) => Err(PolicyError::PermissionDenied(reason)),
        }
    }

    /// Check multiple permissions; returns `Allow` only if ALL are granted.
    pub fn check_all(
        &self,
        user_id: &UserId,
        permissions: &[Permission],
        organization_id: &OrganizationId,
    ) -> PolicyDecision {
        for perm in permissions {
            let decision = self.check(user_id, perm, organization_id);
            if decision.is_denied() {
                return decision;
            }
        }
        PolicyDecision::Allow
    }

    /// Check multiple permissions; returns `Allow` if ANY is granted.
    pub fn check_any(
        &self,
        user_id: &UserId,
        permissions: &[Permission],
        organization_id: &OrganizationId,
    ) -> PolicyDecision {
        for perm in permissions {
            if self.check(user_id, perm, organization_id).is_allowed() {
                return PolicyDecision::Allow;
            }
        }
        PolicyDecision::Deny(format!(
            "User {} does not have any of the required permissions in organization {}",
            user_id, organization_id
        ))
    }
}

impl Default for PolicyEngine {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rbac::roles::PredefinedRole;

    fn setup_engine() -> PolicyEngine {
        let engine = PolicyEngine::new();
        engine.load_roles(PredefinedRole::all_defaults());
        engine
    }

    fn org(id: &str) -> OrganizationId {
        OrganizationId::new(id)
    }

    fn user(id: &str) -> UserId {
        UserId::new(id)
    }

    fn bind(engine: &PolicyEngine, uid: &str, role: &str, oid: &str) {
        engine.bind_role(RoleBinding::new(
            UserId::new(uid),
            RoleId::new(role),
            OrganizationId::new(oid),
        ));
    }

    #[test]
    fn test_admin_allowed_everything() {
        let engine = setup_engine();
        bind(&engine, "alice", "admin", "org1");

        assert!(engine
            .check(
                &user("alice"),
                &Permission::new("swarm", "create"),
                &org("org1"),
            )
            .is_allowed());
        assert!(engine
            .check(
                &user("alice"),
                &Permission::new("settings", "manage"),
                &org("org1"),
            )
            .is_allowed());
        assert!(engine
            .check(
                &user("alice"),
                &Permission::new("anything", "goes"),
                &org("org1"),
            )
            .is_allowed());
    }

    #[test]
    fn test_viewer_read_only() {
        let engine = setup_engine();
        bind(&engine, "bob", "viewer", "org1");

        assert!(engine
            .check(
                &user("bob"),
                &Permission::new("swarm", "read"),
                &org("org1"),
            )
            .is_allowed());

        assert!(engine
            .check(
                &user("bob"),
                &Permission::new("swarm", "create"),
                &org("org1"),
            )
            .is_denied());

        assert!(engine
            .check(
                &user("bob"),
                &Permission::new("settings", "manage"),
                &org("org1"),
            )
            .is_denied());
    }

    #[test]
    fn test_operator_cannot_manage_settings() {
        let engine = setup_engine();
        bind(&engine, "charlie", "operator", "org1");

        assert!(engine
            .check(
                &user("charlie"),
                &Permission::new("swarm", "create"),
                &org("org1"),
            )
            .is_allowed());

        assert!(engine
            .check(
                &user("charlie"),
                &Permission::new("approval", "approve"),
                &org("org1"),
            )
            .is_allowed());

        assert!(engine
            .check(
                &user("charlie"),
                &Permission::new("settings", "manage"),
                &org("org1"),
            )
            .is_denied());
    }

    #[test]
    fn test_developer_cannot_delete_swarms() {
        let engine = setup_engine();
        bind(&engine, "dave", "developer", "org1");

        assert!(engine
            .check(
                &user("dave"),
                &Permission::new("swarm", "create"),
                &org("org1"),
            )
            .is_allowed());

        assert!(engine
            .check(
                &user("dave"),
                &Permission::new("swarm", "delete"),
                &org("org1"),
            )
            .is_denied());
    }

    #[test]
    fn test_no_roles_denied() {
        let engine = setup_engine();

        assert!(engine
            .check(
                &user("nobody"),
                &Permission::new("swarm", "read"),
                &org("org1"),
            )
            .is_denied());
    }

    #[test]
    fn test_tenant_isolation() {
        let engine = setup_engine();
        bind(&engine, "alice", "admin", "org1");

        // Alice is admin in org1, but has no access in org2.
        assert!(engine
            .check(
                &user("alice"),
                &Permission::new("swarm", "create"),
                &org("org1"),
            )
            .is_allowed());

        assert!(engine
            .check(
                &user("alice"),
                &Permission::new("swarm", "create"),
                &org("org2"),
            )
            .is_denied());
    }

    #[test]
    fn test_multiple_roles() {
        let engine = setup_engine();
        bind(&engine, "eve", "viewer", "org1");
        bind(&engine, "eve", "developer", "org1");

        // Eve gets combined permissions from both roles.
        assert!(engine
            .check(
                &user("eve"),
                &Permission::new("swarm", "read"),
                &org("org1"),
            )
            .is_allowed());
        assert!(engine
            .check(
                &user("eve"),
                &Permission::new("swarm", "create"),
                &org("org1"),
            )
            .is_allowed());
        // But still no settings:manage
        assert!(engine
            .check(
                &user("eve"),
                &Permission::new("settings", "manage"),
                &org("org1"),
            )
            .is_denied());
    }

    #[test]
    fn test_enforce() {
        let engine = setup_engine();
        bind(&engine, "alice", "admin", "org1");

        assert!(engine
            .enforce(
                &user("alice"),
                &Permission::new("swarm", "create"),
                &org("org1"),
            )
            .is_ok());

        assert!(engine
            .enforce(
                &user("nobody"),
                &Permission::new("swarm", "create"),
                &org("org1"),
            )
            .is_err());
    }

    #[test]
    fn test_check_all() {
        let engine = setup_engine();
        bind(&engine, "alice", "operator", "org1");

        let perms = vec![
            Permission::new("swarm", "create"),
            Permission::new("swarm", "read"),
        ];
        assert!(engine
            .check_all(&user("alice"), &perms, &org("org1"))
            .is_allowed());

        let perms_with_settings = vec![
            Permission::new("swarm", "create"),
            Permission::new("settings", "manage"),
        ];
        assert!(engine
            .check_all(&user("alice"), &perms_with_settings, &org("org1"))
            .is_denied());
    }

    #[test]
    fn test_check_any() {
        let engine = setup_engine();
        bind(&engine, "bob", "viewer", "org1");

        let perms = vec![
            Permission::new("swarm", "create"),
            Permission::new("swarm", "read"),
        ];
        assert!(engine
            .check_any(&user("bob"), &perms, &org("org1"))
            .is_allowed());

        let perms_write = vec![
            Permission::new("swarm", "create"),
            Permission::new("swarm", "delete"),
        ];
        assert!(engine
            .check_any(&user("bob"), &perms_write, &org("org1"))
            .is_denied());
    }

    #[test]
    fn test_unbind_role() {
        let engine = setup_engine();
        bind(&engine, "alice", "admin", "org1");

        assert!(engine
            .check(
                &user("alice"),
                &Permission::new("swarm", "create"),
                &org("org1"),
            )
            .is_allowed());

        let removed = engine.unbind_role(
            &user("alice"),
            &RoleId::new("admin"),
            &org("org1"),
        );
        assert!(removed);

        assert!(engine
            .check(
                &user("alice"),
                &Permission::new("swarm", "create"),
                &org("org1"),
            )
            .is_denied());
    }

    #[test]
    fn test_effective_permissions() {
        let engine = setup_engine();
        bind(&engine, "eve", "viewer", "org1");
        bind(&engine, "eve", "developer", "org1");

        let perms = engine.effective_permissions(&user("eve"), &org("org1"));
        assert!(perms.contains(&Permission::new("swarm", "read")));
        assert!(perms.contains(&Permission::new("swarm", "create")));
        assert!(perms.contains(&Permission::new("task", "submit")));
    }

    #[test]
    fn test_expired_binding_ignored() {
        let engine = setup_engine();
        let expired = RoleBinding::new(
            UserId::new("alice"),
            RoleId::new("admin"),
            OrganizationId::new("org1"),
        )
        .with_expiry(chrono::Utc::now() - chrono::Duration::hours(1));

        engine.bind_role(expired);

        assert!(engine
            .check(
                &user("alice"),
                &Permission::new("swarm", "create"),
                &org("org1"),
            )
            .is_denied());
    }

    #[test]
    fn test_cannot_remove_system_role() {
        let engine = setup_engine();
        let removed = engine.remove_role(&RoleId::new("admin"));
        assert!(!removed);
        // System role should still exist
        assert!(engine.get_role(&RoleId::new("admin")).is_some());
    }
}
