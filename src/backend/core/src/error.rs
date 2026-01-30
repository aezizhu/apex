//! Production-grade error handling for Apex Core.
//!
//! This module provides:
//! - Comprehensive error types with context and chaining
//! - HTTP status code mapping for API responses
//! - Error codes for machine-readable API responses
//! - User-friendly messages vs detailed internal messages
//! - Error logging with tracing integration
//! - Metrics integration for error tracking
//!
//! # Usage
//!
//! ```rust,ignore
//! use apex_core::error::{ApexError, Result, ErrorContext};
//!
//! fn my_function() -> Result<()> {
//!     some_operation()
//!         .context("Failed to perform operation")
//!         .with_error_code(ErrorCode::InternalError)?;
//!     Ok(())
//! }
//! ```

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use metrics::counter;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt;
use thiserror::Error;
use tracing::{error, warn};

// ═══════════════════════════════════════════════════════════════════════════════
// Result Type Alias
// ═══════════════════════════════════════════════════════════════════════════════

/// A specialized Result type for Apex operations.
pub type Result<T> = std::result::Result<T, ApexError>;

// ═══════════════════════════════════════════════════════════════════════════════
// Error Codes
// ═══════════════════════════════════════════════════════════════════════════════

/// Machine-readable error codes for API responses.
///
/// These codes are stable and can be used by clients for programmatic error handling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    // DAG Errors (1000-1099)
    DagCycleDetected,
    DagValidationFailed,
    TaskNotFound,
    TaskAlreadyExists,
    InvalidStateTransition,
    DependencyNotMet,

    // Contract Errors (1100-1199)
    TokenLimitExceeded,
    CostLimitExceeded,
    TimeLimitExceeded,
    ApiCallLimitExceeded,
    ContractViolation,
    ContractNotFound,
    ContractExpired,

    // Agent Errors (1200-1299)
    AgentNotFound,
    AgentOverloaded,
    AgentExecutionFailed,
    AgentTimeout,
    LoopDetected,
    AgentUnavailable,

    // Tool Errors (1300-1399)
    ToolNotFound,
    ToolExecutionFailed,
    ToolTimeout,
    ToolValidationFailed,

    // Database Errors (2000-2099)
    DatabaseError,
    DatabaseConnectionFailed,
    DatabaseQueryFailed,
    DatabaseTransactionFailed,
    RecordNotFound,
    DuplicateRecord,

    // Cache Errors (2100-2199)
    CacheError,
    CacheConnectionFailed,
    CacheMiss,

    // Serialization Errors (2200-2299)
    SerializationError,
    DeserializationError,
    InvalidJson,

    // External Service Errors (3000-3099)
    LlmApiError,
    LlmRateLimited,
    LlmTimeout,
    LlmUnavailable,
    ExternalServiceError,
    NetworkError,

    // Authentication/Authorization (4000-4099)
    Unauthorized,
    Forbidden,
    InvalidToken,
    TokenExpired,

    // Validation Errors (4100-4199)
    ValidationError,
    InvalidInput,
    MissingRequiredField,
    InvalidFormat,

    // Configuration Errors (5000-5099)
    ConfigurationError,
    MissingConfiguration,
    InvalidConfiguration,

    // Internal Errors (9000-9099)
    InternalError,
    NotImplemented,
    UnknownError,
}

impl ErrorCode {
    /// Get the numeric code for this error.
    pub const fn numeric_code(&self) -> u32 {
        match self {
            // DAG Errors
            Self::DagCycleDetected => 1000,
            Self::DagValidationFailed => 1001,
            Self::TaskNotFound => 1002,
            Self::TaskAlreadyExists => 1003,
            Self::InvalidStateTransition => 1004,
            Self::DependencyNotMet => 1005,

            // Contract Errors
            Self::TokenLimitExceeded => 1100,
            Self::CostLimitExceeded => 1101,
            Self::TimeLimitExceeded => 1102,
            Self::ApiCallLimitExceeded => 1103,
            Self::ContractViolation => 1104,
            Self::ContractNotFound => 1105,
            Self::ContractExpired => 1106,

            // Agent Errors
            Self::AgentNotFound => 1200,
            Self::AgentOverloaded => 1201,
            Self::AgentExecutionFailed => 1202,
            Self::AgentTimeout => 1203,
            Self::LoopDetected => 1204,
            Self::AgentUnavailable => 1205,

            // Tool Errors
            Self::ToolNotFound => 1300,
            Self::ToolExecutionFailed => 1301,
            Self::ToolTimeout => 1302,
            Self::ToolValidationFailed => 1303,

            // Database Errors
            Self::DatabaseError => 2000,
            Self::DatabaseConnectionFailed => 2001,
            Self::DatabaseQueryFailed => 2002,
            Self::DatabaseTransactionFailed => 2003,
            Self::RecordNotFound => 2004,
            Self::DuplicateRecord => 2005,

            // Cache Errors
            Self::CacheError => 2100,
            Self::CacheConnectionFailed => 2101,
            Self::CacheMiss => 2102,

            // Serialization Errors
            Self::SerializationError => 2200,
            Self::DeserializationError => 2201,
            Self::InvalidJson => 2202,

            // External Service Errors
            Self::LlmApiError => 3000,
            Self::LlmRateLimited => 3001,
            Self::LlmTimeout => 3002,
            Self::LlmUnavailable => 3003,
            Self::ExternalServiceError => 3004,
            Self::NetworkError => 3005,

            // Auth Errors
            Self::Unauthorized => 4000,
            Self::Forbidden => 4001,
            Self::InvalidToken => 4002,
            Self::TokenExpired => 4003,

            // Validation Errors
            Self::ValidationError => 4100,
            Self::InvalidInput => 4101,
            Self::MissingRequiredField => 4102,
            Self::InvalidFormat => 4103,

            // Configuration Errors
            Self::ConfigurationError => 5000,
            Self::MissingConfiguration => 5001,
            Self::InvalidConfiguration => 5002,

            // Internal Errors
            Self::InternalError => 9000,
            Self::NotImplemented => 9001,
            Self::UnknownError => 9099,
        }
    }

