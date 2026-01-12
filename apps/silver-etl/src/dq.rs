//! DQ (Data Quality) rule SQL generation
//!
//! Generates SQL expressions for DQ rules that populate
//! the dq_flags TEXT[] column with transparency flags.
//!
//! ## Flag Format
//!
//! Flags follow the structured format: `{rule_type}:{field}:{reason}[:{value}]`
//!
//! Examples:
//! - `range_check:pm25:out_of_bounds`
//! - `null_check:observation_time:missing`
//! - `freshness_check:observation_time:stale`
//! - `cross_field_check:pm10_less_than_pm25`
//!
//! ## Actions
//!
//! - `Flag`: Keep value, add to dq_flags (default)
//! - `Reject`: Set to NULL, add to dq_flags
//! - `Clamp`: Adjust to bounds, add to dq_flags
//! - `Drop`: Filter row in WHERE clause

use neural_core::config::silver_etl::{DqAction, DqRule, SilverEtlConfig};

/// DQ SQL Generator
///
/// Generates SQL expressions for each DQ rule type:
/// - range_check: Validates numeric bounds
/// - null_check: Validates required fields
/// - enum_check: Validates against allowed values
/// - pattern_check: Validates against regex
/// - freshness_check: Validates timestamp recency
/// - rate_of_change: Validates delta between consecutive values
/// - cross_field_check: Validates relationships between fields
pub struct DqSqlGenerator {
    // Configuration could be added here in the future
}

impl DqSqlGenerator {
    /// Create a new DQ SQL generator
    pub fn new() -> Self {
        Self {}
    }

    /// Generate SQL CASE expression for a DQ rule that returns flag string or NULL
    ///
    /// # Arguments
    ///
    /// * `rule` - The DQ rule to generate SQL for
    ///
    /// # Returns
    ///
    /// SQL CASE expression that evaluates to flag string when violation occurs, NULL otherwise
    pub fn generate_check_sql(&self, rule: &DqRule) -> String {
        match rule {
            DqRule::RangeCheck {
                field,
                min,
                max,
                action,
                ..
            } => self.generate_range_check_sql(field, *min, *max, action),

            DqRule::NullCheck { field, .. } => self.generate_null_check_sql(field),

            DqRule::EnumCheck {
                field,
                allowed_values,
                case_sensitive,
                ..
            } => self.generate_enum_check_sql(field, allowed_values, *case_sensitive),

            DqRule::PatternCheck { field, pattern, .. } => {
                self.generate_pattern_check_sql(field, pattern)
            }

            DqRule::FreshnessCheck {
                field,
                max_age,
                max_future,
                reference,
                ..
            } => self.generate_freshness_check_sql(
                field,
                max_age.as_deref(),
                max_future.as_deref(),
                reference,
            ),

            DqRule::RateOfChange {
                field,
                max_change_per_minute,
                partition_by,
                ..
            } => self.generate_rate_of_change_sql(field, *max_change_per_minute, partition_by),

            DqRule::CrossFieldCheck {
                name,
                expression,
                message,
                ..
            } => self.generate_cross_field_check_sql(name, expression, message.as_deref()),

            // Batch-level rules don't generate row-level SQL
            DqRule::CompletenessCheck { .. }
            | DqRule::CardinalityCheck { .. }
            | DqRule::MonotonicCheck { .. }
            | DqRule::ConditionalCheck { .. } => {
                // Batch-level rules are handled separately
                String::new()
            }
        }
    }

    /// Generate SQL expression for the output value, applying the DQ action
    ///
    /// # Arguments
    ///
    /// * `rule` - The DQ rule containing the action
    /// * `raw_expr` - The raw SQL expression for the source value
    ///
    /// # Returns
    ///
    /// SQL expression for the (possibly transformed) output value
    pub fn generate_value_expr(&self, rule: &DqRule, raw_expr: &str) -> String {
        match rule {
            DqRule::RangeCheck {
                field,
                min,
                max,
                action,
                ..
            } => match action {
                DqAction::Clamp => {
                    let clamped = self.generate_clamp_expr(raw_expr, *min, *max);
                    format!("{} AS {}", clamped, field)
                }
                DqAction::Reject => {
                    let condition = self.generate_range_violation_condition(raw_expr, *min, *max);
                    format!(
                        r#"CASE
  WHEN {}
  THEN NULL
  ELSE {}
END AS {}"#,
                        condition, raw_expr, field
                    )
                }
                // Flag and other actions keep the original value
                _ => format!("{} AS {}", raw_expr, field),
            },

            DqRule::NullCheck { field, action, .. } => match action {
                DqAction::Reject | DqAction::Flag => {
                    // For null_check, value is already NULL if it fails
                    format!("{} AS {}", raw_expr, field)
                }
                _ => format!("{} AS {}", raw_expr, field),
            },

            _ => {
                // Other rules don't modify the value
                if let Some(field) = rule.field() {
                    format!("{} AS {}", raw_expr, field)
                } else {
                    raw_expr.to_string()
                }
            }
        }
    }

