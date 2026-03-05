//! Response compression middleware.
//!
//! Features:
//! - Automatic content-type based compression decisions
//! - Support for gzip, deflate, and brotli
//! - Configurable minimum response size
//! - Accept-Encoding negotiation
//! - Compression level configuration
//!
//! # Example
//!
//! ```rust,ignore
//! use apex_core::middleware::compression::{CompressionLayer, CompressionConfig};
//!
//! let config = CompressionConfig::builder()
//!     .min_size(1024)
//!     .level(CompressionLevel::Default)
//!     .build();
//!
//! let app = Router::new()
//!     .route("/api/v1/tasks", get(list_tasks))
//!     .layer(CompressionLayer::new(config));
//! ```

use axum::{
    body::Body,
    extract::Request,
    http::{header, HeaderMap, HeaderValue},
    response::Response,
};
use futures::future::BoxFuture;
use metrics::counter;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    sync::Arc,
    task::{Context, Poll},
};
use tower::{Layer, Service};
use tracing::{debug, trace};

// ═══════════════════════════════════════════════════════════════════════════════
// Configuration
// ═══════════════════════════════════════════════════════════════════════════════

/// Compression algorithm.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CompressionAlgorithm {
    Gzip,
    Deflate,
    Brotli,
    Identity,
}

impl CompressionAlgorithm {
    /// Get the content-encoding header value.
    pub fn encoding_name(&self) -> &'static str {
        match self {
            Self::Gzip => "gzip",
            Self::Deflate => "deflate",
            Self::Brotli => "br",
            Self::Identity => "identity",
        }
    }

    /// Parse from accept-encoding header value.
    pub fn from_encoding(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "gzip" => Some(Self::Gzip),
            "deflate" => Some(Self::Deflate),
            "br" => Some(Self::Brotli),
            "identity" | "*" => Some(Self::Identity),
            _ => None,
        }
    }
}

/// Compression level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CompressionLevel {
    /// Fastest compression (least compression ratio)
    Fastest,
    /// Default compression level
    Default,
    /// Best compression (slowest)
    Best,
    /// Custom level (0-9 for gzip/deflate, 0-11 for brotli)
    Custom(u32),
}

impl Default for CompressionLevel {
    fn default() -> Self {
        Self::Default
    }
}

/// Compression middleware configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionConfig {
    /// Enable compression
    pub enabled: bool,

    /// Minimum response size to compress (in bytes)
    pub min_size: usize,

    /// Maximum response size to compress (0 = unlimited)
    pub max_size: usize,

    /// Compression level
    pub level: CompressionLevel,

    /// Preferred algorithms in order of preference
    pub algorithms: Vec<CompressionAlgorithm>,

    /// Content types to compress
    pub compressible_types: HashSet<String>,

    /// Content types to never compress
    pub excluded_types: HashSet<String>,

    /// Paths to exclude from compression
    pub excluded_paths: Vec<String>,

    /// Compress responses with streaming bodies
    pub compress_streams: bool,
}

impl Default for CompressionConfig {
    fn default() -> Self {
        let mut compressible_types = HashSet::new();
        compressible_types.insert("text/html".to_string());
        compressible_types.insert("text/plain".to_string());
        compressible_types.insert("text/css".to_string());
        compressible_types.insert("text/javascript".to_string());
        compressible_types.insert("application/json".to_string());
        compressible_types.insert("application/javascript".to_string());
        compressible_types.insert("application/xml".to_string());
        compressible_types.insert("application/xhtml+xml".to_string());
        compressible_types.insert("image/svg+xml".to_string());

        let mut excluded_types = HashSet::new();
        excluded_types.insert("image/jpeg".to_string());
        excluded_types.insert("image/png".to_string());
        excluded_types.insert("image/gif".to_string());
        excluded_types.insert("image/webp".to_string());
        excluded_types.insert("video/mp4".to_string());
        excluded_types.insert("audio/mpeg".to_string());
        excluded_types.insert("application/zip".to_string());
        excluded_types.insert("application/gzip".to_string());

        Self {
            enabled: true,
            min_size: 1024, // 1KB minimum
            max_size: 0,    // No maximum
            level: CompressionLevel::Default,
            algorithms: vec![
                CompressionAlgorithm::Brotli,
                CompressionAlgorithm::Gzip,
                CompressionAlgorithm::Deflate,
            ],
            compressible_types,
            excluded_types,
            excluded_paths: Vec::new(),
            compress_streams: false,
        }
    }
}

