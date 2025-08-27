//! Quality Scoring - Calculates data quality metrics for raw market data
//!
//! Implements comprehensive quality scoring based on completeness, freshness,
//! validity, and consistency of market data.

use crate::{RawMarketData, DataQualityMetrics, QualityThresholds};

/// Calculates data quality scores for raw market data
pub struct QualityScorer {
    thresholds: QualityThresholds,
}

impl QualityScorer {
    pub fn new(thresholds: &QualityThresholds) -> Self {
        Self {
            thresholds: thresholds.clone(),
        }
    }
    
    /// Calculate comprehensive quality metrics for raw data
    pub fn calculate_quality(&self, raw_data: &RawMarketData) -> DataQualityMetrics {
        let freshness_score = self.calculate_freshness_score(raw_data);
        let completeness_score = self.calculate_completeness_score(raw_data);
        let validity_score = self.calculate_validity_score(raw_data);
        
        // Overall score is weighted average
        let overall_score = (freshness_score * 0.3) + (completeness_score * 0.4) + (validity_score * 0.3);
        
        let data_age_seconds = raw_data.timestamp
            .map(|ts| chrono::Utc::now().timestamp() - ts)
            .unwrap_or(i64::MAX);
            
        let (missing_required, present_optional) = self.count_field_presence(raw_data);
        
        let validation_errors = self.collect_validation_errors(raw_data);
        
        DataQualityMetrics {
            overall_score,
            freshness_score,
            completeness_score,
            validity_score,
            missing_required_fields: missing_required,
            present_optional_fields: present_optional,
            data_age_seconds,
            validation_errors,
        }
    }
    
    /// Calculate freshness score based on data age
    fn calculate_freshness_score(&self, raw_data: &RawMarketData) -> f32 {
        let timestamp = match raw_data.timestamp {
            Some(ts) => ts,
            None => return 0.0, // No timestamp = no freshness
        };
        
        let now = chrono::Utc::now().timestamp();
        let age_seconds = now - timestamp;
        
        if age_seconds < 0 {
            // Future timestamp - suspicious
            return 0.1;
        }
        
        // Score decreases with age
        match age_seconds {
            0..=5 => 1.0,        // Excellent: 0-5 seconds
            6..=30 => 0.9,       // Great: 6-30 seconds
            31..=60 => 0.8,      // Good: 31-60 seconds
            61..=300 => 0.6,     // Fair: 1-5 minutes
            301..=900 => 0.4,    // Poor: 5-15 minutes
            901..=1800 => 0.2,   // Very poor: 15-30 minutes
            _ => 0.0,            // Unacceptable: >30 minutes
        }
    }
    
    /// Calculate completeness score based on field presence
    fn calculate_completeness_score(&self, raw_data: &RawMarketData) -> f32 {
        let total_possible_fields = 13; // Total fields in RawMarketData
        let mut present_fields = 0;
        let mut required_present = 0;
        let required_count = self.thresholds.required_fields.len();
        
        // Check all fields
        if raw_data.symbol.is_some() { present_fields += 1; }
        if raw_data.price.is_some() { present_fields += 1; }
        if raw_data.volume.is_some() { present_fields += 1; }
        if raw_data.timestamp.is_some() { present_fields += 1; }
        if raw_data.bid.is_some() { present_fields += 1; }
        if raw_data.ask.is_some() { present_fields += 1; }
        if raw_data.exchange.is_some() { present_fields += 1; }
        if raw_data.sequence.is_some() { present_fields += 1; }
        if raw_data.high.is_some() { present_fields += 1; }
        if raw_data.low.is_some() { present_fields += 1; }
        if raw_data.open.is_some() { present_fields += 1; }
        if raw_data.close.is_some() { present_fields += 1; }
        if raw_data.vwap.is_some() { present_fields += 1; }
        
        // Check required fields
        for required_field in &self.thresholds.required_fields {
            match required_field.as_str() {
                "symbol" if raw_data.symbol.is_some() => required_present += 1,
                "price" if raw_data.price.is_some() => required_present += 1,
                "timestamp" if raw_data.timestamp.is_some() => required_present += 1,
                "volume" if raw_data.volume.is_some() => required_present += 1,
                _ => {} // Unknown or missing required field
            }
        }
        
        // Required fields are weighted more heavily
        let required_score = if required_count > 0 {
            required_present as f32 / required_count as f32
        } else {
            1.0
        };
        
        let overall_completeness = present_fields as f32 / total_possible_fields as f32;
        
        // Weighted combination: 70% required fields, 30% overall completeness
        (required_score * 0.7) + (overall_completeness * 0.3)
    }
    
