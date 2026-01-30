//! WebSocket authentication module.
//!
//! Provides JWT-based authentication for WebSocket connections with:
//! - Token generation and validation
//! - Permission-based access control
//! - Token refresh support
//! - Session management

use chrono::{DateTime, Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use thiserror::Error;
use uuid::Uuid;

/// Authentication errors.
#[derive(Debug, Error)]
pub enum AuthError {
    #[error("Token has expired")]
    Expired,
    #[error("Invalid token: {0}")]
    Invalid(String),
    #[error("Missing required claims")]
    MissingClaims,
}

/// JWT claims for WebSocket authentication.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    /// Subject (user ID)
    pub sub: String,
    /// Expiration time
    pub exp: DateTime<Utc>,
    /// Issued at
    pub iat: DateTime<Utc>,
    /// Not before
    pub nbf: DateTime<Utc>,
    /// JWT ID (unique token identifier)
    pub jti: String,
    /// Issuer
    pub iss: String,
    /// Audience
    pub aud: String,
    /// User permissions
    pub permissions: Vec<String>,
    /// Organization ID (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub org_id: Option<String>,
    /// Session ID for tracking
    pub session_id: String,
}

impl Claims {
    /// Check if the token has expired.
    pub fn is_expired(&self) -> bool {
        Utc::now() > self.exp
    }

    /// Check if the token is not yet valid.
    pub fn is_not_yet_valid(&self) -> bool {
        Utc::now() < self.nbf
    }

    /// Check if the user has a specific permission.
    pub fn has_permission(&self, permission: &str) -> bool {
        self.permissions.contains(&permission.to_string())
            || self.permissions.contains(&"admin".to_string())
    }

    /// Check if the user has any of the specified permissions.
    pub fn has_any_permission(&self, permissions: &[&str]) -> bool {
        permissions.iter().any(|p| self.has_permission(p))
    }

    /// Check if the user has all of the specified permissions.
    pub fn has_all_permissions(&self, permissions: &[&str]) -> bool {
        permissions.iter().all(|p| self.has_permission(p))
    }
}

/// Internal claims structure for JWT encoding/decoding.
#[derive(Debug, Serialize, Deserialize)]
struct JwtClaims {
    sub: String,
    exp: i64,
    iat: i64,
    nbf: i64,
    jti: String,
    iss: String,
    aud: String,
    permissions: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    org_id: Option<String>,
    session_id: String,
}

impl From<&Claims> for JwtClaims {
    fn from(claims: &Claims) -> Self {
        Self {
            sub: claims.sub.clone(),
            exp: claims.exp.timestamp(),
            iat: claims.iat.timestamp(),
            nbf: claims.nbf.timestamp(),
            jti: claims.jti.clone(),
            iss: claims.iss.clone(),
            aud: claims.aud.clone(),
            permissions: claims.permissions.clone(),
            org_id: claims.org_id.clone(),
            session_id: claims.session_id.clone(),
        }
    }
}

impl TryFrom<JwtClaims> for Claims {
    type Error = AuthError;

    fn try_from(jwt: JwtClaims) -> Result<Self, Self::Error> {
        Ok(Self {
            sub: jwt.sub,
            exp: DateTime::from_timestamp(jwt.exp, 0)
                .ok_or(AuthError::Invalid("Invalid expiration timestamp".to_string()))?,
            iat: DateTime::from_timestamp(jwt.iat, 0)
                .ok_or(AuthError::Invalid("Invalid issued at timestamp".to_string()))?,
            nbf: DateTime::from_timestamp(jwt.nbf, 0)
                .ok_or(AuthError::Invalid("Invalid not before timestamp".to_string()))?,
            jti: jwt.jti,
            iss: jwt.iss,
            aud: jwt.aud,
            permissions: jwt.permissions,
            org_id: jwt.org_id,
            session_id: jwt.session_id,
        })
    }
}

/// Token for WebSocket authentication.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthToken {
    /// The JWT token string
    pub token: String,
    /// Token expiration
    pub expires_at: DateTime<Utc>,
    /// Refresh token (for obtaining new access tokens)
    pub refresh_token: Option<String>,
    /// Token type (always "Bearer")
    pub token_type: String,
}

/// WebSocket authentication handler.
pub struct WebSocketAuth {
    /// Secret key for signing tokens
    encoding_key: EncodingKey,
    /// Secret key for verifying tokens
    decoding_key: DecodingKey,
    /// Token expiration duration in seconds
    token_expiration_secs: u64,
    /// Issuer name
    issuer: String,
    /// Audience name
    audience: String,
    /// Revoked token IDs
    revoked_tokens: std::sync::RwLock<HashSet<String>>,
}

impl WebSocketAuth {
    /// Create a new authentication handler.
    pub fn new(secret: String, token_expiration_secs: u64) -> Self {
        Self {
            encoding_key: EncodingKey::from_secret(secret.as_bytes()),
            decoding_key: DecodingKey::from_secret(secret.as_bytes()),
            token_expiration_secs,
            issuer: "apex".to_string(),
            audience: "apex-websocket".to_string(),
            revoked_tokens: std::sync::RwLock::new(HashSet::new()),
        }
    }

