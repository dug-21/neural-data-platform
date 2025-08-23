#!/bin/bash

# Neural Trader - gRPC Proto Compilation Script
# Compiles protocol buffer definitions for Python integration
# Usage: ./compile_proto.sh [--clean] [--install-deps]

set -euo pipefail

# Script configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
PROTO_DIR="$SCRIPT_DIR"
OUTPUT_DIR="$PROJECT_ROOT/src/proto"
PYTHON_OUTPUT_DIR="$PROJECT_ROOT/data_ingestion/proto"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Logging functions
log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[SUCCESS]${NC} $1"; }
log_warning() { echo -e "${YELLOW}[WARNING]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

# Print usage information
usage() {
    cat << EOF
Neural Trader Protocol Buffer Compilation Script

Usage: $0 [OPTIONS]

OPTIONS:
    --clean         Clean existing generated files before compilation
    --install-deps  Install required dependencies (protoc, grpcio-tools)
    --python-only   Generate Python bindings only
    --rust-only     Generate Rust bindings only
    --check-deps    Check if dependencies are installed
    --help          Show this help message

EXAMPLES:
    $0                    # Compile all proto files
    $0 --clean           # Clean and compile
    $0 --install-deps    # Install dependencies and compile
    $0 --python-only     # Generate Python bindings only
    
This script generates:
    - Python gRPC stubs in $PYTHON_OUTPUT_DIR
    - Rust bindings in $OUTPUT_DIR (if prost is available)
    
Dependencies:
    - protobuf-compiler (protoc)
    - Python: grpcio-tools, protobuf
    - Rust: prost, tonic (optional)
EOF
}

# Check if a command exists
command_exists() {
    command -v "$1" >/dev/null 2>&1
}

# Check dependencies
check_dependencies() {
    log_info "Checking dependencies..."
    
    local missing_deps=()
    
    # Check protoc
    if ! command_exists protoc; then
        missing_deps+=("protobuf-compiler")
    else
        log_success "protoc found: $(protoc --version)"
    fi
    
    # Check Python dependencies
    if command_exists python3; then
        if ! python3 -c "import grpc_tools.protoc" 2>/dev/null; then
            missing_deps+=("grpcio-tools (Python)")
        else
            log_success "grpcio-tools found"
        fi
        
        if ! python3 -c "import google.protobuf" 2>/dev/null; then
            missing_deps+=("protobuf (Python)")
        else
            log_success "protobuf (Python) found"
        fi
    else
        log_warning "Python3 not found - Python bindings will not be generated"
    fi
    
    # Check Rust dependencies (optional)
    if [ -f "$PROJECT_ROOT/Cargo.toml" ]; then
        if command_exists cargo; then
            if cargo metadata --format-version 1 2>/dev/null | grep -q '"prost"'; then
                log_success "prost (Rust) found"
            else
                log_warning "prost (Rust) not found - Rust bindings will not be generated"
            fi
        else
            log_warning "cargo not found - Rust bindings will not be generated"
        fi
    fi
    
    if [ ${#missing_deps[@]} -ne 0 ]; then
        log_error "Missing dependencies: ${missing_deps[*]}"
        return 1
    fi
    
    log_success "All required dependencies are available"
    return 0
}

# Install dependencies
install_dependencies() {
    log_info "Installing dependencies..."
    
    # Install system dependencies
    if command_exists apt-get; then
        sudo apt-get update && sudo apt-get install -y protobuf-compiler
    elif command_exists brew; then
        brew install protobuf
    elif command_exists pacman; then
        sudo pacman -S protobuf
    else
        log_warning "Please install protobuf-compiler manually"
    fi
    
    # Install Python dependencies
    if command_exists python3; then
        python3 -m pip install --upgrade grpcio-tools protobuf
        log_success "Python dependencies installed"
    fi
    
    # Add Rust dependencies to Cargo.toml if it exists
    if [ -f "$PROJECT_ROOT/Cargo.toml" ] && command_exists cargo; then
        log_info "Adding Rust proto dependencies to Cargo.toml..."
        cd "$PROJECT_ROOT"
        cargo add prost tonic --optional --features "tonic/transport,prost/derive" || true
        log_success "Rust dependencies configured"
    fi
}

# Clean existing generated files
clean_generated_files() {
    log_info "Cleaning existing generated files..."
    
    # Clean Python output
    if [ -d "$PYTHON_OUTPUT_DIR" ]; then
        rm -rf "$PYTHON_OUTPUT_DIR"
        log_success "Cleaned Python output directory"
    fi
    
    # Clean Rust output
    if [ -d "$OUTPUT_DIR" ]; then
        rm -rf "$OUTPUT_DIR"
        log_success "Cleaned Rust output directory"
    fi
    
    # Clean any __pycache__ directories
    find "$PROJECT_ROOT" -name "__pycache__" -type d -exec rm -rf {} + 2>/dev/null || true
    find "$PROJECT_ROOT" -name "*.pyc" -delete 2>/dev/null || true
}

# Generate Python bindings
generate_python_bindings() {
    log_info "Generating Python gRPC bindings..."
    
    # Create output directory
    mkdir -p "$PYTHON_OUTPUT_DIR"
    
    # Create __init__.py files
    touch "$PYTHON_OUTPUT_DIR/__init__.py"
    
    # Find all proto files
    local proto_files=()
    while IFS= read -r -d '' file; do
        proto_files+=("$file")
    done < <(find "$PROTO_DIR" -name "*.proto" -print0)
    
    if [ ${#proto_files[@]} -eq 0 ]; then
        log_warning "No .proto files found in $PROTO_DIR"
        return 1
    fi
    
    log_info "Found ${#proto_files[@]} proto file(s): ${proto_files[*]##*/}"
    
    # Generate Python code
    python3 -m grpc_tools.protoc \
        --proto_path="$PROTO_DIR" \
        --python_out="$PYTHON_OUTPUT_DIR" \
        --grpc_python_out="$PYTHON_OUTPUT_DIR" \
        --pyi_out="$PYTHON_OUTPUT_DIR" \
        "${proto_files[@]}"
    
    # Fix import statements in generated files
    log_info "Fixing import statements in generated Python files..."
    find "$PYTHON_OUTPUT_DIR" -name "*_pb2*.py" -exec sed -i.bak \
        's/^import \([^.]*\)_pb2 as/from . import \1_pb2 as/g' {} \;
    
    # Remove backup files
    find "$PYTHON_OUTPUT_DIR" -name "*.bak" -delete 2>/dev/null || true
    
    # Create configuration client example
    cat > "$PYTHON_OUTPUT_DIR/config_client_example.py" << 'EOF'
"""
Configuration Store Client Example
Demonstrates usage of the generated gRPC stubs
"""

import grpc
from . import config_store_pb2
from . import config_store_pb2_grpc


class ConfigClient:
    """Example configuration client implementation"""
    
    def __init__(self, server_address: str = "localhost:50051"):
        self.channel = grpc.insecure_channel(server_address)
        self.stub = config_store_pb2_grpc.ConfigStoreServiceStub(self.channel)
    
    def get_config(self, namespace_path: str, key: str, version: str = None):
        """Get a configuration value"""
        request = config_store_pb2.GetConfigRequest(
            namespace_path=namespace_path,
            key=key,
            version=version or "",
            include_metadata=True
        )
        
        try:
            response = self.stub.GetConfig(request)
            if response.success:
                return response.value, response.metadata
            else:
                raise ValueError(f"Config retrieval failed: {response.error_message}")
        except grpc.RpcError as e:
            raise ConnectionError(f"gRPC error: {e}")
    
    def set_config(self, namespace_path: str, key: str, value, change_reason: str):
        """Set a configuration value"""
        # Create ConfigValue based on Python type
        config_value = config_store_pb2.ConfigValue()
        
        if isinstance(value, str):
            config_value.type = config_store_pb2.VALUE_TYPE_STRING
            config_value.string_value = value
        elif isinstance(value, bool):
            config_value.type = config_store_pb2.VALUE_TYPE_BOOL
            config_value.bool_value = value
        elif isinstance(value, int):
            config_value.type = config_store_pb2.VALUE_TYPE_INT
            config_value.int_value = value
        elif isinstance(value, float):
            config_value.type = config_store_pb2.VALUE_TYPE_FLOAT
            config_value.float_value = value
        else:
            # For complex objects, use JSON
            import json
            from google.protobuf.struct_pb2 import Struct
            config_value.type = config_store_pb2.VALUE_TYPE_JSON
            config_value.json_value.update(json.loads(json.dumps(value)))
        
        request = config_store_pb2.SetConfigRequest(
            namespace_path=namespace_path,
            key=key,
            value=config_value,
            change_reason=change_reason
        )
        
        try:
            response = self.stub.SetConfig(request)
            if response.success:
                return response.new_version
            else:
                raise ValueError(f"Config update failed: {response.error_message}")
        except grpc.RpcError as e:
            raise ConnectionError(f"gRPC error: {e}")
    
    def watch_config(self, namespace_path: str, keys: list = None):
        """Watch for configuration changes"""
        request = config_store_pb2.WatchConfigRequest(
            namespace_path=namespace_path,
            keys=keys or [],
            include_initial_values=True
        )
        
        try:
            for event in self.stub.WatchConfig(request):
                yield event
        except grpc.RpcError as e:
            raise ConnectionError(f"gRPC error: {e}")
    
    def close(self):
        """Close the gRPC channel"""
        self.channel.close()


# Example usage
if __name__ == "__main__":
    client = ConfigClient()
    
    try:
        # Get configuration
        value, metadata = client.get_config(
            "/neural-trading/data-ingestion",
            "sources.primary.symbols"
        )
        print(f"Config value: {value}")
        
        # Set configuration
        new_version = client.set_config(
            "/neural-trading/data-ingestion",
            "sources.primary.rate_limits.requests_per_minute",
            250,
            "Updated rate limit for better performance"
        )
        print(f"Updated to version: {new_version}")
        
    finally:
        client.close()
EOF
    
    log_success "Python gRPC bindings generated successfully"
    log_info "Generated files in: $PYTHON_OUTPUT_DIR"
    log_info "Example client created: $PYTHON_OUTPUT_DIR/config_client_example.py"
}

# Generate Rust bindings (if available)
generate_rust_bindings() {
    if [ ! -f "$PROJECT_ROOT/Cargo.toml" ]; then
        log_warning "Cargo.toml not found - skipping Rust bindings"
        return 0
    fi
    
    if ! command_exists cargo; then
        log_warning "cargo not found - skipping Rust bindings"
        return 0
    fi
    
    # Check if prost is available
    if ! cargo metadata --format-version 1 2>/dev/null | grep -q '"prost"'; then
        log_warning "prost not found in Cargo.toml - skipping Rust bindings"
        return 0
    fi
    
    log_info "Generating Rust gRPC bindings..."
    
    # Create output directory
    mkdir -p "$OUTPUT_DIR"
    
    # Create build.rs file for automatic proto compilation
    cat > "$PROJECT_ROOT/build.rs" << 'EOF'
use std::env;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let proto_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?).join("proto");
    
    let proto_files = std::fs::read_dir(&proto_dir)?
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
    
    if proto_files.is_empty() {
        println!("cargo:warning=No .proto files found in {:?}", proto_dir);
        return Ok(());
    }
    
    for proto_file in &proto_files {
        println!("cargo:rerun-if-changed={}", proto_file.display());
    }
    
    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .out_dir("src/proto")
        .compile(&proto_files, &[proto_dir])?;
    
    println!("cargo:rerun-if-changed=proto/");
    Ok(())
}
EOF
    
    # Add build dependencies to Cargo.toml if not present
    if ! grep -q "\[build-dependencies\]" "$PROJECT_ROOT/Cargo.toml"; then
        cat >> "$PROJECT_ROOT/Cargo.toml" << 'EOF'

[build-dependencies]
tonic-build = "0.10"
EOF
    fi
    
    log_success "Rust build configuration created"
    log_info "Run 'cargo build' to generate Rust bindings"
}

# Validate generated files
validate_generated_files() {
    log_info "Validating generated files..."
    
    # Check Python files
    if [ -d "$PYTHON_OUTPUT_DIR" ]; then
        local python_files_count=$(find "$PYTHON_OUTPUT_DIR" -name "*.py" | wc -l)
        log_success "Generated $python_files_count Python files"
        
        # Check if main proto file was generated
        if [ -f "$PYTHON_OUTPUT_DIR/config_store_pb2.py" ]; then
            log_success "Main config_store_pb2.py generated"
        else
            log_error "config_store_pb2.py not found"
            return 1
        fi
        
        if [ -f "$PYTHON_OUTPUT_DIR/config_store_pb2_grpc.py" ]; then
            log_success "gRPC service config_store_pb2_grpc.py generated"
        else
            log_error "config_store_pb2_grpc.py not found"
            return 1
        fi
    fi
    
    # Test Python imports
    if command_exists python3; then
        log_info "Testing Python imports..."
        if python3 -c "
import sys
sys.path.insert(0, '$PYTHON_OUTPUT_DIR')
try:
    import config_store_pb2
    import config_store_pb2_grpc
    print('Python imports successful')
except ImportError as e:
    print(f'Import error: {e}')
    sys.exit(1)
" 2>&1; then
            log_success "Python imports validated"
        else
            log_error "Python import validation failed"
            return 1
        fi
    fi
    
    return 0
}

# Main execution
main() {
    local clean_mode=false
    local install_deps=false
    local python_only=false
    local rust_only=false
    local check_deps_only=false
    
    # Parse arguments
    while [[ $# -gt 0 ]]; do
        case $1 in
            --clean)
                clean_mode=true
                shift
                ;;
            --install-deps)
                install_deps=true
                shift
                ;;
            --python-only)
                python_only=true
                shift
                ;;
            --rust-only)
                rust_only=true
                shift
                ;;
            --check-deps)
                check_deps_only=true
                shift
                ;;
            --help)
                usage
                exit 0
                ;;
            *)
                log_error "Unknown option: $1"
                usage
                exit 1
                ;;
        esac
    done
    
    log_info "Neural Trader Protocol Buffer Compilation"
    log_info "Proto directory: $PROTO_DIR"
    log_info "Python output: $PYTHON_OUTPUT_DIR"
    log_info "Rust output: $OUTPUT_DIR"
    
    # Check or install dependencies
    if [ "$check_deps_only" = true ]; then
        check_dependencies
        exit $?
    fi
    
    if [ "$install_deps" = true ]; then
        install_dependencies
    fi
    
    if ! check_dependencies; then
        log_error "Dependencies check failed. Run with --install-deps to install them."
        exit 1
    fi
    
    # Clean if requested
    if [ "$clean_mode" = true ]; then
        clean_generated_files
    fi
    
    # Generate bindings
    local success=true
    
    if [ "$rust_only" != true ]; then
        if ! generate_python_bindings; then
            success=false
        fi
    fi
    
    if [ "$python_only" != true ]; then
        if ! generate_rust_bindings; then
            success=false
        fi
    fi
    
    # Validate results
    if [ "$success" = true ] && [ "$rust_only" != true ]; then
        if ! validate_generated_files; then
            success=false
        fi
    fi
    
    # Final status
    if [ "$success" = true ]; then
        log_success "Protocol buffer compilation completed successfully!"
        
        echo ""
        echo "Next steps:"
        echo "1. Import the generated Python modules in your data ingestion service"
        echo "2. Use the ConfigClient example as a starting point"
        echo "3. Run 'cargo build' to generate Rust bindings (if using Rust)"
        echo ""
        echo "Generated files:"
        [ -d "$PYTHON_OUTPUT_DIR" ] && echo "  Python: $(find "$PYTHON_OUTPUT_DIR" -name "*.py" | wc -l) files in $PYTHON_OUTPUT_DIR"
        [ -f "$PROJECT_ROOT/build.rs" ] && echo "  Rust: build.rs configured for automatic generation"
        
    else
        log_error "Protocol buffer compilation failed!"
        exit 1
    fi
}

# Run main function
main "$@"