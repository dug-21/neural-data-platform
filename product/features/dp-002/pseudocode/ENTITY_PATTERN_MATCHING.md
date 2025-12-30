# ENTITY_PATTERN_MATCHING.md - HomeAssistant Entity Pattern Matching

## Overview

This document defines the pseudocode for matching HomeAssistant entity IDs against glob-style patterns defined in entity_schemas. The pattern matching system enables flexible schema definitions that can match families of entities (e.g., all AirGradient sensors) rather than requiring explicit enumeration.

---

## Pattern Syntax

### Supported Glob Patterns

| Pattern | Meaning | Example Match |
|---------|---------|---------------|
| `*` | Match any characters (except `.`) | `sensor.airgradient_*` matches `sensor.airgradient_living_room` |
| `?` | Match single character | `sensor.temp?` matches `sensor.temp1` |
| `**` | Match any characters including `.` | `**.pm25` matches `sensor.airgradient.pm25` |
| `[abc]` | Match any character in set | `sensor.[lt]emp` matches `sensor.temp` or `sensor.lemp` |
| `[a-z]` | Match range | `sensor.temp[0-9]` matches `sensor.temp5` |

### HomeAssistant Entity ID Format

```
ENTITY_ID_FORMAT: {domain}.{object_id}

Examples:
  - sensor.airgradient_living_room_pm25
  - binary_sensor.front_door_window
  - climate.living_room_thermostat
  - light.bedroom_ceiling

Components:
  - domain: Entity type (sensor, binary_sensor, climate, light, etc.)
  - object_id: Unique identifier within domain (often includes device name and attribute)
```

---

## Data Structures

### EntitySchema Pattern Entry

```
STRUCTURE: SchemaPatternEntry
FIELDS:
  - schema_name: string         -- Unique identifier, often same as pattern
  - pattern: string             -- Glob pattern to match entity_ids
  - regex: RegExp               -- Compiled regex (cached)
  - priority: integer           -- Higher = checked first (default: 0)
  - device_class: string        -- Optional device class filter
  - domain: string              -- Optional domain filter (extracted from pattern)
  - attributes: List<Attribute>
  - metadata: Map<string, any>
```

### Pattern Cache

```
STRUCTURE: PatternCache
FIELDS:
  - compiled_patterns: Map<pattern_string, RegExp>
  - pattern_order: List<SchemaPatternEntry>  -- Sorted by priority desc
  - last_updated: timestamp
  - cache_hits: integer
  - cache_misses: integer
```

---

## Algorithm 1: Glob to Regex Conversion

```
ALGORITHM: GlobToRegex
PURPOSE: Convert glob-style pattern to regular expression

INPUT:
  - glob_pattern: string (e.g., "sensor.airgradient_*_pm25")

OUTPUT:
  - regex: RegExp object

CONSTANTS:
  SPECIAL_CHARS = "\\^$.|+()[]{}!"  -- Regex metacharacters to escape

BEGIN
    result ← ""
    i ← 0
    in_bracket ← false

    // Add anchor for start of string
    result ← "^"

    WHILE i < LENGTH(glob_pattern) DO
        char ← glob_pattern[i]

        // Handle character classes [...]
        IF char = "[" THEN
            in_bracket ← true
            result ← result + char
            i ← i + 1
            CONTINUE
        END IF

        IF char = "]" AND in_bracket THEN
            in_bracket ← false
            result ← result + char
            i ← i + 1
            CONTINUE
        END IF

        IF in_bracket THEN
            // Inside bracket, only escape \ and ]
            IF char = "\\" THEN
                result ← result + "\\\\"
            ELSE IF char = "]" THEN
                result ← result + "\\]"
            ELSE
                result ← result + char
            END IF
            i ← i + 1
            CONTINUE
        END IF

        // Handle glob wildcards
        SWITCH char
            CASE "*":
                // Check for ** (match across dots)
                IF i + 1 < LENGTH(glob_pattern) AND glob_pattern[i + 1] = "*" THEN
                    result ← result + ".*"  // Match anything including dots
                    i ← i + 2
                ELSE
                    result ← result + "[^.]*"  // Match anything except dots
                    i ← i + 1
                END IF

            CASE "?":
                result ← result + "[^.]"  // Match single character except dot
                i ← i + 1

            CASE ".":
                result ← result + "\\."  // Escape literal dot
                i ← i + 1

            DEFAULT:
                // Escape special regex characters
                IF SPECIAL_CHARS CONTAINS char THEN
                    result ← result + "\\" + char
                ELSE
                    result ← result + char
                END IF
                i ← i + 1
        END SWITCH
    END WHILE

    // Add anchor for end of string
    result ← result + "$"

    // Compile and return regex (case insensitive for HA compatibility)
    RETURN CompileRegex(result, flags: "i")
END


SUBROUTINE: ExtractDomainFromPattern
INPUT: pattern (string)
OUTPUT: domain (string or NULL)

BEGIN
    // Pattern format: {domain}.{rest}
    dot_index ← INDEX_OF(pattern, ".")

    IF dot_index = -1 THEN
        RETURN NULL  -- No domain separator found
    END IF

    domain_part ← SUBSTRING(pattern, 0, dot_index)

    // Check if domain part contains wildcards
    IF domain_part CONTAINS "*" OR domain_part CONTAINS "?" THEN
        RETURN NULL  -- Domain is a pattern, not fixed
    END IF

    RETURN domain_part
END
```

