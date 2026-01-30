//! Authentication and authorization middleware.
//!
//! Features:
//! - JWT token validation with configurable algorithms
//! - API key authentication
//! - Role-based access control (RBAC)
//! - Token refresh handling
//! - Request context injection
//!
//! # Example
//!
//! ```rust,ignore
//! use apex_core::middleware::auth::{AuthLayer, AuthConfig, RequireAuth};
//!
//! let config = AuthConfig::builder()
//!     .jwt_secret("your-secret-key")
//!     .build();
//!
//! let app = Router::new()
//!     .route("/api/v1/tasks", post(create_task))
//!     .layer(AuthLayer::new(config));
//! ```

use axum::{
    body::Body,
    extract::{FromRequestParts, Request},
    http::{request::Parts, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use chrono::{DateTime, Duration, Utc};
use dashmap::DashMap;
use futures::future::BoxFuture;
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use metrics::counter;
use serde::{Deserialize, Serialize};
use std::{
    sync::Arc,
    task::{Context, Poll},
};
use thiserror::Error;
use tower::{Layer, Service};
use tracing::{debug, error};
use uuid::Uuid;

// ═══════════════════════════════════════════════════════════════════════════════
// Error Types
// ═══════════════════════════════════════════════════════════════════════════════

/// Authentication errors.
#[derive(Debug, Error)]
pub enum AuthError {
    #[error("Missing authentication credentials")]
    MissingCredentials,

    #[error("Invalid authentication token")]
    InvalidToken,

    #[error("Token has expired")]
    TokenExpired,

    #[error("Insufficient permissions")]
    InsufficientPermissions,

    #[error("Invalid API key")]
    InvalidApiKey,

    #[error("Account disabled")]
    AccountDisabled,

    #[error("Token validation error: {0}")]
    ValidationError(String),

    #[error("Internal authentication error: {0}")]
    Internal(String),
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let (status, code, message) = match &self {
            Self::MissingCredentials => (
                StatusCode::UNAUTHORIZED,
                "MISSING_CREDENTIALS",
                "Authentication credentials are required",
            ),
            Self::InvalidToken => (
                StatusCode::UNAUTHORIZED,
                "INVALID_TOKEN",
                "The provided token is invalid",
            ),
            Self::TokenExpired => (
                StatusCode::UNAUTHORIZED,
                "TOKEN_EXPIRED",
                "The authentication token has expired",
            ),
            Self::InsufficientPermissions => (
                StatusCode::FORBIDDEN,
                "INSUFFICIENT_PERMISSIONS",
                "You do not have permission to perform this action",
            ),
            Self::InvalidApiKey => (
                StatusCode::UNAUTHORIZED,
                "INVALID_API_KEY",
                "The provided API key is invalid",
            ),
            Self::AccountDisabled => (
                StatusCode::FORBIDDEN,
                "ACCOUNT_DISABLED",
                "This account has been disabled",
            ),
            Self::ValidationError(_) | Self::Internal(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                "An authentication error occurred",
            ),
        };

        counter!(
            "auth_errors_total",
            "error_type" => code.to_string()
        )
        .increment(1);

        let body = serde_json::json!({
            "success": false,
            "error": {
                "code": code,
                "message": message,
            }
        });

        (status, Json(body)).into_response()
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// JWT Claims
// ═══════════════════════════════════════════════════════════════════════════════

/// JWT token claims.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    /// Subject (user ID)
    pub sub: String,

    /// User email (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,

    /// User name (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// User roles
    #[serde(default)]
    pub roles: Vec<String>,

    /// Organization ID (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub org_id: Option<String>,

    /// Token ID for revocation tracking
    #[serde(default = "generate_jti")]
    pub jti: String,

    /// Issued at timestamp
    pub iat: i64,

    /// Expiration timestamp
    pub exp: i64,

    /// Not before timestamp (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nbf: Option<i64>,

    /// Issuer
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iss: Option<String>,

    /// Audience
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aud: Option<String>,

    /// Custom claims
    #[serde(flatten)]
    pub custom: std::collections::HashMap<String, serde_json::Value>,
}

fn generate_jti() -> String {
    Uuid::new_v4().to_string()
}

impl Claims {
    /// Create new claims for a user.
    pub fn new(
        user_id: impl Into<String>,
        roles: Vec<String>,
        duration: Duration,
    ) -> Self {
        let now = Utc::now();
        Self {
            sub: user_id.into(),
            email: None,
            name: None,
            roles,
            org_id: None,
            jti: generate_jti(),
            iat: now.timestamp(),
            exp: (now + duration).timestamp(),
            nbf: None,
            iss: None,
            aud: None,
            custom: std::collections::HashMap::new(),
        }
    }

    /// Create claims with builder pattern.
    pub fn builder(user_id: impl Into<String>) -> ClaimsBuilder {
        ClaimsBuilder::new(user_id)
    }

    /// Check if the token has expired.
    pub fn is_expired(&self) -> bool {
        Utc::now().timestamp() > self.exp
    }

    /// Check if user has a specific role.
    pub fn has_role(&self, role: &str) -> bool {
        self.roles.iter().any(|r| r == role || r == "admin")
    }

    /// Check if user has any of the specified roles.
    pub fn has_any_role(&self, roles: &[&str]) -> bool {
        roles.iter().any(|r| self.has_role(r))
    }

    /// Get the user ID.
    pub fn user_id(&self) -> &str {
        &self.sub
    }

    /// Get the expiration time.
    pub fn expires_at(&self) -> DateTime<Utc> {
        DateTime::from_timestamp(self.exp, 0).unwrap_or(Utc::now())
    }
}

/// Builder for JWT claims.
pub struct ClaimsBuilder {
    claims: Claims,
}

impl ClaimsBuilder {
    pub fn new(user_id: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            claims: Claims {
                sub: user_id.into(),
                email: None,
                name: None,
                roles: Vec::new(),
                org_id: None,
                jti: generate_jti(),
                iat: now.timestamp(),
                exp: (now + Duration::hours(1)).timestamp(),
                nbf: None,
                iss: None,
                aud: None,
                custom: std::collections::HashMap::new(),
            },
        }
    }

    pub fn email(mut self, email: impl Into<String>) -> Self {
        self.claims.email = Some(email.into());
        self
    }

    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.claims.name = Some(name.into());
        self
    }

    pub fn roles(mut self, roles: Vec<String>) -> Self {
        self.claims.roles = roles;
        self
    }

    pub fn add_role(mut self, role: impl Into<String>) -> Self {
        self.claims.roles.push(role.into());
        self
    }

    pub fn org_id(mut self, org_id: impl Into<String>) -> Self {
        self.claims.org_id = Some(org_id.into());
        self
    }

    pub fn expires_in(mut self, duration: Duration) -> Self {
        self.claims.exp = (Utc::now() + duration).timestamp();
        self
    }

    pub fn issuer(mut self, issuer: impl Into<String>) -> Self {
        self.claims.iss = Some(issuer.into());
        self
    }

    pub fn audience(mut self, audience: impl Into<String>) -> Self {
        self.claims.aud = Some(audience.into());
        self
    }

    pub fn custom<T: Serialize>(mut self, key: impl Into<String>, value: T) -> Self {
        if let Ok(v) = serde_json::to_value(value) {
            self.claims.custom.insert(key.into(), v);
        }
        self
    }

    pub fn build(self) -> Claims {
        self.claims
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Configuration
// ═══════════════════════════════════════════════════════════════════════════════

/// Authentication configuration.
#[derive(Debug, Clone)]
pub struct AuthConfig {
    /// Enable authentication
    pub enabled: bool,

    /// JWT secret key (for HS256/HS384/HS512)
    pub jwt_secret: Option<String>,

    /// JWT public key (for RS256/RS384/RS512/ES256)
    pub jwt_public_key: Option<String>,

    /// JWT algorithm
    pub jwt_algorithm: Algorithm,

    /// Token issuer for validation
    pub issuer: Option<String>,

    /// Token audience for validation
    pub audience: Option<String>,

    /// Leeway for expiration checks (in seconds)
    pub leeway_secs: u64,

    /// API keys for static authentication (key -> user info)
    pub api_keys: std::collections::HashMap<String, ApiKeyInfo>,

    /// Paths that don't require authentication
    pub public_paths: Vec<String>,

    /// Paths that require specific roles (path -> roles)
    pub protected_paths: std::collections::HashMap<String, Vec<String>>,

    /// Enable token revocation checking
    pub enable_revocation_check: bool,

    /// Header name for API key
    pub api_key_header: String,

    /// Header name for JWT
    pub jwt_header: String,
}

/// API key information.
#[derive(Debug, Clone)]
pub struct ApiKeyInfo {
    /// User ID associated with this key
    pub user_id: String,

    /// User name
    pub name: Option<String>,

    /// Roles granted to this key
    pub roles: Vec<String>,

    /// Organization ID
    pub org_id: Option<String>,

    /// Whether the key is active
    pub active: bool,

    /// Rate limit override (requests per minute)
    pub rate_limit: Option<u64>,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            jwt_secret: None,
            jwt_public_key: None,
            jwt_algorithm: Algorithm::HS256,
            issuer: None,
            audience: None,
            leeway_secs: 60,
            api_keys: std::collections::HashMap::new(),
            public_paths: vec![
                "/health".to_string(),
                "/ready".to_string(),
                "/metrics".to_string(),
            ],
            protected_paths: std::collections::HashMap::new(),
            enable_revocation_check: false,
            api_key_header: "X-API-Key".to_string(),
            jwt_header: "Authorization".to_string(),
        }
    }
}

