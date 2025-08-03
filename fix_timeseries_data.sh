#!/bin/bash

# Fix TimeSeriesData struct initialization in tests
# This script adds the missing fields to TimeSeriesData struct literals

echo "Fixing TimeSeriesData struct initializations..."

# Find all test files that might have TimeSeriesData structs
test_files=$(find tests -name "*.rs" -type f)
src_files=$(find src -name "*.rs" -type f)

# Create a temporary file for sed commands
temp_sed_script=$(mktemp)

# Generate sed script to add missing fields after volume field
cat > "$temp_sed_script" << 'EOF'
# Add missing fields after volume field
/volume: .*[,]$/ {
    a\
        volume_value: 1000.0,\
        source: Some("test".to_string()),\
        entity: Some(symbol.to_string()),\
        value: Some(price),\
        metadata: None,\
        values: vec![price],\
        intervals: vec![0],\
        timestamps: vec![Utc::now()],\
        metadata_map: HashMap::new(),
}
EOF

# Apply fixes to test files
for file in $test_files $src_files; do
    if grep -q "TimeSeriesData {" "$file" && grep -q "volume:" "$file"; then
        echo "Checking $file..."
        # Check if it's missing the new fields
        if ! grep -q "volume_value:" "$file"; then
            echo "  Fixing $file..."
            # Create a backup
            cp "$file" "$file.bak"
            
            # Apply the sed script
            sed -i '' -f "$temp_sed_script" "$file"
        fi
    fi
done

# Clean up
rm "$temp_sed_script"

echo "Done fixing TimeSeriesData initializations."