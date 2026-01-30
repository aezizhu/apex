//! API versioning middleware for Apex Core.
//!
//! This module provides comprehensive API versioning support including:
//! - URL path versioning (/api/v1/, /api/v2/)
//! - Header-based versioning (Accept: application/vnd.apex.v1+json)
//! - Version negotiation with fallback
//! - Deprecation warnings and sunset headers
//! - Version-aware routing
//!
//! # Usage
//!
//! ```rust,ignore
//! use apex_core::api::versioning::{ApiVersion, VersioningLayer, VersionConfig};
//!
//! let config = VersionConfig::default();
//! let app = Router::new()
//!     .layer(VersioningLayer::new(config));
//! ```

use axum::{
    extract::{FromRequestParts, Request},
    http::{
        header::{HeaderMap, HeaderName, HeaderValue, ACCEPT},
        request::Parts,
        StatusCode,
    },
    response::{IntoResponse, Response},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fmt,
    future::Future,
    pin::Pin,
    str::FromStr,
    sync::Arc,
    task::{Context, Poll},
};
use tower::{Layer, Service};
use tracing::{debug, warn};

// ═══════════════════════════════════════════════════════════════════════════════
// API Version Types
// ═══════════════════════════════════════════════════════════════════════════════

/// Represents an API version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ApiVersion {
    /// Major version number
    pub major: u32,
    /// Minor version number (optional, defaults to 0)
    pub minor: u32,
}

impl ApiVersion {
    /// Create a new API version.
    pub const fn new(major: u32, minor: u32) -> Self {
        Self { major, minor }
    }

    /// Create a version with only major number.
    pub const fn major_only(major: u32) -> Self {
        Self { major, minor: 0 }
    }

    /// V1 constant.
    pub const V1: Self = Self::new(1, 0);

    /// V2 constant.
    pub const V2: Self = Self::new(2, 0);

    /// Check if this version is compatible with another.
    /// A version is compatible if it has the same major version.
    pub fn is_compatible_with(&self, other: &Self) -> bool {
        self.major == other.major
    }

    /// Get the version string for URL paths (e.g., "v1", "v2").
    pub fn path_segment(&self) -> String {
        format!("v{}", self.major)
    }

    /// Get the full version string (e.g., "1.0", "2.1").
    pub fn full_version(&self) -> String {
        format!("{}.{}", self.major, self.minor)
    }

    /// Get the media type for this version.
    pub fn media_type(&self) -> String {
        format!("application/vnd.apex.v{}+json", self.major)
    }
}

impl fmt::Display for ApiVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "v{}.{}", self.major, self.minor)
    }
}

impl FromStr for ApiVersion {
    type Err = VersionParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim().to_lowercase();

        // Try parsing "v1", "v2", etc.
        if let Some(rest) = s.strip_prefix('v') {
            return Self::parse_version_number(rest);
        }

        // Try parsing "1", "2", "1.0", "2.1", etc.
        Self::parse_version_number(&s)
    }
}

impl ApiVersion {
    fn parse_version_number(s: &str) -> Result<Self, VersionParseError> {
        if let Some((major, minor)) = s.split_once('.') {
            let major = major
                .parse()
                .map_err(|_| VersionParseError::InvalidFormat(s.to_string()))?;
            let minor = minor
                .parse()
                .map_err(|_| VersionParseError::InvalidFormat(s.to_string()))?;
            Ok(Self { major, minor })
        } else {
            let major = s
                .parse()
                .map_err(|_| VersionParseError::InvalidFormat(s.to_string()))?;
            Ok(Self::major_only(major))
        }
    }
}

/// Error parsing an API version.
#[derive(Debug, Clone)]
pub enum VersionParseError {
    InvalidFormat(String),
    UnsupportedVersion(ApiVersion),
}

impl fmt::Display for VersionParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFormat(s) => write!(f, "Invalid version format: {}", s),
            Self::UnsupportedVersion(v) => write!(f, "Unsupported API version: {}", v),
        }
    }
}

impl std::error::Error for VersionParseError {}

// ═══════════════════════════════════════════════════════════════════════════════
// Version Status
// ═══════════════════════════════════════════════════════════════════════════════