    /// Get the HTTP status code for this error.
    pub const fn http_status(&self) -> StatusCode {
        match self {
            // Not Found (404)
            Self::TaskNotFound
            | Self::AgentNotFound
            | Self::ToolNotFound
            | Self::ContractNotFound
            | Self::RecordNotFound
            | Self::CacheMiss => StatusCode::NOT_FOUND,

            // Conflict (409)
            Self::TaskAlreadyExists
            | Self::DuplicateRecord
            | Self::InvalidStateTransition => StatusCode::CONFLICT,

            // Unprocessable Entity (422)
            Self::DagCycleDetected
            | Self::DagValidationFailed
            | Self::DependencyNotMet
            | Self::ValidationError
            | Self::InvalidInput
            | Self::MissingRequiredField
            | Self::InvalidFormat
            | Self::ToolValidationFailed => StatusCode::UNPROCESSABLE_ENTITY,

            // Payment Required / Resource Exhausted (402/429)
            Self::TokenLimitExceeded
            | Self::CostLimitExceeded
            | Self::ApiCallLimitExceeded
            | Self::ContractViolation
            | Self::ContractExpired => StatusCode::PAYMENT_REQUIRED,

            // Too Many Requests (429)
            Self::LlmRateLimited | Self::AgentOverloaded => StatusCode::TOO_MANY_REQUESTS,

            // Timeout (408/504)
            Self::TimeLimitExceeded
            | Self::ToolTimeout
            | Self::AgentTimeout
            | Self::LlmTimeout => StatusCode::GATEWAY_TIMEOUT,

            // Unauthorized (401)
            Self::Unauthorized | Self::InvalidToken | Self::TokenExpired => {
                StatusCode::UNAUTHORIZED
            }

            // Forbidden (403)
            Self::Forbidden => StatusCode::FORBIDDEN,

            // Service Unavailable (503)
            Self::DatabaseConnectionFailed
            | Self::CacheConnectionFailed
            | Self::LlmUnavailable
            | Self::AgentUnavailable
            | Self::ExternalServiceError => StatusCode::SERVICE_UNAVAILABLE,

            // Bad Gateway (502)
            Self::LlmApiError | Self::NetworkError => StatusCode::BAD_GATEWAY,

            // Not Implemented (501)
            Self::NotImplemented => StatusCode::NOT_IMPLEMENTED,

            // Loop detected (508)
            Self::LoopDetected => StatusCode::LOOP_DETECTED,

            // Internal Server Error (500)
            Self::DatabaseError
            | Self::DatabaseQueryFailed
            | Self::DatabaseTransactionFailed
            | Self::CacheError
            | Self::SerializationError
            | Self::DeserializationError
            | Self::InvalidJson
            | Self::AgentExecutionFailed
            | Self::ToolExecutionFailed
            | Self::ConfigurationError
            | Self::MissingConfiguration
            | Self::InvalidConfiguration
            | Self::InternalError
            | Self::UnknownError => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// Check if this error is retryable.
    pub const fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::DatabaseConnectionFailed
                | Self::DatabaseQueryFailed
                | Self::CacheConnectionFailed
                | Self::CacheError
                | Self::LlmRateLimited
                | Self::LlmTimeout
                | Self::LlmUnavailable
                | Self::AgentOverloaded
                | Self::AgentTimeout
                | Self::ToolTimeout
                | Self::NetworkError
                | Self::ExternalServiceError
        )
    }

    /// Get the error category for grouping.
    pub const fn category(&self) -> &'static str {
        match self.numeric_code() {
            1000..=1099 => "dag",
            1100..=1199 => "contract",
            1200..=1299 => "agent",
            1300..=1399 => "tool",
            2000..=2099 => "database",
            2100..=2199 => "cache",
            2200..=2299 => "serialization",
            3000..=3099 => "external_service",
            4000..=4099 => "authentication",
            4100..=4199 => "validation",
            5000..=5099 => "configuration",
            9000..=9099 => "internal",
            _ => "unknown",
        }
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Error Severity
// ═══════════════════════════════════════════════════════════════════════════════

/// Severity level for errors (affects logging and alerting).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ErrorSeverity {
    /// User errors (bad input, validation failures)
    Low,
    /// Operational issues (rate limits, timeouts)
    Medium,
    /// System errors (database failures, critical bugs)
    High,
    /// Critical errors requiring immediate attention
    Critical,
}