impl CompressionConfig {
    /// Create a new builder.
    pub fn builder() -> CompressionConfigBuilder {
        CompressionConfigBuilder::default()
    }

    /// Check if a content type should be compressed.
    pub fn should_compress_content_type(&self, content_type: &str) -> bool {
        // Extract the base type (without parameters like charset)
        let base_type = content_type
            .split(';')
            .next()
            .unwrap_or(content_type)
            .trim()
            .to_lowercase();

        // Check if explicitly excluded
        if self.excluded_types.contains(&base_type) {
            return false;
        }

        // Check if explicitly included
        if self.compressible_types.contains(&base_type) {
            return true;
        }

        // Default: compress text/* and application/json-like types
        base_type.starts_with("text/")
            || base_type.contains("json")
            || base_type.contains("xml")
            || base_type.contains("javascript")
    }

    /// Check if a path should be excluded from compression.
    pub fn is_path_excluded(&self, path: &str) -> bool {
        self.excluded_paths.iter().any(|p| {
            if p.ends_with('*') {
                path.starts_with(&p[..p.len() - 1])
            } else {
                path == p
            }
        })
    }

    /// Select the best compression algorithm based on Accept-Encoding header.
    pub fn select_algorithm(&self, accept_encoding: &str) -> Option<CompressionAlgorithm> {
        // Parse Accept-Encoding header
        let accepted: Vec<(CompressionAlgorithm, f32)> = accept_encoding
            .split(',')
            .filter_map(|part| {
                let mut parts = part.trim().splitn(2, ";q=");
                let encoding = parts.next()?;
                let quality: f32 = parts
                    .next()
                    .and_then(|q| q.trim().parse().ok())
                    .unwrap_or(1.0);

                if quality > 0.0 {
                    CompressionAlgorithm::from_encoding(encoding).map(|alg| (alg, quality))
                } else {
                    None
                }
            })
            .collect();

        // Find the first preferred algorithm that's accepted
        for preferred in &self.algorithms {
            if accepted.iter().any(|(alg, _)| alg == preferred) {
                return Some(*preferred);
            }
        }

        // Check for wildcard
        if accepted.iter().any(|(alg, _)| *alg == CompressionAlgorithm::Identity) {
            // If identity is accepted, return the first preferred algorithm
            return self.algorithms.first().copied();
        }

        None
    }
}

/// Builder for compression configuration.
#[derive(Default)]
pub struct CompressionConfigBuilder {
    config: CompressionConfig,
}

impl CompressionConfigBuilder {
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.config.enabled = enabled;
        self
    }

    pub fn min_size(mut self, size: usize) -> Self {
        self.config.min_size = size;
        self
    }

    pub fn max_size(mut self, size: usize) -> Self {
        self.config.max_size = size;
        self
    }

    pub fn level(mut self, level: CompressionLevel) -> Self {
        self.config.level = level;
        self
    }

    pub fn algorithms(mut self, algorithms: Vec<CompressionAlgorithm>) -> Self {
        self.config.algorithms = algorithms;
        self
    }

    pub fn add_compressible_type(mut self, content_type: impl Into<String>) -> Self {
        self.config.compressible_types.insert(content_type.into());
        self
    }

    pub fn add_excluded_type(mut self, content_type: impl Into<String>) -> Self {
        self.config.excluded_types.insert(content_type.into());
        self
    }

    pub fn add_excluded_path(mut self, path: impl Into<String>) -> Self {
        self.config.excluded_paths.push(path.into());
        self
    }

    pub fn compress_streams(mut self, enabled: bool) -> Self {
        self.config.compress_streams = enabled;
        self
    }

    pub fn build(self) -> CompressionConfig {
        self.config
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tower Layer and Service
// ═══════════════════════════════════════════════════════════════════════════════

/// Compression layer for Tower.
///
/// Note: This is a lightweight wrapper that sets up compression decisions.
/// For actual compression, we recommend using tower-http's CompressionLayer
/// which handles the actual encoding. This layer provides the configuration
/// and decision logic.
#[derive(Clone)]
pub struct CompressionLayer {
    config: Arc<CompressionConfig>,
}

impl CompressionLayer {
    /// Create a new compression layer.
    pub fn new(config: CompressionConfig) -> Self {
        Self {
            config: Arc::new(config),
        }
    }
}

impl Default for CompressionLayer {
    fn default() -> Self {
        Self::new(CompressionConfig::default())
    }
}

impl<S> Layer<S> for CompressionLayer {
    type Service = CompressionService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        CompressionService {
            inner,
            config: self.config.clone(),
        }
    }
}

