#![forbid(unsafe_code)]

//! Modul Tata Kelola On-Chain & Pensinyalan Peningkatan Protokol (Governance & Fork Signaling).
//! Memungkinkan aktivasi softfork dan hardfork secara terdesentralisasi, deterministik,
//! dan transparan melalui bit-signaling pada BlockHeader (mematuhi prinsip BIP-9).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Status siklus hidup proposal peningkatan protokol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProposalStatus {
    /// Proposal baru terdaftar dan menunggu jendela evaluasi.
    Draft,
    /// Proposal sedang berada dalam jendela pensinyalan validator.
    ActiveSignaling,
    /// Proposal mencapai ambang batas persetujuan (>= 80%) dan terkunci untuk aktivasi.
    LockedIn,
    /// Proposal resmi diaktifkan dan menjadi aturan konsensus kanonikal aktif.
    Activated,
    /// Proposal gagal mencapai ambang batas persetujuan dalam jendela evaluasi.
    Rejected,
}

/// Definisi proposal peningkatan protokol (Softfork / Hardfork).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpgradeProposal {
    pub proposal_id: u32,
    pub name: String,
    pub target_version: u32,
    pub signal_bit: u8, // Bit 1..28 pada BlockHeader.version
    pub start_height: u64,
    pub evaluation_window_blocks: u64,
    pub activation_height: u64,
    pub threshold_bps: u32, // Default: 8000 bps = 80.00%
}

impl UpgradeProposal {
    pub fn new(
        proposal_id: u32,
        name: impl Into<String>,
        target_version: u32,
        signal_bit: u8,
        start_height: u64,
        evaluation_window_blocks: u64,
        activation_height: u64,
    ) -> Result<Self, &'static str> {
        if signal_bit == 0 || signal_bit > 28 {
            return Err("Signal bit must be between 1 and 28");
        }
        if evaluation_window_blocks == 0 {
            return Err("Evaluation window cannot be zero");
        }
        if activation_height < start_height.saturating_add(evaluation_window_blocks) {
            return Err("Activation height must be at or after the evaluation window end");
        }

        Ok(Self {
            proposal_id,
            name: name.into(),
            target_version,
            signal_bit,
            start_height,
            evaluation_window_blocks,
            activation_height,
            threshold_bps: 8_000, // 80.00% ambang batas konsensus
        })
    }
}

/// Ringkasan status proposal untuk inspeksi CLI dan RPC.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProposalSummary {
    pub proposal_id: u32,
    pub name: String,
    pub target_version: u32,
    pub signal_bit: u8,
    pub status: ProposalStatus,
    pub signaling_blocks: u64,
    pub total_window_blocks: u64,
    pub support_bps: u32,
    pub activation_height: u64,
}

/// Mesin tata kelola on-chain untuk mengevaluasi pensinyalan validator.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernanceEngine {
    pub active_protocol_version: u32,
    pub proposals: HashMap<u32, UpgradeProposal>,
    pub statuses: HashMap<u32, ProposalStatus>,
    pub signal_tallies: HashMap<u32, u64>,
}

impl GovernanceEngine {
    pub fn new(initial_version: u32) -> Self {
        Self {
            active_protocol_version: initial_version,
            proposals: HashMap::new(),
            statuses: HashMap::new(),
            signal_tallies: HashMap::new(),
        }
    }

    /// Daftarkan proposal peningkatan baru.
    pub fn register_proposal(&mut self, proposal: UpgradeProposal) -> Result<(), &'static str> {
        let id = proposal.proposal_id;
        if self.proposals.contains_key(&id) {
            return Err("Proposal ID already registered");
        }
        // Pastikan signal_bit tidak bertabrakan dengan proposal lain yang aktif
        for existing in self.proposals.values() {
            if existing.signal_bit == proposal.signal_bit {
                let status = self.statuses.get(&existing.proposal_id).unwrap_or(&ProposalStatus::Draft);
                if *status == ProposalStatus::ActiveSignaling || *status == ProposalStatus::LockedIn {
                    return Err("Signal bit currently in use by an active proposal");
                }
            }
        }

