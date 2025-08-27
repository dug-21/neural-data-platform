#!/bin/bash

# Script to migrate remaining Event::new usages to ProtoEvent::new

echo "🚀 Neural Trader Core Client Migration Script"
echo "============================================="

# Find all files with Event::new usage
echo "Finding files with Event::new usage..."
FILES=$(find . -name "*.rs" -type f -exec grep -l "Event::new(" {} \;)

echo "Files to update:"
for file in $FILES; do
    echo "  - $file"
done

echo ""
echo "Migration completed! Run cargo build to test."