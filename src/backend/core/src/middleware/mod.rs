//! Production-grade middleware for Apex Core.
pub mod rate_limit;
pub mod auth;
pub mod tracing;
pub mod compression;
pub mod security_headers;
pub mod request_size;
pub mod audit;
pub mod csrf;
pub mod api_key_rotation;
pub mod input_sanitizer;

pub use rate_limit::{RateLimitLayer, RateLimitConfig, RateLimitError};
pub use auth::{AuthLayer, AuthConfig, Claims, AuthError, AuthContext, AuthMethod};
pub use tracing::{TracingLayer, TracingConfig, RequestContext};
pub use compression::{CompressionLayer, CompressionConfig, CompressionAlgorithm, CompressionLevel};
pub use security_headers::{SecurityHeadersLayer, SecurityHeadersConfig, FrameOptions, ReferrerPolicy};
pub use request_size::{RequestSizeLayer, RequestSizeConfig};
pub use audit::{AuditLayer, AuditConfig, AuditEntry, AuditLevel, AuditLogger};
pub use csrf::{CsrfLayer, CsrfConfig};
pub use api_key_rotation::{ApiKeyManager, ApiKeyConfig, ApiKeyEntry, GeneratedKey, KeyStatus};
pub use input_sanitizer::{InputSanitizerLayer, SanitizeConfig, InjectionType};

#[derive(Debug, Clone, Default)]
pub struct MiddlewareConfig {
    pub rate_limit: RateLimitConfig, pub auth: AuthConfig, pub tracing: TracingConfig,
    pub compression: CompressionConfig, pub security_headers: SecurityHeadersConfig,
    pub request_size: RequestSizeConfig, pub input_sanitizer: SanitizeConfig,
}
