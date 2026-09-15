# 10 — AURION UNIFIED APPLICATION ERROR MODEL
## Taksonomi Galat Terpadu, Kode Kesalahan Machine-Readable, dan Panduan Penanganan Pengecualian

> **Hierarki Dokumen:**  
> `AURION CONSTITUTIONS` $\longrightarrow$ `PROTOCOL SPECIFICATIONS` $\longrightarrow$ `00-APPLICATION-RULES` $\longrightarrow$ **`10-ERROR-MODEL`**  
>
> **Status:** RATIFIED & LOCKED APPLICATION SPECIFICATION  
> **Klasifikasi:** Standar Penanganan Galat Terpadu (Unified Error Model Standard)  
> **Versi:** 1.0.0-PROD  
> **Sifat Ketetapan:** Normatif Wajib (RFC 2119 / RFC 8174), Machine-Readable, Zero-Ambiguity

---

## 1. Prinsip dan Desain Model Galat

Aplikasi luar tidak boleh menerima pesan galat generik yang kabur seperti *"Transaction Failed"* atau *"Internal Error"*. 

Setiap kegagalan di seluruh ekosistem Aurion **MUST** mengembalikan kode galat yang terdefinisi secara terstruktur, deterministik, dan dapat dipahami oleh mesin (*machine-readable*).

```text
┌────────────────────────────────────────────────────────────────────────┐
│               STRUKTUR OBJEK GALAT TERPADU (ERROR SCHEMA)              │
├──────────────┬──────────────┬──────────────────────────────────────────┤
│ FIELD        │ TIPE DATA    │ DESKRIPSI KONTEN                         │
├──────────────┼──────────────┼──────────────────────────────────────────┤
│ category     │ String       │ Kategori domain (PROTOCOL, TX, RPC, dsb) │
│ code         │ u32 (Integer)│ Kode numerik unik global                 │
│ name         │ String       │ Konstanta identifier resmi               │
│ message      │ String       │ Deskripsi ringkas ramah manusia          │
│ retryable    │ Boolean      │ Indikator apakah permintaan aman diulang │
│ details      │ Object (Map) │ Data kontekstual spesifik (nilai/ambang) │
└──────────────┴──────────────┴──────────────────────────────────────────┘
```

---

## 2. Katalog Domain dan Kode Galat Resmi

### 2.1 Domain Transaksi (`TRANSACTION_ERROR` / Range `3000-3999`)
Kesalahan dalam validasi, struktur, atau eksekusi transaksi:

| Kode | Nama Galat | Keterangan & Kondisi Pemicu | Retryable |
| :---: | :--- | :--- | :---: |
| `3001` | `TX_INVALID_VERSION` | Versi transaksi tidak didukung (`version != 1`). | **FALSE** |
| `3002` | `TX_MISMATCHED_CHAIN_ID`| Chain ID tidak cocok dengan jaringan aktif. | **FALSE** |
| `3003` | `TX_INVALID_NONCE` | Nonce transaksi tidak cocok dengan akun saat ini. | **TRUE** (Sync Nonce) |
| `3004` | `TX_INSUFFICIENT_BALANCE`| Saldo akun tidak mencukupi untuk `amount + fee`.| **FALSE** |
| `3005` | `TX_FEE_TOO_LOW` | Biaya transaksi di bawah batas minimum ($10.000\ Q$).| **TRUE** (Naikkan Fee)|
| `3006` | `TX_PAYLOAD_TOO_LARGE` | Ukuran payload melebihi plafon $65.536\ \text{Bytes}$. | **FALSE** |
| `3007` | `TX_INVALID_SIGNATURE` | Tanda tangan Ed25519 gagal diverifikasi secara ketat. | **FALSE** |
| `3008` | `TX_SENDER_MISMATCH` | Alamat pengirim tidak cocok dengan kunci publik penandatangan. | **FALSE** |
| `3009` | `TX_ALREADY_KNOWN` | Transaksi dengan hash yang sama sudah ada di mempool. | **FALSE** |
| `3010` | `TX_ALREADY_FINALIZED`| Transaksi telah dieksekusi dan masuk blok final. | **FALSE** |
| `3011` | `TX_NONCE_GAP` | Nonce melompati antrean (terdapat celah nonce sebelumnya). | **TRUE** (Kirim Gap) |
| `3012` | `TX_RBF_FEE_TOO_LOW` | Transaksi RBF gagal karena kenaikan fee $< +10\%$. | **TRUE** (Naikkan Fee)|

