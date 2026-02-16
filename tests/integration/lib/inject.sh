#!/usr/bin/env bash
# ops-007: MQTT message injection library
# Injects messages into the integration MQTT broker via docker exec.
#
# Usage (sourced by testbed scripts):
#   source "$(dirname "$0")/../lib/inject.sh"
#   inject_messages --topic "airgradient/readings/test-sensor-001" \
#                   --template "$FIXTURES_DIR/mqtt/airgradient.jsonl" \
#                   --count 10 --rate 1

set -euo pipefail

INJECT_MOSQUITTO_CONTAINER="${INJECT_MOSQUITTO_CONTAINER:-integration-mosquitto}"
INJECT_DEFAULT_TOPIC="airgradient/readings/test-sensor-001"
INJECT_DEFAULT_RATE=1  # messages per second

# Randomize numeric values in a JSON message.
# Replaces each numeric value with a value +/- 20% of the original.
# This provides variance while keeping values in realistic ranges.
#
# Args: $1 = JSON string
# Returns: randomized JSON string on stdout
randomize_message() {
    local msg="$1"

    # Use awk to randomize numeric values in JSON
    # This handles integers and floats, preserving string values
    echo "$msg" | awk -v seed="$RANDOM" '
    BEGIN { srand(seed) }
    {
        result = ""
        rest = $0
        while (match(rest, /:[[:space:]]*-?[0-9]+\.?[0-9]*/)) {
            prefix = substr(rest, 1, RSTART)
            # Extract the colon and optional space
            colon_space = ""
            val_start = RSTART + 1
            for (i = RSTART + 1; i <= RSTART + RLENGTH - 1; i++) {
                c = substr(rest, i, 1)
                if (c == " " || c == "\t") {
                    val_start = i + 1
                } else {
                    break
                }
            }
            colon_space = substr(rest, RSTART + 1, val_start - RSTART - 1)
            val_str = substr(rest, val_start, RSTART + RLENGTH - val_start)

            # Check if next char after value is a letter (part of a key, skip)
            after_pos = RSTART + RLENGTH
            after_char = substr(rest, after_pos + 1, 1)

            val = val_str + 0
            # Randomize: +/- 20%
            factor = 0.8 + (rand() * 0.4)
            new_val = val * factor

            # Preserve integer vs float
            if (index(val_str, ".") > 0) {
                new_val = sprintf("%.1f", new_val)
            } else {
                new_val = sprintf("%d", int(new_val))
            }

            result = result prefix colon_space new_val
            rest = substr(rest, RSTART + RLENGTH)
        }
        result = result rest
        print result
    }'
}

# Inject MQTT messages into the integration broker.
#
# Options:
#   --topic TOPIC       MQTT topic (default: airgradient/readings/test-sensor-001)
#   --template FILE     JSONL template file (each line = one message template)
#   --count N           Number of messages to inject (default: 10)
#   --rate N            Messages per second (default: 1)
#   --randomize         Randomize numeric values in each message (default: true)
#   --no-randomize      Send template messages as-is
#
# Returns: 0 on success, 1 on failure
inject_messages() {
    local topic="$INJECT_DEFAULT_TOPIC"
    local template=""
    local count=10
    local rate="$INJECT_DEFAULT_RATE"
    local randomize=true

    while [[ $# -gt 0 ]]; do
        case "$1" in
            --topic) topic="$2"; shift 2 ;;
            --template) template="$2"; shift 2 ;;
            --count) count="$2"; shift 2 ;;
            --rate) rate="$2"; shift 2 ;;
            --randomize) randomize=true; shift ;;
            --no-randomize) randomize=false; shift ;;
            *) echo "ERROR: Unknown inject option: $1" >&2; return 1 ;;
        esac
    done

    if [ -z "$template" ]; then
        echo "ERROR: --template is required" >&2
        return 1
    fi

    if [ ! -f "$template" ]; then
        echo "ERROR: Template file not found: $template" >&2
        return 1
    fi

    # Read template lines into array
    local -a templates=()
    while IFS= read -r line; do
        [ -n "$line" ] && templates+=("$line")
    done < "$template"

    if [ ${#templates[@]} -eq 0 ]; then
        echo "ERROR: Template file is empty: $template" >&2
        return 1
    fi

    local template_count=${#templates[@]}
    local delay
    if [ "$rate" -gt 0 ]; then
        delay=$(awk "BEGIN { printf \"%.3f\", 1.0 / $rate }")
    else
        delay=0
    fi

    echo "Injecting $count messages to $topic at ${rate} msg/sec..."
    echo "  Container: $INJECT_MOSQUITTO_CONTAINER"
    echo "  Template: $template ($template_count lines)"

    local sent=0
    local failed=0

    for (( i=0; i<count; i++ )); do
        # Cycle through template lines
        local idx=$(( i % template_count ))
        local msg="${templates[$idx]}"

        # Randomize if requested
        if [ "$randomize" = true ]; then
            msg=$(randomize_message "$msg")
        fi

        # Publish via docker exec (ADR-007-004: mosquitto_pub inside container)
        # Pattern ID 46: use arguments not stdin (docker exec -T stdin unreliable)
        if docker exec "$INJECT_MOSQUITTO_CONTAINER" \
            mosquitto_pub -t "$topic" -m "$msg" 2>/dev/null; then
            sent=$((sent + 1))
        else
            failed=$((failed + 1))
            echo "  WARNING: Failed to publish message $((i+1))" >&2
        fi

        # Rate limiting
        if [ "$delay" != "0" ] && [ $i -lt $((count - 1)) ]; then
            sleep "$delay"
        fi
    done

    echo "Injection complete: $sent sent, $failed failed"

    if [ "$failed" -gt 0 ]; then
        return 1
    fi
    return 0
}
