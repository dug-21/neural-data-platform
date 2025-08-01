//! Multi-Modal Fusion Engine
//! 
//! This module implements the core fusion engine that combines features
//! from different data modalities into unified feature vectors.

use super::*;
use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Multi-modal fusion engine
pub struct MultiModalFusionEngine {
    config: MultiModalConfig,
    temporal_aligner: Arc<TemporalAlignmentEngine>,
    normalizer: Arc<DataNormalizer>,
    missing_data_handler: Arc<MissingDataHandler>,
    dimensionality_reducer: Arc<RwLock<DimensionalityReducer>>,
    feature_store: Arc<MultiModalFeatureStore>,
    model_mapper: Arc<ModelFeatureMapper>,
    correlation_cache: Arc<RwLock<HashMap<String, f64>>>,
}

impl MultiModalFusionEngine {
    /// Create new fusion engine
    pub async fn new(config: MultiModalConfig) -> Result<Self> {
        let temporal_aligner = Arc::new(
            TemporalAlignmentEngine::new(config.alignment_window_seconds)
        );
        
        let normalizer = Arc::new(
            DataNormalizer::new(config.normalization_strategy.clone())
        );
        
        let missing_data_handler = Arc::new(
            MissingDataHandler::new(config.missing_data_tolerance)
        );
        
        let dimensionality_reducer = Arc::new(RwLock::new(
            DimensionalityReducer::new(config.target_feature_count)
        ));
        
        let feature_store = Arc::new(
            MultiModalFeatureStore::new(config.feature_store.clone()).await?
        );
        
        let model_mapper = Arc::new(
            ModelFeatureMapper::new(config.model_mapping.clone())
        );
        
        let correlation_cache = Arc::new(RwLock::new(HashMap::new()));
        
        Ok(Self {
            config,
            temporal_aligner,
            normalizer,
            missing_data_handler,
            dimensionality_reducer,
            feature_store,
            model_mapper,
            correlation_cache,
        })
    }
    
