#!/bin/bash
# Debug version of start script to see what's happening

echo "=== Environment Variables ==="
echo "PRIMARY_PROVIDER: $PRIMARY_PROVIDER"
echo "DEFAULT_PROVIDER: $DEFAULT_PROVIDER"
echo "SYMBOLS: $SYMBOLS"
echo ""

# Build provider arguments if PRIMARY_PROVIDER is set
PROVIDER_ARGS=""
if [ -n "$PRIMARY_PROVIDER" ]; then
    echo "Building provider args for: $PRIMARY_PROVIDER"
    PROVIDER_ARGS="--providers $PRIMARY_PROVIDER"
elif [ -n "$DEFAULT_PROVIDER" ]; then
    echo "Building provider args for: $DEFAULT_PROVIDER"
    PROVIDER_ARGS="--providers $DEFAULT_PROVIDER"
else
    echo "No provider environment variable set"
fi

# Convert comma-separated SYMBOLS to multiple --symbols arguments
SYMBOL_ARGS=""
if [ -n "$SYMBOLS" ]; then
    IFS=',' read -ra SYMBOL_ARRAY <<< "$SYMBOLS"
    for symbol in "${SYMBOL_ARRAY[@]}"; do
        SYMBOL_ARGS="$SYMBOL_ARGS --symbols $symbol"
    done
fi

echo ""
echo "=== Final Command ==="
echo "python -m main start $PROVIDER_ARGS $SYMBOL_ARGS"
echo ""

# Actually run it
exec python -m main start $PROVIDER_ARGS $SYMBOL_ARGS "$@"