### 2.2 Domain Konsensus & Protokol (`CONSENSUS_ERROR` / Range `2000-2999`)
Kesalahan dalam validasi blok dan voting BFT:

| Kode | Nama Galat | Keterangan & Kondisi Pemicu | Retryable |
| :---: | :--- | :--- | :---: |
| `2001` | `BFT_QUORUM_NOT_REACHED` | Suara yang terkumpul belum mencapai ambang $> 2/3$. | **TRUE** (Tunggu Suara) |
| `2002` | `BFT_DUPLICATE_VOTE` | Validator terdeteksi mengirim suara ganda pada putaran yang sama.| **FALSE** (Slashing) |
| `2003` | `BFT_INVALID_PROPOSER` | Blok diusulkan oleh simpul di luar giliran round-robin. | **FALSE** |
| `2004` | `BFT_INVALID_COMMIT_CERT`| Sertifikat komitmen memuat tanda tangan yang tidak sah. | **FALSE** |
| `2005` | `BFT_ROUND_TIMEOUT` | Putaran konsensus melewati batas waktu tanpa keputusan. | **TRUE** (Putaran Baru)|

### 2.3 Domain Antarmuka RPC (`RPC_ERROR` / Range `4000-4999`)
Kesalahan interaksi klien dengan gateway simpul:

| Kode | Nama Galat | Keterangan & Kondisi Pemicu | Retryable |
| :---: | :--- | :--- | :---: |
| `4001` | `RPC_RESOURCE_NOT_FOUND` | Blok, transaksi, atau alamat tidak ditemukan di database. | **FALSE** |
| `4002` | `RPC_FINALITY_NOT_REACHED`| Permintaan meminta data status final yang belum tercapai. | **TRUE** (Tunggu Blok)|
| `4003` | `RPC_RATE_LIMIT_EXCEEDED` | Klien melampaui kuota permintaan per detik (RPS). | **TRUE** (Backoff) |
| `4004` | `RPC_NODE_SYNCING` | Simpul sedang mengejar ketinggian rantai (*syncing state*). | **TRUE** (Tunggu Sync) |

### 2.4 Domain Aplikasi Klien (`APPLICATION_ERROR` / Range `6000-6999`)
Kesalahan pada logika dompet dan input pengguna:

| Kode | Nama Galat | Keterangan & Kondisi Pemicu | Retryable |
| :---: | :--- | :--- | :---: |
| `6001` | `APP_WALLET_LOCKED` | Kunci privat terkunci oleh kata sandi pengguna. | **TRUE** (Buka Kunci)|
| `6002` | `APP_INVALID_BECH32M` | String alamat gagal validasi format atau checksum Bech32m.| **FALSE** |
| `6003` | `APP_INVALID_PRECISION`| Input nominal memuat lebih dari 8 digit desimal. | **FALSE** (Koreksi Input)|
| `6004` | `APP_INVOICE_EXPIRED` | Batas waktu tagihan merchant telah kedaluwarsa. | **FALSE** (Minta Tagihan Baru)|
| `6005` | `APP_HW_REJECTED` | Pengguna menolak penandatanganan pada layar hardware wallet. | **FALSE** |

---

## 3. Strategi Penanganan dan Percobaan Ulang Klien (Retry Policy)

Ketika aplikasi klien atau SDK menerima objek galat:

1. **Evaluasi Flag `retryable`:**  
   - Jika `retryable == false`: Aplikasi **MUST NOT** mengulang permintaan yang sama secara otomatis. Antarmuka harus menampilkan pesan kesalahan yang jelas kepada pengguna.
   - Jika `retryable == true`: Aplikasi **MAY** mengulang permintaan menggunakan algoritma **Exponential Backoff dengan Jitter**:
     $$t_{\text{wait}} = \min\left(t_{\max},\; t_{\text{base}} \times 2^{\text{attempt}}\right) \pm \text{RandomJitter}$$
2. **Penanganan Celah Nonce (`TX_NONCE_GAP`):**  
   SDK **SHOULD** secara otomatis memeriksa transaksi tertunda lokal yang belum terkirim untuk menuntaskan urutan nonce secara berurutan.
