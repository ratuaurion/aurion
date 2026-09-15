# 02 — AURION RPC & API RULES SPECIFICATION
## Standar Antarmuka Komunikasi Simpul, Protokol JSON-RPC 2.0, dan Semantik Akses Data

> **Hierarki Dokumen:**  
> `AURION CONSTITUTIONS` $\longrightarrow$ `PROTOCOL SPECIFICATIONS` $\longrightarrow$ `00-APPLICATION-RULES` $\longrightarrow$ **`02-RPC-API-RULES`**  
>
> **Status:** RATIFIED & LOCKED APPLICATION SPECIFICATION  
> **Klasifikasi:** Antarmuka Pemrograman Aplikasi Jaringan (RPC Boundary Standard)  
> **Versi:** 1.0.0-PROD  
> **Sifat Ketetapan:** Normatif Wajib (RFC 2119 / RFC 8174), Machine-Readable, Deterministik

---

## 1. Batasan Arsitektur Antarmuka RPC

RPC (Remote Procedure Call) berfungsi sebagai jembatan tunggal yang memisahkan aplikasi luar (Wallet, Explorer, SDK, Exchange) dari internal mesin konsensus simpul (*Consensus Node*):

```text
[Aplikasi Klien / SDK / Wallet]
              │
              ▼ JSON-RPC 2.0 over HTTP/HTTPS / WebSocket
┌──────────────────────────────────────────────┐
│           LAPISAN RPC GATEWAY                │
│  ├── Autentikasi & Pemeriksaan Kuota (Rate)  │
│  ├── Validasi Skema Request & Sanitasi Input │
│  └── Pembedaan Semantik (Latest/Finalized)   │
└──────────────────────┬───────────────────────┘
                       │
                       ▼ Komunikasi Internal IPC
┌──────────────────────────────────────────────┐
│            SIMPUL INTI AURION                │
│  ├── State Engine (SMT State Lookup)         │
│  ├── Mempool Transaksi                       │
│  └── Mesin Konsensus Aurion-BFT              │
└──────────────────────────────────────────────┘
```

---

## 2. Standar Protokol Transport dan Format Pesan

1. **Format Enkapsulasi:** Seluruh pesan RPC **MUST** mengikuti spesifikasi **JSON-RPC 2.0** resmi.
2. **Protokol Transport Resmi:**
   - **HTTP/1.1 dan HTTP/2:** Menggunakan metode HTTP `POST` dengan header `Content-Type: application/json`.
   - **WebSocket (WSS / WS):** Untuk komunikasi dupleks dua arah dan langganan aliran data (*real-time subscriptions*).
3. **Karakter Enkoding:** Payload JSON-RPC **MUST** dienkode dalam UTF-8 tanpa BOM.

---

## 3. Semantik Konsistensi Data (Consistency Semantics)

Setiap panggilan RPC yang meminta data state akun, saldo, atau blok **MUST** menyediakan parameter penanda konsistensi (*Block Parameter*):

```text
BlockParameter: "finalized" | "safe" | "latest" | <BlockHeight: u64 Hex>
```

### 3.1 Definisi Tiga Semantik State
1. **`finalized` (Sangat Disarankan / Default Finansial):**  
   Mengacu pada blok yang telah mengantongi sertifikat komitmen konsensus $\mathcal{CC}(B)$ kuorum $> 2/3$. Data pada status ini bersifat permanen secara matematis dan kebal terhadap *reorg*.
   - **Klausa Wajib Finansial:** Pertukaran aset, deposit bursa, dan pencairan pembayaran **MUST** membaca state dengan parameter `"finalized"`.
2. **`safe`:**  
   Mengacu pada blok yang telah melewati fase Pre-vote kuorum namun belum merampungkan fase Pre-commit penuh.
3. **`latest`:**  
   Mengacu pada blok kandidat tertinggi yang telah dieksekusi secara lokal oleh simpul, namun belum tentu memiliki sertifikat komitmen final.
   - Server RPC **MUST** memperingatkan klien jika data `latest` berpotensi terdepak jika terjadi pemilihan proposer baru (*round change*).

---

## 4. Katalog Endpoint RPC Resmi (Namespace `aur_`)

Simpul Aurion **MUST** mengimplementasikan endpoint dalam namespace standar berikut:

