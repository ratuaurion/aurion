//! Deterministic Multi-Party Genesis Ceremony Engine (PRD-015).
//!
//! Modul ini mengelola protokol upacara pembentukan blok Genesis ($H=0$) dan State Awal ($\sigma_0$)
//! secara deterministik dan teratestasi kriptografis multi-pihak (Master Treasury dan
//! 4 Genesis Validators $\mathcal{V}_0$).
//!
//! MODEL SINGLE TREASURY: seluruh pasokan genesis (100%) dipegang eksklusif oleh satu
//! akun Master Treasury. Skema alokasi pecahan (Creator dan Developer terpisah) telah
//! dihapus total; tidak ada peserta non-Treasury yang memegang saldo awal.

use crate::consensus::certificate::ValidatorEntry;
use crate::core::{Address, Hash256, Signature};
use crate::crypto::{blake3_derive_key, ed25519_verify_strict, Keypair};
use crate::genesis::builder::{
    build_genesis, GenesisInitialization, GENESIS_CHAIN_ID, GENESIS_TIMESTAMP,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Tag pemisahan domain (Domain Separation Tag) kanonikal untuk penandatanganan upacara Genesis.
pub const DST_GENESIS_CEREMONY: &str = "AURION-GENESIS-CEREMONY-V1";

/// Total bobot voting validator awal: 1.000.000 (AUR-GENESIS-007).
pub const CEREMONY_TOTAL_VOTING_POWER: u64 = 1_000_000;

/// Initial supply Blok 0 dalam AUR pada model **Single Treasury**.
///
/// 100% dari seluruh pasokan genesis dialokasikan eksklusif ke satu akun
/// Master Treasury. Skema alokasi pecahan (mis. 35% genesis / 30% creator /
/// 5% developer) telah dihapus total dan tidak lagi menjadi bagian protokol.
pub const GENESIS_INITIAL_SUPPLY_AUR: u64 = 66_000_000;

/// Alokasi Master Treasury di Blok 0 (100% dari initial supply, model Single Treasury).
pub const MASTER_TREASURY_ALLOCATION_AUR: u64 = GENESIS_INITIAL_SUPPLY_AUR;

/// Kuorum voting BFT awal: >2/3 = 666.667 (AUR-GENESIS-007).
pub const CEREMONY_QUORUM_THRESHOLD: u64 = 666_667;

///
/// Identitas kanonik blok Genesis model Single Treasury.
///
/// STATE ROOT `ec1446f1...` dihitung dari HANYA SATU akun (Master Treasury) pada
/// `state_root` blok 0. Hash ini menggantikan identitas lama yang menyertakan
/// rekening Developer bersaldo 0 pada state awal.
pub const CANONICAL_GENESIS_HASH: &str =
    "42e9a752ddfdd0308fc993077121276beb0386b1433df20156ec0705611daf3a";
pub const CANONICAL_GENESIS_STATE_ROOT: &str =
    "ec1446f10466dc7551edb1ed51028723f22e0b529d524e87a1dac0f6927eb58f";
pub const CANONICAL_CEREMONY_HASH: &str =
    "a747bb72ce0f2ed7d41b72a90ff98eec644c60f6b2e4071b948d48acc5467880";

/// Peran entitas dalam upacara pembentukan Genesis.
///
/// Pada model Single Treasury hanya ada satu peran portasional: `MasterTreasury`,
/// yaitu pemegang eksklusif 100% pasokan genesis, serta 4 `Validator` konsensus.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CeremonyRole {
    MasterTreasury,
    Validator(u32),
}

impl std::fmt::Display for CeremonyRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CeremonyRole::MasterTreasury => write!(f, "Master Treasury Vault"),
            CeremonyRole::Validator(idx) => write!(f, "Genesis Validator {idx}"),
        }
    }
}