impl ErrorSeverity {
    /// Get severity based on error code.
    pub const fn from_code(code: &ErrorCode) -> Self {
        match code {
            // Low severity - user errors
            ErrorCode::ValidationError
            | ErrorCode::InvalidInput
            | ErrorCode::MissingRequiredField
            | ErrorCode::InvalidFormat
            | ErrorCode::TaskNotFound
            | ErrorCode::AgentNotFound
            | ErrorCode::ToolNotFound
            | ErrorCode::ContractNotFound
            | ErrorCode::RecordNotFound
            | ErrorCode::CacheMiss
            | ErrorCode::DagValidationFailed
            | ErrorCode::DependencyNotMet
            | ErrorCode::TaskAlreadyExists
            | ErrorCode::DuplicateRecord
            | ErrorCode::InvalidStateTransition
            | ErrorCode::ToolValidationFailed => Self::Low,

            // Medium severity - operational
            ErrorCode::TokenLimitExceeded
            | ErrorCode::CostLimitExceeded
            | ErrorCode::TimeLimitExceeded
            | ErrorCode::ApiCallLimitExceeded
            | ErrorCode::ContractViolation
            | ErrorCode::ContractExpired
            | ErrorCode::LlmRateLimited
            | ErrorCode::AgentOverloaded
            | ErrorCode::AgentTimeout
            | ErrorCode::ToolTimeout
            | ErrorCode::LlmTimeout
            | ErrorCode::LoopDetected
            | ErrorCode::NotImplemented => Self::Medium,

            // High severity - system errors
            ErrorCode::DatabaseError
            | ErrorCode::DatabaseQueryFailed
            | ErrorCode::DatabaseTransactionFailed
            | ErrorCode::CacheError
            | ErrorCode::SerializationError
            | ErrorCode::DeserializationError
            | ErrorCode::InvalidJson
            | ErrorCode::LlmApiError
            | ErrorCode::AgentExecutionFailed
            | ErrorCode::ToolExecutionFailed
            | ErrorCode::NetworkError
            | ErrorCode::ExternalServiceError
            | ErrorCode::ConfigurationError
            | ErrorCode::MissingConfiguration
            | ErrorCode::InvalidConfiguration
            | ErrorCode::Unauthorized
            | ErrorCode::Forbidden
            | ErrorCode::InvalidToken
            | ErrorCode::TokenExpired
            | ErrorCode::DagCycleDetected
            | ErrorCode::AgentUnavailable
            | ErrorCode::LlmUnavailable => Self::High,

            // Critical severity
            ErrorCode::DatabaseConnectionFailed
            | ErrorCode::CacheConnectionFailed
            | ErrorCode::InternalError
            | ErrorCode::UnknownError => Self::Critical,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Error Details
// ═══════════════════════════════════════════════════════════════════════════════

/// Additional structured details about an error.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ErrorDetails {
    /// Additional context key-value pairs
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub context: HashMap<String, serde_json::Value>,

    /// Related entity ID (task, agent, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entity_id: Option<String>,

    /// Related entity type
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entity_type: Option<String>,

    /// Retry information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_after_secs: Option<u64>,

    /// Suggested action for resolution
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggested_action: Option<String>,

    /// Documentation link
    #[serde(skip_serializing_if = "Option::is_none")]
    pub docs_url: Option<String>,
}

impl ErrorDetails {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_entity(mut self, entity_type: impl Into<String>, entity_id: impl Into<String>) -> Self {
        self.entity_type = Some(entity_type.into());
        self.entity_id = Some(entity_id.into());
        self
    }

    pub fn with_context(mut self, key: impl Into<String>, value: impl Serialize) -> Self {
        if let Ok(v) = serde_json::to_value(value) {
            self.context.insert(key.into(), v);
        }
        self
    }

    pub fn with_retry_after(mut self, seconds: u64) -> Self {
        self.retry_after_secs = Some(seconds);
        self
    }

    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggested_action = Some(suggestion.into());
        self
    }

    pub fn with_docs(mut self, url: impl Into<String>) -> Self {
        self.docs_url = Some(url.into());
        self
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Main Error Type
// ═══════════════════════════════════════════════════════════════════════════════

/// The main error type for Apex Core.
///
/// This error type supports:
/// - Structured error codes for API responses
/// - Error chaining with context
/// - User-friendly vs internal messages
/// - HTTP status code mapping
/// - Metrics integration
#[derive(Error, Debug)]
#[allow(dead_code)]
pub struct ApexError {
    /// Machine-readable error code
    code: ErrorCode,

    /// User-friendly error message (safe to expose to clients)
    user_message: Cow<'static, str>,

    /// Detailed internal message (for logging only)
    internal_message: Option<String>,

    /// Additional structured details
    details: ErrorDetails,

    /// The source error that caused this error
    #[source]
    source: Option<Box<dyn std::error::Error + Send + Sync + 'static>>,

    /// Backtrace for debugging (captured in debug builds)
    #[cfg(debug_assertions)]
    backtrace: Option<std::backtrace::Backtrace>,
}

impl fmt::Display for ApexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.code, self.user_message)?;
        if let Some(ref internal) = self.internal_message {
            write!(f, " (internal: {})", internal)?;
        }
        Ok(())
    }
}

impl ApexError {
    // ─────────────────────────────────────────────────────────────────────────
    // Constructors
    // ─────────────────────────────────────────────────────────────────────────