/// Status of an API version.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VersionStatus {
    /// Version is current and fully supported.
    Current,
    /// Version is supported but deprecated.
    Deprecated {
        /// When this version will be sunset.
        sunset_date: Option<DateTime<Utc>>,
        /// Message to include in deprecation warning.
        message: String,
        /// Recommended version to upgrade to.
        recommended_version: ApiVersion,
    },
    /// Version is no longer supported.
    Sunset {
        /// When this version was sunset.
        sunset_date: DateTime<Utc>,
        /// Final message.
        message: String,
    },
    /// Version is in preview/beta.
    Preview {
        /// Stability warning message.
        message: String,
    },
}

impl VersionStatus {
    /// Check if this version is usable (not sunset).
    pub fn is_usable(&self) -> bool {
        !matches!(self, Self::Sunset { .. })
    }

    /// Check if this version is deprecated.
    pub fn is_deprecated(&self) -> bool {
        matches!(self, Self::Deprecated { .. })
    }

    /// Check if this version is current.
    pub fn is_current(&self) -> bool {
        matches!(self, Self::Current)
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Version Configuration
// ═══════════════════════════════════════════════════════════════════════════════

/// Configuration for a specific API version.
#[derive(Debug, Clone)]
pub struct VersionInfo {
    /// The version.
    pub version: ApiVersion,
    /// Status of this version.
    pub status: VersionStatus,
    /// Release date.
    pub released: DateTime<Utc>,
    /// Changelog or release notes URL.
    pub changelog_url: Option<String>,
}

impl VersionInfo {
    /// Create info for a current version.
    pub fn current(version: ApiVersion) -> Self {
        Self {
            version,
            status: VersionStatus::Current,
            released: Utc::now(),
            changelog_url: None,
        }
    }

    /// Create info for a deprecated version.
    pub fn deprecated(
        version: ApiVersion,
        sunset_date: Option<DateTime<Utc>>,
        recommended: ApiVersion,
    ) -> Self {
        Self {
            version,
            status: VersionStatus::Deprecated {
                sunset_date,
                message: format!(
                    "API version {} is deprecated. Please migrate to {}",
                    version, recommended
                ),
                recommended_version: recommended,
            },
            released: Utc::now(),
            changelog_url: None,
        }
    }

    /// Create info for a preview version.
    pub fn preview(version: ApiVersion) -> Self {
        Self {
            version,
            status: VersionStatus::Preview {
                message: format!(
                    "API version {} is in preview and may change without notice",
                    version
                ),
            },
            released: Utc::now(),
            changelog_url: None,
        }
    }

    /// Set the changelog URL.
    pub fn with_changelog(mut self, url: impl Into<String>) -> Self {
        self.changelog_url = Some(url.into());
        self
    }
}

/// Configuration for API versioning.
#[derive(Debug, Clone)]
pub struct VersionConfig {
    /// Available versions and their status.
    pub versions: HashMap<ApiVersion, VersionInfo>,
    /// Default version when none is specified.
    pub default_version: ApiVersion,
    /// Latest stable version.
    pub latest_version: ApiVersion,
    /// Header name for version override.
    pub version_header: HeaderName,
    /// Whether to allow version in Accept header.
    pub allow_accept_header: bool,
    /// Whether to include deprecation headers.
    pub include_deprecation_headers: bool,
    /// Base path for versioned APIs (e.g., "/api").
    pub base_path: String,
}

impl Default for VersionConfig {
    fn default() -> Self {
        let mut versions = HashMap::new();

        // V1 is current
        versions.insert(ApiVersion::V1, VersionInfo::current(ApiVersion::V1));

        // V2 is preview
        versions.insert(ApiVersion::V2, VersionInfo::preview(ApiVersion::V2));

        Self {
            versions,
            default_version: ApiVersion::V1,
            latest_version: ApiVersion::V1,
            version_header: HeaderName::from_static("x-api-version"),
            allow_accept_header: true,
            include_deprecation_headers: true,
            base_path: "/api".to_string(),
        }
    }
}

impl VersionConfig {
    /// Create a new version config.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a version.
    pub fn with_version(mut self, info: VersionInfo) -> Self {
        self.versions.insert(info.version, info);
        self
    }

    /// Set the default version.
    pub fn with_default(mut self, version: ApiVersion) -> Self {
        self.default_version = version;
        self
    }

    /// Set the latest version.
    pub fn with_latest(mut self, version: ApiVersion) -> Self {
        self.latest_version = version;
        self
    }

    /// Deprecate a version.
    pub fn deprecate_version(
        mut self,
        version: ApiVersion,
        sunset_date: Option<DateTime<Utc>>,
        recommended: ApiVersion,
    ) -> Self {
        if let Some(info) = self.versions.get_mut(&version) {
            info.status = VersionStatus::Deprecated {
                sunset_date,
                message: format!(
                    "API version {} is deprecated. Please migrate to {}",
                    version, recommended
                ),
                recommended_version: recommended,
            };
        }
        self
    }

    /// Check if a version is supported.
    pub fn is_supported(&self, version: &ApiVersion) -> bool {
        self.versions
            .get(version)
            .map(|info| info.status.is_usable())
            .unwrap_or(false)
    }

    /// Get version info.
    pub fn get_version_info(&self, version: &ApiVersion) -> Option<&VersionInfo> {
        self.versions.get(version)
    }

    /// Get all supported versions.
    pub fn supported_versions(&self) -> Vec<&ApiVersion> {
        self.versions
            .iter()
            .filter(|(_, info)| info.status.is_usable())
            .map(|(v, _)| v)
            .collect()
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Version Extraction
// ═══════════════════════════════════════════════════════════════════════════════

/// Extracted API version from a request.
#[derive(Debug, Clone)]
pub struct ExtractedVersion {
    /// The resolved version.
    pub version: ApiVersion,
    /// How the version was determined.
    pub source: VersionSource,
    /// Version info from config.
    pub info: Option<VersionInfo>,
}

/// How the API version was determined.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VersionSource {
    /// From URL path (/api/v1/...).
    UrlPath,
    /// From Accept header (application/vnd.apex.v1+json).
    AcceptHeader,
    /// From custom version header (X-API-Version).
    VersionHeader,
    /// From query parameter (?version=1).
    QueryParameter,
    /// Default version (no version specified).
    Default,
}

impl fmt::Display for VersionSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UrlPath => write!(f, "url_path"),
            Self::AcceptHeader => write!(f, "accept_header"),
            Self::VersionHeader => write!(f, "version_header"),
            Self::QueryParameter => write!(f, "query_parameter"),
            Self::Default => write!(f, "default"),
        }
    }
}

