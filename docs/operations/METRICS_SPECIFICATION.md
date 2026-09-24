# AURION — Canonical Metrics & Observability Specification
> **Status:** Production Standard | **Protokol:** OpenMetrics / Prometheus Text v0.0.4  
> **Akses:** `GET /metrics` | **Integritas:** Integer Precision Only (Zero-Float)

---

## 1. Ikhtisar Arsitektur Telemetri

Sesuai dengan Dokumen 12 (*Operational Rules*) dan Invariant `AUR-ARCH-011`/`AUR-ARCH-012`, seluruh metrik telemetri yang diekspos oleh simpul Aurion harus mematuhi format teks **Prometheus OpenMetrics v0.0.4**, dapat di-scrape secara periodik oleh Prometheus/Grafana agent, dan **tidak boleh memuat representasi floating-point non-deterministik**.

### Karakteristik Utama:
- **Port Default:** `8545` (berbagi dengan gateway JSON-RPC & WebSocket).
- **HTTP Path:** `GET /metrics`
- **Content-Type:** `text/plain; version=0.0.4; charset=utf-8`
- **Metode Pengambilan:** Non-blocking atomic read (`Ordering::SeqCst` / `Ordering::Relaxed`).

---

## 2. Kamus Metrik Kanonikal (Canonical Metrics Dictionary)

| Nama Metrik | Tipe | Label | Deskripsi | SLA / Ambang Batas Normal |
| :--- | :---: | :---: | :--- | :---: |
| `aurion_block_height` | `gauge` | `chain_id` | Ketinggian blok kanonikal terkini pada ledger simpul lokal. | Bertambah tiap slot (~1 detik). |
| `aurion_bft_round` | `gauge` | `chain_id` | Putaran (*round*) konsensus BFT yang sedang aktif untuk slot saat ini. | `0` (putaran primer normal); alert jika $> 2$. |
| `aurion_bft_validators_active` | `gauge` | `chain_id` | Jumlah validator konsensus BFT yang terdaftar dan aktif dalam set. | $\ge 4$ (Genesis set $\mathcal{V}_0$). |
| `aurion_connected_peers` | `gauge` | `chain_id` | Jumlah koneksi P2P peer aktif yang telah melewati mutual handshake. | $\ge 3$ (Validator minimum); $\ge 10$ (Sentry). |
| `aurion_mempool_size` | `gauge` | `chain_id` | Jumlah transaksi pending yang tersimpan di dalam mempool in-memory. | $< 10,000$ transaksi. |
| `aurion_node_sync_status` | `gauge` | `chain_id` | Status sinkronisasi simpul (`1` = tersinkronisasi penuh, `0` = sedang fast-sync). | `1` pada simpul operasional. |
| `aurion_transactions_processed_total` | `counter` | `chain_id` | Akumulasi total transaksi yang berhasil dieksekusi dan dikomit ke ledger. | Monoton naik. |
| `aurion_blocks_finalized_total` | `counter` | `chain_id` | Akumulasi total blok yang memperoleh sertifikat komitmen BFT (`CommitCertificate`). | Monoton naik. |
| `aurion_validator_fees_total` | `counter` | `chain_id` | Akumulasi total unit Quantum fee transaksi yang dialirkan 100% ke validator pembuat blok. | Monoton naik (100% fee routing). |
| `aurion_bft_finality_latency_ms` | `gauge` | `chain_id` | Latensi waktu (milidetik) dari penerbitan proposal hingga kuorum precommit $>2/3$. | $< 1,000\text{ ms}$ (SLA Single-Slot Finality). |
| `aurion_active_protocol_version` | `gauge` | `chain_id` | Versi protokol konsensus aktif yang diakui oleh state machine. | Versi mayor saat ini (`1` untuk Mainnet Genesis). |

---

## 3. Format Output Contoh (`GET /metrics`)

```text
# HELP aurion_block_height Current canonical block height of the sovereign ledger.
# TYPE aurion_block_height gauge
aurion_block_height{chain_id="1001"} 12480

# HELP aurion_bft_round Current BFT consensus round.
# TYPE aurion_bft_round gauge
aurion_bft_round{chain_id="1001"} 0

# HELP aurion_bft_validators_active Number of active consensus validators.
# TYPE aurion_bft_validators_active gauge
aurion_bft_validators_active{chain_id="1001"} 4

# HELP aurion_connected_peers Number of active authenticated P2P peers.
# TYPE aurion_connected_peers gauge
aurion_connected_peers{chain_id="1001"} 16

# HELP aurion_mempool_size Number of pending transactions currently in the mempool.
# TYPE aurion_mempool_size gauge
aurion_mempool_size{chain_id="1001"} 142

# HELP aurion_node_sync_status Node synchronization status (1 = synced, 0 = syncing).
# TYPE aurion_node_sync_status gauge
aurion_node_sync_status{chain_id="1001"} 1

# HELP aurion_transactions_processed_total Total count of transactions processed and finalized.
# TYPE aurion_transactions_processed_total counter
aurion_transactions_processed_total{chain_id="1001"} 98450

# HELP aurion_blocks_finalized_total Total number of blocks committed to the ledger.
# TYPE aurion_blocks_finalized_total counter
aurion_blocks_finalized_total{chain_id="1001"} 12480

# HELP aurion_validator_fees_total Cumulative quanta fees routed 100% to block proposing validators.
# TYPE aurion_validator_fees_total counter
aurion_validator_fees_total{chain_id="1001"} 1969000000

# HELP aurion_bft_finality_latency_ms Single-slot BFT finality commit latency in milliseconds.
# TYPE aurion_bft_finality_latency_ms gauge
aurion_bft_finality_latency_ms{chain_id="1001"} 720

# HELP aurion_active_protocol_version Active on-chain protocol version.
# TYPE aurion_active_protocol_version gauge
aurion_active_protocol_version{chain_id="1001"} 1

# EOF
```

---

## 4. Konfigurasi Prometheus Scrape (`prometheus.yml`)

```yaml
global:
  scrape_interval: 1s
  evaluation_interval: 1s

scrape_configs:
  - job_name: 'aurion-mainnet'
    metrics_path: '/metrics'
    static_configs:
      - targets: ['127.0.0.1:8545']
        labels:
          role: 'validator'
          network: 'mainnet'
          region: 'ap-southeast'
```
