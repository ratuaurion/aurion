# AURION — Single Ecosystem & Single Binary Architecture
### Tunggak Pokok Arsitektural Ekosistem Aplikasi Aurion

> **Status Dokumen:** RATIFIED ARCHITECTURAL INVARIANT  
> **Klasifikasi:** Fondasi Inti Ekosistem Aplikasi (*Foundational Blueprint*)  
> **Kepatuhan:** Normatif Ketat (RFC 2119 / RFC 8174 — `MUST`, `MUST NOT`, `SHOULD`, `MAY`)  
> **Distribusi:** Single Authoritative Source Tree & Single Primary Binary (`/bin/aurion`)

---

## 1. Manifesto Fundamental: Sistem Berdaulat Terpadu

Aurion dirancang dengan satu prinsip arsitektural yang tidak dapat dikompromikan:
> **Aurion bukan sekadar monorepo, dan bukan sekadar "sebuah blockchain yang dikelilingi oleh sekumpulan aplikasi satelit independen". Aurion adalah satu ekosistem perangkat lunak berdaulat tunggal (*single integrated sovereign software system*). Seluruh kapabilitas dibangun sebagai modul internal yang terikat secara organik, dan hasil build production akhirnya adalah satu produk dan satu executable/binary utama.**

Pendekatan industri konvensional yang memecah blockchain menjadi repositori terpisah, dependensi tidak selaras, binary yang bercabang (`node`, `wallet-cli`, `validator-daemon`, `rpc-server`, `indexer-worker`), dan implementasi ulang logika protokol di berbagai tempat telah terbukti menghasilkan kerentanan konsensus, celah keamanan, dan fragmentasi ekosistem.

Aurion mengakhiri fragmentasi tersebut di tingkat fondasi:
1. **Satu Pohon Sumber (*Single Source Tree*):** Tidak ada *hidden second ecosystem*.
2. **Satu Identitas Versi (*Single Version Identity*):** Rilis Aurion mencakup seluruh modul secara atomik.
3. **Satu Model Data & Runtime (*Single Data Model & Runtime*):** Tipe data primitif, representasi moneter, aturan kriptografi, dan state transition didefinisikan satu kali dan digunakan bersama oleh seluruh subsistem.
4. **Satu Binary Utama (*Single Primary Executable*):** Seluruh kapabilitas operasional dijalankan melalui `/bin/aurion`.

---

## 2. Model Arsitektur Aurion

Topologi arsitektur sistem Aurion memusatkan seluruh fungsionalitas di bawah satu executable utama yang mengoperasikan modul-modul internal di atas fondasi inti bersama (*Shared Core*):

```text
                         AURION
                           │
                    ┌──────┴──────┐
                    │  AURION BIN │
                    │   SINGLE    │
                    │  EXECUTABLE │
                    └──────┬──────┘
                           │
        ┌──────────────────┼──────────────────┐
        │                  │                  │
     Consensus          Execution           State
        │                  │                  │
     Mempool            Crypto             Storage
        │                  │                  │
      Network              RPC             Validator
        │                  │                  │
      Wallet             Indexer          Governance
        │                  │                  │
        └──────────────────┼──────────────────┘
                           │
                     Shared Core
                           │
                  ┌────────┴────────┐
                  │ Protocol Types  │
                  │ Primitives      │
                  │ Errors          │
                  │ Config          │
                  │ Runtime         │
                  └─────────────────┘
```

> **Aksioma Inti:** Seluruh komponen di atas adalah **modul internal**, bukan produk perangkat lunak yang berdiri sendiri atau bercabang.

---

## 3. Delapan Prinsip Pokok Arsitektur

### Prinsip 1: Single Ecosystem (Ekosistem Tunggal Berdaulat)
Aurion tidak mengakui pemisahan produk software yang saling bersaing atau memiliki siklus hidup independen dalam infrastruktur intinya.
* **Bukan Produk Terpisah:** Tidak ada binary atau proyek independen bernama `aurion-node`, `aurion-validator`, `aurion-wallet`, `aurion-rpc`, `aurion-explorer`, atau `aurion-indexer`.
* **Satu Ekosistem `aurion`:** Seluruh fungsionalitas tersebut merupakan kapabilitas terintegrasi (*capabilities/modules*) di dalam satu payung rekayasa sistem yang terpadu.
* **Satu Standar Kualitas:** Standar keamanan tanpa kompromi (`#![forbid(unsafe_code)]`, zero float, fixed-precision Quantum) berlaku seragam untuk konsensus, wallet, RPC, maupun indexer.

