# DATA_QUALITY_DETECTION.md - Data Quality Issue Detection

## Overview

This document defines the pseudocode for detecting data quality issues in HomeAssistant data by comparing actual Bronze layer data against the entity_schemas data dictionary. The system identifies unknown entities, missing attributes, extra attributes, and suggests patterns for unclassified entities.

---

## Data Quality Issue Types

| Issue Type | Description | Severity |
|------------|-------------|----------|
| Unknown Entity | Entity ID doesn't match any pattern | Warning |
| Missing Attribute | Expected attribute not present in data | Error |
| Extra Attribute | Attribute present but not in schema | Info |
| Type Mismatch | Attribute value doesn't match declared type | Error |
| Null Violation | NULL value where nullable=false | Error |
| Stale Entity | No data received in expected time window | Warning |

---

## Data Structures

### Bronze Data Sample

```
STRUCTURE: BronzeRecord
FIELDS:
  - timestamp: datetime
  - entity_id: string
  - state: string
  - attributes: JSON object
  - stream_id: string

SOURCE: Parquet files in /data/parquet/{stream_id}/
```

### Data Quality Report

```
STRUCTURE: DataQualityReport
FIELDS:
  - report_id: UUID
  - generated_at: timestamp
  - time_window: {start: timestamp, end: timestamp}
  - stream_id: string

  - coverage: {
      total_entities: integer,
      matched_entities: integer,
      unknown_entities: integer,
      coverage_percentage: float
    }

  - issues: List<DataQualityIssue>

  - suggestions: List<PatternSuggestion>
```

### Data Quality Issue

```
STRUCTURE: DataQualityIssue
FIELDS:
  - issue_id: UUID
  - issue_type: IssueType enum
  - severity: "info" | "warning" | "error"
  - entity_id: string
  - schema_name: string (or NULL for unknown)
  - attribute: string (or NULL)
  - expected: string (for mismatches)
  - actual: string (for mismatches)
  - sample_count: integer
  - first_seen: timestamp
  - last_seen: timestamp
  - message: string
```

---

## Algorithm 1: Detect Unknown Entities

```
ALGORITHM: DetectUnknownEntities
PURPOSE: Identify entities in Bronze data that don't match any defined schema pattern

INPUT:
  - bronze_data_path: Path to Parquet files
  - data_dictionary: List of SchemaPatternEntry from TimescaleDB
  - time_window: {start: timestamp, end: timestamp}
  - stream_id: string (filter for specific stream, or "*" for all)

OUTPUT:
  - unknown_report: {
      unknown_entities: List<UnknownEntity>,
      coverage_percentage: float,
      by_domain: Map<domain, count>,
      suggested_patterns: List<PatternSuggestion>
    }

BEGIN
    // Step 1: Load unique entity_ids from Bronze data
    entity_ids ← QueryBronzeUniqueEntities(bronze_data_path, stream_id, time_window)

    IF entity_ids IS EMPTY THEN
        RETURN {
            unknown_entities: [],
            coverage_percentage: 100.0,
            by_domain: {},
            suggested_patterns: []
        }
    END IF

    // Step 2: Load patterns from data dictionary
    patterns ← LoadPatternsFromDictionary(data_dictionary)

    // Step 3: Batch match all entities
    (match_results, summary) ← BatchEntityMatch(entity_ids, patterns)

    // Step 4: Collect unknown entities
    unknown_list ← []
    FOR EACH (entity_id, result) IN match_results DO
        IF NOT result.matched THEN
            // Get additional context from Bronze data
            context ← GetEntityContext(bronze_data_path, entity_id, LIMIT 5)

            unknown ← {
                entity_id: entity_id,
                domain: ExtractDomain(entity_id),
                sample_attributes: ExtractAttributeKeys(context),
                sample_count: context.count,
                first_seen: MIN(context.timestamps),
                last_seen: MAX(context.timestamps)
            }
            unknown_list.append(unknown)
        END IF
    END FOR

    // Step 5: Group by domain for analysis
    by_domain ← GROUP_AND_COUNT(unknown_list BY domain)

    // Step 6: Generate pattern suggestions
    suggestions ← SuggestPatternsForUnknown(unknown_list)

    // Step 7: Calculate coverage
    total ← LENGTH(entity_ids)
    matched ← total - LENGTH(unknown_list)
    coverage_pct ← (matched / total) * 100

    RETURN {
        unknown_entities: unknown_list,
        coverage_percentage: coverage_pct,
        by_domain: by_domain,
        suggested_patterns: suggestions
    }
END


SUBROUTINE: QueryBronzeUniqueEntities
INPUT: path, stream_id, time_window
OUTPUT: Set<entity_id>

BEGIN
    // Use DuckDB or Parquet direct query
    query ← """
        SELECT DISTINCT entity_id
        FROM read_parquet('{path}/**/*.parquet')
        WHERE timestamp >= '{time_window.start}'
          AND timestamp < '{time_window.end}'
          AND ({stream_id} = '*' OR stream_id = '{stream_id}')
    """
    RETURN EXECUTE(query)
END


SUBROUTINE: GetEntityContext
INPUT: path, entity_id, limit
OUTPUT: List of sample records

BEGIN
    query ← """
        SELECT timestamp, entity_id, state, attributes
        FROM read_parquet('{path}/**/*.parquet')
        WHERE entity_id = '{entity_id}'
        ORDER BY timestamp DESC
        LIMIT {limit}
    """
    RETURN EXECUTE(query)
END


SUBROUTINE: ExtractAttributeKeys
INPUT: records (List of Bronze records)
OUTPUT: Set<attribute_name>

BEGIN
    all_keys ← SET()
    FOR EACH record IN records DO
        IF record.attributes IS JSON OBJECT THEN
            all_keys ← all_keys UNION KEYS(record.attributes)
        END IF
    END FOR
    RETURN all_keys
END
```

