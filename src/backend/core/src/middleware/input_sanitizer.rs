//! Input sanitization middleware for injection attack detection.
use axum::{extract::Request, http::StatusCode, response::{IntoResponse, Response}, Json};
use futures::future::BoxFuture;
use metrics::counter;
use regex::Regex;
use serde::Serialize;
use serde_json::json;
use std::{sync::Arc, task::{Context, Poll}};
use tower::{Layer, Service};
use tracing::warn;
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum InjectionType { SqlInjection, Xss, CommandInjection, PathTraversal, LdapInjection, LogInjection }
impl std::fmt::Display for InjectionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self { Self::SqlInjection => write!(f,"SQL Injection"), Self::Xss => write!(f,"XSS"), Self::CommandInjection => write!(f,"Command Injection"), Self::PathTraversal => write!(f,"Path Traversal"), Self::LdapInjection => write!(f,"LDAP Injection"), Self::LogInjection => write!(f,"Log Injection") }
    }
}
#[derive(Debug, Clone)]
pub struct SanitizeConfig { pub block_on_detection: bool, pub skip_headers: Vec<String>, pub exempt_paths: Vec<String>, pub max_url_length: usize, pub max_header_value_length: usize }
impl Default for SanitizeConfig { fn default() -> Self { Self { block_on_detection: true, skip_headers: vec!["authorization".into(),"cookie".into(),"user-agent".into(),"accept".into(),"content-type".into(),"content-length".into(),"host".into(),"x-request-id".into(),"x-forwarded-for".into(),"x-csrf-token".into(),"x-api-key".into()], exempt_paths: vec![], max_url_length: 2048, max_header_value_length: 8192 } } }
struct Patterns { sql: Vec<Regex>, xss: Vec<Regex>, path: Vec<Regex>, log: Vec<Regex> }
impl Patterns {
    fn new() -> Self { Self { sql: vec![Regex::new(r"(?i)(\bunion\b\s+\bselect\b)").unwrap(), Regex::new(r"(?i)(\bselect\b.+\bfrom\b)").unwrap(), Regex::new(r"(?i)(\bdrop\b\s+\btable\b)").unwrap(), Regex::new(r"(?i)('?\s*(or|and)\s+\d+\s*=\s*\d+)").unwrap(), Regex::new(r"(/\*|\*/|--)").unwrap(), Regex::new(r"(?i)(\bsleep\b\s*\()").unwrap()], xss: vec![Regex::new(r"(?i)(<\s*script)").unwrap(), Regex::new(r"(?i)(javascript\s*:)").unwrap(), Regex::new(r"(?i)(on(error|load|click|mouseover)\s*=)").unwrap(), Regex::new(r"(?i)(<\s*(iframe|object|embed|svg)\b)").unwrap()], path: vec![Regex::new(r"(\.\./|\.\.\\)").unwrap(), Regex::new(r"(%2e%2e%2f|%2e%2e/)").unwrap(), Regex::new(r"(%00|%0d%0a)").unwrap()], log: vec![Regex::new(r"[\r\n]").unwrap()] } }
    fn detect(&self, s: &str) -> Option<InjectionType> { for p in &self.path { if p.is_match(s) { return Some(InjectionType::PathTraversal); } } for p in &self.sql { if p.is_match(s) { return Some(InjectionType::SqlInjection); } } for p in &self.xss { if p.is_match(s) { return Some(InjectionType::Xss); } } for p in &self.log { if p.is_match(s) { return Some(InjectionType::LogInjection); } } None }
    fn detect_url(&self, s: &str) -> Option<InjectionType> { for p in &self.path { if p.is_match(s) { return Some(InjectionType::PathTraversal); } } for p in &self.xss { if p.is_match(s) { return Some(InjectionType::Xss); } } None }
}
#[derive(Clone)] pub struct InputSanitizerLayer { config: Arc<SanitizeConfig>, patterns: Arc<Patterns> }
impl InputSanitizerLayer { pub fn new(config: SanitizeConfig) -> Self { Self { config: Arc::new(config), patterns: Arc::new(Patterns::new()) } } }
impl<S> Layer<S> for InputSanitizerLayer { type Service = InputSanitizerService<S>; fn layer(&self, inner: S) -> Self::Service { InputSanitizerService { inner, config: self.config.clone(), patterns: self.patterns.clone() } } }
#[derive(Clone)] pub struct InputSanitizerService<S> { inner: S, config: Arc<SanitizeConfig>, patterns: Arc<Patterns> }
impl<S> Service<Request> for InputSanitizerService<S> where S: Service<Request, Response = Response> + Clone + Send + 'static, S::Future: Send + 'static, {
    type Response = Response; type Error = S::Error; type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> { self.inner.poll_ready(cx) }
    fn call(&mut self, req: Request) -> Self::Future { let config = self.config.clone(); let patterns = self.patterns.clone(); let mut inner = self.inner.clone(); Box::pin(async move { let path = req.uri().path().to_string(); if config.exempt_paths.iter().any(|p| path.starts_with(p)) { return inner.call(req).await; } if req.uri().to_string().len() > config.max_url_length { counter!("sanitizer_url_too_long").increment(1); if config.block_on_detection { return Ok(bad_req("URL too long")); } } if let Some(t) = patterns.detect_url(&path) { counter!("sanitizer_blocked").increment(1); warn!(path=%path, "Injection in URL"); if config.block_on_detection { return Ok(bad_req(&t.to_string())); } } if let Some(q) = req.uri().query() { if let Some(t) = patterns.detect(q) { counter!("sanitizer_blocked").increment(1); if config.block_on_detection { return Ok(bad_req(&t.to_string())); } } } for (n, v) in req.headers() { let h = n.as_str().to_lowercase(); if config.skip_headers.contains(&h) { continue; } if let Ok(vs) = v.to_str() { if vs.len() > config.max_header_value_length { if config.block_on_detection { return Ok(bad_req("Header too long")); } } if let Some(t) = patterns.detect(vs) { counter!("sanitizer_blocked").increment(1); if config.block_on_detection { return Ok(bad_req(&t.to_string())); } } } } inner.call(req).await }) }
}
fn bad_req(detail: &str) -> Response { (StatusCode::BAD_REQUEST, Json(json!({"success":false,"error":"Malicious input detected","error_code":"MALICIOUS_INPUT","detail":detail}))).into_response() }