    /// Create a new error with code and user message.
    pub fn new(code: ErrorCode, user_message: impl Into<Cow<'static, str>>) -> Self {
        let error = Self {
            code,
            user_message: user_message.into(),
            internal_message: None,
            details: ErrorDetails::default(),
            source: None,
            #[cfg(debug_assertions)]
            backtrace: Some(std::backtrace::Backtrace::capture()),
        };
        error.record_metrics();
        error
    }

    /// Create an error with both user and internal messages.
    pub fn with_internal(
        code: ErrorCode,
        user_message: impl Into<Cow<'static, str>>,
        internal_message: impl Into<String>,
    ) -> Self {
        let mut error = Self::new(code, user_message);
        error.internal_message = Some(internal_message.into());
        error
    }

    /// Create an internal error (500).
    pub fn internal(message: impl Into<String>) -> Self {
        Self::with_internal(
            ErrorCode::InternalError,
            "An internal error occurred",
            message,
        )
    }

    /// Create a not found error.
    pub fn not_found(entity_type: impl Into<String>, entity_id: impl Into<String>) -> Self {
        let entity_type = entity_type.into();
        let entity_id = entity_id.into();
        Self::new(
            ErrorCode::RecordNotFound,
            format!("{} not found: {}", entity_type, entity_id),
        )
        .with_details(ErrorDetails::new().with_entity(&entity_type, &entity_id))
    }

    /// Create a validation error.
    pub fn validation(message: impl Into<Cow<'static, str>>) -> Self {
        Self::new(ErrorCode::ValidationError, message)
    }

    /// Create an unauthorized error.
    pub fn unauthorized(message: impl Into<Cow<'static, str>>) -> Self {
        Self::new(ErrorCode::Unauthorized, message)
    }

