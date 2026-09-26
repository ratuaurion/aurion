# 04 — AURION FINALITY & CONFIRMATION APPLICATION RULES
## Kebijakan Operasional Finalitas Deterministik, Ambang Batas Konfirmasi, dan Standar Penyelesaian Finansial

> **Hierarki Dokumen:**  
> `AURION CONSTITUTIONS` $\longrightarrow$ `PROTOCOL SPECIFICATIONS` $\longrightarrow$ `00-APPLICATION-RULES` $\longrightarrow$ **`04-FINALITY-CONFIRMATION-RULES`**  
>
> **Status:** RATIFIED & LOCKED APPLICATION SPECIFICATION  
> **Klasifikasi:** Standar Finalitas & Penyelesaian Transaksi (Finality & Settlement Standard)  
> **Versi:** 1.0.0-PROD  
> **Sifat Ketetapan:** Normatif Wajib (RFC 2119 / RFC 8174), Zero-Reorg, Deterministik

---

## 1. Hakikat Finalitas Aurion: Deterministik vs Probabilistik

Protokol Aurion **TIDAK MENGGUNAKAN** model finalitas probabilistik ala Nakamoto (seperti "tunggu 6 konfirmasi" pada Bitcoin). 

Aurion menggunakan mesin konsensus **Aurion-BFT** dengan **Finalitas Tunggal Deterministik (*Round-Based BFT with Quorum Finality*)**:
- Sebuah blok $B$ dianggap **FINAL SECARA MATEMATIS** saat dan hanya saat sekurang-kurangnya $> 2/3$ bobot voting validator aktif telah membubuhkan tanda tangan sah pada fase Pre-commit dan menghasilkan **Commit Certificate $\mathcal{CC}(B)$**.
- Setelah $\mathcal{CC}(B)$ terbentuk, probabilitas reorganisasi (*reorg*) adalah **MUTLAK 0%** di bawah asumsi Byzantine $< 1/3$.

> **Aksioma Kedaulatan Finalitas:**  
> Tidak ada aplikasi klien, bursa, dompet, atau antarmuka web yang memiliki otoritas untuk menciptakan definisi finalitas buatan sendiri yang menyimpang dari keberadaan Commit Certificate protokol.

---

## 2. Matriks Kebijakan Penerimaan Konfirmasi (Application Settlement Matrix)

Untuk memastikan konsistensi hukum dan perlindungan aset lintas industri, seluruh aplikasi yang terintegrasi dengan Aurion **MUST** mengikuti matriks kebijakan berikut:

```text
┌────────────────────────────────────────────────────────────────────────┐
│             MATRIKS PENERIMAAN KONFIRMASI BERDASARKAN KASUS            │
├─────────────────────────┬───────────────────────────┬──────────────────┤
│ JENIS APLIKASI / ENTITAS│ STATUS PROTOKOL MINIMUM   │ TINDAKAN DIAKUI  │
├─────────────────────────┼───────────────────────────┼──────────────────┤
│ Bursa Kripto (CEX)      │ FINALIZED (Ber-CC sah)    │ Kredit Deposit   │
│ Deposit Pengguna        │ Tinggi: H (1 Blok Final)  │ Saldo Trading    │
├─────────────────────────┼───────────────────────────┼──────────────────┤
│ Kustodian Institusional │ FINALIZED (Ber-CC sah)    │ Rilis Aset       │
│ & Multi-Signature Vault │ Verifikasi StateRoot SMT  │ Kliring Finansial│
├─────────────────────────┼───────────────────────────┼──────────────────┤
│ Merchant Nilai Tinggi   │ FINALIZED (Ber-CC sah)    │ Pengiriman Barang│
│ (Aset > 10 AUR / Fisik) │ 1 Slot Finality BFT       │ Penerbitan Resi  │
├─────────────────────────┼───────────────────────────┼──────────────────┤
│ Merchant Mikro / Retail │ INCLUDED (Blok Terpilih)  │ "Menunggu Kliring"│
│ (Nilai < 1 AUR / Kopi)  │ Belum CC(B) Final         │ Pesanan Diproses │
├─────────────────────────┼───────────────────────────┼──────────────────┤
│ Layanan Penarikan Dana  │ FINALIZED (Blok H)        │ Eksekusi Outbound│
│ (CEX Withdrawal Engine) │ Debet Spendable Balance   │ Kirim ke On-Chain│
└─────────────────────────┴───────────────────────────┴──────────────────┘
```