#[cfg(test)]
mod tests {
    use super::*;

    fn patterns() -> Patterns {
        Patterns::new()
    }

    // SQL Injection tests
    #[test]
    fn test_detect_sql_union_select() {
        assert_eq!(patterns().detect("UNION SELECT * FROM users"), Some(InjectionType::SqlInjection));
    }

    #[test]
    fn test_detect_sql_select_from() {
        assert_eq!(patterns().detect("SELECT name FROM users"), Some(InjectionType::SqlInjection));
    }

    #[test]
    fn test_detect_sql_drop_table() {
        assert_eq!(patterns().detect("DROP TABLE users"), Some(InjectionType::SqlInjection));
    }

    #[test]
    fn test_detect_sql_or_1_eq_1() {
        assert_eq!(patterns().detect("' or 1=1"), Some(InjectionType::SqlInjection));
    }

    #[test]
    fn test_detect_sql_comment() {
        assert_eq!(patterns().detect("admin'--"), Some(InjectionType::SqlInjection));
    }

    #[test]
    fn test_detect_sql_sleep() {
        assert_eq!(patterns().detect("'; SLEEP(5)"), Some(InjectionType::SqlInjection));
    }

    // XSS tests
    #[test]
    fn test_detect_xss_script_tag() {
        assert_eq!(patterns().detect("<script>alert('xss')</script>"), Some(InjectionType::Xss));
    }

    #[test]
    fn test_detect_xss_javascript_uri() {
        assert_eq!(patterns().detect("javascript:alert(1)"), Some(InjectionType::Xss));
    }

    #[test]
    fn test_detect_xss_event_handler() {
        assert_eq!(patterns().detect("onerror=alert(1)"), Some(InjectionType::Xss));
    }