### Prinsip 2: Modular Internally (Modularitas Internal yang Ketat)
Meskipun didistribusikan sebagai satu binary, arsitektur kode sumber Aurion mengedepankan modularitas tingkat tinggi dengan batas domain (*bounded contexts*) yang eksplisit:
* **Separation of Concerns:** Setiap modul bertanggung jawab penuh atas domain logikanya sendiri.
* **Explicit Interfaces:** Interaksi antar-modul wajib melewati trait publik dan kontrak antarmuka yang terdokumentasi, tanpa akses sembarangan ke state privat internal.
* **Dependency Boundaries:** Batas dependensi yang tegas mencegah kebocoran abstraksi (*leaky abstractions*).
* **Deterministic Behavior:** Modul-modul kritis protokol menjamin eksekusi deterministik bebas efek samping tak terkontrol.
* **Independent Testability:** Setiap modul dapat diuji secara terisolasi melalui unit tests, property-based testing, dan golden vector suite.
* **No Multi-Binary Sprawl:** Modularitas internal adalah disiplin perancangan perangkat lunak, bukan alasan untuk memecah rilis menjadi puluhan binary terpisah.

### Prinsip 3: One Binary (`/bin/aurion`)
Proses kompilasi production dari pohon sumber Aurion menghasilkan satu executable tunggal:
```bash
/bin/aurion
```
Kapabilitas operasional dipilih melalui subperintah (*execution modes*), bukan melalui binary yang berbeda:
* `aurion node`: Menjalankan node P2P jaringan penuh (*full node*).
* `aurion validator`: Menjalankan mesin konsensus dan penandatanganan blok validator.
* `aurion rpc`: Menjalankan server RPC HTTP/JSON-RPC 2.0 dan WebSocket publik/privat.
* `aurion wallet`: Menjalankan kapabilitas manajemen kunci, konstruksi transaksi, dan penandatanganan aman.
* `aurion indexer`: Menjalankan pipeline ekstraksi data historis dan analitik on-chain.
* `aurion conformance`: Menjalankan Conformance Test Suite (CTS) untuk audit kepatuhan protokol.

Perintah-perintah tersebut hanya mengaktifkan subsistem runtime yang relevan dalam satu executable yang sama.

### Prinsip 4: Satu Runtime Tunggal (*Unified Runtime Engine*)
Aurion tidak mengizinkan adanya beberapa runtime independen yang berjalan dengan logika berbeda. Seluruh subsistem beroperasi di bawah payung runtime terpadu:
```text
                  aurion
                    │
                 Runtime
                    │
       ┌────────────┼────────────┐
       │            │            │
    Consensus    Execution    Network
       │            │            │
       └────────────┼────────────┘
                    │
                 State
                    │
                 Storage
```
Penyatuan runtime menjamin bahwa seluruh subsistem menggunakan:
1. **Satu Configuration Model:** Satu format konfigurasi yang konsisten, tervalidasi skemanya, dan deterministik.
2. **Satu Logging & Tracing System:** Structured JSON logging terpusat dengan level tracing terpadu.
3. **Satu Telemetry & Metrics System:** Prometheus exporter tunggal dengan namespace metrik global yang seragam.
4. **Satu Error Taxonomy:** Model kesalahan mesin (*machine-readable error codes*) yang koheren dari layer jaringan hingga UI.
5. **Satu Lifecycle Supervisor:** Orkestrasi startup, health check, grace period, dan graceful shutdown yang terkoordinasi.
6. **Satu Protocol Implementation:** Tidak ada duplikasi parsing wire frame atau validasi transaksi.