---

## 3. Aturan Khusus untuk Bursa Aset Kripto (CEX Deposit Rules)

1. **Larangan Kredit Dini (*Prohibition on Early Credit*):**  
   Bursa aset kripto **MUST NOT** mengkreditkan saldo akun pengguna jika transaksi deposit masih berstatus `MEMPOOL` atau `INCLUDED` tanpa Commit Certificate.
2. **Pemeriksaan Sertifikat Komitmen Wajib:**  
   Sistem penangkap deposit (*Deposit Ingestion Service*) bursa **MUST** memanggil endpoint RPC `aur_getCommitCertificate` untuk blok target sebelum mengeksekusi mutasi saldo database internal:
   ```text
   Assert(CommitCertificate.height == DepositBlock.height)
   Assert(CommitCertificate.block_hash == DepositBlock.hash)
   Assert(CommitCertificate.voting_weight >= QuorumThreshold)
   ```
3. **Ketiadaan Kebutuhan Menunggu Multi-Blok Tambahan:**  
   Karena Aurion-BFT menjamin ketiadaan reorg setelah commit certificate terbit, bursa **SHOULD NOT** membebani pengguna dengan kewajiban menunggu puluhan blok tambahan (misalnya 30 atau 100 blok) yang tidak memiliki dasar matematis pada sistem BFT Aurion.

---

## 4. Aturan untuk Payment Gateway & Merchant

1. **Notifikasi Transaksi Dua Tahap (Two-Stage Payment Notification):**  
   Aplikasi kasir (POS) atau gerbang pembayaran e-commerce **SHOULD** menerapkan alur dua tahap:
   - **Tahap 1 (Deteksi Cepat - Instant Detection):** Saat transaksi terdeteksi di mempool (`MEMPOOL`), tampilkan indikator *"Pembayaran Diterima, Menunggu Konfirmasi Jaringan..."*.
   - **Tahap 2 (Penyelesaian Akhir - Final Settlement):** Saat transaksi berstatus `FINALIZED`, tampilkan tanda centang hijau *"Pembayaran Lunas & Sah Secara Hukum Kripto"*, cetak nota, dan serahkan barang.
2. **Perlindungan Terhadap Partisi Jaringan:**  
   Jika terjadi partisi jaringan global dan waktu pembentukan blok melambat karena simpul menunggu kuorum, kasir **MUST NOT** menyerahkan aset fisik bernilai tinggi sebelum status `FINALIZED` tercapai.

---

## 5. Penanganan Partisi Jaringan dan Reorganisasi Spekulatif

Meskipun blok yang telah difinalisasi tidak dapat mengalami reorg, blok spekulatif tanpa CC berpotensi dibatalkan jika terjadi pergantian putaran konsensus (*Round Change*):

1. **Pemulihan Transaksi yang Terlempar (*Orphaned Transaction Recovery*):**  
   Jika sebuah blok spekulatif $B_{H, R=0}$ gagal mencapai kuorum dan digantikan oleh blok baru $B_{H, R=1}$:
   - Aplikasi klien dan indexer **MUST** memeriksa apakah transaksi yang berada di blok lama kembali masuk ke mempool atau telah dimasukkan ke dalam blok baru.
   - Status transaksi **MUST** dikembalikan secara otomatis dari `INCLUDED` menjadi `MEMPOOL` sampai kepastian blok baru diperoleh.
2. **Peringatan Antarmuka:**  
   Wallet dan explorer **MUST NOT** menampilkan pesan *"Transaksi Berhasil"* selama blok masih dalam status putaran spekulatif.