---

## Algorithm 2: Entity Pattern Matching

```
ALGORITHM: EntityPatternMatch
PURPOSE: Find the best matching schema for a HomeAssistant entity ID

INPUT:
  - entity_id: string (e.g., "sensor.airgradient_living_room_pm25")
  - patterns: List<SchemaPatternEntry> (sorted by priority)
  - cache: PatternCache (optional, for performance)

OUTPUT:
  - match_result: {
      matched: boolean,
      schema_name: string or NULL,
      pattern: string or NULL,
      confidence: float (0.0 - 1.0)
    }

BEGIN
    // Quick domain check for optimization
    entity_domain ← ExtractDomain(entity_id)

    FOR EACH entry IN patterns DO
        // Optimization: Skip if domains don't match
        IF entry.domain IS NOT NULL AND entry.domain != entity_domain THEN
            CONTINUE
        END IF

        // Get compiled regex (from cache or compile)
        regex ← GetOrCompileRegex(entry.pattern, cache)

        // Test match
        IF regex.test(entity_id) THEN
            // Calculate match confidence based on pattern specificity
            confidence ← CalculateMatchConfidence(entry.pattern, entity_id)

            RETURN {
                matched: true,
                schema_name: entry.schema_name,
                pattern: entry.pattern,
                confidence: confidence
            }
        END IF
    END FOR

    // No match found
    RETURN {
        matched: false,
        schema_name: NULL,
        pattern: NULL,
        confidence: 0.0
    }
END


SUBROUTINE: ExtractDomain
INPUT: entity_id (string)
OUTPUT: domain (string)

BEGIN
    dot_index ← INDEX_OF(entity_id, ".")
    IF dot_index = -1 THEN
        RETURN entity_id  -- Malformed, return as-is
    END IF
    RETURN SUBSTRING(entity_id, 0, dot_index)
END


SUBROUTINE: GetOrCompileRegex
INPUT: pattern (string), cache (PatternCache)
OUTPUT: RegExp

BEGIN
    IF cache IS NOT NULL AND cache.compiled_patterns HAS KEY pattern THEN
        cache.cache_hits ← cache.cache_hits + 1
        RETURN cache.compiled_patterns[pattern]
    END IF

    regex ← GlobToRegex(pattern)

    IF cache IS NOT NULL THEN
        cache.cache_misses ← cache.cache_misses + 1
        cache.compiled_patterns[pattern] ← regex
    END IF

    RETURN regex
END


SUBROUTINE: CalculateMatchConfidence
INPUT: pattern (string), entity_id (string)
OUTPUT: confidence (float 0.0 - 1.0)

PURPOSE: More specific patterns get higher confidence

BEGIN
    // Count wildcards in pattern
    wildcard_count ← COUNT(pattern, "*") + COUNT(pattern, "?")

    // Calculate specificity ratio
    // Patterns with fewer wildcards are more specific
    pattern_length ← LENGTH(pattern)
    entity_length ← LENGTH(entity_id)

    // Base confidence on how much of the pattern is literal
    literal_chars ← pattern_length - (wildcard_count * 1)  -- Each * counts as 1
    specificity ← literal_chars / entity_length

    // Clamp to 0.0 - 1.0
    RETURN MAX(0.0, MIN(1.0, specificity))
END
```

