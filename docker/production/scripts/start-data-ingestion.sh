#!/bin/bash
# Start data ingestion with symbols from environment variable

# Convert comma-separated SYMBOLS to multiple --symbols arguments
if [ -n "$SYMBOLS" ]; then
    # Split SYMBOLS by comma and build arguments
    SYMBOL_ARGS=""
    IFS=',' read -ra SYMBOL_ARRAY <<< "$SYMBOLS"
    for symbol in "${SYMBOL_ARRAY[@]}"; do
        SYMBOL_ARGS="$SYMBOL_ARGS --symbols $symbol"
    done
    
    echo "Starting data ingestion with symbols: $SYMBOLS"
    # Use python -m to avoid __main__ execution issues
    exec python -m main start $SYMBOL_ARGS "$@"
else
    echo "No symbols specified, using defaults"
    exec python -m main start "$@"
fi