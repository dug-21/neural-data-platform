# fe-007 Pseudocode: ndp-intelligence-app (Cycle Integration)

## Files Modified

- `apps/ndp-intelligence-app/src/main.rs` -- Feature flag, GrangerScanner lifecycle
- `apps/ndp-intelligence-app/src/config.rs` -- GrangerConfig loading from domain config
- `crates/ndp-intelligence/src/service.rs` -- run_cycle() Granger step, CycleSummary extension

## main.rs Changes

```pseudocode
// In AppConfig::from_env():
pub struct AppConfig {
    // ... existing fields ...
    pub granger_enabled: bool,
}

impl AppConfig:
    fn from_env() -> Result<Self>:
        // ... existing env var loading ...
        let granger_enabled = std::env::var("NDP_GRANGER_ENABLED")
            .map(|v| v == "true" || v == "1")
            .unwrap_or(false)

        Ok(Self {
            // ... existing ...
            granger_enabled,
        })

// In run_daemon():
async fn run_daemon():
    let app_config = AppConfig::from_env()?
    // ... existing pool, config, storage, service creation ...

    if app_config.granger_enabled:
        info!("Granger causality testing ENABLED")
    else:
        info!("Granger causality testing DISABLED (set NDP_GRANGER_ENABLED=true to enable)")

    // ... existing daemon loop (unchanged) ...
    // Granger integration happens inside IntelligenceService::run_cycle()
```

## config.rs Changes

```pseudocode
// DomainConfig already has intelligence: Option<IntelligenceConfig>
// IntelligenceConfig is defined in ndp-lib and now includes:
//   pub granger: Option<GrangerConfig>
// No changes needed in config.rs -- deserialization handles it automatically
// through serde(default) on the new field
```

## service.rs Changes

```pseudocode
// Extended CycleSummary:
pub struct CycleSummary {
    // ... existing fields ...
    pub granger_pairs_tested: usize,
    pub granger_significant: usize,
}

// Extended IntelligenceService:
pub struct IntelligenceService {
    // ... existing fields ...
    granger_enabled: bool,
    granger_config: Option<GrangerConfig>,
    granger_scanner: Option<GrangerScanner>,
    last_granger_run: Option<DateTime<Utc>>,
}

impl IntelligenceService:
    pub async fn new(
        app_config: &AppConfig,
        intelligence_config: &IntelligenceConfig,
        objectives: Vec<ObjectiveMetric>,
        pool: Arc<Pool>,
        storage: Arc<dyn StorageBackend>,
        primary_alias: String,
    ) -> Result<Self>:
        // ... existing initialization ...

        // Initialize Granger scanner if enabled
        let granger_scanner = if app_config.granger_enabled {
            intelligence_config.granger.as_ref().map(|gc| {
                GrangerScanner::new(pool.clone(), gc)
            })
        } else {
            None
        };

        Ok(Self {
            // ... existing fields ...
            granger_enabled: app_config.granger_enabled,
            granger_config: intelligence_config.granger.clone(),
            granger_scanner,
            last_granger_run: None,
        })

    pub async fn run_cycle(&mut self) -> Result<CycleSummary>:
        // ... existing steps 1-6 (OBSERVE through SEARCH) ...

        // 6.5. GRANGER: validate causal relationships (gated by interval)
        if let Some(scanner) = &self.granger_scanner {
            if self.should_run_granger() {
                let view_name = format!("gold.{}_aligned", self.domain_id.replace('-', "_"))
                let candidates = extract_candidates_from_knn(
                    &neighbors_from_search,  // reuse K-NN results from step 6
                    self.granger_config.as_ref().map(|c| c.candidate_count).unwrap_or(10),
                )

                match scanner.run_scan(&self.domain_id, &candidates, &view_name).await {
                    Ok(granger_summary) => {
                        summary.granger_pairs_tested = granger_summary.pairs_tested
                        summary.granger_significant = granger_summary.significant_count
                        self.last_granger_run = Some(Utc::now())
                        info!("Granger scan: {} tested, {} significant",
                            granger_summary.pairs_tested, granger_summary.significant_count)
                    }
                    Err(e) => {
                        warn!("Granger scan failed (non-fatal): {}", e)
                    }
                }
            }
        }

        // ... existing steps 7-8 (PREDICT, EVALUATE) ...

    fn should_run_granger(&self) -> bool:
        if !self.granger_enabled:
            return false
        let config = match &self.granger_config:
            Some(c) => c
            None => return false

        match self.last_granger_run:
            None => true  // never run before
            Some(last) =>
                let elapsed = Utc::now() - last
                elapsed >= chrono::Duration::hours(config.scan_interval_hours as i64)
```
