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
