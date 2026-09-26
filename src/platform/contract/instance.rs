//! [`ContractInstance`]: pengikat metadata + Provider + Signer dengan pipeline
//! otomatis `User Intent -> Metadata -> Auto Tx Builder -> Local VM Simulation
//! -> Wallet Clear Signing -> Auto Broadcast`.

use crate::codec::CanonicalEncode;
use crate::core::{Address, Hash256, Quantum, Signature};
use crate::crypto::{blake3_hash, decode_address_bech32m, encode_address_bech32m};
use crate::state::stf::derive_contract_address;
use crate::transaction::types::{Transaction, TxType};

use super::calldata::{encode_call_payload, encode_deploy_payload};
use super::error::ContractError;
use super::intent::{ContractIntent, IntentAction};
use super::metadata::{AbiType, AbiValue, ContractMetadata, MethodAbi};
use super::provider::{DryRunReport, Provider};
use super::signer::Signer;

/// Fee minimum transaksi kanonikal (Dokumen 02/16): 10.000 Quanta.
pub const MIN_TX_FEE_QUANTA: u128 = 10_000;
/// Surcharge deploy kontrak (Dokumen 16 Bagian 5): 50.000 Quanta.
pub const DEPLOY_FEE_BASE_QUANTA: u128 = 50_000;
/// Biaya per byte kode kontrak saat deploy: 200 Quanta/byte (Dokumen 16 Bagian 5).
pub const DEPLOY_FEE_PER_BYTE_QUANTA: u128 = 200;
/// TTL default `valid_until` transaksi (detik).
pub const DEFAULT_TTL_SECS: u64 = 3_600;

/// Fee otomatis untuk panggilan metode (fee minimum protokol).
#[must_use]
pub fn auto_call_fee() -> Quantum {
    Quantum::new(MIN_TX_FEE_QUANTA)
}

/// Fee otomatis untuk deploy: base + surcharge + per-byte (Dokumen 16 Bagian 5).
#[must_use]
pub fn auto_deploy_fee(bytecode_len: usize) -> Quantum {
    let extra = DEPLOY_FEE_BASE_QUANTA
        .saturating_add(DEPLOY_FEE_PER_BYTE_QUANTA.saturating_mul(bytecode_len as u128));
    Quantum::new(MIN_TX_FEE_QUANTA.saturating_add(extra))
}

/// Opsi panggilan metode — seluruh field berharga memiliki default otomatis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallOptions {
    /// Nilai Quanta yang dikirim bersama panggilan.
    pub value: Quantum,
    /// Fee eksplisit; `None` = fee minimum protokol otomatis.
    pub fee: Option<Quantum>,
    /// Nonce eksplisit; `None` = **diambil otomatis** dari Provider.
    pub nonce: Option<u64>,
    /// Umur `valid_until` relatif terhadap waktu Provider (detik).
    pub ttl_secs: u64,
}

impl Default for CallOptions {
    fn default() -> Self {
        Self {
            value: Quantum::ZERO,
            fee: None,
            nonce: None,
            ttl_secs: DEFAULT_TTL_SECS,
        }
    }
}

impl CallOptions {
    /// Opsi dengan nilai Quanta tertentu (metode payable).
    #[must_use]
    pub fn with_value(value: Quantum) -> Self {
        Self {
            value,
            ..Self::default()
        }
    }
}

/// Permintaan deploy kontrak.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeployRequest {
    /// Nama kontrak untuk metadata & prompt.
    pub name: String,
    /// Bytecode konstruktor (payload `ContractDeploy`, jadi `code_hash` on-chain).
    pub constructor: Vec<u8>,
    /// Bytecode runtime untuk panggilan (terikat metadata via `runtime_hash`).
    pub runtime: Vec<u8>,
    /// Deklarasi metode ABI.
    pub methods: Vec<MethodAbi>,
    /// Saldo awal kontrak (value deploy).
    pub initial_balance: Quantum,
    /// Fee eksplisit; `None` = perhitungan otomatis Dokumen 16.
    pub fee: Option<Quantum>,
    /// Nonce eksplisit; `None` = otomatis dari Provider.
    pub nonce: Option<u64>,
    /// Umur `valid_until` relatif waktu Provider (detik).
    pub ttl_secs: u64,
}

impl DeployRequest {
    /// Permintaan baru tanpa metode & tanpa override fee/nonce.
    #[must_use]
    pub fn new(name: impl Into<String>, constructor: Vec<u8>, runtime: Vec<u8>) -> Self {
        Self {
            name: name.into(),
            constructor,
            runtime,
            methods: Vec::new(),
            initial_balance: Quantum::ZERO,
            fee: None,
            nonce: None,
            ttl_secs: DEFAULT_TTL_SECS,
        }
    }

