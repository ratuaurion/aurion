# 08 — AURION EXPLORER & INDEXER APPLICATION RULES
## Standar Pengindeksan Rantai Kanonikal, Rekonsiliasi Reorganisasi, dan Integritas Penyajian Data Publik

> **Hierarki Dokumen:**  
> `AURION CONSTITUTIONS` $\longrightarrow$ `PROTOCOL SPECIFICATIONS` $\longrightarrow$ `00-APPLICATION-RULES` $\longrightarrow$ **`08-EXPLORER-INDEXER-RULES`**  
>
> **Status:** RATIFIED & LOCKED APPLICATION SPECIFICATION  
> **Klasifikasi:** Standar Pengindeksan & Penjelajah Blok (Indexer & Explorer Standard)  
> **Versi:** 1.0.0-PROD  
> **Sifat Ketetapan:** Normatif Wajib (RFC 2119 / RFC 8174), Zero-Fabrication, Idempoten

---

## 1. Posisi Arsitektur Penjelajah Blok dan Pengindeks

Block Explorer dan Data Indexer beroperasi di luar mesin konsensus inti. Tugas utamanya adalah membaca log biner rantai, merekonstruksi grafik relasional, menyusun indeks pencarian cepat, dan menyajikan visualisasi data kepada publik:

```text
┌───────────────────────────┐
│     AURION FULL NODE      │
│  (State Root, Blocks, CC) │
└─────────────┬─────────────┘
              │ WebSocket Stream / Batch RPC
              ▼
┌───────────────────────────┐
│     DATA INDEXER ENGINE   │
│  1. Ingestion Pipeline    │ ── Verifikasi Commit Certificate
│  2. Reorg Reconciliation  │ ── Penanganan Putaran BFT
│  3. Relational Projection │ ── Akun, Tx, RPI, Suplai
└─────────────┬─────────────┘
              │ SQL / Document DB
              ▼
┌───────────────────────────┐
│      PUBLIC EXPLORER      │
│  (UI Web, Search, Charts) │
└───────────────────────────┘
```

> **HUKUM DASAR PENGINDEKSAN (THE ZERO-FABRICATION MANDATE):**  
> Sebuah Indexer atau Explorer **MUST NOT** memalsukan, mengarang, atau mengekstrapolasi fakta on-chain yang tidak berakar secara matematis dari blok kanonikal Aurion. Explorer adalah cermin kebenaran konsensus, bukan pembuat kebijakan.

---

## 2. Strategi Pengindeksan Dua Jalur (Dual-Track Ingestion Architecture)

Untuk menjaga akurasi mutlak sekaligus memberikan pembaruan instan kepada pengguna:

### 2.1 Jalur 1: State Kanonikal Final (Finalized Canonical Store)
1. Indexer **MUST** memproses seluruh data transaksi dan mutasi saldo yang telah memiliki **Commit Certificate resmi ($\mathcal{CC}(B)$)** ke dalam tabel penyimpanan permanen.
2. Data pada jalur ini bersifat imutabel (*append-only*) dan tidak akan pernah mengalami penghapusan (*rollback*).

### 2.2 Jalur 2: State Spekulatif Mempool & Blok Kandidat (Speculative Store)
1. Indexer **MAY** mengindeks transaksi yang masih mengantre di mempool atau blok kandidat yang belum mengantongi Commit Certificate.
2. Seluruh entitas pada jalur ini **MUST** ditandai dengan flag eksplisit:
   ```json
   "is_finalized": false,
   "settlement_status": "SPECULATIVE"
   ```
3. Jika blok kandidat dibatalkan karena pergantian putaran konsensus (*round change*), indexer **MUST** menghapus (*purge*) data spekulatif tersebut dari antarmuka explorer dalam waktu $< 1\ \text{detik}$.

---

## 3. Penanganan dan Rekonsiliasi Reorganisasi (Reorg Handling)

Meskipun blok ber-CC kebal terhadap reorganisasi, simpul yang mengamati ujung rantai (*chain tip*) dapat mengalami pergantian blok kandidat:

1. **Pemeriksaan Hash Induk Bertingkat:**  
   Indexer **MUST** memverifikasi bahwa `prev_block_hash` dari blok baru $H$ cocok persis dengan `block_hash` dari blok $H-1$ yang tersimpan di database lokal.
2. **Prosedur Rollback Bersih (Atomic Rewind):**  
   Jika terjadi ketidakcocokan hash pada blok non-final:
   - Sistem **MUST** mengeksekusi *database transaction rollback* hingga ke blok final terakhir yang terbukti sah;
   - Memasukkan kembali transaksi yang valid ke dalam tabel antrean spekulatif;
   - Menghitung ulang saldo akun yang terpengaruh.

---

## 4. Standar Penyajian Data Finansial pada Penjelajah Blok (UI Presentation)

Explorer publik **MUST** menyajikan rincian data dengan standar transparansi tinggi:

### 4.1 Rincian Biaya Transaksi (Fee Breakdown)
Setiap halaman detail transaksi **MUST** memecah nilai fee secara eksplisit:
- **Total Biaya Transaksi (Total Fee):** Nilai penuh dalam Quantum dan AUR ($1\text{ AUR} = 10^9\text{ Quantum}$).
- **Alokasi Validator Pembuat Blok (Validator 100%):** Ditampilkan sebagai imbalan penuh kepada validator proposer perakit blok.
- **Biaya yang Dimusnahkan (Burned 0%):** Menampilkan nilai `0` (skema burn fee telah dihapuskan; suplai deterministik diatur di level Genesis 66M dan subsidi blok BFT).

### 4.2 Pelacakan Asal-Usul Penerbitan Moneter (Provenance Tracking)
1. Explorer **MUST** membedakan transaksi transfer biasa dengan pencetakan subsidi blok BFT baru (*Protocol Block Reward* $R = 1\text{ AUR}$ per blok).
2. Setiap hadiah blok baru **MUST** menampilkan rincian pembagian protokol resmi (20% Proposer, 80% Precommit Voters QC) yang mengikat subsidi ke [Konstitusi Protokol Aurion](file:///c:/Projects/aurion/CONSTITUTION.md) Bab II Pasal 3.
3. Explorer **MUST** menyajikan grafik pasokan moneter real-time:
   - Total Suplai Genesis ($66.000.000\text{ AUR}$ ke Master Treasury)
   - Total Emisi Hadiah Blok Kumulatif ($S_{\text{rewards}} = H \times 1\text{ AUR}$)
   - Total Suplai Beredar Aktif ($S_{\text{circulating}}$)
   - Total Slashed / Slashing Burn ($S_{\text{slashed}}$)

---

## 5. Standar Penomoran Halaman dan Batas Beban API (Pagination & Quotas)

1. **Paginasi Berbasis Kursor (Cursor-Based Pagination):**  
   Endpoint API explorer yang menyajikan riwayat transaksi akun atau daftar blok **SHOULD** menggunakan paginasi berbasis kursor atau kombinasi `(block_height, tx_index)` untuk mencegah penurunan performa query database besar.
2. **Batas Maksimum Elemen per Halaman:**  
   Ukuran halaman (`limit`) **MUST NOT** melebihi $100\ \text{elemen}$ per permintaan pada API publik tanpa autentikasi, dan maksimal $500\ \text{elemen}$ untuk klien terotentikasi.
3. **Idempotensi Pengindeksan:**  
   Eksekusi ulang proses indexing terhadap blok yang sama **MUST** menghasilkan state database yang identik tanpa memicu duplikasi data mutasi saldo (*Idempotent Ingestion*).
