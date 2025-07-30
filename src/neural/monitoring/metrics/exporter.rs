//! Metrics Exporter
//!
//! Exports aggregated metrics to external systems like Prometheus, InfluxDB, or custom endpoints.
//! Provides flexible export formats and configurable destinations.

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, instrument, warn};

use super::aggregator::{AggregatedDataPoint, AggregationType};
use super::collector::{MetricPoint, MetricUnit};

/// Export destination configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExportDestination {
    Prometheus {
        endpoint: String,
        push_gateway: Option<String>,
        job_name: String,
        instance: String,
    },
    InfluxDB {
        url: String,
        database: String,
        username: Option<String>,
        password: Option<String>,
        retention_policy: Option<String>,
    },
    CloudWatch {
        region: String,
        namespace: String,
        access_key_id: Option<String>,
        secret_access_key: Option<String>,
    },
    Custom {
        endpoint: String,
        format: ExportFormat,
        headers: HashMap<String, String>,
        method: HttpMethod,
    },
    File {
        path: String,
        format: ExportFormat,
        rotation: FileRotation,
    },
    Console {
        format: ExportFormat,
        level: LogLevel,
    },
}

/// Export formats
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExportFormat {
    Prometheus,
    InfluxLineProtocol,
    Json,
    Csv,
    OpenTelemetry,
    StatsD,
    Custom(String),
}

/// HTTP methods for custom exports
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HttpMethod {
    GET,
    POST,
    PUT,
    PATCH,
}

/// File rotation strategy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FileRotation {
    None,
    Hourly,
    Daily,
    Weekly,
    SizeBased(u64), // bytes
}

/// Log levels for console export
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

/// Export batch configuration
#[derive(Debug, Clone)]
pub struct ExportBatch {
    pub max_size: usize,
    pub max_wait_time: chrono::Duration,
    pub compression: bool,
}

/// Exporter configuration
#[derive(Debug, Clone)]
pub struct ExporterConfig {
    pub destinations: Vec<ExportDestination>,
    pub batch_config: ExportBatch,
    pub export_interval: chrono::Duration,
    pub enable_buffering: bool,
    pub buffer_size: usize,
    pub retry_config: RetryConfig,
    pub filter_config: FilterConfig,
}

/// Retry configuration for failed exports
#[derive(Debug, Clone)]
pub struct RetryConfig {
    pub max_retries: u32,
    pub initial_delay: chrono::Duration,
    pub max_delay: chrono::Duration,
    pub exponential_backoff: bool,
}

/// Filter configuration for selective export
#[derive(Debug, Clone)]
pub struct FilterConfig {
    pub included_metrics: Vec<String>,
    pub excluded_metrics: Vec<String>,
    pub min_priority: Option<String>,
    pub tag_filters: HashMap<String, String>,
}

impl Default for ExporterConfig {
    fn default() -> Self {
        Self {
            destinations: vec![
                ExportDestination::Console {
                    format: ExportFormat::Json,
                    level: LogLevel::Info,
                }
            ],
            batch_config: ExportBatch {
                max_size: 100,
                max_wait_time: chrono::Duration::seconds(30),
                compression: false,
            },
            export_interval: chrono::Duration::seconds(60),
            enable_buffering: true,
            buffer_size: 10000,
            retry_config: RetryConfig {
                max_retries: 3,
                initial_delay: chrono::Duration::seconds(1),
                max_delay: chrono::Duration::seconds(60),
                exponential_backoff: true,
            },
            filter_config: FilterConfig {
                included_metrics: Vec::new(),
                excluded_metrics: Vec::new(),
                min_priority: None,
                tag_filters: HashMap::new(),
            },
        }
    }
}

/// Export result
#[derive(Debug)]
pub struct ExportResult {
    pub destination: String,
    pub success: bool,
    pub exported_count: usize,
    pub error: Option<String>,
    pub duration_ms: u64,
    pub timestamp: DateTime<Utc>,
}

/// Export statistics
#[derive(Debug, Default)]
pub struct ExportStatistics {
    pub total_exports: u64,
    pub successful_exports: u64,
    pub failed_exports: u64,
    pub total_data_points: u64,
    pub average_batch_size: f64,
    pub average_export_duration_ms: f64,
    pub last_export: Option<DateTime<Utc>>,
    pub destinations_status: HashMap<String, DestinationStatus>,
}