---

## Algorithm 2: Detect Missing Attributes

```
ALGORITHM: DetectMissingAttributes
PURPOSE: Find entities that match a schema but are missing expected attributes

INPUT:
  - bronze_data_path: Path to Parquet files
  - data_dictionary: Data dictionary with expected attributes
  - time_window: {start: timestamp, end: timestamp}
  - sample_size: Number of records to sample per entity (default: 100)

OUTPUT:
  - missing_report: {
      entities_with_issues: List<EntityAttributeIssue>,
      by_schema: Map<schema_name, List<missing_attributes>>,
      total_issues: integer
    }

BEGIN
    // Step 1: Load patterns and their expected attributes
    schema_expectations ← LoadSchemaExpectations(data_dictionary)
    // Map<schema_name, Set<required_attribute_names>>

    // Step 2: Get matched entities with their schemas
    entity_ids ← QueryBronzeUniqueEntities(bronze_data_path, "*", time_window)
    patterns ← LoadPatternsFromDictionary(data_dictionary)
    (match_results, _) ← BatchEntityMatch(entity_ids, patterns)

    // Filter to only matched entities
    matched_entities ← FILTER match_results WHERE result.matched = true

    // Step 3: For each matched entity, check attributes
    issues ← []

    FOR EACH (entity_id, match_result) IN matched_entities DO
        schema_name ← match_result.schema_name
        expected_attrs ← schema_expectations[schema_name]

        IF expected_attrs IS EMPTY THEN
            CONTINUE  -- Schema has no defined attributes
        END IF

        // Sample actual attributes from Bronze data
        samples ← QueryEntityAttributes(bronze_data_path, entity_id, sample_size)
        actual_attrs ← UnionAllAttributeKeys(samples)

        // Find missing attributes
        missing ← expected_attrs - actual_attrs

        IF missing IS NOT EMPTY THEN
            // Check if attributes are always missing or intermittently
            missing_analysis ← AnalyzeMissingPattern(samples, missing)

            FOR EACH attr IN missing DO
                issue ← {
                    issue_type: "MISSING_ATTRIBUTE",
                    severity: DetermineSeverity(attr, missing_analysis),
                    entity_id: entity_id,
                    schema_name: schema_name,
                    attribute: attr,
                    expected: "present",
                    actual: "missing",
                    missing_rate: missing_analysis[attr].missing_rate,
                    sample_count: LENGTH(samples),
                    message: "Attribute '" + attr + "' expected by schema but not found"
                }
                issues.append(issue)
            END FOR
        END IF
    END FOR

    // Step 4: Aggregate by schema
    by_schema ← GROUP(issues BY schema_name, COLLECT attribute)

    RETURN {
        entities_with_issues: issues,
        by_schema: by_schema,
        total_issues: LENGTH(issues)
    }
END


SUBROUTINE: QueryEntityAttributes
INPUT: path, entity_id, limit
OUTPUT: List of attribute JSON objects

BEGIN
    query ← """
        SELECT attributes
        FROM read_parquet('{path}/**/*.parquet')
        WHERE entity_id = '{entity_id}'
        ORDER BY timestamp DESC
        LIMIT {limit}
    """
    RETURN EXECUTE(query)
END


SUBROUTINE: AnalyzeMissingPattern
INPUT: samples (List), missing_attrs (Set)
OUTPUT: Map<attr, {missing_rate, first_missing, last_present}>

BEGIN
    analysis ← {}

    FOR EACH attr IN missing_attrs DO
        present_count ← 0
        missing_count ← 0
        first_missing ← NULL
        last_present ← NULL

        FOR EACH sample IN samples DO
            IF attr IN KEYS(sample.attributes) THEN
                present_count ← present_count + 1
                last_present ← sample.timestamp
            ELSE
                missing_count ← missing_count + 1
                IF first_missing IS NULL THEN
                    first_missing ← sample.timestamp
                END IF
            END IF
        END FOR

        analysis[attr] ← {
            missing_rate: missing_count / (present_count + missing_count),
            first_missing: first_missing,
            last_present: last_present,
            always_missing: present_count = 0
        }
    END FOR

    RETURN analysis
END


SUBROUTINE: DetermineSeverity
INPUT: attr_name, missing_analysis
OUTPUT: "info" | "warning" | "error"

BEGIN
    rate ← missing_analysis[attr_name].missing_rate

    IF missing_analysis[attr_name].always_missing THEN
        // Attribute never seen - might be schema error
        RETURN "error"
    ELSE IF rate > 0.5 THEN
        // Missing more than half the time
        RETURN "warning"
    ELSE
        // Occasionally missing
        RETURN "info"
    END IF
END
```