/// Kesalahan dalam eksekusi atau verifikasi upacara Genesis.
#[derive(Debug, Error, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub enum CeremonyError {
    #[error("Signature verification failed for role {role}: {reason}")]
    InvalidSignature { role: String, reason: String },

    #[error("Validator quorum not achieved: attested {attested} < required {required}")]
    QuorumNotAchieved { attested: u64, required: u64 },

    #[error("Monetary policy invariant violation: {reason}")]
    MonetaryInvariantViolation { reason: String },

    #[error("Genesis block hash mismatch: expected {expected}, got {actual}")]
    GenesisHashMismatch { expected: String, actual: String },

    #[error("State root mismatch: expected {expected}, got {actual}")]
    StateRootMismatch { expected: String, actual: String },

    #[error("Ceremony transcript hash corrupted: expected {expected}, got {actual}")]
    CeremonyHashCorrupted { expected: String, actual: String },

    #[error("Participant missing or invalid: {0}")]
    ParticipantMissing(String),

    #[error("Serialization error: {0}")]
    Serialization(String),
}

/// Peserta resmi dalam upacara Genesis.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CeremonyParticipant {
    pub role: CeremonyRole,
    pub name: String,
    pub public_key_hex: String,
    pub address_hex: String,
    pub voting_weight: u64,
}

/// Pengesahan kriptografis bertanda tangan oleh seorang peserta upacara.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CeremonyAttestation {
    pub role: CeremonyRole,
    pub participant_name: String,
    pub public_key_hex: String,
    pub signature_hex: String,
    pub signed_at: u64,
}

/// Laporan hasil verifikasi integritas upacara Genesis.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CeremonyVerificationReport {
    pub overall_status: String,
    pub ceremony_hash: String,
    pub genesis_block_hash: String,
    pub state_root: String,
    pub chain_id: u32,
    pub genesis_timestamp: u64,
    pub total_attestations: usize,
    pub attested_validator_power: u64,
    pub quorum_threshold: u64,
    pub quorum_status: String,
    pub monetary_audit_status: String,
    pub verified_at: u64,
}

/// Transkrip lengkap dan mandiri upacara pembentukan Genesis.
///
/// Skema alokasi bersifat tunggal (Single Treasury): tidak ada field alokasi
/// Creator/Developer terpisah, dan tidak ada field alamat non-Treasury.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CeremonyTranscript {
    pub ceremony_version: u32,
    pub protocol_version: u32,
    pub chain_id: u32,
    pub timestamp: u64,
    pub genesis_block_hash: String,
    pub state_root: String,
    pub hard_cap_aur: u64,
    pub initial_supply_aur: u64,
    pub master_treasury_allocation_aur: u64,
    pub master_treasury_address_hex: String,
    pub participants: Vec<CeremonyParticipant>,
    pub attestations: Vec<CeremonyAttestation>,
    pub total_validator_power: u64,
    pub attested_validator_power: u64,
    pub quorum_threshold: u64,
    pub quorum_achieved: bool,
    pub ceremony_hash: String,
}

#[derive(Debug, Deserialize)]
struct EmbeddedGenesisArtifact {
    genesis_block: EmbeddedGenesisBlock,
    metadata: EmbeddedGenesisMetadata,
}

#[derive(Debug, Deserialize)]
struct EmbeddedGenesisBlock {
    header: EmbeddedGenesisHeader,
}

#[derive(Debug, Deserialize)]
struct EmbeddedGenesisHeader {
    chain_id: u32,
    height: u64,
    timestamp: u64,
    state_root: String,
    block_hash: String,
}

#[derive(Debug, Deserialize)]
struct EmbeddedGenesisMetadata {
    network: String,
    protocol_version: u32,
    ceremony_transcript_hash: String,
    hard_cap_aur: u64,
    initial_supply_aur: u64,
    master_treasury_allocation_aur: u64,
    total_validator_power: u64,
    quorum_threshold: u64,
    genesis_validators_count: usize,
}

/// Hitung pesan 32-byte Blake3 yang wajib ditandatangani oleh seluruh peserta upacara.
pub fn compute_ceremony_signing_message(
    chain_id: u32,
    timestamp: u64,
    genesis_block_hash: &Hash256,
    state_root: &Hash256,
) -> [u8; 32] {
    let mut payload = Vec::with_capacity(4 + 8 + 32 + 32);
    payload.extend_from_slice(&chain_id.to_be_bytes());
    payload.extend_from_slice(&timestamp.to_be_bytes());
    payload.extend_from_slice(genesis_block_hash.as_bytes());
    payload.extend_from_slice(state_root.as_bytes());
    *blake3_derive_key(DST_GENESIS_CEREMONY, &payload).as_bytes()
}

