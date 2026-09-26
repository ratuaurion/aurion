# ==============================================================================
# AURION PRIVATE MULTI-REGION TESTNET & SNAPSHOT UPGRADE GUIDE (NET-011)
# ==============================================================================
# Dokumen Panduan Operasional Jaringan Uji Coba Multi-Region & Fast-Sync
# Mengacu pada Invariant: AUR-ARCH-001, AUR-ARCH-005, AUR-CONS-*, AUR-APP-12
# ==============================================================================

## 1. Ringkasan Arsitektur Multi-Region (NET-011)

Langkah ke-11 (**`NET-011: Private Multi-Region Testnet`**) memperluas penggelaran devnet lokal menjadi jaringan privat terdistribusi lintas wilayah geografis global (*cross-continental*).

### Fitur Utama:
1. **Topologi Lintas Benua dengan Latensi WAN**:
   - Mensimulasikan dan menguji konsensus BFT di bawah latensi jaringan nyata (15 ms hingga 300 ms).
   - Memastikan finalitas *round-based* (< 1.000 ms SLA) tetap terpenuhi meskipun paket suara BFT menyeberangi samudra.
2. **Rotasi Validator Dinamis Berbasis Epoch (`EpochTransition`)**:
   - Pemisahan blok dalam unit *Epoch* (default 10 blok).
   - Mekanisme rotasi validator: penambahan validator baru atau pengeluaran validator yang tidak aktif secara deterministik dengan sertifikat komit kuorum $>2/3$.
3. **Mekanisme State Snapshot & Fast Sync (`StateSnapshot`)**:
   - Format arsip kanonikal biner berotentikasi (`.auss`).
   - Memungkinkan simpul baru bergabung dan menyinkronkan data langsung dari snapshot status terkini tanpa harus memutar ulang transaksi dari Genesis $H=0$.

---

## 2. Peta Geografis & Profil Latensi WAN

```
[ Region 3: North America ] ─────────────┐
(val-us-1, RTT ~220ms)                   │
                                         ▼
[ Region 2: Europe ]       ──────► [ BFT Consensus ] ◄────── [ Region 1: Asia-Pacific ]
(val-eu-1, RTT ~160ms)             [ Quorum > 2/3  ]         (val-ap-1, Proposer, RTT ~15ms)
                                         ▲
[ Region 4: South America ] ─────────────┘
(val-sa-1, RTT ~300ms)
```

| Region ID | Lokasi Geografis | Simpul Utama | Base WAN RTT | Margin SLA BFT (<1000ms) |
| :--- | :--- | :--- | :---: | :---: |
| **`ap-southeast`** | Jakarta / Singapore | `val-ap-1`, `sentry-ap` | 15 ms | **985 ms** (SLA Terpenuhi) |
| **`eu-central`** | Frankfurt / London | `val-eu-1`, `sentry-eu` | 160 ms | **840 ms** (SLA Terpenuhi) |
| **`us-east`** | North Virginia | `val-us-1` | 220 ms | **780 ms** (SLA Terpenuhi) |
| **`sa-east`** | São Paulo / Sydney | `val-sa-1` | 300 ms | **700 ms** (SLA Terpenuhi) |

---

## 3. Manajemen Epoch & Rotasi Validator

### Algoritma Perhitungan Epoch:
$$\text{EpochId}(H) = \left\lfloor \frac{H}{\text{EPOCH\_BLOCK\_INTERVAL}} \right\rfloor$$
Batas epoch terjadi ketika:
$$H \pmod{\text{EPOCH\_BLOCK\_INTERVAL}} = 0$$

### Struktur Sertifikat Transisi Epoch (`EpochTransition`):
- `from_epoch`: Indeks epoch saat ini.
- `to_epoch`: Indeks epoch berikutnya.
- `boundary_height`: Tinggi blok batas pergantian.
- `previous_validator_set`: Set validator lama yang menandatangani blok batas.
- `new_validator_set`: Set validator baru setelah rotasi.
- `transition_certificate`: Sertifikat BFT yang memuat kuorum tanda tangan Precommit $>2/3$.

---

## 4. Format State Snapshot Kanonikal (`.auss`)

File snapshot menyimpan keadaan akun dan riwayat state trie secara deterministik:

| Bagian | Field | Tipe | Deskripsi |
| :--- | :--- | :--- | :--- |
| **Header** | `magic` | `[u8; 4]` | Magic bytes `AUSS` (0x41, 0x55, 0x53, 0x53) |
| | `version` | `u32` | Versi format (v1 kanonikal) |
| | `chain_id` | `u32` | Identifier jaringan testnet |
| | `height` | `u64` | Tinggi blok snapshot dibuat |
| | `epoch` | `u64` | Indeks epoch snapshot |
| | `block_hash` | `[u8; 32]` | Hash blok kanonikal pada tinggi snapshot |
| | `state_root` | `[u8; 32]` | Akar Merkle SMT akun pada tinggi snapshot |
| **Body** | `accounts_count` | `u64` | Jumlah akun yang dicadangkan |
| | `accounts` | `[(Address, Account)]` | Daftar akun dan saldo yang diurutkan deterministik |
| **Footer** | `has_certificate`| `u8` | Flag keberadaan sertifikat komit (0 atau 1) |
| | `certificate` | `CommitCertificate` | Bukti komitmen konsensus BFT dari validator |

---

## 5. Panduan Operasional CLI & Tooling

### 5.1. Memeriksa Status Multi-Region & Matriks Latensi
```powershell
python tools/multi_region_testnet.py latency
aurion testnet status --output json
```

### 5.2. Mengekspor State Snapshot dari Simpul Berjalan
```powershell
aurion snapshot export --height 100 --output data/snapshot_h100.auss --data-dir data/testnet/val-ap-1
```

### 5.3. Menginspeksi & Memverifikasi Snapshot
```powershell
aurion snapshot inspect data/snapshot_h100.auss
aurion snapshot verify data/snapshot_h100.auss
```

### 5.4. Melakukan Fast-Sync pada Simpul Baru
Simpul baru dapat langsung menyerap database tanpa mengunduh blok dari awal:
```powershell
aurion node --data-dir data/testnet/val-new --snapshot data/snapshot_h100.auss
```

---

## 6. Verifikasi Suite Pengujian Terus-Menerus

Jalankan suite pengujian integrasi otomatis multi-region:
```powershell
cargo test --test multi_region_testnet
```
Suite ini memverifikasi secara end-to-end:
- Konsensus BFT 4 region geografis di bawah latensi WAN (15ms - 300ms).
- Rotasi validator antar-epoch dengan eliminasi simpul lama dan onboarding simpul baru.
- Ekspor snapshot ke disk, deserialisasi, dan verifikasi checksum Blake3.
- Pemulihan database simpul baru via snapshot fast-sync dengan State Root 100% identik.