    /// Generate dq_flags array expression from SilverEtlConfig
    ///
    /// This method collects all DQ rules from:
    /// - Global dq_rules on the config
    /// - Per-field dq_rules on each field_mapping
    ///
    /// For per-field rules that don't specify a field, the field name is inherited
    /// from the parent field_mapping's target_column.
    ///
    /// # Arguments
    ///
    /// * `config` - Silver ETL configuration
    ///
    /// # Returns
    ///
    /// SQL expression that produces TEXT[] of all triggered flags
    pub fn generate_dq_flags_expr_from_config(&self, config: &SilverEtlConfig) -> String {
        // Collect all rules
        let mut all_rules = config.dq_rules.clone();

        // Add per-field rules, inheriting field name from parent mapping
        for mapping in &config.field_mappings {
            for rule in &mapping.dq_rules {
                let rule_with_field =
                    self.inherit_field_from_mapping(rule.clone(), &mapping.target_column);
                all_rules.push(rule_with_field);
            }
        }

        self.generate_dq_flags_expr(&all_rules)
    }

    /// Inherit field name from parent mapping if rule's field is empty
    fn inherit_field_from_mapping(&self, rule: DqRule, parent_field: &str) -> DqRule {
        match rule {
            DqRule::RangeCheck {
                field,
                min,
                max,
                action,
                clamp_to_bounds,
            } => DqRule::RangeCheck {
                field: if field.is_empty() {
                    parent_field.to_string()
                } else {
                    field
                },
                min,
                max,
                action,
                clamp_to_bounds,
            },
            DqRule::NullCheck { field, action } => DqRule::NullCheck {
                field: if field.is_empty() {
                    parent_field.to_string()
                } else {
                    field
                },
                action,
            },
            DqRule::EnumCheck {
                field,
                allowed_values,
                case_sensitive,
                action,
            } => DqRule::EnumCheck {
                field: if field.is_empty() {
                    parent_field.to_string()
                } else {
                    field
                },
                allowed_values,
                case_sensitive,
                action,
            },
            DqRule::PatternCheck {
                field,
                pattern,
                action,
            } => DqRule::PatternCheck {
                field: if field.is_empty() {
                    parent_field.to_string()
                } else {
                    field
                },
                pattern,
                action,
            },
            DqRule::FreshnessCheck {
                field,
                max_age,
                max_future,
                reference,
                action,
            } => DqRule::FreshnessCheck {
                field: if field.is_empty() {
                    parent_field.to_string()
                } else {
                    field
                },
                max_age,
                max_future,
                reference,
                action,
            },
            DqRule::MonotonicCheck {
                field,
                direction,
                partition_by,
                allow_reset,
                reset_threshold,
                action,
            } => DqRule::MonotonicCheck {
                field: if field.is_empty() {
                    parent_field.to_string()
                } else {
                    field
                },
                direction,
                partition_by,
                allow_reset,
                reset_threshold,
                action,
            },
            DqRule::RateOfChange {
                field,
                max_change_per_minute,
                partition_by,
                action,
            } => DqRule::RateOfChange {
                field: if field.is_empty() {
                    parent_field.to_string()
                } else {
                    field
                },
                max_change_per_minute,
                partition_by,
                action,
            },
            DqRule::CompletenessCheck {
                level,
                field,
                min_completeness,
                action,
            } => DqRule::CompletenessCheck {
                level,
                field: if field.is_empty() {
                    parent_field.to_string()
                } else {
                    field
                },
                min_completeness,
                action,
            },
            DqRule::CardinalityCheck {
                level,
                field,
                expected_range,
                action,
            } => DqRule::CardinalityCheck {
                level,
                field: if field.is_empty() {
                    parent_field.to_string()
                } else {
                    field
                },
                expected_range,
                action,
            },
            // Cross-field and conditional rules don't have a single field
            other => other,
        }
    }