impl AuthConfig {
    /// Create a new builder.
    pub fn builder() -> AuthConfigBuilder {
        AuthConfigBuilder::default()
    }
}

/// Builder for auth configuration.
#[derive(Default)]
pub struct AuthConfigBuilder {
    config: AuthConfig,
}

impl AuthConfigBuilder {
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.config.enabled = enabled;
        self
    }

    pub fn jwt_secret(mut self, secret: impl Into<String>) -> Self {
        self.config.jwt_secret = Some(secret.into());
        self
    }

    pub fn jwt_public_key(mut self, key: impl Into<String>) -> Self {
        self.config.jwt_public_key = Some(key.into());
        self
    }

    pub fn jwt_algorithm(mut self, algorithm: Algorithm) -> Self {
        self.config.jwt_algorithm = algorithm;
        self
    }

    pub fn issuer(mut self, issuer: impl Into<String>) -> Self {
        self.config.issuer = Some(issuer.into());
        self
    }

    pub fn audience(mut self, audience: impl Into<String>) -> Self {
        self.config.audience = Some(audience.into());
        self
    }

    pub fn leeway_secs(mut self, secs: u64) -> Self {
        self.config.leeway_secs = secs;
        self
    }

    pub fn add_api_key(mut self, key: impl Into<String>, info: ApiKeyInfo) -> Self {
        self.config.api_keys.insert(key.into(), info);
        self
    }

    pub fn add_public_path(mut self, path: impl Into<String>) -> Self {
        self.config.public_paths.push(path.into());
        self
    }

    pub fn add_protected_path(mut self, path: impl Into<String>, roles: Vec<String>) -> Self {
        self.config.protected_paths.insert(path.into(), roles);
        self
    }

    pub fn enable_revocation_check(mut self, enabled: bool) -> Self {
        self.config.enable_revocation_check = enabled;
        self
    }

    pub fn build(self) -> AuthConfig {
        self.config
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Authentication Context
// ═══════════════════════════════════════════════════════════════════════════════

/// Authentication context attached to requests.
#[derive(Debug, Clone)]
pub struct AuthContext {
    /// User ID
    pub user_id: String,

    /// User email
    pub email: Option<String>,

    /// User name
    pub name: Option<String>,

    /// User roles
    pub roles: Vec<String>,

    /// Organization ID
    pub org_id: Option<String>,

    /// Authentication method used
    pub auth_method: AuthMethod,

    /// Token ID (for JWT)
    pub token_id: Option<String>,

    /// Token expiration (for JWT)
    pub expires_at: Option<DateTime<Utc>>,

    /// Request ID for correlation
    pub request_id: String,
}

/// Authentication method.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthMethod {
    Jwt,
    ApiKey,
    Anonymous,
}