    #[test]
    fn test_detect_xss_iframe() {
        assert_eq!(patterns().detect("<iframe src='evil.com'>"), Some(InjectionType::Xss));
    }

    #[test]
    fn test_detect_xss_svg() {
        assert_eq!(patterns().detect("<svg onload=alert(1)>"), Some(InjectionType::Xss));
    }

    // Path traversal tests
    #[test]
    fn test_detect_path_traversal_dotdot() {
        assert_eq!(patterns().detect("../../etc/passwd"), Some(InjectionType::PathTraversal));
    }

    #[test]
    fn test_detect_path_traversal_encoded() {
        assert_eq!(patterns().detect("%2e%2e%2f"), Some(InjectionType::PathTraversal));
    }

    #[test]
    fn test_detect_path_traversal_null_byte() {
        assert_eq!(patterns().detect("file%00.txt"), Some(InjectionType::PathTraversal));
    }

    // Log injection tests
    #[test]
    fn test_detect_log_injection_newline() {
        assert_eq!(patterns().detect("value\nINFO: forged log"), Some(InjectionType::LogInjection));
    }

    #[test]
    fn test_detect_log_injection_carriage_return() {
        assert_eq!(patterns().detect("value\rINFO: forged"), Some(InjectionType::LogInjection));
    }

    // Clean input tests
    #[test]
    fn test_clean_input_normal_text() {
        assert!(patterns().detect("hello world 123").is_none());
    }

    #[test]
    fn test_clean_input_email() {
        assert!(patterns().detect("user@example.com").is_none());
    }

    #[test]
    fn test_clean_input_uuid() {
        assert!(patterns().detect("550e8400-e29b-41d4-a716-446655440000").is_none());
    }

    // URL-specific detection
    #[test]
    fn test_detect_url_path_traversal() {
        assert_eq!(patterns().detect_url("../../etc/passwd"), Some(InjectionType::PathTraversal));
    }

    #[test]
    fn test_detect_url_xss() {
        assert_eq!(patterns().detect_url("<script>alert(1)</script>"), Some(InjectionType::Xss));
    }

    #[test]
    fn test_detect_url_clean() {
        assert!(patterns().detect_url("/api/v1/users").is_none());
    }

    #[test]
    fn test_detect_url_no_sql_detection() {
        // URL detection should NOT check for SQL injection
        assert!(patterns().detect_url("SELECT FROM").is_none());
    }

    // InjectionType display
    #[test]
    fn test_injection_type_display() {
        assert_eq!(InjectionType::SqlInjection.to_string(), "SQL Injection");
        assert_eq!(InjectionType::Xss.to_string(), "XSS");
        assert_eq!(InjectionType::CommandInjection.to_string(), "Command Injection");
        assert_eq!(InjectionType::PathTraversal.to_string(), "Path Traversal");
        assert_eq!(InjectionType::LdapInjection.to_string(), "LDAP Injection");
        assert_eq!(InjectionType::LogInjection.to_string(), "Log Injection");
    }

    // Config tests
    #[test]
    fn test_sanitize_config_defaults() {
        let config = SanitizeConfig::default();
        assert!(config.block_on_detection);
        assert_eq!(config.max_url_length, 2048);
        assert_eq!(config.max_header_value_length, 8192);
        assert!(!config.skip_headers.is_empty());
        assert!(config.skip_headers.contains(&"authorization".to_string()));
    }

    #[test]
    fn test_sanitize_config_exempt_paths() {
        let config = SanitizeConfig {
            exempt_paths: vec!["/health".into(), "/metrics".into()],
            ..Default::default()
        };
        assert!(config.exempt_paths.contains(&"/health".to_string()));
    }

    // Priority behavior: path traversal should be detected before SQL
    #[test]
    fn test_detect_priority_path_over_sql() {
        // Path traversal patterns are checked first
        let result = patterns().detect("../../SELECT FROM");
        assert_eq!(result, Some(InjectionType::PathTraversal));
    }
}