    /// Generate dq_flags array expression combining all rules
    ///
    /// # Arguments
    ///
    /// * `rules` - List of DQ rules to combine
    ///
    /// # Returns
    ///
    /// SQL expression that produces TEXT[] of all triggered flags
    pub fn generate_dq_flags_expr(&self, rules: &[DqRule]) -> String {
        if rules.is_empty() {
            return "[]::VARCHAR[] AS dq_flags".to_string();
        }

        // Collect all flag expressions (skip batch-level rules that return empty strings)
        let flag_exprs: Vec<String> = rules
            .iter()
            .map(|rule| self.generate_check_sql(rule))
            .filter(|s| !s.is_empty())
            .collect();

        if flag_exprs.is_empty() {
            return "[]::VARCHAR[] AS dq_flags".to_string();
        }

        // Use DuckDB's list_filter to remove NULLs (PostgreSQL's ARRAY_REMOVE doesn't exist in DuckDB)
        format!(
            "list_filter([\n{}\n], x -> x IS NOT NULL) AS dq_flags",
            flag_exprs.join(",\n")
        )
    }

    // =========================================================================
    // Private Helper Methods
    // =========================================================================

    /// Generate range_check SQL
    fn generate_range_check_sql(
        &self,
        field: &str,
        min: Option<f64>,
        max: Option<f64>,
        action: &DqAction,
    ) -> String {
        let violation_condition = self.generate_range_violation_condition(field, min, max);
        let reason = match action {
            DqAction::Clamp => "clamped",
            _ => "out_of_bounds",
        };

        format!(
            r#"CASE
  WHEN {}
  THEN 'range_check:{}:{}'
  ELSE NULL
END"#,
            violation_condition, field, reason
        )
    }

    /// Generate the condition that identifies a range violation
    fn generate_range_violation_condition(
        &self,
        field: &str,
        min: Option<f64>,
        max: Option<f64>,
    ) -> String {
        let mut conditions = Vec::new();

        if let Some(min_val) = min {
            conditions.push(format!("{} < {}", field, self.format_f64(min_val)));
        }

        if let Some(max_val) = max {
            conditions.push(format!("{} > {}", field, self.format_f64(max_val)));
        }

        if conditions.is_empty() {
            "FALSE".to_string()
        } else {
            conditions.join(" OR ")
        }
    }

    /// Generate clamp expression using LEAST/GREATEST
    fn generate_clamp_expr(&self, expr: &str, min: Option<f64>, max: Option<f64>) -> String {
        let mut result = expr.to_string();

        if let Some(min_val) = min {
            result = format!("GREATEST({}, {})", result, self.format_f64(min_val));
        }

        if let Some(max_val) = max {
            result = format!("LEAST({}, {})", result, self.format_f64(max_val));
        }

        result
    }

    /// Generate null_check SQL
    fn generate_null_check_sql(&self, field: &str) -> String {
        format!(
            r#"CASE
  WHEN {} IS NULL
  THEN 'null_check:{}:missing'
  ELSE NULL
END"#,
            field, field
        )
    }

    /// Generate enum_check SQL
    fn generate_enum_check_sql(
        &self,
        field: &str,
        allowed_values: &[String],
        case_sensitive: bool,
    ) -> String {
        // Build the IN clause values
        let values_list = if case_sensitive {
            allowed_values
                .iter()
                .map(|v| format!("'{}'", self.escape_sql_string(v)))
                .collect::<Vec<_>>()
                .join(",")
        } else {
            allowed_values
                .iter()
                .map(|v| format!("'{}'", self.escape_sql_string(&v.to_uppercase())))
                .collect::<Vec<_>>()
                .join(",")
        };

        let field_expr = if case_sensitive {
            field.to_string()
        } else {
            format!("UPPER({})", field)
        };

        format!(
            r#"CASE
  WHEN {} NOT IN ({})
  THEN 'enum_check:{}:invalid_value'
  ELSE NULL
END"#,
            field_expr, values_list, field
        )
    }