    /// Create with custom issuer and audience.
    pub fn with_issuer_audience(mut self, issuer: String, audience: String) -> Self {
        self.issuer = issuer;
        self.audience = audience;
        self
    }

    /// Generate a new authentication token.
    pub fn generate_token(
        &self,
        user_id: &str,
        permissions: Vec<String>,
        org_id: Option<String>,
    ) -> Result<AuthToken, AuthError> {
        let now = Utc::now();
        let exp = now + Duration::seconds(self.token_expiration_secs as i64);
        let session_id = Uuid::new_v4().to_string();

        let claims = Claims {
            sub: user_id.to_string(),
            exp,
            iat: now,
            nbf: now,
            jti: Uuid::new_v4().to_string(),
            iss: self.issuer.clone(),
            aud: self.audience.clone(),
            permissions,
            org_id,
            session_id,
        };

        let jwt_claims = JwtClaims::from(&claims);

        let token = encode(&Header::default(), &jwt_claims, &self.encoding_key)
            .map_err(|e| AuthError::Invalid(format!("Failed to encode token: {}", e)))?;

        // Generate refresh token
        let refresh_token = self.generate_refresh_token(user_id)?;

        Ok(AuthToken {
            token,
            expires_at: exp,
            refresh_token: Some(refresh_token),
            token_type: "Bearer".to_string(),
        })
    }

    /// Generate a refresh token.
    fn generate_refresh_token(&self, user_id: &str) -> Result<String, AuthError> {
        let now = Utc::now();
        // Refresh tokens last longer (7 days)
        let exp = now + Duration::days(7);

        let claims = JwtClaims {
            sub: user_id.to_string(),
            exp: exp.timestamp(),
            iat: now.timestamp(),
            nbf: now.timestamp(),
            jti: Uuid::new_v4().to_string(),
            iss: self.issuer.clone(),
            aud: format!("{}-refresh", self.audience),
            permissions: vec!["refresh".to_string()],
            org_id: None,
            session_id: Uuid::new_v4().to_string(),
        };

        encode(&Header::default(), &claims, &self.encoding_key)
            .map_err(|e| AuthError::Invalid(format!("Failed to encode refresh token: {}", e)))
    }

    /// Validate a token and extract claims.
    pub fn validate_token(&self, token: &str) -> Result<Claims, AuthError> {
        let mut validation = Validation::default();
        validation.set_issuer(&[&self.issuer]);
        validation.set_audience(&[&self.audience]);

        let token_data = decode::<JwtClaims>(token, &self.decoding_key, &validation)
            .map_err(|e| match e.kind() {
                jsonwebtoken::errors::ErrorKind::ExpiredSignature => AuthError::Expired,
                _ => AuthError::Invalid(e.to_string()),
            })?;

        let claims = Claims::try_from(token_data.claims)?;

        // Check if token is revoked
        if self.is_revoked(&claims.jti) {
            return Err(AuthError::Invalid("Token has been revoked".to_string()));
        }

        // Additional validation
        if claims.is_expired() {
            return Err(AuthError::Expired);
        }

        if claims.is_not_yet_valid() {
            return Err(AuthError::Invalid("Token is not yet valid".to_string()));
        }

        Ok(claims)
    }

    /// Refresh an access token using a refresh token.
    pub fn refresh_access_token(&self, refresh_token: &str) -> Result<AuthToken, AuthError> {
        let mut validation = Validation::default();
        validation.set_issuer(&[&self.issuer]);
        validation.set_audience(&[&format!("{}-refresh", self.audience)]);

        let token_data = decode::<JwtClaims>(refresh_token, &self.decoding_key, &validation)
            .map_err(|e| match e.kind() {
                jsonwebtoken::errors::ErrorKind::ExpiredSignature => AuthError::Expired,
                _ => AuthError::Invalid(e.to_string()),
            })?;

        // Generate new access token with same user
        self.generate_token(
            &token_data.claims.sub,
            vec![], // Permissions should be fetched from user store
            token_data.claims.org_id,
        )
    }

    /// Revoke a token by its JTI.
    pub fn revoke_token(&self, jti: &str) {
        let mut revoked = self.revoked_tokens.write().unwrap();
        revoked.insert(jti.to_string());
    }

    /// Check if a token is revoked.
    pub fn is_revoked(&self, jti: &str) -> bool {
        let revoked = self.revoked_tokens.read().unwrap();
        revoked.contains(jti)
    }

    /// Clear expired entries from the revocation list.
    /// Should be called periodically.
    pub fn cleanup_revocation_list(&self) {
        // In a real implementation, this would remove entries
        // older than the maximum token lifetime
        // For now, this is a placeholder
    }
}

/// Permission constants for WebSocket operations.
#[allow(dead_code)]
pub mod permissions {
    /// Can subscribe to any task updates
    pub const TASKS_READ: &str = "tasks:read";
    /// Can subscribe to any agent updates
    pub const AGENTS_READ: &str = "agents:read";
    /// Can subscribe to any DAG updates
    pub const DAGS_READ: &str = "dags:read";
    /// Can view system metrics
    pub const METRICS_READ: &str = "metrics:read";
    /// Can receive and respond to approval requests
    pub const APPROVALS_MANAGE: &str = "approvals:manage";
    /// Can receive error notifications
    pub const ERRORS_READ: &str = "errors:read";
    /// Full admin access
    pub const ADMIN: &str = "admin";
}

