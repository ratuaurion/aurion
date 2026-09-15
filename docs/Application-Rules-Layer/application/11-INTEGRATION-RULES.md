# 11 — AURION INSTITUTIONAL INTEGRATION RULES
## Pedoman Integrasi Bursa Aset Kripto (CEX), Gerbang Pembayaran, Kustodian, dan Layanan Ekosistem

> **Hierarki Dokumen:**  
> `AURION CONSTITUTIONS` $\longrightarrow$ `PROTOCOL SPECIFICATIONS` $\longrightarrow$ `00-APPLICATION-RULES` $\longrightarrow$ **`11-INTEGRATION-RULES`**  
>
> **Status:** RATIFIED & LOCKED APPLICATION SPECIFICATION  
> **Klasifikasi:** Pedoman Integrasi Institusional (Enterprise Integration Standard)  
> **Versi:** 1.0.0-PROD  
> **Sifat Ketetapan:** Normatif Wajib (RFC 2119 / RFC 8174), Keamanan Finansial Tingkat Tinggi

---

## 1. Arsitektur Integrasi Bursa Aset Kripto (CEX Architecture)

Bursa aset kripto terpusat yang mencatatkan aset AUR **MUST** mematuhi arsitektur dompet berjenjang (*Tiered Wallet Architecture*):

```text
┌────────────────────────────────────────────────────────┐
│                   BURSA ASET KRIPTO                    │
│                                                        │
│  [COLD STORAGE VAULT] (95% Dana Cadangan)              │
│  ├── Perangkat Air-Gapped / Hardware HSM               │
│  └── Skema Multi-Signature / Threshold Sig             │
│            │                                           │
│            ▼ Transfer Terjadwal Manual (Whitelisted)   │
│  [WARM / HOT WALLET] (5% Dana Operasional Harian)      │
│  ├── Mesin Penarikan Otomatis (Automated Withdrawals)  │
│  └── Batas Penarikan Harian (Velocity Rate Limits)     │
│            ▲                                           │
│            │ Sweeping Terkonsolidasi                   │
│  [OMNIBUS DEPOSIT ADDRESS / MEMO ROUTING]              │
│  └── Alamat Publik Penampung Deposit Pengguna          │
└────────────────────────────────────────────────────────┘
```

---

## 2. Pipa Pemrosesan Deposit Bursa (Deposit Processing Pipeline)

Untuk mencegah kerugian akibat transaksi yang dibatalkan atau serangan pengeluaran ganda (*double-spending*), sistem backend bursa **MUST** mengeksekusi 5 tahapan atomik:

```text
[TAHAP 1] Deteksi Transaksi Masuk melalui Langganan WebSocket RPC (aur_subscribe)
    ↓
[TAHAP 2] Penguraian Format: Validasi Alamat Tujuan & Ekstraksi Tag Memo (USR:XXXXX)
    ↓
[TAHAP 3] Asersi Ketinggian Rantai & Validasi Keberadaan Commit Certificate (CC)
    ↓
[TAHAP 4] Pengecekan Idempotensi Internal: Pastikan TxID Belum Pernah Diproses
    ↓
[TAHAP 5] Eksekusi Mutasi Kredit Saldo Pengguna di Database Internal Bursa
```

### 2.1 Mandat Mutlak Anti-Kredit Spekulatif
1. Bursa **MUST NOT** mengkreditkan saldo pengguna pada tahap `MEMPOOL` atau `INCLUDED`.
2. Kredit saldo **HANYA SAH SECARA OPERASIONAL** setelah simpul RPC mengembalikan status `FINALIZED` dengan bukti Commit Certificate yang valid.
3. Karena Aurion-BFT menjamin finalitas dalam 1 slot blok, bursa **SHOULD** segera mengkreditkan dana begitu blok final terkonfirmasi tanpa membebani pengguna dengan waktu tunggu tambahan.

---

## 3. Pipa Pemrosesan Penarikan Dana (Withdrawal Processing Pipeline)

Sistem penarikan dana bursa (*Withdrawal Engine*) **MUST** mengikuti prosedur berikut:

1. **Pemeriksaan Saldo Terpakai (Spendable Balance Check):**  
   Pastikan hot wallet bursa memiliki saldo aktif yang memadai setelah memperhitungkan seluruh transaksi penarikan sebelumnya yang masih mengantre di mempool.
2. **Manajemen Nonce Tunggal (Strict Nonce Mutex):**  
   Proses penandatanganan penarikan **MUST** menggunakan mekanisme penguncian (*mutex / atomic counter*) untuk memastikan tidak ada dua transaksi penarikan paralel yang menggunakan nilai `nonce` yang sama.
3. **Penyertaan Biaya Layanan Jaringan:**  
   Bursa **MUST** menyertakan biaya transaksi yang mematuhi plafon dinamis jaringan dan tidak boleh membebankan fee penarikan kepada pengguna melebihi biaya operasional wajar yang transparan.

---

## 4. Standar Layanan Kustodian & Multi-Signature (Custody Standards)

Layanan kustodian institusional dan perbendaharaan DAO Aurion **SHOULD** menerapkan pengamanan multi-tanda tangan:

1. **Skema Kriptografi Tanda Tangan Ambang (Threshold Signatures / FROST):**  
   Untuk kurva Ed25519, kustodian **SHOULD** memanfaatkan skema FROST (Flexible Round-Optimized Schnorr Threshold) atau MuSig2.
   - Menghasilkan tanda tangan 64-byte Ed25519 kanonikal standar yang indistinguishable dari tanda tangan tunggal.
   - Menghemat ruang blok dan menyembunyikan kebijakan ambang (*threshold policy*) dari pengamatan publik demi keamanan operasional.
2. **Daftar Putih Alamat (Address Whitelisting):**  
   Sistem perbendaharaan **MUST** memberlakukan masa tunggu (*timelock delay*) sekurang-kurangnya 24 jam sebelum alamat penarikan baru dapat didaftarkan sebagai tujuan transfer sah.

---

## 5. Standar Jembatan Antar-Rantai (Cross-Chain Bridge Rules)

Jembatan yang menghubungkan Aurion dengan blockchain lain (Ethereum, Solana, Bitcoin, Cosmos) **MUST** beroperasi dengan prinsip keamanan minim kepercayaan (*Trust-Minimized Light Client Verification*):

1. **Verifikasi Kriptografis Header & Commit Certificate:**  
   Smart contract jembatan di rantai tujuan **MUST** memverifikasi langsung tanda tangan $> 2/3$ validator Aurion pada Commit Certificate blok sumber sebelum merilis aset sintetis (*wrapped asset*).
2. **Larangan Bergantung pada Orakel Terpusat Tunggal:**  
   Jembatan **MUST NOT** mengizinkan pencetakan (*minting*) aset hanya berdasarkan tanda tangan server RPC tunggal tanpa bukti keabsahan Merkle state root.
