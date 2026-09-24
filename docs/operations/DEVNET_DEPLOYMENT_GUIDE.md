# ==============================================================================
# AURION DEVNET CONTINUOUS DEPLOYMENT GUIDE (NET-010)
# ==============================================================================
# Dokumen Panduan Operasional & Penerapan Jaringan Uji Coba Berkelanjutan
# Mengacu pada Invariant: AUR-ARCH-001, AUR-ARCH-009, AUR-APP-12, AUR-CONS-*
# ==============================================================================

## 1. Ringkasan Eksekutif & Jawaban Arsitektur

### Pertanyaan Operasional: Docker vs. PC Lokal
> **"Apakah di Era V kita membutuhkan Docker atau masih berjalan di PC lokal?"**

**Jawaban Resmi Arsitektur Sistem Aurion:**
1. **Tidak Wajib Docker — Sangat Direkomendasikan Native PC Lokal:**
   - Aurion adalah **Single Sovereign Monolithic Binary (`AUR-ARCH-001`)** yang dikompilasi secara deterministik tanpa dependensi eksternal (zero C++ runtime, pure Rust, redb ACID).
   - Menjalankan 6 simpul secara native di PC lokal via `tools/devnet_orchestrator.py` atau CLI `/bin/aurion devnet` menghemat resource secara dramatis:
     - **Waktu Startup:** < 500 ms (instan).
     - **Konsumsi Memori Total:** < 100 MB RAM untuk 6 simpul gabungan.
     - **Zero Environment Friction:** Tidak memerlukan instalasi Docker Desktop atau konfigurasi WSL2 yang berat pada host Windows.
2. **Dukungan Penuh Docker (Siap Disk D):**
   - Repository menyediakan aset kontainerisasi produksi: [`Dockerfile`](./Dockerfile) multi-stage minimal dan [`docker-compose.devnet.yml`](./docker-compose.devnet.yml).
   - Konfigurasi Docker Compose telah diatur secara fleksibel dengan environment variable `AURION_DATA_DIR` yang siap dipetakan ke disk D (`D:/aurion-devnet/data` atau `D:\aurion-devnet\data`) untuk server staging atau lingkungan Linux cloud.

---

## 2. Topologi Jaringan Devnet (6 Simpul)

Kluster devnet Aurion terdiri dari 6 simpul berdaulat dengan peran dan alokasi port terisolasi:

```
                                  [ Klien Publik / Dompet / Explorer ]
                                                   │
                                                   ▼
                                      ┌────────────────────────┐
                                      │  Public JSON-RPC & WS  │
                                      │  Gateway (rpc-gateway) │
                                      │  Port: 8545 / 8550     │
                                      └────────────┬───────────┘
                                                   │
                                                   ▼
                                      ┌────────────────────────┐
                                      │   Anti-DDoS Sentry     │
                                      │   Edge Node (sentry-1) │
                                      │   P2P: 7451 | RPC: 8549 │
                                      └────────────┬───────────┘
                                                   │
                          ┌────────────────────────┼────────────────────────┐
                          │                        │                        │
                          ▼                        ▼                        ▼
               ┌──────────────────────┐ ┌──────────────────────┐ ┌──────────────────────┐ ┌──────────────────────┐
               │  Validator 1 (val-1) │ │  Validator 2 (val-2) │ │  Validator 3 (val-3) │ │  Validator 4 (val-4) │
               │  Proposer / BFT Core │ │  BFT Consensus Peer  │ │  BFT Consensus Peer  │ │  BFT Consensus Peer  │
               │ P2P: 7447 | RPC:8545│ │ P2P: 7448 | RPC:8546│ │ P2P: 7449 | RPC:8547│ │ P2P: 7450 | RPC:8548│
               └──────────────────────┘ └──────────────────────┘ └──────────────────────┘ └──────────────────────┘
```

| Simpul | Peran | Alamat P2P Wire (`AUR0`) | Antarmuka RPC (`HTTP/WS`) | Path Basis Data Persisten |
| :--- | :--- | :--- | :--- | :--- |
| **`val-1`** | BFT Validator (Proposer H=1) | `127.0.0.1:7447` | `http://127.0.0.1:8545` | `data/devnet/val-1/storage.redb` |
| **`val-2`** | BFT Validator (Peer Kuorum) | `127.0.0.1:7448` | `http://127.0.0.1:8546` | `data/devnet/val-2/storage.redb` |
| **`val-3`** | BFT Validator (Peer Kuorum) | `127.0.0.1:7449` | `http://127.0.0.1:8547` | `data/devnet/val-3/storage.redb` |
| **`val-4`** | BFT Validator (Peer Kuorum) | `127.0.0.1:7450` | `http://127.0.0.1:8548` | `data/devnet/val-4/storage.redb` |
| **`sentry-1`** | Edge Proxy Anti-DDoS Filter | `127.0.0.1:7451` | `http://127.0.0.1:8549` | `data/devnet/sentry-1/storage.redb` |
| **`rpc-gateway`**| Gerbang Publik DApp/SDK | `127.0.0.1:7452` | `http://127.0.0.1:8550` | `data/devnet/rpc-gateway/storage.redb` |

---