---

## Algorithm 3: Detect Extra Attributes

```
ALGORITHM: DetectExtraAttributes
PURPOSE: Find attributes in data that aren't defined in the schema

INPUT:
  - bronze_data_path: Path to Parquet files
  - data_dictionary: Data dictionary with expected attributes
  - time_window: {start: timestamp, end: timestamp}

OUTPUT:
  - extra_report: {
      extra_attributes: List<ExtraAttribute>,
      by_schema: Map<schema_name, Set<extra_attr_names>>,
      potential_additions: List<AttributeSuggestion>
    }

BEGIN
    // Step 1: Load schema expectations
    schema_expectations ← LoadSchemaExpectations(data_dictionary)

    // Step 2: Get matched entities
    entity_ids ← QueryBronzeUniqueEntities(bronze_data_path, "*", time_window)
    patterns ← LoadPatternsFromDictionary(data_dictionary)
    (match_results, _) ← BatchEntityMatch(entity_ids, patterns)

    matched_entities ← FILTER match_results WHERE result.matched = true

    // Step 3: Collect all actual attributes per schema
    actual_by_schema ← {}

    FOR EACH (entity_id, match_result) IN matched_entities DO
        schema_name ← match_result.schema_name

        // Sample attributes
        samples ← QueryEntityAttributes(bronze_data_path, entity_id, 10)
        attrs ← UnionAllAttributeKeys(samples)

        IF schema_name NOT IN actual_by_schema THEN
            actual_by_schema[schema_name] ← {
                attributes: SET(),
                entities: SET()
            }
        END IF

        actual_by_schema[schema_name].attributes ←
            actual_by_schema[schema_name].attributes UNION attrs
        actual_by_schema[schema_name].entities.add(entity_id)
    END FOR

    // Step 4: Find extra attributes
    extra_list ← []
    suggestions ← []

    FOR EACH (schema_name, data) IN actual_by_schema DO
        expected ← schema_expectations[schema_name] OR SET()
        actual ← data.attributes

        extra ← actual - expected

        FOR EACH attr IN extra DO
            // Analyze the extra attribute
            type_hint ← InferAttributeType(bronze_data_path, data.entities, attr)

            extra_entry ← {
                schema_name: schema_name,
                attribute: attr,
                found_in_entities: LENGTH(data.entities),
                inferred_type: type_hint.type,
                sample_values: type_hint.samples
            }
            extra_list.append(extra_entry)

            // Suggest adding to schema if found in most entities
            IF type_hint.coverage > 0.8 THEN
                suggestions.append({
                    schema_name: schema_name,
                    attribute: attr,
                    suggested_type: type_hint.type,
                    suggested_unit: type_hint.unit,
                    confidence: type_hint.coverage,
                    reason: "Found in " + (type_hint.coverage * 100) + "% of entities"
                })
            END IF
        END FOR
    END FOR

    // Step 5: Group by schema
    by_schema ← GROUP(extra_list BY schema_name, COLLECT attribute)

    RETURN {
        extra_attributes: extra_list,
        by_schema: by_schema,
        potential_additions: suggestions
    }
END


SUBROUTINE: InferAttributeType
INPUT: path, entity_ids, attr_name
OUTPUT: {type, unit, samples, coverage}

BEGIN
    // Sample values across entities
    values ← []
    entities_with_attr ← 0

    FOR EACH entity_id IN entity_ids (LIMIT 20) DO
        sample ← QuerySingleAttributeValue(path, entity_id, attr_name)
        IF sample IS NOT NULL THEN
            values.append(sample)
            entities_with_attr ← entities_with_attr + 1
        END IF
    END FOR

    // Infer type from values
    type ← InferTypeFromValues(values)
    unit ← InferUnitFromValues(values, attr_name)
    coverage ← entities_with_attr / MIN(20, LENGTH(entity_ids))

    RETURN {
        type: type,
        unit: unit,
        samples: values[0:5],
        coverage: coverage
    }
END


SUBROUTINE: InferTypeFromValues
INPUT: values (List)
OUTPUT: type_string

BEGIN
    IF values IS EMPTY THEN
        RETURN "unknown"
    END IF

    // Check if all values are same type
    types ← SET()
    FOR EACH v IN values DO
        IF v IS NULL THEN
            CONTINUE
        ELSE IF v IS BOOLEAN THEN
            types.add("boolean")
        ELSE IF v IS INTEGER THEN
            types.add("integer")
        ELSE IF v IS FLOAT THEN
            types.add("float")
        ELSE IF v IS STRING THEN
            types.add("string")
        ELSE IF v IS ARRAY THEN
            types.add("array")
        ELSE IF v IS OBJECT THEN
            types.add("object")
        END IF
    END FOR

    IF LENGTH(types) = 1 THEN
        RETURN FIRST(types)
    ELSE IF types CONTAINS "float" AND types CONTAINS "integer" THEN
        RETURN "float"  -- Promote to float
    ELSE
        RETURN "mixed"
    END IF
END
```

