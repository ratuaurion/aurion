# Aurion Application Rules Layer

> **Status:** RATIFIED APPLICATION SPECIFICATION  
> **Parent Architecture:** [Aurion Master Architecture (README.md)](../../README.md)  
> **Konformansi Normatif:** RFC 2119 / RFC 8174 (`MUST`, `MUST NOT`, `SHOULD`, `SHOULD NOT`, `MAY`)

Lapisan Aturan Aplikasi (*Application Rules Layer*) mendefinisikan batasan operasional, semantik integrasi, dan kewajiban perilaku yang mengikat seluruh aplikasi klien, wallet, antarmuka RPC, penjelajah blok (explorer), pengindeks (indexer), SDK, dan layanan pertukaran (exchange) yang beroperasi di atas protokol Aurion.

Sesuai dengan prinsip **Single Ecosystem / Single Binary Architecture**, aturan-aturan ini bukan merupakan spesifikasi untuk ekosistem software independen yang terpisah, melainkan aturan operasional kanonikal yang dijamin dan dieksekusi secara terpadu oleh binary utama `/bin/aurion` dan modul-modul terkait.

---

## Indeks Spesifikasi Aturan Aplikasi (Dokumen 00 s/d 17)

### Bagian I: Aturan Aplikasi & Integrasi Klien (Dokumen 00 s/d 13)
| Dokumen | Judul | Fokus Utama |
| :--- | :--- | :--- |
| **[00-APPLICATION-RULES.md](application/00-APPLICATION-RULES.md)** | Arsitektur & Prinsip Umum | Batas boundary protokol-aplikasi, 3 Compliance Tiers (Tier 1 Consensus, Tier 2 Operational, Tier 3 Client), dan prinsip non-reimplementsi. |
| **[01-WALLET-RULES.md](application/01-WALLET-RULES.md)** | Aturan Aplikasi Wallet | Siklus hidup kunci (Argon2id + ChaCha20-Poly1305), jalur derivasi BIP-44 `m/44'/9999'/account'/0/index`, mitigasi blind signing (Clear Signing Mandate), manajemen nonce, estimasi fee, dan perhitungan saldo spendable. |
| **[02-RPC-API-RULES.md](application/02-RPC-API-RULES.md)** | Antarmuka RPC & API | Standarisasi JSON-RPC 2.0 namespace `aur_`, skema request/response, konsistensi data (`latest`, `safe`, `finalized`), rate limiting, dan WebSocket streaming (`aur_subscribe`). |
| **[03-TRANSACTION-LIFECYCLE.md](application/03-TRANSACTION-LIFECYCLE.md)** | Siklus Hidup Transaksi | Mesin state transaksi formal: `CREATED -> SIGNED -> SUBMITTED -> MEMPOOL -> INCLUDED -> FINALIZED`, penanganan kegagalan (`REJECTED`, `EXPIRED`, `DROPPED`, `REPLACED`), dan format receipt kanonikal. |
| **[04-FINALITY-CONFIRMATION-RULES.md](application/04-FINALITY-CONFIRMATION-RULES.md)** | Aturan Konfirmasi & Finalitas | Single-slot BFT finality semantics, batasan tegas kredit deposit bursa (hanya pada `FINALIZED`), pelepasan barang dagangan merchant, dan penanganan reorg mitigasi. |
| **[05-ADDRESS-ACCOUNT-RULES.md](application/05-ADDRESS-ACCOUNT-RULES.md)** | Aturan Alamat & Akun | Format Bech32m (`aur1...` / `aurt1...`), normalisasi huruf kecil, representasi QR code Alphanumeric Uppercase, URI Scheme BIP-21 `aurion:<address>?amount=<val>&memo=<ref>`, dan deteksi typo/checksum. |
| **[06-FEE-PAYMENT-RULES.md](application/06-FEE-PAYMENT-RULES.md)** | Aturan Biaya & Pembayaran | Triad fee (`estimated_fee`, `maximum_fee`, `actual_fee`), pengungkapan transparansi pembakaran 20% koin, penanganan otomatis toleransi pembayaran (exact payment, underpayment, overpayment), dan protokol pengembalian dana. |
| **[07-PAYMENT-REFERENCE-RULES.md](application/07-PAYMENT-REFERENCE-RULES.md)** | Standarisasi Memo Pembayaran | Batasan ukuran memo ($\le 64$ byte standar, maks 256 byte), encoding UTF-8 / Hex, skema prefiks (`USR:`, `INV:`, `REF:`, `TXT:`), larangan keras data identitas pribadi (PII), dan semantik pengindeksan. |
| **[08-EXPLORER-INDEXER-RULES.md](application/08-EXPLORER-INDEXER-RULES.md)** | Aturan Explorer & Indexer | Prinsip Nol-Fabrikasi (*Zero-Fabrication Mandate*), pemisahan ingestion dual-track (*finalized* vs *speculative*), penelusuran provenance RPI (Re-org Protected Ingestion), dan integritas data historis. |
| **[09-SDK-RULES.md](application/09-SDK-RULES.md)** | Spesifikasi & Standar SDK | 9 modul wajib kanonikal (`client`, `wallet`, `transaction`, `signing`, `rpc`, `address`, `amount`, `fee`, `codec`), identikalitas byte deterministik antar bahasa pemrograman (Rust, Go, Python, TypeScript), dan larangan floating-point. |
| **[10-ERROR-MODEL.md](application/10-ERROR-MODEL.md)** | Model Kesalahan Terstruktur | Taksonomi error mesin terstruktur dengan rentang kode unik (1000-6999: `TRANSACTION_ERROR`, `CONSENSUS_ERROR`, `RPC_ERROR`, `NETWORK_ERROR`, `APPLICATION_ERROR`) dan skema respons error JSON-RPC standard. |
| **[11-INTEGRATION-RULES.md](application/11-INTEGRATION-RULES.md)** | Aturan Integrasi Ekosistem | Pedoman arsitektur custody bursa kripto (95% Cold Storage / 5% Hot Wallet), penarikan dana dengan distributed lock / mutex, skema multi-signature/TSS (FROST / MuSig2), dan verifikasi SPV/Light Client untuk bridge lintas chain. |
| **[12-OPERATIONAL-RULES.md](application/12-OPERATIONAL-RULES.md)** | Aturan Operasional Produksi | Arsitektur Sentry Node, isolasi ketat validator tanpa public IP, endpoint pemeriksaan kesehatan mendalam (`/healthz/deep`), metrik Prometheus global, dan protokol mitigasi DoS. |
| **[13-COMPATIBILITY-VERSIONING.md](application/13-COMPATIBILITY-VERSIONING.md)** | Kompatibilitas & Versioning | Model versioning independen 4-dimensi (Protokol, RPC, SDK, Database Schema), aturan SemVer 2.0.0, jendela depresiasi minimal 180 hari, dan siklus rilis terkoordinasi. |

