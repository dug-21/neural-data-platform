# Build.rs Configuration for Proto Compilation in Neural-Core EventBus

> ⚠️ **CRITICAL: PROTO COMPILATION IS MANDATORY**  
> This system **CANNOT** be built without Protocol Buffer support. Proto compilation is always enabled and build failures are intentional if proto files cannot be compiled.

## Overview

This document provides a comprehensive build.rs configuration for Protocol Buffers (protobuf) compilation in the neural-core EventBus system. Proto compilation is **MANDATORY** - the system cannot be built without proto support. The configuration supports proto files from the `/proto` and `/schemas` directories with proper organization and CI/CD integration.

### Key Requirements:
- ✅ Protocol Buffer compiler (`protoc`) **MUST** be installed
- ✅ All proto files **MUST** compile successfully  
- ✅ No feature flags - proto is always compiled
- ✅ Build fails intentionally if proto compilation fails
- ❌ No graceful degradation without proto support
- ❌ No stub files or fallback implementations

## Table of Contents

1. [Complete build.rs Script](#complete-buildrs-script)
2. [Proto File Structure](#proto-file-structure)
3. [Mandatory Proto Configuration](#mandatory-proto-configuration)
4. [Generated Code Organization](#generated-code-organization)
5. [Custom Derives and Attributes](#custom-derives-and-attributes)
6. [Error Handling](#error-handling)
7. [Incremental Compilation Optimization](#incremental-compilation-optimization)
8. [CI/CD Integration](#cicd-integration)
9. [Development Workflow](#development-workflow)
10. [Troubleshooting](#troubleshooting)

## Complete build.rs Script

### Primary build.rs for neural-core

```rust
//! Build script for neural-core proto compilation
//! 
//! This script compiles Protocol Buffer files from both `/proto` and `/schemas`
//! directories, organizing generated code with proper feature flags and 
//! incremental compilation support.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug)]
struct ProtoConfig {
    proto_path: PathBuf,
    output_path: PathBuf,
    package_name: String,
    feature_flag: Option<String>,
    additional_attributes: Vec<String>,
    custom_derives: Vec<String>,
}

impl ProtoConfig {
    fn new(proto_path: impl Into<PathBuf>, package_name: impl Into<String>) -> Self {
        Self {
            proto_path: proto_path.into(),
            output_path: PathBuf::from("src/generated"),
            package_name: package_name.into(),
            feature_flag: None,
            additional_attributes: Vec::new(),
            custom_derives: Vec::new(),
        }
    }

    fn with_feature_flag(mut self, flag: impl Into<String>) -> Self {
        self.feature_flag = Some(flag.into());
        self
    }

    fn with_custom_derives(mut self, derives: Vec<String>) -> Self {
        self.custom_derives = derives;
        self
    }

    fn with_attributes(mut self, attrs: Vec<String>) -> Self {
        self.additional_attributes = attrs;
        self
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Set up environment
    setup_environment()?;

    // Proto compilation is MANDATORY - build fails without it
    println!("cargo:warning=Compiling Protocol Buffers - REQUIRED for build");
    
    // Ensure protoc is available - fail build if not
    if !check_protoc_available() {
        return Err("Protocol Buffer compiler (protoc) is required but not found. Install with: apt-get install protobuf-compiler".into());
    }
    
    // Configure proto compilation
    let configs = get_proto_configurations()?;
    
    // Compile all proto configurations - any failure fails the build
    for config in configs {
        compile_proto_config(&config)
            .map_err(|e| format!("FATAL: Proto compilation failed for {}: {}. Build cannot continue.", config.package_name, e))?;
    }

    // Generate module declarations
    generate_module_declarations()?;
    
    // Verify compilation success - fail build if verification fails
    verify_compilation_success()
        .map_err(|e| format!("FATAL: Proto validation failed: {}. Build cannot continue.", e))?;
    
    println!("cargo:warning=Protocol Buffer compilation completed successfully");

    Ok(())
}

fn setup_environment() -> Result<(), Box<dyn std::error::Error>> {
    // Set up rerun-if-changed for all proto files
    let workspace_root = get_workspace_root()?;
    
    // Watch proto directories
    let proto_dirs = [
        workspace_root.join("proto"),
        workspace_root.join("schemas"),
    ];

    for proto_dir in &proto_dirs {
        if proto_dir.exists() {
            watch_directory_recursively(proto_dir)?;
        }
    }

    // Watch build script itself
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=Cargo.toml");
    
    // Watch for environment changes
    println!("cargo:rerun-if-env-changed=PROTOC");
    println!("cargo:rerun-if-env-changed=PROTOC_INCLUDE");

    Ok(())
}

fn get_workspace_root() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR")?;
    let mut path = PathBuf::from(manifest_dir);
    
    // Navigate up to find workspace root
    while !path.join("Cargo.toml").exists() || path.file_name() != Some("neural-trader".as_ref()) {
        if !path.pop() {
            return Err("Could not find workspace root".into());
        }
    }
    
    Ok(path)
}

fn watch_directory_recursively(dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    if dir.is_dir() {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.is_dir() {
                watch_directory_recursively(&path)?;
            } else if path.extension().and_then(|s| s.to_str()) == Some("proto") {
                println!("cargo:rerun-if-changed={}", path.display());
            }
        }
    }
    Ok(())
}

fn get_proto_configurations() -> Result<Vec<ProtoConfig>, Box<dyn std::error::Error>> {
    let workspace_root = get_workspace_root()?;
    
    let mut configs = Vec::new();

    // Core proto files from /proto directory
    configs.extend(vec![
        ProtoConfig::new(workspace_root.join("proto/common.proto"), "common")
            .with_custom_derives(vec![
                "serde::Serialize".to_string(),
                "serde::Deserialize".to_string(),
                "Clone".to_string(),
                "Debug".to_string(),
                "PartialEq".to_string(),
            ])
            .with_attributes(vec![
                "#[serde(rename_all = \"camelCase\")]".to_string(),
            ]),

        ProtoConfig::new(workspace_root.join("proto/market_data.proto"), "market_data")
            .with_custom_derives(vec![
                "serde::Serialize".to_string(),
                "serde::Deserialize".to_string(),
                "Clone".to_string(),
                "Debug".to_string(),
                "PartialEq".to_string(),
            ]),

        ProtoConfig::new(workspace_root.join("proto/trading.proto"), "trading")
            .with_custom_derives(vec![
                "serde::Serialize".to_string(),
                "serde::Deserialize".to_string(),
                "Clone".to_string(),
                "Debug".to_string(),
                "PartialEq".to_string(),
            ]),

        ProtoConfig::new(workspace_root.join("proto/features.proto"), "features")
            .with_custom_derives(vec![
                "serde::Serialize".to_string(),
                "serde::Deserialize".to_string(),
                "Clone".to_string(),
                "Debug".to_string(),
            ]),

        ProtoConfig::new(workspace_root.join("proto/models.proto"), "models")
            .with_custom_derives(vec![
                "serde::Serialize".to_string(),
                "serde::Deserialize".to_string(),
                "Clone".to_string(),
                "Debug".to_string(),
            ]),

        ProtoConfig::new(workspace_root.join("proto/config_store.proto"), "config_store")
            .with_custom_derives(vec![
                "serde::Serialize".to_string(),
                "serde::Deserialize".to_string(),
                "Clone".to_string(),
                "Debug".to_string(),
            ]),
    ]);

    // EventBus interface schemas from /schemas directory
    configs.extend(vec![
        ProtoConfig::new(workspace_root.join("schemas/ingestion-eventbus.proto"), "eventbus_ingestion")
            .with_custom_derives(vec![
                "serde::Serialize".to_string(),
                "serde::Deserialize".to_string(),
                "Clone".to_string(),
                "Debug".to_string(),
                "PartialEq".to_string(),
                "Eq".to_string(),
                "Hash".to_string(),
            ])
            .with_attributes(vec![
                "#[serde(rename_all = \"camelCase\")]".to_string(),
            ]),

        ProtoConfig::new(workspace_root.join("schemas/eventbus-mlops.proto"), "eventbus_mlops")
            .with_custom_derives(vec![
                "serde::Serialize".to_string(),
                "serde::Deserialize".to_string(),
                "Clone".to_string(),
                "Debug".to_string(),
            ]),

        ProtoConfig::new(workspace_root.join("schemas/mlops-execution.proto"), "mlops_execution")
            .with_custom_derives(vec![
                "serde::Serialize".to_string(),
                "serde::Deserialize".to_string(),
                "Clone".to_string(),
                "Debug".to_string(),
            ]),

        ProtoConfig::new(workspace_root.join("schemas/execution-action.proto"), "execution_action")
            .with_custom_derives(vec![
                "serde::Serialize".to_string(),
                "serde::Deserialize".to_string(),
                "Clone".to_string(),
                "Debug".to_string(),
            ]),
    ]);

    // Filter configs based on file existence
    configs.retain(|config| config.proto_path.exists());

    if configs.is_empty() {
        return Err("FATAL: No valid proto files found. Proto files are required for build.".into());
    }

    Ok(configs)
}

fn compile_proto_config(config: &ProtoConfig) -> Result<(), Box<dyn std::error::Error>> {
    let workspace_root = get_workspace_root()?;
    
    // Create output directory
    let output_dir = PathBuf::from(env::var("OUT_DIR")?);
    let generated_dir = output_dir.join("generated");
    fs::create_dir_all(&generated_dir)?;

    // Set up tonic-build configuration
    let mut tonic_build = tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .out_dir(&generated_dir)
        .type_attribute(".", "#[derive(Clone, PartialEq)]")
        .extern_path(".google.protobuf.Any", "::prost_types::Any")
        .extern_path(".google.protobuf.Timestamp", "::prost_types::Timestamp")
        .extern_path(".google.protobuf.Duration", "::prost_types::Duration")
        .extern_path(".google.protobuf.Empty", "::prost_types::Empty")
        .extern_path(".google.protobuf.Value", "::prost_types::Value")
        .extern_path(".google.protobuf.Struct", "::prost_types::Struct");

    // Add custom derives
    for derive in &config.custom_derives {
        tonic_build = tonic_build.type_attribute(".", format!("#[derive({})]", derive));
    }

    // Add additional attributes
    for attr in &config.additional_attributes {
        tonic_build = tonic_build.type_attribute(".", attr);
    }

    // Add serde annotations for JSON compatibility
    tonic_build = tonic_build
        .type_attribute(".", "#[serde(rename_all = \"camelCase\")]")
        .field_attribute(".", "#[serde(skip_serializing_if = \"Option::is_none\")]");

    // Set include paths
    let include_paths = vec![
        workspace_root.join("proto"),
        workspace_root.join("schemas"),
        workspace_root.clone(),
    ];

    // Compile the proto file
    println!("cargo:warning=Compiling proto: {}", config.proto_path.display());
    
    tonic_build
        .compile(&[&config.proto_path], &include_paths)
        .map_err(|e| format!("Failed to compile {}: {}", config.proto_path.display(), e))?;

    // Create module file in src/generated
    let src_generated_dir = PathBuf::from("src/generated");
    fs::create_dir_all(&src_generated_dir)?;
    
    let module_content = generate_module_content(config)?;
    let module_file = src_generated_dir.join(format!("{}.rs", config.package_name));
    
    fs::write(&module_file, module_content)?;
    
    println!("cargo:warning=Generated module: {}", module_file.display());

    Ok(())
}

fn generate_module_content(config: &ProtoConfig) -> Result<String, Box<dyn std::error::Error>> {
    let out_dir = env::var("OUT_DIR")?;
    let generated_files = find_generated_files(&PathBuf::from(out_dir).join("generated"))?;
    
    let mut content = String::new();
    
    // Proto is always compiled - no feature gates
    
    content.push_str("// Generated Protocol Buffer code\n");
    content.push_str("// This file is automatically generated by build.rs\n\n");
    
    // Include generated files
    for file in generated_files {
        if let Some(stem) = file.file_stem().and_then(|s| s.to_str()) {
            content.push_str(&format!("include!(concat!(env!(\"OUT_DIR\"), \"/generated/{}.rs\"));\n", stem));
        }
    }

    // Add re-exports for common types
    content.push_str("\n// Re-exports for convenience\n");
    content.push_str("pub use prost_types::*;\n");
    
    // Always include tonic since proto is mandatory
    content.push_str("pub use tonic::*;\n");

    Ok(content)
}

fn find_generated_files(dir: &Path) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    let mut files = Vec::new();
    
    if dir.exists() && dir.is_dir() {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("rs") {
                files.push(path);
            }
        }
    }
    
    Ok(files)
}

fn generate_module_declarations() -> Result<(), Box<dyn std::error::Error>> {
    let generated_dir = PathBuf::from("src/generated");
    let mod_file = generated_dir.join("mod.rs");
    
    let mut content = String::new();
    content.push_str("// Auto-generated module declarations\n");
    content.push_str("// This file is automatically generated by build.rs\n\n");
    
    // Proto is always available - no conditional compilation
    content.push_str("pub mod proto {\n");
    
    // Add module declarations for all generated files
    for entry in fs::read_dir(&generated_dir)? {
        let entry = entry?;
        let path = entry.path();
        
        if path.is_file() 
            && path.extension().and_then(|s| s.to_str()) == Some("rs")
            && path.file_name() != Some("mod.rs".as_ref()) {
            
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                content.push_str(&format!("    pub mod {};\n", stem));
            }
        }
    }
    
    content.push_str("}\n");
    
    fs::write(&mod_file, content)?;
    
    Ok(())
}

// Stub files are not created - proto is mandatory
// fn create_stub_files() removed - proto compilation always required

fn verify_compilation_success() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = PathBuf::from(env::var("OUT_DIR")?);
    let generated_dir = out_dir.join("generated");
    
    if !generated_dir.exists() {
        return Err("Generated directory does not exist".into());
    }
    
    let files = find_generated_files(&generated_dir)?;
    if files.is_empty() {
        return Err("No files were generated".into());
    }
    
    println!("cargo:warning=Verification complete: {} files generated", files.len());
    
    Ok(())
}

// Helper function to check if protoc is available
fn check_protoc_available() -> bool {
    Command::new("protoc")
        .arg("--version")
        .output()
        .is_ok()
}

// Development mode helper
fn is_development_mode() -> bool {
    env::var("CARGO_CFG_DEBUG_ASSERTIONS").is_ok()
}

#[cfg(test)]
mod build_tests {
    use super::*;
    
    #[test]
    fn test_proto_config_creation() {
        let config = ProtoConfig::new("test.proto", "test_package");
        assert_eq!(config.package_name, "test_package");
        assert_eq!(config.proto_path, PathBuf::from("test.proto"));
    }
    
    #[test]
    fn test_workspace_root_detection() {
        // This test would need to be run in the actual workspace
        // Implementation depends on your specific setup
    }
}
```

## Proto File Structure

### Current Proto Files Analysis

Based on the examination of existing proto files, here's the organized structure:

```
/workspace/neural-trader/
├── proto/                          # Core system protos
│   ├── common.proto                # Common types and enums
│   ├── market_data.proto           # Market data service definitions
│   ├── trading.proto               # Trading service definitions
│   ├── features.proto              # Feature extraction
│   ├── models.proto                # ML model definitions
│   └── config_store.proto          # Configuration management
└── schemas/                        # EventBus interface schemas
    ├── ingestion-eventbus.proto    # Data ingestion interface
    ├── eventbus-mlops.proto        # ML Ops integration
    ├── mlops-execution.proto       # Model execution interface
    └── execution-action.proto      # Action execution interface
```

### Generated Code Structure

```
src/generated/
├── mod.rs                          # Module declarations with feature gates
├── common.rs                       # Common types (always generated)
├── market_data.rs                  # Market data types and services
├── trading.rs                      # Trading types and services
├── features.rs                     # Feature extraction types
├── models.rs                       # ML model types
├── config_store.rs                 # Configuration types
├── eventbus_ingestion.rs           # EventBus ingestion interface
├── eventbus_mlops.rs              # EventBus ML Ops interface
├── mlops_execution.rs             # ML Ops execution interface
└── execution_action.rs            # Action execution interface
```

## Mandatory Proto Configuration

### Cargo.toml Configuration

```toml
# Proto dependencies are MANDATORY - no feature flags
[dependencies]
# Core dependencies (always required)
serde = { version = "1.0", features = ["derive"] }
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1.0", features = ["v4", "serde"] }

# Proto dependencies (REQUIRED - not optional)
tonic = { version = "0.10", features = ["tls"] }
prost = { version = "0.12" }
prost-types = { version = "0.12" }

# Additional dependencies
serde_json = { version = "1.0" }
tracing-subscriber = { version = "0.3", optional = true }

[build-dependencies]
tonic-build = "0.10"

# No features for proto - it's always compiled
[features]
default = []
development = ["tracing-subscriber"]
```

### Mandatory Proto Code Organization

```rust
// src/generated/mod.rs
// Proto is always available - no feature gates
pub mod proto {
    pub mod common;
    pub mod market_data;
    pub mod trading;
    pub mod features;
    pub mod models;
    pub mod config_store;
    pub mod eventbus_ingestion;
    pub mod eventbus_mlops;
    pub mod mlops_execution;
    pub mod execution_action;
    
    // Re-export common types
    pub use common::*;
    pub use tonic;
    pub use prost_types;
}

// Extended functionality always available
pub mod extended {
    pub use super::proto::*;
    pub use serde_json;
    pub use uuid;
}
```

## Generated Code Organization

### Module Structure

```rust
// src/lib.rs
pub mod eventbus;
pub mod events;
pub mod interfaces;
pub mod types;
pub mod traits;

// Proto is always compiled
pub mod generated;
pub use generated::proto;

// EventBus integration with proto types - always available
impl crate::eventbus::Event for proto::eventbus_ingestion::EventEnvelope {
    fn event_id(&self) -> &str {
        &self.message_id
    }
    
    fn event_type(&self) -> &str {
        &self.event_type
    }
    
    fn timestamp(&self) -> chrono::DateTime<chrono::Utc> {
        self.created_at
            .as_ref()
            .map(|ts| chrono::DateTime::from_timestamp(ts.seconds, ts.nanos as u32))
            .flatten()
            .unwrap_or_else(chrono::Utc::now)
    }
}
```

### Service Integration

```rust
// src/interfaces/grpc_traits.rs
// Proto and tonic always available
use tonic::{Request, Response, Status};

pub trait GrpcServiceIntegration {
    type ProtoService;
    type EventBusService;
    
    fn integrate_service(
        eventbus: Self::EventBusService,
    ) -> Self::ProtoService;
}

// Always implement since proto is mandatory
impl GrpcServiceIntegration for MarketDataService {
    type ProtoService = proto::market_data::market_data_service_server::MarketDataServiceServer<Self>;
    type EventBusService = crate::eventbus::EventBus;
    
    fn integrate_service(eventbus: Self::EventBusService) -> Self::ProtoService {
        proto::market_data::market_data_service_server::MarketDataServiceServer::new(
            MarketDataServiceImpl::new(eventbus)
        )
    }
}
```

## Custom Derives and Attributes

### Standard Derives Configuration

```rust
// In build.rs tonic_build configuration
.type_attribute(".", "#[derive(Clone, Debug, PartialEq)]")
.type_attribute(".", "#[derive(serde::Serialize, serde::Deserialize)]")
.type_attribute(".", "#[serde(rename_all = \"camelCase\")]")
.field_attribute(".", "#[serde(skip_serializing_if = \"Option::is_none\")]")

// EventBus-specific attributes
.type_attribute(".neural_trader.interfaces.ingestion.EventEnvelope", 
    "#[derive(Eq, Hash)]")
.type_attribute(".neural_trader.interfaces.ingestion.EventEnvelope",
    r#"#[serde(tag = "eventType")]"#)

// Performance optimizations
.type_attribute(".", "#[derive(Copy)]") // For simple enums
.message_attribute(".", "#[derive(Default)]") // For message types
```

### Custom Implementations

```rust
// Custom trait implementations for EventBus integration
impl<T> From<proto::common::TimeWindow> for crate::types::TimeRange<T> 
where 
    T: chrono::TimeZone,
{
    fn from(window: proto::common::TimeWindow) -> Self {
        Self {
            start: window.start_time.map(|ts| ts.into()),
            end: window.end_time.map(|ts| ts.into()),
        }
    }
}

// Custom error handling
impl From<proto::common::CommonError> for crate::errors::EventBusError {
    fn from(proto_error: proto::common::CommonError) -> Self {
        match proto_error.code() {
            proto::common::CommonErrorCode::InvalidRequest => 
                Self::InvalidRequest(proto_error.message),
            proto::common::CommonErrorCode::ServiceUnavailable => 
                Self::ServiceUnavailable,
            _ => Self::Internal(proto_error.message),
        }
    }
}
```

## Error Handling

### Build Script Error Handling

```rust
// Enhanced error handling in build.rs
#[derive(Debug)]
enum BuildError {
    ProtoNotFound(PathBuf),
    CompilationFailed(String),
    OutputError(String),
    EnvironmentError(String),
}

impl std::fmt::Display for BuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BuildError::ProtoNotFound(path) => 
                write!(f, "Proto file not found: {}", path.display()),
            BuildError::CompilationFailed(msg) => 
                write!(f, "Proto compilation failed: {}", msg),
            BuildError::OutputError(msg) => 
                write!(f, "Output generation failed: {}", msg),
            BuildError::EnvironmentError(msg) => 
                write!(f, "Environment error: {}", msg),
        }
    }
}

impl std::error::Error for BuildError {}

// Detailed error reporting
fn handle_compilation_error(error: tonic_build::Error, proto_file: &Path) -> BuildError {
    eprintln!("Proto compilation failed for: {}", proto_file.display());
    eprintln!("Error details: {}", error);
    
    // Check common issues
    if error.to_string().contains("protoc") {
        eprintln!("Hint: Make sure protoc is installed and in PATH");
        eprintln!("Install with: apt-get install protobuf-compiler");
    }
    
    if error.to_string().contains("import") {
        eprintln!("Hint: Check import paths in proto files");
        eprintln!("Include paths: proto/, schemas/");
    }
    
    BuildError::CompilationFailed(error.to_string())
}
```

### Runtime Error Integration

```rust
// src/generated/error_conversion.rs
// Proto error handling always available
impl From<tonic::Status> for crate::errors::EventBusError {
    fn from(status: tonic::Status) -> Self {
        match status.code() {
            tonic::Code::NotFound => Self::NotFound,
            tonic::Code::InvalidArgument => Self::InvalidRequest(status.message().to_string()),
            tonic::Code::Unavailable => Self::ServiceUnavailable,
            tonic::Code::DeadlineExceeded => Self::Timeout,
            _ => Self::Internal(format!("gRPC error: {}", status.message())),
        }
    }
}

// Error propagation helpers - always available
pub trait TonicResultExt<T> {
    fn into_eventbus_result(self) -> Result<T, crate::errors::EventBusError>;
}

// Always implement since proto is mandatory
impl<T> TonicResultExt<T> for Result<T, tonic::Status> {
    fn into_eventbus_result(self) -> Result<T, crate::errors::EventBusError> {
        self.map_err(|e| e.into())
    }
}
```

## Incremental Compilation Optimization

### Dependency Tracking

```rust
// Enhanced dependency tracking in build.rs
fn setup_incremental_compilation() -> Result<(), Box<dyn std::error::Error>> {
    let workspace_root = get_workspace_root()?;
    
    // Track proto files with checksums
    let proto_manifest = generate_proto_manifest(&workspace_root)?;
    let manifest_path = PathBuf::from(env::var("OUT_DIR")?)
        .join("proto_manifest.json");
    
    // Check if recompilation is needed
    let needs_recompilation = if manifest_path.exists() {
        let old_manifest: ProtoManifest = serde_json::from_slice(
            &fs::read(&manifest_path)?
        )?;
        old_manifest != proto_manifest
    } else {
        true
    };
    
    if needs_recompilation {
        fs::write(&manifest_path, serde_json::to_vec_pretty(&proto_manifest)?)?;
        env::set_var("FORCE_PROTO_COMPILATION", "1");
    }
    
    Ok(())
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct ProtoManifest {
    files: HashMap<PathBuf, ProtoFileInfo>,
    build_timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct ProtoFileInfo {
    checksum: String,
    modified: chrono::DateTime<chrono::Utc>,
    dependencies: Vec<PathBuf>,
}

fn generate_proto_manifest(workspace_root: &Path) -> Result<ProtoManifest, Box<dyn std::error::Error>> {
    let mut files = HashMap::new();
    
    for proto_path in find_all_proto_files(workspace_root)? {
        let info = ProtoFileInfo {
            checksum: calculate_file_checksum(&proto_path)?,
            modified: get_file_modified_time(&proto_path)?,
            dependencies: parse_proto_dependencies(&proto_path)?,
        };
        files.insert(proto_path, info);
    }
    
    Ok(ProtoManifest {
        files,
        build_timestamp: chrono::Utc::now(),
    })
}
```

### Conditional Compilation

```rust
// Conditional compilation based on changes
fn should_compile_proto(config: &ProtoConfig) -> Result<bool, Box<dyn std::error::Error>> {
    // Proto compilation is ALWAYS required
    // No conditional compilation based on environment
    // Always return true since proto is mandatory
    return Ok(true);
    
    // Check if proto file or dependencies changed
    let proto_modified = get_file_modified_time(&config.proto_path)?;
    let output_file = get_expected_output_file(config)?;
    
    if !output_file.exists() {
        return Ok(true);
    }
    
    let output_modified = get_file_modified_time(&output_file)?;
    
    // Compile if proto is newer than output
    Ok(proto_modified > output_modified)
}

// Parallel compilation for multiple proto files
fn compile_protos_parallel(configs: &[ProtoConfig]) -> Result<(), Box<dyn std::error::Error>> {
    use std::thread;
    use std::sync::mpsc;
    
    let (tx, rx) = mpsc::channel();
    let mut handles = Vec::new();
    
    for config in configs {
        if should_compile_proto(config)? {
            let config = config.clone();
            let tx = tx.clone();
            
            let handle = thread::spawn(move || {
                let result = compile_proto_config(&config);
                tx.send((config.package_name.clone(), result)).unwrap();
            });
            
            handles.push(handle);
        }
    }
    
    drop(tx);
    
    // Collect results
    let mut results = Vec::new();
    for result in rx {
        results.push(result);
    }
    
    // Wait for all threads
    for handle in handles {
        handle.join().unwrap();
    }
    
    // Check for errors
    for (package, result) in results {
        if let Err(e) = result {
            return Err(format!("Failed to compile {}: {}", package, e).into());
        }
    }
    
    Ok(())
}
```

## CI/CD Integration

### GitHub Actions Configuration

```yaml
# .github/workflows/proto-validation.yml
name: Protocol Buffer Validation

on:
  push:
    paths:
      - 'proto/**/*.proto'
      - 'schemas/**/*.proto'
      - 'neural-core/build.rs'
      - 'neural-core/Cargo.toml'
  pull_request:
    paths:
      - 'proto/**/*.proto'
      - 'schemas/**/*.proto'
      - 'neural-core/build.rs'
      - 'neural-core/Cargo.toml'

jobs:
  proto-validation:
    runs-on: ubuntu-latest
    
    steps:
    - uses: actions/checkout@v4
    
    - name: Install Protocol Buffer Compiler
      run: |
        sudo apt-get update
        sudo apt-get install -y protobuf-compiler
        protoc --version
    
    - name: Install Rust
      uses: actions-rs/toolchain@v1
      with:
        toolchain: stable
        components: rustfmt, clippy
        override: true
    
    - name: Cache Dependencies
      uses: actions/cache@v3
      with:
        path: |
          ~/.cargo/registry
          ~/.cargo/git
          neural-core/target
        key: ${{ runner.os }}-cargo-proto-${{ hashFiles('**/Cargo.lock') }}
    
    - name: Validate Proto Files
      run: |
        cd neural-core
        # Validate all proto files can be compiled
        find ../proto ../schemas -name "*.proto" -exec protoc --proto_path=../proto --proto_path=../schemas --include_imports --descriptor_set_out=/dev/null {} \;
    
    - name: Build with Proto (MANDATORY)
      run: |
        cd neural-core
        cargo build
        # Note: Proto is always compiled, no feature flags
    
    - name: Run Tests
      run: |
        cd neural-core
        cargo test
        # Proto is always available for tests
    
    - name: Check Generated Code Formatting
      run: |
        cd neural-core
        cargo build --features=grpc
        # Format check would go here if we format generated code
    
    - name: Lint Generated Code
      run: |
        cd neural-core
        cargo clippy --features=grpc -- -D warnings
```

### Docker Support

```dockerfile
# Dockerfile.proto-builder
FROM rust:1.75 AS proto-builder

# Install protoc
RUN apt-get update && \
    apt-get install -y protobuf-compiler && \
    rm -rf /var/lib/apt/lists/*

# Verify installation
RUN protoc --version

# Set working directory
WORKDIR /workspace

# Copy proto files
COPY proto/ proto/
COPY schemas/ schemas/

# Copy neural-core
COPY neural-core/ neural-core/

# Build with proto compilation (mandatory)
WORKDIR /workspace/neural-core
RUN cargo build --release
# Proto is always compiled - no feature flags needed

# Create runtime image
FROM debian:bookworm-slim AS runtime

# Install runtime dependencies
RUN apt-get update && \
    apt-get install -y ca-certificates && \
    rm -rf /var/lib/apt/lists/*

# Copy compiled binary
COPY --from=proto-builder /workspace/neural-core/target/release/neural-core /usr/local/bin/

CMD ["neural-core"]
```

### Build Optimization Script

```bash
#!/bin/bash
# scripts/optimize-proto-build.sh

set -e

WORKSPACE_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
NEURAL_CORE_DIR="$WORKSPACE_ROOT/neural-core"

echo "🚀 Optimizing Protocol Buffer build..."

# Check if protoc is available
if ! command -v protoc &> /dev/null; then
    echo "❌ protoc not found. Installing..."
    if [[ "$OSTYPE" == "darwin"* ]]; then
        brew install protobuf
    elif [[ "$OSTYPE" == "linux-gnu"* ]]; then
        sudo apt-get update && sudo apt-get install -y protobuf-compiler
    else
        echo "Please install protoc manually"
        exit 1
    fi
fi

# Verify proto files
echo "🔍 Validating proto files..."
find "$WORKSPACE_ROOT/proto" "$WORKSPACE_ROOT/schemas" -name "*.proto" | while read -r proto_file; do
    echo "Validating: $proto_file"
    protoc \
        --proto_path="$WORKSPACE_ROOT/proto" \
        --proto_path="$WORKSPACE_ROOT/schemas" \
        --include_imports \
        --descriptor_set_out=/dev/null \
        "$proto_file"
done

# Clean previous builds
echo "🧹 Cleaning previous builds..."
cd "$NEURAL_CORE_DIR"
cargo clean

# Build with optimizations
echo "🔨 Building with proto compilation..."
RUSTFLAGS="-C target-cpu=native" cargo build --release --features=grpc

# Verify generated code
echo "✅ Verifying generated code..."
if [ -d "src/generated" ]; then
    echo "Generated files:"
    find src/generated -name "*.rs" -exec basename {} \;
else
    echo "❌ No generated code found!"
    exit 1
fi

echo "✅ Protocol Buffer build optimization complete!"
```

## Development Workflow

### Development Commands

```bash
# Build with proto compilation (MANDATORY)
cargo build
# Note: Proto is always compiled, cannot build without it

# Force proto recompilation
FORCE_PROTO_COMPILATION=1 cargo build

# Clean generated files
cargo clean
rm -rf src/generated

# Validate proto files only
find proto schemas -name "*.proto" -exec protoc --proto_path=proto --proto_path=schemas --include_imports --descriptor_set_out=/dev/null {} \;

# Generate proto documentation
protoc --proto_path=proto --proto_path=schemas --doc_out=docs/proto --doc_opt=html,index.html proto/*.proto schemas/*.proto
```

### IDE Integration

```json
// .vscode/settings.json
{
    "rust-analyzer.cargo.allFeatures": false,
    // No specific features needed since proto is always compiled
    "files.associations": {
        "*.proto": "proto3"
    },
    "protoc": {
        "path": "/usr/bin/protoc",
        "options": [
            "--proto_path=proto",
            "--proto_path=schemas"
        ]
    }
}
```

## Troubleshooting

### Common Issues and Solutions

#### 1. Proto Compilation Fails

```bash
# Check protoc installation
protoc --version

# Check include paths
find proto schemas -name "*.proto" | head -5 | xargs -I {} protoc --proto_path=proto --proto_path=schemas --include_imports --descriptor_set_out=/dev/null {}

# Enable verbose output
RUST_LOG=trace cargo build --features=grpc
```

#### 2. Generated Code Not Found

```rust
// Add to build.rs for debugging
fn debug_generated_files() {
    let out_dir = env::var("OUT_DIR").unwrap();
    println!("cargo:warning=OUT_DIR: {}", out_dir);
    
    if let Ok(entries) = fs::read_dir(format!("{}/generated", out_dir)) {
        for entry in entries {
            if let Ok(entry) = entry {
                println!("cargo:warning=Generated file: {:?}", entry.path());
            }
        }
    }
}
```

#### 3. Feature Flag Issues

```rust
// Debug proto compilation in build.rs
fn debug_proto_status() {
    println!("cargo:warning=Proto compilation: ALWAYS ENABLED (mandatory)");
    println!("cargo:warning=- tonic: YES (required)");
    println!("cargo:warning=- prost: YES (required)");
    println!("cargo:warning=- protoc available: {}", check_protoc_available());
}
```

#### 4. Import Path Issues

```proto
// Correct import paths in proto files
syntax = "proto3";

// Use relative paths from proto root
import "common.proto";  // ✅ Correct
import "google/protobuf/timestamp.proto";  // ✅ Correct

// Avoid absolute paths
import "/proto/common.proto";  // ❌ Wrong
import "../../proto/common.proto";  // ❌ Wrong
```

### Build Script Debugging

```bash
# Enable build script debugging
RUST_LOG=debug cargo build 2>&1 | grep "cargo:warning"

# Check build script output  
cargo build --verbose

# Force rebuild
cargo clean && cargo build
# Proto is always compiled
```

### Performance Monitoring

```rust
// Add to build.rs for performance monitoring
use std::time::Instant;

fn time_compilation<F, R>(name: &str, f: F) -> R
where
    F: FnOnce() -> R,
{
    let start = Instant::now();
    let result = f();
    let elapsed = start.elapsed();
    println!("cargo:warning={} took: {:?}", name, elapsed);
    result
}

// Use in compilation
time_compilation("Proto compilation", || {
    tonic_build.compile(&[&config.proto_path], &include_paths)
})?;
```

## ⚠️ BREAKING CHANGE: Mandatory Proto Compilation

### What Changed
- **REMOVED**: Feature flags (`grpc`, `full-proto`)
- **REMOVED**: Optional proto compilation
- **REMOVED**: Stub files and graceful degradation
- **REMOVED**: Conditional compilation based on features

### Impact
- **Build Environment**: `protoc` (Protocol Buffer compiler) is now **REQUIRED**
- **Dependencies**: `tonic`, `prost`, `prost-types` are now **REQUIRED** (not optional)
- **CI/CD**: All build environments **MUST** have `protoc` installed
- **Development**: Cannot build partial system without proto support
- **Docker**: Base images **MUST** include `protobuf-compiler`

### Migration Steps
1. Install `protoc` on all development and CI environments
2. Update `Cargo.toml` to remove optional proto dependencies
3. Remove feature flag references in code
4. Update Docker images to include Protocol Buffer compiler
5. Ensure all proto files are valid and compile successfully

### Benefits
- **Simplified Build**: No conditional compilation complexity
- **Type Safety**: Always have proto types available
- **Performance**: No runtime feature checking
- **Reliability**: Build fails fast on proto issues
- **Maintenance**: Single code path to maintain

This comprehensive build.rs configuration provides a robust foundation for Protocol Buffer compilation in the neural-core EventBus system, with mandatory proto compilation, proper error handling, incremental compilation, and CI/CD integration.