/// Status of export destination
#[derive(Debug, Clone)]
pub struct DestinationStatus {
    pub healthy: bool,
    pub last_success: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub consecutive_failures: u32,
}

/// Formatted export data
#[derive(Debug, Clone)]
pub struct FormattedExportData {
    pub format: ExportFormat,
    pub data: String,
    pub metadata: HashMap<String, String>,
}

/// Metrics exporter
pub struct MetricsExporter {
    config: ExporterConfig,
    
    // Data sources
    aggregated_rx: mpsc::UnboundedReceiver<AggregatedDataPoint>,
    raw_metrics_rx: Option<mpsc::UnboundedReceiver<MetricPoint>>,
    
    // Export buffer
    export_buffer: Arc<RwLock<Vec<AggregatedDataPoint>>>,
    
    // Statistics
    export_stats: Arc<RwLock<ExportStatistics>>,
    
    // HTTP client for external exports (placeholder - would use reqwest in production)
    // http_client: reqwest::Client,
}

impl MetricsExporter {
    /// Create a new metrics exporter
    pub fn new(
        config: ExporterConfig,
        aggregated_rx: mpsc::UnboundedReceiver<AggregatedDataPoint>,
        raw_metrics_rx: Option<mpsc::UnboundedReceiver<MetricPoint>>,
    ) -> Self {
        // HTTP client would be initialized here in production
        // let http_client = reqwest::Client::builder()
        //     .timeout(std::time::Duration::from_secs(30))
        //     .build()
        //     .expect("Failed to create HTTP client");
        
        Self {
            config,
            aggregated_rx,
            raw_metrics_rx,
            export_buffer: Arc::new(RwLock::new(Vec::new())),
            export_stats: Arc::new(RwLock::new(ExportStatistics::default())),
            // http_client,
        }
    }

    /// Start exporting metrics
    #[instrument(skip(self))]
    pub async fn start_export(&mut self) -> Result<()> {
        info!("Starting metrics export with {} destinations", self.config.destinations.len());
        
        let mut export_interval = tokio::time::interval(
            self.config.export_interval.to_std()?
        );
        
        loop {
            tokio::select! {
                // Process incoming aggregated data
                Some(data_point) = self.aggregated_rx.recv() => {
                    if let Err(e) = self.buffer_data_point(data_point).await {
                        error!("Failed to buffer data point: {}", e);
                    }
                }
                
                // Process raw metrics if available
                Some(metric) = async {
                    if let Some(ref mut rx) = self.raw_metrics_rx {
                        rx.recv().await
                    } else {
                        std::future::pending().await
                    }
                } => {
                    if let Err(e) = self.process_raw_metric(metric).await {
                        error!("Failed to process raw metric: {}", e);
                    }
                }
                
                // Periodic export
                _ = export_interval.tick() => {
                    if let Err(e) = self.perform_export().await {
                        error!("Failed to perform export: {}", e);
                    }
                }
            }
        }
    }

    /// Buffer aggregated data point
    async fn buffer_data_point(&self, data_point: AggregatedDataPoint) -> Result<()> {
        if !self.should_export_data_point(&data_point) {
            return Ok(());
        }
        
        let mut buffer = self.export_buffer.write().await;
        buffer.push(data_point);
        
        // Check if buffer is full and needs immediate export
        if buffer.len() >= self.config.batch_config.max_size {
            drop(buffer); // Release lock before export
            self.perform_export().await?;
        }
        
        Ok(())
    }

    /// Process raw metric (for direct export scenarios)
    async fn process_raw_metric(&self, _metric: MetricPoint) -> Result<()> {
        // For now, we focus on aggregated data
        // Raw metrics could be exported directly in some scenarios
        Ok(())
    }

