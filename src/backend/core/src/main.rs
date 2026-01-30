//! Apex Server - Main entry point
//!
//! The world's No. 1 Agent Swarm Orchestration Engine.

use std::sync::Arc;
use std::net::SocketAddr;

use apex_core::{
    config::Config,
    db::Database,
    db::health::DatabaseHealthMonitor,
    orchestrator::{SwarmOrchestrator, OrchestratorConfig},
    observability::{self, Tracer},
    api::{self, AppState},
    contracts::ResourceLimits,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load environment variables
    dotenvy::dotenv().ok();

    // Load configuration
    let config = Config::load().unwrap_or_else(|e| {
        eprintln!("Warning: Could not load config: {}. Using defaults.", e);
        Config {
            server: Default::default(),
            database: apex_core::config::DatabaseConfig {
                url: std::env::var("DATABASE_URL")
                    .unwrap_or_else(|_| "postgres://apex:apex_secret@localhost:5432/apex".to_string()),
                max_connections: 20,
                min_connections: 5,
            },
            redis: Default::default(),
            observability: Default::default(),
            orchestrator: Default::default(),
            llm: Default::default(),
        }
    });

    // Initialize observability
    observability::init(
        "apex-server",
        config.observability.otlp_endpoint.as_deref(),
    )?;

    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        "Starting Apex Server"
    );

    // Connect to database
    let db = Arc::new(Database::new(&config.database.url).await?);
    tracing::info!("Connected to database");

    // Create database health monitor and run startup validation
    let db_health_monitor = DatabaseHealthMonitor::new(
        db.pool().clone(),
        config.database.max_connections,
        config.database.min_connections,
    );
    db_health_monitor.startup_validation().await?;
    tracing::info!("Database startup validation passed (migrations applied, connectivity verified)");

    // Create tracer
    let tracer = Arc::new(Tracer::new("apex-server"));

    // Create Redis client
    let redis_client = redis::Client::open(config.redis.url.as_str())
        .map_err(|e| anyhow::anyhow!("Failed to create Redis client: {}", e))?;
    tracing::info!("Redis client created for {}", config.redis.url);

    // Create orchestrator
    let orchestrator_config = OrchestratorConfig {
        max_concurrent_agents: config.orchestrator.max_concurrent_agents,
        default_limits: ResourceLimits {
            token_limit: config.orchestrator.default_token_limit,
            cost_limit: config.orchestrator.default_cost_limit,
            api_call_limit: 100,
            time_limit_seconds: config.orchestrator.default_time_limit,
        },
        enable_model_routing: config.orchestrator.enable_model_routing,
        circuit_breaker_threshold: config.orchestrator.circuit_breaker_threshold,
        retry_delay_ms: 1000,
        task_result_timeout_secs: 300,
    };

    let orchestrator = Arc::new(
        SwarmOrchestrator::new(orchestrator_config, db.clone(), redis_client, tracer).await?
    );
    tracing::info!("Orchestrator initialized");

    // Create app state
    let app_state = AppState {
        orchestrator,
        db,
    };

    // Build router
    let app = api::build_router(app_state);

    // Start server
    let addr = SocketAddr::from(([0, 0, 0, 0], config.server.port));
    tracing::info!(address = %addr, "Starting HTTP server");

    let listener = tokio::net::TcpListener::bind(addr).await?;

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    // Cleanup
    observability::shutdown();
    tracing::info!("Server shutdown complete");

    Ok(())
}

/// Wait for shutdown signal.
async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("Failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    tracing::info!("Shutdown signal received");
}
