# AURION MAINNET — Post-Launch Operations, Observability & Incident Runbook
> **Standard:** Production Grade | **Era:** Era VI (Step 17: Post-Mainnet Operations)  
> **Klasifikasi:** Dokumen Panduan Operator Jaringan, Validator, & Bursa Global

---

## 1. Arsitektur Operasional Pasca Peluncuran

Setelah peluncuran sovereign Mainnet (`Chain ID: 1001`), kestabilan dan kedaulatan jaringan dijaga melalui 3 pilar operasional:
1. **Observabilitas Waktu-Nyata (Real-Time Observability):** Telemetri Prometheus via `GET /metrics` dan dasbor Grafana terpadu.
2. **Pemeriksaan Kesehatan Berlapis (Tiered Health Probes):** Shallow liveness (`/healthz`) untuk ingress load balancer dan deep readiness (`/healthz/deep`) untuk konsensus dan penyimpanan.
3. **Penanganan Insiden & Pemulihan Bencana (Incident Response & Disaster Recovery):** Mekanisme Circuit Breaker otomatis dan pemulihan cepat berbasis snapshot `.auss`.
4. **Tata Kelola Peningkatan On-Chain (On-Chain Fork Signaling):** Pensinyalan bit versi pada BlockHeader untuk softfork/hardfork tanpa pemisahan rantai (chain split).

---

## 2. Setup Pemantauan & Observabilitas (Prometheus & Grafana)

### 2.1. Prometheus Scraping Target
Simpul Aurion mengekspos metrik OpenMetrics standar pada endpoint `/metrics` di port RPC (`8545`).

Tambahkan target berikut pada `prometheus.yml`:
```yaml
scrape_configs:
  - job_name: 'aurion-mainnet'
    scrape_interval: 1s
    metrics_path: '/metrics'
    static_configs:
      - targets: ['127.0.0.1:8545']
        labels:
          role: 'validator'
          network: 'aurion-mainnet'
```

### 2.2. Grafana Dashboard Import
1. Buka antarmuka Grafana (`http://localhost:3000`).
2. Masuk ke menu **Dashboards** $\to$ **New** $\to$ **Import**.
3. Unggah file [`MAINNET_DASHBOARD.json`](file:///c:/Projects/aurion/MAINNET_DASHBOARD.json).
4. Pilih data source Prometheus Aurion. Panel metrik latensi BFT, TPS, ukuran mempool, dan tinggi blok akan aktif seketika.

---

## 3. Matriks Aturan Peringatan (Alerting Rules Matrix)

Simpan aturan ini di Prometheus Alertmanager (`aurion_alerts.yml`):

| Nama Alert | Kondisi Evaluasi | Tingkat Keparahan | Tindakan Operasional SOP |
| :--- | :--- | :---: | :--- |
| `AurionConsensusStall` | `increase(aurion_block_height[30s]) == 0` | **CRITICAL** | Periksa konektivitas P2P antar validator; pastikan kuorum $>2/3$ aktif. |
| `AurionBftHighRound` | `aurion_bft_round > 2` | **WARNING** | Deteksi adanya kegagalan proposer atau latensi jaringan WAN tinggi. |
| `AurionBftHighLatency` | `aurion_bft_finality_latency_ms > 1000` | **WARNING** | Evaluasi I/O disk database `redb 4.3` dan throughput sentry node. |
| `AurionLowPeers` | `aurion_connected_peers < 3` | **CRITICAL** | Periksa firewall sentry node; verifikasi IP bootnodes kanonikal. |
| `AurionMempoolCongestion` | `aurion_mempool_size > 8000` | **WARNING** | Pantau gas market dan dinamika pemisahan fee 20% burn / 80% miner. |
| `AurionStorageDegraded` | `aurion_node_sync_status == 0` | **HIGH** | Node tertinggal dari ujung rantai; lakukan sinkronisasi catchup atau snapshot. |

---

## 4. Standar Operasional Prosedur (SOP) Penanganan Insiden

### SOP 1: Penanganan Pemutus Sirkuit Darurat (Circuit Breaker Tripped)
Jika terjadi partisi jaringan ekstrem atau kegagalan putaran konsensus berturut-turut melebihi ambang batas, Circuit Breaker akan memutus siklus mutasi:
```powershell
# 1. Periksa status Circuit Breaker
aurion recovery status --output json

# 2. Periksa audit integritas ledger lokal
aurion recovery audit --output json

# 3. Setelah konektivitas pulih, lakukan reset operasional
aurion recovery reset
```

### SOP 2: Pemulihan Bencana dari Snapshot State (.auss Fast-Sync)
Jika terjadi kerusakan perangkat keras disk atau kegagalan node fatal:
```powershell
# 1. Unduh paket snapshot kanonikal resmi bertanda tangan
# 2. Pulihkan state store ACID redb secara instan
aurion recovery restore --snapshot /var/lib/aurion/snapshots/mainnet_snapshot_latest.auss

# 3. Jalankan kembali node sinkronisasi
aurion node start
```

### SOP 3: Rotasi Kunci Validator (Validator Key Rotation Drill)
1. Buat pasangan kunci Ed25519 baru menggunakan dompet berstandar SLIP-0010:
   ```powershell
   aurion wallet generate --output json
   ```
2. Salin kunci publik baru dan daftarkan pada konfigurasi validator:
   Perbarui entri `validator_keypair` di [`MAINNET_CONFIG.toml`](file:///c:/Projects/aurion/MAINNET_CONFIG.toml).
3. Lakukan restart anggun (*graceful restart*) simpul validator di antara slot konsensus yang aman.

---

## 5. Tata Kelola Peningkatan Protokol On-Chain (On-Chain Fork Governance)

Aurion menerapkan tata kelola deterministik bebas perpecahan rantai (*chain-split resistant*) melalui mekanisme bit-signaling pada `BlockHeader.version`:

1. **Pendaftaran Proposal:**
   ```powershell
   aurion governance propose --id 2 --name "Dynamic Storage Gas Adjustment" --version 2 --bit 2 --start 10000 --window 1000 --activation 12000
   ```
2. **Pensinyalan Validator:**
   Validator yang menyetujui mengaktifkan bit pensinyalan pada konfigurasi simpulnya:
   ```powershell
   aurion governance signal --bit 2
   ```
3. **Evaluasi & Penguncian:**
   Jika pada akhir jendela evaluasi blok dukungan validator $\ge 80.00\%$ ($8,000$ bps), status proposal menjadi `LockedIn`.
4. **Aktivasi Otomatis:**
   Pada ketinggian blok `activation_height`, state machine secara otomatis mengaktifkan aturan baru (`active_protocol_version = 2`) secara deterministik tanpa intervensi manual.