/// Extract version from request.
pub fn extract_version(
    uri_path: &str,
    headers: &HeaderMap,
    config: &VersionConfig,
) -> Result<ExtractedVersion, VersionError> {
    // Priority 1: URL path (/api/v1/...)
    if let Some(version) = extract_from_path(uri_path, &config.base_path) {
        return validate_version(version, VersionSource::UrlPath, config);
    }

    // Priority 2: Custom version header
    if let Some(version) = extract_from_header(headers, &config.version_header) {
        return validate_version(version, VersionSource::VersionHeader, config);
    }

    // Priority 3: Accept header
    if config.allow_accept_header {
        if let Some(version) = extract_from_accept_header(headers) {
            return validate_version(version, VersionSource::AcceptHeader, config);
        }
    }

    // Default version
    Ok(ExtractedVersion {
        version: config.default_version,
        source: VersionSource::Default,
        info: config.get_version_info(&config.default_version).cloned(),
    })
}

fn extract_from_path(path: &str, base_path: &str) -> Option<ApiVersion> {
    // Remove base path and get the version segment
    let path = path.strip_prefix(base_path)?;
    let path = path.trim_start_matches('/');

    // Get first segment
    let segment = path.split('/').next()?;

    // Parse version from segment (e.g., "v1", "v2")
    if segment.starts_with('v') {
        segment.parse().ok()
    } else {
        None
    }
}

fn extract_from_header(headers: &HeaderMap, header_name: &HeaderName) -> Option<ApiVersion> {
    headers
        .get(header_name)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse().ok())
}

