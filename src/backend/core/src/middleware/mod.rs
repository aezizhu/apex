//! Production-grade middleware for Apex Core.
//!
//! This module provides comprehensive middleware for HTTP request processing:
//!
//! - **Rate Limiting**: Token bucket, sliding window, Redis-backed distributed limiting
//! - **Authentication**: JWT-based auth with API key support
//! - **Tracing**: Distributed request tracing with OpenTelemetry integration
//! - **Compression**: Adaptive response compression
//!
//! # Usage
//!
//! ```rust,ignore
//! use apex_core::middleware::{
//!     rate_limit::{RateLimitLayer, RateLimitConfig},
//!     auth::{AuthLayer, AuthConfig},
//!     tracing::TracingLayer,
//!     compression::CompressionLayer,
//! };
//!
//! let app = Router::new()
//!     .route("/api/v1/tasks", post(create_task))
//!     .layer(RateLimitLayer::new(rate_limit_config))
//!     .layer(AuthLayer::new(auth_config))
//!     .layer(TracingLayer::new())
//!     .layer(CompressionLayer::new());
//! ```

pub mod rate_limit;
pub mod auth;
pub mod tracing;
pub mod compression;

pub use rate_limit::{RateLimitLayer, RateLimitConfig, RateLimitError};
pub use auth::{AuthLayer, AuthConfig, Claims, AuthError, AuthContext, AuthMethod};
pub use tracing::{TracingLayer, TracingConfig, RequestContext};
pub use compression::{CompressionLayer, CompressionConfig, CompressionAlgorithm, CompressionLevel};

/// Common middleware configuration.
#[derive(Debug, Clone, Default)]
pub struct MiddlewareConfig {
    /// Rate limiting configuration
    pub rate_limit: RateLimitConfig,

    /// Authentication configuration
    pub auth: AuthConfig,

    /// Tracing configuration
    pub tracing: TracingConfig,

    /// Compression configuration
    pub compression: CompressionConfig,
}

