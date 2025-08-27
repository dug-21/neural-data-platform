//! Build script for data-staging service proto compilation
//! 
//! This script compiles Protocol Buffer files for EventEnvelope and market data.

use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:warning=Data-staging build script running");
    
    // Set up rerun triggers
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../schemas/ingestion-eventbus.proto");
    println!("cargo:rerun-if-changed=../proto/market_data.proto");
    println!("cargo:rerun-if-changed=../proto/common.proto");
    
    // Compile proto files for EventEnvelope and market data
    let proto_files = vec![
        "../schemas/ingestion-eventbus.proto",
        "../proto/market_data.proto", 
        "../proto/common.proto",
    ];
    
    let includes = vec!["../schemas", "../proto"];
    
    // Check if proto files exist
    let mut missing_files = Vec::new();
    for proto_file in &proto_files {
        if !Path::new(proto_file).exists() {
            missing_files.push(*proto_file);
        }
    }
    
    if !missing_files.is_empty() {
        return Err(format!("Proto files not found: {:?}", missing_files).into());
    }
    
    // Compile proto files
    println!("cargo:warning=Compiling protobuf files...");
    
    tonic_build::configure()
        .build_server(false)
        .build_client(false)
        .extern_path(".google.protobuf.Any", "::prost_types::Any")
        .extern_path(".google.protobuf.Timestamp", "::prost_types::Timestamp")
        .extern_path(".google.protobuf.Duration", "::prost_types::Duration")
        .compile(&proto_files, &includes)?;
    
    println!("cargo:warning=Proto compilation successful");
    println!("cargo:warning=Data-staging build completed");
    
    Ok(())
}