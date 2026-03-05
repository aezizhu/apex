//! Output formatting utilities for the Apex CLI.
//!
//! Supports table, JSON, and YAML output formats.

use clap::ValueEnum;
use colored::*;
use serde::Serialize;
use tabled::{
    settings::{object::Columns, Alignment, Modify, Style},
    Table, Tabled,
};

/// Output format selection.
#[derive(Debug, Clone, Copy, ValueEnum, Default)]
pub enum OutputFormat {
    /// Render as a formatted table
    #[default]
    Table,
    /// Render as JSON
    Json,
    /// Render as YAML
    Yaml,
}

/// Print a success message to stdout.
pub fn print_success(msg: &str) {
    println!("{} {}", "[OK]".green().bold(), msg);
}

/// Print an error message to stderr.
pub fn print_error(msg: &str) {
    eprintln!("{} {}", "[ERROR]".red().bold(), msg);
}

/// Print an informational message to stdout.
pub fn print_info(msg: &str) {
    println!("{} {}", "[INFO]".blue().bold(), msg);
}

/// Print a list of items in the requested format.
///
/// For table output, items must implement `Tabled`. For JSON/YAML, items must
/// implement `Serialize`.
pub fn print_list<T: Tabled + Serialize>(items: &[T], format: OutputFormat) {
    match format {
        OutputFormat::Table => {
            if items.is_empty() {
                println!("{}", "No results found.".dimmed());
                return;
            }
            let table = Table::new(items)
                .with(Style::rounded())
                .with(Modify::new(Columns::first()).with(Alignment::left()))
                .to_string();
            println!("{}", table);
        }
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(items).expect("serialize to JSON");
            println!("{}", json);
        }
        OutputFormat::Yaml => {
            let yaml = serde_yaml::to_string(items).expect("serialize to YAML");
            print!("{}", yaml);
        }
    }
}

/// Print a single item in the requested format.
pub fn print_item<T: Serialize>(item: &T, format: OutputFormat) {
    match format {
        OutputFormat::Table | OutputFormat::Json => {
            let json = serde_json::to_string_pretty(item).expect("serialize to JSON");
            println!("{}", json);
        }
        OutputFormat::Yaml => {
            let yaml = serde_yaml::to_string(item).expect("serialize to YAML");
            print!("{}", yaml);
        }
    }
}

/// Print key-value details to the terminal (non-JSON/YAML output).
pub fn print_detail(key: &str, value: &str) {
    println!("  {}: {}", key.cyan(), value);
}

/// Print a section header.
pub fn print_header(title: &str) {
    println!();
    println!("{}", title.bold().underline());
    println!();
}
