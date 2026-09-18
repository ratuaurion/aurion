# 07 — AURION GENESIS SPECIFICATION
## Spesifikasi Formal Genesis State, Parameter Inisial, dan Hash Blok Nol Protokol Aurion

> **Hierarki Dokumen:**  
> `00 — AURION CONSTITUTION` $\longrightarrow$ `01 — MONETARY` $\longrightarrow$ `02 — CONSENSUS` $\longrightarrow$ `03 — STATE TRANSITION` $\longrightarrow$ `04 — TRANSACTION` $\longrightarrow$ `05 — CRYPTOGRAPHY` $\longrightarrow$ `06 — SERIALIZATION & WIRE` $\longrightarrow$ **`07 — GENESIS SPECIFICATION`**  
>
> **Status:** AUDITED & LOCKED SPECIFICATION  
> **Klasifikasi:** Inisialisasi Kedaulatan Protokol Layer 0 (Genesis Core)  
> **Versi:** 1.0.0-PROD  
> **Sifat Ketetapan:** Imutabel, Kriptografis, Titik Awal Deterministik Tunggal (*Unique Root of Trust*)

---

## 1. Hakikat dan Filosofi Genesis Aurion

Genesis dalam Aurion bukanlah sekadar file konfigurasi format JSON yang fleksibel atau dapat diubah-ubah menurut selera operator simpul.

Genesis adalah **akar kebenaran kriptografis pertama (*primordial root of cryptographic truth*)** yang mengunci:
1. Batas-batas konstitusi dan kebijakan moneter yang telah diratifikasi;
2. Penyerahan hak awal atas 35% suplai genesis (Creator 30% dan Developer 5%);
3. Penguncian 65% suplai komunitas untuk ditambang secara murni;
4. Parameter konsensus dan himpunan validator pemula (*genesis validator set*);
5. Komitmen hash tunggal yang tidak dapat dipalsukan.

```text
┌────────────────────────────────────────────────────────────────────────┐
│                        PIPA DETERMINISTIK GENESIS                      │
│                                                                        │
│   Genesis Specification Object (GSO)                                   │
│                 │                                                      │
│                 ▼ Canonical Binary Serialization                       │
│   Genesis State (σ_0)                                                  │
│                 │                                                      │
│                 ▼ Blake3 Sparse Merkle Tree Construction               │
│   StateRoot_0 (Hash256)                                                │
│                 │                                                      │
│                 ▼ Assembly Header Blok Nol                             │
│   Block 0 (Genesis Block)                                              │
│                 │                                                      │
│                 ▼ Blake3 Canonical Hashing ("AURION-BLOCK-ID-V1")      │
│   GENESIS HASH (Root of Trust Jaringan Aurion)                         │
└────────────────────────────────────────────────────────────────────────┘
```

> **Invarian Konsensus Genesis:**  
> Seluruh simpul yang mengklaim menjalankan rantai Aurion yang sah **WAJIB (MUST)** menghasilkan `GenesisHash` yang identik hingga ke tingkat bit terakhir. Simpul dengan `GenesisHash` yang berbeda dianggap berada pada alam semesta konsensus yang berbeda dan ditolak seketika pada jabat tangan P2P wire (*Handshake Termination*).

---

## 2. Skema Objek Spesifikasi Genesis (Genesis Object Schema)

Struktur data formal spesifikasi genesis didefinisikan sebagai:

```text
GenesisSpecification
├── protocol_version       : u32        (Format versi protokol, 0x00000001)
├── chain_id               : u32        (Mainnet = 1001)
├── genesis_time           : u64        (Unix epoch timestamp resmi dalam detik)
├── initial_supply         : InitialSupplyConfig
│   ├── creator_vault      : AccountAllocation (30% = 19.800.000 AUR)
│   ├── developer_vault    : AccountAllocation ( 5% =  3.300.000 AUR)
│   └── unminted_reserve   : Quantum           (65% = 42.900.000 AUR)
├── consensus_parameters   : ConsensusParams
│   ├── target_block_time  : u64        (60 detik)
│   ├── epoch_length       : u64        (10.000 blok)
│   ├── halving_interval   : u64        (2.145.000 blok)
│   ├── initial_subsidy    : Quantum    (1.000.000.000 Q = 10 AUR)
│   ├── coinbase_maturity  : u64        (100 blok)
│   ├── fee_burn_pct       : u128       (20%)
│   ├── fee_miner_pct      : u128       (80%)
│   └── max_payload_bytes  : u32        (4.194.304 B = 4 MB)
└── initial_validators     : Vector<GenesisValidator>
    └── [validator_id, consensus_pubkey, voting_weight]
```

---

## 3. Alokasi Moneter Awal dan Akun Genesis

