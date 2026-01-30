#![allow(clippy::result_large_err)]
//! # Apex Core
//!
//! The world's No. 1 Agent Swarm Orchestration Engine.
//!
//! ## Architecture
//!
//! - **DAG Executor**: Manages task dependencies and parallel execution
//! - **Agent Contracts**: Enforces resource limits (tokens, cost, time)
//! - **Model Router**: FrugalGPT-style adaptive model selection
//! - **Observability**: Full distributed tracing and metrics
//! - **Telemetry**: Comprehensive logging, tracing, and metrics infrastructure
//! - **WebSocket**: Real-time communication with heartbeat, rooms, and broadcasting
//! - **Middleware**: Production-grade rate limiting, authentication, tracing, and compression
//! - **Validation**: Comprehensive request validation with sync and async support
//! - **Cache**: Multi-tier caching with tag-based invalidation and HTTP caching middleware
//! - **Pagination**: Cursor and offset-based pagination utilities
//! - **RBAC**: Role-based access control with multi-tenancy and policy engine
//! - **Plugins**: Plugin marketplace with manifest-driven discovery, sandboxed execution, and lifecycle management

pub mod orchestrator;
pub mod dag;
pub mod contracts;
pub mod agents;
pub mod routing;
pub mod api;
pub mod db;
pub mod observability;
pub mod telemetry;
pub mod config;
pub mod error;
pub mod websocket;
pub mod middleware;
pub mod rbac;
pub mod validation;
pub mod cache;
pub mod pagination;
pub mod health;
pub mod jobs;
pub mod events;
pub mod plugins;

pub use error::{ApexError, Result, ErrorCode, ErrorContext, ErrorDetails, ErrorSeverity};

/// Re-export commonly used types
pub mod prelude {
    pub use crate::orchestrator::SwarmOrchestrator;
    pub use crate::dag::{TaskDAG, Task, TaskId, TaskStatus};
    pub use crate::contracts::{AgentContract, ResourceLimits};
    pub use crate::agents::{Agent, AgentId, AgentStatus};
    pub use crate::routing::ModelRouter;
    pub use crate::error::{ApexError, Result, ErrorCode, ErrorContext, ErrorDetails, ErrorSeverity};
    pub use crate::websocket::{
        WebSocketState, WebSocketConfig, WebSocketStats,
        ConnectionId, ConnectionState,
        ClientMessage, ServerMessage, SubscriptionTarget,
        TaskUpdate, AgentUpdate, DagUpdate, MetricsSnapshot,
        ApprovalRequest, ApprovalResponse,
        RoomId, RoomType,
    };
    pub use crate::rbac::{
        PolicyEngine, PolicyDecision, PolicyError,
        Permission, Role, RoleId, RoleBinding, UserId, OrganizationId,
        Organization, OrganizationMember, OrganizationStatus, MemberRole,
        ResourceScope, PredefinedRole,
        RequirePermissionLayer, RequirePermissionService, RbacContext,
    };
    pub use crate::middleware::{
        RateLimitLayer, RateLimitConfig, RateLimitError,
        AuthLayer, AuthConfig, Claims, AuthError, AuthContext, AuthMethod,
        TracingLayer, TracingConfig, RequestContext,
        CompressionLayer, CompressionConfig, CompressionAlgorithm, CompressionLevel,
        SecurityHeadersLayer, SecurityHeadersConfig, FrameOptions, ReferrerPolicy,
        RequestSizeLayer, RequestSizeConfig,
        AuditLayer, AuditConfig, AuditEntry, AuditLevel, AuditLogger,
        CsrfLayer, CsrfConfig,
        ApiKeyManager, ApiKeyConfig, ApiKeyEntry, GeneratedKey, KeyStatus,
        InputSanitizerLayer, SanitizeConfig, InjectionType,
    };
    pub use crate::validation::{
        Validate, ValidateAsync, ValidateFull, ValidationRule,
        ValidationErrors, ValidationResult, ValidationErrorKind, FieldError,
        validate_field, validate_request, validate_field_async, validate_request_async,
        Required, Email, Url, Uuid, MinLength, MaxLength, Min, Max, Range, Pattern,
    };
    pub use crate::cache::{
        Cache, CacheConfig, CacheKey, KeyType, KeyBuilder,
        CacheBackend, CacheEntry, CacheStats,
        InMemoryBackend, InMemoryConfig,
        RedisBackend, RedisConfig,
        MultiTierBackend, MultiTierConfig,
        InvalidationEngine, InvalidationEvent, InvalidationStrategy,
        CacheMiddlewareLayer, CacheMiddleware, CacheMiddlewareConfig,
        ETagGenerator, CacheControl, CacheDirective,
    };
    pub use crate::pagination::{
        Cursor, CursorBuilder, CursorPagination, CursorValue, SortDirection, SortField,
        OffsetPagination, OffsetPaginationBuilder, PageMetadata,
        PaginationMode, PaginationQuery, PaginationQueryBuilder,
        CursorInfo, PageInfo, PaginatedResponse, PaginationInfo,
    };
}
