# Aurion Public Testnet & Community Sandbox Guide (NET-012)

Panduan resmi operasional dan pengembangan bagi komunitas, pengembang dApp, penyedia dompet, dan pihak ketiga untuk terhubung, berinteraksi, dan membangun di atas **Jaringan Testnet Publik Aurion**.

---

## 1. Parameter Konfigurasi Jaringan Publik

Gunakan parameter berikut untuk mengonfigurasi dompet (Web Wallet, ekstensi peramban) dan SDK klien:

| Parameter Jaringan | Nilai Resmi | Keterangan |
| :--- | :--- | :--- |
| **Nama Jaringan** | `Aurion Public Testnet` | Nama tampilan di dompet & explorer |
| **Chain ID** | `9999` (atau `1001` untuk genesis staging) | ID rantai deterministik pencegah replay lintas-jaringan |
| **Simbol Aset** | `AUR` | Mata uang kripto native berdaulat |
| **Unit Terkecil (Atomic Unit)** | `Quantum` (1 AUR = $10^9$ Quanta) | Presisi fixed integer 9 desimal tanpa floating-point |
| **Mekanisme Konsensus** | Round-Based BFT Finality | Waktu blok deterministik, kuorum $>2/3$, SLA $<1.000$ ms |
| **Mekanisme Fee** | 100% ke Validator Proposer | Melengkapi insentif emisi blok BFT |
| **Public JSON-RPC URL** | `http://127.0.0.1:8545` (atau `bootnode.ratuaurion.store:8080`) | Mendukung JSON-RPC 2.0 & CORS |
| **Public WebSocket URL** | `ws://127.0.0.1:8545` | Streaming blok & tx via RFC 6455 |
| **Sandbox & Explorer Web UI**| `http://127.0.0.1:8545/sandbox` | Dashboard visual interaktif bawaan |

---

## 2. Kebijakan CORS & Gateway Terbuka

Server Gateway Aurion (`RpcServer`) telah dilengkapi dengan penanganan preflight `OPTIONS` dan header CORS universal:

```http
Access-Control-Allow-Origin: *
Access-Control-Allow-Methods: GET, POST, OPTIONS
Access-Control-Allow-Headers: Content-Type, Authorization, X-Requested-With
```

Hal ini memungkinkan aplikasi frontend dApp, penjelajah blok pihak ketiga, dan antarmuka web peramban untuk berkomunikasi langsung dengan simpul Aurion tanpa membutuhkan reverse proxy tambahan.

---

## 3. Keran Dana Uji Coba (Testnet Faucet)

Untuk memulai pengujian transaksi dan smart contract di testnet publik, pengembang dapat mengklaim dana testnet gratis sebesar **10 AUR (10.000.000.000 Quanta)** per klaim.

### Aturan & Batas Frekuensi (Rate Limiting)
- **Kuota per Klaim:** 10 AUR (10.000.000.000 Quanta).
- **Periode Cooldown:** 60 detik per alamat penerima.
- **Biaya Transaksi Faucet:** Ditanggung oleh otoritas faucet (20.000 Quanta).

### Cara Mengklaim Token Faucet

#### A. Melalui Web Sandbox Dashboard (Satu Klik)
Buka peramban ke alamat:
```
http://127.0.0.1:8545/sandbox
```
Masukkan alamat Aurion Anda (`aur1...`) pada kartu **Testnet Faucet Dispenser**, lalu klik **Claim 10 AUR**.

#### B. Melalui Single Binary CLI (`/bin/aurion`)
```bash
aurion faucet request <ALAMAT_PENERIMA>
```
Contoh luaran format mesin (`--output json`):
```json
{
  "status": "DISPENSED",
  "recipient": "aur1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqsqqqqqqqq",
  "amount_aur": "10.00000000",
  "amount_quanta": 1000000000,
  "tx_hash": "0x8f10a7b4892c5d1e2f3a4b5c6d7e8f90123456789abcdef0123456789abcdef0"
}
```

#### C. Melalui JSON-RPC 2.0 API (`aur_requestFaucet`)
```bash
curl -X POST http://127.0.0.1:8545 \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "method": "aur_requestFaucet",
    "params": ["aur1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqsqqqqqqqq"],
    "id": 1
  }'
```

#### D. Melalui Perkakas Python (`tools/public_testnet.py`)
```bash
python tools/public_testnet.py faucet <ALAMAT_PENERIMA> --rpc http://127.0.0.1:8545
```

---

## 4. Penjelajah Blok REST API (Explorer Endpoints)

Simpul Aurion menyediakan endpoint REST HTTP terpadu untuk integrasi explorer cepat:

| Endpoint HTTP | Format | Deskripsi |
| :--- | :---: | :--- |
| `GET /explorer/stats` | JSON | Statistik jaringan: tinggi blok, status finalitas, mempool, jumlah akun, dan konfigurasi faucet. |
| `GET /explorer/block/latest` | JSON | Detail blok terbaru: hash, prev_hash, state root, tx root, jumlah tanda tangan kuorum. |
| `GET /explorer/block/<HEIGHT>` | JSON | Detail blok pada tinggi $H$ tertentu. |
| `GET /explorer/tx/<TX_HASH>` | JSON | Detail status transaksi dari mempool antrean (sender, recipient, amount, fee, nonce). |
| `GET /sandbox` | HTML | Dashboard Web Interaktif visual komunitas. |

Contoh kueri statistik:
```bash
curl http://127.0.0.1:8545/explorer/stats
```
Respons:
```json
{
  "chain_id": 9999,
  "current_height": 142,
  "finalized_height": 142,
  "mempool_size": 2,
  "accounts_count": 18,
  "headers_count": 143,
  "faucet": {
    "enabled": true,
    "address": "aur1dev0000000000000000000000000000000000000000000000000sqqqqqqqq",
    "dispense_amount_aur": "10.00000000",
    "cooldown_secs": 60
  }
}
```

---

## 5. Deployment Smart Contract di Testnet Publik

Pengembang dapat mengompilasi dan men-deploy smart contract Aurion Native VM (AVM) secara langsung ke testnet publik:

1. **Siapkan Bytecode Kontrak (Hex):**
   Contoh kontrak arithmetic counter sederhana:
   ```text
   0x600160020100
   ```
2. **Deploy Kontrak via CLI:**
   ```bash
   aurion contract deploy --bytecode 600160020100 --gas-limit 100000 --output json
   ```
3. **Inspeksi Status Kontrak:**
   ```bash
   aurion contract inspect <CONTRACT_ADDRESS> --output json
   ```

---

## 6. Pemeriksaan Kesehatan Simpul Publik

Gateway publik menyediakan endpoint pemantauan liveness dan readiness standar industri:
- **Liveness probe:** `GET /healthz` (mengembalikan `{"status":"OK","service":"aurion-rpc"}`).
- **Deep health probe:** `GET /healthz/deep`.