    /// Check if data point should be exported based on filters
    fn should_export_data_point(&self, data_point: &AggregatedDataPoint) -> bool {
        let filter = &self.config.filter_config;
        
        // Check included metrics
        if !filter.included_metrics.is_empty() {
            if !filter.included_metrics.contains(&data_point.metric_name) {
                return false;
            }
        }
        
        // Check excluded metrics
        if filter.excluded_metrics.contains(&data_point.metric_name) {
            return false;
        }
        
        // Check tag filters
        for (key, expected_value) in &filter.tag_filters {
            if let Some(actual_value) = data_point.tags.get(key) {
                if actual_value != expected_value {
                    return false;
                }
            } else {
                return false;
            }
        }
        
        true
    }

    /// Perform export to all configured destinations
    async fn perform_export(&self) -> Result<()> {
        let data_points = {
            let mut buffer = self.export_buffer.write().await;
            if buffer.is_empty() {
                return Ok(());
            }
            
            let data = buffer.clone();
            buffer.clear();
            data
        };
        
        if data_points.is_empty() {
            return Ok(());
        }
        
        debug!("Exporting {} data points to {} destinations", 
               data_points.len(), self.config.destinations.len());
        
        let mut export_results = Vec::new();
        
        // Export to each destination
        for destination in &self.config.destinations {
            let result = self.export_to_destination(destination, &data_points).await;
            export_results.push(result);
        }
        
        // Update statistics
        self.update_export_statistics(&export_results, data_points.len()).await;
        
        Ok(())
    }

    /// Export to a specific destination
    async fn export_to_destination(
        &self,
        destination: &ExportDestination,
        data_points: &[AggregatedDataPoint],
    ) -> ExportResult {
        let start_time = std::time::Instant::now();
        let destination_name = self.get_destination_name(destination);
        
        let result = match destination {
            ExportDestination::Prometheus { .. } => {
                self.export_to_prometheus(destination, data_points).await
            }
            ExportDestination::InfluxDB { .. } => {
                self.export_to_influxdb(destination, data_points).await
            }
            ExportDestination::Custom { .. } => {
                self.export_to_custom(destination, data_points).await
            }
            ExportDestination::File { .. } => {
                self.export_to_file(destination, data_points).await
            }
            ExportDestination::Console { .. } => {
                self.export_to_console(destination, data_points).await
            }
            ExportDestination::CloudWatch { .. } => {
                self.export_to_cloudwatch(destination, data_points).await
            }
        };
        
        let duration = start_time.elapsed();
        
        match result {
            Ok(()) => ExportResult {
                destination: destination_name,
                success: true,
                exported_count: data_points.len(),
                error: None,
                duration_ms: duration.as_millis() as u64,
                timestamp: Utc::now(),
            },
            Err(e) => ExportResult {
                destination: destination_name,
                success: false,
                exported_count: 0,
                error: Some(e.to_string()),
                duration_ms: duration.as_millis() as u64,
                timestamp: Utc::now(),
            },
        }
    }

    /// Export to Prometheus
    async fn export_to_prometheus(
        &self,
        destination: &ExportDestination,
        data_points: &[AggregatedDataPoint],
    ) -> Result<()> {
        if let ExportDestination::Prometheus { endpoint: _, push_gateway, job_name, instance } = destination {
            let formatted_data = self.format_for_prometheus(data_points, job_name, instance)?;
            
            if push_gateway.is_some() {
                // HTTP client would be used here in production
                info!("Would push {} bytes to Prometheus gateway", formatted_data.data.len());
                // Simulate success for now
            } else {
                warn!("Direct Prometheus export not yet implemented, use push gateway");
            }
        }
        
        Ok(())
    }

    /// Export to InfluxDB
    async fn export_to_influxdb(
        &self,
        destination: &ExportDestination,
        data_points: &[AggregatedDataPoint],
    ) -> Result<()> {
        if let ExportDestination::InfluxDB { url, database, username: _, password: _, .. } = destination {
            let formatted_data = self.format_for_influxdb(data_points)?;
            
            // HTTP client would be used here in production
            info!("Would write {} bytes to InfluxDB at {}/write?db={}", 
                  formatted_data.data.len(), url, database);
        }
        
        Ok(())
    }

    /// Export to custom endpoint
    async fn export_to_custom(
        &self,
        destination: &ExportDestination,
        data_points: &[AggregatedDataPoint],
    ) -> Result<()> {
        if let ExportDestination::Custom { endpoint, format, headers: _, method } = destination {
            let formatted_data = self.format_data(data_points, format)?;
            
            // HTTP client would be used here in production
            info!("Would send {} bytes via {:?} to custom endpoint: {}", 
                  formatted_data.data.len(), method, endpoint);
        }
        
        Ok(())
    }