---

## Algorithm 3: Batch Entity Matching

```
ALGORITHM: BatchEntityMatch
PURPOSE: Match multiple entity IDs efficiently with pre-compiled patterns

INPUT:
  - entity_ids: List<string>
  - patterns: List<SchemaPatternEntry>

OUTPUT:
  - match_results: Map<entity_id, match_result>
  - summary: {
      matched_count: integer,
      unmatched_count: integer,
      by_schema: Map<schema_name, count>
    }

BEGIN
    // Initialize cache for this batch
    cache ← CreatePatternCache()

    // Pre-compile all patterns
    FOR EACH entry IN patterns DO
        cache.compiled_patterns[entry.pattern] ← GlobToRegex(entry.pattern)
    END FOR

    // Sort patterns by priority (higher first)
    sorted_patterns ← SORT(patterns BY priority DESC)

    // Group entities by domain for optimization
    entities_by_domain ← GROUP(entity_ids BY ExtractDomain)

    // Initialize results
    results ← {}
    summary ← {
        matched_count: 0,
        unmatched_count: 0,
        by_schema: {}
    }

    // Process each domain group
    FOR EACH (domain, domain_entities) IN entities_by_domain DO
        // Filter patterns applicable to this domain
        applicable_patterns ← FILTER sorted_patterns WHERE
            entry.domain IS NULL OR entry.domain = domain

        // Match each entity in domain
        FOR EACH entity_id IN domain_entities DO
            match ← EntityPatternMatch(entity_id, applicable_patterns, cache)
            results[entity_id] ← match

            IF match.matched THEN
                summary.matched_count ← summary.matched_count + 1
                summary.by_schema[match.schema_name] ←
                    (summary.by_schema[match.schema_name] OR 0) + 1
            ELSE
                summary.unmatched_count ← summary.unmatched_count + 1
            END IF
        END FOR
    END FOR

    RETURN (results, summary)
END
```

---

## Algorithm 4: Pattern Priority Resolution

```
ALGORITHM: ResolvePatternPriority
PURPOSE: Determine pattern evaluation order when multiple patterns could match

INPUT:
  - patterns: List<SchemaPatternEntry>

OUTPUT:
  - sorted_patterns: List<SchemaPatternEntry> (in evaluation order)

PRIORITY_RULES:
  1. Explicit priority field (if set)
  2. Fewer wildcards = higher priority
  3. Longer literal prefix = higher priority
  4. Domain-specific patterns before domain-wildcards
  5. Alphabetical order (tiebreaker)

BEGIN
    // Calculate priority score for each pattern
    scored_patterns ← []

    FOR EACH entry IN patterns DO
        score ← CalculatePriorityScore(entry)
        scored_patterns.append({entry: entry, score: score})
    END FOR

    // Sort by score descending, then by schema_name ascending
    sorted ← SORT(scored_patterns BY score DESC, entry.schema_name ASC)

    RETURN [item.entry FOR item IN sorted]
END


SUBROUTINE: CalculatePriorityScore
INPUT: entry (SchemaPatternEntry)
OUTPUT: score (integer)

BEGIN
    // Start with explicit priority (multiplied to be significant)
    score ← entry.priority * 1000

    pattern ← entry.pattern

    // Factor 1: Fewer wildcards = higher score
    wildcard_count ← COUNT(pattern, "*") + COUNT(pattern, "?")
    score ← score + (100 - wildcard_count * 10)

    // Factor 2: Has fixed domain = higher score
    IF entry.domain IS NOT NULL THEN
        score ← score + 50
    END IF

    // Factor 3: Longer literal prefix = higher score
    literal_prefix_length ← 0
    FOR i FROM 0 TO LENGTH(pattern) - 1 DO
        IF pattern[i] IN ["*", "?", "["] THEN
            BREAK
        END IF
        literal_prefix_length ← literal_prefix_length + 1
    END FOR
    score ← score + literal_prefix_length

    // Factor 4: Pattern length (longer = more specific usually)
    score ← score + LENGTH(pattern) / 10

    RETURN score
END
```