### Bagian II: Kontrak Arsitektur Sistem Inti L1 (Dokumen 14 s/d 16)
| Dokumen | Judul | Fokus Utama |
| :--- | :--- | :--- |
| **[14-STORAGE-PERSISTENCE-SPECIFICATION.md](application/14-STORAGE-PERSISTENCE-SPECIFICATION.md)** | Mesin Penyimpanan & Persistensi | Kontrak penyimpanan murni Rust `redb 4.3` ACID transactional multi-table commit, zero C++ library, pemulihan crash deterministik, dan isolasi storage API dari storage engine. |
| **[15-UNIFIED-CLI-SPECIFICATION.md](application/15-UNIFIED-CLI-SPECIFICATION.md)** | Unified CLI & Application Control Plane | Antarmuka operasional tunggal `/bin/aurion` mengontrol node, validator, wallet, storage, account, block, genesis, dan conformance dengan output ganda (Human Text & Machine JSON `--output json`). |
| **[16-SMART-CONTRACT-EXECUTION-SPECIFICATION.md](application/16-SMART-CONTRACT-EXECUTION-SPECIFICATION.md)** | Smart Contract & Execution Layer (AVM) | Aurion Native Virtual Machine (AVM) 256-bit word stack, 48 opcodes deterministik, zero float, gas metering integer exact, verifier bytecode statis, dan rollback state atomik. |

### Bagian III: Blueprint Arsitektur Evolusi & Horizon L2/L3 (Dokumen 17)
| Dokumen | Judul | Fokus Utama |
| :--- | :--- | :--- |
| **[17-L2-EVOLUTION-ARCHITECTURE-BLUEPRINT.md](application/17-L2-EVOLUTION-ARCHITECTURE-BLUEPRINT.md)** | Blueprint L2 Scaling & Settlement | Ekstensi kapasitas eksekusi horizontal, kompresi batch, L1 Settlement Bridge pada AVM, Data Availability (DA), Validity/Fraud proofs, invariant `L2-*`, kriteria transisi gerbang, dan model pengukuran progres multi-layer terukur. |

---

## Kerangka Kerja Tiga Cakupan (Multi-Layer Evolution Framework)

```text
                    APPLICATION RULES LAYER
                              │
        ┌─────────────────────┼─────────────────────┐
        │                     │                     │
     L1 RULES              L2 RULES              L3 RULES
  (Current Scope)       (Planned Scope)       (Future Scope)
  Dokumen 00 s/d 16       Dokumen 17          Horizon L3+
        │                     │                     │
  Reference Impl        L2 Architecture       App-Specific
  64/64 Tests PASS       Blueprint 100%       Rollup Domains
```

---

Untuk kembali ke dokumentasi fondasi konsensus dan konstitusi protokol, lihat [`docs/Constitutions/`](../Constitutions/).
