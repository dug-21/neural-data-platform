//! NULL handling strategies for aligned views
//!
//! Implements different NULL handling approaches per ADR-FE001-004:
//! - Preserve: Keep NULLs as-is (observation, forecast)
//! - CarryForward: Use LAG IGNORE NULLS (state_event, dimension)
//! - Interpolate: Linear interpolation between values

use crate::config::NullHandling;

/// Trait for wrapping column expressions with NULL handling
pub trait NullHandler: Send + Sync {
    /// Wrap a column expression with appropriate NULL handling
    ///
    /// # Arguments
    /// * `source_expr` - The source expression (e.g., "indoor.pm25_mean")
    /// * `target_alias` - The target column alias (e.g., "indoor_pm25_mean")
    /// * `bucket_expr` - The bucket expression for window ordering
    ///
    /// # Returns
    /// SQL expression with NULL handling applied
    fn wrap_column(&self, source_expr: &str, target_alias: &str, bucket_expr: &str) -> String;
}

/// Preserve NULL handler - passes through without transformation
pub struct PreserveNullHandler;

impl NullHandler for PreserveNullHandler {
    fn wrap_column(&self, source_expr: &str, target_alias: &str, _bucket_expr: &str) -> String {
        format!("{} AS {}", source_expr, target_alias)
    }
}

/// Carry Forward (LOCF) NULL handler - uses LAG IGNORE NULLS
pub struct CarryForwardNullHandler;

impl NullHandler for CarryForwardNullHandler {
    fn wrap_column(&self, source_expr: &str, target_alias: &str, bucket_expr: &str) -> String {
        format!(
            "COALESCE(\n        {},\n        LAG({}) IGNORE NULLS OVER (ORDER BY {})\n    ) AS {}",
            source_expr, source_expr, bucket_expr, target_alias
        )
    }
}

/// Interpolate NULL handler - linear interpolation between values
pub struct InterpolateNullHandler;

impl NullHandler for InterpolateNullHandler {
    fn wrap_column(&self, source_expr: &str, target_alias: &str, bucket_expr: &str) -> String {
        format!(
            r#"CASE
        WHEN {} IS NOT NULL THEN {}
        ELSE (
            LAG({}) IGNORE NULLS OVER (ORDER BY {}) +
            LEAD({}) IGNORE NULLS OVER (ORDER BY {})
        ) / 2.0
    END AS {}"#,
            source_expr,
            source_expr,
            source_expr,
            bucket_expr,
            source_expr,
            bucket_expr,
            target_alias
        )
    }
}

/// Get the appropriate NULL handler for a given strategy
pub fn get_null_handler(strategy: NullHandling) -> Box<dyn NullHandler> {
    match strategy {
        NullHandling::Preserve => Box::new(PreserveNullHandler),
        NullHandling::CarryForward => Box::new(CarryForwardNullHandler),
        NullHandling::Interpolate => Box::new(InterpolateNullHandler),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preserve_null_handler() {
        let handler = PreserveNullHandler;
        let result = handler.wrap_column("indoor.pm25_mean", "indoor_pm25_mean", "bucket");
        assert_eq!(result, "indoor.pm25_mean AS indoor_pm25_mean");
    }

    #[test]
    fn test_carry_forward_null_handler() {
        let handler = CarryForwardNullHandler;
        let result = handler.wrap_column("state.window_state", "state_window_state", "bucket");

        assert!(result.contains("COALESCE"));
        assert!(result.contains("LAG(state.window_state) IGNORE NULLS"));
        assert!(result.contains("ORDER BY bucket"));
        assert!(result.contains("AS state_window_state"));
    }

    #[test]
    fn test_interpolate_null_handler() {
        let handler = InterpolateNullHandler;
        let result = handler.wrap_column("indoor.temp_mean", "indoor_temp_mean", "bucket");

        assert!(result.contains("CASE"));
        assert!(result.contains("WHEN indoor.temp_mean IS NOT NULL"));
        assert!(result.contains("LAG(indoor.temp_mean) IGNORE NULLS"));
        assert!(result.contains("LEAD(indoor.temp_mean) IGNORE NULLS"));
        assert!(result.contains("/ 2.0"));
        assert!(result.contains("AS indoor_temp_mean"));
    }

    #[test]
    fn test_get_null_handler() {
        // Just verify we get the right types
        let preserve = get_null_handler(NullHandling::Preserve);
        let result = preserve.wrap_column("x", "x_alias", "bucket");
        assert!(result.contains("x AS x_alias"));

        let carry = get_null_handler(NullHandling::CarryForward);
        let result = carry.wrap_column("x", "x_alias", "bucket");
        assert!(result.contains("COALESCE"));

        let interp = get_null_handler(NullHandling::Interpolate);
        let result = interp.wrap_column("x", "x_alias", "bucket");
        assert!(result.contains("CASE"));
    }
}
