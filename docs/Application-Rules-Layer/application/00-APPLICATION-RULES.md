# 00 — AURION APPLICATION RULES ARCHITECTURE & COMPLIANCE MANDATE
## Kerangka Kerja, Hierarki Kedaulatan, dan Konvensi Normatif Application Layer Protokol Aurion

> **Hierarki Dokumen:**  
> `AURION CONSTITUTIONS` $\longrightarrow$ `PROTOCOL SPECIFICATIONS (01-12)` $\longrightarrow$ **`APPLICATION RULES LAYER (00-13)`**  
>
> **Status:** RATIFIED & LOCKED APPLICATION SPECIFICATION  
> **Klasifikasi:** Arsitektur Perangkat Lunak, API, Wallet, RPC, dan Layanan Layer 2 (Application Boundary)  
> **Versi:** 1.0.0-PROD  
> **Sifat Ketetapan:** Normatif Wajib (RFC 2119 / RFC 8174), Deterministik, Anti-Fragmentasi Ekosistem

---

## 1. Posisi dan Batasan Arsitektur (Architectural Boundary)

Application Rules Layer mendefinisikan batasan operasional dan perilaku yang mengikat seluruh perangkat lunak yang berinteraksi di atas protokol Aurion:

```text
                    AURION CONSTITUTION (00)
                              │
                    PROTOCOL SPECIFICATIONS (01-12)
                    (Consensus, STF, Moneter, Wire)
                              │
            ═════════════════════════════════════════════
                      APPLICATION RULES LAYER (00-13)
            ═════════════════════════════════════════════
                              │
        ┌─────────────────────┼─────────────────────┐
        ▼                     ▼                     ▼
   CLIENT APPLICATIONS    INTEGRATION & RPC       ECOSYSTEM SERVICES
   ├── Desktop Wallet     ├── Node RPC / JSON-RPC ├── Block Explorer
   ├── Mobile Wallet      ├── WebSocket Stream    ├── Data Indexer
   ├── Hardware Wallet    ├── Client SDKs         ├── Payment Gateway
   └── Custody Systems    └── Gateway Middleware  └── Exchange Engine
```

### 1.1 Pemisahan Domain Konsensus dan Domain Aplikasi
1. **Domain Protokol (L0/L1):** Menjawab pertanyaan *"Apakah blok atau transaksi ini sah secara matematis dan kriptografis menurut konsensus Aurion-BFT?"*
2. **Domain Aplikasi (L2):** Menjawab pertanyaan *"Bagaimana wallet, exchange, RPC, dan service mengonstruksi, mengirimkan, memverifikasi, menampilkan, dan mengelola data tersebut secara aman, konsisten, dan dapat diaudit?"*

Setiap aplikasi ekosistem dilarang keras menciptakan interpretasi privat atau aturan ad-hoc yang bertentangan dengan invarian protokol.

---

## 2. Konvensi Bahasa Normatif (RFC 2119 / RFC 8174)

Seluruh dokumen dalam **Application Rules Layer (00 s/d 13)** menggunakan istilah normatif baku berikut:

- **MUST / WAJIB / SHALL:** Menandakan persyaratan mutlak (*absolute requirement*). Ketidakpatuhan terhadap klausa ini menyebabkan perangkat lunak diklasifikasikan sebagai **NON-CONFORMANT** dan berbahaya bagi keamanan pengguna.
- **MUST NOT / DILARANG KERAS / SHALL NOT:** Menandakan larangan mutlak (*absolute prohibition*).
- **SHOULD / SANGAT DISARANKAN / RECOMMENDED:** Menandakan praktik rekayasa terbaik yang wajib diikuti kecuali terdapat alasan teknis valid yang terdokumentasi dan dijustifikasi.
- **SHOULD NOT / TIDAK DISARANKAN / NOT RECOMMENDED:** Menandakan praktik yang berpotensi memicu kerentanan atau degradasi performa dan harus dihindari.
- **MAY / DIIZINKAN / OPTIONAL:** Menandakan fitur pilihan yang sepenuhnya opsional tanpa merusak interoperabilitas sistem lain.

---

## 3. Peta Modul Application Rules Layer

Application Rules Layer terbagi ke dalam 14 spesifikasi resmi:

| Kode Dokumen | Judul Spesifikasi | Fokus dan Ruang Lingkup |
| :--- | :--- | :--- |
| **`00-APPLICATION-RULES`** | Arsitektur & Mandat Kepatuhan | Hierarki kedaulatan, definisi normatif, dan tingkat sertifikasi kepatuhan ekosistem. |
| **`01-WALLET-RULES`** | Aturan Operasional Dompet (Wallet) | Siklus kunci, derivasi Bech32m, estimasi fee, nonce counter, dan status konfirmasi. |
| **`02-RPC-API-RULES`** | Standar Antarmuka RPC & API | Skema JSON-RPC, semantik status `latest`/`safe`/`finalized`, rate limits, dan WebSocket. |
| **`03-TRANSACTION-LIFECYCLE`** | Siklus Hidup Transaksi Aplikasi | State machine transaksi dari `CREATED` hingga `FINALIZED`, RBF, dan penanganan kegagalan. |
| **`04-FINALITY-CONFIRMATION-RULES`**| Aturan Finalitas & Konfirmasi | Ambang batas kredit exchange, rilis barang merchant, dan penanganan partisi jaringan. |
| **`05-ADDRESS-ACCOUNT-RULES`** | Standar Alamat & Skema URI | Normalisasi display Bech32m, QR code, dan skema URI universal `aurion:...`. |
| **`06-FEE-PAYMENT-RULES`** | Estimasi Biaya & Pembayaran | Alokasi 100% fee ke validator, toleransi under/overpayment, dan batas kedaluwarsa invoice. |
| **`07-PAYMENT-REFERENCE-RULES`** | Standar Memo & Referensi Tag | Payload memo biner/teks, identifikasi invoice exchange, dan perlindungan privasi. |
| **`08-EXPLORER-INDEXER-RULES`** | Aturan Explorer & Indexer | Integritas pengindeksan rantai kanonikal, rekonsiliasi reorg, dan larangan fabrikasi fakta. |
| **`09-SDK-RULES`** | Spesifikasi Pustaka Klien (SDK) | Arsitektur seragam pustaka multi-bahasa (Rust, Go, Python, TypeScript). |
| **`10-ERROR-MODEL`** | Model Galat Terpadu Mesin | Katalog kode galat baku machine-readable lintas protokol, RPC, dan aplikasi. |
| **`11-INTEGRATION-RULES`** | Panduan Integrasi Institusional | Pedoman integrasi bursa aset kripto (CEX), payment processor, dan layanan kustodian. |
| **`12-OPERATIONAL-RULES`** | Standar Operasi & Keandalan | Health check simpul, load balancer, caching deterministik, monitoring, dan circuit breaker. |
| **`13-COMPATIBILITY-VERSIONING`** | Kompatibilitas & Versioning | Matriks versi Protokol vs RPC vs SDK vs Skema DB, serta siklus depresiasi terencana. |

---

## 4. Tiga Tingkat Sertifikasi Kepatuhan Aplikasi (Compliance Tiers)

Untuk menjamin kualitas dan keamanan perangkat lunak pihak ketiga di ekosistem Aurion:

```text
┌────────────────────────────────────────────────────────────────────────┐
│               TINGKAT KEPATUHAN PERANGKAT LUNAK AURION                 │
├──────────────┬─────────────────────────────────────────────────────────┤
│ TIER 1       │ CLIENT CONFORMANCE (Wallet, UI, Light Apps)             │
│ (Dasar)      │ Mematuhi derivasi alamat, format display, validasi      │
│              │ input, pencegahan penyiaran ganda, dan transparansi fee.│
├──────────────┼─────────────────────────────────────────────────────────┤
│ TIER 2       │ SERVICE CONFORMANCE (RPC, Indexer, Explorer, SDK)       │
│ (Menengah)   │ Mematuhi pembedaan status finalitas, penanganan error   │
│              │ baku, serialisasi kanonikal, dan konsistensi data P2P.  │
├──────────────┼─────────────────────────────────────────────────────────┤
│ TIER 3       │ ENTERPRISE CONFORMANCE (CEX, Kustodian, Payment Gateway)│
│ (Tertinggi)  │ Mematuhi aturan kredit mutlak pasca-finalitas BFT,      │
│              │ rekonsiliasi saldo zero-float, dan audit provenance.    │
└──────────────┴─────────────────────────────────────────────────────────┘
```

Setiap entitas pengembang atau penyedia layanan di atas jaringan Aurion **MUST** mengacu pada spesifikasi ini sebagai dasar audit teknis dan sertifikasi integrasi.
