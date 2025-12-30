# Topic Router Algorithm - DP-003

## Overview

The TopicRouter is responsible for converting MQTT wildcard patterns to regex and routing incoming messages to the correct stream based on topic matching.

## Data Structures

```
STRUCTURE RouteEntry:
    pattern: String          // Original MQTT pattern (e.g., "airgradient/readings/+")
    regex: CompiledRegex     // Compiled regex for matching
    stream_id: String        // Target stream (e.g., "air-quality")
    parser: ParserReference  // Parser instance for this subscription
    enabled: Boolean         // Whether this route is active

STRUCTURE TopicRouter:
    routes: Array<RouteEntry>    // Routes in priority order (first match wins)
    pattern_count: Integer       // Number of active routes
```

---

## Algorithm 1: MQTT Pattern to Regex Conversion

Converts MQTT wildcard patterns to regular expressions at config load time.

### Input/Output

```
INPUT:
    pattern: String     // MQTT topic pattern with wildcards (e.g., "sensors/+/temperature")

OUTPUT:
    regex: CompiledRegex OR Error

CONSTRAINTS:
    - '+' wildcard must match exactly one topic level
    - '#' wildcard must be at the end of pattern (or after trailing '/')
    - Pattern cannot be empty
```

### Algorithm

```
ALGORITHM: mqtt_pattern_to_regex
INPUT: pattern (String)
OUTPUT: CompiledRegex OR Error

BEGIN
    // Step 1: Validate pattern
    IF pattern is empty THEN
        RETURN Error("Empty pattern not allowed")
    END IF

    // Step 2: Validate '#' wildcard placement
    // '#' must be at end of pattern or preceded by '/'
    IF pattern contains '#' THEN
        IF NOT (pattern ends with '#' OR pattern ends with '/#') THEN
            RETURN Error("# wildcard must be at end of pattern: {pattern}")
        END IF

        // Check '#' appears only once
        count <- count occurrences of '#' in pattern
        IF count > 1 THEN
            RETURN Error("Multiple # wildcards not allowed: {pattern}")
        END IF
    END IF

    // Step 3: Build regex string
    regex_str <- "^"    // Anchor at start
    segments <- split pattern by '/'

    FOR i <- 0 TO length(segments) - 1 DO
        segment <- segments[i]

        // Add separator (except for first segment)
        IF i > 0 THEN
            regex_str <- regex_str + "/"
        END IF

        // Convert segment based on wildcard type
        CASE segment OF
            "+":
                // Single-level wildcard: match any non-empty string without '/'
                regex_str <- regex_str + "[^/]+"

            "#":
                // Multi-level wildcard: match zero or more characters
                regex_str <- regex_str + ".*"
                BREAK   // '#' matches rest of topic, stop processing

            DEFAULT:
                // Literal segment: escape regex special characters
                escaped <- regex_escape(segment)
                regex_str <- regex_str + escaped
        END CASE
    END FOR

    // Step 4: Anchor at end
    regex_str <- regex_str + "$"

    // Step 5: Compile regex
    TRY
        compiled <- compile_regex(regex_str)
        RETURN compiled
    CATCH regex_error
        RETURN Error("Failed to compile regex: {regex_error}")
    END TRY
END
```

### Conversion Examples

| MQTT Pattern | Regex | Description |
|--------------|-------|-------------|
| `sensors/+/temp` | `^sensors/[^/]+/temp$` | Single-level wildcard |
| `sensors/#` | `^sensors/.*$` | Multi-level wildcard |
| `airgradient/readings/+` | `^airgradient/readings/[^/]+$` | Device serial |
| `homeassistant/+/+/state` | `^homeassistant/[^/]+/[^/]+/state$` | Domain/entity |
| `home/sensor.living_room` | `^home/sensor\.living_room$` | Escaped dot |

---

## Algorithm 2: Topic Router Creation

Creates a TopicRouter from subscription configurations.

### Input/Output

```
INPUT:
    subscriptions: Array<SubscriptionConfig>
        - stream_id: String
        - topic_pattern: String
        - parser: Optional<ParserConfig>
        - enabled: Boolean

OUTPUT:
    TopicRouter OR Error
```

### Algorithm