        self.statuses.insert(id, ProposalStatus::Draft);
        self.signal_tallies.insert(id, 0);
        self.proposals.insert(id, proposal);
        Ok(())
    }

    /// Rekam blok baru dan evaluasi bit pensinyalan pada BlockHeader.
    pub fn record_block(&mut self, height: u64, version_signal: u32) {
        let ids: Vec<u32> = self.proposals.keys().copied().collect();

        for id in ids {
            let proposal = self.proposals.get(&id).cloned().unwrap();
            let mut status = *self.statuses.get(&id).unwrap_or(&ProposalStatus::Draft);

            let window_end = proposal.start_height.saturating_add(proposal.evaluation_window_blocks);

            // Transisi dari Draft ke ActiveSignaling saat mencapai start_height
            if status == ProposalStatus::Draft && height >= proposal.start_height && height < window_end {
                status = ProposalStatus::ActiveSignaling;
                self.statuses.insert(id, status);
            }

            // Hitung suara pensinyalan selama jendela evaluasi aktif
            if status == ProposalStatus::ActiveSignaling && height >= proposal.start_height && height < window_end {
                let mask = 1u32 << proposal.signal_bit;
                if (version_signal & mask) != 0 {
                    let count = self.signal_tallies.entry(id).or_insert(0);
                    *count = count.saturating_add(1);
                }

                // Pada blok terakhir jendela evaluasi, tentukan kelulusan
                if height == window_end.saturating_sub(1) {
                    let count = *self.signal_tallies.get(&id).unwrap_or(&0);
                    // Hitung persentase dukungan dalam bps (basis points): count * 10000 / window
                    let support_bps = if proposal.evaluation_window_blocks > 0 {
                        ((count as u128 * 10_000) / (proposal.evaluation_window_blocks as u128)) as u32
                    } else {
                        0
                    };

                    if support_bps >= proposal.threshold_bps {
                        status = ProposalStatus::LockedIn;
                    } else {
                        status = ProposalStatus::Rejected;
                    }
                    self.statuses.insert(id, status);
                }
            }

            // Transisi dari LockedIn ke Activated saat mencapai activation_height
            if status == ProposalStatus::LockedIn && height >= proposal.activation_height {
                status = ProposalStatus::Activated;
                self.statuses.insert(id, status);
                self.active_protocol_version = proposal.target_version;
            }
        }
    }

    /// Dapatkan status proposal tertentu.
    pub fn get_status(&self, proposal_id: u32) -> Option<ProposalStatus> {
        self.statuses.get(&proposal_id).copied()
    }

    /// Ringkasan seluruh proposal terdaftar.
    pub fn list_proposals(&self) -> Vec<ProposalSummary> {
        let mut list = Vec::new();
        for (id, p) in &self.proposals {
            let status = *self.statuses.get(id).unwrap_or(&ProposalStatus::Draft);
            let signaling_blocks = *self.signal_tallies.get(id).unwrap_or(&0);
            let support_bps = if p.evaluation_window_blocks > 0 {
                ((signaling_blocks as u128 * 10_000) / (p.evaluation_window_blocks as u128)) as u32
            } else {
                0
            };

            list.push(ProposalSummary {
                proposal_id: *id,
                name: p.name.clone(),
                target_version: p.target_version,
                signal_bit: p.signal_bit,
                status,
                signaling_blocks,
                total_window_blocks: p.evaluation_window_blocks,
                support_bps,
                activation_height: p.activation_height,
            });
        }
        list.sort_by_key(|p| p.proposal_id);
        list
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_governance_lifecycle_activation() {
        let mut gov = GovernanceEngine::new(1);
        let proposal = UpgradeProposal::new(101, "Aurion Fast Finality V2", 2, 1, 10, 20, 35).unwrap();
        gov.register_proposal(proposal).unwrap();

        assert_eq!(gov.get_status(101), Some(ProposalStatus::Draft));

        // Simulasikan blok 0..9 (sebelum window, status Draft)
        for h in 0..10 {
            gov.record_block(h, 0);
            assert_eq!(gov.get_status(101), Some(ProposalStatus::Draft));
        }

        // Simulasikan window 10..29 (20 blok). Sinyalkan bit 1 pada 17 dari 20 blok (85% > 80%)
        let signal_mask = 1u32 << 1;
        for h in 10..30 {
            let signal = if h < 27 { signal_mask } else { 0 };
            gov.record_block(h, signal);
            if h < 29 {
                assert_eq!(gov.get_status(101), Some(ProposalStatus::ActiveSignaling));
            }
        }

        assert_eq!(gov.get_status(101), Some(ProposalStatus::LockedIn));
        assert_eq!(gov.active_protocol_version, 1);

        // Lanjutkan blok hingga activation_height 35
        for h in 30..=35 {
            gov.record_block(h, 0);
        }

        assert_eq!(gov.get_status(101), Some(ProposalStatus::Activated));
        assert_eq!(gov.active_protocol_version, 2);
    }

    #[test]
    fn test_governance_rejection_on_insufficient_signals() {
        let mut gov = GovernanceEngine::new(1);
        let proposal = UpgradeProposal::new(102, "Rejected Proposal", 2, 2, 10, 20, 35).unwrap();
        gov.register_proposal(proposal).unwrap();

        // Hanya sinyalkan 10 dari 20 blok (50% < 80%)
        let signal_mask = 1u32 << 2;
        for h in 0..30 {
            let signal = if (10..20).contains(&h) { signal_mask } else { 0 };
            gov.record_block(h, signal);
        }

        assert_eq!(gov.get_status(102), Some(ProposalStatus::Rejected));
        assert_eq!(gov.active_protocol_version, 1);
    }
}