fn extract_from_accept_header(headers: &HeaderMap) -> Option<ApiVersion> {
    let accept = headers.get(ACCEPT)?.to_str().ok()?;

    // Parse Accept header for vendor media type
    // Format: application/vnd.apex.v1+json
    for media_type in accept.split(',') {
        let media_type = media_type.trim();

        if media_type.starts_with("application/vnd.apex.") {
            // Extract version from media type
            let rest = media_type.strip_prefix("application/vnd.apex.")?;
            let version_part = rest.split('+').next()?;
            if let Some(version) = version_part.strip_prefix('v') {
                return version.parse().ok();
            }
        }
    }

    None
}

fn validate_version(
    version: ApiVersion,
    source: VersionSource,
    config: &VersionConfig,
) -> Result<ExtractedVersion, VersionError> {
    let info = config.get_version_info(&version);

    match info {
        Some(info) if info.status.is_usable() => Ok(ExtractedVersion {
            version,
            source,
            info: Some(info.clone()),
        }),
        Some(info) => Err(VersionError::Sunset {
            version,
            info: info.clone(),
        }),
        None => Err(VersionError::Unsupported {
            version,
            supported: config.supported_versions().iter().map(|v| **v).collect(),
        }),
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Version Errors
// ═══════════════════════════════════════════════════════════════════════════════

/// Errors related to API versioning.
#[derive(Debug, Clone)]
pub enum VersionError {
    /// Version is not supported.
    Unsupported {
        version: ApiVersion,
        supported: Vec<ApiVersion>,
    },
    /// Version has been sunset.
    Sunset { version: ApiVersion, info: VersionInfo },
    /// Invalid version format.
    InvalidFormat(String),
}

impl fmt::Display for VersionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported { version, supported } => {
                write!(
                    f,
                    "API version {} is not supported. Supported versions: {:?}",
                    version, supported
                )
            }
            Self::Sunset { version, info } => {
                if let VersionStatus::Sunset { sunset_date, message } = &info.status {
                    write!(
                        f,
                        "API version {} was sunset on {}. {}",
                        version, sunset_date, message
                    )
                } else {
                    write!(f, "API version {} has been sunset", version)
                }
            }
            Self::InvalidFormat(s) => write!(f, "Invalid API version format: {}", s),
        }
    }
}

impl std::error::Error for VersionError {}

