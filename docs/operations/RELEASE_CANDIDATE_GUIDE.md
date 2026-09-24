# AURION — Mainnet Release Candidate Freeze Runbook (v1.0.0-rc1)
> **Status:** KANDIDAT RILIS RESMI TERKUNCI (PRD-014 SELESAI)  
> **Versi Biner:** `aurion v1.0.0-rc1`  
> **Klasifikasi:** Panduan Operasional Verifikasi Reproduksibilitas & Pembekuan Konsensus  
> **Prinsip Tertinggi:** Immutability of Monetary Policy (`AUR-ARCH-006`) | Zero Unsafe (`AUR-ARCH-011`) | Zero Float (`AUR-ARCH-012`)

---

## 1. Ringkasan Eksekutif Pembekuan Kandidat Rilis

Dokumen ini merupakan runbook resmi pembekuan kandidat rilis mainnet (*Mainnet Release Candidate Freeze*) untuk versi **`aurion v1.0.0-rc1`**.

Tahap pembekuan ini menandai bahwa seluruh logika konsensus, mesin virtual kontrak pintar (AVM), state machine, kebijakan moneter, dan transport jaringan telah **terkunci permanen**. Tidak ada perubahan logika konsensus yang diizinkan setelah tonggak pencapaian ini.

---

## 2. Parameter Pembekuan Konsensus & Kebijakan Moneter (`AUR-ARCH-006`)

| Parameter Protokol | Nilai Terkunci | Keterangan & Invariant |
| :--- | :--- | :--- |
| **Pasokan Dasar Genesis (Blok 0)** | `66.000.000 AUR` ($6.6 \times 10^{16}$ Quantum) | 100% dialokasikan ke Master Treasury Account |
| **Presisi Moneter** | `1 AUR = 1.000.000.000 Quantum (10^9)` | Zero floating point `Quantum(u128)` |
| **Emisi Blok Berkelanjutan (H > 0)**| `1 AUR` ($10^9$ Quantum per blok) | 20% Proposer, 80% Precommit Voters QC |
| **Alokasi Fee Transaksi** | `100% ke Validator Proposer` | Melengkapi insentif perakitan blok |
| **Topologi Bootnode Resmi** | `116.212.72.89` (TCP Port 7447) | Nginx SSL Gateway `bootnode.ratuaurion.store:8080` |
| **Waktu Slot Target BFT** | `60.000 ms` (60 detik) | `AUR-APP-05` |
| **Ambang Kuorum Finalitas** | `> 2/3 Total Bobot Voting Validator` | Single-slot BFT guarantee |
| **Doktrin Integritas Kode** | Zero-Mock Policy | Larangan mutlak `--dev` & mock consensus |

---

## 3. Atestasi Kriptografis Biner Rilis

Biner rilis dikompilasi secara deterministik dengan optimasi produksi maksimal:

* **Target Biner:** `target/release/aurion.exe` (atau `/bin/aurion` pada Linux)
* **Ukuran File:** `1.955.328 bytes` (~1,86 MB)
* **SHA-256 Checksum:**
  ```
  17d6a47eb37289b5320f23afa7097242ff4adbcbfdc63cab7540d00ff3a07666
  ```
* **Kompiler Toolchain Pinned:**
  * `rustc 1.98.1 (48a229cea 2026-09-01)`
  * `cargo 1.98.1 (797e8a9bc 2026-08-05)`
* **Git Commit Ref:** `4605f7ca33c1bb06e229dd8b5d8cf975b49ab66e`
* **Flag Kompilasi `[profile.release]`:**
  * `opt-level = 3`
  * `lto = "fat"`
  * `codegen-units = 1`
  * `panic = "abort"`
  * `strip = "symbols"`
  * `overflow-checks = true`

---

## 4. Langkah Reproduksi & Verifikasi Mandiri oleh Operator

Operator simpul publik, validator, dan auditor independen dapat mereproduksi biner yang identik dengan langkah-langkah berikut:

### 4.1 Clone & Checkout Commit Freeze
```bash
git clone https://github.com/ratuaurion/aurion.git
cd aurion
git checkout main
```

### 4.2 Kompilasi Biner Produksi
```bash
cargo build --release --bin aurion
```

### 4.3 Verifikasi Checksum SHA-256
Di Windows PowerShell:
```powershell
Get-FileHash -Algorithm SHA256 target\release\aurion.exe
```
Di Linux / macOS:
```bash
sha256sum target/release/aurion
```
Bandingkan hash yang dihasilkan dengan file atestasi [`RELEASE_CANDIDATE_rc1.json`](file:///c:/Projects/aurion/RELEASE_CANDIDATE_rc1.json).

### 4.4 Verifikasi Identitas Biner & Invariant Kepatuhan
```bash
./target/release/aurion version --output json
```
Output yang diharapkan:
```json
{
  "application": "aurion",
  "version": "1.0.0-rc1",
  "architecture": "Single Sovereign Primary Binary (/bin/aurion)",
  "hard_cap_aur": 66000000,
  "quantum_scale": "10^9 (1 AUR = 1,000,000,000 Quanta)",
  "consensus": "Single-Slot BFT Finality (>2/3 Quorum)",
  "hashing": "Blake3 256-bit",
  "signatures": "Ed25519 (Strict Anti-Malleability)"
}
```

### 4.5 Menjalankan Audit Kesiapan Produksi Otomatis
```bash
./target/release/aurion audit run
```
Seluruh 10/10 pengujian keamanan audit harus berstatus `[PASSED]` dan menghasilkan vonis:
`AURION MAINNET PRODUCTION READY (PASS)`.

---

## 5. Gerbang Transisi Menuju Mainnet

Dengan selesainya **`PRD-014: Mainnet Release Candidate Freeze`**:
* **Gerbang Rilis Terkunci:** Versi `v1.0.0-rc1` menjadi basis kode kanonikal mutlak.
* **Langkah Selanjutnya:** Melanjutkan ke **Langkah 15: Deterministic Genesis Ceremony (`PRD-015`)** untuk pembentukan blok genesis $H=0$, penandatanganan attestation hash multi-pihak, dan penguncian state awal database `redb`.