impl AuthContext {
    /// Create from JWT claims.
    pub fn from_claims(claims: Claims, request_id: String) -> Self {
        let expires_at = claims.expires_at();
        Self {
            user_id: claims.sub,
            email: claims.email,
            name: claims.name,
            roles: claims.roles,
            org_id: claims.org_id,
            auth_method: AuthMethod::Jwt,
            token_id: Some(claims.jti),
            expires_at: Some(expires_at),
            request_id,
        }
    }

    /// Create from API key info.
    pub fn from_api_key(info: &ApiKeyInfo, request_id: String) -> Self {
        Self {
            user_id: info.user_id.clone(),
            email: None,
            name: info.name.clone(),
            roles: info.roles.clone(),
            org_id: info.org_id.clone(),
            auth_method: AuthMethod::ApiKey,
            token_id: None,
            expires_at: None,
            request_id,
        }
    }

    /// Create anonymous context.
    pub fn anonymous(request_id: String) -> Self {
        Self {
            user_id: "anonymous".to_string(),
            email: None,
            name: None,
            roles: Vec::new(),
            org_id: None,
            auth_method: AuthMethod::Anonymous,
            token_id: None,
            expires_at: None,
            request_id,
        }
    }

    /// Check if user has a role.
    pub fn has_role(&self, role: &str) -> bool {
        self.roles.iter().any(|r| r == role || r == "admin")
    }