---

## Algorithm 4: Pattern Suggestion for Unknown Entities

```
ALGORITHM: SuggestPatternsForUnknown
PURPOSE: Analyze unknown entities and suggest glob patterns to classify them

INPUT:
  - unknown_entities: List<UnknownEntity>

OUTPUT:
  - suggestions: List<PatternSuggestion>

BEGIN
    suggestions ← []

    // Step 1: Group by domain
    by_domain ← GROUP(unknown_entities BY domain)

    FOR EACH (domain, entities) IN by_domain DO
        // Step 2: Extract object_ids (part after domain.)
        object_ids ← [ExtractObjectId(e.entity_id) FOR e IN entities]

        // Step 3: Find common prefixes
        prefix_groups ← FindCommonPrefixes(object_ids, min_group_size: 3)

        FOR EACH (prefix, grouped_ids) IN prefix_groups DO
            // Step 4: Analyze attribute consistency
            common_attrs ← FindCommonAttributes(entities WHERE id IN grouped_ids)

            // Step 5: Generate pattern suggestion
            IF LENGTH(grouped_ids) >= 3 THEN
                pattern ← domain + "." + prefix + "*"

                suggestion ← {
                    suggested_pattern: pattern,
                    matched_count: LENGTH(grouped_ids),
                    example_entities: grouped_ids[0:5],
                    common_attributes: common_attrs,
                    confidence: CalculatePatternConfidence(grouped_ids, common_attrs),
                    suggested_schema: {
                        schema_name: pattern,
                        device_class: InferDeviceClass(domain, prefix),
                        attributes: GenerateAttributeDefinitions(common_attrs)
                    }
                }
                suggestions.append(suggestion)
            END IF
        END FOR

        // Step 6: Handle remaining ungrouped entities
        ungrouped ← entities NOT IN any prefix_group
        IF LENGTH(ungrouped) > 10 THEN
            // Suggest a catch-all pattern
            suggestions.append({
                suggested_pattern: domain + ".*",
                matched_count: LENGTH(ungrouped),
                example_entities: [e.entity_id FOR e IN ungrouped][0:5],
                common_attributes: [],
                confidence: 0.3,
                note: "Catch-all pattern for remaining " + domain + " entities"
            })
        END IF
    END FOR

    // Sort by confidence descending
    RETURN SORT(suggestions BY confidence DESC)
END


SUBROUTINE: FindCommonPrefixes
INPUT: object_ids (List<string>), min_group_size (integer)
OUTPUT: Map<prefix, List<object_id>>

BEGIN
    prefix_counts ← {}

    // Try different prefix lengths
    FOR length FROM 3 TO 20 DO
        FOR EACH id IN object_ids DO
            IF LENGTH(id) >= length THEN
                prefix ← SUBSTRING(id, 0, length)
                // Stop at underscore or dash boundaries
                boundary ← FindLastBoundary(prefix, ["_", "-"])
                IF boundary > 2 THEN
                    prefix ← SUBSTRING(prefix, 0, boundary + 1)
                END IF

                IF prefix NOT IN prefix_counts THEN
                    prefix_counts[prefix] ← []
                END IF
                prefix_counts[prefix].append(id)
            END IF
        END FOR
    END FOR

    // Filter to meaningful groups
    result ← {}
    FOR EACH (prefix, ids) IN prefix_counts DO
        // Remove duplicate IDs
        unique_ids ← SET(ids)
        IF LENGTH(unique_ids) >= min_group_size THEN
            // Check this prefix is not subset of larger match
            is_subset ← false
            FOR EACH (other_prefix, other_ids) IN result DO
                IF prefix STARTS_WITH other_prefix AND LENGTH(other_ids) >= LENGTH(unique_ids) THEN
                    is_subset ← true
                    BREAK
                END IF
            END FOR

            IF NOT is_subset THEN
                result[prefix] ← LIST(unique_ids)
            END IF
        END IF
    END FOR

    RETURN result
END


SUBROUTINE: FindCommonAttributes
INPUT: entities (List<UnknownEntity>)
OUTPUT: List<{name, type, frequency}>

BEGIN
    attr_counts ← {}
    total_entities ← LENGTH(entities)

    FOR EACH entity IN entities DO
        FOR EACH attr IN entity.sample_attributes DO
            IF attr NOT IN attr_counts THEN
                attr_counts[attr] ← {count: 0, types: []}
            END IF
            attr_counts[attr].count ← attr_counts[attr].count + 1
        END FOR
    END FOR

    // Return attributes present in >50% of entities
    common ← []
    FOR EACH (attr, data) IN attr_counts DO
        frequency ← data.count / total_entities
        IF frequency > 0.5 THEN
            common.append({
                name: attr,
                frequency: frequency,
                type: "unknown"  -- Type inference would happen separately
            })
        END IF
    END FOR

    RETURN SORT(common BY frequency DESC)
END


SUBROUTINE: CalculatePatternConfidence
INPUT: grouped_ids, common_attrs
OUTPUT: confidence (float 0.0 - 1.0)

BEGIN
    // Factors:
    // 1. Group size (more entities = higher confidence)
    size_factor ← MIN(1.0, LENGTH(grouped_ids) / 20)

    // 2. Attribute consistency (more common attrs = higher confidence)
    attr_factor ← MIN(1.0, LENGTH(common_attrs) / 5)

    // 3. Naming consistency (similar length IDs = higher confidence)
    lengths ← [LENGTH(id) FOR id IN grouped_ids]
    length_variance ← VARIANCE(lengths)
    length_factor ← 1.0 / (1.0 + length_variance)

    // Weighted average
    confidence ← (size_factor * 0.4) + (attr_factor * 0.4) + (length_factor * 0.2)

    RETURN confidence
END


SUBROUTINE: InferDeviceClass
INPUT: domain, prefix
OUTPUT: device_class (string) or NULL

BEGIN
    // Known HomeAssistant device class patterns
    patterns ← {
        "temperature": ["temp", "temperature", "thermo"],
        "humidity": ["humid", "rh", "moisture"],
        "battery": ["battery", "bat"],
        "motion": ["motion", "pir", "presence"],
        "door": ["door", "entry"],
        "window": ["window", "contact"],
        "air_quality": ["airgradient", "aq", "pm25", "pm10", "aqi"],
        "power": ["power", "watt", "energy"],
        "light": ["light", "lux", "illumin"]
    }

    prefix_lower ← LOWERCASE(prefix)

    FOR EACH (device_class, keywords) IN patterns DO
        FOR EACH keyword IN keywords DO
            IF prefix_lower CONTAINS keyword THEN
                RETURN device_class
            END IF
        END FOR
    END FOR

    RETURN NULL
END
```

