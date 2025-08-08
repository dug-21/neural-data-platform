/*!
 * Fair processing scheduler to prevent symbol monopolization.
 * 
 * Implements time-slice based fair scheduling ensuring no single symbol
 * consumes more than the configured percentage of processing time.
 */

use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};
use crate::multi_channel::{WorkItem, ProcessingPriority, SymbolStats};
use tracing;

/// Time window for tracking processing times
#[derive(Debug, Clone)]
pub struct TimeWindow {
    pub start_time: Instant,
    pub duration: Duration,
    pub processing_time: Duration,
}

impl TimeWindow {
    pub fn new(duration: Duration) -> Self {
        Self {
            start_time: Instant::now(),
            duration,
            processing_time: Duration::ZERO,
        }
    }
    
    pub fn add_processing_time(&mut self, time: Duration) {
        self.processing_time += time;
    }
    
    pub fn is_expired(&self) -> bool {
        self.start_time.elapsed() > self.duration
    }
    
    pub fn reset(&mut self) {
        self.start_time = Instant::now();
        self.processing_time = Duration::ZERO;
    }
    
    pub fn get_percentage(&self, total_time: Duration) -> f64 {
        if total_time.is_zero() {
            return 0.0;
        }
        self.processing_time.as_nanos() as f64 / total_time.as_nanos() as f64
    }
}

/// Fair processing scheduler
pub struct FairProcessingScheduler {
    /// Processing time tracking per symbol
    symbol_processing_times: HashMap<String, TimeWindow>,
    /// Total processing time for current window
    total_processing_time: TimeWindow,
    /// Symbol work queues
    symbol_queues: HashMap<String, VecDeque<WorkItem>>,
    /// Priority weights for symbols
    priority_weights: HashMap<String, f64>,
    /// Currently throttled symbols
    throttled_symbols: HashMap<String, Instant>,
    /// Configuration
    fairness_window: Duration,
    max_symbol_percentage: f64,
    /// Round-robin state
    current_symbol_index: usize,
    symbol_order: Vec<String>,
    /// Statistics
    processing_stats: HashMap<String, SymbolStats>,
    compliance_violations: u32,
    total_processed: u64,
}

impl FairProcessingScheduler {
    /// Create new fair processing scheduler
    pub fn new(fairness_window: Duration, max_symbol_percentage: f64) -> Self {
        Self {
            symbol_processing_times: HashMap::new(),
            total_processing_time: TimeWindow::new(fairness_window),
            symbol_queues: HashMap::new(),
            priority_weights: HashMap::new(),
            throttled_symbols: HashMap::new(),
            fairness_window,
            max_symbol_percentage,
            current_symbol_index: 0,
            symbol_order: Vec::new(),
            processing_stats: HashMap::new(),
            compliance_violations: 0,
            total_processed: 0,
        }
    }
    
    /// Check if a symbol should be processed (not throttled)
    pub fn should_process(&mut self, symbol: &str) -> bool {
        // Update time windows
        self.update_time_windows();
        
        // Check if symbol is throttled and ready for recovery
        if let Some(throttle_time) = self.throttled_symbols.get(symbol) {
            if throttle_time.elapsed() < Duration::from_secs(10) {
                return false; // Still throttled
            } else {
                // Remove from throttled list
                self.throttled_symbols.remove(symbol);
                tracing::info!("Symbol {} recovered from throttling", symbol);
            }
        }
        
        // Check current processing percentage
        let symbol_percentage = self.get_symbol_processing_percentage(symbol);
        
        if symbol_percentage > self.max_symbol_percentage {
            // Throttle this symbol
            self.throttled_symbols.insert(symbol.to_string(), Instant::now());
            self.compliance_violations += 1;
            
            tracing::warn!(
                "Throttling symbol {} - processing percentage: {:.2}% (max: {:.2}%)",
                symbol,
                symbol_percentage * 100.0,
                self.max_symbol_percentage * 100.0
            );
            
            return false;
        }
        
        true
    }
    
    /// Add work item to processing queue
    pub fn add_work_item(&mut self, work_item: WorkItem) {
        let symbol = work_item.symbol.clone();
        
        // Initialize symbol if new
        if !self.symbol_queues.contains_key(&symbol) {
            self.symbol_queues.insert(symbol.clone(), VecDeque::new());
            self.symbol_processing_times.insert(
                symbol.clone(), 
                TimeWindow::new(self.fairness_window)
            );
            self.priority_weights.insert(symbol.clone(), 1.0);
            self.processing_stats.insert(symbol.clone(), SymbolStats::default());
            
            if !self.symbol_order.contains(&symbol) {
                self.symbol_order.push(symbol.clone());
            }
        }
        
        // Add to queue
        if let Some(queue) = self.symbol_queues.get_mut(&symbol) {
            queue.push_back(work_item);
        }
    }
    
    /// Get next work item using fair round-robin scheduling
    pub fn get_next_work_item(&mut self) -> Option<WorkItem> {
        if self.symbol_order.is_empty() {
            return None;
        }
        
        let mut attempts = 0;
        let max_attempts = self.symbol_order.len();
        
        while attempts < max_attempts {
            let symbol = &self.symbol_order[self.current_symbol_index].clone();
            
            // Move to next symbol for round-robin
            self.current_symbol_index = (self.current_symbol_index + 1) % self.symbol_order.len();
            attempts += 1;
            
            // Check if symbol should be processed
            if !self.should_process(symbol) {
                continue;
            }
            
            // Get work item from this symbol's queue
            if let Some(queue) = self.symbol_queues.get_mut(symbol) {
                if let Some(work_item) = queue.pop_front() {
                    return Some(work_item);
                }
            }
        }
        
        None
    }
    