    /// Generate pattern_check SQL
    fn generate_pattern_check_sql(&self, field: &str, pattern: &str) -> String {
        // DuckDB uses ~ for regex matching
        let escaped_pattern = self.escape_sql_string(pattern);

        format!(
            r#"CASE
  WHEN {} !~ '{}'
  THEN 'pattern_check:{}:pattern_mismatch'
  ELSE NULL
END"#,
            field, escaped_pattern, field
        )
    }

    /// Generate freshness_check SQL
    ///
    /// Note: DuckDB doesn't support TIMESTAMPTZ - INTERVAL directly,
    /// so we cast both sides to TIMESTAMP for interval arithmetic.
    fn generate_freshness_check_sql(
        &self,
        field: &str,
        max_age: Option<&str>,
        max_future: Option<&str>,
        reference: &str,
    ) -> String {
        let mut cases = Vec::new();

        // Cast to TIMESTAMP for DuckDB interval arithmetic compatibility
        if let Some(age) = max_age {
            cases.push(format!(
                "WHEN {}::TIMESTAMP < ({}::TIMESTAMP - INTERVAL '{}') THEN 'freshness_check:{}:stale'",
                field, reference, age, field
            ));
        }

        if let Some(future) = max_future {
            cases.push(format!(
                "WHEN {}::TIMESTAMP > ({}::TIMESTAMP + INTERVAL '{}') THEN 'freshness_check:{}:future'",
                field, reference, future, field
            ));
        }

        if cases.is_empty() {
            // No conditions specified
            return String::new();
        }

        format!(
            r#"CASE
  {}
  ELSE NULL
END"#,
            cases.join("\n  ")
        )
    }

    /// Generate rate_of_change SQL using LAG window function
    fn generate_rate_of_change_sql(
        &self,
        field: &str,
        max_change_per_minute: f64,
        partition_by: &[String],
    ) -> String {
        let partition_clause = if partition_by.is_empty() {
            String::new()
        } else {
            format!("PARTITION BY {} ", partition_by.join(", "))
        };

        // Note: This generates a complex expression that requires window context
        // In practice, this would be used in a CTE with proper window definitions
        format!(
            r#"CASE
  WHEN ABS({} - LAG({}) OVER ({}ORDER BY observation_time)) /
       NULLIF(EXTRACT(EPOCH FROM observation_time - LAG(observation_time) OVER ({}ORDER BY observation_time)) / 60.0, 0)
       > {}
  THEN 'rate_of_change:{}:exceeded'
  ELSE NULL
END"#,
            field, field, partition_clause, partition_clause, max_change_per_minute, field
        )
    }

    /// Generate cross_field_check SQL
    fn generate_cross_field_check_sql(
        &self,
        name: &str,
        expression: &str,
        message: Option<&str>,
    ) -> String {
        let flag_message = message.unwrap_or(name);

        format!(
            r#"CASE
  WHEN NOT ({})
  THEN 'cross_field_check:{}'
  ELSE NULL
END"#,
            expression, flag_message
        )
    }

    /// Escape single quotes in SQL strings
    fn escape_sql_string(&self, value: &str) -> String {
        value.replace('\'', "''")
    }

    /// Format f64 as SQL literal, always including decimal point
    ///
    /// Ensures consistent SQL output (e.g., "0.0" instead of "0")
    fn format_f64(&self, value: f64) -> String {
        let s = value.to_string();
        if s.contains('.') || s.contains('e') || s.contains('E') {
            s
        } else {
            format!("{}.0", s)
        }
    }
}

impl Default for DqSqlGenerator {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ============================================================
    // Test 1: range_check with flag action
    // ============================================================
    #[test]
    fn test_range_check_flag_sql() {
        let rule = DqRule::RangeCheck {
            field: "pm25".to_string(),
            min: Some(0.0),
            max: Some(1000.0),
            action: DqAction::Flag,
            clamp_to_bounds: false,
        };

        let gen = DqSqlGenerator::new();
        let sql = gen.generate_check_sql(&rule);

        assert_eq!(
            sql,
            r#"CASE
  WHEN pm25 < 0.0 OR pm25 > 1000.0
  THEN 'range_check:pm25:out_of_bounds'
  ELSE NULL
END"#
        );
    }