---

## Algorithm 5: Comprehensive Data Quality Report

```
ALGORITHM: GenerateDataQualityReport
PURPOSE: Create comprehensive data quality report combining all detection algorithms

INPUT:
  - bronze_data_path: Path to Parquet files
  - data_dictionary: TimescaleDB data dictionary
  - time_window: {start: timestamp, end: timestamp}
  - stream_id: string (or "*" for all)
  - options: {
      check_unknown: boolean (default: true),
      check_missing: boolean (default: true),
      check_extra: boolean (default: true),
      suggest_patterns: boolean (default: true),
      sample_size: integer (default: 100)
    }

OUTPUT:
  - report: DataQualityReport

BEGIN
    report ← {
        report_id: GenerateUUID(),
        generated_at: NOW(),
        time_window: time_window,
        stream_id: stream_id,
        coverage: NULL,
        issues: [],
        suggestions: []
    }

    // Step 1: Run unknown entity detection
    IF options.check_unknown THEN
        unknown_report ← DetectUnknownEntities(
            bronze_data_path,
            data_dictionary,
            time_window,
            stream_id
        )

        report.coverage ← {
            total_entities: unknown_report.matched + LENGTH(unknown_report.unknown_entities),
            matched_entities: unknown_report.matched,
            unknown_entities: LENGTH(unknown_report.unknown_entities),
            coverage_percentage: unknown_report.coverage_percentage
        }

        // Add unknown entity issues
        FOR EACH unknown IN unknown_report.unknown_entities DO
            report.issues.append({
                issue_type: "UNKNOWN_ENTITY",
                severity: "warning",
                entity_id: unknown.entity_id,
                schema_name: NULL,
                attribute: NULL,
                sample_count: unknown.sample_count,
                first_seen: unknown.first_seen,
                last_seen: unknown.last_seen,
                message: "Entity does not match any defined schema pattern"
            })
        END FOR

        // Add pattern suggestions
        IF options.suggest_patterns THEN
            report.suggestions ← unknown_report.suggested_patterns
        END IF
    END IF

    // Step 2: Run missing attribute detection
    IF options.check_missing THEN
        missing_report ← DetectMissingAttributes(
            bronze_data_path,
            data_dictionary,
            time_window,
            options.sample_size
        )

        report.issues ← report.issues + missing_report.entities_with_issues
    END IF

    // Step 3: Run extra attribute detection
    IF options.check_extra THEN
        extra_report ← DetectExtraAttributes(
            bronze_data_path,
            data_dictionary,
            time_window
        )

        FOR EACH extra IN extra_report.extra_attributes DO
            report.issues.append({
                issue_type: "EXTRA_ATTRIBUTE",
                severity: "info",
                entity_id: NULL,  -- Applies to schema, not specific entity
                schema_name: extra.schema_name,
                attribute: extra.attribute,
                sample_count: extra.found_in_entities,
                message: "Attribute found in data but not defined in schema"
            })
        END FOR

        // Add attribute addition suggestions
        report.suggestions ← report.suggestions + extra_report.potential_additions
    END IF

    // Step 4: Calculate summary statistics
    report.summary ← {
        total_issues: LENGTH(report.issues),
        by_severity: GroupAndCount(report.issues BY severity),
        by_type: GroupAndCount(report.issues BY issue_type),
        top_affected_schemas: TopN(report.issues BY schema_name, 10),
        top_affected_entities: TopN(report.issues BY entity_id, 10)
    }

    RETURN report
END
```

