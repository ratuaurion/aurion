# 07 — AURION GENESIS SPECIFICATION
## Spesifikasi Formal Blok Nol (Genesis State), Alokasi Master Treasury, dan Jangkar Konsensus BFT

> **Hierarki Dokumen:**  
> `00 — AURION CONSTITUTION` $\longrightarrow$ `01 — MONETARY` $\longrightarrow$ `02 — CONSENSUS` $\longrightarrow$ `03 — STATE TRANSITION` $\longrightarrow$ `04 — TRANSACTION` $\longrightarrow$ `05 — CRYPTOGRAPHY` $\longrightarrow$ `06 — SERIALIZATION & WIRE` $\longrightarrow$ **`07 — GENESIS SPECIFICATION`**  
>
> **Status:** AUDITED & LOCKED SPECIFICATION  
> **Klasifikasi:** Spesifikasi Bootstrapping State Protokol Layer 1 (Genesis Core)  
> **Versi Protokol:** 1.0.0-BFT  
> **Sifat Ketetapan:** Kriptografis, Deterministik Mutlak, Titik Awal Kekal Rantai Blok

---

## 1. Prinsip dan Hakikat Blok Nol (Genesis Block)

1. **Jangkar Kedaulatan Tunggal:** Blok Nol ($H=0$) adalah akar absolut dari seluruh rantai blok Aurion. Seluruh pohon state akun, saldo moneter awal, dan himpunan validator genesis ($\mathcal{V}_0$) didefinisikan secara deterministik pada titik ini.
2. **Ketiadaan Transaksi Eksternal:** Blok Nol tidak memuat transaksi transfer atau coinbase buatan pengguna. Seluruh saldo awal tercipta secara murni melalui inisialisasi state tree ($\sigma_0$).
3. **Imutabilitas Permanen:** Setelah diratifikasi, konfigurasi Blok Nol tidak dapat diubah oleh proposal tata kelola on-chain maupun *soft fork*. Segala perubahan terhadap parameter genesis menghasilkan jaringan rantai independen yang terpisah.

---

## 2. Skema Objek Spesifikasi Genesis (Genesis Object Schema)

Struktur data formal spesifikasi genesis didefinisikan sebagai:

```text
GenesisSpecification
├── protocol_version       : u32        (Format versi protokol, 0x00000001)
├── chain_id               : u32        (Mainnet = 1001)
├── genesis_time           : u64        (Unix epoch timestamp resmi dalam detik)
├── initial_supply         : InitialSupplyConfig
│   └── master_treasury    : AccountAllocation (100% = 66.000.000 AUR)
├── consensus_parameters   : BftConsensusParams
│   ├── target_block_time  : u64        (60 detik)
│   ├── epoch_length       : u64        (10.000 blok)
│   ├── block_reward       : Quantum    (1.000.000.000 Q = 1 AUR)
│   ├── proposer_share_pct : u128       (20%)
│   ├── voter_share_pct    : u128       (80%)
│   ├── gas_to_validator   : u128       (100%)
│   └── max_payload_bytes  : u32        (4.194.304 B = 4 MB)
└── initial_validators     : Vector<GenesisValidator>
    └── [validator_id, consensus_pubkey, voting_weight]
```

---

## 3. Alokasi Moneter Awal dan Akun Genesis