### Prinsip 5: Single Binary $\neq$ Single Process Architecture
Prinsip *Single Binary* tidak membatasi arsitektur deployment di lingkungan production:
* Untuk keamanan maksimum, node validator production **WAJIB** diisolasi dari RPC publik.
* Operator infrastruktur dapat menjalankan beberapa proses terpisah pada server yang berbeda, namun **semuanya berasal dari executable `/bin/aurion` yang sama**:
  * Server A (Isolasi Jaringan Ketat): Menjalankan `aurion validator --config /etc/aurion/val.toml`
  * Server B (DMZ / Public Edge): Menjalankan `aurion rpc --config /etc/aurion/rpc.toml`
  * Server C (Sentry / P2P Relay): Menjalankan `aurion node --sentry-mode`
* Pemisahan proses operasional tercapai tanpa memecah kesatuan basis kode, pipeline CI/CD, atau identitas rilis perangkat lunak.

### Prinsip 6: Strict Acyclic Dependency Hierarchy (Arah Ketergantungan Ketat)
Pohon dependensi modul Aurion diatur secara satu arah (*strict directed acyclic graph*) untuk menjamin stabilitas dan mencegah circular dependencies:

```text
                    main (CLI Dispatcher)
                     │
                   runtime (Supervisor)
                     │
        ┌────────────┼────────────┐
        ▼            ▼            ▼
    consensus    execution     network
        │            │            │
        └──────┬─────┘            │
               ▼                  │
             state ◄──────────────┘
               │
             storage
               │
          shared core (Types, Crypto, Codec)
```

#### Larangan Keras Ketergantungan (*Illegal Dependency Anti-Patterns*):
* `wallet ─X→ consensus`: Modul wallet **DILARANG KERAS** bergantung pada mesin konsensus. Wallet hanya berinteraksi melalui interface transaksi dan RPC client.
* `rpc ─X→ wallet implementation`: Modul RPC publik **DILARANG KERAS** memiliki dependensi langsung pada private key atau penyimpanan internal wallet.
* `crypto ─X→ node runtime`: Modul primitif kriptografi adalah pustaka murni tanpa dependensi ke runtime sistem operasi atau event loop.
* `storage ─X→ application UI`: Modul penyimpanan persisten tingkat rendah tidak boleh dipengaruhi oleh kebutuhan presentasi antarmuka.

### Prinsip 7: Tidak Ada "Hidden Second Ecosystem"
Ekosistem Aurion menolak keberadaan pustaka eksternal yang menulis ulang (*reimplementing*) logika konsensus atau transaksi:
```text
                    AURION
                      │
             Single Source Tree
                      │
             Single Dependency Graph
                      │
             Single Release Process
                      │
             Single Version Identity
                      │
             Single Binary
```
* **Status SDK Klien:** SDK resmi (Rust, Go, Python, TypeScript) berfungsi sebagai *thin client bindings* yang membungkus serialisasi kanonikal wire protocol dan JSON-RPC 2.0. SDK **DILARANG KERAS** mendefinisikan aturan konsensus atau state transition sendiri.
* **Kebenaran Tunggal (*Single Source of Truth*):** Setiap perubahan pada aturan transaksi, skema encoding, atau kriptografi berpusat pada spesifikasi kanonikal Aurion dan diuji langsung terhadap test suite single binary.

### Prinsip 8: Landasan Menuju Executable Specification
Arsitektur Single Binary dan Application Rules Layer ini merupakan fondasi wajib sebelum melangkah ke *Executable Specification*:
* Seluruh aturan tidak lagi berbentuk spekulasi, melainkan diikat oleh aturan normatif RFC 2119.
* Dari arsitektur ini, diturunkan *Conformance Requirements* dan *Test Harness* yang mampu mengevaluasi integritas setiap subsistem Aurion secara terotomatisasi.

---

## 4. Daftar Invariant Arsitektur Aurion (AUR-ARCH)

Setiap kontribusi kode, refaktorisasi, dan rilis Aurion wajib mematuhi invariant formal berikut:

| ID Invariant | Pernyataan Normatif | Kategori |
| :--- | :--- | :--- |
| **AUR-ARCH-001** | Aurion **MUST** be distributed and packaged as a single primary executable binary (`/bin/aurion`). | Distribusi |
| **AUR-ARCH-002** | All protocol-critical functionality **MUST** belong to the unified Aurion software ecosystem. | Tata Kelola |
| **AUR-ARCH-003** | Internal components **MUST** be strictly modular, private by default, and explicitly bounded by trait contracts. | Desain Sistem |
| **AUR-ARCH-004** | Internal and external modules **MUST NOT** introduce independent or diverging protocol implementations. | Konsensus |
| **AUR-ARCH-005** | All subsystems **MUST** share the identical canonical protocol types, serialization codecs, cryptographic primitives, and state representation. | Tipe Data |
| **AUR-ARCH-006** | The production release **MUST** bear one authoritative version identity across all included capabilities. | Versioning |
| **AUR-ARCH-007** | No module or execution mode **MAY** silently fork or override protocol semantics from the canonical specifications. | Integritas |
| **AUR-ARCH-008** | Subsystems **MUST** interact exclusively through documented interfaces without cyclic dependencies. | Modul |
| **AUR-ARCH-009** | Execution subcommands (`node`, `validator`, `wallet`, `rpc`, dll.) **MUST** be orchestrated by the unified runtime engine. | Runtime |
| **AUR-ARCH-010** | Conformance test suites **MUST** evaluate the single binary across all supported operational modes. | Verifikasi |
| **AUR-ARCH-011** | All production code within the single repository **MUST** enforce `#![forbid(unsafe_code)]`. | Keamanan Memori |
| **AUR-ARCH-012** | All balance and monetary calculations **MUST** use the fixed-point `Quantum` ($u128$) primitive with zero floating-point arithmetic. | Integritas Finansial |

---

## 5. Tata Letak Repositori & Struktur Modul Terpadu

Repositori Aurion merefleksikan prinsip modularitas internal dalam satu pohon sumber:

```text
aurion/
│
├── Cargo.toml                          # Workspace manifest (forbid unsafe, deny float)
├── Cargo.lock                          # Deterministic dependency lockfile
├── README.md                           # Master Architectural Manifesto (Dokumen ini)
│
├── src/
│   ├── main.rs                         # Single Binary CLI entrypoint & subcommands dispatcher
│   │
│   ├── core/                           # Shared Core: Types, Quantum u128, Hash256, Address
│   ├── protocol/                       # Canonical wire formats, codecs, serializations
│   ├── crypto/                         # Blake3 standard/KDF, Ed25519 strict, Bech32m
│   ├── consensus/                      # Single-slot BFT, CommitCertificate, Quorum validation
│   ├── execution/                      # State Transition Function (STF), Block processing
│   ├── state/                          # Accounts, Sparse Merkle Tree (SMT), Monetary State
│   ├── storage/                        # Persistent KV storage engine, RocksDB/MDBX bindings
│   ├── network/                        # P2P wire framing (AUR0), Sentry discovery, DoS guards
│   ├── mempool/                        # In-memory transaction validation, RBF, eviction policy
│   ├── validator/                      # Validator key management, slashing protection, duty engine
│   ├── wallet/                         # HD Key derivation (BIP-44), clear signing, recovery
│   ├── rpc/                            # JSON-RPC 2.0 (aur_ namespace), WebSocket pub/sub
│   ├── indexer/                        # Canonical ledger ingestion, RPI tracking, analytics
│   ├── governance/                     # Protocol parameter upgrades, constitutional invariants
│   └── runtime/                        # Unified supervisor, config parser, telemetry, logging
│
├── crates/                             # Granular internal crates (for zero-compile leak boundaries)
│   ├── aurion-types/                   # Primitives, Address, Quantum, Signature
│   ├── aurion-crypto/                  # Cryptographic engine & golden vectors
│   ├── aurion-codec/                   # Canonical serialization & deserialization
│   ├── aurion-transaction/             # Transaction structure & verification
│   ├── aurion-state/                   # World state, SMT, fee burn distribution
│   ├── aurion-consensus/              # Consensus header, voting, certificates
│   ├── aurion-wire/                    # P2P frame format & network message catalogs
│   ├── aurion-genesis/                 # Genesis state σ0 and initial allocations
│   └── aurion-conformance-tests/       # 8-Pillar protocol conformance harness
│
├── docs/                               # Dokumentasi Spesifikasi & Aturan
│   ├── Constitutions/                  # Dokumen 00-12: Protocol & Consensus Specifications
│   └── Application-Rules-Layer/        # Dokumen 00-13: Application, Wallet, RPC, & Integration Rules
│
├── tests/                              # End-to-end multi-mode integration tests
├── vectors/                            # Golden cryptographic and state test vectors
└── benches/                            # High-throughput benchmarks (STF, SMT, Wire)
```

