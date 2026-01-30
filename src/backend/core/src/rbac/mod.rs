//! Role-Based Access Control (RBAC) and multi-tenancy foundations.
//!
//! This module provides:
//! - **Models**: Role, Permission, User, Organization data structures
//! - **Policy Engine**: Evaluates whether a user can perform an action on a resource
//! - **Predefined Roles**: Admin, Operator, Viewer, Developer with default permission sets
//! - **Authorization Middleware**: Axum middleware for request-level permission checks
//! - **Tenant Isolation**: Organization-scoped resource access
//!
//! # Usage
//!
//! ```rust,ignore
//! use apex_core::rbac::{
//!     PolicyEngine, Permission, RequirePermission,
//!     roles::PredefinedRole,
//! };
//!
//! // Check permissions programmatically
//! let engine = PolicyEngine::new();
//! engine.load_role_permissions(PredefinedRole::all_defaults());
//!
//! let allowed = engine.check(
//!     &user_id,
//!     &Permission::new("swarm", "create"),
//!     Some(&org_id),
//! );
//!
//! // Use as Axum middleware
//! let app = Router::new()
//!     .route("/api/v1/swarms", post(create_swarm))
//!     .layer(RequirePermissionLayer::new(engine, "swarm:create"));
//! ```

pub mod models;
pub mod policy;
pub mod middleware;
pub mod roles;

pub use models::{
    Permission, Role, RoleId, RoleBinding, UserId, OrganizationId,
    Organization, OrganizationMember, OrganizationStatus, MemberRole,
    ResourceScope,
};
pub use policy::{PolicyEngine, PolicyDecision, PolicyError};
pub use middleware::{
    RequirePermissionLayer, RequirePermissionService, RbacContext,
};
pub use roles::PredefinedRole;