    /// Tambahkan satu deklarasi metode ABI.
    #[must_use]
    pub fn with_method(mut self, method: MethodAbi) -> Self {
        self.methods.push(method);
        self
    }

    /// Tetapkan seluruh deklarasi metode sekaligus.
    #[must_use]
    pub fn with_methods(mut self, methods: Vec<MethodAbi>) -> Self {
        self.methods = methods;
        self
    }
}

/// Hasil deploy: TxID, alamat prediksi, dan bukti clear signing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeployOutcome {
    /// TxID kanonikal transaksi deploy.
    pub tx_id: Hash256,
    /// Alamat kontrak prediksi (`blake3_derive("AURION-CONTRACT-ADDRESS-V1")`).
    pub contract_address: Address,
    /// Format Bech32m alamat kontrak.
    pub contract_bech32m: String,
    /// Nonce yang dipakai (terikat ke alamat prediksi).
    pub nonce: u64,
    /// Fee yang dibayarkan.
    pub fee: Quantum,
    /// Gas hasil simulasi STF.
    pub gas_used: u64,
    /// `blake3(konstruktor)` = `code_hash` on-chain.
    pub code_hash: Hash256,
    /// Transaksi kanonikal tercatat (hex).
    pub raw_hex: String,
    /// Prompt clear signing yang disetujui (jejak audit).
    pub prompt: String,
}

/// Hasil panggilan metode: TxID + hasil simulasi yang mendahului signing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallOutcome {
    /// TxID kanonikal transaksi panggilan.
    pub tx_id: Hash256,
    /// Nonce otomatis/explicit yang dipakai.
    pub nonce: u64,
    /// Nilai Quanta terkirim.
    pub amount: Quantum,
    /// Fee yang dibayarkan.
    pub fee: Quantum,
    /// Gas hasil simulasi STF.
    pub gas_used: u64,
    /// Data kembalian VM dari simulasi.
    pub return_data: Vec<u8>,
    /// Transaksi kanonikal tercatat (hex).
    pub raw_hex: String,
    /// Prompt clear signing yang disetujui (jejak audit).
    pub prompt: String,
    /// Laporan dry-run lengkap.
    pub simulation: DryRunReport,
}

impl CallOutcome {
    /// Dekode word data kembalian (32 byte) ke nilai ABI ter-tipe.
    ///
    /// # Errors
    /// Panjang data kembalian bukan 32 byte.
    pub fn decode_return(&self, ty: AbiType) -> Result<AbiValue, ContractError> {
        if self.return_data.len() != 32 {
            return Err(ContractError::Abi(format!(
                "Data kembalian {} byte, diharapkan 32 byte",
                self.return_data.len()
            )));
        }
        let mut word = [0u8; 32];
        word.copy_from_slice(&self.return_data);
        ty.decode_word(&word)
    }
}

/// Transaksi yang sudah disiapkan & berhasil disimulasikan (belum ditandatangani).
struct PreparedCall {
    tx: Transaction,
    method: MethodAbi,
    rendered_args: Vec<String>,
    payload_hash: Hash256,
    code_hash: Hash256,
    chain_id: u32,
    simulation: DryRunReport,
}

/// Parameter penyusun transaksi kanonikal **belum ditandatangani**
/// (`Signature::ZERO`) untuk aksi deploy maupun call.
///
/// Menggantikan daftar argumen posisional `build_unsigned_tx` yang melampaui
/// ambang `clippy::too_many_arguments`, sekaligus memperjelas makna setiap
/// field di call site (AUR-ARCH-003: batas modul eksplisit).
///
/// # Fields
/// - `tx_type`: `ContractDeploy` atau `ContractCall`.
/// - `chain_id`: jaringan tujuan dari Provider.
/// - `sender`: pemilik tanda tangan (harus sama dengan `Signer::address`).
/// - `recipient`: alamat kontrak; `Address::ZERO` untuk deploy.
/// - `nonce`: nonce on-chain dari Provider.
/// - `amount`: nilai Quanta yang dikirim (value deploy / call payable).
/// - `fee`: fee transaksi dalam Quanta.
/// - `valid_until`: batas kedaluwarsa (detik Unix).
/// - `payload`: AVM Call Frame (call) atau konstruktor terverifikasi (deploy).
struct UnsignedTxSpec {
    tx_type: TxType,
    chain_id: u32,
    sender: Address,
    recipient: Address,
    nonce: u64,
    amount: Quantum,
    fee: Quantum,
    valid_until: u64,
    payload: Vec<u8>,
}