```
ALGORITHM: create_topic_router
INPUT: subscriptions (Array<SubscriptionConfig>)
OUTPUT: TopicRouter OR Error

BEGIN
    routes <- empty array
    seen_patterns <- empty set

    // Step 1: Process each subscription
    FOR EACH sub IN subscriptions DO
        // Skip disabled subscriptions
        IF NOT sub.enabled THEN
            log_debug("Skipping disabled subscription: {sub.stream_id}")
            CONTINUE
        END IF

        // Step 2: Validate stream_id
        IF sub.stream_id is empty THEN
            RETURN Error("Subscription missing stream_id")
        END IF

        // Step 3: Check for duplicate patterns
        IF sub.topic_pattern IN seen_patterns THEN
            log_warn("Duplicate topic pattern: {sub.topic_pattern}")
            // Allow duplicates but warn (first match wins)
        END IF
        seen_patterns.add(sub.topic_pattern)

        // Step 4: Convert pattern to regex
        regex <- mqtt_pattern_to_regex(sub.topic_pattern)
        IF regex is Error THEN
            RETURN Error("Invalid pattern for {sub.stream_id}: {regex.message}")
        END IF

        // Step 5: Create parser for this subscription
        parser <- create_parser(sub.parser)
        IF parser is Error THEN
            RETURN Error("Failed to create parser for {sub.stream_id}: {parser.message}")
        END IF

        // Step 6: Create route entry
        route <- RouteEntry {
            pattern: sub.topic_pattern,
            regex: regex,
            stream_id: sub.stream_id,
            parser: parser,
            enabled: true
        }

        routes.append(route)
        log_info("Added route: {sub.topic_pattern} -> {sub.stream_id}")
    END FOR

    // Step 7: Validate we have at least one route
    IF routes is empty THEN
        RETURN Error("No enabled subscriptions configured")
    END IF

    // Step 8: Return router
    RETURN TopicRouter {
        routes: routes,
        pattern_count: length(routes)
    }
END
```

---

## Algorithm 3: Topic Routing

Routes an incoming MQTT topic to the appropriate stream.

### Input/Output

```
INPUT:
    router: TopicRouter
    topic: String       // Incoming MQTT topic (e.g., "airgradient/readings/ABC123")

OUTPUT:
    RouteEntry OR None  // Matching route, or None if no match
```

### Algorithm

```
ALGORITHM: route_topic
INPUT: router (TopicRouter), topic (String)
OUTPUT: RouteEntry OR None

BEGIN
    // Step 1: Validate topic
    IF topic is empty THEN
        log_warn("Empty topic received")
        RETURN None
    END IF

    // Step 2: Try each route in order (FIRST MATCH WINS)
    FOR EACH route IN router.routes DO
        IF route.regex.is_match(topic) THEN
            log_debug("Topic '{topic}' matched pattern '{route.pattern}' -> stream '{route.stream_id}'")
            RETURN route
        END IF
    END FOR

    // Step 3: No match found
    log_debug("No route found for topic: {topic}")
    RETURN None
END
```

### Routing Examples

Given routes (in order):
1. `homeassistant/climate/+/state` -> hvac
2. `homeassistant/+/+/state` -> homeassistant
3. `airgradient/readings/+` -> air-quality

| Incoming Topic | Matched Route | Stream |
|----------------|---------------|--------|
| `airgradient/readings/ABC123` | #3 | air-quality |
| `homeassistant/sensor/temp/state` | #2 | homeassistant |
| `homeassistant/climate/living_room/state` | #1 | hvac |
| `unknown/topic` | None | (dead letter) |

---

## Algorithm 4: Get Subscription Topics

Returns all topic patterns for MQTT subscription.

### Input/Output

```
INPUT:
    router: TopicRouter

OUTPUT:
    Array<String>    // Topic patterns to subscribe to
```

### Algorithm

```
ALGORITHM: get_topic_patterns
INPUT: router (TopicRouter)
OUTPUT: Array<String>

BEGIN
    patterns <- empty array

    FOR EACH route IN router.routes DO
        patterns.append(route.pattern)
    END FOR

    RETURN patterns
END
```

---

## Edge Cases and Error Handling

### Invalid Pattern Detection

