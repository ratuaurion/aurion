# Panduan Operasional Peluncuran Aurion Mainnet (PRD-016)

> **Dokumen Resmi:** Panduan Peluncuran dan Pengoperasian Rantai Blok Produksi Aurion  
> **Status:** KANONIKAL & TERSEGEL (Mainnet Production Ready)  
> **Versi Rilis:** `v1.0.0-rc1` / `v1.0.0` (Single Sovereign Executable `/bin/aurion`)  
> **Target Audiens:** Genesis Validators ($\mathcal{V}_0$), Operator Sentry, Bursa Kripto (Exchanges), Pengembang Dompet, dan Operator Simpul Publik.

---

## 1. Identitas Inti & Parameter Kanonikal Mainnet

Seluruh peserta jaringan produksi Aurion terikat pada invariant konstitusional berikut:

| Parameter | Nilai Resmi Mainnet | Keterangan |
| :--- | :--- | :--- |
| **Network Name** | `aurion-mainnet` | Jaringan produksi berdaulat utama |
| **Chain ID** | `1001` | Sesuai `GENESIS_CHAIN_ID` |
| **Genesis Timestamp** | `1773532800` | 15 Maret 2026, 00:00:00 UTC |
| **Genesis Block Hash ($H=0$)** | `d82f72ac1be185911bd803987660e624c0ed1c12d4a189b147de9c5b7f5635f9` | Blake3 digest atas header blok nol |
| **Initial State Root ($\sigma_0$)** | `61e647706990a010ba95f781d506620cf69b2dc57a7b1b53ca96f0f1d07bb850` | SMT Blake3 256-bit state root |
| **Ceremony Transcript Hash** | `f88d06b37766746bcb9c9985c35ebf6448c441a578a576ee51bd7efbc64e3380` | Checksum transkrip upacara multi-pihak |
| **Hard Cap Total Supply** | `66,000,000` AUR | $6.6 \times 10^{15}$ Quantum ($u128$) |
| **Alokasi Genesis (35%)** | `23,100,000` AUR | 30% Creator (19.8M) + 5% Dev (3.3M) |
| **Konsensus Finalitas** | Single-Slot BFT Finality | Kuorum $>2/3$ voting power ($\ge 666,667$) |
| **Mesin Penyimpanan** | `redb 4.3` | Murni Rust ACID, zero C++ runtime |
| **Protokol P2P Wire** | Magic `AUR0`, 52-byte Header | Blake3 payload checksum, port default 9000 |

---

## 2. Verifikasi Mandiri Artefak Genesis

Sebelum menyalakan simpul, operator wajib memverifikasi integritas dokumen transkrip seremoni [`GENESIS_CEREMONY.json`](./GENESIS_CEREMONY.json) dan blok genesis [`MAINNET_GENESIS_BLOCK.json`](./MAINNET_GENESIS_BLOCK.json):

```powershell
# 1. Verifikasi transkrip seremoni kriptografis
aurion genesis ceremony verify --file GENESIS_CEREMONY.json --output json

# 2. Inspeksi parameter jaringan mainnet
aurion network status --output json

# 3. Periksa daftar bootnode kanonikal
aurion network peers --output json
```

Ekspektasi keluaran:
```json
{
  "overall_status": "VERIFIED_CANONICAL",
  "ceremony_hash": "f88d06b37766746bcb9c9985c35ebf6448c441a578a576ee51bd7efbc64e3380",
  "genesis_block_hash": "d82f72ac1be185911bd803987660e624c0ed1c12d4a189b147de9c5b7f5635f9",
  "state_root": "61e647706990a010ba95f781d506620cf69b2dc57a7b1b53ca96f0f1d07bb850",
  "quorum_status": "PASSED (1000000/1000000 >= 666667)",
  "monetary_audit_status": "PASSED (100% Invariant Compliant: 35% Genesis, Zero-Float)"
}
```

---

## 3. Topologi Simpul Produksi Mainnet

Arsitektur produksi menerapkan **Topologi Tiga Lapis Berdaulat** sesuai Dokumen Operasional 12:

```
[ Internet / Komunitas / Bursa / RPC Klien ]
                       │
                       ▼
         ┌───────────────────────────┐
         │     Public RPC Gateway    │  (Port 8545: HTTP & WebSocket)
         └─────────────┬─────────────┘
                       │
                       ▼
         ┌───────────────────────────┐
         │     Sentry Node Mesh      │  (Port 9000: DDoS Filtering, Rate Limiting)
         └─────────────┬─────────────┘
                       │  (Private LAN / WireGuard / Isolated VPC)
                       ▼
         ┌───────────────────────────┐
         │  BFT Validator Engine V0  │  (Zero Public Inbound, Isolated Signing)
         └───────────────────────────┘
```