impl UnsignedTxSpec {
    /// Bangun transaksi kanonikal **belum bertanda tangan** (signature ZERO).
    #[must_use]
    fn build(self) -> Transaction {
        Transaction {
            version: 1,
            chain_id: self.chain_id,
            tx_type: self.tx_type,
            flags: 0,
            sender: self.sender,
            recipient: self.recipient,
            nonce: self.nonce,
            amount: self.amount,
            fee: self.fee,
            valid_until: self.valid_until,
            payload: self.payload,
            signature: Signature::ZERO,
        }
    }
}

/// Encode transaksi tercatat menjadi `(raw_hex, pubkey_hex)` untuk broadcast.
///
/// # Errors
/// Encoding gagal (tidak terjadi pada tipe kanonikal).
fn encode_signed(tx: &Transaction, pubkey: &[u8; 32]) -> Result<(String, String), ContractError> {
    let mut raw = Vec::new();
    tx.encode_canonical(&mut raw);
    Ok((hex::encode(&raw), hex::encode(pubkey)))
}

/// Pengikat kontrak: alamat + metadata + [`Provider`] + [`Signer`].
///
/// Satu tipe ini mewujudkan unifikasi Wallet dan VM: pembacaan state, penyusunan
/// transaksi otomatis, simulasi lokal, clear signing, dan broadcast berada dalam
/// satu alur di dalam binary `/bin/aurion`.
pub struct ContractInstance<P: Provider, S: Signer> {
    /// Alamat kontrak (byte 32).
    pub address: Address,
    /// Format Bech32m alamat kontrak.
    pub address_bech32m: String,
    /// Metadata kontrak (terikat `code_hash`).
    pub metadata: ContractMetadata,
    provider: P,
    signer: S,
}

impl<P: Provider + std::fmt::Debug, S: Signer> std::fmt::Debug for ContractInstance<P, S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ContractInstance")
            .field("address_bech32m", &self.address_bech32m)
            .field("metadata", &self.metadata)
            .field("provider", &self.provider)
            .field("signer_address", &self.signer.address())
            .finish()
    }
}

impl<P: Provider, S: Signer> ContractInstance<P, S> {
    /// Pasang instance kontrak yang sudah ada.
    ///
    /// # Errors
    /// - Alamat/`metadata.address` bukan Bech32m valid atau keduanya berbeda.
    /// - Chain ID metadata tidak cocok dengan Provider.
    pub fn new(
        address_bech32m: &str,
        metadata: ContractMetadata,
        provider: P,
        signer: S,
    ) -> Result<Self, ContractError> {
        let address = decode_address_bech32m(address_bech32m, "aur")
            .map_err(|e| ContractError::InvalidAddress(format!("{e}")))?;
        let meta_address = decode_address_bech32m(&metadata.address, "aur")
            .map_err(|e| ContractError::InvalidAddress(format!("{e}")))?;
        if address != meta_address {
            return Err(ContractError::InvalidAddress(format!(
                "Alamat instance ({address_bech32m}) tidak sama dengan metadata ({})",
                metadata.address
            )));
        }
        let chain = provider.chain_id()?;
        if chain != metadata.chain_id {
            return Err(ContractError::ChainIdMismatch {
                metadata: metadata.chain_id,
                provider: chain,
            });
        }
        Ok(Self {
            address,
            address_bech32m: address_bech32m.to_string(),
            metadata,
            provider,
            signer,
        })
    }

    /// Provider yang terpasang.
    #[must_use]
    pub fn provider(&self) -> &P {
        &self.provider
    }

    /// Signer yang terpasang.
    #[must_use]
    pub fn signer(&self) -> &S {
        &self.signer
    }