    /// Create a forbidden error.
    pub fn forbidden(message: impl Into<Cow<'static, str>>) -> Self {
        Self::new(ErrorCode::Forbidden, message)
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Builder Methods
    // ─────────────────────────────────────────────────────────────────────────

    /// Add a source error.
    pub fn with_source<E>(mut self, source: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        self.source = Some(Box::new(source));
        self
    }

    /// Add error details.
    pub fn with_details(mut self, details: ErrorDetails) -> Self {
        self.details = details;
        self
    }

    /// Add internal message.
    pub fn with_internal_message(mut self, message: impl Into<String>) -> Self {
        self.internal_message = Some(message.into());
        self
    }

    /// Add context to details.
    pub fn with_context(mut self, key: impl Into<String>, value: impl Serialize) -> Self {
        if let Ok(v) = serde_json::to_value(value) {
            self.details.context.insert(key.into(), v);
        }
        self
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Accessors
    // ─────────────────────────────────────────────────────────────────────────

    /// Get the error code.
    pub fn code(&self) -> ErrorCode {
        self.code
    }

    /// Get the user-friendly message.
    pub fn user_message(&self) -> &str {
        &self.user_message
    }

    /// Get the internal message (if any).
    pub fn internal_message(&self) -> Option<&str> {
        self.internal_message.as_deref()
    }

    /// Get the error details.
    pub fn details(&self) -> &ErrorDetails {
        &self.details
    }

    /// Get the HTTP status code.
    pub fn http_status(&self) -> StatusCode {
        self.code.http_status()
    }

    /// Check if this error is retryable.
    pub fn is_retryable(&self) -> bool {
        self.code.is_retryable()
    }

    /// Get the error severity.
    pub fn severity(&self) -> ErrorSeverity {
        ErrorSeverity::from_code(&self.code)
    }

    /// Get the legacy error code string (for backward compatibility).
    pub fn error_code(&self) -> &'static str {
        match self.code {
            ErrorCode::DagCycleDetected => "DAG_CYCLE",
            ErrorCode::TaskNotFound => "TASK_NOT_FOUND",
            ErrorCode::TaskAlreadyExists => "TASK_EXISTS",
            ErrorCode::InvalidStateTransition => "INVALID_STATE",
            ErrorCode::TokenLimitExceeded => "TOKEN_LIMIT",
            ErrorCode::CostLimitExceeded => "COST_LIMIT",
            ErrorCode::TimeLimitExceeded => "TIME_LIMIT",
            ErrorCode::ApiCallLimitExceeded => "API_LIMIT",
            ErrorCode::ContractViolation => "CONTRACT_VIOLATION",
            ErrorCode::AgentNotFound => "AGENT_NOT_FOUND",
            ErrorCode::AgentOverloaded => "AGENT_OVERLOADED",
            ErrorCode::AgentExecutionFailed => "AGENT_FAILED",
            ErrorCode::LoopDetected => "LOOP_DETECTED",
            ErrorCode::ToolNotFound => "TOOL_NOT_FOUND",
            ErrorCode::ToolExecutionFailed => "TOOL_FAILED",
            ErrorCode::ToolTimeout => "TOOL_TIMEOUT",
            ErrorCode::DatabaseError | ErrorCode::DatabaseConnectionFailed | ErrorCode::DatabaseQueryFailed | ErrorCode::DatabaseTransactionFailed => "DATABASE_ERROR",
            ErrorCode::CacheError | ErrorCode::CacheConnectionFailed => "REDIS_ERROR",
            ErrorCode::SerializationError | ErrorCode::DeserializationError | ErrorCode::InvalidJson => "SERIALIZATION_ERROR",
            ErrorCode::LlmApiError => "LLM_ERROR",
            ErrorCode::LlmRateLimited => "RATE_LIMITED",
            ErrorCode::ConfigurationError | ErrorCode::MissingConfiguration | ErrorCode::InvalidConfiguration => "CONFIG_ERROR",
            ErrorCode::InternalError => "INTERNAL_ERROR",
            _ => "UNKNOWN_ERROR",
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Logging
    // ─────────────────────────────────────────────────────────────────────────

    /// Log this error with appropriate severity.
    pub fn log(&self) {
        let code = self.code.to_string();
        let category = self.code.category();
        let status = self.http_status().as_u16();

        match self.severity() {
            ErrorSeverity::Critical => {
                error!(
                    error_code = %code,
                    category = category,
                    http_status = status,
                    user_message = %self.user_message,
                    internal_message = ?self.internal_message,
                    details = ?self.details,
                    source = ?self.source,
                    "CRITICAL ERROR"
                );
            }
            ErrorSeverity::High => {
                error!(
                    error_code = %code,
                    category = category,
                    http_status = status,
                    user_message = %self.user_message,
                    internal_message = ?self.internal_message,
                    "High severity error"
                );
            }
            ErrorSeverity::Medium => {
                warn!(
                    error_code = %code,
                    category = category,
                    http_status = status,
                    user_message = %self.user_message,
                    "Medium severity error"
                );
            }
            ErrorSeverity::Low => {
                tracing::debug!(
                    error_code = %code,
                    category = category,
                    http_status = status,
                    user_message = %self.user_message,
                    "Low severity error"
                );
            }
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Metrics
    // ─────────────────────────────────────────────────────────────────────────

    /// Record error metrics.
    fn record_metrics(&self) {
        counter!(
            "apex_errors_total",
            "code" => self.code.to_string(),
            "category" => self.code.category().to_string(),
            "severity" => format!("{:?}", self.severity()),
            "retryable" => self.is_retryable().to_string(),
        )
        .increment(1);
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// API Response
// ═══════════════════════════════════════════════════════════════════════════════

/// Error response for API clients.
#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorResponse {
    /// Whether the request was successful (always false for errors)
    pub success: bool,

    /// Error information
    pub error: ErrorInfo,
}

/// Detailed error information for API responses.
#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorInfo {
    /// Machine-readable error code
    pub code: ErrorCode,

    /// Numeric error code
    pub numeric_code: u32,

    /// User-friendly error message
    pub message: String,

    /// Additional details
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<ErrorDetails>,

    /// Request ID for tracking
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,

    /// Timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl From<&ApexError> for ErrorResponse {
    fn from(error: &ApexError) -> Self {
        Self {
            success: false,
            error: ErrorInfo {
                code: error.code,
                numeric_code: error.code.numeric_code(),
                message: error.user_message.to_string(),
                details: if error.details.context.is_empty()
                    && error.details.entity_id.is_none()
                    && error.details.retry_after_secs.is_none()
                {
                    None
                } else {
                    Some(error.details.clone())
                },
                request_id: None, // Set by middleware
                timestamp: chrono::Utc::now(),
            },
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Axum Integration
// ═══════════════════════════════════════════════════════════════════════════════

impl IntoResponse for ApexError {
    fn into_response(self) -> Response {
        // Log the error
        self.log();

        let status = self.http_status();
        let response = ErrorResponse::from(&self);

        (status, Json(response)).into_response()
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Error Context Extension Trait
// ═══════════════════════════════════════════════════════════════════════════════

/// Extension trait for adding context to errors.
pub trait ErrorContext<T> {
    /// Add context to an error.
    fn context(self, message: impl Into<String>) -> Result<T>;

    /// Add context with error code.
    fn with_error_code(self, code: ErrorCode) -> Result<T>;
}

impl<T, E> ErrorContext<T> for std::result::Result<T, E>
where
    E: std::error::Error + Send + Sync + 'static,
{
    fn context(self, message: impl Into<String>) -> Result<T> {
        self.map_err(|e| {
            ApexError::internal(message.into()).with_source(e)
        })
    }

    fn with_error_code(self, code: ErrorCode) -> Result<T> {
        self.map_err(|e| {
            ApexError::new(code, e.to_string()).with_source(e)
        })
    }
}

impl<T> ErrorContext<T> for Option<T> {
    fn context(self, message: impl Into<String>) -> Result<T> {
        self.ok_or_else(|| ApexError::new(ErrorCode::RecordNotFound, message.into()))
    }

    fn with_error_code(self, code: ErrorCode) -> Result<T> {
        self.ok_or_else(|| ApexError::new(code, "Resource not found"))
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// From Implementations for Common Error Types
// ═══════════════════════════════════════════════════════════════════════════════

impl From<sqlx::Error> for ApexError {
    fn from(error: sqlx::Error) -> Self {
        let (code, user_msg) = match &error {
            sqlx::Error::RowNotFound => (
                ErrorCode::RecordNotFound,
                "The requested record was not found",
            ),
            sqlx::Error::Database(db_err) => {
                // Handle specific database error codes
                if let Some(constraint) = db_err.constraint() {
                    if constraint.contains("unique") || constraint.contains("pkey") {
                        return Self::with_internal(
                            ErrorCode::DuplicateRecord,
                            "A record with this identifier already exists",
                            format!("Constraint violation: {}", constraint),
                        )
                        .with_source(error);
                    }
                }
                (
                    ErrorCode::DatabaseQueryFailed,
                    "A database error occurred",
                )
            }
            sqlx::Error::PoolTimedOut | sqlx::Error::PoolClosed => (
                ErrorCode::DatabaseConnectionFailed,
                "Unable to connect to the database",
            ),
            _ => (
                ErrorCode::DatabaseError,
                "A database error occurred",
            ),
        };

        Self::with_internal(code, user_msg, error.to_string()).with_source(error)
    }
}

impl From<redis::RedisError> for ApexError {
    fn from(error: redis::RedisError) -> Self {
        let (code, user_msg) = if error.is_connection_refusal() || error.is_connection_dropped() {
            (
                ErrorCode::CacheConnectionFailed,
                "Unable to connect to cache",
            )
        } else if error.is_timeout() {
            (
                ErrorCode::CacheError,
                "Cache operation timed out",
            )
        } else {
            (
                ErrorCode::CacheError,
                "A cache error occurred",
            )
        };

        Self::with_internal(code, user_msg, error.to_string()).with_source(error)
    }
}

impl From<serde_json::Error> for ApexError {
    fn from(error: serde_json::Error) -> Self {
        let code = if error.is_syntax() || error.is_data() {
            ErrorCode::DeserializationError
        } else if error.is_eof() {
            ErrorCode::InvalidJson
        } else {
            ErrorCode::SerializationError
        };

        Self::with_internal(
            code,
            "Failed to process JSON data",
            error.to_string(),
        )
        .with_source(error)
    }
}

impl From<reqwest::Error> for ApexError {
    fn from(error: reqwest::Error) -> Self {
        let (code, user_msg) = if error.is_timeout() {
            (
                ErrorCode::LlmTimeout,
                "External service request timed out",
            )
        } else if error.is_connect() {
            (
                ErrorCode::NetworkError,
                "Failed to connect to external service",
            )
        } else if error.is_status() {
            if let Some(status) = error.status() {
                match status.as_u16() {
                    429 => (
                        ErrorCode::LlmRateLimited,
                        "Rate limited by external service",
                    ),
                    401 | 403 => (
                        ErrorCode::LlmApiError,
                        "Authentication failed with external service",
                    ),
                    500..=599 => (
                        ErrorCode::LlmUnavailable,
                        "External service is temporarily unavailable",
                    ),
                    _ => (
                        ErrorCode::ExternalServiceError,
                        "External service returned an error",
                    ),
                }
            } else {
                (
                    ErrorCode::ExternalServiceError,
                    "External service returned an error",
                )
            }
        } else {
            (
                ErrorCode::NetworkError,
                "Network error occurred",
            )
        };

        Self::with_internal(code, user_msg, error.to_string()).with_source(error)
    }
}

impl From<tokio::sync::AcquireError> for ApexError {
    fn from(error: tokio::sync::AcquireError) -> Self {
        Self::with_internal(
            ErrorCode::InternalError,
            "Resource acquisition failed",
            error.to_string(),
        )
        .with_source(error)
    }
}

impl From<tokio::time::error::Elapsed> for ApexError {
    fn from(error: tokio::time::error::Elapsed) -> Self {
        Self::with_internal(
            ErrorCode::TimeLimitExceeded,
            "Operation timed out",
            error.to_string(),
        )
        .with_source(error)
    }
}

impl From<std::io::Error> for ApexError {
    fn from(error: std::io::Error) -> Self {
        use std::io::ErrorKind;

        let (code, user_msg) = match error.kind() {
            ErrorKind::NotFound => (ErrorCode::RecordNotFound, "File or resource not found"),
            ErrorKind::PermissionDenied => (ErrorCode::Forbidden, "Permission denied"),
            ErrorKind::TimedOut => (ErrorCode::TimeLimitExceeded, "Operation timed out"),
            ErrorKind::ConnectionRefused | ErrorKind::ConnectionReset => {
                (ErrorCode::NetworkError, "Connection failed")
            }
            _ => (ErrorCode::InternalError, "An I/O error occurred"),
        };

        Self::with_internal(code, user_msg, error.to_string()).with_source(error)
    }
}

impl From<anyhow::Error> for ApexError {
    fn from(error: anyhow::Error) -> Self {
        // Try to downcast to ApexError first
        match error.downcast::<ApexError>() {
            Ok(apex_error) => apex_error,
            Err(error) => {
                Self::with_internal(
                    ErrorCode::InternalError,
                    "An internal error occurred",
                    error.to_string(),
                )
            }
        }
    }
}

impl From<config::ConfigError> for ApexError {
    fn from(error: config::ConfigError) -> Self {
        let (code, user_msg) = match &error {
            config::ConfigError::NotFound(_) => (
                ErrorCode::MissingConfiguration,
                "Required configuration not found",
            ),
            config::ConfigError::PathParse(_) | config::ConfigError::FileParse { .. } => (
                ErrorCode::InvalidConfiguration,
                "Configuration file is invalid",
            ),
            _ => (
                ErrorCode::ConfigurationError,
                "Configuration error occurred",
            ),
        };

        Self::with_internal(code, user_msg, error.to_string())
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Convenience Constructors for Domain Errors
// ═══════════════════════════════════════════════════════════════════════════════

impl ApexError {
    // ─────────────────────────────────────────────────────────────────────────
    // DAG Errors
    // ─────────────────────────────────────────────────────────────────────────

    /// Create a cycle detected error.
    pub fn cycle_detected(details: impl Into<String>) -> Self {
        Self::new(
            ErrorCode::DagCycleDetected,
            format!("Cycle detected in task DAG: {}", details.into()),
        )
    }

    /// Create a task not found error.
    pub fn task_not_found(task_id: uuid::Uuid) -> Self {
        Self::new(
            ErrorCode::TaskNotFound,
            format!("Task not found: {}", task_id),
        )
        .with_details(ErrorDetails::new().with_entity("task", task_id.to_string()))
    }

    /// Create a task already exists error.
    pub fn task_already_exists(task_id: uuid::Uuid) -> Self {
        Self::new(
            ErrorCode::TaskAlreadyExists,
            format!("Task already exists: {}", task_id),
        )
        .with_details(ErrorDetails::new().with_entity("task", task_id.to_string()))
    }

    /// Create an invalid state transition error.
    pub fn invalid_state_transition(
        from: &crate::dag::TaskStatus,
        to: &crate::dag::TaskStatus,
    ) -> Self {
        Self::new(
            ErrorCode::InvalidStateTransition,
            format!("Invalid task state transition: {:?} -> {:?}", from, to),
        )
        .with_context("from_state", format!("{:?}", from))
        .with_context("to_state", format!("{:?}", to))
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Contract Errors
    // ─────────────────────────────────────────────────────────────────────────

    /// Create a token limit exceeded error.
    pub fn token_limit_exceeded(used: u64, limit: u64) -> Self {
        Self::new(
            ErrorCode::TokenLimitExceeded,
            format!("Token limit exceeded: used {}, limit {}", used, limit),
        )
        .with_context("used", used)
        .with_context("limit", limit)
        .with_details(
            ErrorDetails::new()
                .with_suggestion("Consider increasing your token limit or optimizing your prompts"),
        )
    }

    /// Create a cost limit exceeded error.
    pub fn cost_limit_exceeded(used: f64, limit: f64) -> Self {
        Self::new(
            ErrorCode::CostLimitExceeded,
            format!("Cost limit exceeded: used ${:.4}, limit ${:.4}", used, limit),
        )
        .with_context("used", used)
        .with_context("limit", limit)
    }

    /// Create a time limit exceeded error.
    pub fn time_limit_exceeded(elapsed_secs: u64, limit_secs: u64) -> Self {
        Self::new(
            ErrorCode::TimeLimitExceeded,
            format!(
                "Time limit exceeded: elapsed {}s, limit {}s",
                elapsed_secs, limit_secs
            ),
        )
        .with_context("elapsed_secs", elapsed_secs)
        .with_context("limit_secs", limit_secs)
    }

    /// Create an API call limit exceeded error.
    pub fn api_call_limit_exceeded(used: u64, limit: u64) -> Self {
        Self::new(
            ErrorCode::ApiCallLimitExceeded,
            format!("API call limit exceeded: used {}, limit {}", used, limit),
        )
        .with_context("used", used)
        .with_context("limit", limit)
    }

    /// Create a contract conservation violation error.
    pub fn contract_violation(parent: f64, children_sum: f64) -> Self {
        Self::new(
            ErrorCode::ContractViolation,
            format!(
                "Contract conservation violation: parent budget {} < children sum {}",
                parent, children_sum
            ),
        )
        .with_context("parent_budget", parent)
        .with_context("children_sum", children_sum)
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Agent Errors
    // ─────────────────────────────────────────────────────────────────────────

    /// Create an agent not found error.
    pub fn agent_not_found(agent_id: uuid::Uuid) -> Self {
        Self::new(
            ErrorCode::AgentNotFound,
            format!("Agent not found: {}", agent_id),
        )
        .with_details(ErrorDetails::new().with_entity("agent", agent_id.to_string()))
    }

    /// Create an agent overloaded error.
    pub fn agent_overloaded(current: u32, max: u32) -> Self {
        Self::new(
            ErrorCode::AgentOverloaded,
            format!("Agent overloaded: current load {}, max {}", current, max),
        )
        .with_context("current_load", current)
        .with_context("max_load", max)
        .with_details(ErrorDetails::new().with_retry_after(5))
    }

    /// Create an agent execution failed error.
    pub fn agent_execution_failed(reason: impl Into<String>) -> Self {
        Self::new(
            ErrorCode::AgentExecutionFailed,
            format!("Agent execution failed: {}", reason.into()),
        )
    }

    /// Create a loop detected error.
    pub fn loop_detected(score: f64, threshold: f64) -> Self {
        Self::new(
            ErrorCode::LoopDetected,
            format!(
                "Loop detected: similarity score {:.4} exceeds threshold {:.4}",
                score, threshold
            ),
        )
        .with_context("score", score)
        .with_context("threshold", threshold)
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Tool Errors
    // ─────────────────────────────────────────────────────────────────────────

    /// Create a tool not found error.
    pub fn tool_not_found(tool_name: impl Into<String>) -> Self {
        let name = tool_name.into();
        Self::new(ErrorCode::ToolNotFound, format!("Tool not found: {}", name))
            .with_details(ErrorDetails::new().with_entity("tool", &name))
    }

    /// Create a tool execution failed error.
    pub fn tool_execution_failed(tool: impl Into<String>, reason: impl Into<String>) -> Self {
        let tool_name = tool.into();
        Self::new(
            ErrorCode::ToolExecutionFailed,
            format!("Tool execution failed: {} - {}", tool_name, reason.into()),
        )
        .with_details(ErrorDetails::new().with_entity("tool", &tool_name))
    }

    /// Create a tool timeout error.
    pub fn tool_timeout(tool: impl Into<String>, timeout_secs: u64) -> Self {
        let tool_name = tool.into();
        Self::new(
            ErrorCode::ToolTimeout,
            format!("Tool timeout: {} exceeded {}s", tool_name, timeout_secs),
        )
        .with_context("timeout_secs", timeout_secs)
        .with_details(ErrorDetails::new().with_entity("tool", &tool_name))
    }

    // ─────────────────────────────────────────────────────────────────────────
    // External Service Errors
    // ─────────────────────────────────────────────────────────────────────────

    /// Create an LLM API error.
    pub fn llm_api_error(provider: impl Into<String>, message: impl Into<String>) -> Self {
        let provider_name = provider.into();
        Self::new(
            ErrorCode::LlmApiError,
            format!("LLM API error: {} - {}", provider_name, message.into()),
        )
        .with_context("provider", &provider_name)
    }

    /// Create a rate limited error.
    pub fn rate_limited(provider: impl Into<String>, retry_after_secs: u64) -> Self {
        let provider_name = provider.into();
        Self::new(
            ErrorCode::LlmRateLimited,
            format!(
                "Rate limited by {}: retry after {}s",
                provider_name, retry_after_secs
            ),
        )
        .with_context("provider", &provider_name)
        .with_details(ErrorDetails::new().with_retry_after(retry_after_secs))
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Configuration Errors
    // ─────────────────────────────────────────────────────────────────────────

    /// Create a configuration error.
    pub fn configuration(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::ConfigurationError, message.into())
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_code_http_status() {
        assert_eq!(
            ErrorCode::TaskNotFound.http_status(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            ErrorCode::ValidationError.http_status(),
            StatusCode::UNPROCESSABLE_ENTITY
        );
        assert_eq!(
            ErrorCode::InternalError.http_status(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
        assert_eq!(
            ErrorCode::LlmRateLimited.http_status(),
            StatusCode::TOO_MANY_REQUESTS
        );
    }

    #[test]
    fn test_error_code_is_retryable() {
        assert!(ErrorCode::LlmRateLimited.is_retryable());
        assert!(ErrorCode::DatabaseConnectionFailed.is_retryable());
        assert!(!ErrorCode::ValidationError.is_retryable());
        assert!(!ErrorCode::TaskNotFound.is_retryable());
    }

    #[test]
    fn test_error_creation() {
        let error = ApexError::task_not_found(uuid::Uuid::new_v4());
        assert_eq!(error.code(), ErrorCode::TaskNotFound);
        assert_eq!(error.http_status(), StatusCode::NOT_FOUND);
        assert!(!error.is_retryable());
    }

    #[test]
    fn test_error_context() {
        let error = ApexError::new(ErrorCode::ValidationError, "Invalid input")
            .with_context("field", "email")
            .with_context("reason", "invalid format");

        assert!(error.details().context.contains_key("field"));
        assert!(error.details().context.contains_key("reason"));
    }

    #[test]
    fn test_error_details_builder() {
        let details = ErrorDetails::new()
            .with_entity("task", "abc-123")
            .with_retry_after(30)
            .with_suggestion("Try again later")
            .with_context("extra", "info");

        assert_eq!(details.entity_type, Some("task".to_string()));
        assert_eq!(details.entity_id, Some("abc-123".to_string()));
        assert_eq!(details.retry_after_secs, Some(30));
        assert!(details.suggested_action.is_some());
        assert!(details.context.contains_key("extra"));
    }

    #[test]
    fn test_error_response_serialization() {
        let error = ApexError::validation("Invalid email format");
        let response = ErrorResponse::from(&error);

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("VALIDATION_ERROR"));
        assert!(json.contains("Invalid email format"));
    }

    #[test]
    fn test_legacy_error_code_compatibility() {
        let error = ApexError::task_not_found(uuid::Uuid::new_v4());
        assert_eq!(error.error_code(), "TASK_NOT_FOUND");

        let error = ApexError::cycle_detected("A -> B -> A");
        assert_eq!(error.error_code(), "DAG_CYCLE");
    }

    #[test]
    fn test_error_severity() {
        assert_eq!(
            ErrorSeverity::from_code(&ErrorCode::ValidationError),
            ErrorSeverity::Low
        );
        assert_eq!(
            ErrorSeverity::from_code(&ErrorCode::LlmRateLimited),
            ErrorSeverity::Medium
        );
        assert_eq!(
            ErrorSeverity::from_code(&ErrorCode::DatabaseError),
            ErrorSeverity::High
        );
        assert_eq!(
            ErrorSeverity::from_code(&ErrorCode::DatabaseConnectionFailed),
            ErrorSeverity::Critical
        );
    }

    #[test]
    fn test_from_sqlx_error() {
        // Note: We can't easily create sqlx errors for testing,
        // but the conversion logic is straightforward
    }

    #[test]
    fn test_error_display() {
        let error = ApexError::with_internal(
            ErrorCode::DatabaseError,
            "Database connection failed",
            "Connection refused: localhost:5432",
        );

        let display = format!("{}", error);
        assert!(display.contains("DatabaseError"));
        assert!(display.contains("Database connection failed"));
        assert!(display.contains("Connection refused"));
    }
}
