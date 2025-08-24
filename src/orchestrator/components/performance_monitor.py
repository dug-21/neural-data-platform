"""
Performance monitoring component for Phase 3 orchestrator.
Tracks execution times, resource usage, and performance metrics.
"""

import time
from typing import Dict, List, Any, Optional
from datetime import datetime, timedelta
from statistics import mean, median, stdev
from collections import defaultdict, deque
import logging


class PerformanceMonitor:
    """Performance monitoring and metrics collection for orchestrator"""
    
    def __init__(self, history_size: int = 1000):
        """
        Initialize performance monitor.
        
        Args:
            history_size: Maximum number of metrics to keep in history
        """
        self.history_size = history_size
        self.metrics: Dict[str, deque] = defaultdict(lambda: deque(maxlen=history_size))
        self.operation_times: Dict[str, List[float]] = defaultdict(list)
        self.performance_targets: Dict[str, float] = {}
        self.logger = logging.getLogger(__name__)
        
        # Set default performance targets (in milliseconds)
        self.set_performance_targets({
            'swarm_init_duration_ms': 100.0,
            'agent_spawn_avg_duration_ms': 10.0,
            'architecture_comprehension_duration_ms': 200.0,
            'validation_pipeline_duration_ms': 500.0
        })
    
    def record_metric(self, metric_name: str, value: float, timestamp: Optional[datetime] = None):
        """
        Record a performance metric.
        
        Args:
            metric_name: Name of the metric
            value: Metric value
            timestamp: Optional timestamp (defaults to now)
        """
        if timestamp is None:
            timestamp = datetime.utcnow()
        
        metric_entry = {
            'value': value,
            'timestamp': timestamp.isoformat()
        }
        
        self.metrics[metric_name].append(metric_entry)
        
        # Check against performance targets
        if metric_name in self.performance_targets:
            target = self.performance_targets[metric_name]
            if value > target:
                self.logger.warning(
                    f"Performance target exceeded for {metric_name}: "
                    f"{value:.2f} > {target:.2f}"
                )
    
    def set_performance_targets(self, targets: Dict[str, float]):
        """
        Set performance targets for metrics.
        
        Args:
            targets: Dict mapping metric names to target values
        """
        self.performance_targets.update(targets)
        self.logger.info(f"Updated performance targets: {targets}")
    
    def get_metric_statistics(self, metric_name: str, time_window_hours: float = 24.0) -> Dict[str, Any]:
        """
        Get statistics for a specific metric.
        
        Args:
            metric_name: Name of metric to analyze
            time_window_hours: Hours to look back for analysis
            
        Returns:
            Dict with metric statistics
        """
        if metric_name not in self.metrics:
            return {'error': f'Metric {metric_name} not found'}
        
        # Filter by time window
        cutoff_time = datetime.utcnow() - timedelta(hours=time_window_hours)
        
        values = []
        for entry in self.metrics[metric_name]:
            entry_time = datetime.fromisoformat(entry['timestamp'])
            if entry_time >= cutoff_time:
                values.append(entry['value'])
        
        if not values:
            return {'error': f'No data for {metric_name} in last {time_window_hours} hours'}
        
        # Calculate statistics
        stats = {
            'metric_name': metric_name,
            'count': len(values),
            'min': min(values),
            'max': max(values),
            'mean': mean(values),
            'median': median(values),
            'time_window_hours': time_window_hours
        }
        
        # Add standard deviation if enough data points
        if len(values) > 1:
            stats['std_dev'] = stdev(values)
        
        # Add performance target comparison
        if metric_name in self.performance_targets:
            target = self.performance_targets[metric_name]
            stats['target'] = target
            stats['target_met_percentage'] = (sum(1 for v in values if v <= target) / len(values)) * 100
            stats['avg_over_target'] = max(0, stats['mean'] - target)
        
        return stats
    
    def get_all_metrics_summary(self, time_window_hours: float = 1.0) -> Dict[str, Any]:
        """
        Get summary of all tracked metrics.
        
        Args:
            time_window_hours: Hours to look back for analysis
            
        Returns:
            Dict with summary of all metrics
        """
        summary = {
            'timestamp': datetime.utcnow().isoformat(),
            'time_window_hours': time_window_hours,
            'metrics': {}
        }
        
        for metric_name in self.metrics.keys():
            stats = self.get_metric_statistics(metric_name, time_window_hours)
            if 'error' not in stats:
                summary['metrics'][metric_name] = stats
        
        # Overall performance assessment
        performance_issues = []
        for metric_name, stats in summary['metrics'].items():
            if 'target_met_percentage' in stats and stats['target_met_percentage'] < 80:
                performance_issues.append(f"{metric_name}: {stats['target_met_percentage']:.1f}% meeting target")
        
        summary['performance_issues'] = performance_issues
        summary['overall_health'] = 'good' if len(performance_issues) == 0 else 'needs_attention'
        
        return summary
    
    def detect_performance_degradation(self, metric_name: str, threshold_percentage: float = 20.0) -> Dict[str, Any]:
        """
        Detect performance degradation by comparing recent vs historical performance.
        
        Args:
            metric_name: Metric to analyze
            threshold_percentage: Threshold for degradation detection
            
        Returns:
            Dict with degradation analysis
        """
        if metric_name not in self.metrics or len(self.metrics[metric_name]) < 10:
            return {'error': 'Insufficient data for degradation analysis'}
        
        # Get recent vs historical data
        all_entries = list(self.metrics[metric_name])
        recent_entries = all_entries[-10:]  # Last 10 entries
        historical_entries = all_entries[:-10] if len(all_entries) > 10 else []
        
        if not historical_entries:
            return {'error': 'Insufficient historical data'}
        
        recent_avg = mean([entry['value'] for entry in recent_entries])
        historical_avg = mean([entry['value'] for entry in historical_entries])
        
        # Calculate percentage change
        percentage_change = ((recent_avg - historical_avg) / historical_avg) * 100
        
        degraded = percentage_change > threshold_percentage
        
        return {
            'metric_name': metric_name,
            'recent_average': recent_avg,
            'historical_average': historical_avg,
            'percentage_change': percentage_change,
            'threshold_percentage': threshold_percentage,
            'degradation_detected': degraded,
            'severity': 'high' if percentage_change > threshold_percentage * 2 else 'medium' if degraded else 'low'
        }
    
    def get_performance_trends(self, hours_back: int = 24) -> Dict[str, Any]:
        """
        Get performance trends over time.
        
        Args:
            hours_back: Hours to analyze
            
        Returns:
            Dict with trend analysis
        """
        trends = {}
        cutoff_time = datetime.utcnow() - timedelta(hours=hours_back)
        
        for metric_name in self.metrics.keys():
            # Group by hour buckets
            hourly_buckets = defaultdict(list)
            
            for entry in self.metrics[metric_name]:
                entry_time = datetime.fromisoformat(entry['timestamp'])
                if entry_time >= cutoff_time:
                    hour_bucket = entry_time.strftime('%Y-%m-%d-%H')
                    hourly_buckets[hour_bucket].append(entry['value'])
            
            if len(hourly_buckets) < 2:
                continue
            
            # Calculate hourly averages
            hourly_averages = []
            for hour in sorted(hourly_buckets.keys()):
                hourly_avg = mean(hourly_buckets[hour])
                hourly_averages.append(hourly_avg)
            
            # Determine trend
            if len(hourly_averages) >= 2:
                first_half_avg = mean(hourly_averages[:len(hourly_averages)//2])
                second_half_avg = mean(hourly_averages[len(hourly_averages)//2:])
                
                trend_direction = 'improving' if second_half_avg < first_half_avg else 'degrading' if second_half_avg > first_half_avg else 'stable'
                trend_magnitude = abs(second_half_avg - first_half_avg)
                
                trends[metric_name] = {
                    'direction': trend_direction,
                    'magnitude': trend_magnitude,
                    'first_half_avg': first_half_avg,
                    'second_half_avg': second_half_avg,
                    'data_points': len(hourly_averages)
                }
        
        return {
            'analysis_period_hours': hours_back,
            'trends': trends,
            'analyzed_at': datetime.utcnow().isoformat()
        }
    
    def clear_metrics(self, metric_name: Optional[str] = None):
        """
        Clear metric history.
        
        Args:
            metric_name: Optional specific metric to clear (clears all if None)
        """
        if metric_name:
            if metric_name in self.metrics:
                self.metrics[metric_name].clear()
                self.logger.info(f"Cleared metrics for {metric_name}")
        else:
            self.metrics.clear()
            self.operation_times.clear()
            self.logger.info("Cleared all performance metrics")