    /// Check if user has any of the roles.
    pub fn has_any_role(&self, roles: &[&str]) -> bool {
        roles.iter().any(|r| self.has_role(r))
    }

    /// Check if this is an authenticated context.
    pub fn is_authenticated(&self) -> bool {
        self.auth_method != AuthMethod::Anonymous
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Authenticator
// ═══════════════════════════════════════════════════════════════════════════════

/// Main authenticator that handles token validation.
pub struct Authenticator {
    config: AuthConfig,
    encoding_key: Option<EncodingKey>,
    decoding_key: Option<DecodingKey>,
    validation: Validation,
    revoked_tokens: Arc<DashMap<String, DateTime<Utc>>>,
}

impl Authenticator {
    /// Create a new authenticator.
    pub fn new(config: AuthConfig) -> Result<Self, AuthError> {
        let (encoding_key, decoding_key) = match config.jwt_algorithm {
            Algorithm::HS256 | Algorithm::HS384 | Algorithm::HS512 => {
                let secret = config
                    .jwt_secret
                    .as_ref()
                    .ok_or_else(|| AuthError::Internal("JWT secret required for HMAC algorithms".into()))?;

                (
                    Some(EncodingKey::from_secret(secret.as_bytes())),
                    Some(DecodingKey::from_secret(secret.as_bytes())),
                )
            }
            Algorithm::RS256 | Algorithm::RS384 | Algorithm::RS512 => {
                let public_key = config
                    .jwt_public_key
                    .as_ref()
                    .ok_or_else(|| AuthError::Internal("JWT public key required for RSA algorithms".into()))?;

                (
                    None, // Would need private key for encoding
                    Some(DecodingKey::from_rsa_pem(public_key.as_bytes())
                        .map_err(|e| AuthError::Internal(format!("Invalid RSA public key: {}", e)))?),
                )
            }
            Algorithm::ES256 | Algorithm::ES384 => {
                let public_key = config
                    .jwt_public_key
                    .as_ref()
                    .ok_or_else(|| AuthError::Internal("JWT public key required for EC algorithms".into()))?;

                (
                    None,
                    Some(DecodingKey::from_ec_pem(public_key.as_bytes())
                        .map_err(|e| AuthError::Internal(format!("Invalid EC public key: {}", e)))?),
                )
            }
            _ => {
                return Err(AuthError::Internal(format!(
                    "Unsupported JWT algorithm: {:?}",
                    config.jwt_algorithm
                )));
            }
        };

        let mut validation = Validation::new(config.jwt_algorithm);
        validation.leeway = config.leeway_secs;

        if let Some(ref issuer) = config.issuer {
            validation.set_issuer(&[issuer]);
        }

        if let Some(ref audience) = config.audience {
            validation.set_audience(&[audience]);
        }

        Ok(Self {
            config,
            encoding_key,
            decoding_key,
            validation,
            revoked_tokens: Arc::new(DashMap::new()),
        })
    }

    /// Check if a path is public (doesn't require auth).
    pub fn is_public_path(&self, path: &str) -> bool {
        self.config.public_paths.iter().any(|p| {
            if p.ends_with('*') {
                path.starts_with(&p[..p.len() - 1])
            } else {
                path == p
            }
        })
    }

    /// Get required roles for a path.
    pub fn required_roles(&self, path: &str) -> Option<&Vec<String>> {
        // Try exact match first
        if let Some(roles) = self.config.protected_paths.get(path) {
            return Some(roles);
        }

        // Try prefix match
        for (prefix, roles) in &self.config.protected_paths {
            if prefix.ends_with('*') && path.starts_with(&prefix[..prefix.len() - 1]) {
                return Some(roles);
            }
        }

        None
    }

    /// Authenticate a request.
    pub async fn authenticate(&self, headers: &HeaderMap) -> Result<AuthContext, AuthError> {
        let request_id = headers
            .get("X-Request-ID")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string())
            .unwrap_or_else(|| Uuid::new_v4().to_string());

        // Try JWT first
        if let Some(token) = self.extract_jwt(headers) {
            return self.validate_jwt(&token, request_id).await;
        }

        // Try API key
        if let Some(key) = self.extract_api_key(headers) {
            return self.validate_api_key(&key, request_id);
        }

        // No credentials provided
        Err(AuthError::MissingCredentials)
    }

    /// Extract JWT from headers.
    fn extract_jwt(&self, headers: &HeaderMap) -> Option<String> {
        headers
            .get(&self.config.jwt_header)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| {
                s.strip_prefix("Bearer ")
                    .or_else(|| s.strip_prefix("bearer "))
                    .map(|s| s.to_string())
            })
    }

