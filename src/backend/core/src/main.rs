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
    agents::{DelegationManager, DelegationStrategy},
    contracts::ResourceLimits,
    plugins::PluginRegistry,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load environment variables
    dotenvy::dotenv().ok();

    // Load configuration - fail if DATABASE_URL is not provided
    let config = match Config::load() {
        Ok(cfg) => {
            tracing::info!("Configuration loaded successfully");
            cfg
        }
        Err(e) => {
            // Check if DATABASE_URL is provided - it's required
            let db_url = match std::env::var("DATABASE_URL") {
                Ok(url) => url,
                Err(_) => {
                    return Err(anyhow::anyhow!(
                        "Failed to load configuration and DATABASE_URL is not set. \
                         Please provide configuration via config file or environment variables (APEX__*)."
                    ));
                }
            };
            // Log warning and use defaults with DATABASE_URL
            eprintln!("Warning: Could not load full config: {}. Using defaults with DATABASE_URL from environment.", e);
            tracing::warn!("Using default configuration due to config load failure: {}", e);
            Config {
                server: Default::default(),
                database: apex_core::config::DatabaseConfig {
                    url: db_url,
                    max_connections: 20,
                    min_connections: 5,
                },
                redis: Default::default(),
                observability: Default::default(),
                orchestrator: Default::default(),
                llm: Default::default(),
            }
        }
    };

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
    let db = Arc::new(Database::new(
        &config.database.url,
        config.database.max_connections,
        config.database.min_connections,
    ).await?);
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

    // Create Redis client and connection manager (with pooling)
    let redis_client = redis::Client::open(config.redis.url.as_str())
        .map_err(|e| anyhow::anyhow!("Failed to create Redis client: {}", e))?;
    tracing::info!("Redis client created for {}", config.redis.url);

    // Create connection manager with connection pooling
    let redis_manager = redis::aio::ConnectionManager::new(redis_client)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to create Redis connection manager: {}", e))?;
    tracing::info!("Redis connection manager created with pool");

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
        SwarmOrchestrator::new(orchestrator_config, db.clone(), redis_manager, tracer).await?
    );
    tracing::info!("Orchestrator initialized");

    // Create plugin registry
    let plugin_registry = Arc::new(PluginRegistry::new("./plugins"));
    tracing::info!("Plugin registry initialized");

    // Create delegation manager (shares the orchestrator's agent registry)
    let delegation_manager = Arc::new(DelegationManager::new(
        DelegationStrategy::LeastBusy,
        orchestrator.agents(),
    ));
    tracing::info!("Delegation manager initialized");

    // Create app state
    let app_state = AppState {
        orchestrator,
        db,
        plugin_registry,
        delegation_manager,
        allowed_origins: config.server.allowed_origins,
    };

    // Build router
    let app = api::build_router(app_state);

    // Start server
    let addr = SocketAddr::from(([0, 0, 0, 0], config.server.port));
    tracing::info!(address = %addr, "Starting HTTP server");

    let listener = tokio::net::TcpListener::bind(addr).await.map_err(|e| {
        if e.kind() == std::io::ErrorKind::AddrInUse {
            anyhow::anyhow!(
                "Port {} is already in use. Please either:\n\
                 1. Stop the other process using this port\n\
                 2. Use a different port by setting APEX__SERVER__PORT environment variable\n\
                 3. Check if another instance of this server is already running",
                config.server.port
            )
        } else {
            anyhow::anyhow!("Failed to bind to address {}: {}", addr, e)
        }
    })?;

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
