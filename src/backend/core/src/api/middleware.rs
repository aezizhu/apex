//! API middleware for validation, content-type enforcement, and versioning headers.
//!
//! This module provides middleware functions for:
//! - Content-Type validation (enforces application/json for mutation requests)
//! - API version response headers
//! - Input string sanitization utilities
//! - Pagination parameter validation

use axum::{
    extract::Request,
    http::{
        header::{HeaderName, HeaderValue, CONTENT_TYPE},
        Method, StatusCode,
    },
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};

/// Middleware that validates Content-Type header for mutation requests.
///
/// POST, PUT, and PATCH requests must include `Content-Type: application/json`.
/// GET, DELETE, HEAD, and OPTIONS requests are allowed without Content-Type.
pub async fn content_type_validation(req: Request, next: Next) -> Response {
    let method = req.method().clone();
    if matches!(method, Method::POST | Method::PUT | Method::PATCH) {
        if let Some(content_type) = req.headers().get(CONTENT_TYPE) {
            let ct_str = content_type.to_str().unwrap_or("");
            if !ct_str.contains("application/json") {
                return (
                    StatusCode::UNSUPPORTED_MEDIA_TYPE,
                    Json(serde_json::json!({
                        "success": false,
                        "error": "Content-Type must be application/json",
                        "error_code": "UNSUPPORTED_MEDIA_TYPE"
                    })),
                ).into_response();
            }
        } else {
            return (
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                Json(serde_json::json!({
                    "success": false,
                    "error": "Content-Type header is required for this request",
                    "error_code": "MISSING_CONTENT_TYPE"
                })),
            ).into_response();
        }
    }
    next.run(req).await
}

/// Middleware that adds standard API response headers.
pub async fn api_version_headers(req: Request, next: Next) -> Response {
    let request_id = req
        .headers()
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    let mut response = next.run(req).await;
    let headers = response.headers_mut();

    let _ = headers.try_insert(
        HeaderName::from_static("x-api-version"),
        HeaderValue::from_static("1.0"),
    );
    if let Ok(val) = HeaderValue::from_str(&request_id) {
        let _ = headers.try_insert(
            HeaderName::from_static("x-request-id"),
            val,
        );
    }
    let _ = headers.try_insert(
        HeaderName::from_static("x-content-type-options"),
        HeaderValue::from_static("nosniff"),
    );

    response
}

/// Sanitize a string input by trimming whitespace, removing null bytes,
/// and stripping basic HTML tags.
pub fn sanitize_string(input: &str) -> String {
    let trimmed = input.trim();
    let no_nulls: String = trimmed.chars().filter(|c| *c != '\0').collect();
    strip_html_tags(&no_nulls)
}

fn strip_html_tags(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut in_tag = false;
    for ch in input.chars() {
        match ch {
            '<' => in_tag = true,
            '>' if in_tag => in_tag = false,
            _ if !in_tag => result.push(ch),
            _ => {}
        }
    }
    result
}

/// Sanitize a JSON value recursively.
pub fn sanitize_json_value(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::String(s) => { *s = sanitize_string(s); }
        serde_json::Value::Array(arr) => { for item in arr.iter_mut() { sanitize_json_value(item); } }
        serde_json::Value::Object(map) => { for (_key, val) in map.iter_mut() { sanitize_json_value(val); } }
        _ => {}
    }
}

/// Validated pagination parameters with enforced bounds.
#[derive(Debug, Clone)]
pub struct ValidatedPagination {
    pub page: u32,
    pub per_page: u32,
    pub offset: u32,
}

const DEFAULT_PAGE_SIZE: u32 = 20;
const MAX_PAGE_SIZE: u32 = 100;

/// Validate and normalize pagination parameters.
pub fn validate_pagination(page: Option<u32>, per_page: Option<u32>) -> ValidatedPagination {
    let page = page.unwrap_or(1).max(1);
    let per_page = per_page.unwrap_or(DEFAULT_PAGE_SIZE).clamp(1, MAX_PAGE_SIZE);
    let offset = (page - 1) * per_page;
    ValidatedPagination { page, per_page, offset }
}

/// Validation error details for API responses.
#[derive(Debug, serde::Serialize)]
pub struct ValidationErrors {
    pub errors: Vec<FieldError>,
}

/// A single field validation error.
#[derive(Debug, serde::Serialize)]
pub struct FieldError {
    pub field: String,
    pub message: String,
}

impl ValidationErrors {
    pub fn new() -> Self { Self { errors: Vec::new() } }
    pub fn add(&mut self, field: impl Into<String>, message: impl Into<String>) {
        self.errors.push(FieldError { field: field.into(), message: message.into() });
    }
    pub fn is_empty(&self) -> bool { self.errors.is_empty() }
}

impl Default for ValidationErrors {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_string_trims() {
        assert_eq!(sanitize_string("  hello  "), "hello");
    }

    #[test]
    fn test_sanitize_string_strips_html() {
        assert_eq!(sanitize_string("<b>bold</b> text"), "bold text");
    }

    #[test]
    fn test_validate_pagination_defaults() {
        let p = validate_pagination(None, None);
        assert_eq!(p.page, 1);
        assert_eq!(p.per_page, 20);
        assert_eq!(p.offset, 0);
    }

    #[test]
    fn test_validate_pagination_clamps_max() {
        let p = validate_pagination(Some(1), Some(500));
        assert_eq!(p.per_page, 100);
    }

    #[test]
    fn test_validate_pagination_offset() {
        let p = validate_pagination(Some(3), Some(25));
        assert_eq!(p.offset, 50);
    }

    #[test]
    fn test_validation_errors() {
        let mut errors = ValidationErrors::new();
        assert!(errors.is_empty());
        errors.add("name", "must not be empty");
        assert!(!errors.is_empty());
        assert_eq!(errors.errors.len(), 1);
    }

    #[test]
    fn test_sanitize_json_value() {
        let mut val = serde_json::json!({
            "name": "  test  ",
            "nested": { "value": "<b>html</b>" },
            "number": 42
        });
        sanitize_json_value(&mut val);
        assert_eq!(val["name"], "test");
        assert_eq!(val["nested"]["value"], "html");
        assert_eq!(val["number"], 42);
    }
}
