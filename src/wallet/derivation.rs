//! Derivasi Kunci Hierarkis Ed25519 (SLIP-0010 / BIP-44) Jalur Resmi Aurion.
//! Mematuhi Dokumen 01 (01-WALLET-RULES.md Bagian 1.3).
//! Jalur Resmi: m / 44' / 9999' / account' / 0' / address_index'

use crate::core::Address;
use crate::crypto::{derive_address_from_pubkey, encode_address_bech32m};
use crate::wallet::sha512::hmac_sha512;
use ed25519_dalek::SigningKey;
use zeroize::Zeroize;

pub const AURION_COIN_TYPE: u32 = 9999;
const HARDENED_OFFSET: u32 = 0x8000_0000;

#[derive(Debug, Clone)]
pub struct ExtendedKey {
    pub key: [u8; 32],
    pub chain_code: [u8; 32],
}

impl Drop for ExtendedKey {
    fn drop(&mut self) {
        self.key.zeroize();
        self.chain_code.zeroize();
    }
}

impl ExtendedKey {
    /// Menghasilkan Master Key dari 64-byte Seed (SLIP-0010).
    pub fn from_master_seed(seed: &[u8; 64]) -> Self {
        let i = hmac_sha512(b"ed25519 seed", seed);
        let mut key = [0u8; 32];
        let mut chain_code = [0u8; 32];
        key.copy_from_slice(&i[0..32]);
        chain_code.copy_from_slice(&i[32..64]);
        Self { key, chain_code }
    }

    /// Menurunkan anak kunci terkeraskan (Hardened Child Key) dengan indeks `i`.
    pub fn derive_hardened(&self, index: u32) -> Self {
        let mut data = [0u8; 37];
        data[0] = 0x00;
        data[1..33].copy_from_slice(&self.key);
        let hardened_index = HARDENED_OFFSET | index;
        data[33..37].copy_from_slice(&hardened_index.to_be_bytes());

        let i = hmac_sha512(&self.chain_code, &data);
        let mut child_key = [0u8; 32];
        let mut child_chain = [0u8; 32];
        child_key.copy_from_slice(&i[0..32]);
        child_chain.copy_from_slice(&i[32..64]);

        Self {
            key: child_key,
            chain_code: child_chain,
        }
    }
}

/// Akun Dompet Aurion yang Diturunkan.
pub struct DerivedAccount {
    pub path: String,
    pub signing_key: SigningKey,
    pub address: Address,
    pub bech32m_address: String,
}

impl DerivedAccount {
    /// Menurunkan akun Aurion resmi pada jalur `m / 44' / 9999' / account' / 0' / address_index'`.
    pub fn derive_account(master_seed: &[u8; 64], account: u32, address_index: u32) -> Self {
        let master = ExtendedKey::from_master_seed(master_seed);
        let purpose = master.derive_hardened(44);
        let coin_type = purpose.derive_hardened(AURION_COIN_TYPE);
        let acc = coin_type.derive_hardened(account);
        let change = acc.derive_hardened(0);
        let target = change.derive_hardened(address_index);

        let signing_key = SigningKey::from_bytes(&target.key);
        let verifying_key = signing_key.verifying_key();
        let address = derive_address_from_pubkey(verifying_key.as_bytes());
        let bech32m_address = encode_address_bech32m(&address, "aur")
            .expect("Alamat kanonikal Aurion harus valid Bech32m");

        let path = format!("m/44'/9999'/{account}'/0'/{address_index}'");

        Self {
            path,
            signing_key,
            address,
            bech32m_address,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_derivation_path_aurion_deterministic() {
        let seed = [0x5au8; 64];
        let acc1 = DerivedAccount::derive_account(&seed, 0, 0);
        let acc2 = DerivedAccount::derive_account(&seed, 0, 0);

        assert_eq!(acc1.path, "m/44'/9999'/0'/0'/0'");
        assert_eq!(acc1.address, acc2.address);
        assert_eq!(acc1.bech32m_address, acc2.bech32m_address);
        assert!(acc1.bech32m_address.starts_with("aur1"));

        let acc_diff_idx = DerivedAccount::derive_account(&seed, 0, 1);
        assert_ne!(acc1.address, acc_diff_idx.address);
    }
}
