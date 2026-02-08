// Metrics data types
//
// Defines the structure of metrics data:
// - ProcessMetrics: Running processes, restarts, health checks
// - SystemMetrics: Monitor uptime, app count
// - PerformanceMetrics: Command execution times, health check latency
// - AppMetrics: Per-app status and statistics

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Complete metrics snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AllMetrics {
    pub processes: ProcessMetrics,
    pub system: SystemMetrics,
    pub performance: PerformanceMetrics,
    pub timestamp: DateTime<Utc>,
}

/// Process-level metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessMetrics {
    pub total_processes: usize,
    pub running_processes: usize,
    pub stopped_processes: usize,
    pub total_restarts: u64,
    pub restarts_last_hour: u64,
    pub health_check_success_rate: f64, // 0.0-1.0
    pub exit_code_distribution: HashMap<i32, u64>, // exit_code -> count
    pub apps: Vec<AppMetrics>,
}

/// Per-app metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppMetrics {
    pub project: String,
    pub name: String,
    pub status: String, // "running", "stopped"
    pub uptime_seconds: Option<u64>,
    pub restart_count: u32,
    pub last_exit_code: Option<i32>,
    pub health_status: String, // "healthy", "unhealthy", "unknown"
    pub health_check_failures: u32,
}

/// System-level metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMetrics {
    pub monitor_uptime_seconds: u64,
    pub monitor_loop_iterations: u64,
    pub devcli_version: String,
    pub total_apps_configured: usize,
    pub config_last_modified: Option<DateTime<Utc>>,
}

/// Performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    pub avg_startup_time_ms: f64,
    pub avg_health_check_duration_ms: f64,
    pub avg_restart_duration_ms: f64,
    pub recent_operations: Vec<OperationTiming>,
}

/// Individual operation timing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationTiming {
    pub operation: String, // "start", "stop", "restart", "health_check"
    pub app: String,
    pub duration_ms: u64,
    pub timestamp: DateTime<Utc>,
    pub success: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_metrics_serialization() {
        let metrics = AllMetrics {
            processes: ProcessMetrics {
                total_processes: 5,
                running_processes: 3,
                stopped_processes: 2,
                total_restarts: 10,
                restarts_last_hour: 2,
                health_check_success_rate: 0.95,
                exit_code_distribution: HashMap::new(),
                apps: vec![],
            },
            system: SystemMetrics {
                monitor_uptime_seconds: 3600,
                monitor_loop_iterations: 1200,
                devcli_version: "0.1.0".to_string(),
                total_apps_configured: 10,
                config_last_modified: None,
            },
            performance: PerformanceMetrics {
                avg_startup_time_ms: 500.0,
                avg_health_check_duration_ms: 50.0,
                avg_restart_duration_ms: 1000.0,
                recent_operations: vec![],
            },
            timestamp: Utc::now(),
        };

        // Test JSON serialization
        let json = serde_json::to_string(&metrics).unwrap();
        assert!(json.contains("total_processes"));
        assert!(json.contains("monitor_uptime_seconds"));

        // Test deserialization
        let _deserialized: AllMetrics = serde_json::from_str(&json).unwrap();
    }

    #[test]
    fn test_operation_timing_creation() {
        let timing = OperationTiming {
            operation: "start".to_string(),
            app: "api-server".to_string(),
            duration_ms: 500,
            timestamp: Utc::now(),
            success: true,
        };

        assert_eq!(timing.operation, "start");
        assert_eq!(timing.app, "api-server");
        assert_eq!(timing.duration_ms, 500);
        assert!(timing.success);
    }

    #[test]
    fn test_app_metrics_creation() {
        let app = AppMetrics {
            project: "myproject".to_string(),
            name: "api-server".to_string(),
            status: "running".to_string(),
            uptime_seconds: Some(3600),
            restart_count: 2,
            last_exit_code: None,
            health_status: "healthy".to_string(),
            health_check_failures: 0,
        };

        assert_eq!(app.status, "running");
        assert_eq!(app.uptime_seconds, Some(3600));
        assert_eq!(app.health_status, "healthy");
    }
}
