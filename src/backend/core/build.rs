//! Build script for Apex Core
//!
//! Compiles Protocol Buffer definitions using tonic-build to generate
//! Rust code for gRPC services.

use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Define the path to the proto file
    let proto_file = "proto/apex.proto";

    // Get the OUT_DIR where generated code will be placed
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());

    // Configure tonic-build
    let builder = tonic_build::configure()
        // Generate server code
        .build_server(true)
        // Generate client code (useful for testing and inter-service communication)
        .build_client(true)
        // Generate transport code
        .build_transport(true)
        // Enable compile_well_known_types if needed
        .compile_well_known_types(false)
        // Output directory
        .out_dir(&out_dir)
        // Add serde derives for all types for better Rust integration
        .type_attribute(".", "#[derive(serde::Serialize, serde::Deserialize)]")
        .type_attribute(".", "#[serde(rename_all = \"camelCase\")]");

    // Compile the proto file
    builder.compile(
        &[proto_file],
        &["proto/"], // Include path for imports
    )?;

    // Tell Cargo to rerun this script if the proto file changes
    println!("cargo:rerun-if-changed={}", proto_file);
    println!("cargo:rerun-if-changed=build.rs");

    // Print the output directory for debugging
    println!("cargo:warning=Proto files compiled to: {:?}", out_dir);

    Ok(())
}