    /// Fuse multi-modal data into unified feature vector
    pub async fn fuse_features(
        &self,
        data: &MultiModalData,
        historical_context: Option<&[MultiModalData]>,
    ) -> Result<MultiModalFeatureResult> {
        let start_time = std::time::Instant::now();
        debug!("Starting multi-modal feature fusion for symbol: {}", data.symbol);
        
        // Step 1: Extract features from each modality
        let mut modality_features = HashMap::new();
        let mut data_completeness = HashMap::new();
        
        // Price features
        if let Some(price_data) = &data.price_data {
            let features = self.extract_price_features(price_data, historical_context).await?;
            let completeness = self.calculate_modality_completeness(&features);
            modality_features.insert(DataModality::Price, features);
            data_completeness.insert(DataModality::Price, completeness);
        }
        
        // Sentiment features
        if let Some(sentiment_data) = &data.sentiment_data {
            let features = self.extract_sentiment_features(sentiment_data, historical_context).await?;
            let completeness = self.calculate_modality_completeness(&features);
            modality_features.insert(DataModality::Sentiment, features);
            data_completeness.insert(DataModality::Sentiment, completeness);
        }
        
        // Economic features
        if let Some(economic_data) = &data.economic_data {
            let features = self.extract_economic_features(economic_data, historical_context).await?;
            let completeness = self.calculate_modality_completeness(&features);
            modality_features.insert(DataModality::Economic, features);
            data_completeness.insert(DataModality::Economic, completeness);
        }
        
        // Fundamental features
        if let Some(fundamental_data) = &data.fundamental_data {
            let features = self.extract_fundamental_features(fundamental_data, historical_context).await?;
            let completeness = self.calculate_modality_completeness(&features);
            modality_features.insert(DataModality::Fundamental, features);
            data_completeness.insert(DataModality::Fundamental, completeness);
        }
        
        // Order book features
        if let Some(orderbook_data) = &data.orderbook_data {
            let features = self.extract_orderbook_features(orderbook_data, historical_context).await?;
            let completeness = self.calculate_modality_completeness(&features);
            modality_features.insert(DataModality::OrderBook, features);
            data_completeness.insert(DataModality::OrderBook, completeness);
        }
        
        // Alternative data features
        if !data.alternative_data.is_empty() {
            let features = self.extract_alternative_features(&data.alternative_data, historical_context).await?;
            let completeness = self.calculate_modality_completeness(&features);
            modality_features.insert(DataModality::Alternative, features);
            data_completeness.insert(DataModality::Alternative, completeness);
        }
        
        // Step 2: Temporal alignment
        let aligned_features = self.temporal_aligner
            .align_features(&modality_features, data.timestamp)
            .await?;
        
        // Step 3: Handle missing data
        let cleaned_features = self.missing_data_handler
            .handle_missing_data(&aligned_features)
            .await?;
        
        // Step 4: Data normalization
        let normalized_features = self.normalizer
            .normalize_features(&cleaned_features)
            .await?;
        
        // Step 5: Cross-modal correlation analysis
        let cross_modal_correlations = if self.config.enable_cross_modal_correlations {
            self.compute_cross_modal_correlations(&normalized_features).await?
        } else {
            HashMap::new()
        };
        
        // Step 6: Feature fusion
        let mut fused_features = HashMap::new();
        
        // Combine all normalized features
        for (modality, features) in &normalized_features {
            let modality_prefix = modality.as_str();
            for (feature_name, value) in features {
                let fused_name = format!("{}_{}", modality_prefix, feature_name);
                fused_features.insert(fused_name, *value);
            }
        }
        
        // Add cross-modal correlation features
        for (correlation_name, value) in &cross_modal_correlations {
            let fused_name = format!("cross_modal_{}", correlation_name);
            fused_features.insert(fused_name, *value);
        }
        
        // Step 7: Dimensionality reduction (if enabled)
        let final_features = if self.config.enable_dimensionality_reduction {
            let mut reducer = self.dimensionality_reducer.write().await;
            reducer.reduce_features(&fused_features).await?
        } else {
            fused_features
        };
        
        // Step 8: Calculate quality metrics
        let processing_time = start_time.elapsed().as_millis() as f64;
        let alignment_quality = self.calculate_alignment_quality(&aligned_features);
        let cross_modal_consistency = self.calculate_cross_modal_consistency(&cross_modal_correlations);
        let overall_completeness = data_completeness.values().sum::<f64>() / data_completeness.len() as f64;
        
        let quality_metrics = QualityMetrics {
            overall_quality: self.calculate_overall_quality(&data_completeness, alignment_quality, cross_modal_consistency),
            data_completeness: overall_completeness,
            temporal_alignment_quality: alignment_quality,
            cross_modal_consistency,
            importance_balance: self.calculate_importance_balance(&final_features),
            processing_latency_ms: processing_time,
        };
        
        // Step 9: Create metadata
        let metadata = MultiModalMetadata {
            timestamp: data.timestamp,
            modalities_used: data.available_modalities(),
            feature_counts: modality_features.iter()
                .map(|(modality, features)| (*modality, features.len()))
                .collect(),
            processing_time_ms: processing_time,
            data_completeness,
            alignment_quality,
        };
        
        // Step 10: Store features
        self.feature_store
            .store_features(&data.symbol, &data.timestamp, &final_features, &metadata)
            .await?;
        
        let result = MultiModalFeatureResult {
            features: final_features,
            modality_features: normalized_features,
            cross_modal_correlations,
            metadata,
            quality_metrics,
        };
        
        info!("Multi-modal fusion completed for {} in {:.2}ms", 
              data.symbol, processing_time);
        
        Ok(result)
    }
    
    /// Get features optimized for specific model
    pub async fn get_model_features(
        &self,
        fusion_result: &MultiModalFeatureResult,
        model_name: &str,
    ) -> Result<HashMap<String, f64>> {
        self.model_mapper
            .map_features_for_model(model_name, &fusion_result.features)
            .await
    }
    
