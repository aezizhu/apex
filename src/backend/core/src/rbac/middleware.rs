//! Axum authorization middleware that enforces RBAC permissions on requests.
//!
//! This middleware reads the `AuthContext` (injected by the auth middleware)
//! and checks the policy engine to decide whether the request should proceed.

use axum::{
    body::Body,
    extract::{FromRequestParts, Request},
    http::{request::Parts, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use futures::future::BoxFuture;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::task::{Context, Poll};
use tower::{Layer, Service};
use tracing::warn;

use super::models::{OrganizationId, Permission, UserId};
use super::policy::PolicyEngine;
use crate::middleware::auth::AuthContext;

// ═══════════════════════════════════════════════════════════════════════════════
// RBAC Context (extracted in handlers)
// ═══════════════════════════════════════════════════════════════════════════════

/// Extended authorization context that includes RBAC-resolved information.
///
/// Inserted into request extensions by the authorization middleware so that
/// downstream handlers can access the verified user, organization, and
/// effective permissions without re-evaluating the policy.
#[derive(Debug, Clone)]
pub struct RbacContext {
    /// The authenticated user id.
    pub user_id: UserId,
    /// The organization scope for this request.
    pub organization_id: OrganizationId,
    /// The permission that was checked (if middleware was applied).
    pub checked_permission: Option<Permission>,
}

/// Axum extractor for `RbacContext`.
#[axum::async_trait]
impl<S> FromRequestParts<S> for RbacContext
where
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<RbacContext>()
            .cloned()
            .ok_or_else(|| {
                let body = serde_json::json!({
                    "success": false,
                    "error": {
                        "code": "MISSING_RBAC_CONTEXT",
                        "message": "Authorization context not available. Ensure RBAC middleware is applied.",
                    }
                });
                (StatusCode::INTERNAL_SERVER_ERROR, Json(body)).into_response()
            })
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tower Layer
// ═══════════════════════════════════════════════════════════════════════════════

/// Layer that wraps services with permission enforcement.
///
/// # Example
///
/// ```rust,ignore
/// use apex_core::rbac::{RequirePermissionLayer, PolicyEngine};
///
/// let engine = PolicyEngine::new();
/// // ... load roles and bindings ...
///
/// let app = Router::new()
///     .route("/api/v1/swarms", post(create_swarm))
///     .layer(RequirePermissionLayer::new(engine.clone(), "swarm:create"));
/// ```
#[derive(Clone)]
pub struct RequirePermissionLayer {
    engine: Arc<PolicyEngine>,
    permission: Permission,
}

impl RequirePermissionLayer {
    /// Create a new layer requiring the given permission (e.g., `"swarm:create"`).
    pub fn new(engine: Arc<PolicyEngine>, permission_str: &str) -> Self {
        let permission = Permission::parse(permission_str)
            .unwrap_or_else(|| Permission::new(permission_str, "*"));
        Self { engine, permission }
    }

    /// Create from an already-parsed `Permission`.
    pub fn from_permission(engine: Arc<PolicyEngine>, permission: Permission) -> Self {
        Self { engine, permission }
    }
}

impl<S> Layer<S> for RequirePermissionLayer {
    type Service = RequirePermissionService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        RequirePermissionService {
            inner,
            engine: self.engine.clone(),
            permission: self.permission.clone(),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tower Service
// ═══════════════════════════════════════════════════════════════════════════════

/// Service that enforces a required permission per request.
#[derive(Clone)]
pub struct RequirePermissionService<S> {
    inner: S,
    engine: Arc<PolicyEngine>,
    permission: Permission,
}

impl<S> Service<Request<Body>> for RequirePermissionService<S>
where
    S: Service<Request<Body>, Response = Response> + Clone + Send + 'static,
    S::Future: Send,
{
    type Response = Response;
    type Error = S::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut request: Request<Body>) -> Self::Future {
        let engine = self.engine.clone();
        let permission = self.permission.clone();
        let mut inner = self.inner.clone();

        Box::pin(async move {
            // Extract auth context set by upstream auth middleware.
            let auth_ctx = request
                .extensions()
                .get::<AuthContext>()
                .cloned();

            let auth_ctx = match auth_ctx {
                Some(ctx) if ctx.is_authenticated() => ctx,
                _ => {
                    return Ok(forbidden_response(
                        "Authentication required for this resource",
                    ));
                }
            };

            // Determine organization from auth context.
            let org_id = match &auth_ctx.org_id {
                Some(id) => OrganizationId::new(id),
                None => {
                    return Ok(forbidden_response(
                        "Organization context required. Include org_id in your token.",
                    ));
                }
            };

            let user_id = UserId::new(&auth_ctx.user_id);

            // Evaluate policy.
            let decision = engine.check(&user_id, &permission, &org_id);

            if decision.is_denied() {
                warn!(
                    user_id = %user_id,
                    permission = %permission,
                    org_id = %org_id,
                    "Permission denied"
                );
                return Ok(forbidden_response(&format!(
                    "You do not have permission: {}",
                    permission
                )));
            }

            // Inject RBAC context for downstream handlers.
            let rbac_ctx = RbacContext {
                user_id,
                organization_id: org_id,
                checked_permission: Some(permission),
            };
            request.extensions_mut().insert(rbac_ctx);

            inner.call(request).await
        })
    }
}

/// Build a 403 Forbidden JSON response.
fn forbidden_response(message: &str) -> Response {
    let body = serde_json::json!({
        "success": false,
        "error": {
            "code": "FORBIDDEN",
            "message": message,
        }
    });
    (StatusCode::FORBIDDEN, Json(body)).into_response()
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::middleware::auth::{AuthContext, AuthMethod};
    use crate::rbac::models::RoleBinding;
    use crate::rbac::roles::PredefinedRole;

    fn setup_engine_with_user(
        uid: &str,
        role: &str,
        oid: &str,
    ) -> Arc<PolicyEngine> {
        let engine = PolicyEngine::new();
        engine.load_roles(PredefinedRole::all_defaults());
        engine.bind_role(RoleBinding::new(
            UserId::new(uid),
            super::super::models::RoleId::new(role),
            OrganizationId::new(oid),
        ));
        Arc::new(engine)
    }

    fn make_auth_context(user_id: &str, org_id: Option<&str>) -> AuthContext {
        AuthContext {
            user_id: user_id.to_string(),
            email: None,
            name: None,
            roles: vec![],
            org_id: org_id.map(|s| s.to_string()),
            auth_method: AuthMethod::Jwt,
            token_id: None,
            expires_at: None,
            request_id: "test-req".to_string(),
        }
    }

    #[test]
    fn test_rbac_context_creation() {
        let ctx = RbacContext {
            user_id: UserId::new("alice"),
            organization_id: OrganizationId::new("org1"),
            checked_permission: Some(Permission::new("swarm", "create")),
        };

        assert_eq!(ctx.user_id.as_str(), "alice");
        assert_eq!(ctx.organization_id.as_str(), "org1");
    }

    #[test]
    fn test_require_permission_layer_parse() {
        let engine = Arc::new(PolicyEngine::new());
        let layer = RequirePermissionLayer::new(engine, "swarm:create");
        assert_eq!(layer.permission.resource, "swarm");
        assert_eq!(layer.permission.action, "create");
    }
}
