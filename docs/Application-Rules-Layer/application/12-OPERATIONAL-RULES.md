# 12 — AURION PRODUCTION OPERATIONAL & SENTRY ARCHITECTURE RULES
## Standar Operasional Infrastruktur Produksi, Topologi Bootnode/Sentry, Pemantauan, dan Doktrin Zero-Mock

> **Hierarki Dokumen:**  
> `AURION CONSTITUTION` $\longrightarrow$ `PROTOCOL SPECIFICATIONS` $\longrightarrow$ `00-APPLICATION-RULES` $\longrightarrow$ **`12-OPERATIONAL-RULES`**  
>
> **Status:** RATIFIED & LOCKED APPLICATION SPECIFICATION  
> **Klasifikasi:** Standar Infrastruktur & Operasi Sistem (Production Infrastructure Standard)  
> **Versi Protokol:** 1.0.0-BFT  
> **Sifat Ketetapan:** Normatif Wajib (RFC 2119 / RFC 8174), High-Availability, Anti-DDoS, Zero-Mock

---

## 1. Topologi Infrastruktur Produksi dan Pemisahan Peran Node

Mengukuhkan amanat **Bab III dan Bab V [CONSTITUTION.md](file:///c:/Projects/aurion/CONSTITUTION.md)**, infrastruktur produksi Aurion beroperasi di atas pemisahan peran tiga lapis yang ketat:

```text
┌──────────────────┐               ┌────────────────────────┐               ┌────────────────────┐
│ Validator Nodes  │ <---(P2P)---> │  P2P Anchor / Bootnode │ <---(P2P)---> │ Gateway / Explorer │
│  (Ed25519 BFT)   │               │   (TCP Port 7447)      │               │ (HTTP/WS Port 8080)│
└──────────────────┘               └────────────────────────┘               └────────────────────┘
                                               │                                       │
                                    Alamat: 116.212.72.89                   Nginx SSL Reverse Proxy
                                    (Isolasi Signing Key)                  bootnode.ratuaurion.store
```

### 1.1 Node Jangkar / P2P Bootnode (`116.212.72.89`)
1. **Soket Pendengar Persisten:** Wajib membuka soket pendengar TCP (`TCP Listener`) pada **Port 7447** secara persisten untuk melayani jabat tangan masuk (*inbound handshake*) dari validator luar.
2. **Isolasi Kunci Konsensus:** Bootnode **DILARANG KERAS** menyimpan atau mengakses kunci privat penandatangan konsensus (*consensus signing key*). Jika bootnode diserang atau dikompromikan, kuorum voting BFT tetap aman.
3. **Fungsi Utama:** Melayani penemuan simpul (*peer discovery*), pertukaran peer (*PEX*), dan relai pesan konsensus/transaksi.

### 1.2 Node Validator Inti (Private Core)
1. **Fungsi Konsensus:** Menjalankan mesin status konsensus BFT (*Proposal*, *Prevote*, *Precommit*, *Commit*).
2. **Kunci Rahasia:** Menyimpan pasangan kunci privat Ed25519 untuk menandatangani proposal dan suara blok.
3. **MANDAT ISOLASI PORT VALIDATOR:**  
   Port RPC/Gateway validator **MUST NOT** diekspos ke internet publik. Seluruh komunikasi validator **HANYA BOLEH** berjalan melalui saluran P2P terenkripsi ke Bootnode atau sesama peer terverifikasi.

### 1.3 Gateway API & Telemetri
1. **Layanan Aplikasi:** Melayani kueri status ledger, data blok, histori transaksi, dan stream WebSocket telemetri ke penjelajah blok (`aurion-explorer`).
2. **Integrasi Nginx SSL:** Gateway lokal mengikat port internal **`127.0.0.1:8080`**, yang diteruskan oleh Nginx dengan sertifikat SSL resmi ke domain publik **`bootnode.ratuaurion.store`**.

---

## 2. Standar Pemeriksaan Kesehatan Sistem (Health Checks)

Simpul gateway dan penyedia layanan RPC **MUST** mengekspos dua endpoint pemantauan kesehatan standar:

### 2.1 Pemeriksaan Ringan (Shallow Liveness Check: `/healthz`)
- **Tujuan:** Mengetahui apakah proses daemon simpul berjalan dan port TCP terbuka.
- **Kriteria Lulus (HTTP 200):** Proses aktif dan dapat menerima panggilan internal.

### 2.2 Pemeriksaan Mendalam (Deep Readiness Check: `/healthz/deep`)
- **Tujuan:** Mengetahui apakah simpul siap melayani lalu lintas aplikasi secara akurat.
- **Kriteria Lulus (HTTP 200):**
  1. Jumlah peer aktif $\ge 3$ koneksi P2P.
  2. Selisih tinggi blok lokal terhadap ujung rantai jaringan $\le 1$ blok (`is_syncing == false`).
  3. Latensi pembacaan state database ACID `redb` $\le 50\ \text{ms}$.
- Jika salah satu kriteria gagal, load balancer **MUST** mencabut simpul tersebut dari pool aktif secara instan (*Drain Traffic*).

---

## 3. Kebijakan Caching Deterministik (Caching Semantics)

1. **Data Final Imutabel (Finalized Blocks & Receipts):**  
   Blok, transaksi, dan tanda terima yang telah mengantongi Quorum Certificate (QC) **MUST** dicache secara permanen pada level CDN, Reverse Proxy (Nginx), atau Redis:
   ```http
   Cache-Control: public, max-age=31536000, immutable
   ```
2. **Data Dinamis (State Akun & Mempool):**  
   Permintaan terhadap saldo akun (`aur_getBalance`) atau mempool (`aur_getMempool`) **MUST NOT** dicache melampaui batas waktu 1 detik untuk mencegah penyajian data usang (*stale state*).

---

## 4. Metrik Observabilitas Prometheus Baku (Observability Metrics)

Setiap simpul operasional produksi **MUST** mengekspos metrik standar pada port `/metrics` format Prometheus:

| Nama Metrik | Tipe | Deskripsi & Ambang Batas Peringatan |
| :--- | :--- | :--- |
| `aurion_node_block_height` | Gauge | Ketinggian blok lokal saat ini. |
| `aurion_node_peer_count` | Gauge | Jumlah koneksi peer P2P aktif (Alert jika $< 3$). |
| `aurion_bft_current_round` | Gauge | Nomor putaran konsensus (Alert jika $> 2$). |
| `aurion_mempool_size_bytes` | Gauge | Ukuran memori mempool transaksi. |
| `aurion_stf_execution_duration_seconds` | Histogram | Waktu eksekusi STF (Alert jika p99 $> 500\ \text{ms}$). |
| `aurion_rpc_requests_total` | Counter | Total volume panggilan RPC berdasarkan metode & status code. |

---

## 5. Prosedur Pencadangan dan Pemulihan Bencana (Backup & Disaster Recovery)

1. **Snapshot Database Atomik ACID (`redb`):**  
   Pencadangan database state simpul menggunakan mesin basis data bertipe ACID murni Rust **`redb`** (`Rule 14`) melalui mekanisme snapshot atomik tanpa mematikan simpul atau merusak integritas Sparse Merkle Tree (SMT).
2. **Uji Pemulihan Rutin (Recovery Drills):**  
   Operator infrastruktur enterprise **SHOULD** menguji prosedur pemulihan simpul dari snapshot `.auss` sekurang-kurangnya sekali per kuartal dengan target **RTO $\le 15\ \text{menit}$** dan **RPO $\le 1\ \text{Blok}$**.

---

## 6. Doktrin Integritas Kode & Kebijakan Anti-Tiruan (Zero-Mock Policy)

Sesuai **Pasal 8 dan Pasal 10 Konstitusi Protokol Aurion**:

1. **Larangan Mutlak Mode Tiruan:**  
   Segala bentuk flag mode tiruan (`--dev`), *mock consensus*, kluster proses simulasi dalam satu server fisik publik, dan kunci privat hardcoded **diharamkan secara mutlak** dari lingkungan produksi.
2. **Verifikasi Lingkungan Publik:**  
   Lingkungan publik VPS (`116.212.72.89`) hanya boleh mengeksekusi biner dengan arsitektur jaringan P2P nyata dan konfigurasi `genesis.json` resmi.
3. **Integritas Produk:**  
   Setiap upaya memasukkan kode tiruan atau bypass konsensus ke jaringan publik dianggap sebagai cacat integritas produk (*architectural contamination*) yang membatalkan akreditasi simpul.