---

## Complexity Analysis

### DetectUnknownEntities

| Operation | Time Complexity | Space Complexity |
|-----------|-----------------|------------------|
| Query unique entities | O(n) scan | O(e) entities |
| Load patterns | O(p) | O(p) |
| Batch match | O(e * p) | O(e) |
| Context queries | O(u * s) | O(u * s) |
| Pattern suggestion | O(u^2) | O(u) |
| **Total** | **O(n + e*p + u^2)** | **O(e + p + u*s)** |

Where:
- n = number of Bronze records
- e = unique entities
- p = number of patterns
- u = unknown entities
- s = sample size per entity

### DetectMissingAttributes

| Operation | Time Complexity | Space Complexity |
|-----------|-----------------|------------------|
| Load expectations | O(p * a) | O(p * a) |
| Entity matching | O(e * p) | O(e) |
| Attribute sampling | O(m * s) | O(m * a) |
| Analysis | O(m * a) | O(m * a) |
| **Total** | **O(e*p + m*s)** | **O(p*a + m*a)** |

Where:
- m = matched entities
- a = attributes per schema

### SuggestPatternsForUnknown

| Operation | Time Complexity | Space Complexity |
|-----------|-----------------|------------------|
| Group by domain | O(u) | O(u) |
| Find prefixes | O(u * L^2) | O(u * L) |
| Common attributes | O(u * a) | O(a) |
| **Total** | **O(u * L^2 + u * a)** | **O(u * L + a)** |

