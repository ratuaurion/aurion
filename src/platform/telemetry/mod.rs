#![forbid(unsafe_code)]

//! Modul Telemetri & Observabilitas Aurion.
//! Menyediakan registri metrik Prometheus OpenMetrics dan pemeriksaan kesehatan liveness/readiness.

pub mod health;
pub mod metrics;

pub use health::{ComponentHealth, DeepHealthReport, HealthReporter, ShallowHealthReport};
pub use metrics::{MetricsRegistry, MetricsSnapshot};