/// Kunci-kunci upacara kanonikal untuk pembentukan deterministik rilis resmi.
pub struct CanonicalCeremonyKeypairs {
    pub master_treasury: Keypair,
    pub validators: Vec<Keypair>,
}

impl CanonicalCeremonyKeypairs {
    /// Pembangkitan 5 keypair kanonikal dari seed terdefinisi secara deterministik
    /// (1 Master Treasury + 4 Genesis Validator).
    pub fn new_deterministic() -> Self {
        let master_treasury = Keypair::from_seed(&[0x01; 32]);
        let validators = vec![
            Keypair::from_seed(&[0x11; 32]), // Validator 1 (Alpha)
            Keypair::from_seed(&[0x12; 32]), // Validator 2 (Beta)
            Keypair::from_seed(&[0x13; 32]), // Validator 3 (Gamma)
            Keypair::from_seed(&[0x14; 32]), // Validator 4 (Delta)
        ];
        Self {
            master_treasury,
            validators,
        }
    }
}

impl CeremonyTranscript {
    /// Eksekusi upacara genesis deterministik kanonikal menggunakan himpunan keypair.
    pub fn build_and_seal(keys: &CanonicalCeremonyKeypairs) -> Result<Self, CeremonyError> {
        let treasury_addr = keys.master_treasury.derive_address();

        let mut validator_entries = Vec::with_capacity(keys.validators.len());
        let mut participants = Vec::with_capacity(1 + keys.validators.len());

        // Peserta 1: Master Treasury (pemegang eksklusif 100% pasokan genesis)
        participants.push(CeremonyParticipant {
            role: CeremonyRole::MasterTreasury,
            name: "Master Treasury Sovereign Vault".to_string(),
            public_key_hex: hex::encode(keys.master_treasury.public_key_bytes()),
            address_hex: treasury_addr.to_hex(),
            voting_weight: 0,
        });

        // Peserta 3..6: 4 Validator Genesis masing-masing bobot 250.000
        let weight_per_val = CEREMONY_TOTAL_VOTING_POWER / keys.validators.len() as u64;
        for (i, val_key) in keys.validators.iter().enumerate() {
            let val_idx = (i + 1) as u32;
            let val_addr = val_key.derive_address();
            let pubkey_bytes = val_key.public_key_bytes();

            validator_entries.push(ValidatorEntry {
                validator_id: val_addr,
                consensus_pubkey: pubkey_bytes,
                voting_weight: weight_per_val,
            });

            let name = match val_idx {
                1 => "Genesis Validator 1 (Bootnode Alpha)".to_string(),
                2 => "Genesis Validator 2 (Bootnode Beta)".to_string(),
                3 => "Genesis Validator 3 (Bootnode Gamma)".to_string(),
                4 => "Genesis Validator 4 (Bootnode Delta)".to_string(),
                n => format!("Genesis Validator {n}"),
            };

            participants.push(CeremonyParticipant {
                role: CeremonyRole::Validator(val_idx),
                name,
                public_key_hex: hex::encode(pubkey_bytes),
                address_hex: val_addr.to_hex(),
                voting_weight: weight_per_val,
            });
        }

        // Bangun State Genesis σ0 dan Header Blok Nol (100% ke Master Treasury)
        let genesis: GenesisInitialization = build_genesis(treasury_addr, validator_entries);
        let block_hash = genesis.header.compute_block_hash();
        let state_root = genesis.header.state_root;

        // Hitung pesan tanda tangan kanonikal
        let signing_msg = compute_ceremony_signing_message(
            GENESIS_CHAIN_ID,
            GENESIS_TIMESTAMP,
            &block_hash,
            &state_root,
        );

        // Kumpulkan atestasi dari seluruh 5 pihak
        let mut attestations = Vec::with_capacity(participants.len());
        let mut attested_weight: u64 = 0;

        // Tanda tangan Master Treasury
        let treasury_sig = keys.master_treasury.sign(&signing_msg);
        attestations.push(CeremonyAttestation {
            role: CeremonyRole::MasterTreasury,
            participant_name: "Master Treasury Sovereign Vault".to_string(),
            public_key_hex: hex::encode(keys.master_treasury.public_key_bytes()),
            signature_hex: hex::encode(treasury_sig.as_bytes()),
            signed_at: GENESIS_TIMESTAMP,
        });

        // Tanda tangan Validator 1..4
        for (i, val_key) in keys.validators.iter().enumerate() {
            let val_idx = (i + 1) as u32;
            let val_sig = val_key.sign(&signing_msg);
            let name = match val_idx {
                1 => "Genesis Validator 1 (Bootnode Alpha)".to_string(),
                2 => "Genesis Validator 2 (Bootnode Beta)".to_string(),
                3 => "Genesis Validator 3 (Bootnode Gamma)".to_string(),
                4 => "Genesis Validator 4 (Bootnode Delta)".to_string(),
                n => format!("Genesis Validator {n}"),
            };

            attestations.push(CeremonyAttestation {
                role: CeremonyRole::Validator(val_idx),
                participant_name: name,
                public_key_hex: hex::encode(val_key.public_key_bytes()),
                signature_hex: hex::encode(val_sig.as_bytes()),
                signed_at: GENESIS_TIMESTAMP,
            });

            attested_weight = attested_weight
                .checked_add(weight_per_val)
                .expect("Voting weight sum overflow");
        }

        let quorum_achieved = attested_weight >= CEREMONY_QUORUM_THRESHOLD;
        if !quorum_achieved {
            return Err(CeremonyError::QuorumNotAchieved {
                attested: attested_weight,
                required: CEREMONY_QUORUM_THRESHOLD,
            });
        }

        // Hitung digest transkrip keseluruhan
        let mut transcript = Self {
            ceremony_version: 1,
            protocol_version: 1,
            chain_id: GENESIS_CHAIN_ID,
            timestamp: GENESIS_TIMESTAMP,
            genesis_block_hash: block_hash.to_hex(),
            state_root: state_root.to_hex(),
            hard_cap_aur: GENESIS_INITIAL_SUPPLY_AUR,
            initial_supply_aur: GENESIS_INITIAL_SUPPLY_AUR,
            master_treasury_allocation_aur: MASTER_TREASURY_ALLOCATION_AUR,
            master_treasury_address_hex: treasury_addr.to_hex(),
            participants,
            attestations,
            total_validator_power: CEREMONY_TOTAL_VOTING_POWER,
            attested_validator_power: attested_weight,
            quorum_threshold: CEREMONY_QUORUM_THRESHOLD,
            quorum_achieved,
            ceremony_hash: String::new(),
        };

        transcript.ceremony_hash = transcript.compute_transcript_hash();
        Ok(transcript)
    }

