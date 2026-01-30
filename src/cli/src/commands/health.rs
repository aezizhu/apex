//! Health check command.
//!
//! Queries the `/health` endpoint and displays component status.

use anyhow::Result;
use clap::Args;

use crate::client::ApiClient;
use crate::output::{self, OutputFormat};

#[derive(Args)]
pub struct HealthArgs {
    /// Include detailed component checks
    #[arg(short, long)]
    detailed: bool,
}

pub async fn execute(args: HealthArgs, client: &ApiClient, format: OutputFormat) -> Result<()> {
    let health: serde_json::Value = client.get_raw("/health").await?;

    match format {
        OutputFormat::Table => {
            let status = health
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");

            output::print_header("System Health");
            output::print_detail("Status", status);
            output::print_detail("API URL", client.base_url());

            if let Some(version) = health.get("version").and_then(|v| v.as_str()) {
                output::print_detail("Version", version);
            }

            if let Some(ts) = health.get("timestamp").and_then(|v| v.as_str()) {
                output::print_detail("Timestamp", ts);
            }

            if args.detailed {
                if let Some(components) = health.get("components").and_then(|v| v.as_array()) {
                    println!();
                    output::print_header("Components");
                    for comp in components {
                        let name = comp
                            .get("name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("?");
                        let comp_status = comp
                            .get("status")
                            .and_then(|v| v.as_str())
                            .unwrap_or("?");
                        output::print_detail(name, comp_status);
                    }
                }
            }

            if status == "healthy" || status == "ok" {
                output::print_success("All systems operational");
            } else {
                output::print_error(&format!("System status: {}", status));
            }
        }
        _ => output::print_item(&health, format),
    }

    Ok(())
}