---

## Algorithm 5: Pattern Validation

```
ALGORITHM: ValidatePattern
PURPOSE: Validate that a pattern is syntactically correct and usable

INPUT:
  - pattern: string

OUTPUT:
  - validation_result: {
      valid: boolean,
      errors: List<string>,
      warnings: List<string>,
      normalized_pattern: string
    }

BEGIN
    result ← {
        valid: true,
        errors: [],
        warnings: [],
        normalized_pattern: pattern
    }

    // Check 1: Pattern is not empty
    IF pattern IS EMPTY OR pattern IS NULL THEN
        result.valid ← false
        result.errors.append("Pattern cannot be empty")
        RETURN result
    END IF

    // Check 2: Pattern has valid domain separator
    IF NOT pattern CONTAINS "." THEN
        result.warnings.append("Pattern has no domain separator - will match all domains")
    END IF

    // Check 3: Balanced brackets
    bracket_depth ← 0
    FOR i FROM 0 TO LENGTH(pattern) - 1 DO
        IF pattern[i] = "[" THEN
            bracket_depth ← bracket_depth + 1
        ELSE IF pattern[i] = "]" THEN
            bracket_depth ← bracket_depth - 1
        END IF

        IF bracket_depth < 0 THEN
            result.valid ← false
            result.errors.append("Unbalanced brackets: unexpected ] at position " + i)
        END IF
    END FOR

    IF bracket_depth > 0 THEN
        result.valid ← false
        result.errors.append("Unbalanced brackets: missing closing ]")
    END IF

    // Check 4: No empty brackets
    IF pattern CONTAINS "[]" THEN
        result.valid ← false
        result.errors.append("Empty character class [] is invalid")
    END IF

    // Check 5: Try to compile regex
    TRY
        GlobToRegex(pattern)
    CATCH RegexError AS e
        result.valid ← false
        result.errors.append("Pattern produces invalid regex: " + e.message)
    END TRY

    // Check 6: Warn about overly broad patterns
    IF pattern = "*" OR pattern = "**" OR pattern = "*.*" THEN
        result.warnings.append("Pattern is very broad and will match many entities")
    END IF

    // Normalize pattern (trim whitespace)
    result.normalized_pattern ← TRIM(pattern)

    RETURN result
END
```

---

## Complexity Analysis

### GlobToRegex

| Operation | Time Complexity | Space Complexity |
|-----------|-----------------|------------------|
| Character iteration | O(n) | O(n) |
| Regex compilation | O(n) | O(n) |
| **Total** | **O(n)** | **O(n)** |

Where n = pattern length

### EntityPatternMatch

| Operation | Time Complexity | Space Complexity |
|-----------|-----------------|------------------|
| Domain extraction | O(1) | O(1) |
| Pattern iteration | O(p) | O(1) |
| Regex matching | O(m) per pattern | O(1) |
| **Total** | **O(p * m)** | **O(1)** |

Where:
- p = number of patterns
- m = length of entity_id

### BatchEntityMatch

| Operation | Time Complexity | Space Complexity |
|-----------|-----------------|------------------|
| Pre-compile patterns | O(p * n) | O(p * n) |
| Group by domain | O(e) | O(e) |
| Match all entities | O(e * p * m) | O(e) |
| **Total** | **O(e * p * m)** | **O(p * n + e)** |

Where:
- e = number of entity_ids
- p = number of patterns
- n = average pattern length
- m = average entity_id length

---

## Caching Strategy

### Cache Structure