Mengukuhkan mandat **Bab 2 [AURION CONSTITUTION.md](file:///c:/Projects/aurion/AURION%20CONSTITUTION.md)** dan **[AURION-MONETARY-POLICY-SPECIFICATION.md](file:///c:/Projects/aurion/AURION-MONETARY-POLICY-SPECIFICATION.md)**:

### 3.1 Rincian Alokasi Suplai Genesis

$$\mathbf{S_{\max} = 66.000.000\ AUR = 6.600.000.000.000.000\ Quantum\ (10^8\ \text{Scale})}$$

| Entitas Akun Genesis | Alamat Kanonikal (Bech32m Mainnet) | Saldo dalam AUR | Saldo dalam Quantum ($Q$) | Status Hak |
| :--- | :--- | ---:| ---:| :--- |
| **Creator Vault** | `aur1q_creator_vault_sovereign_mainnet_genesis_key_001` | $19.800.000\ \text{AUR}$ | $1.980.000.000.000.000\ Q$ | Terbit pada State $\sigma_0$ |
| **Developer Vault** | `aur1q_developer_vault_r_and_d_faucet_source_key_002` | $3.300.000\ \text{AUR}$ | $330.000.000.000.000\ Q$ | Terbit pada State $\sigma_0$ |
| **Cadangan Penambangan** | *Virtual Reserve (Belum Terbit)* | $42.900.000\ \text{AUR}$ | $4.290.000.000.000.000\ Q$ | Terbit via PoW / BFT Block Rewards ($H \ge 1$) |
| **TOTAL INITIAL STATE** | — | $\mathbf{23.100.000\ \text{AUR}}$ | $\mathbf{2.310.000.000.000.000\ Q}$ | **Total Suplai Beredar Awal (35%)** |

### 3.2 Invarian Suplai Awal Blok Nol
Pada pembentukan state genesis $\sigma_0$:
1. **Total Suplai Pernah Diterbitkan:**
   $$S_{\text{emitted}}(0) = 1.980.000.000.000.000 + 330.000.000.000.000 = 2.310.000.000.000.000\ Q\ (35\%)$$
2. **Total Suplai Aktif Beredar:**
   $$S_{\text{circulating}}(0) = S_{\text{emitted}}(0) = 2.310.000.000.000.000\ Q$$
3. **Total Suplai Terbakar:**
   $$S_{\text{burned}}(0) = 0\ Q$$
4. **Cadangan Tertunda Komunitas:**
   $$S_{\text{mining\_reserve}} = 4.290.000.000.000.000\ Q\ (65\%)$$

---

## 4. Himpunan Validator Genesis (Initial Validator Set $\mathcal{V}_0$)

Untuk memastikan konsensus Aurion-BFT dapat langsung memproses blok pertama ($H=1$) tanpa ketergantungan eksternal:

### 4.1 Definisi Himpunan $\mathcal{V}_0$
Himpunan validator genesis $\mathcal{V}_0$ terdiri dari sekurang-kurangnya empat simpul genesis independen untuk memenuhi toleransi kegagalan Byzantine minimum ($N \ge 3f + 1$ dengan $f=1 \implies N=4$):

$$\mathcal{V}_0 = \big\{ \text{Val}_1,\; \text{Val}_2,\; \text{Val}_3,\; \text{Val}_4 \big\}$$

| Simpul Validator | Bobot Voting ($w_i$) | Persentase Hak Suara | Peran Operasional Genesis |
| :--- | ---:| ---:| :--- |
| **Genesis Validator 1** | $250.000$ | $25,00\%$ | Primary Bootnode Alpha |
| **Genesis Validator 2** | $250.000$ | $25,00\%$ | Primary Bootnode Beta |
| **Genesis Validator 3** | $250.000$ | $25,00\%$ | Primary Bootnode Gamma |
| **Genesis Validator 4** | $250.000$ | $25,00\%$ | Primary Bootnode Delta |
| **TOTAL BOBOT ($W_0$)** | $\mathbf{1.000.000}$ | $\mathbf{100,00\%}$ | **Kuorum $\mathcal{Q}_0 = 666.667$ Suara** |

### 4.2 Ambang Batas Kuorum Genesis
$$\mathcal{Q}_0 = \left\lfloor \frac{2 \times 1.000.000}{3} \right\rfloor + 1 = 666.666 + 1 = 666.667\ \text{Suara}\ (> 66,6667\%)$$

Setiap proposal blok pada Epoch 0 ($H \in [1, 10.000]$) wajib mengumpulkan tanda tangan Pre-commit dengan total bobot $\ge 666.667$ untuk mencapai status finalitas.

---

## 5. Komitmen Kriptografis dan Struktur Blok Nol (Block 0)

Blok Nol (Genesis Block) adalah satu-satunya blok dalam sejarah Aurion yang tidak diproduksi oleh Proposer reguler, melainkan dibentuk secara deterministik murni dari komitmen state awal.

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
2. **Ketiadaan Transaksi:** Blok Nol tidak memuat transaksi reguler maupun transaksi coinbase (`transactions_count = 0`). Saldo genesis tidak diciptakan via transaksi, melainkan didefinisikan langsung pada state tree $\sigma_0$.
3. **Ketiadaan Sertifikat Komitmen Induk:** Blok Nol tidak membutuhkan Commit Certificate karena keabsahannya diverifikasi langsung terhadap konfigurasi spesifikasi genesis yang dikompilasi ke dalam kode simpul.

---

## 6. Prosedur Perhitungan Genesis Hash Kanonikal

Setiap implementasi simpul Aurion wajib mengeksekusi urutan perhitungan hash berikut saat inisialisasi database awal:

### Langkah 1: Konstruksi State Awal ($\sigma_0$)
1. Bentuk Sparse Merkle Tree (SMT) 256-bit berbasis Blake3.
2. Masukkan akun Creator Vault pada kunci `DeriveKey(CreatorAddress)` dengan saldo $1.980.000.000.000.000\ Q$ dan nonce $0$.
3. Masukkan akun Developer Vault pada kunci `DeriveKey(DeveloperAddress)` dengan saldo $330.000.000.000.000\ Q$ dan nonce $0$.
4. Masukkan entri keempat validator $\mathcal{V}_0$ ke dalam sub-pohon validator.
5. Masukkan state moneter awal $\mathcal{M}$ ($S_{\text{emitted}} = 2.310.000.000.000.000\ Q$).
6. Dapatkan root hash pohon:
   $$\text{StateRoot}_0 = \text{SMT\_Root}(\sigma_0)$$

### Langkah 2: Perakitan Biner Header Blok Nol
Serialisasikan ke-7 field header Blok Nol menjadi array tepat 124 bytes mengikuti aturan **[06 — SERIALIZATION & WIRE PROTOCOL](file:///c:/Projects/aurion/AURION-SERIALIZATION-AND-WIRE-PROTOCOL.md)**:

$$\text{HeaderBytes}_0 = \text{version} \parallel \text{height} \parallel \text{round} \parallel \text{timestamp} \parallel \mathbf{0}_{32} \parallel \mathbf{0}_{32} \parallel \text{StateRoot}_0$$

### Langkah 3: Komputasi Genesis Hash
Terapkan fungsi hash Blake3 dengan Domain Separation Tag resmi:

$$\mathbf{GenesisHash} = \text{Blake3}\Big( \text{"AURION-BLOCK-ID-V1"} \mathbin{\Vert} \text{HeaderBytes}_0 \Big)$$

---

## 7. Rangkuman Konstanta Resmi Genesis Mainnet

Untuk menjamin ketiadaan divergensi, konstanta resmi Genesis Mainnet Aurion dikunci sebagai berikut:

```rust
// ==============================================================================
// AURION MAINNET GENESIS CONSTANTS
// ==============================================================================

/// Protocol Version
pub const GENESIS_PROTOCOL_VERSION: u32 = 1;

/// Network Chain ID (Mainnet = 1001)
pub const GENESIS_CHAIN_ID: u32 = 1001;

/// Genesis Unix Timestamp (Contoh Epoch Resmi Peluncuran)
pub const GENESIS_TIMESTAMP_MAINNET: u64 = 1773570000;

/// Total Suplai Terbit pada Blok 0 (35% = 2.310.000.000.000.000 Q)
pub const GENESIS_INITIAL_EMITTED_QUANTA: u128 = 2_310_000_000_000_000;

/// Cadangan Mining Komunitas Murni (65% = 4.290.000.000.000.000 Q)
pub const GENESIS_COMMUNITY_RESERVE_QUANTA: u128 = 4_290_000_000_000_000;

/// Batas Suplai Tertinggi Permanen (100% = 6.600.000.000.000.000 Q)
pub const GENESIS_HARD_CAP_QUANTA: u128 = 6_600_000_000_000_000;

/// Jumlah Validator Pemula
pub const GENESIS_VALIDATOR_COUNT: usize = 4;

/// Total Voting Power Awal
pub const GENESIS_TOTAL_VOTING_POWER: u64 = 1_000_000;

/// Kuorum Konsensus Awal (2/3 + 1)
pub const GENESIS_QUORUM_THRESHOLD: u64 = 666_667;
```

---

## 8. Surat Ratifikasi Konstitusional Genesis

Spesifikasi Genesis ini mengikat seluruh node dan developer Aurion. Blok 0 adalah jangkar identitas permanen ekosistem. Tidak ada entitas, voting tata kelola, maupun *hard fork* yang diizinkan mengubah peristiwa masa lalu yang terkunci di dalam Genesis State dan Genesis Hash.