    /// Langkah 1-3 otomatisasi: binding metadata -> encode calldata -> ambil
    /// nonce -> bangun transaksi -> **simulasi lokal**. Tidak ada signing di sini.
    fn prepare_call(
        &self,
        method_name: &str,
        args: &[AbiValue],
        opts: &CallOptions,
    ) -> Result<PreparedCall, ContractError> {
        let method = self
            .metadata
            .find_method(method_name)
            .ok_or_else(|| ContractError::UnknownMethod(method_name.to_string()))?
            .clone();

        // Binding metadata ke kode on-chain (tolak metadata palsu).
        let account = self.provider.get_account(&self.address)?;
        self.metadata.verify_binding(account.code_hash.as_ref())?;

        // Serialisasi argumen ke AVM call frame + runtime (terverifikasi).
        let runtime = self.metadata.runtime_bytes()?;
        let payload = encode_call_payload(&method, args, &runtime)?;

        // Nonce & fee otomatis.
        let sender = self.signer.address();
        let nonce = match opts.nonce {
            Some(n) => n,
            None => self.provider.get_nonce(&sender)?,
        };
        let fee = opts.fee.unwrap_or_else(auto_call_fee);
        if fee.as_u128() < MIN_TX_FEE_QUANTA {
            return Err(ContractError::FeeBelowMinimum(fee.as_u128()));
        }

        let chain_id = self.provider.chain_id()?;
        if chain_id != self.metadata.chain_id {
            return Err(ContractError::ChainIdMismatch {
                metadata: self.metadata.chain_id,
                provider: chain_id,
            });
        }
        let valid_until = self.provider.current_time().saturating_add(opts.ttl_secs);

        let tx = UnsignedTxSpec {
            tx_type: TxType::ContractCall,
            chain_id,
            sender,
            recipient: self.address,
            nonce,
            amount: opts.value,
            fee,
            valid_until,
            payload,
        }
        .build();

        // Gerbang dry-run: eksekusi persis STF pada state sandbox.
        let simulation = self.provider.simulate(&tx)?;
        if !simulation.success {
            return Err(ContractError::SimulationFailed(
                simulation
                    .reason
                    .unwrap_or_else(|| "simulasi gagal tanpa keterangan".to_string()),
            ));
        }

        let rendered_args = method
            .inputs
            .iter()
            .zip(args.iter())
            .map(|(param, value)| format!("{} = {}", param.name, value.render()))
            .collect();

        // Hash payload diturunkan dari transaksi yang sudah dibangun; byte-nya
        // identik sehingga tidak perlu menyalin payload.
        let payload_hash = blake3_hash(&tx.payload);
        Ok(PreparedCall {
            tx,
            method,
            rendered_args,
            payload_hash,
            code_hash: self.metadata.code_hash_bytes()?,
            chain_id,
            simulation,
        })
    }

    /// Panggil metode kontrak dengan **pipeline otomatis penuh**:
    /// metadata -> nonce/gas otomatis -> simulasi lokal -> clear signing -> broadcast.
    ///
    /// # Errors
    /// Metode tidak ditemukan, binding `code_hash` gagal, argumen tidak valid,
    /// simulasi revert/gagal, intent tidak cocok, ditolak pengguna, atau broadcast gagal.
    pub fn call(
        &self,
        method_name: &str,
        args: &[AbiValue],
        opts: &CallOptions,
    ) -> Result<CallOutcome, ContractError> {
        let prepared = self.prepare_call(method_name, args, opts)?;
        let sender_bech32m = encode_address_bech32m(&prepared.tx.sender, "aur")
            .map_err(|e| ContractError::InvalidAddress(e.to_string()))?;

        let intent = ContractIntent {
            action: IntentAction::Call,
            chain_id: prepared.chain_id,
            sender: prepared.tx.sender,
            sender_bech32m,
            tx_recipient: prepared.tx.recipient,
            tx_recipient_bech32m: self.address_bech32m.clone(),
            contract_bech32m: self.address_bech32m.clone(),
            contract_name: self.metadata.name.clone(),
            method_name: Some(prepared.method.signature.clone()),
            rendered_args: prepared.rendered_args,
            amount: prepared.tx.amount,
            fee: prepared.tx.fee,
            nonce: prepared.tx.nonce,
            valid_until: prepared.tx.valid_until,
            payload_hash: prepared.payload_hash,
            code_hash: prepared.code_hash,
            dry_run: prepared.simulation.clone(),
        };

        let signed = self.signer.sign(&intent, &prepared.tx)?;
        let (raw_hex, pubkey_hex) = encode_signed(&signed, &self.signer.public_key())?;
        let tx_id = self.provider.broadcast(&raw_hex, &pubkey_hex)?;

        Ok(CallOutcome {
            tx_id,
            nonce: prepared.tx.nonce,
            amount: prepared.tx.amount,
            fee: prepared.tx.fee,
            gas_used: prepared.simulation.gas_used,
            return_data: prepared.simulation.return_data.clone(),
            raw_hex,
            prompt: intent.format_clear_signing_prompt(),
            simulation: prepared.simulation,
        })
    }

    /// Panggil metode **hanya untuk simulasi** (view/dry-run): tidak menandatangani
    /// dan tidak menyiarkan apa pun.
    ///
    /// # Errors
    /// Sama dengan [`ContractInstance::call`] hingga tahap signing.
    pub fn read(
        &self,
        method_name: &str,
        args: &[AbiValue],
        opts: &CallOptions,
    ) -> Result<DryRunReport, ContractError> {
        let prepared = self.prepare_call(method_name, args, opts)?;
        Ok(prepared.simulation)
    }