    /// Calculate validity score based on data consistency and ranges
    fn calculate_validity_score(&self, raw_data: &RawMarketData) -> f32 {
        let mut validity_checks = 0;
        let mut passed_checks = 0;
        
        // Price validity
        validity_checks += 1;
        if let Some(price) = raw_data.price {
            if price > 0.0 && price < 1_000_000.0 {
                passed_checks += 1;
            }
        } else {
            // Missing price is handled by completeness, not validity
            passed_checks += 1;
        }
        
        // Volume validity
        validity_checks += 1;
        if let Some(volume) = raw_data.volume {
            if volume >= 0.0 && volume < 1_000_000_000.0 {
                passed_checks += 1;
            }
        } else {
            passed_checks += 1; // Missing is OK for validity
        }
        
        // Bid/Ask validity
        if raw_data.bid.is_some() && raw_data.ask.is_some() {
            validity_checks += 1;
            if let (Some(bid), Some(ask)) = (raw_data.bid, raw_data.ask) {
                if bid > 0.0 && ask > 0.0 && bid < ask {
                    let spread_pct = (ask - bid) / ((ask + bid) / 2.0) * 100.0;
                    if spread_pct <= 10.0 { // Reasonable spread
                        passed_checks += 1;
                    }
                }
            }
        }
        
        // OHLC consistency
        if raw_data.open.is_some() && raw_data.high.is_some() && 
           raw_data.low.is_some() && raw_data.close.is_some() {
            validity_checks += 1;
            if let (Some(open), Some(high), Some(low), Some(close)) = 
                (raw_data.open, raw_data.high, raw_data.low, raw_data.close) {
                
                if high >= low && high >= open.max(close) && 
                   low <= open.min(close) {
                    passed_checks += 1;
                }
            }
        }
        
        // Symbol format validity
        validity_checks += 1;
        if let Some(ref symbol) = raw_data.symbol {
            if symbol.len() <= 10 && symbol.chars().all(|c| c.is_ascii_uppercase()) {
                passed_checks += 1;
            }
        } else {
            passed_checks += 1; // Missing handled by completeness
        }
        
        // Exchange validity
        validity_checks += 1;
        if let Some(ref exchange) = raw_data.exchange {
            if !exchange.is_empty() && exchange.len() <= 20 {
                passed_checks += 1;
            }
        } else {
            passed_checks += 1; // Missing is OK
        }
        
        // Timestamp validity
        validity_checks += 1;
        if let Some(timestamp) = raw_data.timestamp {
            let now = chrono::Utc::now().timestamp();
            if timestamp <= now && timestamp > (now - 86400) { // Within last 24 hours
                passed_checks += 1;
            }
        } else {
            passed_checks += 1; // Missing handled by completeness
        }
        
        if validity_checks > 0 {
            passed_checks as f32 / validity_checks as f32
        } else {
            1.0
        }
    }
    
    /// Count missing required fields and present optional fields
    fn count_field_presence(&self, raw_data: &RawMarketData) -> (u32, u32) {
        let mut missing_required = 0;
        let mut present_optional = 0;
        
        // Count missing required fields
        for required_field in &self.thresholds.required_fields {
            let is_present = match required_field.as_str() {
                "symbol" => raw_data.symbol.is_some(),
                "price" => raw_data.price.is_some(),
                "timestamp" => raw_data.timestamp.is_some(),
                "volume" => raw_data.volume.is_some(),
                _ => false, // Unknown required field
            };
            
            if !is_present {
                missing_required += 1;
            }
        }
        
        // Count present optional fields
        let optional_fields = [
            raw_data.bid.is_some(),
            raw_data.ask.is_some(),
            raw_data.exchange.is_some(),
            raw_data.sequence.is_some(),
            raw_data.high.is_some(),
            raw_data.low.is_some(),
            raw_data.open.is_some(),
            raw_data.close.is_some(),
            raw_data.vwap.is_some(),
        ];
        
        present_optional = optional_fields.iter().filter(|&&present| present).count() as u32;
        
        (missing_required, present_optional)
    }
    
