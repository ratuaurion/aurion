# Laporan Audit Kemurnian Konsensus BFT (AUD-BFT-001)

> **Tanggal:** 2026-09-26
> **Ruang lingkup:** Lapisan produksi `src/`, artefak genesis, dokumen konstitusi
> **Status:** Perbaikan mekanis selesai. Tiga temuan terbuka menunggu keputusan pemilik.

---

## 1. Ringkasan Eksekutif

Aurion adalah protokol **BFT deterministik dengan single-slot finality**. Konstitusi
Pasal 2 secara eksplisit melarang konsep penambangan (*mining*), kalkulasi tingkat
kesulitan (*difficulty*), dan kompetisi hash untuk memproduksi blok.

Audit menemukan bahwa **terminologi dan skema fee model *mining* bocor ke lapisan
produksi**, bukan hanya ke dokumen. Yang paling serius: skema tersebut
**bertentangan dengan implementasi aktual**, sehingga bagian validasi repo ini
sempat menguji sesuatu yang sudah dihapus dari kanonik.

---

## 2. Temuan dan Status

| # | Temuan | Severity | Status |
| :--- | :--- | :--- | :--- |
| 1 | Skema fee 20% burn / 80% miner bertentangan dengan `MonetaryState::split_fee` | **Kritis** | ✅ Diperbaiki |
| 2 | Field `miner_fee` duplikat pada receipt STF | Tinggi | ✅ Diperbaiki |
| 3 | Field `fee_miner_quanta` duplikat pada receipt mempool | Sedang | ✅ Diperbaiki |
| 4 | Variabel produksi blok bernama `miner` | Sedang | ✅ Diperbaiki |
| 5 | Kunci seremoni dapat direkonstruksi dari seed konstan | **Kritis** | ⛔ **Terbuka** |
| 6 | Tiga angka alokasi Treasury tidak konsisten | **Kritis** | ⛔ **Terbuka** |
| 7 | Dua salinan dokumen Konstitusi berbeda isi | Tinggi | ⛔ **Terbuka** |

---

## 3. Temuan 1 — Skema Fee yang Bertentangan dengan Kanonik

`FEE_BURN_PERCENTAGE = 0` dan `FEE_VALIDATOR_PERCENTAGE = 100` sudah benar di
`src/primitives/core/quantum.rs`, dan `MonetaryState::split_fee` sudah
menerapkannya. Namun tiga lokasi menghitung ulang skema sendiri dengan angka
**lama**:

```rust
// SEBELUM — src/platform/audit/runner.rs
let burn = (total_fee * 20) / 100;      // ← 20% burn, sudah dihapus
let conserved = (burn + miner) == total_fee && burn == 20 && miner == 80;
```

```rust
// SEBELUM — src/platform/wallet/signing.rs
pub fn fee_split(&self) -> (Quantum, Quantum) {
    let burn_raw = fee_raw * 20 / 100;   // ← 20% burn, sudah dihapus
    let miner_raw = fee_raw - burn_raw;
    (Quantum::new(burn_raw), Quantum::new(miner_raw))
}
```

Prompt clear-signing yang dilihat pengguna juga menampilkan:

```text
  ├─ Permanent Burn (20%):   20.000 Quantum
  └─ Miner Reward   (80%):   80.000 Quantum
```

**Dampak:** user melihat rincian fee yang salah, dan audit runner memvalidasi
skema yang sudah tidak berlaku. Golden vector (`conformance/vectors.rs`) yang
dikonsumsi SDK lintas bahasa juga mengekspor `fee_miner_percent: 80`.

**Perbaikan:** semua lokasi kini memakai `MonetaryState::split_fee` atau
diturunkan langsung dari konstanta `FEE_*_PERCENTAGE`, sehingga tidak ada lagi

---

## 4. Temuan 5 — Kunci Seremoni Dapat Direkonstruksi (TERBUKA)

`src/primitives/genesis/ceremony.rs`:

```rust
pub fn new_deterministic() -> Self {
    let creator = Keypair::from_seed(&[0x01; 32]);   // ← Master Treasury
    let developer = Keypair::from_seed(&[0x02; 32]);
    let validators = vec![
        Keypair::from_seed(&[0x11; 32]),
        Keypair::from_seed(&[0x12; 32]),
        Keypair::from_seed(&[0x13; 32]),
        Keypair::from_seed(&[0x14; 32]),
    ];
```

Verifikasi melalui penghitungan address:

| Sumber | Address Creator / Treasury |
| :--- | :--- |
| `Keypair::from_seed(&[0x01; 32])` | `cb095697ccc5acbf23e176ee4f05e7e77ddfe54236bfe8c8b42e78c3d5a95aad` |
| `GENESIS_CEREMONY.json` → `creator_address_hex` | `cb095697ccc5acbf23e176ee4f05e7e77ddfe54236bfe8c8b42e78c3d5a95aad` |

**Keduanya identik.** Private key Master Treasury dapat dihitung siapa pun yang
memiliki salinan source.

**Kenapa ini penting:** selama blockchain hanya berjalan di localhost, tidak ada
pihak ketiga yang dirugikan. Tetapi bila genesis yang sama dipakai saat
go-public, siapa pun bisa menjalankan `Keypair::from_seed(&[0x01; 32])` dan
menandatangani transaksi atas nama Master Treasury.

**Status: menunggu keputusan pemilik.** Perbaikan memerlukan mnemonic Ceremony
milik pemilik, yang tidak boleh ada di repositori publik.

---

## 5. Temuan 6 — Tiga Angka Alokasi Treasury (TERBUKA)

| Sumber | Angka |
| :--- | :--- |
| `AURION CONSTITUTION.md` | 66.000.000 AUR |
| `GENESIS_CEREMONY.json` → `initial_supply_aur` | 23.100.000 AUR |
| `GENESIS_CEREMONY.json` → `creator_allocation_aur` | 19.800.000 AUR |
| `conformance/vectors.rs` → `genesis_allocation_quanta` | 2.310.000.000.000.000 Q |

Governance Specification §1 melarang tata kelola mengubah alokasi Blok 0. Karena
itu, angka yang benar harus ditetapkan **sebelum mainnet**, saat belum ada blok
yang mengikat siapa pun.

---

## 6. Gerbang Anti-Regresi

Perbaikan di atas tidak cukup bila tidak dapat diuji ulang. Aturan ini kini
dijalankan otomatis:

```bash
cargo test --offline --test bft_purity_gate
```

| Test | Yang dijaga |
| :--- | :--- |
| `production_source_has_no_mining_terminology` | Nol istilah `miner`/`mining` di `src/` |
| `no_hardcoded_legacy_fee_split_in_production` | Skema 20/80 tidak bisa dieksekusi |
| `canonical_fee_constants_match_constitution` | `FEE_BURN_PERCENTAGE = 0`, `FEE_VALIDATOR_PERCENTAGE = 100` |
| `split_fee_matches_canonical_constants` | Nilai STF sama dengan konstanta kanonik |
| `no_proof_of_work_concepts_in_consensus` | Nol pola kompetisi hash di `src/` |

Jika istilah mining atau skema lama kembali masuk ke lapisan produksi, `cargo test`
gagal dan menyebutkan **file serta nomor baris**.

### Cara memverifikasi manual (tanpa agent)

```bash
# 1. Terminologi miner harus NOL di produksi
grep -rn "miner" src/

# 2. Skema fee lama harus NOL
grep -rn "20% burn\|80% miner\|fee_miner" src/

# 3. Kanonik fee
grep -rn "FEE_BURN_PERCENTAGE\|FEE_VALIDATOR_PERCENTAGE" src/primitives/core/quantum.rs
# → harus 0 dan 100

# 4. Konsep PoW harus NOL
grep -rn "difficulty_target\|pow_hash\|mining_reward" src/
```

---

## 7. Rujukan

- `README.md` §4.1 — Gerbang Kemurnian BFT
- `docs/Constitutions/AURION CONSTITUTION.md` Pasal 2 — Penolakan Total Proof-of-Work
- `docs/Constitutions/AURION-GOVERNANCE-SPECIFICATION.md` — larangan perubahan alokasi genesis

---

## 8. Catatan Topologi Ekosistem (DOKUMENTASI SAJA — BUKAN TINDAKAN)

### 8.1 Apa yang diverifikasi

Audit tidak menghapus berkas apa pun. Pemeriksaan terhadap direktori
sekitar `C:\Projects` menunjukkan adanya **delapan repositori terpisah**
di luar repo `aurion`:

| Direktori | Git | Manifest | Status |
| :--- | :--- | :--- | :--- |
| `aurion` | ya | `Cargo.toml` | Repo inti — **Single Binary `/bin/aurion`** |
| `aur-wallet` | ya | `package.json` | Backend Aurion (dompet) |
| `aurion-bootnode` | ya | `Cargo.toml` | Backend Aurion (bootnode) |
| `aurion-explorer` | ya | `package.json` | Backend Aurion (explorer web) |
| `aurion-faucet` | ya | `Cargo.toml` | Backend Aurion (layanan faucet) |
| `aurion-market` | ya | `Cargo.toml` | Backend Aurion (pasar) |
| `aurion-web` | ya | `package.json` | Backend Aurion (web) |
| `aurion-test` | tidak | – | Harness uji |
| `aurion-validator` | tidak | – | Harness validator |
| `nebula` | tidak | `go.mod` | **Proyek terpisah — bukan bagian Aurion** |

> **Pernyataan pemilik proyek:** kedelapan direktori tersebut adalah backend
> Aurion; `nebula` adalah proyek terpisah dan berada di luar lingkup Aurion.

### 8.2 Mengapa ini dicatat

AUR-ARCH-001 menyatakan Aurion didistribusikan sebagai **satu binary utama**
(`/bin/aurion`). Repo `aurion` sendiri sudah memenuhi hal ini: hanya ada satu
`Cargo.toml`, dan `docker-compose.yml` menjalankan `./bin/aurion` sebagai image
yang sama (`aurion:latest`) untuk validator, sentry, dan gateway.

Namun, beberapa backend Aurion (bootnode, faucet, market, explorer,
wallet) berada di repo terpisah. Hal ini **tidak otomatis melanggar
AUR-ARCH-001**, karena invarian tersebut mengatur *distribusi binary konsensus*,
bukan jumlah repositori. Namun observasi ini perlu dicatat karena:

1. **`aurion-faucet` (Rust) berada di repo terpisah** sementara `faucet.rs`
   kini sudah terintegrasi ke dalam `/bin/aurion` (lihat commit `49ca833`).
   Ada risiko dua implementasi faucet yang tidak sinkron.
2. **`aurion-explorer` (Next.js) adalah dashboard terpisah**, sementara
   Explorer native `/explorer` sudah tertanam di dalam binary. Keduanya harus
   memakai kontrak API yang sama.
3. Aturan fee dan terminologi BFT yang baru dikunci oleh `bft_purity_gate`
   hanya berlaku pada repo `aurion`. Repo lain **tidak** tercakup gerbang ini.

### 8.3 Batas gerbang anti-regresi

```bash
cargo test --offline --test bft_purity_gate
```

hanya memindai `aurion/src/`. Untuk repositori Aurion lain, aturan yang sama
belum diterapkan. Ini **bukan** kelalaian —-setiap repo memiliki siklus rilis dan
karakteristik sendiri — namun dicatat agar tidak disalahpahami sebagai
"seluruh ekosistem Aurion sudah murni BFT".

### 8.4 Tindakan yang DIAMBIL

Pemilik proyek memutuskan untuk menghapus berkas berikut setelah verifikasi
nol referensi di seluruh `src/`, `tests/`, `tools/`, `benches/`, dan `examples/`:

| Berkas | Alasan |
| :--- | :--- |
| `CONFORMANCE_MATRIX.json` | Check-in duplikat, nol pembaca kode |
| `MAINNET_CONFIG.toml` | Digantikan flag CLI runtime |
| `MAINNET_DASHBOARD.json` | Template Grafana; langkah impor ditulis ulang |
| `RELEASE_CANDIDATE_rc1.json` | Kandidat rilis usang (`v1.0.0-rc1`) |
| `SBOM_rc1.json` | SBOM kandidat usang |

Rujukan dokumentasi yang menunjuk ke berkas tersebut ikut diperbarui agar
tidak ada tautan rusak:

- `docs/operations/POST_MAINNET_OPERATIONS_GUIDE.md` — langkah impor
  dashboard Grafana ditulis ulang agar operator mendefinisikan panel sendiri
  dari `METRICS_SPECIFICATION.md`; rujukan `MAINNET_CONFIG.toml` diganti
  menjadi flag CLI (`--data-dir` / `--identity-key`).
- `docs/operations/RELEASE_CANDIDATE_GUIDE.md` — rujukan file atestasi
  diganti menjadi catatan rilis resmi.

> **Peringatan (tetap berlaku):** `GENESIS_CEREMONY.json` dan
> `MAINNET_GENESIS_BLOCK.json` **HARUS TETAP ADA**. Keduanya di-embed lewat
> `include_str!` pada `src/primitives/genesis/ceremony.rs`. Menghapusnya akan
> membuat build gagal.

- `.internal-tasks/TASK_REGISTER.md` — entri AUD-BFT-001