    /// **Deploy kontrak otomatis**: nonce -> verifikasi konstruktor -> simulasi
    /// STF -> clear signing -> broadcast, lalu kembalikan siap instance untuk dipanggil.
    ///
    /// # Errors
    /// Konstruktor/runtime tidak valid, fee/nonce tidak valid, simulasi gagal,
    /// ditolak pengguna, atau broadcast gagal.
    pub fn deploy(
        provider: P,
        signer: S,
        request: DeployRequest,
    ) -> Result<(DeployOutcome, Self), ContractError> {
        let chain_id = provider.chain_id()?;
        let sender = signer.address();
        let nonce = match request.nonce {
            Some(n) => n,
            None => provider.get_nonce(&sender)?,
        };
        let fee = request.fee.unwrap_or_else(|| auto_deploy_fee(request.constructor.len()));
        if fee.as_u128() < MIN_TX_FEE_QUANTA {
            return Err(ContractError::FeeBelowMinimum(fee.as_u128()));
        }
        let valid_until = provider.current_time().saturating_add(request.ttl_secs);

        // Payload deploy = konstruktor terverifikasi statis.
        let payload = encode_deploy_payload(&request.constructor)?;

        // Alamat kontrak prediksi dari (sender, nonce) — konsisten dengan STF.
        let contract_address = derive_contract_address(&sender, nonce);
        let contract_bech32m = encode_address_bech32m(&contract_address, "aur")
            .map_err(|e| ContractError::InvalidAddress(e.to_string()))?;
        let code_hash = blake3_hash(&request.constructor);

        let metadata = ContractMetadata::new(
            &request.name,
            chain_id,
            &contract_bech32m,
            &code_hash,
            &request.runtime,
            request.methods,
        )?;

        let tx = UnsignedTxSpec {
            tx_type: TxType::ContractDeploy,
            chain_id,
            sender,
            recipient: Address::ZERO,
            nonce,
            amount: request.initial_balance,
            fee,
            valid_until,
            payload,
        }
        .build();

        // Gerbang dry-run sebelum signing.
        let simulation = provider.simulate(&tx)?;
        if !simulation.success {
            return Err(ContractError::SimulationFailed(
                simulation
                    .reason
                    .unwrap_or_else(|| "simulasi deploy gagal".to_string()),
            ));
        }
        if simulation.deployed_contract != Some(contract_address) {
            return Err(ContractError::SimulationFailed(format!(
                "Prediksi alamat kontrak tidak konsisten: prediksi {contract_bech32m}, STF {:?}",
                simulation
                    .deployed_contract
                    .map(|a| a.to_hex())
                    .unwrap_or_default()
            )));
        }

        let sender_bech32m = encode_address_bech32m(&sender, "aur")
            .map_err(|e| ContractError::InvalidAddress(e.to_string()))?;
        // Panjang & hash payload diambil dari transaksi yang sudah dibangun.
        let payload_len = tx.payload.len();
        let intent = ContractIntent {
            action: IntentAction::Deploy,
            chain_id,
            sender,
            sender_bech32m,
            tx_recipient: Address::ZERO,
            tx_recipient_bech32m: encode_address_bech32m(&Address::ZERO, "aur")
                .map_err(|e| ContractError::InvalidAddress(e.to_string()))?,
            contract_bech32m: contract_bech32m.clone(),
            contract_name: request.name.clone(),
            method_name: None,
            rendered_args: vec![format!("konstruktor = {payload_len} byte bytecode")],
            amount: request.initial_balance,
            fee,
            nonce,
            valid_until,
            payload_hash: blake3_hash(&tx.payload),
            code_hash,
            dry_run: simulation.clone(),
        };

        let signed = signer.sign(&intent, &tx)?;
        let (raw_hex, pubkey_hex) = encode_signed(&signed, &signer.public_key())?;
        let tx_id = provider.broadcast(&raw_hex, &pubkey_hex)?;

        let outcome = DeployOutcome {
            tx_id,
            contract_address,
            contract_bech32m: contract_bech32m.clone(),
            nonce,
            fee,
            gas_used: simulation.gas_used,
            code_hash,
            raw_hex,
            prompt: intent.format_clear_signing_prompt(),
        };

        let instance = Self {
            address: contract_address,
            address_bech32m: contract_bech32m,
            metadata,
            provider,
            signer,
        };
        Ok((outcome, instance))
    }
}