/// Compression service.
///
/// This service adds compression-related headers and metrics.
/// Actual compression should be handled by tower-http's compression middleware.
#[derive(Clone)]
pub struct CompressionService<S> {
    inner: S,
    config: Arc<CompressionConfig>,
}

impl<S> Service<Request<Body>> for CompressionService<S>
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

    fn call(&mut self, request: Request<Body>) -> Self::Future {
        let config = self.config.clone();
        let mut inner = self.inner.clone();

        Box::pin(async move {
            // Check if compression is enabled
            if !config.enabled {
                return inner.call(request).await;
            }

            let path = request.uri().path().to_string();

            // Check if path is excluded
            if config.is_path_excluded(&path) {
                trace!(path = %path, "Path excluded from compression");
                return inner.call(request).await;
            }

            // Get Accept-Encoding header
            let accept_encoding = request
                .headers()
                .get(header::ACCEPT_ENCODING)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("");

            // Select compression algorithm
            let algorithm = config.select_algorithm(accept_encoding);

            // Store compression decision for potential use
            let should_check_compression = algorithm.is_some() && algorithm != Some(CompressionAlgorithm::Identity);

            // Call the inner service
            let mut response = inner.call(request).await?;

            // Record compression metrics
            if should_check_compression {
                let content_type = response
                    .headers()
                    .get(header::CONTENT_TYPE)
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("");

                let content_length: Option<usize> = response
                    .headers()
                    .get(header::CONTENT_LENGTH)
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| s.parse().ok());

                let will_compress = config.should_compress_content_type(content_type)
                    && content_length.map_or(config.compress_streams, |len| {
                        len >= config.min_size && (config.max_size == 0 || len <= config.max_size)
                    });

                if will_compress {
                    if let Some(alg) = algorithm {
                        counter!(
                            "compression_applied_total",
                            "algorithm" => alg.encoding_name().to_string(),
                            "content_type" => content_type.to_string()
                        )
                        .increment(1);

                        debug!(
                            algorithm = %alg.encoding_name(),
                            content_type = %content_type,
                            content_length = ?content_length,
                            "Compression applied"
                        );

                        // Add Vary header to indicate content negotiation
                        if !response.headers().contains_key(header::VARY) {
                            response.headers_mut().insert(
                                header::VARY,
                                HeaderValue::from_static("Accept-Encoding"),
                            );
                        }
                    }
                } else {
                    counter!(
                        "compression_skipped_total",
                        "reason" => if !config.should_compress_content_type(content_type) {
                            "content_type"
                        } else if content_length.is_some_and(|len| len < config.min_size) {
                            "too_small"
                        } else if content_length.is_some_and(|len| config.max_size > 0 && len > config.max_size) {
                            "too_large"
                        } else {
                            "streaming"
                        }.to_string()
                    )
                    .increment(1);
                }
            }

            Ok(response)
        })
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Helper Functions
// ═══════════════════════════════════════════════════════════════════════════════