## 3. Menjalankan di PC Lokal (Metode Utama & Cepat)

### Langkah 3.1: Inisialisasi Devnet
Gunakan skrip orkestrator Python bawaan:
```powershell
python tools/devnet_orchestrator.py init
```
*Hasil:* Direktori `data/devnet/val-1..4`, `sentry-1`, `rpc-gateway` dan `devnet_manifest.json` disiapkan.

### Langkah 3.2: Menjalankan Kluster Devnet
```powershell
python tools/devnet_orchestrator.py start
```
*Hasil:* 6 proses sub-simpul diluncurkan di latar belakang dengan log terpisah pada `data/devnet/<node_id>/node.log`.

### Langkah 3.3: Memeriksa Status Kluster
```powershell
python tools/devnet_orchestrator.py status
```
Output tabel real-time:
```
Node ID      | Role       | PID      | RPC Port  | Port Status  | Health
----------------------------------------------------------------------
val-1        | validator  | 14220    | 8545      | LISTENING    | UP
val-2        | validator  | 14228    | 8546      | LISTENING    | UP
val-3        | validator  | 14236    | 8547      | LISTENING    | UP
val-4        | validator  | 14244    | 8548      | LISTENING    | UP
sentry-1     | sentry     | 14252    | 8549      | LISTENING    | UP
rpc-gateway  | rpc        | 14260    | 8550      | LISTENING    | UP
```

### Langkah 3.4: Perintah CLI Terpadu Bawaan Aurion
Anda juga dapat memeriksa dan mengendalikan devnet langsung dari binary Aurion:
```powershell
aurion devnet init
aurion devnet status --output json
aurion devnet start --node-id val-1 --role validator --rpc-bind 127.0.0.1:8545 --p2p-bind 127.0.0.1:7447
```

### Langkah 3.5: Menghentikan Kluster
```powershell
python tools/devnet_orchestrator.py stop
```

---

## 4. Menjalankan via Docker Compose (Siap Disk D)

Jika Anda ingin menjalankan devnet dalam kontainer terisolasi di disk D (misalnya `D:\aurion-devnet\data`):

### Langkah 4.1: Persiapan Direktori Disk D
```powershell
New-Item -ItemType Directory -Force -Path "D:\aurion-devnet\data"
```

### Langkah 4.2: Menjalankan Docker Compose dengan Mount Disk D
```powershell
$env:AURION_DATA_DIR="D:/aurion-devnet/data"
docker compose -f docker-compose.devnet.yml up -d
```

### Langkah 4.3: Memantau Log Kontainer
```powershell
docker compose -f docker-compose.devnet.yml logs -f aurion-val-1
```

### Langkah 4.4: Menghentikan Kontainer Docker
```powershell
docker compose -f docker-compose.devnet.yml down
```

---

## 5. Pemeriksaan Kesehatan & Integrasi JSON-RPC

### Health Check Endpoint
Seluruh simpul menyediakan endpoint HTTP GET `/healthz`:
```bash
curl -i http://127.0.0.1:8545/healthz
```
*Response:*
```http
HTTP/1.1 200 OK
Content-Type: application/json

{"status":"UP","node_role":"FullNode","chain_id":1001,"version":"1.0.0"}
```

### JSON-RPC 2.0 Test Call
Kueri tinggi blok melalui RPC gateway publik:
```bash
curl -X POST http://127.0.0.1:8545 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"aur_blockHeight","params":[],"id":1}'
```

---

## 6. Verifikasi Suite Pengujian Terus-Menerus (CI/CD)

Suite pengujian integrasi otomatis `tests/devnet_continuous.rs` memverifikasi seluruh siklus hidup devnet:
- Inisialisasi 6 simpul dengan genesis bersama.
- Pelayanan HTTP `/healthz` pada validator dan gateway.
- Penyaluran transaksi melalui mempool dan pembentukan blok BFT.
- Verifikasi kuorum suara Prevote dan Precommit (>2/3 threshold).
- Konvergensi State Root 100% identik.
- Ketahanan terhadap crash simpul tunggal dan pemulihan tanpa fork.

Jalankan suite verifikasi:
```powershell
cargo test --test devnet_continuous
```

---

## 7. Kesimpulan & Rekomendasi Alur Kerja

| Kebutuhan | Lingkungan Rekomendasi | Cara Eksekusi |
| :--- | :--- | :--- |
| **Development & Test Harian** | **PC Lokal (Native)** | `python tools/devnet_orchestrator.py start` |
| **Integrasi Otomatis CI/CD** | **Local Test Runner** | `cargo test --test devnet_continuous` |
| **Staging Jangka Panjang / Disk D** | **Docker Compose** | `$env:AURION_DATA_DIR="D:/..."; docker compose up -d` |

> **PERINGATAN DOKTRIN ZERO-MOCK (PASAL 8 KONSTITUSI):**  
> Seluruh flag mode tiruan (`--dev`), simulasi proses kluster dalam 1 server publik, dan kunci privat hardcoded **diharamkan secara mutlak** pada biner produksi di server VPS publik (`116.212.72.89`). Biner produksi hanya boleh beroperasi menggunakan topologi P2P nyata dan `genesis.json` resmi.
