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
- **`aur_getNonce`:** Mengembalikan nilai penghitung sekuensil transaksi akun saat ini.
- **`aur_getAccount`:** Mengembalikan objek state akun utuh: `balance`, `nonce`, `payload_root`, `is_contract`, dan `code_hash`.

### 4.4 Modul Kontrak Cerdas (Contract SDK Bridge)

> Modul ini adalah pasangan sisi-simpul dari pipeline **Contract SDK** (`src/platform/contract/`) yang menyatukan Wallet dan AVM dalam satu binary (`AUR-ARCH-001`). Seluruh endpoint bersifat **read-only terhadap state blockchain** dan tidak boleh mengubah `state_root`, konsensus BFT, maupun block production.

Pola eksekusi `aur_call` dan `aur_estimateGas` **wajib** mengikuti urutan kanonik berikut (`AUR-ARCH-004`, implementasi tunggal di `src/statemachine/state/sandbox.rs`):

```text
1. Clone State  -> snapshot hanya akun sender + recipient (O(1)), lock dilepas
2. Apply Tx     -> state::stf::apply_transaction() pada snapshot lokal
3. Extract      -> gas_used, return_data, deployed_contract, storage_changes
```

**Batasan yang tidak dapat dilanggar:** simpul **tidak boleh** pernah memutasi `accounts` state on-chain dari handler mana pun pada modul ini. Simulasi wajib berjalan pada salinan yang dibuang.

#### 4.4.1 Format Transaksi Masukan (Bentuk Kanonik)

Seluruh endpoint kontrak menerima **serialisasi kanonikal `Transaction`** (`Transaction::encode_canonical`) dalam hex, opsional berprefiks `0x`. Bentuk ini dipilih karena:

1. Satu encoding tunggal yang sama dengan `aur_sendRawTransaction` (`AUR-ARCH-005`).
2. Mengandung **seluruh** field terstruktur yang dibutuhkan (calldata, sender, kontrak tujuan, nonce, nilai, fee, `valid_until`) tanpa menambah skema JSON baru.
3. Deterministik penuh: byte yang disimulasikan adalah **byte yang sama persis** yang akan ditandatangani.

Layout kanonikal (184 byte header + payload panjang-berawalan, `MAX_TRANSACTION_PAYLOAD_BYTES` = 24 KB):

| Offset | Ukuran | Field |
| :--- | :---: | :--- |
| 0 | 2 | `version` (wajib `1`) |
| 2 | 4 | `chain_id` |
| 6 | 1 | `tx_type` (`0x05` Deploy, `0x06` Call) |
| 7 | 1 | `flags` |
| 8 | 32 | `sender` |
| 40 | 32 | `recipient` (kontrak; `Address::ZERO` saat deploy) |
| 72 | 8 | `nonce` |
| 80 | 16 | `amount` (Quanta, big-endian) |
| 96 | 16 | `fee` (Quanta, big-endian) |
| 112 | 8 | `valid_until` (detik Unix) |
| 120 | 4 | panjang payload (BE) |
| 124 | varies | `payload` (AVM Call Frame + runtime) |
| 124+N | 64 | `signature` Ed25519 |

#### 4.4.2 `aur_call` — Simulasi Dry-Run Jarak Jauh

- **Fungsi:** Menjalankan `apply_transaction` pada snapshot sandbox dan melaporkan gas, data kembalian, serta hasil deploy **tanpa memutasi state**.
- **Parameter:** `params[0] = raw_tx_hex`, `params[1] = gas_limit` (opsional, harus `1..=1000000`).
- **Tanda tangan:** **tidak** diverifikasi. Transaksi yang belum ditandatangani (`signature = 0x00..00`) **wajib** diterima karena simulasi justru mendahului clear signing.
- **Batas keras:** gas limit kanonik `1_000_000` (`AUR-VM-003`, identik STF), payload maksimum 24 KB, token bucket 20 permintaan/detik dengan burst 40, deadline wall-clock 2 detik.

Request:

```json
{"jsonrpc":"2.0","id":1,"method":"aur_call","params":["<raw_tx_hex>"]}
```

Response sukses (kontrak tereksekusi tanpa revert):

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "success": true,
    "gas_used": 96,
    "return_data": "000000000000000000000000000000000000000000000000000000000000002a",
    "reason": null,
    "deployed_contract": null,
    "storage_changes": 0,
    "gas_limit": 1000000
  }
}
```

Response gagal-STF tetap HTTP 200 dengan `success: false` (agar klien dapat menampilkan alasan):

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "success": false,
    "gas_used": 0,
    "return_data": "",
    "reason": "Target recipient is not a smart contract: ...",
    "deployed_contract": null,
    "storage_changes": 0,
    "gas_limit": 1000000
  }
}
```
- **`aur_getBalance`:** Mengembalikan saldo aktif akun dalam satuan Quantum ($u128$) pada target block parameter.
- **`aur_getNonce`:** Mengembalikan nilai penghitung sekuensial transaksi akun saat ini.
- **`aur_getAccount`:** Mengembalikan objek state akun utuh: `balance`, `nonce`, dan `payload_root`.

---

#### 4.4.3 `aur_estimateGas` — Estimasi Gas

- **Fungsi:** Menjalankan bytecode dengan konteks eksekusi **identik STF** (gas limit 1.000.000, storage kosong, block 0, timestamp 0) dan melaporkan `gas_used`.
- **Parameter:** identik dengan `aur_call`.
- **Perilaku galat:** kegagalan eksekusi dikembalikan **di dalam payload** (`success: false` + `reason`), bukan sebagai JSON-RPC error, sehingga klien dapat membedakan "kontrak revert" dari "permintaan tidak valid".

Request:

```json
{"jsonrpc":"2.0","id":2,"method":"aur_estimateGas","params":["<raw_tx_hex>"]}
```

Response:

```json
{"jsonrpc":"2.0","id":2,"result":{"gas_used":96,"gas_limit":1000000,"success":true,"reason":null}}
```

#### 4.4.4 Metadata Kontrak — Asumsi Off-Chain (WAJIB DIDOKUMENTASIKAN)

> **Asumsi normative:** Aurion **tidak** menyimpan metadata/ABI kontrak on-chain. `Account` on-chain hanya memuat `balance`, `nonce`, `code_hash`, dan `storage_root` (`AUR-VM-006`). Menyimpan metadata ke state akan mengubah `state_root` setiap blok — dengan demikian mengubah consensus, yang dilarang secara eksplisit.
>
> Konsekuensinya: metadata adalah indeks **off-chain, in-memory, best-effort per simpul**. Registry hilang saat restart simpul dan **tidak** tercermin dalam konsensus. Penulisannya tidak pernah menyentuh mempool, state, BFT, atau block production.
>
> Pertahanan anti-penipuan tetap terjaga: `ContractMetadata::verify_binding` mewajibkan `code_hash` metadata sama dengan `code_hash` on-chain yang dibaca via `aur_getAccount` / `aur_getCode`, dan setiap selector ABI **selalu diturunkan ulang** dari `signature` saat deserialisasi (selector dari JSON tidak pernah dipercaya).

- **`aur_sendContractMetadata`:** Mendaftarkan metadata mentah ke registry simpul. Parameter: `[code_hash_hex, metadata_json]`. Ditolak (`-32602`) bila skema tidak cocok, `code_hash` metadata berbeda dari parameter, atau `runtime_hash != blake3(runtime)`.
- **`aur_getContractMetadata`:** Mengambil metadata terdaftar. Parameter: `[code_hash_hex]`. Mengembalikan `-32002` (`RESOURCE_NOT_FOUND`) bila belum terdaftar; SDK lalu memakai metadata lokal terikat `code_hash`.
- **`aur_getCode`:** Mengembalikan bentuk minimal untuk binding kontrak. Parameter: `[address]`. Selalu mengembalikan `code_hash`, `storage_root`, `is_contract`, dan `code_available: false` (kode tidak disimpan on-chain).

Request:

```json
{"jsonrpc":"2.0","id":3,"method":"aur_getContractMetadata","params":["<code_hash_hex>"]}
```

Response:

```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "result": {
    "schema": "aurion/contract-metadata-v1",
    "name": "Echo",
    "chain_id": 1001,
    "address": "aur1...",
    "code_hash": "<64 hex>",
    "runtime": "<hex>",
    "runtime_hash": "<64 hex>",
    "methods": [{ "name": "echo", "signature": "echo()", "inputs": [], "outputs": [], "payable": false }]
  }
}
```

#### 4.4.5 Validasi Intent pada `aur_sendRawTransaction`

Parameter `aur_sendRawTransaction` diperluas secara **backward compatible** dari `[raw_hex, sender_pubkey_hex]` menjadi:

```text
[raw_tx_hex, sender_pubkey_hex, intent_payload_hash?, intent_code_hash?]
```

Untuk `ContractDeploy` dan `ContractCall`, simpul **wajib** melakukan:

1. **Verifikasi bytecode statis** (`AUR-VM-005`) sebelum masuk mempool — payload rusak ditolak sejak awal, bukan hanya saat block building.
2. Bila `params[2]` dan `params[3]` diberikan, keduanya **wajib** ada dan divalidasi:
   - `params[2]` harus sama dengan `blake3(tx.payload)` — mengikat intent ke byte yang tepat.
   - Deploy: `params[3]` harus sama dengan `blake3(tx.payload)` (= `code_hash` on-chain yang akan dicatat STF).
   - Call: `params[3]` harus sama dengan `code_hash` kontrak pada state on-chain — inilah yang menolak klien yang menampilkan metadata palsu.
3. Pengiriman hanya satu dari dua hash ditolak (`-32602`).
4. Verifikasi tanda tangan Ed25519, chain ID, kedaluwarsa, nonce, dan saldo tetap ditangani `MempoolEngine::submit_transaction` (tidak berubah).

Bila parameter intent tidak diberikan (klien non-SDK), validasi intent dilewati dan perilaku lama dipertahankan penuh.

#### 4.4.6 Matriks Galat Modul Kontrak

| Kode | Nama | Pemicu |
| :---: | :--- | :--- |
| `-32602` | `INVALID_PARAMS` | Hex rusak, `version != 1`, `chain_id` salah, `gas_limit` di luar `1..=1000000`, intent binding tidak lengkap, metadata tidak valid. |
| `-32001` | `TX_REJECTED` | Bytecode gagal verifikasi statis; intent `payload_hash`/`code_hash` tidak cocok; tujuan bukan kontrak on-chain. |
| `-32002` | `RESOURCE_NOT_FOUND` | Metadata kontrak belum terdaftar pada registry off-chain. |
| `-32004` | `RATE_LIMIT_EXCEEDED` | Token bucket simulasi habis. |
| `-32603` | `INTERNAL_ERROR` | Eksekusi melampaui deadline wall-clock 2 detik (sandbox tidak termutasi). |

#### 4.4.7 Pemetaan `RpcProvider` (Klien) ke Endpoint Simpul

| Metode `Provider` (`src/platform/contract/provider.rs`) | Endpoint simpul | Catatan fallback |
| :--- | :--- | :--- |
| `get_account` | `aur_getAccount` | Tidak ada fallback (wajib on-chain). |
| `get_nonce` | `aur_getAccount` (default trait) | Turunan `get_account`. |
| `simulate` | **`aur_call`** | Bila simpul mengembalikan `-32601`, jatuh ke simulasi lokal dengan akun yang di-fetch via RPC. |
| `estimate_gas` | **`aur_estimateGas`** | Bila endpoint tidak tersedia, jatuh ke `state::sandbox::estimate_gas` lokal. |
| `fetch_metadata` | **`aur_getContractMetadata`** | Bila `-32002`, SDK memakai metadata lokal terikat `code_hash`. |
| `broadcast` | `aur_sendRawTransaction` | Tidak ada fallback (broadcast bersifat wajib). |

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