Mengukuhkan mandat **[CONSTITUTION.md](file:///c:/Projects/aurion/CONSTITUTION.md)** dan **[AURION-MONETARY-POLICY-SPECIFICATION.md](file:///c:/Projects/aurion/docs/Constitutions/AURION-MONETARY-POLICY-SPECIFICATION.md)**:

### 3.1 Rincian Alokasi Pasokan Genesis

$$\mathbf{S_{\text{genesis}} = 66.000.000\ AUR = 66.000.000.000.000.000\ Quantum\ (10^9\ \text{Scale})}$$

| Entitas Akun Genesis | Alamat Kanonikal (Bech32m Mainnet) | Saldo dalam AUR | Saldo dalam Quantum ($Q$) | Status Hak |
| :--- | :--- | ---:| ---:| :--- |
| **Master Treasury Account** | `aur1jjtqrlqy9suehhltnzt2ml4zwsr8ukpyvvhm2gw899u0e0w22qusq0pjql` | $66.000.000\ \text{AUR}$ | $66.000.000.000.000.000\ Q$ | Terbit penuh pada State $\sigma_0$ |
| **TOTAL INITIAL STATE** | — | $\mathbf{66.000.000\ \text{AUR}}$ | $\mathbf{66.000.000.000.000.000\ Q}$ | **100% Pasokan Dasar Blok 0** |

### 3.2 Kunci Kanonikal dan Derivasi Akun Master Treasury

- **Address (Bech32m Mainnet):** `aur1jjtqrlqy9suehhltnzt2ml4zwsr8ukpyvvhm2gw899u0e0w22qusq0pjql`
- **Address (Hex, 32 Bytes):** `cb095697ccc5acbf23e176ee4f05e7e77ddfe54236bfe8c8b42e78c3d5a95aad`
- **Public Key (Ed25519, Hex):** `8a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c`
- **Derivation Path BIP-44:** `m/44'/9999'/0'/0'/0'`

Akun Master Treasury mengisi state genesis awal $\sigma_0$ sebagai saldo perbendaharaan berdaulat untuk menopang seluruh kebutuhan operasional, likuiditas, infrastruktur bootnode, dan cadangan jaringan.

### 3.3 Invarian Suplai Awal Blok Nol
Pada pembentukan state genesis $\sigma_0$:
1. **Total Suplai Diterbitkan:**
   $$S_{\text{emitted}}(0) = 66.000.000.000.000.000\ Q\ (100\%)$$
2. **Total Suplai Aktif Beredar:**
   $$S_{\text{circulating}}(0) = 66.000.000.000.000.000\ Q$$
3. **Total Suplai Terbakar:**
   $$S_{\text{burned}}(0) = 0\ Q$$

---

## 4. Himpunan Validator Genesis (Initial Validator Set $\mathcal{V}_0$)

Untuk memastikan konsensus Aurion-BFT dapat langsung memproses proposal blok pertama ($H=1$) tanpa penundaan:

### 4.1 Definisi Himpunan $\mathcal{V}_0$
Himpunan validator genesis $\mathcal{V}_0$ terdiri dari sekurang-kurangnya empat simpul genesis independen untuk memenuhi toleransi kegagalan Byzantine minimum ($N \ge 3f + 1$ dengan $f=1 \implies N=4$):

$$\mathcal{V}_0 = \big\{ \text{Val}_1,\; \text{Val}_2,\; \text{Val}_3,\; \text{Val}_4 \big\}$$

| Simpul Validator | Bobot Voting ($w_i$) | Persentase Hak Suara | Peran Operasional Genesis |
| :--- | ---:| ---:| :--- |
| **Genesis Validator 1** | $250.000$ | $25,00\%$ | Primary BFT Engine Alpha |
| **Genesis Validator 2** | $250.000$ | $25,00\%$ | Primary BFT Engine Beta |
| **Genesis Validator 3** | $250.000$ | $25,00\%$ | Primary BFT Engine Gamma |
| **Genesis Validator 4** | $250.000$ | $25,00\%$ | Primary BFT Engine Delta |
| **TOTAL BOBOT ($W_0$)** | $\mathbf{1.000.000}$ | $\mathbf{100,00\%}$ | **Kuorum $\mathcal{Q}_0 = 666.667$ Suara** |

### 4.2 Ambang Batas Kuorum Genesis
$$\mathcal{Q}_0 = \left\lfloor \frac{2 \times 1.000.000}{3} \right\rfloor + 1 = 666.666 + 1 = 666.667\ \text{Suara}\ (> \frac{2}{3} W_0)$$

Setiap proposal blok pada Epoch 0 ($H \in [1, 10.000]$) wajib mengumpulkan tanda tangan Precommit dengan total bobot $\ge 666.667$ untuk mencapai finalitas Quorum Certificate (QC).

---

## 5. Komitmen Kriptografis dan Struktur Blok Nol (Block 0)

```text
┌────────────────────────────────────────────────────────────────────────┐
│                   STRUKTUR HEADER BLOK NOL (BLOCK 0)                   │
├───────────────────┬──────────────┬─────────────────────────────────────┤
│ FIELD HEADER      │ NILAI BINER  │ KETERANGAN FORMAL                   │
├───────────────────┼──────────────┼─────────────────────────────────────┤
│ version           │ 0x00000001   │ Versi Protokol 1                    │
├───────────────────┼──────────────┼─────────────────────────────────────┤
│ height            │ 0x00000000   │ Height H = 0 (Akar Rantai)          │
├───────────────────┼──────────────┼─────────────────────────────────────┤
│ round             │ 0x00000000   │ Putaran R = 0                       │
├───────────────────┼──────────────┼─────────────────────────────────────┤
│ timestamp         │ T_genesis    │ Unix Epoch Resmi Genesis            │
├───────────────────┼──────────────┼─────────────────────────────────────┤
│ prev_block_hash   │ 0x0000...00  │ Tepat 32 Bytes Nilai Nol (Genesis)  │
├───────────────────┼──────────────┼─────────────────────────────────────┤
│ tx_merkle_root    │ 0x0000...00  │ Tepat 32 Bytes Nilai Nol (Zero Tx)  │
├───────────────────┼──────────────┼─────────────────────────────────────┤
│ state_root        │ StateRoot_0  │ Blake3 Root SMT dari State σ_0      │
└───────────────────┴──────────────┴─────────────────────────────────────┘
```

### 5.1 Karakteristik Unik Blok Nol
1. **Ketiadaan Blok Induk:** `prev_block_hash` diisi dengan array 32-byte bernilai nol (`[0u8; 32]`).
2. **Ketiadaan Transaksi:** Blok Nol tidak memuat transaksi reguler maupun transaksi luar (`transactions_count = 0`). Saldo genesis didefinisikan langsung pada pohon Sparse Merkle Tree (SMT) $\sigma_0$.
3. **Ketiadaan Sertifikat Kuorum Induk:** Blok Nol tidak memerlukan QC induk karena keabsahannya diverifikasi langsung terhadap konfigurasi biner `genesis.json` kanonikal.

---

## 6. Prosedur Perhitungan Genesis Hash Kanonikal

1. **Konstruksi SMT State Awal ($\sigma_0$):**
   - Masukkan akun Master Treasury pada kunci `DeriveKey(TreasuryAddress)` dengan saldo $66.000.000.000.000.000\ Q$ dan nonce $0$.
   - Masukkan entri keempat validator genesis $\mathcal{V}_0$.
   - Dapatkan `StateRoot_0 = SMT_Root(\sigma_0)`.
2. **Perakitan Header 124 Bytes:**
   $$\text{HeaderBytes}_0 = \text{version} \parallel \text{height} \parallel \text{round} \parallel \text{timestamp} \parallel \mathbf{0}_{32} \parallel \mathbf{0}_{32} \parallel \text{StateRoot}_0$$
3. **Komputasi Genesis Hash:**
   $$\mathbf{GenesisHash} = \text{Blake3}\Big( \text{"AURION-BLOCK-ID-V1"} \mathbin{\Vert} \text{HeaderBytes}_0 \Big)$$

---

## 7. Rangkuman Konstanta Resmi Genesis Mainnet

```rust
// ==============================================================================
// AURION MAINNET GENESIS CONSTANTS (1.0.0-BFT)
// ==============================================================================

/// Protocol Version
pub const GENESIS_PROTOCOL_VERSION: u32 = 1;

/// Network Chain ID (Mainnet = 1001)
pub const GENESIS_CHAIN_ID: u32 = 1001;

/// Genesis Unix Timestamp Resmi Peluncuran Mainnet
pub const GENESIS_TIMESTAMP_MAINNET: u64 = 1773532800;

/// Total Suplai Terbit pada Blok 0 (100% = 66 Juta AUR = 6,6 x 10^16 Quantum)
pub const GENESIS_MASTER_TREASURY_QUANTA: u128 = 66_000_000_000_000_000;

/// Unit atomik per 1 AUR (10^9)
pub const GENESIS_QUANTA_PER_AUR: u128 = 1_000_000_000;

/// Reward blok BFT (1 AUR per blok)
pub const GENESIS_BLOCK_REWARD_QUANTA: u128 = 1_000_000_000;

/// Jumlah Validator Pemula
pub const GENESIS_VALIDATOR_COUNT: usize = 4;

/// Total Voting Power Awal
pub const GENESIS_TOTAL_VOTING_POWER: u64 = 1_000_000;

/// Kuorum Konsensus Awal (> 2/3)
pub const GENESIS_QUORUM_THRESHOLD: u64 = 666_667;
```
