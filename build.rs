use std::env;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR")?;
    let proto_dir = PathBuf::from(&manifest_dir).join("proto");
    let schemas_dir = PathBuf::from(&manifest_dir).join("schemas");
    
    // Collect main proto files
    let mut main_proto_files = Vec::new();
    if proto_dir.exists() {
        main_proto_files = std::fs::read_dir(&proto_dir)?
            .filter_map(|entry| {
                let entry = entry.ok()?;
                let path = entry.path();
                if path.extension()? == "proto" {
                    Some(path)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
    }
    
    // Collect schema proto files  
    let mut schema_proto_files = Vec::new();
    if schemas_dir.exists() {
        schema_proto_files = std::fs::read_dir(&schemas_dir)?
            .filter_map(|entry| {
                let entry = entry.ok()?;
                let path = entry.path();
                if path.extension()? == "proto" {
                    Some(path)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
    }
    
    // Set up rerun conditions
    for proto_file in &main_proto_files {
        println!("cargo:rerun-if-changed={}", proto_file.display());
    }
    for proto_file in &schema_proto_files {
        println!("cargo:rerun-if-changed={}", proto_file.display());
    }
    println!("cargo:rerun-if-changed=proto/");
    println!("cargo:rerun-if-changed=schemas/");
    
    // Compile main proto files with namespace separation
    if !main_proto_files.is_empty() {
        let out_dir = env::var("OUT_DIR").unwrap();
        tonic_build::configure()
            .build_server(true)
            .build_client(true)
            .out_dir(&out_dir)
            .compile_well_known_types(true)
            // Configure enum generation to avoid conflicts
            .type_attribute("neural_trader.market_data.v1.DataType", "#[derive(serde::Serialize, serde::Deserialize)]")
            .type_attribute("neural_trader.common.v1.CommonErrorCode", "#[derive(serde::Serialize, serde::Deserialize)]")
            .type_attribute("neural_trader.models.v1.ModelType", "#[derive(serde::Serialize, serde::Deserialize)]")
            .type_attribute("neural_trader.models.v1.TrainingStage", "#[derive(serde::Serialize, serde::Deserialize)]")
            .type_attribute("neural_trader.models.v1.DeploymentStatus", "#[derive(serde::Serialize, serde::Deserialize)]")
            .type_attribute("neural_trader.trading.v1.OrderSide", "#[derive(serde::Serialize, serde::Deserialize)]")
            .type_attribute("neural_trader.trading.v1.OrderType", "#[derive(serde::Serialize, serde::Deserialize)]")
            .type_attribute("neural_trader.trading.v1.OrderState", "#[derive(serde::Serialize, serde::Deserialize)]")
            // Use fully qualified names for enum imports to avoid conflicts
            .extern_path(".google.protobuf", "::prost_types")
            .extern_path(".neural_trader.common.v1", "crate::proto::neural_trader::common::v1")
            .compile(&main_proto_files, &[&proto_dir])?;
    }
    
    // Compile schema proto files separately to avoid enum conflicts  
    if !schema_proto_files.is_empty() {
        // Create schemas output directory
        let out_dir = env::var("OUT_DIR").unwrap();
        let schemas_out = PathBuf::from(&out_dir).join("schemas");
        if !schemas_out.exists() {
            std::fs::create_dir_all(&schemas_out)?;
        }
        
        tonic_build::configure()
            .build_server(true)
            .build_client(true)
            .out_dir(&schemas_out)
            // Include protobuf include paths for well-known types
            .include_file("mod.rs")
            .compile_well_known_types(true)
            // Configure separate namespace for schemas to avoid conflicts with main proto enums
            .type_attribute("neural_trader.interfaces.ingestion.ValidationStatus", "#[derive(serde::Serialize, serde::Deserialize)]")
            .type_attribute("neural_trader.interfaces.mlops.SourceType", "#[derive(serde::Serialize, serde::Deserialize)]")
            .type_attribute("neural_trader.interfaces.mlops.Severity", "#[derive(serde::Serialize, serde::Deserialize)]")
            // Rename the conflicting DataType enum by adding namespace prefix
            .type_attribute("neural_trader.interfaces.mlops.FeatureDefinition.DataType", "#[allow(clippy::enum_variant_names)]")
            .type_attribute("neural_trader.interfaces.execution.DataType", "#[allow(clippy::enum_variant_names)]")
            // Enable well-known types for google.protobuf imports
            .extern_path(".google.protobuf", "::prost_types")
            .compile(&schema_proto_files, &[&schemas_dir])?;
    }
    
    if main_proto_files.is_empty() && schema_proto_files.is_empty() {
        println!("cargo:warning=No .proto files found in {:?} or {:?}", proto_dir, schemas_dir);
    }
    
    Ok(())
}