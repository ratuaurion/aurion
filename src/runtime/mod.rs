//! Modul Runtime Terpadu Aurion.

pub mod config;
pub mod node;
pub mod supervisor;

pub use config::{NodeConfig, NodeRole};
pub use node::AurionNode;
pub use supervisor::{HealthCheckError, RuntimeSupervisor, MAX_STATE_LATENCY_MS};