    /// Extract price features
    async fn extract_price_features(
        &self,
        price_data: &PriceData,
        historical_context: Option<&[MultiModalData]>,
    ) -> Result<HashMap<String, f64>> {
        let mut features = HashMap::new();
        
        // Basic OHLCV features
        features.insert("open".to_string(), price_data.open);
        features.insert("high".to_string(), price_data.high);
        features.insert("low".to_string(), price_data.low);
        features.insert("close".to_string(), price_data.close);
        features.insert("volume".to_string(), price_data.volume);
        
        // Derived price features
        features.insert("price_range".to_string(), price_data.high - price_data.low);
        features.insert("price_range_pct".to_string(), (price_data.high - price_data.low) / price_data.close);
        features.insert("body_size".to_string(), (price_data.close - price_data.open).abs());
        features.insert("body_size_pct".to_string(), (price_data.close - price_data.open).abs() / price_data.close);
        features.insert("upper_shadow".to_string(), price_data.high - price_data.close.max(price_data.open));
        features.insert("lower_shadow".to_string(), price_data.close.min(price_data.open) - price_data.low);
        
        // Add technical indicators
        features.extend(price_data.technical_indicators.clone());
        
        // Add microstructure features if available
        if let Some(microstructure) = &price_data.microstructure_features {
            features.extend(microstructure.clone());
        }
        
        // Historical context features
        if let Some(historical) = historical_context {
            let historical_features = self.extract_historical_price_context(historical).await?;
            features.extend(historical_features);
        }
        
        Ok(features)
    }
    
    /// Extract sentiment features
    async fn extract_sentiment_features(
        &self,
        sentiment_data: &SentimentData,
        _historical_context: Option<&[MultiModalData]>,
    ) -> Result<HashMap<String, f64>> {
        let mut features = HashMap::new();
        
        // Basic sentiment features
        features.insert("news_sentiment".to_string(), sentiment_data.news_sentiment);
        features.insert("social_sentiment".to_string(), sentiment_data.social_sentiment);
        features.insert("analyst_sentiment".to_string(), sentiment_data.analyst_sentiment);
        features.insert("sentiment_momentum".to_string(), sentiment_data.sentiment_momentum);
        features.insert("sentiment_volatility".to_string(), sentiment_data.sentiment_volatility);
        features.insert("news_volume".to_string(), sentiment_data.news_volume);
        features.insert("social_volume".to_string(), sentiment_data.social_volume);
        features.insert("sentiment_strength".to_string(), sentiment_data.sentiment_strength);
        
        // Composite sentiment features
        let avg_sentiment = (sentiment_data.news_sentiment + sentiment_data.social_sentiment + sentiment_data.analyst_sentiment) / 3.0;
        features.insert("avg_sentiment".to_string(), avg_sentiment);
        
        let sentiment_divergence = (sentiment_data.news_sentiment - sentiment_data.social_sentiment).abs();
        features.insert("sentiment_divergence".to_string(), sentiment_divergence);
        
        let volume_weighted_sentiment = (sentiment_data.news_sentiment * sentiment_data.news_volume + 
                                        sentiment_data.social_sentiment * sentiment_data.social_volume) /
                                       (sentiment_data.news_volume + sentiment_data.social_volume + 1e-8);
        features.insert("volume_weighted_sentiment".to_string(), volume_weighted_sentiment);
        
        // Entity sentiment features
        let entity_sentiment_avg = sentiment_data.entity_sentiment.values().sum::<f64>() / 
                                  (sentiment_data.entity_sentiment.len() as f64 + 1e-8);
        features.insert("entity_sentiment_avg".to_string(), entity_sentiment_avg);
        
        let entity_sentiment_std = {
            let variance = sentiment_data.entity_sentiment.values()
                .map(|&x| (x - entity_sentiment_avg).powi(2))
                .sum::<f64>() / (sentiment_data.entity_sentiment.len() as f64 + 1e-8);
            variance.sqrt()
        };
        features.insert("entity_sentiment_std".to_string(), entity_sentiment_std);
        
        // Topic sentiment features
        let topic_sentiment_avg = sentiment_data.topic_sentiment.values().sum::<f64>() / 
                                 (sentiment_data.topic_sentiment.len() as f64 + 1e-8);
        features.insert("topic_sentiment_avg".to_string(), topic_sentiment_avg);
        
        Ok(features)
    }
    