/// Check if claims have permission to subscribe to a target.
#[allow(dead_code)]
pub fn can_subscribe(claims: &Claims, room_type: &super::room::RoomType) -> bool {
    use super::room::RoomType;

    match room_type {
        RoomType::Task => claims.has_permission(permissions::TASKS_READ),
        RoomType::Agent => claims.has_permission(permissions::AGENTS_READ),
        RoomType::Dag => claims.has_permission(permissions::DAGS_READ),
        RoomType::Metrics => claims.has_permission(permissions::METRICS_READ),
        RoomType::Approval => claims.has_permission(permissions::APPROVALS_MANAGE),
        RoomType::Error => claims.has_permission(permissions::ERRORS_READ),
        RoomType::Custom => claims.has_permission(permissions::ADMIN),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_auth() -> WebSocketAuth {
        WebSocketAuth::new("test-secret-key-for-testing".to_string(), 3600)
    }

    #[test]
    fn test_token_generation_and_validation() {
        let auth = create_test_auth();

        let token = auth
            .generate_token(
                "user-123",
                vec!["tasks:read".to_string(), "agents:read".to_string()],
                Some("org-456".to_string()),
            )
            .unwrap();

        assert!(!token.token.is_empty());
        assert_eq!(token.token_type, "Bearer");
        assert!(token.refresh_token.is_some());

        // Validate the token
        let claims = auth.validate_token(&token.token).unwrap();
        assert_eq!(claims.sub, "user-123");
        assert_eq!(claims.org_id, Some("org-456".to_string()));
        assert!(claims.has_permission("tasks:read"));
        assert!(claims.has_permission("agents:read"));
        assert!(!claims.has_permission("admin"));
    }

    #[test]
    fn test_permission_checks() {
        let claims = Claims {
            sub: "user-123".to_string(),
            exp: Utc::now() + Duration::hours(1),
            iat: Utc::now(),
            nbf: Utc::now(),
            jti: Uuid::new_v4().to_string(),
            iss: "apex".to_string(),
            aud: "apex-websocket".to_string(),
            permissions: vec!["tasks:read".to_string(), "metrics:read".to_string()],
            org_id: None,
            session_id: Uuid::new_v4().to_string(),
        };

        assert!(claims.has_permission("tasks:read"));
        assert!(!claims.has_permission("admin"));
        assert!(claims.has_any_permission(&["tasks:read", "admin"]));
        assert!(!claims.has_all_permissions(&["tasks:read", "admin"]));
        assert!(claims.has_all_permissions(&["tasks:read", "metrics:read"]));
    }

    #[test]
    fn test_admin_has_all_permissions() {
        let claims = Claims {
            sub: "admin-user".to_string(),
            exp: Utc::now() + Duration::hours(1),
            iat: Utc::now(),
            nbf: Utc::now(),
            jti: Uuid::new_v4().to_string(),
            iss: "apex".to_string(),
            aud: "apex-websocket".to_string(),
            permissions: vec!["admin".to_string()],
            org_id: None,
            session_id: Uuid::new_v4().to_string(),
        };

        // Admin should have all permissions
        assert!(claims.has_permission("tasks:read"));
        assert!(claims.has_permission("agents:read"));
        assert!(claims.has_permission("any-permission"));
    }

    #[test]
    fn test_token_revocation() {
        let auth = create_test_auth();

        let token = auth
            .generate_token("user-123", vec![], None)
            .unwrap();

        let claims = auth.validate_token(&token.token).unwrap();

        // Revoke the token
        auth.revoke_token(&claims.jti);

        // Should fail validation now
        let result = auth.validate_token(&token.token);
        assert!(result.is_err());
    }

    #[test]
    fn test_expired_token() {
        let auth = WebSocketAuth::new("test-secret".to_string(), 0); // 0 second expiration

        let token = auth
            .generate_token("user-123", vec![], None)
            .unwrap();

        // Wait a tiny bit to ensure expiration
        std::thread::sleep(std::time::Duration::from_millis(100));

        let result = auth.validate_token(&token.token);
        assert!(matches!(result, Err(AuthError::Expired)));
    }

    #[test]
    fn test_invalid_token() {
        let auth = create_test_auth();

        let result = auth.validate_token("invalid.token.here");
        assert!(matches!(result, Err(AuthError::Invalid(_))));
    }

    #[test]
    fn test_refresh_token() {
        let auth = create_test_auth();

        let token = auth
            .generate_token("user-123", vec!["tasks:read".to_string()], None)
            .unwrap();

        let refresh_token = token.refresh_token.unwrap();

        // Use refresh token to get new access token
        let new_token = auth.refresh_access_token(&refresh_token).unwrap();

        assert!(!new_token.token.is_empty());
        assert_ne!(new_token.token, token.token);
    }
}