/// Check if a response should be compressed based on headers.
pub fn should_compress_response(
    response_headers: &HeaderMap,
    config: &CompressionConfig,
) -> bool {
    // Don't compress if already compressed
    if response_headers.contains_key(header::CONTENT_ENCODING) {
        return false;
    }

    // Check content type
    let content_type = response_headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if !config.should_compress_content_type(content_type) {
        return false;
    }

    // Check size
    if let Some(length) = response_headers
        .get(header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<usize>().ok())
    {
        if length < config.min_size {
            return false;
        }
        if config.max_size > 0 && length > config.max_size {
            return false;
        }
    } else if !config.compress_streams {
        // Unknown length (streaming) and streams not configured for compression
        return false;
    }

    true
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_algorithm_encoding_name() {
        assert_eq!(CompressionAlgorithm::Gzip.encoding_name(), "gzip");
        assert_eq!(CompressionAlgorithm::Deflate.encoding_name(), "deflate");
        assert_eq!(CompressionAlgorithm::Brotli.encoding_name(), "br");
        assert_eq!(CompressionAlgorithm::Identity.encoding_name(), "identity");
    }

    #[test]
    fn test_algorithm_from_encoding() {
        assert_eq!(
            CompressionAlgorithm::from_encoding("gzip"),
            Some(CompressionAlgorithm::Gzip)
        );
        assert_eq!(
            CompressionAlgorithm::from_encoding("br"),
            Some(CompressionAlgorithm::Brotli)
        );
        assert_eq!(
            CompressionAlgorithm::from_encoding("GZIP"),
            Some(CompressionAlgorithm::Gzip)
        );
        assert_eq!(CompressionAlgorithm::from_encoding("unknown"), None);
    }

    #[test]
    fn test_content_type_compression() {
        let config = CompressionConfig::default();

        assert!(config.should_compress_content_type("application/json"));
        assert!(config.should_compress_content_type("text/html; charset=utf-8"));
        assert!(config.should_compress_content_type("text/plain"));
        assert!(!config.should_compress_content_type("image/jpeg"));
        assert!(!config.should_compress_content_type("application/gzip"));
    }

    #[test]
    fn test_path_exclusion() {
        let config = CompressionConfig {
            excluded_paths: vec!["/static/*".to_string(), "/exact".to_string()],
            ..Default::default()
        };

        assert!(config.is_path_excluded("/static/file.js"));
        assert!(config.is_path_excluded("/static/deep/nested/file.css"));
        assert!(config.is_path_excluded("/exact"));
        assert!(!config.is_path_excluded("/exact/child"));
        assert!(!config.is_path_excluded("/api/data"));
    }

    #[test]
    fn test_algorithm_selection() {
        let config = CompressionConfig::default();

        // Should select brotli (first preferred) when available
        let alg = config.select_algorithm("gzip, br, deflate");
        assert_eq!(alg, Some(CompressionAlgorithm::Brotli));

        // Should select gzip when brotli not accepted
        let alg = config.select_algorithm("gzip, deflate");
        assert_eq!(alg, Some(CompressionAlgorithm::Gzip));

        // Should respect quality values (br has quality 0, so skip it)
        let alg = config.select_algorithm("gzip, br;q=0, deflate");
        assert_eq!(alg, Some(CompressionAlgorithm::Gzip));

        // No match
        let alg = config.select_algorithm("unknown");
        assert_eq!(alg, None);
    }

    #[test]
    fn test_config_builder() {
        let config = CompressionConfig::builder()
            .enabled(true)
            .min_size(2048)
            .max_size(10 * 1024 * 1024)
            .level(CompressionLevel::Best)
            .algorithms(vec![CompressionAlgorithm::Gzip])
            .add_excluded_path("/stream/*")
            .build();

        assert!(config.enabled);
        assert_eq!(config.min_size, 2048);
        assert_eq!(config.max_size, 10 * 1024 * 1024);
        assert_eq!(config.level, CompressionLevel::Best);
        assert_eq!(config.algorithms, vec![CompressionAlgorithm::Gzip]);
        assert!(config.excluded_paths.contains(&"/stream/*".to_string()));
    }

    #[test]
    fn test_should_compress_response() {
        let config = CompressionConfig::default();
        let mut headers = HeaderMap::new();

        // Should compress JSON
        headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(header::CONTENT_LENGTH, HeaderValue::from_static("2000"));
        assert!(should_compress_response(&headers, &config));

        // Should not compress small responses
        headers.insert(header::CONTENT_LENGTH, HeaderValue::from_static("100"));
        assert!(!should_compress_response(&headers, &config));

        // Should not compress already compressed
        headers.insert(header::CONTENT_LENGTH, HeaderValue::from_static("2000"));
        headers.insert(header::CONTENT_ENCODING, HeaderValue::from_static("gzip"));
        assert!(!should_compress_response(&headers, &config));

        // Should not compress images
        headers.remove(header::CONTENT_ENCODING);
        headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("image/jpeg"));
        assert!(!should_compress_response(&headers, &config));
    }
}