```
ALGORITHM: validate_mqtt_pattern
INPUT: pattern (String)
OUTPUT: Boolean (valid) AND String (error message if invalid)

BEGIN
    // Case 1: Empty pattern
    IF pattern is empty THEN
        RETURN (false, "Pattern cannot be empty")
    END IF

    // Case 2: Pattern starts with '/'
    IF pattern starts with '/' THEN
        RETURN (false, "Pattern cannot start with /")
    END IF

    // Case 3: Pattern ends with '/' (except for "#")
    IF pattern ends with '/' AND NOT pattern ends with '/#' THEN
        RETURN (false, "Pattern cannot end with /")
    END IF

    // Case 4: Empty segments (double slashes)
    IF pattern contains '//' THEN
        RETURN (false, "Pattern cannot have empty segments")
    END IF

    // Case 5: '#' not at end
    IF pattern contains '#' AND NOT (pattern ends with '#' OR pattern ends with '/#') THEN
        RETURN (false, "# wildcard must be at end")
    END IF

    // Case 6: '+' spanning multiple levels (invalid)
    // '+' is always single-level, this is validated by segment parsing

    RETURN (true, "")
END
```

### Performance Optimization

```
PERFORMANCE NOTES:

1. Regex Compilation
   - Compile regex ONCE at startup
   - Store compiled regex, not pattern string
   - Expected: < 1ms per pattern

2. Route Matching
   - Linear scan through routes (O(n) where n = number of routes)
   - For typical use (< 10 routes): < 10 microseconds
   - Consider trie optimization if routes > 100

3. Pattern Ordering
   - Place high-volume patterns FIRST for early match
   - Place specific patterns BEFORE general patterns
   - Example: "climate/+/state" before "+/+/state"

4. Memory Usage
   - Each RouteEntry: ~200 bytes (pattern + regex + metadata)
   - 10 routes: ~2KB
   - 100 routes: ~20KB
```

---

## Complexity Analysis

| Algorithm | Time Complexity | Space Complexity |
|-----------|-----------------|------------------|
| `mqtt_pattern_to_regex` | O(n) where n = pattern length | O(n) for regex string |
| `create_topic_router` | O(k * n) where k = subscriptions, n = pattern length | O(k * n) for compiled routes |
| `route_topic` | O(k * m) where k = routes, m = topic length | O(1) |
| `get_topic_patterns` | O(k) | O(k) for output array |

---

## Test Cases

### Unit Tests for Pattern Conversion

```
TEST: single_level_wildcard
    INPUT: "sensors/+/temperature"
    EXPECTED REGEX: ^sensors/[^/]+/temperature$
    MATCHES: "sensors/room1/temperature", "sensors/kitchen/temperature"
    NO MATCH: "sensors/temperature", "sensors/room1/sub/temperature"

TEST: multi_level_wildcard
    INPUT: "homeassistant/#"
    EXPECTED REGEX: ^homeassistant/.*$
    MATCHES: "homeassistant", "homeassistant/sensor", "homeassistant/sensor/temp/state"
    NO MATCH: "other/topic"

TEST: combined_wildcards
    INPUT: "home/+/devices/#"
    EXPECTED REGEX: ^home/[^/]+/devices/.*$
    MATCHES: "home/room1/devices", "home/room1/devices/light/status"
    NO MATCH: "home/devices/light"

TEST: invalid_hash_position
    INPUT: "sensors/#/temperature"
    EXPECTED: Error("# wildcard must be at end")

TEST: escaped_special_chars
    INPUT: "home/sensor.room1"
    EXPECTED REGEX: ^home/sensor\.room1$
    MATCHES: "home/sensor.room1"
    NO MATCH: "home/sensorXroom1"
```

### Integration Tests for Routing

```
TEST: first_match_wins
    ROUTES:
        1. "climate/+/state" -> hvac
        2. "+/+/state" -> general

    INPUT: "climate/living_room/state"
    EXPECTED: hvac (matches route 1 first)

TEST: no_match_returns_none
    ROUTES:
        1. "airgradient/readings/+" -> air-quality

    INPUT: "unknown/topic/here"
    EXPECTED: None

TEST: disabled_routes_skipped
    ROUTES:
        1. "sensors/+/temp" (disabled)
        2. "sensors/#" -> fallback

    INPUT: "sensors/room1/temp"
    EXPECTED: fallback (route 1 skipped)
```

---

## Related Documents

- ADR-003-TOPIC-ROUTING.md: Architectural decision
- CONFIG_PARSER.md: How config is loaded
- MESSAGE_PROCESSOR.md: How routing integrates with message processing