### 4.1 Modul Informasi Rantai & Blok
- **`aur_chainId`:** Mengembalikan ID jaringan aktif (`1` untuk Mainnet, `2` untuk Testnet).
- **`aur_blockHeight`:** Mengembalikan tinggi blok kanonikal tertinggi saat ini.
- **`aur_getBlockByHeight`:** Mengambil data blok lengkap atau header berdasarkan tinggi blok integer dan parameter konsistensi.
- **`aur_getBlockByHash`:** Mengambil data blok berdasarkan `BlockHash` 32-byte (hex 64 karakter).
- **`aur_getCommitCertificate`:** Mengambil bukti Commit Certificate resmi suatu blok.

### 4.2 Modul Transaksi & Mempool
- **`aur_sendRawTransaction`:** Menyiarkan serialisasi transaksi biner (hex) ke mempool jaringan.
- **`aur_getTransactionByHash`:** Mengambil detail transaksi, lokasi blok, dan status eksekusi berdasarkan `TxID`.
- **`aur_estimateFee`:** Mengembalikan estimasi biaya transaksi minimum yang direkomendasikan untuk masuk ke blok berikutnya.
- **`aur_getMempool`:** Mengembalikan daftar hash transaksi yang saat ini mengantre di mempool lokal simpul.

### 4.3 Modul State Akun
- **`aur_getBalance`:** Mengembalikan saldo aktif akun dalam satuan Quantum ($u128$) pada target block parameter.
- **`aur_getNonce`:** Mengembalikan nilai penghitung sekuensial transaksi akun saat ini.
- **`aur_getAccount`:** Mengembalikan objek state akun utuh: `balance`, `nonce`, dan `payload_root`.

---

## 5. Model Galat RPC Baku (Standard JSON-RPC Errors)

Server RPC **MUST** mengembalikan kode galat terstruktur saat permintaan gagal:

| Kode Galat | Nama Galat | Deskripsi Kesalahan |
| :---: | :--- | :--- |
| `-32700` | `PARSE_ERROR` | Format JSON tidak valid atau rusak. |
| `-32600` | `INVALID_REQUEST` | Struktur JSON-RPC 2.0 tidak sesuai spesifikasi. |
| `-32601` | `METHOD_NOT_FOUND` | Nama metode tidak tersedia pada simpul. |
| `-32602` | `INVALID_PARAMS` | Parameter metode tidak valid atau tipe data salah. |
| `-32603` | `INTERNAL_ERROR` | Kesalahan internal pemrosesan simpul. |
| `-32001` | `TX_REJECTED` | Transaksi ditolak oleh mempool (memuat detail sub-error). |
| `-32002` | `RESOURCE_NOT_FOUND` | Blok atau transaksi target tidak ditemukan pada state. |
| `-32003` | `FINALITY_NOT_REACHED`| Operasi meminta bukti finalitas yang belum tercapai. |
| `-32004` | `RATE_LIMIT_EXCEEDED` | Jumlah panggilan melebihi kuota beban simpul. |
| `-32005` | `NODE_SYNCING` | Simpul sedang dalam proses sinkronisasi data historis. |

Contoh Format Galat Spesifik Transaksi:
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "error": {
    "code": -32001,
    "message": "Transaction Rejected",
    "data": {
      "reason": "TX_INSUFFICIENT_BALANCE",
      "required_quanta": "250010000",
      "available_quanta": "100000000"
    }
  }
}
```

---

## 6. Protokol Langganan Aliran Data WebSocket (`aur_subscribe`)

1. Simpul dengan antarmuka WebSocket **MUST** mendukung metode langganan asinkron `aur_subscribe`:
   - **`newHeads`:** Menerbitkan header setiap kali ada blok baru yang dieksekusi.
   - **`finalizedHeads`:** Menerbitkan header blok hanya ketika Commit Certificate resmi telah diratifikasi.
   - **`newPendingTransactions`:** Menyiarkan TxID dari setiap transaksi baru yang diterima di mempool.
   - **`addressEvents`:** Menyiarkan peristiwa transfer dan mutasi saldo untuk alamat akun tertentu.
2. **Mekanisme Heartbeat:** Koneksi WebSocket **MUST** mengirimkan frame `ping` / `pong` setiap 30 detik untuk mendeteksi putusnya sambungan jaringan.
