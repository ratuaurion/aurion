//! Tipe kesalahan Contract SDK (Unifikasi Wallet & VM Aurion).
//! Mematuhi Invariant AUR-ARCH-011 (Zero Unsafe) & AUR-ARCH-012 (Zero Float).

use thiserror::Error;

/// Kesalahan tunggal seluruh alur Contract SDK (`Provider` -> `Signer` -> Broadcast).
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ContractError {
    /// Kegagalan transport/infrastruktur RPC (jaringan, respons tidak valid).
    #[error("Provider RPC gagal: {0}")]
    Provider(String),

    /// Metadata kontrak tidak valid atau gagal di-decode.
    #[error("Metadata kontrak tidak valid: {0}")]
    Metadata(String),

    /// Metode tidak terdaftar dalam metadata kontrak.
    #[error("Metode '{0}' tidak ditemukan dalam metadata kontrak")]
    UnknownMethod(String),

    /// Enkripsi/dekripsi argumen ABI gagal (tipe tidak cocok atau jumlah salah).
    #[error("Enkripsi ABI gagal: {0}")]
    Abi(String),

    /// Verifikasi statis bytecode AVM gagal (AUR-VM-005).
    #[error("Verifikasi bytecode AVM gagal: {0}")]
    Verification(String),

    /// Simulasi lokal (dry-run) terhadap STF gagal sebelum signing.
    #[error("Simulasi (dry-run) gagal: {0}")]
    SimulationFailed(String),

    /// Guardrail anti-blind-signing: intent tidak cocok dengan transaksi yang akan ditandatangani.
    #[error("Intent clear-signing tidak cocok dengan transaksi: {0}")]
    IntentMismatch(String),

    /// Kunci signer tidak cocok dengan alamat pengirim transaksi.
    #[error("Signer tidak cocok dengan pengirim transaksi")]
    SignerMismatch,

    /// Pengguna menolak prompt clear signing.
    #[error("Pengguna menolak persetujuan clear signing")]
    UserRejected,

    /// Validasi nir-status (stateless) transaksi tercatat gagal.
    #[error("Validasi transaksi nir-status gagal: {0}")]
    StatelessValidation(String),

    /// Broadcast transaksi ke mempool/jaringan gagal.
    #[error("Broadcast transaksi gagal: {0}")]
    Broadcast(String),

    /// Alamat Bech32m tidak valid.
    #[error("Alamat tidak valid: {0}")]
    InvalidAddress(String),

    /// Fee di bawah batas minimum protokol (10.000 Quanta).
    #[error("Fee transaksi {0} Q di bawah batas minimum 10.000 Q")]
    FeeBelowMinimum(u128),

    /// Jendela instruksi `DUP4`/`SWAP4` AVM hanya menampung maksimal 4 argumen.
    #[error("Terlalu banyak argumen ({got}); jendela stack AVM membatasi maksimal {max} argumen")]
    TooManyArguments { got: usize, max: usize },

    /// Metadata terikat pada chain ID berbeda dari Provider.
    #[error("Chain ID tidak cocok: metadata={metadata}, provider={provider}")]
    ChainIdMismatch { metadata: u32, provider: u32 },

    /// Binding metadata ke kode kontrak on-chain gagal (pertahanan anti-penipuan).
    #[error("Binding kode kontrak gagal: {0}")]
    CodeBinding(String),
}