    // ============================================================
    // Test 2: range_check with clamp action (value expression)
    // ============================================================
    #[test]
    fn test_range_check_clamp_value_sql() {
        let rule = DqRule::RangeCheck {
            field: "humidity_pct".to_string(),
            min: Some(0.0),
            max: Some(100.0),
            action: DqAction::Clamp,
            clamp_to_bounds: true,
        };

        let gen = DqSqlGenerator::new();
        let sql = gen.generate_value_expr(&rule, "humidity_pct_raw");

        assert_eq!(
            sql,
            "LEAST(GREATEST(humidity_pct_raw, 0.0), 100.0) AS humidity_pct"
        );
    }

    #[test]
    fn test_range_check_clamp_flag_sql() {
        let rule = DqRule::RangeCheck {
            field: "humidity_pct".to_string(),
            min: Some(0.0),
            max: Some(100.0),
            action: DqAction::Clamp,
            clamp_to_bounds: true,
        };

        let gen = DqSqlGenerator::new();
        let sql = gen.generate_check_sql(&rule);

        assert_eq!(
            sql,
            r#"CASE
  WHEN humidity_pct < 0.0 OR humidity_pct > 100.0
  THEN 'range_check:humidity_pct:clamped'
  ELSE NULL
END"#
        );
    }

    // ============================================================
    // Test 3: range_check with reject action (NULL value)
    // ============================================================
    #[test]
    fn test_range_check_reject_value_sql() {
        let rule = DqRule::RangeCheck {
            field: "temperature_c".to_string(),
            min: Some(-60.0),
            max: Some(60.0),
            action: DqAction::Reject,
            clamp_to_bounds: false,
        };

        let gen = DqSqlGenerator::new();
        let sql = gen.generate_value_expr(&rule, "temperature_c_raw");

        assert_eq!(
            sql,
            r#"CASE
  WHEN temperature_c_raw < -60.0 OR temperature_c_raw > 60.0
  THEN NULL
  ELSE temperature_c_raw
END AS temperature_c"#
        );
    }

    // ============================================================
    // Test 4: null_check with reject action
    // ============================================================
    #[test]
    fn test_null_check_sql() {
        let rule = DqRule::NullCheck {
            field: "observation_time".to_string(),
            action: DqAction::Reject,
        };

        let gen = DqSqlGenerator::new();
        let sql = gen.generate_check_sql(&rule);

        assert_eq!(
            sql,
            r#"CASE
  WHEN observation_time IS NULL
  THEN 'null_check:observation_time:missing'
  ELSE NULL
END"#
        );
    }

    // ============================================================
    // Test 5: enum_check generates IN expression
    // ============================================================
    #[test]
    fn test_enum_check_sql() {
        let rule = DqRule::EnumCheck {
            field: "wind_direction".to_string(),
            allowed_values: vec![
                "N".to_string(),
                "NE".to_string(),
                "E".to_string(),
                "SE".to_string(),
                "S".to_string(),
                "SW".to_string(),
                "W".to_string(),
                "NW".to_string(),
            ],
            case_sensitive: false,
            action: DqAction::Flag,
        };

        let gen = DqSqlGenerator::new();
        let sql = gen.generate_check_sql(&rule);

        assert!(sql.contains("UPPER(wind_direction) NOT IN"));
        assert!(sql.contains("'N','NE','E','SE','S','SW','W','NW'"));
        assert!(sql.contains("enum_check:wind_direction:invalid_value"));
    }

    // ============================================================
    // Test 6: pattern_check generates regex
    // ============================================================
    #[test]
    fn test_pattern_check_sql() {
        let rule = DqRule::PatternCheck {
            field: "device_serial".to_string(),
            pattern: r"^[A-Z0-9]{8,12}$".to_string(),
            action: DqAction::Flag,
        };

        let gen = DqSqlGenerator::new();
        let sql = gen.generate_check_sql(&rule);

        assert!(sql.contains("device_serial !~"));
        assert!(sql.contains("'^[A-Z0-9]{8,12}$'"));
        assert!(sql.contains("pattern_check:device_serial:pattern_mismatch"));
    }

    // ============================================================
    // Test 7: cross_field_check expression
    // ============================================================
    #[test]
    fn test_cross_field_check_sql() {
        let rule = DqRule::CrossFieldCheck {
            name: "pm10_gte_pm25".to_string(),
            expression: "pm10 IS NULL OR pm25 IS NULL OR pm10 >= pm25".to_string(),
            message: Some("pm10_less_than_pm25".to_string()),
            action: DqAction::Flag,
        };

        let gen = DqSqlGenerator::new();
        let sql = gen.generate_check_sql(&rule);

        assert_eq!(
            sql,
            r#"CASE
  WHEN NOT (pm10 IS NULL OR pm25 IS NULL OR pm10 >= pm25)
  THEN 'cross_field_check:pm10_less_than_pm25'
  ELSE NULL
END"#
        );
    }