Where:
- L = max object_id length

---

## Worked Example

### Sample Bronze Data (HomeAssistant)

```
| timestamp | entity_id | state | attributes |
|-----------|-----------|-------|------------|
| 2024-01-01T10:00:00Z | sensor.airgradient_living_room_pm25 | 12.5 | {"unit": "ug/m3"} |
| 2024-01-01T10:00:00Z | sensor.airgradient_living_room_co2 | 450 | {"unit": "ppm"} |
| 2024-01-01T10:00:00Z | sensor.unknown_temperature | 22.5 | {"unit": "C"} |
| 2024-01-01T10:00:00Z | binary_sensor.front_door | on | {"device_class": "door"} |
| 2024-01-01T10:00:00Z | sensor.mystery_device_value | 42 | {} |
```

### Data Dictionary Patterns

```yaml
entity_schemas:
  - schema_name: "sensor.airgradient_*"
    pattern: "sensor.airgradient_*"
    attributes:
      - name: pm25
      - name: co2
      - name: temperature
```

### Expected Detection Results

**Unknown Entities:**
- `sensor.unknown_temperature` - No matching pattern
- `binary_sensor.front_door` - No matching pattern
- `sensor.mystery_device_value` - No matching pattern

**Pattern Suggestions:**
```
{
  suggested_pattern: "sensor.airgradient_*",
  matched_count: 2,
  confidence: 0.85
}
```

**Coverage Report:**
```
Total entities: 5
Matched: 2 (40%)
Unknown: 3 (60%)
```