    /// Extract economic features
    async fn extract_economic_features(
        &self,
        economic_data: &EconomicData,
        _historical_context: Option<&[MultiModalData]>,
    ) -> Result<HashMap<String, f64>> {
        let mut features = HashMap::new();
        
        // Macro indicators
        if let Some(gdp) = economic_data.gdp_growth {
            features.insert("gdp_growth".to_string(), gdp);
        }
        if let Some(inflation) = economic_data.inflation_rate {
            features.insert("inflation_rate".to_string(), inflation);
        }
        if let Some(unemployment) = economic_data.unemployment_rate {
            features.insert("unemployment_rate".to_string(), unemployment);  
        }
        if let Some(interest_rate) = economic_data.interest_rate {
            features.insert("interest_rate".to_string(), interest_rate);
        }
        if let Some(cb_rate) = economic_data.central_bank_rate {
            features.insert("central_bank_rate".to_string(), cb_rate);
        }
        if let Some(money_supply) = economic_data.money_supply_m2 {
            features.insert("money_supply_m2".to_string(), money_supply);
        }
        if let Some(trade_balance) = economic_data.trade_balance {
            features.insert("trade_balance".to_string(), trade_balance);
        }
        if let Some(consumer_conf) = economic_data.consumer_confidence {
            features.insert("consumer_confidence".to_string(), consumer_conf);
        }
        if let Some(mfg_pmi) = economic_data.manufacturing_pmi {
            features.insert("manufacturing_pmi".to_string(), mfg_pmi);
        }
        if let Some(svc_pmi) = economic_data.services_pmi {
            features.insert("services_pmi".to_string(), svc_pmi);
        }
        
        // Currency strength features
        for (currency, strength) in &economic_data.currency_strength {
            features.insert(format!("currency_strength_{}", currency), *strength);
        }
        
        // Commodity price features
        for (commodity, price) in &economic_data.commodity_prices {
            features.insert(format!("commodity_{}", commodity), *price);
        }
        
        // Yield curve features
        for (maturity, yield_val) in &economic_data.yield_curve {
            features.insert(format!("yield_{}", maturity), *yield_val);
        }
        
        // Derived economic features
        if let (Some(short_yield), Some(long_yield)) = (economic_data.yield_curve.get("2Y"), economic_data.yield_curve.get("10Y")) {
            features.insert("yield_curve_slope".to_string(), long_yield - short_yield);
        }
        
        Ok(features)
    }
    