    /// Export to file
    async fn export_to_file(
        &self,
        destination: &ExportDestination,
        data_points: &[AggregatedDataPoint],
    ) -> Result<()> {
        if let ExportDestination::File { path, format, rotation } = destination {
            let formatted_data = self.format_data(data_points, format)?;
            let file_path = self.get_rotated_file_path(path, rotation);
            
            tokio::fs::write(&file_path, formatted_data.data).await?;
            debug!("Exported {} data points to file: {}", data_points.len(), file_path);
        }
        
        Ok(())
    }

    /// Export to console
    async fn export_to_console(
        &self,
        destination: &ExportDestination,
        data_points: &[AggregatedDataPoint],
    ) -> Result<()> {
        if let ExportDestination::Console { format, level } = destination {
            let formatted_data = self.format_data(data_points, format)?;
            
            match level {
                LogLevel::Debug => debug!("Metrics Export:\n{}", formatted_data.data),
                LogLevel::Info => info!("Metrics Export:\n{}", formatted_data.data),
                LogLevel::Warn => warn!("Metrics Export:\n{}", formatted_data.data),
                LogLevel::Error => error!("Metrics Export:\n{}", formatted_data.data),
            }
        }
        
        Ok(())
    }

    /// Export to CloudWatch (placeholder)
    async fn export_to_cloudwatch(
        &self,
        _destination: &ExportDestination,
        _data_points: &[AggregatedDataPoint],
    ) -> Result<()> {
        // CloudWatch export would be implemented here
        warn!("CloudWatch export not yet implemented");
        Ok(())
    }

    /// Format data according to specified format
    fn format_data(&self, data_points: &[AggregatedDataPoint], format: &ExportFormat) -> Result<FormattedExportData> {
        match format {
            ExportFormat::Json => self.format_as_json(data_points),
            ExportFormat::Csv => self.format_as_csv(data_points),
            ExportFormat::Prometheus => self.format_for_prometheus(data_points, "neural-trader", "default"),
            ExportFormat::InfluxLineProtocol => self.format_for_influxdb(data_points),
            ExportFormat::StatsD => self.format_as_statsd(data_points),
            _ => Err(anyhow::anyhow!("Unsupported format: {:?}", format)),
        }
    }

    /// Format as JSON
    fn format_as_json(&self, data_points: &[AggregatedDataPoint]) -> Result<FormattedExportData> {
        let json = serde_json::to_string_pretty(data_points)?;
        
        Ok(FormattedExportData {
            format: ExportFormat::Json,
            data: json,
            metadata: HashMap::from([
                ("content_type".to_string(), "application/json".to_string()),
            ]),
        })
    }

    /// Format as CSV
    fn format_as_csv(&self, data_points: &[AggregatedDataPoint]) -> Result<FormattedExportData> {
        let mut csv = String::from("timestamp,metric_name,aggregation_type,value,count,window_size,tags\n");
        
        for point in data_points {
            let tags_str = point.tags.iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect::<Vec<_>>()
                .join(";");
            
            csv.push_str(&format!(
                "{},{},{:?},{},{},{},{}\n",
                point.timestamp.format("%Y-%m-%d %H:%M:%S UTC"),
                point.metric_name,
                point.aggregation_type,
                point.value,
                point.count,
                point.window_size.num_seconds(),
                tags_str
            ));
        }
        
        Ok(FormattedExportData {
            format: ExportFormat::Csv,
            data: csv,
            metadata: HashMap::from([
                ("content_type".to_string(), "text/csv".to_string()),
            ]),
        })
    }