---

## 4. Runbook Pengoperasian Simpul

### 4.1 Menjalankan Genesis Validator Node ($\mathcal{V}_0$)
Genesis validator mengikat kunci konsensus Ed25519 dan hanya menerima koneksi privat dari Sentry nodes:

```powershell
# Jalankan Genesis Validator 1 (Alpha)
aurion validator start --index 0 --data-dir data/val1.redb --rpc-bind 127.0.0.1:8545

# Jalankan Genesis Validator 2 (Beta)
aurion validator start --index 1 --data-dir data/val2.redb --rpc-bind 127.0.0.1:8546

# Jalankan Genesis Validator 3 (Gamma)
aurion validator start --index 2 --data-dir data/val3.redb --rpc-bind 127.0.0.1:8547

# Jalankan Genesis Validator 4 (Delta)
aurion validator start --index 3 --data-dir data/val4.redb --rpc-bind 127.0.0.1:8548
```

### 4.2 Menjalankan Sentry Node (DDoS Protection Proxy)
```powershell
# Jalankan Sentry Node di depan Validator Alpha
aurion node start --network mainnet --data-dir data/sentry.redb --rpc-bind 127.0.0.1:8545
```

### 4.3 Menjalankan Full Node Publik & Gateway JSON-RPC untuk Bursa
```powershell
aurion node start --network mainnet --data-dir data/fullnode.redb --rpc-bind 0.0.0.0:8545
```

---

## 5. Transisi Konsensus: Blok 0 (Genesis) $\to$ Blok 1 (Production)

Saat 4 simpul validator $\mathcal{V}_0$ saling terhubung melalui protokol Zenoh / Wire `AUR0`:
1. Proposer Blok 1 mengumpulkan transaksi yang valid dari mempool.
2. Membentuk proposal header Blok 1 dengan `prev_block_hash = d82f72ac1be185911bd803987660e624c0ed1c12d4a189b147de9c5b7f5635f9` (Genesis Hash).
3. Melakukan 2-phase BFT voting (`prevote` $\to$ `precommit`).
4. Setelah mengumpulkan $\ge 666,667$ bobot voting suara validator, dibentuk `CommitCertificate`.
5. Blok 1 dikomit secara atomik ke dalam penyimpanan fisik `redb 4.3`.
6. Subsidi blok perdana dikreditkan ke alamat proposer, dan 20% fee transaksi dibakar secara deflasioner.

---

## 6. Siklus Transaksi Perdana Mainnet

Untuk mengirimkan transaksi perdana di jaringan produksi Mainnet:
```powershell
# 1. Buat atau impor dompet berstandar BIP-39 / BIP-44
aurion wallet create --name treasury

# 2. Cek saldo akun Creator genesis
aurion account balance aur1... --output json

# 3. Kirim transaksi transfer berdaulat
aurion tx send --from creator --to aur1... --amount 100.00 --fee 0.01 --output json
```

---

## 7. Observabilitas & Metrik Pemantauan

Setiap simpul produksi menyediakan endpoint telemetri Prometheus di `http://127.0.0.1:9100/metrics`:
- `aurion_ledger_height`: Tinggi blok kanonikal saat ini.
- `aurion_finalized_height`: Tinggi blok yang telah difinalisasi secara BFT.
- `aurion_bft_round`: Putaran konsensus aktif.
- `aurion_mempool_size`: Jumlah transaksi antrean di mempool.
- `aurion_p2p_connected_peers`: Jumlah rekan P2P yang terhubung aktif.
- `aurion_storage_commit_latency_ms`: Latensi penulisan atomik redb.

---

## 8. Ringkasan Kesiapan Produksi (Go / No-Go Checklist)

- [x] **Zero Unsafe Code:** `#![forbid(unsafe_code)]` ditegakkan di seluruh modul.
- [x] **Zero Float Arithmetic:** 100% kuantitas moneter menggunakan exact integer `Quantum(u128)`.
- [x] **Monetary Cap Invariant:** Maksimum suplai terkunci mati pada 66,000,000 AUR.
- [x] **Genesis Quorum:** 4/4 genesis validators menandatangani atestasi resmi (1,000,000 / 1,000,000 weight).
- [x] **ACID Durability:** Database `redb 4.3` memulihkan state 100% identik pasca-crash.
- [x] **Binary Determinism:** Single sovereign executable `/bin/aurion` terverifikasi SHA-256 dan SBOM.