    /// Hitung Blake3 hash atas seluruh transkrip upacara kanonikal.
    pub fn compute_transcript_hash(&self) -> String {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"AURION-CEREMONY-TRANSCRIPT-V1");
        hasher.update(&self.ceremony_version.to_be_bytes());
        hasher.update(&self.protocol_version.to_be_bytes());
        hasher.update(&self.chain_id.to_be_bytes());
        hasher.update(&self.timestamp.to_be_bytes());
        hasher.update(self.genesis_block_hash.as_bytes());
        hasher.update(self.state_root.as_bytes());
        hasher.update(&self.initial_supply_aur.to_be_bytes());
        hasher.update(&self.master_treasury_allocation_aur.to_be_bytes());
        hasher.update(self.master_treasury_address_hex.as_bytes());

        for att in &self.attestations {
            hasher.update(att.public_key_hex.as_bytes());
            hasher.update(att.signature_hex.as_bytes());
        }

        let hash: Hash256 = Hash256::from_bytes(*hasher.finalize().as_bytes());
        hash.to_hex()
    }

    /// Verifikasi penuh seluruh tanda tangan, aturan moneter, kuorum validator, dan hash blok.
    pub fn verify(&self) -> Result<CeremonyVerificationReport, CeremonyError> {
        // 1. Verifikasi Invarian Moneter
        if self.hard_cap_aur != GENESIS_INITIAL_SUPPLY_AUR {
            return Err(CeremonyError::MonetaryInvariantViolation {
                reason: format!(
                    "Hard cap must be {GENESIS_INITIAL_SUPPLY_AUR} AUR, got {}",
                    self.hard_cap_aur
                ),
            });
        }
        if self.initial_supply_aur != GENESIS_INITIAL_SUPPLY_AUR {
            return Err(CeremonyError::MonetaryInvariantViolation {
                reason: format!(
                    "Initial supply must be {GENESIS_INITIAL_SUPPLY_AUR} AUR (100% Single Treasury), got {}",
                    self.initial_supply_aur
                ),
            });
        }
        if self.master_treasury_allocation_aur != MASTER_TREASURY_ALLOCATION_AUR {
            return Err(CeremonyError::MonetaryInvariantViolation {
                reason: format!(
                    "Single Treasury violation: Master Treasury must hold exactly {MASTER_TREASURY_ALLOCATION_AUR} AUR (100% of genesis supply), got {}",
                    self.master_treasury_allocation_aur
                ),
            });
        }

        // 2. Rekonstruksi Blok Genesis & Validasi Hash
        let treasury_bytes: [u8; 32] = hex::decode(&self.master_treasury_address_hex)
            .map_err(|e| CeremonyError::Serialization(e.to_string()))?
            .try_into()
            .map_err(|_| {
                CeremonyError::Serialization("Master Treasury addr must be 32 bytes".to_string())
            })?;

        let treasury_addr = Address::from_bytes(treasury_bytes);

        let mut validator_entries = Vec::new();
        for p in &self.participants {
            if let CeremonyRole::Validator(_) = p.role {
                let val_addr_bytes: [u8; 32] = hex::decode(&p.address_hex)
                    .map_err(|e| CeremonyError::Serialization(e.to_string()))?
                    .try_into()
                    .map_err(|_| CeremonyError::Serialization("Val addr 32 bytes".to_string()))?;
                let pubkey_bytes: [u8; 32] = hex::decode(&p.public_key_hex)
                    .map_err(|e| CeremonyError::Serialization(e.to_string()))?
                    .try_into()
                    .map_err(|_| CeremonyError::Serialization("Pubkey 32 bytes".to_string()))?;

                validator_entries.push(ValidatorEntry {
                    validator_id: Address::from_bytes(val_addr_bytes),
                    consensus_pubkey: pubkey_bytes,
                    voting_weight: p.voting_weight,
                });
            }
        }

        let genesis = build_genesis(treasury_addr, validator_entries);
        let expected_hash = genesis.header.compute_block_hash();
        if expected_hash.to_hex() != self.genesis_block_hash {
            return Err(CeremonyError::GenesisHashMismatch {
                expected: expected_hash.to_hex(),
                actual: self.genesis_block_hash.clone(),
            });
        }

        if genesis.header.state_root.to_hex() != self.state_root {
            return Err(CeremonyError::StateRootMismatch {
                expected: genesis.header.state_root.to_hex(),
                actual: self.state_root.clone(),
            });
        }

        // 3. Verifikasi Pesan Penandatanganan
        let block_hash_bytes: [u8; 32] = hex::decode(&self.genesis_block_hash)
            .map_err(|e| CeremonyError::Serialization(e.to_string()))?
            .try_into()
            .map_err(|_| CeremonyError::Serialization("Hash 32 bytes".to_string()))?;
        let state_root_bytes: [u8; 32] = hex::decode(&self.state_root)
            .map_err(|e| CeremonyError::Serialization(e.to_string()))?
            .try_into()
            .map_err(|_| CeremonyError::Serialization("Root 32 bytes".to_string()))?;

        let signing_msg = compute_ceremony_signing_message(
            self.chain_id,
            self.timestamp,
            &Hash256::from_bytes(block_hash_bytes),
            &Hash256::from_bytes(state_root_bytes),
        );

        // 4. Verifikasi Seluruh Tanda Tangan Ed25519 (Strict RFC 8032)
        let mut attested_weight: u64 = 0;
        let mut has_master_treasury = false;

        for att in &self.attestations {
            let pubkey_bytes: [u8; 32] = hex::decode(&att.public_key_hex)
                .map_err(|e| CeremonyError::Serialization(e.to_string()))?
                .try_into()
                .map_err(|_| CeremonyError::Serialization("Pubkey 32 bytes".to_string()))?;
            let sig_bytes: [u8; 64] = hex::decode(&att.signature_hex)
                .map_err(|e| CeremonyError::Serialization(e.to_string()))?
                .try_into()
                .map_err(|_| CeremonyError::Serialization("Sig 64 bytes".to_string()))?;

            let signature = Signature(sig_bytes);

            ed25519_verify_strict(&pubkey_bytes, &signing_msg, &signature).map_err(|e| {
                CeremonyError::InvalidSignature {
                    role: format!("{:?}", att.role),
                    reason: e.to_string(),
                }
            })?;

            match att.role {
                CeremonyRole::MasterTreasury => has_master_treasury = true,
                CeremonyRole::Validator(idx) => {
                    let part = self
                        .participants
                        .iter()
                        .find(|p| p.role == CeremonyRole::Validator(idx))
                        .ok_or_else(|| {
                            CeremonyError::ParticipantMissing(format!("Validator {idx}"))
                        })?;
                    attested_weight = attested_weight
                        .checked_add(part.voting_weight)
                        .expect("Weight overflow");
                }
            }
        }

        if !has_master_treasury {
            return Err(CeremonyError::ParticipantMissing(
                "Master Treasury attestation missing".to_string(),
            ));
        }

        // 5. Verifikasi Kuorum Validator
        if attested_weight < self.quorum_threshold {
            return Err(CeremonyError::QuorumNotAchieved {
                attested: attested_weight,
                required: self.quorum_threshold,
            });
        }

        // 6. Verifikasi Integritas Transkrip Hash
        let expected_ceremony_hash = self.compute_transcript_hash();
        if expected_ceremony_hash != self.ceremony_hash {
            return Err(CeremonyError::CeremonyHashCorrupted {
                expected: expected_ceremony_hash,
                actual: self.ceremony_hash.clone(),
            });
        }

        Ok(CeremonyVerificationReport {
            overall_status: "VERIFIED_CANONICAL".to_string(),
            ceremony_hash: self.ceremony_hash.clone(),
            genesis_block_hash: self.genesis_block_hash.clone(),
            state_root: self.state_root.clone(),
            chain_id: self.chain_id,
            genesis_timestamp: self.timestamp,
            total_attestations: self.attestations.len(),
            attested_validator_power: attested_weight,
            quorum_threshold: self.quorum_threshold,
            quorum_status: format!("PASSED ({attested_weight}/{CEREMONY_TOTAL_VOTING_POWER} >= {CEREMONY_QUORUM_THRESHOLD})"),
            monetary_audit_status: "PASSED (100% Invariant Compliant: 100% Treasury Genesis, Zero-Float)".to_string(),
            verified_at: self.timestamp,
        })
    }

    /// Serialisasi transkrip ke JSON terformat (pretty).
    pub fn to_json_pretty(&self) -> Result<String, CeremonyError> {
        serde_json::to_string_pretty(self).map_err(|e| CeremonyError::Serialization(e.to_string()))
    }

    /// Deserialisasi transkrip dari string JSON.
    pub fn from_json_str(json: &str) -> Result<Self, CeremonyError> {
        serde_json::from_str(json).map_err(|e| CeremonyError::Serialization(e.to_string()))
    }

    /// Merekonstruksi `GenesisInitialization` lengkap dari transkrip yang terverifikasi.
    pub fn build_genesis_initialization(&self) -> Result<GenesisInitialization, CeremonyError> {
        let treasury_bytes: [u8; 32] = hex::decode(&self.master_treasury_address_hex)
            .map_err(|e| CeremonyError::Serialization(e.to_string()))?
            .try_into()
            .map_err(|_| {
                CeremonyError::Serialization("Master Treasury addr must be 32 bytes".to_string())
            })?;

        let treasury_addr = Address::from_bytes(treasury_bytes);

        let mut validator_entries = Vec::new();
        for p in &self.participants {
            if let CeremonyRole::Validator(_) = p.role {
                let val_addr_bytes: [u8; 32] = hex::decode(&p.address_hex)
                    .map_err(|e| CeremonyError::Serialization(e.to_string()))?
                    .try_into()
                    .map_err(|_| CeremonyError::Serialization("Val addr 32 bytes".to_string()))?;
                let pubkey_bytes: [u8; 32] = hex::decode(&p.public_key_hex)
                    .map_err(|e| CeremonyError::Serialization(e.to_string()))?
                    .try_into()
                    .map_err(|_| CeremonyError::Serialization("Pubkey 32 bytes".to_string()))?;

                validator_entries.push(ValidatorEntry {
                    validator_id: Address::from_bytes(val_addr_bytes),
                    consensus_pubkey: pubkey_bytes,
                    voting_weight: p.voting_weight,
                });
            }
        }

        let genesis = build_genesis(treasury_addr, validator_entries);
        Ok(genesis)
    }

    /// JSON transkrip upacara genesis kanonikal yang tersegel dan teratestasi resmi.
    pub const CANONICAL_SEALED_TRANSCRIPT_JSON: &'static str =
        include_str!("../../../GENESIS_CEREMONY.json");

    /// Artefak blok genesis Mainnet yang diikat langsung ke binary.
    pub const EMBEDDED_MAINNET_GENESIS_JSON: &'static str =
        include_str!("../../../MAINNET_GENESIS_BLOCK.json");

    /// Membangun inisialisasi Mainnet kanonikal dari artefak yang diikat ke binary.
    pub fn canonical_mainnet_genesis() -> GenesisInitialization {
        let transcript = CeremonyTranscript::from_json_str(Self::CANONICAL_SEALED_TRANSCRIPT_JSON)
            .expect("Embedded canonical genesis transcript must be valid JSON");
        let genesis = transcript
            .build_genesis_initialization()
            .expect("Deterministic genesis reconstruction must succeed");
        let artifact: EmbeddedGenesisArtifact =
            serde_json::from_str(Self::EMBEDDED_MAINNET_GENESIS_JSON)
                .expect("Embedded mainnet genesis artifact must be valid JSON");

        assert_eq!(artifact.metadata.network, "aurion-mainnet");
        assert_eq!(
            artifact.metadata.protocol_version,
            transcript.protocol_version
        );
        assert_eq!(
            artifact.metadata.ceremony_transcript_hash,
            transcript.ceremony_hash
        );
        assert_eq!(artifact.metadata.hard_cap_aur, transcript.hard_cap_aur);
        assert_eq!(
            artifact.metadata.initial_supply_aur,
            transcript.initial_supply_aur
        );
        assert_eq!(
            artifact.metadata.master_treasury_allocation_aur,
            transcript.master_treasury_allocation_aur
        );
        assert_eq!(
            artifact.metadata.total_validator_power,
            transcript.total_validator_power
        );
        assert_eq!(
            artifact.metadata.quorum_threshold,
            transcript.quorum_threshold
        );
        assert_eq!(
            artifact.metadata.genesis_validators_count,
            genesis.validator_set.validators.len()
        );

        let computed_hash = genesis.header.compute_block_hash().to_hex();
        let computed_state_root = genesis.header.state_root.to_hex();
        assert_eq!(computed_hash, CANONICAL_GENESIS_HASH);
        assert_eq!(computed_state_root, CANONICAL_GENESIS_STATE_ROOT);
        assert_eq!(artifact.genesis_block.header.chain_id, GENESIS_CHAIN_ID);
        assert_eq!(artifact.genesis_block.header.height, 0);
        assert_eq!(artifact.genesis_block.header.timestamp, GENESIS_TIMESTAMP);
        assert_eq!(
            artifact.genesis_block.header.state_root,
            CANONICAL_GENESIS_STATE_ROOT
        );
        assert_eq!(
            artifact.genesis_block.header.block_hash,
            CANONICAL_GENESIS_HASH
        );

        genesis
    }
}