    /// Format for Prometheus
    fn format_for_prometheus(
        &self, 
        data_points: &[AggregatedDataPoint], 
        job_name: &str, 
        instance: &str
    ) -> Result<FormattedExportData> {
        let mut prometheus_data = String::new();
        
        for point in data_points {
            let metric_name = self.sanitize_prometheus_name(&point.metric_name, &point.aggregation_type);
            let timestamp = point.timestamp.timestamp_millis();
            
            // Build labels
            let mut labels = point.tags.clone();
            labels.insert("job".to_string(), job_name.to_string());
            labels.insert("instance".to_string(), instance.to_string());
            labels.insert("aggregation".to_string(), format!("{:?}", point.aggregation_type));
            
            let labels_str = labels.iter()
                .map(|(k, v)| format!("{}=\"{}\"", k, v))
                .collect::<Vec<_>>()
                .join(",");
            
            prometheus_data.push_str(&format!(
                "{}{{{}}}{} {} {}\n",
                metric_name,
                labels_str,
                "",
                point.value,
                timestamp
            ));
        }
        
        Ok(FormattedExportData {
            format: ExportFormat::Prometheus,
            data: prometheus_data,
            metadata: HashMap::from([
                ("content_type".to_string(), "text/plain".to_string()),
            ]),
        })
    }

    /// Format for InfluxDB line protocol
    fn format_for_influxdb(&self, data_points: &[AggregatedDataPoint]) -> Result<FormattedExportData> {
        let mut influx_data = String::new();
        
        for point in data_points {
            let measurement = format!("{}_{:?}", point.metric_name, point.aggregation_type)
                .to_lowercase()
                .replace(' ', "_");
            
            // Build tags
            let tags_str = point.tags.iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect::<Vec<_>>()
                .join(",");
            
            let timestamp = point.timestamp.timestamp_nanos();
            
            let line = if tags_str.is_empty() {
                format!("{} value={} {}\n", measurement, point.value, timestamp)
            } else {
                format!("{},{} value={} {}\n", measurement, tags_str, point.value, timestamp)
            };
            
            influx_data.push_str(&line);
        }
        
        Ok(FormattedExportData {
            format: ExportFormat::InfluxLineProtocol,
            data: influx_data,
            metadata: HashMap::from([
                ("content_type".to_string(), "text/plain".to_string()),
            ]),
        })
    }

    /// Format as StatsD
    fn format_as_statsd(&self, data_points: &[AggregatedDataPoint]) -> Result<FormattedExportData> {
        let mut statsd_data = String::new();
        
        for point in data_points {
            let metric_name = format!("{}_{:?}", point.metric_name, point.aggregation_type)
                .to_lowercase()
                .replace(' ', "_");
            
            // StatsD format: metric_name:value|type
            let statsd_type = match point.aggregation_type {
                AggregationType::Count => "c",
                AggregationType::Sum => "c",
                _ => "g", // gauge for most other types
            };
            
            statsd_data.push_str(&format!("{}:{}|{}\n", metric_name, point.value, statsd_type));
        }
        
        Ok(FormattedExportData {
            format: ExportFormat::StatsD,
            data: statsd_data,
            metadata: HashMap::from([
                ("content_type".to_string(), "text/plain".to_string()),
            ]),
        })
    }

    /// Sanitize metric name for Prometheus
    fn sanitize_prometheus_name(&self, metric_name: &str, agg_type: &AggregationType) -> String {
        let base_name = metric_name
            .replace('.', "_")
            .replace('-', "_")
            .to_lowercase();
        
        let suffix = match agg_type {
            AggregationType::Sum => "_total",
            AggregationType::Count => "_total",
            AggregationType::Rate => "_per_second",
            _ => "",
        };
        
        format!("{}{}", base_name, suffix)
    }

    /// Get rotated file path based on rotation strategy
    fn get_rotated_file_path(&self, base_path: &str, rotation: &FileRotation) -> String {
        match rotation {
            FileRotation::None => base_path.to_string(),
            FileRotation::Hourly => {
                let now = Utc::now();
                format!("{}.{}", base_path, now.format("%Y%m%d_%H"))
            }
            FileRotation::Daily => {
                let now = Utc::now();
                format!("{}.{}", base_path, now.format("%Y%m%d"))
            }
            FileRotation::Weekly => {
                let now = Utc::now();
                format!("{}.{}", base_path, now.format("%Y_W%W"))
            }
            FileRotation::SizeBased(_) => {
                // Would implement size-based rotation logic here
                base_path.to_string()
            }
        }
    }