    /// Extract fundamental features
    async fn extract_fundamental_features(
        &self,
        fundamental_data: &FundamentalData,
        _historical_context: Option<&[MultiModalData]>,
    ) -> Result<HashMap<String, f64>> {
        let mut features = HashMap::new();
        
        // Valuation metrics
        if let Some(market_cap) = fundamental_data.market_cap {
            features.insert("market_cap".to_string(), market_cap);
            features.insert("log_market_cap".to_string(), market_cap.ln());
        }
        if let Some(pe) = fundamental_data.pe_ratio {
            features.insert("pe_ratio".to_string(), pe);
        }
        if let Some(pb) = fundamental_data.pb_ratio {
            features.insert("pb_ratio".to_string(), pb);
        }
        
        // Financial health metrics
        if let Some(debt_equity) = fundamental_data.debt_to_equity {
            features.insert("debt_to_equity".to_string(), debt_equity);
        }
        if let Some(roe) = fundamental_data.return_on_equity {
            features.insert("return_on_equity".to_string(), roe);
        }
        if let Some(roa) = fundamental_data.return_on_assets {
            features.insert("return_on_assets".to_string(), roa);
        }
        
        // Growth metrics
        if let Some(rev_growth) = fundamental_data.revenue_growth {
            features.insert("revenue_growth".to_string(), rev_growth);
        }
        if let Some(earn_growth) = fundamental_data.earnings_growth {
            features.insert("earnings_growth".to_string(), earn_growth);
        }
        
        // Cash flow and dividends
        if let Some(fcf) = fundamental_data.free_cash_flow {
            features.insert("free_cash_flow".to_string(), fcf);
        }
        if let Some(div_yield) = fundamental_data.dividend_yield {
            features.insert("dividend_yield".to_string(), div_yield);
        }
        
        // Per-share metrics
        if let Some(bvps) = fundamental_data.book_value_per_share {
            features.insert("book_value_per_share".to_string(), bvps);
        }
        if let Some(eps) = fundamental_data.earnings_per_share {
            features.insert("earnings_per_share".to_string(), eps);
        }
        if let Some(rps) = fundamental_data.revenue_per_share {
            features.insert("revenue_per_share".to_string(), rps);
        }
        
        // Sector and industry features
        for (metric, value) in &fundamental_data.sector_metrics {
            features.insert(format!("sector_{}", metric), *value);
        }
        for (metric, value) in &fundamental_data.industry_metrics {
            features.insert(format!("industry_{}", metric), *value);
        }
        
        // Peer comparison features
        for (metric, value) in &fundamental_data.peer_comparison {
            features.insert(format!("peer_{}", metric), *value);
        }
        
        Ok(features)
    }
    
    /// Extract order book features
    async fn extract_orderbook_features(
        &self,
        orderbook_data: &OrderBookData,
        _historical_context: Option<&[MultiModalData]>,
    ) -> Result<HashMap<String, f64>> {
        let mut features = HashMap::new();
        
        // Basic microstructure features
        features.insert("bid_price".to_string(), orderbook_data.bid_price);
        features.insert("ask_price".to_string(), orderbook_data.ask_price);
        features.insert("bid_size".to_string(), orderbook_data.bid_size);
        features.insert("ask_size".to_string(), orderbook_data.ask_size);
        features.insert("spread".to_string(), orderbook_data.spread);
        features.insert("mid_price".to_string(), orderbook_data.mid_price);
        features.insert("spread_pct".to_string(), orderbook_data.spread / orderbook_data.mid_price);
        
        // Order flow features
        features.insert("imbalance".to_string(), orderbook_data.imbalance);
        features.insert("depth_imbalance".to_string(), orderbook_data.depth_imbalance);
        features.insert("order_flow".to_string(), orderbook_data.order_flow);
        features.insert("trade_intensity".to_string(), orderbook_data.trade_intensity);
        features.insert("volatility_estimate".to_string(), orderbook_data.volatility_estimate);
        features.insert("liquidity_score".to_string(), orderbook_data.liquidity_score);
        
        // Level-based features
        let bid_levels: Vec<_> = orderbook_data.level_data.iter()
            .filter(|level| matches!(level.side, OrderSide::Bid))
            .collect();
        let ask_levels: Vec<_> = orderbook_data.level_data.iter()
            .filter(|level| matches!(level.side, OrderSide::Ask))
            .collect();
        
        // Weighted average prices
        let bid_weighted_price = bid_levels.iter()
            .map(|level| level.price * level.size)
            .sum::<f64>() / (bid_levels.iter().map(|level| level.size).sum::<f64>() + 1e-8);
        features.insert("bid_weighted_price".to_string(), bid_weighted_price);
        
        let ask_weighted_price = ask_levels.iter()
            .map(|level| level.price * level.size)
            .sum::<f64>() / (ask_levels.iter().map(|level| level.size).sum::<f64>() + 1e-8);
        features.insert("ask_weighted_price".to_string(), ask_weighted_price);
        
        // Order book depth
        let total_bid_size = bid_levels.iter().map(|level| level.size).sum::<f64>();
        let total_ask_size = ask_levels.iter().map(|level| level.size).sum::<f64>();
        features.insert("total_bid_size".to_string(), total_bid_size);
        features.insert("total_ask_size".to_string(), total_ask_size);
        features.insert("size_ratio".to_string(), total_bid_size / (total_ask_size + 1e-8));
        
        Ok(features)
    }
    
