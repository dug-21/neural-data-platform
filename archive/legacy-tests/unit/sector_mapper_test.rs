//! Comprehensive Unit Tests for SectorMapper
//!
//! Tests symbol-to-sector mapping, ETF associations, and dynamic updates.

use anyhow::Result;
use std::collections::HashMap;

use crate::data::sector_mapper::{
    SectorMapper, SectorMapperConfig, SectorInfo, SectorId, MarketCapTier, 
    SectorUpdate, SectorStats
};

// Test utilities
fn create_test_config() -> SectorMapperConfig {
    SectorMapperConfig {
        enable_dynamic_updates: true,
        cache_ttl_seconds: 1800,
        default_sector: "technology".to_string(),
    }
}

fn create_custom_sector_info(sector: SectorId, sub_sector: &str, weight: f64) -> SectorInfo {
    SectorInfo {
        id: sector.as_str().to_string(),
        sector_id: sector,
        sub_sector: Some(sub_sector.to_string()),
        market_cap_tier: MarketCapTier::LargeCap,
        weight_in_sector: weight,
        correlation_group: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_sector_id_string_conversion() {
        // Test enum to string conversion
        assert_eq!(SectorId::Technology.as_str(), "technology");
        assert_eq!(SectorId::FinancialServices.as_str(), "financial_services");
        assert_eq!(SectorId::Healthcare.as_str(), "healthcare");
        assert_eq!(SectorId::Energy.as_str(), "energy");
        assert_eq!(SectorId::ConsumerDiscretionary.as_str(), "consumer_discretionary");
        assert_eq!(SectorId::ConsumerStaples.as_str(), "consumer_staples");
        assert_eq!(SectorId::Industrials.as_str(), "industrials");
        assert_eq!(SectorId::Materials.as_str(), "materials");
        assert_eq!(SectorId::Utilities.as_str(), "utilities");
        assert_eq!(SectorId::RealEstate.as_str(), "real_estate");
        assert_eq!(SectorId::Communication.as_str(), "communication");
        assert_eq!(SectorId::Custom(42).as_str(), "custom");
    }
    
    #[test]
    fn test_sector_id_from_string() {
        // Test string to enum conversion
        assert_eq!(SectorId::from_str("technology"), Some(SectorId::Technology));
        assert_eq!(SectorId::from_str("TECH"), Some(SectorId::Technology));
        assert_eq!(SectorId::from_str("tech"), Some(SectorId::Technology));
        
        assert_eq!(SectorId::from_str("financial_services"), Some(SectorId::FinancialServices));
        assert_eq!(SectorId::from_str("financial"), Some(SectorId::FinancialServices));
        assert_eq!(SectorId::from_str("finance"), Some(SectorId::FinancialServices));
        
        assert_eq!(SectorId::from_str("healthcare"), Some(SectorId::Healthcare));
        assert_eq!(SectorId::from_str("health"), Some(SectorId::Healthcare));
        
        assert_eq!(SectorId::from_str("energy"), Some(SectorId::Energy));
        
        assert_eq!(SectorId::from_str("consumer_discretionary"), Some(SectorId::ConsumerDiscretionary));
        assert_eq!(SectorId::from_str("consumer_staples"), Some(SectorId::ConsumerStaples));
        assert_eq!(SectorId::from_str("industrials"), Some(SectorId::Industrials));
        assert_eq!(SectorId::from_str("materials"), Some(SectorId::Materials));
        assert_eq!(SectorId::from_str("utilities"), Some(SectorId::Utilities));
        
        assert_eq!(SectorId::from_str("real_estate"), Some(SectorId::RealEstate));
        assert_eq!(SectorId::from_str("realestate"), Some(SectorId::RealEstate));
        
        assert_eq!(SectorId::from_str("communication"), Some(SectorId::Communication));
        assert_eq!(SectorId::from_str("comm"), Some(SectorId::Communication));
        
        // Test unknown sector
        assert_eq!(SectorId::from_str("unknown_sector"), None);
        assert_eq!(SectorId::from_str(""), None);
    }
    
    #[test]
    fn test_sector_mapper_creation() {
        let config = create_test_config();
        let mapper = SectorMapper::new(config.clone());
        
        // Verify configuration
        assert_eq!(mapper.config.enable_dynamic_updates, config.enable_dynamic_updates);
        assert_eq!(mapper.config.cache_ttl_seconds, config.cache_ttl_seconds);
        assert_eq!(mapper.config.default_sector, config.default_sector);
        
        // Verify default mappings were loaded
        assert!(mapper.symbol_sectors.len() > 0);
    }
    
    #[test]
    fn test_default_mappings() {
        let mapper = SectorMapper::new(SectorMapperConfig::default());
        
        // Test technology sector mappings
        let aapl_sector = mapper.get_sector("AAPL").unwrap();
        assert_eq!(aapl_sector.sector_id, SectorId::Technology);
        assert_eq!(aapl_sector.sub_sector, Some("Consumer Electronics".to_string()));
        assert_eq!(aapl_sector.market_cap_tier, MarketCapTier::LargeCap);
        assert_eq!(aapl_sector.weight_in_sector, 0.22);
        assert_eq!(aapl_sector.correlation_group, Some("FAANG".to_string()));
        
        let msft_sector = mapper.get_sector("MSFT").unwrap();
        assert_eq!(msft_sector.sector_id, SectorId::Technology);
        assert_eq!(msft_sector.sub_sector, Some("Software".to_string()));
        assert_eq!(msft_sector.weight_in_sector, 0.21);
        assert_eq!(msft_sector.correlation_group, None);
        
        let googl_sector = mapper.get_sector("GOOGL").unwrap();
        assert_eq!(googl_sector.sector_id, SectorId::Technology);
        assert_eq!(googl_sector.sub_sector, Some("Internet Services".to_string()));
        assert_eq!(googl_sector.correlation_group, Some("FAANG".to_string()));
        
        // Test financial sector mappings
        let jpm_sector = mapper.get_sector("JPM").unwrap();
        assert_eq!(jpm_sector.sector_id, SectorId::FinancialServices);
        assert_eq!(jpm_sector.sub_sector, Some("Banking".to_string()));
        assert_eq!(jpm_sector.correlation_group, Some("big_banks".to_string()));
        
        let bac_sector = mapper.get_sector("BAC").unwrap();
        assert_eq!(bac_sector.sector_id, SectorId::FinancialServices);
        assert_eq!(bac_sector.correlation_group, Some("big_banks".to_string()));
        
        // Test healthcare sector mapping
        let jnj_sector = mapper.get_sector("JNJ").unwrap();
        assert_eq!(jnj_sector.sector_id, SectorId::Healthcare);
        assert_eq!(jnj_sector.sub_sector, Some("Pharmaceuticals".to_string()));
        assert_eq!(jnj_sector.correlation_group, None);
    }
    
    #[test]
    fn test_add_symbol_mapping() {
        let mapper = SectorMapper::new(SectorMapperConfig::default());
        
        let custom_info = SectorInfo {
            id: "energy".to_string(),
            sector_id: SectorId::Energy,
            sub_sector: Some("Oil & Gas".to_string()),
            market_cap_tier: MarketCapTier::LargeCap,
            weight_in_sector: 0.15,
            correlation_group: Some("energy_majors".to_string()),
        };
        
        mapper.add_symbol_mapping("XOM", custom_info.clone());
        
        let retrieved = mapper.get_sector("XOM").unwrap();
        assert_eq!(retrieved.sector_id, SectorId::Energy);
        assert_eq!(retrieved.sub_sector, Some("Oil & Gas".to_string()));
        assert_eq!(retrieved.weight_in_sector, 0.15);
        assert_eq!(retrieved.correlation_group, Some("energy_majors".to_string()));
    }
    
    #[test]
    fn test_get_sector_unknown_symbol() {
        let mapper = SectorMapper::new(SectorMapperConfig::default());
        
        // Test unknown symbol - should use default sector
        let unknown_sector = mapper.get_sector("UNKNOWN_SYMBOL").unwrap();
        assert_eq!(unknown_sector.sector_id, SectorId::Technology); // Default sector
        assert_eq!(unknown_sector.id, "technology");
        assert_eq!(unknown_sector.market_cap_tier, MarketCapTier::MidCap);
        assert_eq!(unknown_sector.weight_in_sector, 0.01);
        assert_eq!(unknown_sector.sub_sector, None);
        assert_eq!(unknown_sector.correlation_group, None);
        
        // Verify the symbol was added to the mapping with default values
        let retrieved_again = mapper.get_sector("UNKNOWN_SYMBOL").unwrap();
        assert_eq!(retrieved_again.sector_id, SectorId::Technology);
    }
    
    #[test]
    fn test_get_symbols_in_sector() {
        let mapper = SectorMapper::new(SectorMapperConfig::default());
        
        // Test technology sector
        let tech_symbols = mapper.get_symbols_in_sector(&SectorId::Technology);
        assert!(tech_symbols.contains(&"AAPL".to_string()));
        assert!(tech_symbols.contains(&"MSFT".to_string()));
        assert!(tech_symbols.contains(&"GOOGL".to_string()));
        assert_eq!(tech_symbols.len(), 3);
        
        // Test financial services sector
        let finance_symbols = mapper.get_symbols_in_sector(&SectorId::FinancialServices);
        assert!(finance_symbols.contains(&"JPM".to_string()));
        assert!(finance_symbols.contains(&"BAC".to_string()));
        assert_eq!(finance_symbols.len(), 2);
        
        // Test healthcare sector
        let health_symbols = mapper.get_symbols_in_sector(&SectorId::Healthcare);
        assert!(health_symbols.contains(&"JNJ".to_string()));
        assert_eq!(health_symbols.len(), 1);
        
        // Test empty sector
        let energy_symbols = mapper.get_symbols_in_sector(&SectorId::Energy);
        assert!(energy_symbols.is_empty());
    }
    
    #[test]
    fn test_sector_etf_mappings() {
        let mapper = SectorMapper::new(SectorMapperConfig::default());
        
        // Test default ETF mappings
        assert_eq!(mapper.get_sector_etf(&SectorId::Technology), Some("XLK".to_string()));
        assert_eq!(mapper.get_sector_etf(&SectorId::FinancialServices), Some("XLF".to_string()));
        assert_eq!(mapper.get_sector_etf(&SectorId::Healthcare), Some("XLV".to_string()));
        assert_eq!(mapper.get_sector_etf(&SectorId::Energy), Some("XLE".to_string()));
        assert_eq!(mapper.get_sector_etf(&SectorId::ConsumerDiscretionary), Some("XLY".to_string()));
        
        // Test non-existent ETF mapping
        assert_eq!(mapper.get_sector_etf(&SectorId::Materials), None);
        assert_eq!(mapper.get_sector_etf(&SectorId::Custom(999)), None);
    }
    
    #[tokio::test]
    async fn test_update_sector() {
        let mapper = SectorMapper::new(SectorMapperConfig::default());
        
        // Initially AAPL is in technology
        let initial_sector = mapper.get_sector("AAPL").unwrap();
        assert_eq!(initial_sector.sector_id, SectorId::Technology);
        
        // Update AAPL to healthcare (hypothetical scenario)
        let result = mapper.update_sector("AAPL", SectorId::Healthcare, "Reclassification due to health focus").await;
        assert!(result.is_ok());
        
        // Verify update
        let updated_sector = mapper.get_sector("AAPL").unwrap();
        assert_eq!(updated_sector.sector_id, SectorId::Healthcare);
        assert_eq!(updated_sector.id, "healthcare");
        
        // Verify update was recorded
        let updates = mapper.sector_updates.read().await;
        assert_eq!(updates.len(), 1);
        
        let update = &updates[0];
        assert_eq!(update.symbol, "AAPL");
        assert_eq!(update.old_sector, Some(SectorId::Technology));
        assert_eq!(update.new_sector, SectorId::Healthcare);
        assert_eq!(update.reason, "Reclassification due to health focus");
        assert!(update.timestamp <= chrono::Utc::now());
    }
    
    #[tokio::test]
    async fn test_update_nonexistent_symbol() {
        let mapper = SectorMapper::new(SectorMapperConfig::default());
        
        // Try to update a symbol that doesn't exist
        let result = mapper.update_sector("NONEXISTENT", SectorId::Energy, "New company").await;
        assert!(result.is_ok());
        
        // Verify update was recorded with no old sector
        let updates = mapper.sector_updates.read().await;
        assert_eq!(updates.len(), 1);
        
        let update = &updates[0];
        assert_eq!(update.symbol, "NONEXISTENT");
        assert_eq!(update.old_sector, None);
        assert_eq!(update.new_sector, SectorId::Energy);
    }
    
    #[test]
    fn test_sector_statistics() {
        let mapper = SectorMapper::new(SectorMapperConfig::default());
        
        let stats = mapper.get_sector_stats();
        
        // Verify technology sector stats
        let tech_stats = stats.get(&SectorId::Technology).unwrap();
        assert_eq!(tech_stats.symbol_count, 3); // AAPL, MSFT, GOOGL
        assert_eq!(tech_stats.large_cap_count, 3);
        assert_eq!(tech_stats.mid_cap_count, 0);
        assert_eq!(tech_stats.small_cap_count, 0);
        
        // Total weight should be sum of individual weights
        let expected_tech_weight = 0.22 + 0.21 + 0.10; // AAPL + MSFT + GOOGL
        assert!((tech_stats.total_weight - expected_tech_weight).abs() < 0.01);
        
        // Verify financial services sector stats
        let finance_stats = stats.get(&SectorId::FinancialServices).unwrap();
        assert_eq!(finance_stats.symbol_count, 2); // JPM, BAC
        assert_eq!(finance_stats.large_cap_count, 2);
        
        let expected_finance_weight = 0.13 + 0.09; // JPM + BAC
        assert!((finance_stats.total_weight - expected_finance_weight).abs() < 0.01);
        
        // Verify healthcare sector stats
        let health_stats = stats.get(&SectorId::Healthcare).unwrap();
        assert_eq!(health_stats.symbol_count, 1); // JNJ
        assert_eq!(health_stats.large_cap_count, 1);
        assert_eq!(health_stats.total_weight, 0.12);
    }
    
    #[test]
    fn test_market_cap_tier_distribution() {
        let mapper = SectorMapper::new(SectorMapperConfig::default());
        
        // Add symbols with different market cap tiers
        mapper.add_symbol_mapping("SMALL_CAP", SectorInfo {
            id: "technology".to_string(),
            sector_id: SectorId::Technology,
            sub_sector: Some("Small Tech".to_string()),
            market_cap_tier: MarketCapTier::SmallCap,
            weight_in_sector: 0.01,
            correlation_group: None,
        });
        
        mapper.add_symbol_mapping("MID_CAP", SectorInfo {
            id: "technology".to_string(),
            sector_id: SectorId::Technology,
            sub_sector: Some("Mid Tech".to_string()),
            market_cap_tier: MarketCapTier::MidCap,
            weight_in_sector: 0.02,
            correlation_group: None,
        });
        
        let stats = mapper.get_sector_stats();
        let tech_stats = stats.get(&SectorId::Technology).unwrap();
        
        // Should now have: 3 large cap (default) + 1 mid cap + 1 small cap
        assert_eq!(tech_stats.symbol_count, 5);
        assert_eq!(tech_stats.large_cap_count, 3);
        assert_eq!(tech_stats.mid_cap_count, 1);
        assert_eq!(tech_stats.small_cap_count, 1);
    }
    
    #[test]
    fn test_correlation_groups() {
        let mapper = SectorMapper::new(SectorMapperConfig::default());
        
        // Test FAANG correlation group
        let aapl_sector = mapper.get_sector("AAPL").unwrap();
        let googl_sector = mapper.get_sector("GOOGL").unwrap();
        
        assert_eq!(aapl_sector.correlation_group, Some("FAANG".to_string()));
        assert_eq!(googl_sector.correlation_group, Some("FAANG".to_string()));
        
        // Test big banks correlation group
        let jpm_sector = mapper.get_sector("JPM").unwrap();
        let bac_sector = mapper.get_sector("BAC").unwrap();
        
        assert_eq!(jpm_sector.correlation_group, Some("big_banks".to_string()));
        assert_eq!(bac_sector.correlation_group, Some("big_banks".to_string()));
        
        // Test no correlation group
        let msft_sector = mapper.get_sector("MSFT").unwrap();
        let jnj_sector = mapper.get_sector("JNJ").unwrap();
        
        assert_eq!(msft_sector.correlation_group, None);
        assert_eq!(jnj_sector.correlation_group, None);
    }
    
    #[tokio::test]
    async fn test_load_from_config_placeholder() {
        let mut mapper = SectorMapper::new(SectorMapperConfig::default());
        
        // Test loading from config file (placeholder implementation)
        let result = mapper.load_from_config(&std::path::Path::new("/tmp/test_config.toml")).await;
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_default_config() {
        let config = SectorMapperConfig::default();
        
        assert!(config.enable_dynamic_updates);
        assert_eq!(config.cache_ttl_seconds, 3600);
        assert_eq!(config.default_sector, "technology");
    }
    
    #[test]
    fn test_custom_config() {
        let custom_config = SectorMapperConfig {
            enable_dynamic_updates: false,
            cache_ttl_seconds: 7200,
            default_sector: "financial_services".to_string(),
        };
        
        let mapper = SectorMapper::new(custom_config.clone());
        
        assert_eq!(mapper.config.enable_dynamic_updates, false);
        assert_eq!(mapper.config.cache_ttl_seconds, 7200);
        assert_eq!(mapper.config.default_sector, "financial_services");
    }
    
    #[test]
    fn test_sector_info_cloning() {
        let info = SectorInfo {
            id: "test_sector".to_string(),
            sector_id: SectorId::Technology,
            sub_sector: Some("Test Sub".to_string()),
            market_cap_tier: MarketCapTier::LargeCap,
            weight_in_sector: 0.15,
            correlation_group: Some("test_group".to_string()),
        };
        
        let cloned = info.clone();
        
        assert_eq!(info.id, cloned.id);
        assert_eq!(info.sector_id, cloned.sector_id);
        assert_eq!(info.sub_sector, cloned.sub_sector);
        assert_eq!(info.market_cap_tier, cloned.market_cap_tier);
        assert_eq!(info.weight_in_sector, cloned.weight_in_sector);
        assert_eq!(info.correlation_group, cloned.correlation_group);
    }
    
    #[test]
    fn test_concurrent_access() {
        use std::sync::Arc;
        use std::thread;
        
        let mapper = Arc::new(SectorMapper::new(SectorMapperConfig::default()));
        let mut handles = vec![];
        
        // Spawn multiple threads accessing the mapper concurrently
        for i in 0..5 {
            let mapper_clone = Arc::clone(&mapper);
            let handle = thread::spawn(move || {
                // Try to get sector for existing symbols
                let _ = mapper_clone.get_sector("AAPL");
                let _ = mapper_clone.get_sector("MSFT");
                
                // Try to get symbols in sector
                let _ = mapper_clone.get_symbols_in_sector(&SectorId::Technology);
                
                // Add new symbol
                let custom_info = create_custom_sector_info(SectorId::Energy, "Oil", 0.05);
                mapper_clone.add_symbol_mapping(&format!("OIL_{}", i), custom_info);
                
                // Get ETF mapping
                let _ = mapper_clone.get_sector_etf(&SectorId::Technology);
            });
            handles.push(handle);
        }
        
        // Wait for all threads to complete
        for handle in handles {
            handle.join().unwrap();
        }
        
        // Verify final state
        let stats = mapper.get_sector_stats();
        let energy_stats = stats.get(&SectorId::Energy);
        if let Some(energy_stats) = energy_stats {
            assert_eq!(energy_stats.symbol_count, 5); // 5 OIL_* symbols added
        }
    }
    
    #[tokio::test]
    async fn test_multiple_sector_updates() {
        let mapper = SectorMapper::new(SectorMapperConfig::default());
        
        // Perform multiple updates
        let updates = vec![
            ("AAPL", SectorId::Healthcare, "Health pivot"),
            ("MSFT", SectorId::Communication, "Cloud services focus"),
            ("GOOGL", SectorId::Communication, "Advertising platform"),
            ("JPM", SectorId::Technology, "Fintech transformation"),
        ];
        
        for (symbol, new_sector, reason) in updates {
            let result = mapper.update_sector(symbol, new_sector, reason).await;
            assert!(result.is_ok());
        }
        
        // Verify all updates were recorded
        let update_history = mapper.sector_updates.read().await;
        assert_eq!(update_history.len(), 4);
        
        // Verify sectors were actually updated
        assert_eq!(mapper.get_sector("AAPL").unwrap().sector_id, SectorId::Healthcare);
        assert_eq!(mapper.get_sector("MSFT").unwrap().sector_id, SectorId::Communication);
        assert_eq!(mapper.get_sector("GOOGL").unwrap().sector_id, SectorId::Communication);
        assert_eq!(mapper.get_sector("JPM").unwrap().sector_id, SectorId::Technology);
        
        // Check updated statistics
        let stats = mapper.get_sector_stats();
        
        // Technology sector should now have JPM (1 symbol)
        let tech_stats = stats.get(&SectorId::Technology).unwrap();
        assert_eq!(tech_stats.symbol_count, 1);
        
        // Communication sector should have MSFT and GOOGL (2 symbols)
        let comm_stats = stats.get(&SectorId::Communication).unwrap();
        assert_eq!(comm_stats.symbol_count, 2);
        
        // Healthcare should have AAPL and JNJ (2 symbols)
        let health_stats = stats.get(&SectorId::Healthcare).unwrap();
        assert_eq!(health_stats.symbol_count, 2);
    }
}