    // ============================================================
    // Test 8: Multiple rules generate list_filter (DuckDB compatible)
    // ============================================================
    #[test]
    fn test_multiple_rules_array_construct() {
        let rules = vec![
            DqRule::RangeCheck {
                field: "pm25".to_string(),
                min: Some(0.0),
                max: Some(1000.0),
                action: DqAction::Flag,
                clamp_to_bounds: false,
            },
            DqRule::NullCheck {
                field: "observation_time".to_string(),
                action: DqAction::Reject,
            },
        ];

        let gen = DqSqlGenerator::new();
        let sql = gen.generate_dq_flags_expr(&rules);

        assert!(sql.contains("list_filter(["));
        assert!(sql.contains("range_check:pm25:out_of_bounds"));
        assert!(sql.contains("null_check:observation_time:missing"));
        assert!(sql.contains("], x -> x IS NOT NULL) AS dq_flags"));
    }

    // ============================================================
    // Test 9: Empty rules produce empty array
    // ============================================================
    #[test]
    fn test_empty_rules_array() {
        let rules: Vec<DqRule> = vec![];

        let gen = DqSqlGenerator::new();
        let sql = gen.generate_dq_flags_expr(&rules);

        assert_eq!(sql, "[]::VARCHAR[] AS dq_flags");
    }

    // ============================================================
    // Test 10: Freshness check SQL (with DuckDB TIMESTAMP casts)
    // ============================================================
    #[test]
    fn test_freshness_check_sql() {
        let rule = DqRule::FreshnessCheck {
            field: "observation_time".to_string(),
            max_age: Some("2 hours".to_string()),
            max_future: Some("10 minutes".to_string()),
            reference: "ingestion_time".to_string(),
            action: DqAction::Flag,
        };

        let gen = DqSqlGenerator::new();
        let sql = gen.generate_check_sql(&rule);

        // DuckDB requires casting TIMESTAMPTZ to TIMESTAMP for interval arithmetic
        assert!(sql.contains("observation_time::TIMESTAMP < (ingestion_time::TIMESTAMP - INTERVAL '2 hours')"));
        assert!(sql.contains("freshness_check:observation_time:stale"));
        assert!(sql.contains("observation_time::TIMESTAMP > (ingestion_time::TIMESTAMP + INTERVAL '10 minutes')"));
        assert!(sql.contains("freshness_check:observation_time:future"));
    }

    // ============================================================
    // Additional Tests
    // ============================================================

    #[test]
    fn test_range_check_min_only() {
        let rule = DqRule::RangeCheck {
            field: "temperature".to_string(),
            min: Some(-273.15), // Absolute zero
            max: None,
            action: DqAction::Flag,
            clamp_to_bounds: false,
        };

        let gen = DqSqlGenerator::new();
        let sql = gen.generate_check_sql(&rule);

        assert!(sql.contains("temperature < -273.15"));
        assert!(!sql.contains(" OR "));
    }

    #[test]
    fn test_range_check_max_only() {
        let rule = DqRule::RangeCheck {
            field: "pressure".to_string(),
            min: None,
            max: Some(1200.0),
            action: DqAction::Flag,
            clamp_to_bounds: false,
        };

        let gen = DqSqlGenerator::new();
        let sql = gen.generate_check_sql(&rule);

        assert!(sql.contains("pressure > 1200.0"));
        assert!(!sql.contains(" OR "));
    }

    #[test]
    fn test_enum_check_case_sensitive() {
        let rule = DqRule::EnumCheck {
            field: "status".to_string(),
            allowed_values: vec!["Active".to_string(), "Inactive".to_string()],
            case_sensitive: true,
            action: DqAction::Flag,
        };

        let gen = DqSqlGenerator::new();
        let sql = gen.generate_check_sql(&rule);

        assert!(sql.contains("status NOT IN"));
        assert!(sql.contains("'Active','Inactive'"));
        assert!(!sql.contains("UPPER(status)"));
    }