---

## 6. Pipeline Rekayasa Sistem: Dari Konstitusi ke Mainnet

Siklus hidup pengembangan Aurion mengikuti tahapan formal yang bertingkat dan tidak melompati verifikasi:

```text
                 AURION CONSTITUTIONS
                 (Nilai Fundamental & Batas Tertinggi Protokol)
                          │
                          ▼
                PROTOCOL SPECIFICATIONS
                (Spesifikasi Formal Konsensus, Kriptografi, STF, Wire)
                          │
                          ▼
                 APPLICATION RULES
                (Aturan Operasional Wallet, RPC, Explorer, Indexer)
                          │
                          ▼
            ARCHITECTURAL INVARIANTS (README.md)
            (Single Ecosystem / Single Binary Mandate)
                          │
                          ▼
               EXECUTABLE SPECIFICATION
               (Formal Test Vectors & Requirement Registry)
                          │
                          ▼
               CONFORMANCE TEST SUITE
               (Pengujian Kepatuhan Objektif Seluruh Subsistem)
                          │
                          ▼
                SINGLE BINARY RUNTIME
                (Implementasi Produksi Rust `/bin/aurion`)
                          │
                          ▼
                 DEVNET & TESTNET
                 (Simulasi Jaringan Terdesentralisasi P2P)
                          │
                          ▼
                  AURION MAINNET
                  (Peluncuran Blok Genesis Kanonikal)
```

### Horizon Evolusi Arsitektur Aurion (L1 s/d L5 Evolution Domains)

Roadmap jangka panjang Aurion tidak memandang "L" sebagai blockchain terpisah secara sembarangan, melainkan sebagai **Aurion Evolution Domains** dengan pemisahan peran arsitektural yang tegas:

| Layer / Domain | Peran Utama | Status Arsitektur | Cakupan Aturan |
| :--- | :--- | :---: | :--- |
| **L1 — Sovereign Core** | Sovereign blockchain, konsensus BFT, settlement, native execution, smart contract AVM, state, storage redb | **Core / Active** | Dokumen 00 s/d 16 |
| **L2 — Scaling Layer** | Scaling & high-throughput execution, rollup, data availability, validity/fraud proofs | **Planned** | Dokumen 17 |
| **L3 — Specialized Execution** | Specialized application execution domains (App-chains, microsecond DeFi, gaming, privacy ZK, AI compute) | **Planned** | Dokumen 18 |
| **L4 — Interoperability Layer** | Cross-chain messaging, asset/state/proof/identity interoperability, cross-domain settlement, external adapters | **Future** | Dokumen 19 |
| **L5 — Global Infrastructure** | Global distributed infrastructure (Decentralized compute, distributed storage, decentralized DA, indexing, M2M economy) | **Long-term** | Dokumen 20 |

```text
┌────────────────────────────────────────────────────────────┐
│                    AURION EVOLUTION                        │
├────────────────────────────────────────────────────────────┤
│                                                            │
│ L1  Sovereign Core                                         │
│     Consensus / State / Storage / VM / Smart Contract      │
│                                                            │
│ L2  Scaling                                                │
│     High-throughput / Rollup / DA / Proof                  │
│                                                            │
│ L3  Specialized Execution                                  │
│     App-specific / Privacy / Specialized Domains           │
│                                                            │
│ L4  Interoperability                                       │
│     Cross-chain / Cross-domain / Messaging / Assets        │
│                                                            │
│ L5  Global Infrastructure                                  │
│     Compute / Storage / Data / Identity / Services         │
│                                                            │
└────────────────────────────────────────────────────────────┘
```

#### Model Pengukuran Progres Bertahap (*Phased Progress Model*)
Setiap layer diukur secara bertahap tanpa melompati tahap validasi:
$$\text{DESIGN} \longrightarrow \text{RULES} \longrightarrow \text{REQUIREMENTS} \longrightarrow \text{BUILD} \longrightarrow \text{TEST} \longrightarrow \text{AUDIT} \longrightarrow \text{DONE}$$

---

## 7. Peta Indeks Dokumentasi Resmi

Dokumentasi lengkap protokol Aurion terbagi ke dalam dua pilar utama:

### A. Protocol & Consensus Specifications (`docs/Constitutions/`)
1. [`AURION CONSTITUTION.md`](docs/Constitutions/AURION%20CONSTITUTION.md): Konstitusi Tertinggi Protokol Aurion.
2. [`AURION-MONETARY-POLICY-SPECIFICATION.md`](docs/Constitutions/AURION-MONETARY-POLICY-SPECIFICATION.md): Kebijakan Moneter, Hard Cap 66M AUR, Deflasi.
3. [`AURION-CONSENSUS-SPECIFICATION.md`](docs/Constitutions/AURION-CONSENSUS-SPECIFICATION.md): Mekanisme Konsensus BFT Single-Slot Finality.
4. [`AURION-STATE-TRANSITION-SPECIFICATION.md`](docs/Constitutions/AURION-STATE-TRANSITION-SPECIFICATION.md): Aturan Transisi State $\sigma' = \Upsilon(\sigma, B)$.
5. [`AURION-TRANSACTION-SPECIFICATION.md`](docs/Constitutions/AURION-TRANSACTION-SPECIFICATION.md): Struktur Transaksi Kanonikal 184B+.
6. [`AURION-CRYPTOGRAPHY-SPECIFICATION.md`](docs/Constitutions/AURION-CRYPTOGRAPHY-SPECIFICATION.md): Primitif Kriptografi Blake3 & Ed25519.
7. [`AURION-SERIALIZATION-AND-WIRE-PROTOCOL.md`](docs/Constitutions/AURION-SERIALIZATION-AND-WIRE-PROTOCOL.md): Serialisasi Kanonikal & Protokol Wire P2P.
8. [`AURION-GENESIS-SPECIFICATION.md`](docs/Constitutions/AURION-GENESIS-SPECIFICATION.md): Spesifikasi Blok Genesis & Alokasi Awal 35%.
9. [`AURION-VALIDATOR-STAKING-SPECIFICATION.md`](docs/Constitutions/AURION-VALIDATOR-STAKING-SPECIFICATION.md): Validator Set, Staking, Slashing.
10. [`AURION-GOVERNANCE-SPECIFICATION.md`](docs/Constitutions/AURION-GOVERNANCE-SPECIFICATION.md): Mekanisme Tata Kelola On-Chain & Amandemen.
11. [`AURION-SECURITY-SPECIFICATION.md`](docs/Constitutions/AURION-SECURITY-SPECIFICATION.md): Model Ancaman, Mitigasi DoS, Audit.
12. [`AURION-REFERENCE-TEST-VECTORS.md`](docs/Constitutions/AURION-REFERENCE-TEST-VECTORS.md): Golden Cryptographic Test Vectors.
13. [`AURION-PROTOCOL-CONFORMANCE-SPECIFICATION.md`](docs/Constitutions/AURION-PROTOCOL-CONFORMANCE-SPECIFICATION.md): Standar Evaluasi Kepatuhan Protokol.

