//! Supervisor siklus hidup proses Aurion.

use crate::runtime::config::NodeConfig;

pub struct RuntimeSupervisor {
    pub config: NodeConfig,
    pub is_running: bool,
}

impl RuntimeSupervisor {
    pub fn new(config: NodeConfig) -> Self {
        Self {
            config,
            is_running: false,
        }
    }

    pub fn start(&mut self) {
        self.is_running = true;
    }

    pub fn stop(&mut self) {
        self.is_running = false;
    }

    pub fn is_healthy(&self) -> bool {
        self.is_running
    }
}
