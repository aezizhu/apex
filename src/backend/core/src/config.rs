//! Configuration management.

use serde::Deserialize;

/// Main application configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    /// Server configuration
    #[serde(default)]
    pub server: ServerConfig,

    /// Database configuration
    pub database: DatabaseConfig,

    /// Redis configuration
    #[serde(default)]
    pub redis: RedisConfig,

    /// Observability configuration
    #[serde(default)]
    pub observability: ObservabilityConfig,

    /// Orchestrator configuration
    #[serde(default)]
    pub orchestrator: OrchestratorConfig,

    /// LLM provider configurations
    #[serde(default)]
    pub llm: LlmConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    /// HTTP server host
    #[serde(default = "default_host")]
    pub host: String,

    /// HTTP server port
    #[serde(default = "default_port")]
    pub port: u16,

    /// gRPC server port
    #[serde(default = "default_grpc_port")]
    pub grpc_port: u16,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_port(),
            grpc_port: default_grpc_port(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseConfig {
    /// PostgreSQL connection URL
    pub url: String,

    /// Maximum number of connections
    #[serde(default = "default_max_connections")]
    pub max_connections: u32,

    /// Minimum number of connections
    #[serde(default = "default_min_connections")]
    pub min_connections: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RedisConfig {
    /// Redis connection URL
    #[serde(default = "default_redis_url")]
    pub url: String,

    /// Connection pool size
    #[serde(default = "default_redis_pool_size")]
    pub pool_size: u32,
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            url: default_redis_url(),
            pool_size: default_redis_pool_size(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ObservabilityConfig {
    /// OpenTelemetry OTLP endpoint
    pub otlp_endpoint: Option<String>,

    /// Log level
    #[serde(default = "default_log_level")]
    pub log_level: String,

    /// Enable JSON logging
    #[serde(default = "default_json_logging")]
    pub json_logging: bool,
}

impl Default for ObservabilityConfig {
    fn default() -> Self {
        Self {
            otlp_endpoint: None,
            log_level: default_log_level(),
            json_logging: default_json_logging(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct OrchestratorConfig {
    /// Maximum concurrent agents
    #[serde(default = "default_max_concurrent_agents")]
    pub max_concurrent_agents: usize,

    /// Enable FrugalGPT model routing
    #[serde(default = "default_enable_model_routing")]
    pub enable_model_routing: bool,

    /// Circuit breaker failure threshold
    #[serde(default = "default_circuit_breaker_threshold")]
    pub circuit_breaker_threshold: u32,

    /// Default token limit for tasks
    #[serde(default = "default_token_limit")]
    pub default_token_limit: u64,

    /// Default cost limit for tasks
    #[serde(default = "default_cost_limit")]
    pub default_cost_limit: f64,

    /// Default time limit in seconds
    #[serde(default = "default_time_limit")]
    pub default_time_limit: u64,
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self {
            max_concurrent_agents: default_max_concurrent_agents(),
            enable_model_routing: default_enable_model_routing(),
            circuit_breaker_threshold: default_circuit_breaker_threshold(),
            default_token_limit: default_token_limit(),
            default_cost_limit: default_cost_limit(),
            default_time_limit: default_time_limit(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct LlmConfig {
    /// OpenAI API key
    pub openai_api_key: Option<String>,

    /// Anthropic API key
    pub anthropic_api_key: Option<String>,

    /// Default model
    #[serde(default = "default_model")]
    pub default_model: String,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            openai_api_key: None,
            anthropic_api_key: None,
            default_model: default_model(),
        }
    }
}

// Default value functions
fn default_host() -> String { "0.0.0.0".to_string() }
fn default_port() -> u16 { 8080 }
fn default_grpc_port() -> u16 { 50051 }
fn default_max_connections() -> u32 { 20 }
fn default_min_connections() -> u32 { 5 }
fn default_redis_url() -> String { "redis://localhost:6379".to_string() }
fn default_redis_pool_size() -> u32 { 10 }
fn default_log_level() -> String { "info".to_string() }
fn default_json_logging() -> bool { true }
fn default_max_concurrent_agents() -> usize { 100 }
fn default_enable_model_routing() -> bool { true }
fn default_circuit_breaker_threshold() -> u32 { 5 }
fn default_token_limit() -> u64 { 20000 }
fn default_cost_limit() -> f64 { 0.25 }
fn default_time_limit() -> u64 { 300 }
fn default_model() -> String { "gpt-4o-mini".to_string() }

impl Config {
    /// Load configuration from environment and config files.
    pub fn load() -> anyhow::Result<Self> {
        let config = config::Config::builder()
            .add_source(config::Environment::with_prefix("APEX").separator("__"))
            .build()?;

        let cfg: Config = config.try_deserialize()?;
        Ok(cfg)
    }

    /// Load from a specific file path.
    pub fn from_file(path: &str) -> anyhow::Result<Self> {
        let config = config::Config::builder()
            .add_source(config::File::with_name(path))
            .add_source(config::Environment::with_prefix("APEX").separator("__"))
            .build()?;

        let cfg: Config = config.try_deserialize()?;
        Ok(cfg)
    }
}
