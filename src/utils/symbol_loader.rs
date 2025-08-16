use std::env;
use std::collections::HashSet;
use tracing::{info, warn};

/// Default fallback symbols if environment variable is not set
const DEFAULT_TRADING_SYMBOLS: &str = "AAPL,MSFT,GOOGL,AMZN,NVDA,DDOG,XLK,XLF,XLV,XLE,XLY,XLP,XLI,XLB,XLU,XLRE";

/// Load trading symbols from environment variable or fallback to defaults
/// 
/// This function reads the TRADING_SYMBOLS_PRIMARY environment variable and parses
/// the comma-separated symbols. It also validates that sector ETFs are included
/// and removes duplicates while preserving order.
pub fn load_trading_symbols() -> Vec<String> {
    let symbols_str = env::var("TRADING_SYMBOLS_PRIMARY")
        .unwrap_or_else(|_| {
            warn!("TRADING_SYMBOLS_PRIMARY environment variable not set, using defaults");
            DEFAULT_TRADING_SYMBOLS.to_string()
        });

    let mut symbols: Vec<String> = symbols_str
        .split(',')
        .map(|s| s.trim().to_uppercase())
        .filter(|s| !s.is_empty())
        .collect();

    // Remove duplicates while preserving order
    let mut seen = HashSet::new();
    symbols.retain(|symbol| seen.insert(symbol.clone()));

    // Ensure key sector ETFs are included
    let required_etfs = ["XLK", "XLF", "XLV", "XLE", "XLY", "XLP", "XLI", "XLB", "XLU", "XLRE"];
    for etf in &required_etfs {
        if !symbols.contains(&etf.to_string()) {
            warn!("Adding missing sector ETF: {}", etf);
            symbols.push(etf.to_string());
        }
    }

    info!("Loaded {} trading symbols from configuration", symbols.len());
    info!("Primary symbols: {}", symbols.join(", "));

    symbols
}

/// Load primary stock symbols (excluding ETFs)
/// 
/// This function filters out sector ETFs to get only individual stock symbols
pub fn load_stock_symbols() -> Vec<String> {
    let all_symbols = load_trading_symbols();
    let etf_symbols = ["XLK", "XLF", "XLV", "XLE", "XLY", "XLP", "XLI", "XLB", "XLU", "XLRE"];
    
    all_symbols
        .into_iter()
        .filter(|symbol| !etf_symbols.contains(&symbol.as_str()))
        .collect()
}

/// Load only sector ETF symbols
/// 
/// This function returns only the sector ETF symbols for sector-specific operations
pub fn load_sector_etf_symbols() -> Vec<String> {
    let all_symbols = load_trading_symbols();
    let etf_symbols = ["XLK", "XLF", "XLV", "XLE", "XLY", "XLP", "XLI", "XLB", "XLU", "XLRE"];
    
    all_symbols
        .into_iter()
        .filter(|symbol| etf_symbols.contains(&symbol.as_str()))
        .collect()
}

/// Get symbol count for memory allocation and planning
pub fn get_symbol_count() -> usize {
    load_trading_symbols().len()
}

/// Check if a symbol is a sector ETF
pub fn is_sector_etf(symbol: &str) -> bool {
    let etf_symbols = ["XLK", "XLF", "XLV", "XLE", "XLY", "XLP", "XLI", "XLB", "XLU", "XLRE"];
    etf_symbols.contains(&symbol.to_uppercase().as_str())
}

/// Get sector for a given ETF symbol
pub fn get_sector_for_etf(etf_symbol: &str) -> Option<&'static str> {
    match etf_symbol.to_uppercase().as_str() {
        "XLK" => Some("Technology"),
        "XLF" => Some("Financial Services"),
        "XLV" => Some("Healthcare"),
        "XLE" => Some("Energy"),
        "XLY" => Some("Consumer Discretionary"),
        "XLP" => Some("Consumer Staples"),
        "XLI" => Some("Industrials"),
        "XLB" => Some("Materials"),
        "XLU" => Some("Utilities"),
        "XLRE" => Some("Real Estate"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_load_trading_symbols_default() {
        // Test when no environment variable is set
        std::env::remove_var("TRADING_SYMBOLS_PRIMARY");
        let symbols = load_trading_symbols();
        
        assert!(!symbols.is_empty());
        assert!(symbols.contains(&"AAPL".to_string()));
        assert!(symbols.contains(&"XLK".to_string()));
    }

    #[test]
    fn test_load_trading_symbols_custom() {
        // Test with custom environment variable
        std::env::set_var("TRADING_SYMBOLS_PRIMARY", "AAPL,MSFT,GOOGL");
        let symbols = load_trading_symbols();
        
        // Should include custom symbols plus required ETFs
        assert!(symbols.contains(&"AAPL".to_string()));
        assert!(symbols.contains(&"MSFT".to_string()));
        assert!(symbols.contains(&"GOOGL".to_string()));
        assert!(symbols.contains(&"XLK".to_string())); // Added automatically
    }

    #[test]
    fn test_symbol_filtering() {
        std::env::set_var("TRADING_SYMBOLS_PRIMARY", "AAPL,XLK,MSFT,XLF");
        
        let stock_symbols = load_stock_symbols();
        let etf_symbols = load_sector_etf_symbols();
        
        assert!(stock_symbols.contains(&"AAPL".to_string()));
        assert!(stock_symbols.contains(&"MSFT".to_string()));
        assert!(!stock_symbols.contains(&"XLK".to_string()));
        
        assert!(etf_symbols.contains(&"XLK".to_string()));
        assert!(etf_symbols.contains(&"XLF".to_string()));
        assert!(!etf_symbols.contains(&"AAPL".to_string()));
    }

    #[test]
    fn test_sector_mapping() {
        assert_eq!(get_sector_for_etf("XLK"), Some("Technology"));
        assert_eq!(get_sector_for_etf("XLF"), Some("Financial Services"));
        assert_eq!(get_sector_for_etf("INVALID"), None);
    }

    #[test]
    fn test_etf_identification() {
        assert!(is_sector_etf("XLK"));
        assert!(is_sector_etf("xlf")); // Case insensitive
        assert!(!is_sector_etf("AAPL"));
    }
}