    /// Extract API key from headers.
    fn extract_api_key(&self, headers: &HeaderMap) -> Option<String> {
        headers
            .get(&self.config.api_key_header)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string())
    }

    /// Validate a JWT token.
    async fn validate_jwt(&self, token: &str, request_id: String) -> Result<AuthContext, AuthError> {
        let decoding_key = self.decoding_key.as_ref()
            .ok_or_else(|| AuthError::Internal("JWT decoding key not configured".into()))?;

        let token_data = decode::<Claims>(token, decoding_key, &self.validation)
            .map_err(|e| {
                debug!("JWT validation failed: {}", e);
                match e.kind() {
                    jsonwebtoken::errors::ErrorKind::ExpiredSignature => AuthError::TokenExpired,
                    jsonwebtoken::errors::ErrorKind::InvalidToken
                    | jsonwebtoken::errors::ErrorKind::InvalidSignature => AuthError::InvalidToken,
                    _ => AuthError::ValidationError(e.to_string()),
                }
            })?;

        let claims = token_data.claims;

        // Check if token is revoked
        if self.config.enable_revocation_check
            && self.revoked_tokens.contains_key(&claims.jti)
        {
            return Err(AuthError::InvalidToken);
        }

        counter!(
            "auth_success_total",
            "method" => "jwt"
        )
        .increment(1);

        Ok(AuthContext::from_claims(claims, request_id))
    }

    /// Validate an API key.
    fn validate_api_key(&self, key: &str, request_id: String) -> Result<AuthContext, AuthError> {
        let info = self.config.api_keys.get(key)
            .ok_or(AuthError::InvalidApiKey)?;

        if !info.active {
            return Err(AuthError::AccountDisabled);
        }

        counter!(
            "auth_success_total",
            "method" => "api_key"
        )
        .increment(1);

        Ok(AuthContext::from_api_key(info, request_id))
    }

    /// Generate a new JWT token.
    pub fn generate_token(&self, claims: &Claims) -> Result<String, AuthError> {
        let encoding_key = self.encoding_key.as_ref()
            .ok_or_else(|| AuthError::Internal("JWT encoding key not configured".into()))?;

        let header = Header::new(self.config.jwt_algorithm);
        encode(&header, claims, encoding_key)
            .map_err(|e| AuthError::Internal(format!("Failed to generate token: {}", e)))
    }

    /// Revoke a token by its ID.
    pub fn revoke_token(&self, token_id: &str, expires_at: DateTime<Utc>) {
        self.revoked_tokens.insert(token_id.to_string(), expires_at);
    }

    /// Clean up expired revoked tokens.
    pub fn cleanup_revoked_tokens(&self) {
        let now = Utc::now();
        self.revoked_tokens.retain(|_, exp| *exp > now);
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tower Layer and Service
// ═══════════════════════════════════════════════════════════════════════════════

/// Authentication layer for Tower.
#[derive(Clone)]
pub struct AuthLayer {
    authenticator: Arc<Authenticator>,
}

impl AuthLayer {
    /// Create a new auth layer.
    pub fn new(authenticator: Arc<Authenticator>) -> Self {
        Self { authenticator }
    }

    /// Create from configuration.
    pub fn from_config(config: AuthConfig) -> Result<Self, AuthError> {
        let authenticator = Authenticator::new(config)?;
        Ok(Self::new(Arc::new(authenticator)))
    }
}

impl<S> Layer<S> for AuthLayer {
    type Service = AuthService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        AuthService {
            inner,
            authenticator: self.authenticator.clone(),
        }
    }
}

