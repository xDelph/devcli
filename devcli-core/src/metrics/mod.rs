// Metrics collection and reporting module
//
// This module provides:
// - Metrics data types (process, system, performance)
// - Metrics collector for aggregating data
// - HTTP JSON API server for exposing metrics
// - Integration with process tracker and monitor

pub mod collector;
pub mod server;
pub mod types;

pub use collector::MetricsCollector;
pub use server::start_metrics_server;
pub use types::{
    AllMetrics, AppMetrics, OperationTiming, PerformanceMetrics, ProcessMetrics, SystemMetrics,
};
