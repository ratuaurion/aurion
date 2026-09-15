# 12 — AURION PRODUCTION OPERATIONAL & SENTRY ARCHITECTURE RULES
## Standar Operasional Infrastruktur Produksi, Topologi Sentry, Pemantauan, dan Pemulihan Bencana

> **Hierarki Dokumen:**  
> `AURION CONSTITUTIONS` $\longrightarrow$ `PROTOCOL SPECIFICATIONS` $\longrightarrow$ `00-APPLICATION-RULES` $\longrightarrow$ **`12-OPERATIONAL-RULES`**  
>
> **Status:** RATIFIED & LOCKED APPLICATION SPECIFICATION  
> **Klasifikasi:** Standar Infrastruktur & Operasi Sistem (Production Infrastructure Standard)  
> **Versi:** 1.0.0-PROD  
> **Sifat Ketetapan:** Normatif Wajib (RFC 2119 / RFC 8174), High-Availability, Anti-DDoS

---

## 1. Topologi Infrastruktur Produksi dan Isolasi Validator (Sentry Architecture)

Validator konsensus Aurion-BFT memegang kunci penandatanganan konsensus yang berbobot tinggi dan rentan terhadap serangan penolakan layanan (DDoS) jika alamat IP publiknya terungkap.

Oleh karena itu, seluruh infrastruktur produksi **MUST** menerapkan **Topologi Arsitektur Sentry Node**:

```text
[INTERNET / JARINGAN P2P PUBLIK]
               │
               ▼
┌──────────────────────────────────────────────┐
│         LAPISAN SENTRY NODES PUBLIK          │
│  ├── Simpul Sentry 1 (IP Publik A)           │
│  ├── Simpul Sentry 2 (IP Publik B)           │
│  └── Simpul Sentry 3 (IP Publik C)           │
│  (Menyaring DDoS, Rate Limiting, P2P Scrub)  │
└──────────────────────┬───────────────────────┘
                       │ Jaringan Privat Terenkripsi (WireGuard / VPN)
                       ▼
┌──────────────────────────────────────────────┐
│       SIMPUL VALIDATOR INTI (PRIVATE CORE)   │
│  ├── Port P2P Hanya Mendengarkan Sentry      │
│  ├── Port RPC Publik DITUTUP TOTAL           │
│  └── Menandatangani Proposal & Suara BFT     │
└──────────────────────────────────────────────┘
```

> **MANDAT ISOLASI VALIDATOR (THE VALIDATOR ISOLATION MANDATE):**  
> Simpul Validator konsensus **MUST NOT** membuka antarmuka RPC publik atau menghubungkan alamat IP-nya secara langsung ke internet terbuka. Validator **HANYA BOLEH** terhubung ke simpul Sentry privat miliknya sendiri.

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
  3. Latensi pembacaan state database $\le 50\ \text{ms}$.
- Jika salah satu kriteria gagal, load balancer **MUST** mencabut simpul tersebut dari pool aktif secara instan (*Drain Traffic*).

---

## 3. Kebijakan Caching Deterministik (Caching Semantics)

1. **Data Final Imutabel (Finalized Blocks & Receipts):**  
   Blok, transaksi, dan tanda terima yang telah mengantongi Commit Certificate **MUST** dicache secara permanen pada level CDN, Reverse Proxy (Nginx/Envoy), atau Redis:
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

1. **Snapshot Database Atomik (Atomic State Snapshots):**  
   Pencadangan database state simpul (RocksDB / MDBX) **MUST** menggunakan mekanisme snapshot atomik tanpa mematikan simpul atau merusak integritas Sparse Merkle Tree.
2. **Uji Pemulihan Rutin (Recovery Drills):**  
   Operator infrastruktur enterprise **SHOULD** menguji prosedur pemulihan simpul dari snapshot sekurang-kurangnya sekali per kuartal dengan target **RTO (Recovery Time Objective) $\le 15\ \text{menit}$** dan **RPO (Recovery Point Objective) $\le 1\ \text{Blok}$**.