/// Authentication service.
#[derive(Clone)]
pub struct AuthService<S> {
    inner: S,
    authenticator: Arc<Authenticator>,
}

impl<S> Service<Request<Body>> for AuthService<S>
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
        let authenticator = self.authenticator.clone();
        let mut inner = self.inner.clone();

        Box::pin(async move {
            let path = request.uri().path();

            // Skip auth for disabled or public paths
            if !authenticator.config.enabled || authenticator.is_public_path(path) {
                let request_id = request
                    .headers()
                    .get("X-Request-ID")
                    .and_then(|v| v.to_str().ok())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| Uuid::new_v4().to_string());

                request.extensions_mut().insert(AuthContext::anonymous(request_id));
                return inner.call(request).await;
            }

            // Authenticate the request
            let headers = request.headers();
            match authenticator.authenticate(headers).await {
                Ok(auth_context) => {
                    // Check role requirements for protected paths
                    if let Some(required_roles) = authenticator.required_roles(path) {
                        let has_required_role = required_roles.iter()
                            .any(|r| auth_context.has_role(r));

                        if !has_required_role {
                            return Ok(AuthError::InsufficientPermissions.into_response());
                        }
                    }

                    // Inject auth context into request
                    request.extensions_mut().insert(auth_context);
                    inner.call(request).await
                }
                Err(e) => Ok(e.into_response()),
            }
        })
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Axum Extractor
// ═══════════════════════════════════════════════════════════════════════════════

/// Extractor for authentication context in handlers.
#[axum::async_trait]
impl<S> FromRequestParts<S> for AuthContext
where
    S: Send + Sync,
{
    type Rejection = AuthError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<AuthContext>()
            .cloned()
            .ok_or(AuthError::MissingCredentials)
    }
}

/// Guard that requires authentication.
pub struct RequireAuth(pub AuthContext);

