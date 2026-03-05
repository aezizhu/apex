//! Health check HTTP routes

use super::{HealthService, LivenessResponse, ReadinessResponse};
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Shared health service state
pub type SharedHealthService = Arc<RwLock<HealthService>>;

/// GET /health - Basic health check
pub async fn health_check(
    State(service): State<SharedHealthService>,
) -> impl IntoResponse {
    let service = service.read().await;
    let report = service.check_health().await;
    let status = if report.status.is_operational() {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (status, Json(report))
}

/// GET /health/ready - Readiness probe for Kubernetes
pub async fn readiness_check(
    State(service): State<SharedHealthService>,
) -> impl IntoResponse {
    let service = service.read().await;
    let report = service.check_health().await;
    let response = ReadinessResponse::from_health_report(&report);

    let status = if response.ready {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };

    (status, Json(response))
}

/// GET /health/live - Liveness probe for Kubernetes
pub async fn liveness_check() -> impl IntoResponse {
    let response = LivenessResponse::alive();
    (StatusCode::OK, Json(response))
}

/// GET /health/detailed - Detailed component health
pub async fn detailed_health(
    State(service): State<SharedHealthService>,
) -> impl IntoResponse {
    let service = service.read().await;
    let report = service.check_health().await;
    let status = if report.status.is_operational() {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (status, Json(report))
}

/// GET /health/db/pool - Database connection pool metrics
pub async fn db_pool_metrics(
    State(pool): State<sqlx::PgPool>,
) -> impl IntoResponse {
    let monitor = crate::db::health::DatabaseHealthMonitor::new(pool, 20, 5);
    let metrics = monitor.pool_metrics();
    (StatusCode::OK, Json(metrics))
}

/// GET /health/db/migrations - Database migration status
pub async fn db_migration_status(
    State(pool): State<sqlx::PgPool>,
) -> impl IntoResponse {
    let monitor = crate::db::health::DatabaseHealthMonitor::new(pool, 20, 5);
    match monitor.validate_migrations().await {
        Ok(result) => (StatusCode::OK, Json(serde_json::to_value(result).unwrap_or_default())),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

/// GET /health/db/backups - Database backup validation
pub async fn db_backup_status(
    State(pool): State<sqlx::PgPool>,
) -> impl IntoResponse {
    let monitor = crate::db::health::DatabaseHealthMonitor::new(pool, 20, 5);
    let result = monitor.validate_backups().await;
    (StatusCode::OK, Json(serde_json::to_value(result).unwrap_or_default()))
}
