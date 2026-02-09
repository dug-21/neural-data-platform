//! Parsing utilities for Gold DDL generation
//!
//! Provides granularity and window parsing used by generators and registry modules.
//! Validation of Gold ETL config semantics is handled by `validate::semantic::gold`.

use crate::gold::error::{GoldDdlError, Result};

/// Parse a granularity string and validate its format
/// Returns (value, unit) if valid
pub fn parse_granularity(granularity: &str) -> Result<(u32, String)> {
    let parts: Vec<&str> = granularity.split_whitespace().collect();

    if parts.len() != 2 {
        return Err(GoldDdlError::InvalidGranularity {
            granularity: granularity.to_string(),
        });
    }

    let value: u32 = parts[0]
        .parse()
        .map_err(|_| GoldDdlError::InvalidGranularity {
            granularity: granularity.to_string(),
        })?;

    let unit = parts[1].to_lowercase();
    match unit.as_str() {
        "hour" | "hours" | "day" | "days" | "minute" | "minutes" | "week" | "weeks" => {
            Ok((value, unit))
        }
        _ => Err(GoldDdlError::InvalidGranularity {
            granularity: granularity.to_string(),
        }),
    }
}

/// Parse a window string to number of hourly rows
/// "4 hours" -> 4
/// "1 day" -> 24
pub fn parse_window(window: &str) -> Result<u32> {
    let parts: Vec<&str> = window.split_whitespace().collect();

    if parts.len() != 2 {
        return Err(GoldDdlError::InvalidWindow {
            window: window.to_string(),
        });
    }

    let value: u32 = parts[0].parse().map_err(|_| GoldDdlError::InvalidWindow {
        window: window.to_string(),
    })?;

    let unit = parts[1].to_lowercase();
    match unit.as_str() {
        "hour" | "hours" => Ok(value),
        "day" | "days" => Ok(value * 24),
        _ => Err(GoldDdlError::InvalidWindow {
            window: window.to_string(),
        }),
    }
}

/// Convert granularity to view name suffix
pub fn granularity_to_suffix(granularity: &str) -> String {
    let (value, unit) = parse_granularity(granularity).unwrap_or((1, "hour".to_string()));

    match unit.as_str() {
        "hour" | "hours" => {
            if value == 1 {
                "hourly".to_string()
            } else {
                format!("{}hourly", value)
            }
        }
        "day" | "days" => {
            if value == 1 {
                "daily".to_string()
            } else {
                format!("{}daily", value)
            }
        }
        "minute" | "minutes" => format!("{}min", value),
        "week" | "weeks" => {
            if value == 1 {
                "weekly".to_string()
            } else {
                format!("{}weekly", value)
            }
        }
        _ => "custom".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // Granularity Parsing Tests
    // =========================================================================

    #[test]
    fn test_parse_granularity_1_hour() {
        let (value, unit) = parse_granularity("1 hour").unwrap();
        assert_eq!(value, 1);
        assert_eq!(unit, "hour");
    }

    #[test]
    fn test_parse_granularity_4_hours() {
        let (value, unit) = parse_granularity("4 hours").unwrap();
        assert_eq!(value, 4);
        assert_eq!(unit, "hours");
    }

    #[test]
    fn test_parse_granularity_1_day() {
        let (value, unit) = parse_granularity("1 day").unwrap();
        assert_eq!(value, 1);
        assert_eq!(unit, "day");
    }

    #[test]
    fn test_parse_granularity_invalid_format() {
        assert!(parse_granularity("hourly").is_err());
        assert!(parse_granularity("1").is_err());
        assert!(parse_granularity("one hour").is_err());
        assert!(parse_granularity("1 second").is_err());
    }

    // =========================================================================
    // Window Parsing Tests
    // =========================================================================

    #[test]
    fn test_parse_window_4_hours() {
        let rows = parse_window("4 hours").unwrap();
        assert_eq!(rows, 4);
    }

    #[test]
    fn test_parse_window_1_day() {
        let rows = parse_window("1 day").unwrap();
        assert_eq!(rows, 24);
    }

    #[test]
    fn test_parse_window_invalid() {
        assert!(parse_window("4hours").is_err());
        assert!(parse_window("4 minutes").is_err());
    }

    // =========================================================================
    // Granularity Suffix Tests
    // =========================================================================

    #[test]
    fn test_granularity_to_suffix() {
        assert_eq!(granularity_to_suffix("1 hour"), "hourly");
        assert_eq!(granularity_to_suffix("4 hours"), "4hourly");
        assert_eq!(granularity_to_suffix("1 day"), "daily");
        assert_eq!(granularity_to_suffix("7 days"), "7daily");
        assert_eq!(granularity_to_suffix("15 minutes"), "15min");
    }
}