impl IntoResponse for VersionError {
    fn into_response(self) -> Response {
        let (status, body) = match &self {
            VersionError::Unsupported { version, supported } => (
                StatusCode::BAD_REQUEST,
                serde_json::json!({
                    "success": false,
                    "error": {
                        "code": "UNSUPPORTED_API_VERSION",
                        "message": format!("API version {} is not supported", version),
                        "supported_versions": supported.iter().map(|v| v.to_string()).collect::<Vec<_>>(),
                    }
                }),
            ),
            VersionError::Sunset { version, info } => {
                let message = if let VersionStatus::Sunset { message, .. } = &info.status {
                    message.clone()
                } else {
                    format!("API version {} has been sunset", version)
                };
                (
                    StatusCode::GONE,
                    serde_json::json!({
                        "success": false,
                        "error": {
                            "code": "API_VERSION_SUNSET",
                            "message": message,
                            "version": version.to_string(),
                        }
                    }),
                )
            }
            VersionError::InvalidFormat(s) => (
                StatusCode::BAD_REQUEST,
                serde_json::json!({
                    "success": false,
                    "error": {
                        "code": "INVALID_API_VERSION",
                        "message": format!("Invalid API version format: {}", s),
                    }
                }),
            ),
        };

        (status, axum::Json(body)).into_response()
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Deprecation Headers
// ═══════════════════════════════════════════════════════════════════════════════

/// Standard deprecation headers.
pub mod headers {
    use super::*;

    /// Deprecation header (RFC 8594).
    pub const DEPRECATION: &str = "Deprecation";

    /// Sunset header (RFC 8594).
    pub const SUNSET: &str = "Sunset";

    /// Link header for documentation.
    pub const LINK: &str = "Link";

    /// Custom header for API version.
    pub const X_API_VERSION: &str = "X-API-Version";

    /// Custom header for deprecation warning.
    pub const X_API_WARN: &str = "X-API-Warn";

    /// Add deprecation headers to a response.
    pub fn add_deprecation_headers(
        headers: &mut HeaderMap,
        info: &VersionInfo,
    ) {
        // Add version header
        if let Ok(version_value) = HeaderValue::from_str(&info.version.full_version()) {
            headers.insert(
                HeaderName::from_static("x-api-version"),
                version_value,
            );
        }

        if let VersionStatus::Deprecated {
            sunset_date,
            message,
            recommended_version,
        } = &info.status
        {
            // Deprecation header
            let value = HeaderValue::from_static("true");
            headers.insert(HeaderName::from_static("deprecation"), value);

            // Sunset header
            if let Some(sunset) = sunset_date {
                if let Ok(value) = HeaderValue::from_str(&sunset.to_rfc2822()) {
                    headers.insert(HeaderName::from_static("sunset"), value);
                }
            }

            // Warning header
            let warning = format!(
                "299 - \"{}. Recommended version: {}\"",
                message, recommended_version
            );
            if let Ok(value) = HeaderValue::from_str(&warning) {
                headers.insert(HeaderName::from_static("x-api-warn"), value);
            }
        }

        // Add Link header for changelog
        if let Some(changelog) = &info.changelog_url {
            let link = format!("<{}>; rel=\"deprecation\"", changelog);
            if let Ok(value) = HeaderValue::from_str(&link) {
                headers.insert(HeaderName::from_static("link"), value);
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Versioning Middleware
// ═══════════════════════════════════════════════════════════════════════════════

/// Layer for API versioning middleware.
#[derive(Clone)]
pub struct VersioningLayer {
    config: Arc<VersionConfig>,
}

impl VersioningLayer {
    /// Create a new versioning layer.
    pub fn new(config: VersionConfig) -> Self {
        Self {
            config: Arc::new(config),
        }
    }
}

impl<S> Layer<S> for VersioningLayer {
    type Service = VersioningMiddleware<S>;

    fn layer(&self, inner: S) -> Self::Service {
        VersioningMiddleware {
            inner,
            config: Arc::clone(&self.config),
        }
    }
}

/// Middleware that handles API versioning.
#[derive(Clone)]
pub struct VersioningMiddleware<S> {
    inner: S,
    config: Arc<VersionConfig>,
}

impl<S> Service<Request> for VersioningMiddleware<S>
where
    S: Service<Request, Response = Response> + Clone + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request) -> Self::Future {
        let config = Arc::clone(&self.config);
        let mut inner = self.inner.clone();

        Box::pin(async move {
            let (parts, body) = req.into_parts();

            // Extract version
            let extracted = extract_version(parts.uri.path(), &parts.headers, &config);

            match extracted {
                Ok(extracted) => {
                    debug!(
                        version = %extracted.version,
                        source = %extracted.source,
                        "API version extracted"
                    );

                    // Log deprecation warning if applicable
                    if let Some(info) = &extracted.info {
                        if info.status.is_deprecated() {
                            warn!(
                                version = %extracted.version,
                                "Deprecated API version used"
                            );
                        }
                    }

                    // Reconstruct request with version in extensions
                    let mut req = Request::from_parts(parts, body);
                    req.extensions_mut().insert(extracted.clone());

                    // Call inner service
                    let mut response = inner.call(req).await?;

                    // Add deprecation headers if needed
                    if config.include_deprecation_headers {
                        if let Some(info) = &extracted.info {
                            headers::add_deprecation_headers(response.headers_mut(), info);
                        }
                    }

                    Ok(response)
                }
                Err(error) => {
                    warn!(error = %error, "API version error");
                    Ok(error.into_response())
                }
            }
        })
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Axum Extractor
// ═══════════════════════════════════════════════════════════════════════════════

/// Extractor for getting the API version in handlers.
#[derive(Debug, Clone)]
pub struct Version(pub ExtractedVersion);

impl Version {
    /// Get the API version.
    pub fn version(&self) -> ApiVersion {
        self.0.version
    }

    /// Check if this is v1.
    pub fn is_v1(&self) -> bool {
        self.0.version.major == 1
    }

    /// Check if this is v2.
    pub fn is_v2(&self) -> bool {
        self.0.version.major == 2
    }

    /// Check if version is deprecated.
    pub fn is_deprecated(&self) -> bool {
        self.0
            .info
            .as_ref()
            .map(|info| info.status.is_deprecated())
            .unwrap_or(false)
    }
}

impl<S> FromRequestParts<S> for Version
where
    S: Send + Sync,
{
    type Rejection = VersionError;

    fn from_request_parts<'life0, 'life1, 'async_trait>(
        parts: &'life0 mut Parts,
        _state: &'life1 S,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = std::result::Result<Self, Self::Rejection>> + Send + 'async_trait>>
    where
        'life0: 'async_trait,
        'life1: 'async_trait,
        Self: 'async_trait,
    {
        Box::pin(async move {
            parts
                .extensions
                .get::<ExtractedVersion>()
                .cloned()
                .map(Version)
                .ok_or(VersionError::InvalidFormat(
                    "Version not found in request extensions".to_string(),
                ))
        })
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Version Router Helper
// ═══════════════════════════════════════════════════════════════════════════════

/// Helper for creating versioned routes.
pub struct VersionedRouter;

impl VersionedRouter {
    /// Create a path with version prefix.
    pub fn path(version: ApiVersion, path: &str) -> String {
        let path = path.trim_start_matches('/');
        format!("/api/{}/{}", version.path_segment(), path)
    }

    /// Create v1 path.
    pub fn v1(path: &str) -> String {
        Self::path(ApiVersion::V1, path)
    }

    /// Create v2 path.
    pub fn v2(path: &str) -> String {
        Self::path(ApiVersion::V2, path)
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_parsing() {
        assert_eq!("v1".parse::<ApiVersion>().unwrap(), ApiVersion::V1);
        assert_eq!("v2".parse::<ApiVersion>().unwrap(), ApiVersion::V2);
        assert_eq!("1".parse::<ApiVersion>().unwrap(), ApiVersion::major_only(1));
        assert_eq!(
            "1.5".parse::<ApiVersion>().unwrap(),
            ApiVersion::new(1, 5)
        );
        assert_eq!(
            "V2".parse::<ApiVersion>().unwrap(),
            ApiVersion::V2
        );
    }

    #[test]
    fn test_version_display() {
        assert_eq!(ApiVersion::V1.to_string(), "v1.0");
        assert_eq!(ApiVersion::new(2, 1).to_string(), "v2.1");
        assert_eq!(ApiVersion::V1.path_segment(), "v1");
        assert_eq!(ApiVersion::V2.media_type(), "application/vnd.apex.v2+json");
    }

    #[test]
    fn test_version_compatibility() {
        let v1_0 = ApiVersion::new(1, 0);
        let v1_5 = ApiVersion::new(1, 5);
        let v2_0 = ApiVersion::new(2, 0);

        assert!(v1_0.is_compatible_with(&v1_5));
        assert!(v1_5.is_compatible_with(&v1_0));
        assert!(!v1_0.is_compatible_with(&v2_0));
    }

    #[test]
    fn test_extract_from_path() {
        assert_eq!(
            extract_from_path("/api/v1/tasks", "/api"),
            Some(ApiVersion::V1)
        );
        assert_eq!(
            extract_from_path("/api/v2/agents", "/api"),
            Some(ApiVersion::V2)
        );
        assert_eq!(extract_from_path("/api/tasks", "/api"), None);
        assert_eq!(extract_from_path("/other/v1/tasks", "/api"), None);
    }

    #[test]
    fn test_config_default() {
        let config = VersionConfig::default();
        assert_eq!(config.default_version, ApiVersion::V1);
        assert!(config.is_supported(&ApiVersion::V1));
        assert!(config.is_supported(&ApiVersion::V2));
    }

    #[test]
    fn test_versioned_router() {
        assert_eq!(VersionedRouter::v1("tasks"), "/api/v1/tasks");
        assert_eq!(VersionedRouter::v2("agents"), "/api/v2/agents");
        assert_eq!(VersionedRouter::v1("/tasks"), "/api/v1/tasks");
    }

    #[test]
    fn test_version_ordering() {
        assert!(ApiVersion::V1 < ApiVersion::V2);
        assert!(ApiVersion::new(1, 5) < ApiVersion::new(2, 0));
        assert!(ApiVersion::new(2, 0) < ApiVersion::new(2, 1));
    }
}
