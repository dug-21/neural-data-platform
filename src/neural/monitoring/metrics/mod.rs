//! Metrics Collection and Aggregation Module
//!
//! This module provides comprehensive metrics collection, aggregation, and export capabilities
//! for the neural trading system. It enables real-time performance monitoring and analysis.

pub mod collector;
pub mod aggregator;  
pub mod exporter;

pub use collector::{MetricsCollector, MetricPoint, MetricUnit, MetricStatistics, CollectorConfig, CollectionStatistics};
pub use aggregator::{MetricsAggregator, AggregatedDataPoint, AggregationType, TimeWindow, AggregatorConfig, RealTimeStatistics};
pub use exporter::{MetricsExporter, ExportDestination, ExportFormat, ExporterConfig, ExportResult, ExportStatistics};

use anyhow::Result;
use tokio::sync::mpsc;
use tracing::info;

/// Complete metrics pipeline configuration
#[derive(Debug, Clone)]
pub struct MetricsPipelineConfig {
    pub collector: CollectorConfig,
    pub aggregator: AggregatorConfig,
    pub exporter: ExporterConfig,
    pub enable_collector: bool,
    pub enable_aggregator: bool,
    pub enable_exporter: bool,
}

impl Default for MetricsPipelineConfig {
    fn default() -> Self {
        Self {
            collector: CollectorConfig::default(),
            aggregator: AggregatorConfig::default(),
            exporter: ExporterConfig::default(),
            enable_collector: true,
            enable_aggregator: true,
            enable_exporter: true,
        }
    }
}

/// Complete metrics processing pipeline
pub struct MetricsPipeline {
    collector: Option<MetricsCollector>,
    aggregator: Option<MetricsAggregator>,
    exporter: Option<MetricsExporter>,
    config: MetricsPipelineConfig,
}

impl MetricsPipeline {
    /// Create a new metrics pipeline
    pub fn new(
        config: MetricsPipelineConfig,
        performance_events_rx: mpsc::UnboundedReceiver<super::performance_channel::PerformanceEvent>,
    ) -> Self {
        // Create the pipeline components
        let (collector, metric_rx) = if config.enable_collector {
            let (collector, rx) = MetricsCollector::new(config.collector.clone(), performance_events_rx);
            (Some(collector), Some(rx))
        } else {
            (None, None)
        };

        let (aggregator, aggregated_rx) = if config.enable_aggregator && metric_rx.is_some() {
            let (aggregator, rx) = MetricsAggregator::new(config.aggregator.clone(), metric_rx.unwrap());
            (Some(aggregator), Some(rx))
        } else {
            (None, None)
        };

        let exporter = if config.enable_exporter && aggregated_rx.is_some() {
            Some(MetricsExporter::new(config.exporter.clone(), aggregated_rx.unwrap(), None))
        } else {
            None
        };

        Self {
            collector,
            aggregator,
            exporter,
            config,
        }
    }

    /// Start the complete metrics pipeline
    pub async fn start(mut self) -> Result<()> {
        info!("Starting metrics pipeline with components: collector={}, aggregator={}, exporter={}", 
               self.config.enable_collector, self.config.enable_aggregator, self.config.enable_exporter);

        // Start all components concurrently
        let mut tasks = Vec::new();

        if let Some(mut collector) = self.collector.take() {
            tasks.push(tokio::spawn(async move {
                if let Err(e) = collector.start_collection().await {
                    tracing::error!("Metrics collector failed: {}", e);
                }
            }));
        }

        if let Some(mut aggregator) = self.aggregator.take() {
            tasks.push(tokio::spawn(async move {
                if let Err(e) = aggregator.start_aggregation().await {
                    tracing::error!("Metrics aggregator failed: {}", e);
                }
            }));
        }

        if let Some(mut exporter) = self.exporter.take() {
            tasks.push(tokio::spawn(async move {
                if let Err(e) = exporter.start_export().await {
                    tracing::error!("Metrics exporter failed: {}", e);
                }
            }));
        }

        // Wait for all tasks to complete (they should run indefinitely)
        for task in tasks {
            if let Err(e) = task.await {
                tracing::error!("Metrics pipeline task failed: {}", e);
            }
        }

        Ok(())
    }

    /// Get metrics statistics from collector
    pub async fn get_collection_statistics(&self) -> Option<collector::CollectionStatistics> {
        if let Some(collector) = &self.collector {
            Some(collector.get_collection_statistics().await)
        } else {
            None
        }
    }

    /// Get real-time statistics from aggregator
    pub async fn get_real_time_statistics(&self) -> Option<std::collections::HashMap<String, RealTimeStatistics>> {
        if let Some(aggregator) = &self.aggregator {
            Some(aggregator.get_real_time_statistics().await)
        } else {
            None
        }
    }

    /// Get export statistics from exporter
    pub async fn get_export_statistics(&self) -> Option<ExportStatistics> {
        if let Some(exporter) = &self.exporter {
            Some(exporter.get_export_statistics().await)
        } else {
            None
        }
    }

    /// Force immediate export
    pub async fn force_export(&self) -> Result<()> {
        if let Some(exporter) = &self.exporter {
            exporter.force_export().await
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn test_pipeline_creation() {
        let (_tx, rx) = mpsc::unbounded_channel();
        let config = MetricsPipelineConfig::default();
        
        let pipeline = MetricsPipeline::new(config, rx);
        assert!(pipeline.collector.is_some());
        assert!(pipeline.aggregator.is_some());
        assert!(pipeline.exporter.is_some());
    }

    #[tokio::test]
    async fn test_disabled_components() {
        let (_tx, rx) = mpsc::unbounded_channel();
        let config = MetricsPipelineConfig {
            enable_collector: false,
            enable_aggregator: false,
            enable_exporter: false,
            ..Default::default()
        };
        
        let pipeline = MetricsPipeline::new(config, rx);
        assert!(pipeline.collector.is_none());
        assert!(pipeline.aggregator.is_none());
        assert!(pipeline.exporter.is_none());
    }
}