    #[test]
    fn test_cross_field_check_without_message() {
        let rule = DqRule::CrossFieldCheck {
            name: "dew_point_check".to_string(),
            expression: "dew_point_c <= temperature_c".to_string(),
            message: None,
            action: DqAction::Flag,
        };

        let gen = DqSqlGenerator::new();
        let sql = gen.generate_check_sql(&rule);

        // Should use name when message is None
        assert!(sql.contains("cross_field_check:dew_point_check"));
    }

    #[test]
    fn test_rate_of_change_sql() {
        let rule = DqRule::RateOfChange {
            field: "temperature_c".to_string(),
            max_change_per_minute: 2.0,
            partition_by: vec!["ndp_id".to_string()],
            action: DqAction::Flag,
        };

        let gen = DqSqlGenerator::new();
        let sql = gen.generate_check_sql(&rule);

        assert!(sql.contains("LAG(temperature_c)"));
        assert!(sql.contains("PARTITION BY ndp_id"));
        assert!(sql.contains("> 2"));
        assert!(sql.contains("rate_of_change:temperature_c:exceeded"));
    }

    #[test]
    fn test_freshness_check_max_age_only() {
        let rule = DqRule::FreshnessCheck {
            field: "observation_time".to_string(),
            max_age: Some("1 hour".to_string()),
            max_future: None,
            reference: "NOW()".to_string(),
            action: DqAction::Flag,
        };

        let gen = DqSqlGenerator::new();
        let sql = gen.generate_check_sql(&rule);

        // DuckDB requires casting TIMESTAMPTZ to TIMESTAMP for interval arithmetic
        assert!(sql.contains("observation_time::TIMESTAMP < (NOW()::TIMESTAMP - INTERVAL '1 hour')"));
        assert!(sql.contains("freshness_check:observation_time:stale"));
        assert!(!sql.contains("future"));
    }

    #[test]
    fn test_batch_level_rules_return_empty() {
        let rule = DqRule::CompletenessCheck {
            level: "batch".to_string(),
            field: "pm25".to_string(),
            min_completeness: 0.95,
            action: DqAction::Warn,
        };

        let gen = DqSqlGenerator::new();
        let sql = gen.generate_check_sql(&rule);

        assert!(
            sql.is_empty(),
            "Batch-level rules should return empty string"
        );
    }

    #[test]
    fn test_dq_flags_with_batch_rules_filtered() {
        let rules = vec![
            DqRule::RangeCheck {
                field: "pm25".to_string(),
                min: Some(0.0),
                max: Some(1000.0),
                action: DqAction::Flag,
                clamp_to_bounds: false,
            },
            // Batch-level rule should be filtered out
            DqRule::CompletenessCheck {
                level: "batch".to_string(),
                field: "pm25".to_string(),
                min_completeness: 0.95,
                action: DqAction::Warn,
            },
        ];

        let gen = DqSqlGenerator::new();
        let sql = gen.generate_dq_flags_expr(&rules);

        assert!(sql.contains("range_check:pm25"));
        assert!(!sql.contains("completeness_check"));
    }

    #[test]
    fn test_escape_sql_string() {
        let gen = DqSqlGenerator::new();
        let escaped = gen.escape_sql_string("O'Brien");
        assert_eq!(escaped, "O''Brien");
    }

    #[test]
    fn test_clamp_expr_both_bounds() {
        let gen = DqSqlGenerator::new();
        let expr = gen.generate_clamp_expr("value", Some(0.0), Some(100.0));
        assert_eq!(expr, "LEAST(GREATEST(value, 0.0), 100.0)");
    }

    #[test]
    fn test_clamp_expr_min_only() {
        let gen = DqSqlGenerator::new();
        let expr = gen.generate_clamp_expr("value", Some(0.0), None);
        assert_eq!(expr, "GREATEST(value, 0.0)");
    }

    #[test]
    fn test_clamp_expr_max_only() {
        let gen = DqSqlGenerator::new();
        let expr = gen.generate_clamp_expr("value", None, Some(100.0));
        assert_eq!(expr, "LEAST(value, 100.0)");
    }

    #[test]
    fn test_default_trait() {
        let gen = DqSqlGenerator::default();
        // Just verify it constructs successfully with DuckDB-compatible syntax
        let empty: Vec<DqRule> = vec![];
        let sql = gen.generate_dq_flags_expr(&empty);
        assert!(sql.contains("[]::VARCHAR[]"));
    }
}