### B. Application Rules Layer (`docs/Application-Rules-Layer/application/`)
1. [`00-APPLICATION-RULES.md`](docs/Application-Rules-Layer/application/00-APPLICATION-RULES.md): Batas Arsitektural & Compliance Tiers.
2. [`01-WALLET-RULES.md`](docs/Application-Rules-Layer/application/01-WALLET-RULES.md): Siklus Kunci, BIP-44, Clear Signing, Manajemen Nonce.
3. [`02-RPC-API-RULES.md`](docs/Application-Rules-Layer/application/02-RPC-API-RULES.md): Antarmuka JSON-RPC 2.0, Namespace `aur_`, WebSocket.
4. [`03-TRANSACTION-LIFECYCLE.md`](docs/Application-Rules-Layer/application/03-TRANSACTION-LIFECYCLE.md): Mesin State Transaksi dari Pembuatan ke Finalitas.
5. [`04-FINALITY-CONFIRMATION-RULES.md`](docs/Application-Rules-Layer/application/04-FINALITY-CONFIRMATION-RULES.md): Aturan Finalitas Tunggal untuk Bursa & Merchant.
6. [`05-ADDRESS-ACCOUNT-RULES.md`](docs/Application-Rules-Layer/application/05-ADDRESS-ACCOUNT-RULES.md): Format Alamat Bech32m, Validasi, URI Scheme `aurion:`.
7. [`06-FEE-PAYMENT-RULES.md`](docs/Application-Rules-Layer/application/06-FEE-PAYMENT-RULES.md): Estimasi Biaya, Pembakaran 20%, Under/Overpayment.
8. [`07-PAYMENT-REFERENCE-RULES.md`](docs/Application-Rules-Layer/application/07-PAYMENT-REFERENCE-RULES.md): Standarisasi Memo Pembayaran & Pencegahan PII.
9. [`08-EXPLORER-INDEXER-RULES.md`](docs/Application-Rules-Layer/application/08-EXPLORER-INDEXER-RULES.md): Zero-Fabrication Mandate & Ingestion State Ganda.
10. [`09-SDK-RULES.md`](docs/Application-Rules-Layer/application/09-SDK-RULES.md): 9 Modul Kanonikal SDK & Identikalitas Byte Lintas Bahasa.
11. [`10-ERROR-MODEL.md`](docs/Application-Rules-Layer/application/10-ERROR-MODEL.md): Taksonomi Kode Kesalahan Mesin Terstruktur (1000-6999).
12. [`11-INTEGRATION-RULES.md`](docs/Application-Rules-Layer/application/11-INTEGRATION-RULES.md): Integrasi Bursa Kripto, Custody, Mutex Penarikan.
13. [`12-OPERATIONAL-RULES.md`](docs/Application-Rules-Layer/application/12-OPERATIONAL-RULES.md): Topologi Sentry Node, Health Check, Observabilitas.
14. [`13-COMPATIBILITY-VERSIONING.md`](docs/Application-Rules-Layer/application/13-COMPATIBILITY-VERSIONING.md): Matriks Kompatibilitas 4-Dimensi & Siklus Depresiasi.
15. [`14-STORAGE-PERSISTENCE-SPECIFICATION.md`](docs/Application-Rules-Layer/application/14-STORAGE-PERSISTENCE-SPECIFICATION.md): Mesin Penyimpanan & Persistensi murni Rust `redb 4.3`.
16. [`15-UNIFIED-CLI-SPECIFICATION.md`](docs/Application-Rules-Layer/application/15-UNIFIED-CLI-SPECIFICATION.md): Unified CLI & Application Control Plane (`/bin/aurion`).
17. [`16-SMART-CONTRACT-EXECUTION-SPECIFICATION.md`](docs/Application-Rules-Layer/application/16-SMART-CONTRACT-EXECUTION-SPECIFICATION.md): Smart Contract & Execution Layer (AVM).
18. [`17-L2-EVOLUTION-ARCHITECTURE-BLUEPRINT.md`](docs/Application-Rules-Layer/application/17-L2-EVOLUTION-ARCHITECTURE-BLUEPRINT.md): Blueprint L2 Scaling & Settlement.
19. [`18-L3-ECOSYSTEM-EXPANSION-BLUEPRINT.md`](docs/Application-Rules-Layer/application/18-L3-ECOSYSTEM-EXPANSION-BLUEPRINT.md): Blueprint L3 Ecosystem Expansion.
20. [`19-L4-INTEROPERABILITY-ARCHITECTURE-BLUEPRINT.md`](docs/Application-Rules-Layer/application/19-L4-INTEROPERABILITY-ARCHITECTURE-BLUEPRINT.md): Blueprint L4 Interoperability & Cross-Domain Ecosystem.
21. [`20-L5-GLOBAL-INFRASTRUCTURE-BLUEPRINT.md`](docs/Application-Rules-Layer/application/20-L5-GLOBAL-INFRASTRUCTURE-BLUEPRINT.md): Blueprint L5 Global Distributed Infrastructure.

---

## 8. Panduan Kompilasi & Verifikasi

Proyek Aurion diuji secara ketat tanpa peringatan (*zero warnings*) di bawah compiler Rust modern:

```powershell
# Jalankan seluruh test suite konformansi
cargo test --workspace

# Jalankan audit static analysis dan clippy linter ketat
cargo clippy --workspace --all-targets -- -D warnings
```

---
*Aurion Engineering Directive — Single Ecosystem / Single Binary Sovereign Architecture.*
