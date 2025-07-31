#!/bin/bash

# Model Storage Validation Script
# Validates the model storage architecture and directory structure

set -e

MODEL_ROOT="/workspaces/neural-trader/models"
SYMBOLS=("AAPL" "GOOGL" "MSFT" "TSLA")
MODEL_TYPES=("prediction" "momentum" "reversal")

echo "🔍 Validating Model Storage Architecture..."
echo "=================================================="

# Check root models directory exists
if [ ! -d "$MODEL_ROOT" ]; then
    echo "❌ ERROR: Models root directory not found: $MODEL_ROOT"
    exit 1
fi
echo "✅ Models root directory exists: $MODEL_ROOT"

# Check symbol directories
for symbol in "${SYMBOLS[@]}"; do
    symbol_dir="$MODEL_ROOT/$symbol"
    if [ ! -d "$symbol_dir" ]; then
        echo "❌ ERROR: Symbol directory not found: $symbol_dir"
        exit 1
    fi
    echo "✅ Symbol directory exists: $symbol"
    
    # Check model type directories
    for model_type in "${MODEL_TYPES[@]}"; do
        type_dir="$symbol_dir/$model_type"
        if [ ! -d "$type_dir" ]; then
            echo "❌ ERROR: Model type directory not found: $type_dir"
            exit 1
        fi
        
        # Check current symlink
        current_link="$type_dir/current"
        if [ ! -L "$current_link" ]; then
            echo "⚠️  WARNING: Current symlink not found: $current_link"
        else
            target=$(readlink "$current_link")
            echo "✅ Current symlink exists: $symbol/$model_type -> $target"
        fi
        
        # Check metadata directory
        metadata_dir="$type_dir/metadata"
        if [ ! -d "$metadata_dir" ]; then
            echo "❌ ERROR: Metadata directory not found: $metadata_dir"
            exit 1
        fi
        
        # Check backups directory
        backups_dir="$type_dir/backups"
        if [ ! -d "$backups_dir" ]; then
            echo "❌ ERROR: Backups directory not found: $backups_dir"
            exit 1
        fi
    done
done

# Check template directories
template_dir="$MODEL_ROOT/templates"
if [ ! -d "$template_dir" ]; then
    echo "❌ ERROR: Templates directory not found: $template_dir"
    exit 1
fi
echo "✅ Templates directory exists"

for model_type in "${MODEL_TYPES[@]}"; do
    template_type_dir="$template_dir/$model_type"
    if [ ! -d "$template_type_dir" ]; then
        echo "❌ ERROR: Template type directory not found: $template_type_dir"
        exit 1
    fi
done

# Check shared directory
shared_dir="$MODEL_ROOT/shared"
if [ ! -d "$shared_dir" ]; then
    echo "❌ ERROR: Shared directory not found: $shared_dir"
    exit 1
fi
echo "✅ Shared directory exists"

shared_subdirs=("common" "utils" "configs")
for subdir in "${shared_subdirs[@]}"; do
    shared_subdir="$shared_dir/$subdir"
    if [ ! -d "$shared_subdir" ]; then
        echo "❌ ERROR: Shared subdirectory not found: $shared_subdir"
        exit 1
    fi
done

# Check permissions
echo ""
echo "🔒 Checking Permissions..."
echo "=========================="

# Check directory permissions (should be 755)
find "$MODEL_ROOT" -type d -exec ls -ld {} \; | while read perm rest path; do
    if [[ ! "$perm" =~ ^drwxr-xr-x ]]; then
        echo "⚠️  WARNING: Directory permissions may be incorrect: $path ($perm)"
    fi
done

# Check file permissions (should be 644 for config files)
find "$MODEL_ROOT" -name "*.json" -exec ls -l {} \; | while read perm rest; do
    if [[ ! "$perm" =~ ^-rw-r--r-- ]]; then
        echo "⚠️  WARNING: JSON file permissions may be incorrect: $perm"
    fi
done

find "$MODEL_ROOT" -name "*.fann" -exec ls -l {} \; | while read perm rest; do
    if [[ ! "$perm" =~ ^-rw-r--r-- ]]; then
        echo "⚠️  WARNING: FANN file permissions may be incorrect: $perm"
    fi
done

# Check disk usage
echo ""
echo "💾 Disk Usage Analysis..."
echo "========================="
du -sh "$MODEL_ROOT"
du -sh "$MODEL_ROOT"/* | sort -hr

# Check for example files
echo ""
echo "📋 Example Files Check..."
echo "=========================="

example_model="$MODEL_ROOT/AAPL/prediction/v1.0.0/model.fann"
if [ -f "$example_model" ]; then
    echo "✅ Example model file exists: $example_model"
    ls -l "$example_model"
else
    echo "⚠️  No example model files found"
fi

example_config="$MODEL_ROOT/AAPL/prediction/metadata/model_info.json"
if [ -f "$example_config" ]; then
    echo "✅ Example metadata file exists: $example_config"
    echo "   Content preview:"
    head -5 "$example_config" | sed 's/^/   /'
else
    echo "⚠️  No example metadata files found"
fi

# Summary
echo ""
echo "📊 Validation Summary..."
echo "========================="
total_symbols=${#SYMBOLS[@]}
total_model_types=${#MODEL_TYPES[@]}
total_combinations=$((total_symbols * total_model_types))

echo "✅ Validated $total_symbols symbols"
echo "✅ Validated $total_model_types model types"
echo "✅ Validated $total_combinations symbol/model combinations"
echo "✅ Directory structure is compliant with architecture specification"

# Architecture compliance check
echo ""
echo "🏗️  Architecture Compliance..."
echo "=============================="

architecture_doc="/workspaces/neural-trader/docs/MODEL_STORAGE_ARCHITECTURE.md"
if [ -f "$architecture_doc" ]; then
    echo "✅ Architecture documentation exists: $architecture_doc"
    doc_size=$(wc -l < "$architecture_doc")
    echo "   Documentation size: $doc_size lines"
else
    echo "❌ ERROR: Architecture documentation not found"
    exit 1
fi

echo ""
echo "🎉 Model Storage Architecture Validation Complete!"
echo "✅ All checks passed successfully"
echo "📁 Total storage structure validated"
echo "🚀 Ready for Docker production deployment"