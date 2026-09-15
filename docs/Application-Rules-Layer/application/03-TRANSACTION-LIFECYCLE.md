# 03 — AURION TRANSACTION LIFECYCLE & SUBMISSION RULES
## Siklus Hidup Transaksi Aplikasi, Status Mesin Penyerahan, dan Semantik Eksekusi

> **Hierarki Dokumen:**  
> `AURION CONSTITUTIONS` $\longrightarrow$ `PROTOCOL SPECIFICATIONS` $\longrightarrow$ `00-APPLICATION-RULES` $\longrightarrow$ **`03-TRANSACTION-LIFECYCLE`**  
>
> **Status:** RATIFIED & LOCKED APPLICATION SPECIFICATION  
> **Klasifikasi:** Manajemen Transaksi Aplikasi (Transaction Lifecycle Management)  
> **Versi:** 1.0.0-PROD  
> **Sifat Ketetapan:** Normatif Wajib (RFC 2119 / RFC 8174), Deterministik, State-Machine Formal

---

## 1. Mesin Keadaan Siklus Transaksi (Transaction State Machine)

Setiap transaksi di dalam ekosistem Aurion berpindah melalui diagram keadaan (*finite state machine*) formal yang terdefinisi secara ketat:

```text
       [1. CREATED]
            │
            ▼ (Tandatangani via Private Key)
       [2. SIGNED]
            │
            ▼ (Kirim via aur_sendRawTransaction)
      [3. SUBMITTED]
       │          │
 (Valid)          ▼ (Gagal Validasi Awal)
       │    [REJECTED] ── (Terminal: Format Cacat / Saldo Kurang)
       ▼
 [4. MEMPOOL] ────┬─── (TTL Habis / Tergusur) ──► [DROPPED / EXPIRED]
       │          │
       │          └─── (Diganti Fee Lebih Tinggi) ──► [REPLACED]
       ▼ (Dimasukkan ke Blok Kandidat)
 [5. INCLUDED]
       │
       ▼ (Commit Certificate Kuorum >2/3 Diratifikasi)
 [6. FINALIZED] ── (Terminal: Sukses Abadi & Tidak Dapat Dibatalkan)
```

---

## 2. Rincian dan Aturan Transisi Keadaan (State Transitions)

### 2.1 Status 1: `CREATED` (Pembuatan Objek Transaksi)
- **Kondisi:** Transaksi telah disusun oleh aplikasi atau SDK dengan field parameter lengkap (`version`, `chain_id`, `nonce`, `sender`, `recipient`, `amount`, `fee`, `payload`), namun field `signature` masih bernilai kosong (`[0u8; 64]`).
- **Aturan:** Aplikasi **MUST** memverifikasi bahwa `nonce` telah disinkronkan dengan state on-chain terbaru ditambah antrean lokal.

### 2.2 Status 2: `SIGNED` (Penandatanganan Digital)
- **Kondisi:** Hash preimage penandatanganan `Blake3("AURION-TX-V1" || UnsignedTxBytes)` telah ditandatangani secara kriptografis oleh kunci privat pengirim Ed25519.
- **Aturan:** Wallet **MUST NOT** mengubah byte apa pun pada transaksi setelah tanda tangan dibubuhkan. Setiap mutasi byte sekecil apa pun akan membatalkan tanda tangan.

### 2.3 Status 3: `SUBMITTED` (Pengiriman ke Jaringan)
- **Kondisi:** Transaksi biner lengkap telah dikirimkan ke simpul melalui antarmuka RPC `aur_sendRawTransaction`.
- **Aturan:** Klien **MUST** mencatat timestamp penyerahan dan memulai timer batas waktu pemantauan (*submission timeout*).

### 2.4 Status 4: `MEMPOOL` (Penerimaan di Kolam Memori)
- **Kondisi:** Simpul telah menjalankan validasi nir-status (*stateless validation*), memeriksa keabsahan tanda tangan, kecukupan saldo pengirim terhadap `amount + fee`, dan kebenaran `nonce`.
- **Aturan:** 
  - Simpul **MUST** menyiarkan transaksi ke tetangga peer P2P via pesan `TX_GOSSIP`.
  - Transaksi pada status ini berpotensi mengalami:
    - **`REPLACED`:** Jika pengirim menyiarkan transaksi baru dengan nonce yang sama namun dengan fee sekurang-kurangnya $+10\%$ lebih tinggi.
    - **`DROPPED` / `EXPIRED`:** Jika transaksi tertahan di mempool melampaui masa hidup maksimum $\text{TTL} = 3.600\ \text{detik}$ (sekitar 60 blok) tanpa dimasukkan ke dalam blok.

