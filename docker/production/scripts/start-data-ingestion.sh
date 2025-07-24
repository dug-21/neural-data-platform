#!/bin/bash
# Start data ingestion with symbols and providers from environment variables

# Build provider arguments
PROVIDER_ARGS=""

# Check for multiple providers (ACTIVE_PROVIDERS or FALLBACK setup)
if [ -n "$ACTIVE_PROVIDERS" ]; then
    echo "Using active providers: $ACTIVE_PROVIDERS"
    # Parse JSON array or comma-separated list
    if [[ "$ACTIVE_PROVIDERS" =~ ^\[.*\]$ ]]; then
        # JSON array format: ["polygon","alpaca"]
        PROVIDERS=$(echo "$ACTIVE_PROVIDERS" | sed 's/\[//g' | sed 's/\]//g' | sed 's/"//g' | sed 's/,/ /g')
    else
        # Comma-separated format: polygon,alpaca
        PROVIDERS=$(echo "$ACTIVE_PROVIDERS" | sed 's/,/ /g')
    fi
    for provider in $PROVIDERS; do
        PROVIDER_ARGS="$PROVIDER_ARGS --providers $provider"
    done
elif [ -n "$PRIMARY_PROVIDER" ] && [ -n "$FALLBACK_PROVIDERS" ]; then
    echo "Using primary provider: $PRIMARY_PROVIDER with fallbacks: $FALLBACK_PROVIDERS"
    PROVIDER_ARGS="--providers $PRIMARY_PROVIDER"
    # Add fallback providers
    if [[ "$FALLBACK_PROVIDERS" =~ ^\[.*\]$ ]]; then
        # JSON array format
        FALLBACKS=$(echo "$FALLBACK_PROVIDERS" | sed 's/\[//g' | sed 's/\]//g' | sed 's/"//g' | sed 's/,/ /g')
    else
        # Comma-separated format
        FALLBACKS=$(echo "$FALLBACK_PROVIDERS" | sed 's/,/ /g')
    fi
    for provider in $FALLBACKS; do
        PROVIDER_ARGS="$PROVIDER_ARGS --providers $provider"
    done
elif [ -n "$PRIMARY_PROVIDER" ]; then
    echo "Using primary provider: $PRIMARY_PROVIDER"
    PROVIDER_ARGS="--providers $PRIMARY_PROVIDER"
elif [ -n "$DEFAULT_PROVIDER" ]; then
    echo "Using default provider: $DEFAULT_PROVIDER"
    PROVIDER_ARGS="--providers $DEFAULT_PROVIDER"
fi

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
    exec python -m main start $PROVIDER_ARGS $SYMBOL_ARGS "$@"
else
    echo "No symbols specified, using defaults"
    exec python -m main start $PROVIDER_ARGS "$@"
fi