    /// Record processing completion
    pub fn record_processing_completion(
        &mut self, 
        symbol: &str, 
        processing_time: Duration
    ) {
        // Update symbol processing time
        if let Some(time_window) = self.symbol_processing_times.get_mut(symbol) {
            time_window.add_processing_time(processing_time);
        }
        
        // Update total processing time
        self.total_processing_time.add_processing_time(processing_time);
        
        // Update statistics
        if let Some(stats) = self.processing_stats.get_mut(symbol) {
            stats.messages_processed += 1;
            stats.total_processing_time += processing_time;
            stats.average_latency = stats.total_processing_time / stats.messages_processed as u32;
            stats.last_processed = Some(Instant::now());
        }
        
        self.total_processed += 1;
        
        // Check for fairness violations after processing
        let percentage = self.get_symbol_processing_percentage(symbol);
        if percentage > self.max_symbol_percentage {
            tracing::debug!(
                "Symbol {} processing percentage: {:.2}% (threshold: {:.2}%)",
                symbol, percentage * 100.0, self.max_symbol_percentage * 100.0
            );
        }
    }
    
    /// Get processing percentage for a symbol in current window
    pub fn get_symbol_processing_percentage(&self, symbol: &str) -> f64 {
        let symbol_time = self.symbol_processing_times.get(symbol)
            .map(|w| w.processing_time)
            .unwrap_or_default();
            
        let total_time = self.total_processing_time.processing_time;
        
        if total_time.is_zero() {
            return 0.0;
        }
        
        symbol_time.as_nanos() as f64 / total_time.as_nanos() as f64
    }
    
    /// Get priority for a symbol
    pub fn get_priority(&self, symbol: &str) -> f64 {
        // Lower processing percentage = higher priority
        let percentage = self.get_symbol_processing_percentage(symbol);
        let base_priority = 1.0 - percentage;
        
        // Apply priority weight
        let weight = self.priority_weights.get(symbol).unwrap_or(&1.0);
        
        base_priority * weight
    }
    
    /// Get compliance rate (percentage of time in compliance)
    pub fn get_compliance_rate(&self) -> f64 {
        if self.total_processed == 0 {
            return 1.0;
        }
        
        let violation_rate = self.compliance_violations as f64 / self.total_processed as f64;
        1.0 - violation_rate
    }
    
    /// Get current processing statistics
    pub fn get_processing_stats(&self) -> HashMap<String, SymbolStats> {
        self.processing_stats.clone()
    }
    
    /// Update time windows, resetting if expired
    fn update_time_windows(&mut self) {
        // Check if total time window expired
        if self.total_processing_time.is_expired() {
            self.total_processing_time.reset();
            
            // Reset all symbol time windows
            for (_, time_window) in self.symbol_processing_times.iter_mut() {
                time_window.reset();
            }
            
            tracing::debug!("Time windows reset for new fairness period");
        }
    }
    
    /// Set priority weight for a symbol
    pub fn set_priority_weight(&mut self, symbol: &str, weight: f64) {
        self.priority_weights.insert(symbol.to_string(), weight);
    }
    
    /// Get queue depth for a symbol
    pub fn get_queue_depth(&self, symbol: &str) -> usize {
        self.symbol_queues.get(symbol)
            .map(|q| q.len())
            .unwrap_or(0)
    }
    
    /// Get total queue depth across all symbols
    pub fn get_total_queue_depth(&self) -> usize {
        self.symbol_queues.values().map(|q| q.len()).sum()
    }
    
    /// Clear all queues (emergency stop)
    pub fn clear_all_queues(&mut self) {
        self.symbol_queues.clear();
        tracing::warn!("All processing queues cleared");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::MarketData;
    
    #[test]
    fn test_fair_scheduler_creation() {
        let scheduler = FairProcessingScheduler::new(
            Duration::from_secs(60), 
            0.20
        );
        
        assert_eq!(scheduler.max_symbol_percentage, 0.20);
        assert_eq!(scheduler.fairness_window, Duration::from_secs(60));
    }
    
    #[test]
    fn test_should_process_new_symbol() {
        let mut scheduler = FairProcessingScheduler::new(
            Duration::from_secs(60), 
            0.20
        );
        
        // New symbols should be allowed to process
        assert!(scheduler.should_process("AAPL"));
    }
    
    #[test]
    fn test_throttling_behavior() {
        let mut scheduler = FairProcessingScheduler::new(
            Duration::from_millis(100), // Short window for testing
            0.20
        );
        
        // Simulate heavy processing for one symbol
        scheduler.record_processing_completion("NVDA", Duration::from_millis(50));
        scheduler.record_processing_completion("AAPL", Duration::from_millis(10));
        
        // NVDA should be throttled (50ms out of 60ms = 83% > 20%)
        assert!(!scheduler.should_process("NVDA"));
        // AAPL should still be allowed
        assert!(scheduler.should_process("AAPL"));
    }
}