    /// Extract alternative data features
    async fn extract_alternative_features(
        &self,
        alternative_data: &[AlternativeData],
        _historical_context: Option<&[MultiModalData]>,
    ) -> Result<HashMap<String, f64>> {
        let mut features = HashMap::new();
        
        // Group by data type
        let mut data_by_type: HashMap<AlternativeDataType, Vec<&AlternativeData>> = HashMap::new();
        for data in alternative_data {
            data_by_type.entry(data.data_type.clone()).or_default().push(data);
        }
        
        // Extract features for each data type
        for (data_type, data_points) in data_by_type {
            let type_name = format!("{:?}", data_type).to_lowercase();
            
            // Aggregate statistics
            let values: Vec<f64> = data_points.iter().map(|d| d.value).collect();
            let confidences: Vec<f64> = data_points.iter().map(|d| d.confidence).collect();
            
            if !values.is_empty() {
                let avg_value = values.iter().sum::<f64>() / values.len() as f64;
                let avg_confidence = confidences.iter().sum::<f64>() / confidences.len() as f64;
                
                features.insert(format!("alt_{}_avg", type_name), avg_value);
                features.insert(format!("alt_{}_confidence", type_name), avg_confidence);
                features.insert(format!("alt_{}_count", type_name), values.len() as f64);
                
                // Value volatility
                let variance = values.iter()
                    .map(|&x| (x - avg_value).powi(2))
                    .sum::<f64>() / (values.len() as f64 - 1.0);
                features.insert(format!("alt_{}_volatility", type_name), variance.sqrt());
            }
        }
        
        Ok(features)
    }
    
    /// Extract historical price context features
    async fn extract_historical_price_context(
        &self,
        historical: &[MultiModalData],
    ) -> Result<HashMap<String, f64>> {
        let mut features = HashMap::new();
        
        let prices: Vec<f64> = historical.iter()
            .filter_map(|data| data.price_data.as_ref().map(|pd| pd.close))
            .collect();
        
        if prices.len() >= 2 {
            // Price momentum features
            let recent_prices = &prices[prices.len().saturating_sub(10)..];
            let short_ma = recent_prices.iter().sum::<f64>() / recent_prices.len() as f64;
            
            let longer_prices = &prices[prices.len().saturating_sub(20)..];
            let long_ma = longer_prices.iter().sum::<f64>() / longer_prices.len() as f64;
            
            features.insert("price_momentum".to_string(), (short_ma / long_ma) - 1.0);
            
            // Volatility features
            let returns: Vec<f64> = prices.windows(2)
                .map(|w| (w[1] / w[0]).ln())
                .collect();
            
            if !returns.is_empty() {
                let mean_return = returns.iter().sum::<f64>() / returns.len() as f64;
                let variance = returns.iter()
                    .map(|&r| (r - mean_return).powi(2))
                    .sum::<f64>() / (returns.len() - 1) as f64;
                
                features.insert("historical_volatility".to_string(), variance.sqrt());
                features.insert("historical_return".to_string(), mean_return);
            }
        }
        
        Ok(features)
    }
    
    /// Compute cross-modal correlations
    async fn compute_cross_modal_correlations(
        &self,
        modality_features: &HashMap<DataModality, HashMap<String, f64>>,
    ) -> Result<HashMap<String, f64>> {
        let mut correlations = HashMap::new();
        
        let modalities: Vec<_> = modality_features.keys().collect();
        
        // Compute pairwise correlations between modalities
        for i in 0..modalities.len() {
            for j in i+1..modalities.len() {
                let mod1 = modalities[i];
                let mod2 = modalities[j];
                
                if let (Some(features1), Some(features2)) = (
                    modality_features.get(mod1),
                    modality_features.get(mod2)
                ) {
                    let correlation = self.compute_feature_correlation(features1, features2).await;
                    let correlation_name = format!("{}_{}", mod1.as_str(), mod2.as_str());
                    correlations.insert(correlation_name, correlation);
                }
            }
        }
        
        Ok(correlations)
    }
    
