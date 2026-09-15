//! Konteks Eksekusi & Penerbitan Event Aurion VM.
//! Mematuhi Invariant AUR-VM-006 & AUR-VM-010.

use crate::core::{Address, Hash256, Quantum};

pub const MAX_CALL_DEPTH: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    pub contract: Address,
    pub topics: Vec<Hash256>,
    pub data: Vec<u8>,
}

impl serde::Serialize for Event {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("Event", 3)?;
        state.serialize_field("contract", &self.contract.to_hex())?;
        let topics_hex: Vec<String> = self.topics.iter().map(|t| t.to_hex()).collect();
        state.serialize_field("topics", &topics_hex)?;
        state.serialize_field("data", &hex::encode(&self.data))?;
        state.end()
    }
}

#[derive(Debug, Clone)]
pub struct ExecutionContext {
    pub caller: Address,
    pub contract_address: Address,
    pub origin: Address,
    pub value: Quantum,
    pub gas_limit: u64,
    pub call_depth: usize,
    pub block_height: u64,
    pub timestamp: u64,
    pub events: Vec<Event>,
}

impl ExecutionContext {
    pub fn new(
        caller: Address,
        contract_address: Address,
        origin: Address,
        value: Quantum,
        gas_limit: u64,
        block_height: u64,
        timestamp: u64,
    ) -> Self {
        Self {
            caller,
            contract_address,
            origin,
            value,
            gas_limit,
            call_depth: 1,
            block_height,
            timestamp,
            events: Vec::new(),
        }
    }

    pub fn emit_event(&mut self, topics: Vec<Hash256>, data: Vec<u8>) {
        self.events.push(Event {
            contract: self.contract_address,
            topics,
            data,
        });
    }
}