```
STRUCTURE: GlobalPatternCache
FIELDS:
  - compiled_patterns: LRU_Map<pattern_string, RegExp>
    - max_size: 1000
    - ttl: 3600 seconds

  - pattern_index: Map<domain, List<SchemaPatternEntry>>
    - Pre-sorted by priority
    - Rebuilt on pattern changes

  - match_cache: LRU_Map<entity_id, match_result>
    - max_size: 10000
    - ttl: 300 seconds
    - Invalidated when patterns change

CACHE_POLICY:
  - Compile regex lazily on first use
  - Pre-compute domain index on startup
  - Invalidate match cache when patterns updated
  - Use weak references for regex objects (GC friendly)
```

### Cache Invalidation

```
ALGORITHM: InvalidatePatternCache
TRIGGER: When entity_schemas are updated via sync

BEGIN
    // Clear compiled patterns for affected schema
    FOR EACH changed_pattern IN changes DO
        cache.compiled_patterns.remove(changed_pattern)
    END FOR

    // Rebuild domain index
    cache.pattern_index ← BuildDomainIndex(all_patterns)

    // Clear match cache (entity matches may have changed)
    cache.match_cache.clear()

    // Log invalidation
    Log.info("Pattern cache invalidated: " + LENGTH(changes) + " patterns changed")
END
```

---

## Worked Examples

### Example 1: AirGradient Sensor Matching

**Pattern Definition:**
```yaml
entity_schemas:
  - schema_name: "sensor.airgradient_*"
    pattern: "sensor.airgradient_*"
    device_class: "air_quality"
    attributes:
      - name: "pm25"
        type: "float"
```

**Test Cases:**

| Entity ID | Matches? | Schema Name |
|-----------|----------|-------------|
| `sensor.airgradient_living_room_pm25` | Yes | sensor.airgradient_* |
| `sensor.airgradient_bedroom` | Yes | sensor.airgradient_* |
| `sensor.temperature_outside` | No | - |
| `binary_sensor.airgradient_status` | No | - (domain mismatch) |

**Regex Generated:** `^sensor\.airgradient_[^.]*$`

### Example 2: Window Sensors

**Pattern Definition:**
```yaml
entity_schemas:
  - schema_name: "binary_sensor.*_window*"
    pattern: "binary_sensor.*_window*"
    device_class: "window"
```

**Test Cases:**

| Entity ID | Matches? | Confidence |
|-----------|----------|------------|
| `binary_sensor.front_door_window` | Yes | 0.75 |
| `binary_sensor.bedroom_window_1` | Yes | 0.70 |
| `binary_sensor.garage_window_sensor` | Yes | 0.65 |
| `sensor.window_temperature` | No | - |

**Regex Generated:** `^binary_sensor\.[^.]*_window[^.]*$`

### Example 3: Priority Resolution

**Multiple Patterns:**
```yaml
entity_schemas:
  - schema_name: "sensor.*"
    pattern: "sensor.*"
    priority: 0

  - schema_name: "sensor.temperature_*"
    pattern: "sensor.temperature_*"
    priority: 10

  - schema_name: "sensor.temperature_living_room"
    pattern: "sensor.temperature_living_room"
    priority: 100
```

**Entity ID:** `sensor.temperature_living_room`

**Evaluation Order:**
1. `sensor.temperature_living_room` (priority 100) - **MATCH**
2. `sensor.temperature_*` (priority 10) - Not evaluated
3. `sensor.*` (priority 0) - Not evaluated

**Result:** Matched to `sensor.temperature_living_room` with confidence 1.0

---

## Integration with Data Quality Detection

The pattern matching system feeds into data quality detection:

```
WORKFLOW: Entity Classification

1. Bronze Parquet Data
   ↓
2. Extract unique entity_ids
   ↓
3. BatchEntityMatch(entity_ids, patterns)
   ↓
4. Separate into:
   - KNOWN: Matched entities → validate attributes
   - UNKNOWN: Unmatched entities → DQ dashboard "Unknown Entities" panel
   ↓
5. For UNKNOWN, run pattern suggestion algorithm
```

See `DATA_QUALITY_DETECTION.md` for continuation of this workflow.