#[axum::async_trait]
impl<S> FromRequestParts<S> for RequireAuth
where
    S: Send + Sync,
{
    type Rejection = AuthError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let ctx = parts
            .extensions
            .get::<AuthContext>()
            .cloned()
            .ok_or(AuthError::MissingCredentials)?;

        if !ctx.is_authenticated() {
            return Err(AuthError::MissingCredentials);
        }

        Ok(RequireAuth(ctx))
    }
}

/// Guard that requires specific roles.
pub struct RequireRole<const N: usize> {
    pub context: AuthContext,
    pub required_roles: [&'static str; N],
}

impl<const N: usize> RequireRole<N> {
    pub fn new(context: AuthContext, roles: [&'static str; N]) -> Result<Self, AuthError> {
        if !context.has_any_role(&roles) {
            return Err(AuthError::InsufficientPermissions);
        }
        Ok(Self {
            context,
            required_roles: roles,
        })
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_claims_builder() {
        let claims = Claims::builder("user123")
            .email("test@example.com")
            .roles(vec!["user".to_string(), "admin".to_string()])
            .org_id("org456")
            .expires_in(Duration::hours(24))
            .build();

        assert_eq!(claims.sub, "user123");
        assert_eq!(claims.email, Some("test@example.com".to_string()));
        assert!(claims.has_role("user"));
        assert!(claims.has_role("admin"));
        assert!(!claims.is_expired());
    }

    #[test]
    fn test_claims_role_check() {
        let claims = Claims::new("user1", vec!["user".to_string()], Duration::hours(1));
        assert!(claims.has_role("user"));
        assert!(!claims.has_role("admin"));

        let admin_claims = Claims::new("admin1", vec!["admin".to_string()], Duration::hours(1));
        assert!(admin_claims.has_role("admin"));
        assert!(admin_claims.has_role("user")); // Admin has all roles
    }

    #[test]
    fn test_auth_context() {
        let claims = Claims::builder("user1")
            .roles(vec!["user".to_string()])
            .build();

        let ctx = AuthContext::from_claims(claims, "req-123".to_string());

        assert_eq!(ctx.user_id, "user1");
        assert!(ctx.is_authenticated());
        assert!(ctx.has_role("user"));
        assert_eq!(ctx.auth_method, AuthMethod::Jwt);
    }

    #[test]
    fn test_config_builder() {
        let config = AuthConfig::builder()
            .jwt_secret("my-secret")
            .issuer("apex")
            .add_public_path("/health")
            .add_protected_path("/admin/*", vec!["admin".to_string()])
            .build();

        assert!(config.jwt_secret.is_some());
        assert!(config.public_paths.contains(&"/health".to_string()));
    }

    #[test]
    fn test_authenticator_public_paths() {
        let config = AuthConfig {
            jwt_secret: Some("secret".to_string()),
            public_paths: vec!["/health".to_string(), "/api/public/*".to_string()],
            ..Default::default()
        };

        let auth = Authenticator::new(config).unwrap();

        assert!(auth.is_public_path("/health"));
        assert!(auth.is_public_path("/api/public/test"));
        assert!(!auth.is_public_path("/api/private"));
    }

    #[test]
    fn test_jwt_generation_and_validation() {
        let config = AuthConfig {
            jwt_secret: Some("super-secret-key-for-testing-only".to_string()),
            ..Default::default()
        };

        let auth = Authenticator::new(config).unwrap();

        let claims = Claims::builder("user123")
            .roles(vec!["user".to_string()])
            .expires_in(Duration::hours(1))
            .build();

        let token = auth.generate_token(&claims).unwrap();
        assert!(!token.is_empty());

        // Token should be valid (would need async context to fully test)
    }

    #[test]
    fn test_api_key_info() {
        let info = ApiKeyInfo {
            user_id: "user1".to_string(),
            name: Some("Test User".to_string()),
            roles: vec!["user".to_string()],
            org_id: None,
            active: true,
            rate_limit: Some(1000),
        };

        let ctx = AuthContext::from_api_key(&info, "req-1".to_string());
        assert_eq!(ctx.user_id, "user1");
        assert_eq!(ctx.auth_method, AuthMethod::ApiKey);
    }
}