    /// Get destination name for logging
    fn get_destination_name(&self, destination: &ExportDestination) -> String {
        match destination {
            ExportDestination::Prometheus { endpoint, .. } => format!("prometheus:{}", endpoint),
            ExportDestination::InfluxDB { url, database, .. } => format!("influxdb:{}:{}", url, database),
            ExportDestination::Custom { endpoint, .. } => format!("custom:{}", endpoint),
            ExportDestination::File { path, .. } => format!("file:{}", path),
            ExportDestination::Console { .. } => "console".to_string(),
            ExportDestination::CloudWatch { namespace, .. } => format!("cloudwatch:{}", namespace),
        }
    }

    /// Update export statistics
    async fn update_export_statistics(&self, results: &[ExportResult], data_point_count: usize) {
        let mut stats = self.export_stats.write().await;
        
        stats.total_exports += results.len() as u64;
        stats.total_data_points += data_point_count as u64;
        
        let successful_count = results.iter().filter(|r| r.success).count();
        stats.successful_exports += successful_count as u64;
        stats.failed_exports += (results.len() - successful_count) as u64;
        
        if stats.total_exports > 0 {
            stats.average_batch_size = stats.total_data_points as f64 / stats.total_exports as f64;
        }
        
        let total_duration: u64 = results.iter().map(|r| r.duration_ms).sum();
        if !results.is_empty() {
            stats.average_export_duration_ms = total_duration as f64 / results.len() as f64;
        }
        
        stats.last_export = Some(Utc::now());
        
        // Update destination status
        for result in results {
            let status = stats.destinations_status.entry(result.destination.clone())
                .or_insert_with(|| DestinationStatus {
                    healthy: true,
                    last_success: None,
                    last_error: None,
                    consecutive_failures: 0,
                });
            
            if result.success {
                status.healthy = true;
                status.last_success = Some(result.timestamp);
                status.consecutive_failures = 0;
            } else {
                status.healthy = false;
                status.last_error = result.error.clone();
                status.consecutive_failures += 1;
            }
        }
    }

    /// Get export statistics
    pub async fn get_export_statistics(&self) -> ExportStatistics {
        self.export_stats.read().await.clone()
    }

    /// Force immediate export
    pub async fn force_export(&self) -> Result<()> {
        self.perform_export().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn test_exporter_creation() {
        let (_tx, rx) = mpsc::unbounded_channel();
        let config = ExporterConfig::default();
        
        let exporter = MetricsExporter::new(config, rx, None);
        assert_eq!(exporter.config.destinations.len(), 1);
    }

    #[tokio::test]
    async fn test_json_formatting() {
        let (_tx, rx) = mpsc::unbounded_channel();
        let config = ExporterConfig::default();
        let exporter = MetricsExporter::new(config, rx, None);
        
        let data_points = vec![
            AggregatedDataPoint {
                timestamp: Utc::now(),
                metric_name: "test_metric".to_string(),
                aggregation_type: AggregationType::Average,
                value: 42.0,
                count: 10,
                window_size: chrono::Duration::minutes(1),
                tags: HashMap::from([("env".to_string(), "test".to_string())]),
            }
        ];
        
        let formatted = exporter.format_as_json(&data_points).unwrap();
        assert!(formatted.data.contains("test_metric"));
        assert!(formatted.data.contains("42"));
    }

    #[tokio::test]
    async fn test_prometheus_formatting() {
        let (_tx, rx) = mpsc::unbounded_channel();
        let config = ExporterConfig::default();
        let exporter = MetricsExporter::new(config, rx, None);
        
        let data_points = vec![
            AggregatedDataPoint {
                timestamp: Utc::now(),
                metric_name: "test.metric".to_string(),
                aggregation_type: AggregationType::Sum,
                value: 100.0,
                count: 5,
                window_size: chrono::Duration::minutes(5),
                tags: HashMap::from([("service".to_string(), "neural-trader".to_string())]),
            }
        ];
        
        let formatted = exporter.format_for_prometheus(&data_points, "test_job", "test_instance").unwrap();
        assert!(formatted.data.contains("test_metric_total"));
        assert!(formatted.data.contains("job=\"test_job\""));
        assert!(formatted.data.contains("100"));
    }
}