    /// Collect validation errors for debugging
    fn collect_validation_errors(&self, raw_data: &RawMarketData) -> Vec<String> {
        let mut errors = Vec::new();
        
        // Check for specific validation issues
        if let Some(price) = raw_data.price {
            if price <= 0.0 {
                errors.push("Price must be positive".to_string());
            }
            if price > 1_000_000.0 {
                errors.push("Price exceeds maximum allowed value".to_string());
            }
        }
        
        if let Some(volume) = raw_data.volume {
            if volume < 0.0 {
                errors.push("Volume cannot be negative".to_string());
            }
        }
        
        if let (Some(bid), Some(ask)) = (raw_data.bid, raw_data.ask) {
            if bid >= ask {
                errors.push("Bid must be less than ask".to_string());
            }
            
            let spread_pct = (ask - bid) / ((ask + bid) / 2.0) * 100.0;
            if spread_pct > 10.0 {
                errors.push(format!("Bid-ask spread too wide: {:.2}%", spread_pct));
            }
        }
        
        if let Some(timestamp) = raw_data.timestamp {
            let now = chrono::Utc::now().timestamp();
            if timestamp > now + 60 {
                errors.push("Timestamp is in the future".to_string());
            }
            
            let age = now - timestamp;
            if age > self.thresholds.max_age_seconds {
                errors.push(format!("Data too old: {} seconds", age));
            }
        }
        
        if let Some(ref symbol) = raw_data.symbol {
            if symbol.is_empty() {
                errors.push("Symbol cannot be empty".to_string());
            }
            if symbol.len() > 10 {
                errors.push("Symbol too long".to_string());
            }
            if !symbol.chars().all(|c| c.is_ascii_uppercase()) {
                errors.push("Symbol must be uppercase letters only".to_string());
            }
        }
        
        errors
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    
    fn create_test_thresholds() -> QualityThresholds {
        QualityThresholds {
            minimum_quality_score: 0.7,
            max_age_seconds: 300,
            required_fields: vec![
                "symbol".to_string(),
                "price".to_string(),
                "timestamp".to_string(),
            ],
        }
    }
    
    fn create_high_quality_data() -> RawMarketData {
        RawMarketData {
            symbol: Some("AAPL".to_string()),
            price: Some(150.25),
            volume: Some(1000.0),
            timestamp: Some(chrono::Utc::now().timestamp() - 10), // 10 seconds ago
            bid: Some(150.20),
            ask: Some(150.30),
            exchange: Some("NASDAQ".to_string()),
            sequence: Some(12345),
            high: Some(151.0),
            low: Some(149.5),
            open: Some(150.0),
            close: Some(150.25),
            vwap: Some(150.1),
            metadata: HashMap::new(),
        }
    }
    
    #[test]
    fn test_high_quality_data_scoring() {
        let scorer = QualityScorer::new(&create_test_thresholds());
        let data = create_high_quality_data();
        
        let metrics = scorer.calculate_quality(&data);
        
        assert!(metrics.overall_score > 0.9);
        assert!(metrics.freshness_score > 0.9);
        assert!(metrics.completeness_score > 0.9);
        assert!(metrics.validity_score > 0.9);
        assert_eq!(metrics.missing_required_fields, 0);
        assert!(metrics.present_optional_fields >= 8);
        assert!(metrics.validation_errors.is_empty());
    }
    
    #[test]
    fn test_missing_required_fields_lowers_score() {
        let scorer = QualityScorer::new(&create_test_thresholds());
        let mut data = create_high_quality_data();
        data.symbol = None; // Remove required field
        
        let metrics = scorer.calculate_quality(&data);
        
        assert!(metrics.overall_score < 0.9);
        assert_eq!(metrics.missing_required_fields, 1);
        assert!(metrics.completeness_score < 0.9);
    }
    
    #[test]
    fn test_old_data_lowers_freshness() {
        let scorer = QualityScorer::new(&create_test_thresholds());
        let mut data = create_high_quality_data();
        data.timestamp = Some(chrono::Utc::now().timestamp() - 1000); // 16+ minutes old
        
        let metrics = scorer.calculate_quality(&data);
        
        assert!(metrics.freshness_score < 0.5);
        assert!(metrics.overall_score < 0.9);
    }
    
    #[test]
    fn test_invalid_data_lowers_validity() {
        let scorer = QualityScorer::new(&create_test_thresholds());
        let mut data = create_high_quality_data();
        data.price = Some(-100.0); // Invalid negative price
        data.bid = Some(150.30);   // Bid higher than ask
        data.ask = Some(150.20);
        
        let metrics = scorer.calculate_quality(&data);
        
        assert!(metrics.validity_score < 0.9);
        assert!(metrics.overall_score < 0.9);
        assert!(!metrics.validation_errors.is_empty());
    }
    
    #[test]
    fn test_bid_ask_spread_validation() {
        let scorer = QualityScorer::new(&create_test_thresholds());
        let mut data = create_high_quality_data();
        
        // Wide spread should lower validity
        data.bid = Some(100.0);
        data.ask = Some(120.0); // 18% spread
        
        let metrics = scorer.calculate_quality(&data);
        
        assert!(metrics.validation_errors.iter()
            .any(|e| e.contains("spread too wide")));
    }
    
    #[test]
    fn test_ohlc_consistency_validation() {
        let scorer = QualityScorer::new(&create_test_thresholds());
        let mut data = create_high_quality_data();
        
        // Inconsistent OHLC data
        data.high = Some(150.0);
        data.low = Some(155.0); // Low higher than high
        
        let metrics = scorer.calculate_quality(&data);
        
        assert!(metrics.validity_score < 1.0);
    }
}