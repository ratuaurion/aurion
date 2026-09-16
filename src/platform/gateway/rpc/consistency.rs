//! Penyeleksi Konsistensi State Rantai (Strict Consistency State Selector).
//! Mematuhi Dokumen 02 Bagian 3 dan Dokumen 04 (04-FINALITY-CONFIRMATION-RULES.md).

use crate::gateway::rpc::errors::invalid_params;
use crate::gateway::rpc::types::JsonRpcError;

/// Penanda semantik konsistensi saat menanyakan state, saldo, atau blok.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ConsistencySelector {
    /// Hanya mengacu pada blok yang telah memiliki Commit Certificate resmi (>2/3 kuorum).
    /// Default wajib untuk bursa, settlement, dan merchant.
    #[default]
    Finalized,
    /// Mengacu pada blok yang telah melewati fase Pre-vote kuorum.
    Safe,
    /// Mengacu pada blok kandidat lokal tertinggi (spekulatif).
    Latest,
    /// Membaca snapshot state historis pada ketinggian tertentu.
    SpecificHeight(u64),
}

impl ConsistencySelector {
    /// Parser parameter konsistensi dari argumen string JSON-RPC.
    pub fn parse(s: &str) -> Result<Self, JsonRpcError> {
        let trimmed = s.trim();
        match trimmed.to_ascii_lowercase().as_str() {
            "finalized" => Ok(Self::Finalized),
            "safe" => Ok(Self::Safe),
            "latest" => Ok(Self::Latest),
            _ => {
                // Periksa apakah format hex ("0x...") atau desimal ("123")
                let height = if trimmed.starts_with("0x") || trimmed.starts_with("0X") {
                    u64::from_str_radix(&trimmed[2..], 16).map_err(|_| {
                        invalid_params(format!("Invalid hex block height parameter: '{trimmed}'"))
                    })?
                } else {
                    trimmed.parse::<u64>().map_err(|_| {
                        invalid_params(format!(
                            "Invalid block parameter: expected 'finalized', 'safe', 'latest', or u64 height, got '{trimmed}'"
                        ))
                    })?
                };
                Ok(Self::SpecificHeight(height))
            }
        }
    }

    /// Memeriksa apakah operasi ini memerlukan pembuktian finalitas mutlak.
    pub fn requires_finality_proof(&self) -> bool {
        matches!(self, Self::Finalized)
    }
}
