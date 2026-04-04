// Metrics collector - aggregates metrics from various sources
//
// This module:
// - Collects process metrics from process-manager state
// - Tracks operation timings (start, stop, restart, health checks)
// - Aggregates system-level statistics
// - Calculates performance averages

use super::types::*;
use crate::config::load_config;
use crate::process_manager_support::state_store;
use crate::Result;
use chrono::Utc;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// Metrics collector with in-memory storage
pub struct MetricsCollector {
    start_time: Instant,
    loop_iterations: Arc<Mutex<u64>>,
    operation_timings: Arc<Mutex<Vec<OperationTiming>>>,
    exit_code_counts: Arc<Mutex<HashMap<i32, u64>>>,
}

impl MetricsCollector {
    /// Create a new metrics collector
    pub fn new() -> Self {
        tracing::info!("Initializing metrics collector");
        Self {
            start_time: Instant::now(),
            loop_iterations: Arc::new(Mutex::new(0)),
            operation_timings: Arc::new(Mutex::new(Vec::new())),
            exit_code_counts: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Record an operation timing
    pub fn record_operation(&self, timing: OperationTiming) {
        tracing::debug!(
            operation = %timing.operation,
            app = %timing.app,
            duration_ms = timing.duration_ms,
            success = timing.success,
            "Recording operation timing"
        );

        let mut timings = self.operation_timings.lock().unwrap();
        timings.push(timing);

        // Keep last 1000 operations
        if timings.len() > 1000 {
            timings.drain(0..500);
        }
    }

    /// Increment the monitor loop iteration counter
    pub fn increment_loop_iteration(&self) {
        let mut count = self.loop_iterations.lock().unwrap();
        *count += 1;
    }

    /// Record an exit code
    pub fn record_exit_code(&self, code: i32) {
        tracing::debug!(exit_code = code, "Recording exit code");
        let mut codes = self.exit_code_counts.lock().unwrap();
        *codes.entry(code).or_insert(0) += 1;
    }

    /// Collect all metrics
    pub async fn collect_all(&self) -> Result<AllMetrics> {
        tracing::debug!("Collecting all metrics");

        let store = state_store()?;
        let config = load_config()?;

        Ok(AllMetrics {
            processes: self.collect_process_metrics(&store, &config).await?,
            system: self.collect_system_metrics(&config).await?,
            performance: self.collect_performance_metrics().await?,
            timestamp: Utc::now(),
        })
    }

    /// Collect process metrics from tracker
    async fn collect_process_metrics(
        &self,
        store: &process_manager::StateStore,
        _config: &crate::config::Config,
    ) -> Result<ProcessMetrics> {
        let processes = store.list()?;

        let running_processes: Vec<_> = processes
            .iter()
            .filter(|p| store.is_running(p))
            .collect();

        let total_processes = processes.len();
        let running_count = running_processes.len();
        let stopped_count = total_processes - running_count;

        // Calculate total restarts
        let total_restarts: u64 = processes.iter().map(|p| p.runtime.restart_count as u64).sum();

        // Calculate restarts in last hour
        let one_hour_ago = Utc::now() - chrono::Duration::hours(1);
        let restarts_last_hour: u64 = processes
            .iter()
            .flat_map(|p| &p.runtime.history)
            .filter(|entry| entry.timestamp > one_hour_ago)
            .count() as u64;

        // Calculate health check success rate
        let total_health_checks: u32 = processes.iter().map(|p| p.runtime.health_failures).sum();
        let health_check_success_rate = if total_health_checks > 0 {
            // Estimate based on failures (rough approximation)
            let estimated_total = total_health_checks * 10; // Assume 10x checks vs failures
            1.0 - (total_health_checks as f64 / estimated_total as f64)
        } else {
            1.0
        };

        // Get exit code distribution
        let exit_code_distribution = self.exit_code_counts.lock().unwrap().clone();

        // Build app metrics
        let mut app_metrics = Vec::new();
        for process in &processes {
            let is_running = store.is_running(process);
            let uptime_seconds = if is_running {
                Some((Utc::now() - process.start_time).num_seconds() as u64)
            } else {
                None
            };

            let health_status = if process.runtime.health_failures > 0 {
                "unhealthy".to_string()
            } else if is_running {
                "healthy".to_string()
            } else {
                "unknown".to_string()
            };

            app_metrics.push(AppMetrics {
                project: process
                    .metadata
                    .get("project")
                    .cloned()
                    .unwrap_or_else(|| "unknown".to_string()),
                name: process
                    .metadata
                    .get("app_config_name")
                    .cloned()
                    .unwrap_or_else(|| process.id.clone()),
                status: if is_running { "running" } else { "stopped" }.to_string(),
                uptime_seconds,
                restart_count: process.runtime.restart_count,
                last_exit_code: process.runtime.last_exit_code,
                health_status,
                health_check_failures: process.runtime.health_failures,
            });
        }

        Ok(ProcessMetrics {
            total_processes,
            running_processes: running_count,
            stopped_processes: stopped_count,
            total_restarts,
            restarts_last_hour,
            health_check_success_rate,
            exit_code_distribution,
            apps: app_metrics,
        })
    }

    /// Collect system metrics
    async fn collect_system_metrics(
        &self,
        config: &crate::config::Config,
    ) -> Result<SystemMetrics> {
        let monitor_uptime_seconds = self.start_time.elapsed().as_secs();
        let loop_iterations = *self.loop_iterations.lock().unwrap();

        // Count total apps configured
        let total_apps_configured: usize = config.projects.values().map(|p| p.apps.len()).sum();

        Ok(SystemMetrics {
            monitor_uptime_seconds,
            monitor_loop_iterations: loop_iterations,
            devcli_version: env!("CARGO_PKG_VERSION").to_string(),
            total_apps_configured,
            config_last_modified: None, // TODO: Get actual config file mtime
        })
    }

    /// Collect performance metrics
    async fn collect_performance_metrics(&self) -> Result<PerformanceMetrics> {
        let timings = self.operation_timings.lock().unwrap();

        // Calculate averages by operation type
        let mut start_times = Vec::new();
        let mut health_check_times = Vec::new();
        let mut restart_times = Vec::new();

        for timing in timings.iter() {
            match timing.operation.as_str() {
                "start" => start_times.push(timing.duration_ms),
                "health_check" => health_check_times.push(timing.duration_ms),
                "restart" => restart_times.push(timing.duration_ms),
                _ => {}
            }
        }

        let avg_startup_time_ms = if start_times.is_empty() {
            0.0
        } else {
            start_times.iter().sum::<u64>() as f64 / start_times.len() as f64
        };

        let avg_health_check_duration_ms = if health_check_times.is_empty() {
            0.0
        } else {
            health_check_times.iter().sum::<u64>() as f64 / health_check_times.len() as f64
        };

        let avg_restart_duration_ms = if restart_times.is_empty() {
            0.0
        } else {
            restart_times.iter().sum::<u64>() as f64 / restart_times.len() as f64
        };

        // Get recent operations (last 100)
        let recent_operations = if timings.len() > 100 {
            timings[timings.len() - 100..].to_vec()
        } else {
            timings.clone()
        };

        Ok(PerformanceMetrics {
            avg_startup_time_ms,
            avg_health_check_duration_ms,
            avg_restart_duration_ms,
            recent_operations,
        })
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_collector_creation() {
        let collector = MetricsCollector::new();
        let iterations = *collector.loop_iterations.lock().unwrap();
        assert_eq!(iterations, 0);
    }

    #[test]
    fn test_increment_loop_iteration() {
        let collector = MetricsCollector::new();
        collector.increment_loop_iteration();
        collector.increment_loop_iteration();
        let iterations = *collector.loop_iterations.lock().unwrap();
        assert_eq!(iterations, 2);
    }

    #[test]
    fn test_record_exit_code() {
        let collector = MetricsCollector::new();
        collector.record_exit_code(0);
        collector.record_exit_code(1);
        collector.record_exit_code(0);

        let codes = collector.exit_code_counts.lock().unwrap();
        assert_eq!(codes.get(&0), Some(&2));
        assert_eq!(codes.get(&1), Some(&1));
    }

    #[test]
    fn test_record_operation() {
        let collector = MetricsCollector::new();
        let timing = OperationTiming {
            operation: "start".to_string(),
            app: "test-app".to_string(),
            duration_ms: 500,
            timestamp: Utc::now(),
            success: true,
        };

        collector.record_operation(timing);

        let timings = collector.operation_timings.lock().unwrap();
        assert_eq!(timings.len(), 1);
        assert_eq!(timings[0].operation, "start");
    }

    #[test]
    fn test_operation_limit() {
        let collector = MetricsCollector::new();

        // Add 1100 operations
        for i in 0..1100 {
            let timing = OperationTiming {
                operation: "start".to_string(),
                app: format!("app-{}", i),
                duration_ms: 500,
                timestamp: Utc::now(),
                success: true,
            };
            collector.record_operation(timing);
        }

        let timings = collector.operation_timings.lock().unwrap();
        // Should have pruned to 600 (kept last 600 after removing first 500)
        assert!(timings.len() <= 600);
    }
}