### 2.5 Status 5: `INCLUDED` (Penyertaan dalam Blok)
- **Kondisi:** Transaksi telah dipilih oleh Proposer aktif dan disertakan ke dalam blok kandidat $B_H$ yang telah dieksekusi oleh State Transition Function ($\text{STF}$).
- **Aturan:** Saldo pengirim telah terdebet dan penerima telah terkredit pada state spekulatif blok $H$. Namun, karena blok belum mengantongi bukti kuorum finalitas BFT, status ini **MUST NOT** dianggap sebagai pembayaran selesai oleh bursa atau merchant.

### 2.6 Status 6: `FINALIZED` (Finalitas Mutlak Rantai)
- **Kondisi:** Blok $B_H$ telah mengantongi Commit Certificate resmi $\mathcal{CC}(B_H)$ yang ditandatangani oleh $> 2/3$ bobot validator aktif.
- **Aturan:** 
  - Transaksi mencapai status permanen mutlak (*Irreversible State*).
  - Transaksi dijamin secara matematis tidak akan pernah mengalami pembatalan (*zero reorg probability*).
  - Seluruh sistem akuntansi, bursa, dan merchant **MUST** memperlakukan status ini sebagai transaksi sah yang telah tuntas.

---

## 3. Keadaan Kegagalan dan Terminasi (Terminal Failure States)

Aplikasi **MUST** menangani empat jenis status terminasi kegagalan:

1. **`REJECTED` (Penolakan Awal):**  
   Transaksi ditolak seketika pada saat penyerahan ke RPC karena tanda tangan tidak valid, saldo kurang, nonce salah, atau format cacat. Transaksi tidak pernah masuk ke mempool.
2. **`DROPPED` (Tergusur dari Mempool):**  
   Transaksi terdepak dari mempool karena kolam memori penuh (*mempool congestion*) dan terdapat transaksi lain dengan rasio fee yang lebih kompetitif.
3. **`EXPIRED` (Kedaluwarsa):**  
   Transaksi tidak terpilih ke dalam blok hingga batas waktu 3.600 detik terlewati. Aplikasi **SHOULD** menyarankan pengguna untuk menyiarkan ulang dengan fee yang disesuaikan.
4. **`REPLACED` (Tergantikan):**  
   Transaksi dibatalkan karena transaksi ber-nonce identik dengan fee lebih tinggi telah dimasukkan ke dalam blok kanonikal.

---

## 4. Tanda Terima Transaksi (Transaction Receipt Specification)

Ketika sebuah transaksi mencapai status `FINALIZED`, simpul RPC **MUST** mampu mengembalikan objek tanda terima (*Receipt*) terotentikasi:

```json
{
  "tx_id": "7a9f82635921820db84639e8e503b879101265c05295c20c46b5a37130a10972",
  "block_height": 1042,
  "block_hash": "9b456209c11874296df4425b42d1396a58231d8e1329a43587b1c42e54308821",
  "transaction_index": 0,
  "sender": "aur10t9h48nhaunuj2uqf8kfd6jqjq07h0ecv85h49lsreaq9uqhqffshf0p6h",
  "recipient": "aur1qpz6j2e8vvg9x5z4yqw7sc6v0c8y37v5a2p004fswh2z3xql6k8sq66evcf",
  "amount_quanta": "100000000",
  "fee_quanta": "10000",
  "fee_burned_quanta": "2000",
  "fee_miner_quanta": "8000",
  "nonce": 42,
  "status": "FINALIZED",
  "finality_proof": {
    "round": 0,
    "commit_signatures_count": 4,
    "voting_power_percent": "100.00"
  }
}
```

Aplikasi akuntansi dan explorer **MUST** menggunakan rincian `fee_burned_quanta` dan `fee_miner_quanta` dari receipt resmi untuk memastikan rekonsiliasi pembukuan yang akurat 100%.