    /// Compute correlation between two feature sets
    async fn compute_feature_correlation(
        &self,
        features1: &HashMap<String, f64>,
        features2: &HashMap<String, f64>,
    ) -> f64 {
        // Simplified correlation based on feature value distributions
        let values1: Vec<f64> = features1.values().cloned().collect();
        let values2: Vec<f64> = features2.values().cloned().collect();
        
        if values1.is_empty() || values2.is_empty() {
            return 0.0;
        }
        
        let mean1 = values1.iter().sum::<f64>() / values1.len() as f64;
        let mean2 = values2.iter().sum::<f64>() / values2.len() as f64;
        
        // Use Jensen-Shannon divergence as a correlation measure
        let mut js_divergence = 0.0;
        let min_len = values1.len().min(values2.len());
        
        for i in 0..min_len {
            let p1 = (values1[i] - mean1).abs() + 1e-8;
            let p2 = (values2[i] - mean2).abs() + 1e-8;
            let m = (p1 + p2) / 2.0;
            
            js_divergence += 0.5 * (p1 * (p1 / m).ln() + p2 * (p2 / m).ln());
        }
        
        // Convert divergence to correlation-like measure
        (-js_divergence).exp()
    }
    
    /// Helper methods for quality calculations
    fn calculate_modality_completeness(&self, features: &HashMap<String, f64>) -> f64 {
        let non_nan_count = features.values()
            .filter(|&&v| !v.is_nan())
            .count();
        non_nan_count as f64 / features.len() as f64
    }
    
    fn calculate_alignment_quality(&self, _aligned_features: &HashMap<DataModality, HashMap<String, f64>>) -> f64 {
        // Simplified alignment quality - in practice would measure temporal consistency
        0.85
    }
    
    fn calculate_cross_modal_consistency(&self, correlations: &HashMap<String, f64>) -> f64 {
        if correlations.is_empty() {
            return 1.0;
        }
        correlations.values().sum::<f64>() / correlations.len() as f64
    }
    
    fn calculate_overall_quality(&self, completeness: &HashMap<DataModality, f64>, alignment: f64, consistency: f64) -> f64 {
        let avg_completeness = completeness.values().sum::<f64>() / completeness.len() as f64;
        (avg_completeness * 0.4) + (alignment * 0.3) + (consistency * 0.3)
    }
    
    fn calculate_importance_balance(&self, features: &HashMap<String, f64>) -> f64 {
        if features.is_empty() {
            return 0.0;
        }
        
        let values: Vec<f64> = features.values().map(|v| v.abs()).collect();
        let mean = values.iter().sum::<f64>() / values.len() as f64;
        let variance = values.iter()
            .map(|&v| (v - mean).powi(2))
            .sum::<f64>() / values.len() as f64;
        
        // Lower variance indicates better balance
        1.0 / (1.0 + variance)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[tokio::test]
    async fn test_fusion_engine_creation() {
        let config = MultiModalConfig::default();
        let engine = MultiModalFusionEngine::new(config).await;
        assert!(engine.is_ok());
    }

    #[tokio::test]
    async fn test_price_feature_extraction() {
        let config = MultiModalConfig::default();
        let engine = MultiModalFusionEngine::new(config).await.unwrap();
        
        let price_data = PriceData {
            timestamp: Utc::now(),
            symbol: "AAPL".to_string(),
            open: 150.0,
            high: 155.0,
            low: 148.0,
            close: 152.0,
            volume: 1000000.0,
            technical_indicators: HashMap::new(),
            microstructure_features: None,
        };
        
        let features = engine.extract_price_features(&price_data, None).await.unwrap();
        assert!(features.contains_key("open"));
        assert!(features.contains_key("close"));
        assert!(features.contains_key("price_range"));
        assert!(features.contains_key("body_size"));
    }
}