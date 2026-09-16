# Aurion Layer-5 (L5) Global Infrastructure Execution Phases & Task Register

> **Status:** RATIFIED EXECUTION ROADMAP  
> **Parent Specification:** [20-L5-GLOBAL-INFRASTRUCTURE-BLUEPRINT.md](../20-L5-GLOBAL-INFRASTRUCTURE-BLUEPRINT.md)  
> **Sub-Project Name:** `aurion-l5-infrastructure`  
> **Target Module:** `src/l5/` (Modular Monolith under `/bin/aurion`)  
> **Standard:** RFC 2119 / RFC 8174 (`MUST`, `MUST NOT`, `REQUIRED`, `SHALL`)

---

## 1. Peta Eksekusi Master Layer-5 (*Aurion L5 Execution Phases*)

```text
┌────────────────────────────────────────────────────────────────────────────────────────┐
│                          AURION LAYER-5 EXECUTION PHASES                               │
├────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                        │
│ [FASE L5-0] Spesifikasi Protokol Node & Jaringan Mesh Service (100% SPEC)              │
│   ├── L5-TSK-001: Definisi Standar Node Layanan Off-Chain & Service Registry           │
│   ├── L5-TSK-002: Arsitektur Pemisahan Konsensus L1 & Non-Consensus Mandate            │
│   └── L5-TSK-003: Sinkronisasi Matriks Spesifikasi L5 & Gate 6 Transition             │
│                                           │                                            │
│                                           ▼                                            │
│ [FASE L5-1] Primitif Mesh Network & Service Envelope (`src/l5/types.rs`, `codec.rs`)   │
│   ├── L5-TSK-101: Primitif Data L5 (ServiceTask, NodeAttestation, ProofOfExecution)   │
│   ├── L5-TSK-102: Wire Protocol P2P Khusus Layanan L5 (Transport Zenoh/QUIC)           │
│   └── L5-TSK-103: Canonical Codec & Golden Vectors untuk Request/Response Layanan     │
│                                           │                                            │
│                                           ▼                                            │
│ [FASE L5-2] Decentralized Compute & Verifiable Execution Engine (`src/l5/compute.rs`)  │
│   ├── L5-TSK-201: Sandboxed Compute Worker Daemon (WASM / MicroVM Task Isolator)       │
│   ├── L5-TSK-202: Zero-Knowledge & Fraud Proof of Compute Generation                   │
│   └── L5-TSK-203: Resource Quota & CPU/Memory Limiting Engine                          │
│                                           │                                            │
│                                           ▼                                            │
│ [FASE L5-3] Distributed Storage & Decentralized DA Mesh (`src/l5/storage.rs`, `da.rs`) │
│   ├── L5-TSK-301: Content-Addressed Chunk Storage & Blake3 Hash Verification           │
│   ├── L5-TSK-302: Proof of Retrievability (PoR) & Proof of Spacetime (PoST) Challenge │
│   └── L5-TSK-303: Decentralized Data Availability (DA) Sampling Engine                 │
│                                           │                                            │
│                                           ▼                                            │
│ [FASE L5-4] Distributed Indexing & Service Query Network (`src/l5/indexing.rs`)        │
│   ├── L5-TSK-401: Distributed State & Historical Event Indexer Worker                  │
│   ├── L5-TSK-402: High-Speed Query Node API (GraphQL / REST / gRPC Subsystem)          │
│   └── L5-TSK-403: Verifiable Query Response Attestation Engine                        │
│                                           │                                            │
│                                           ▼                                            │
│ [FASE L5-5] M2M Autonomous Economy & Streaming Billing (`src/l5/billing.rs`, `m2m.rs`) │
│   ├── L5-TSK-501: Micro-Payment Streaming Channels (Sub-Penny Exact Quantum u128)      │
│   ├── L5-TSK-502: Autonomous Machine Identity & Service License Minting               │
│   └── L5-TSK-503: Automated Economic Slashing & Collateral Vault Ingestion            │
│                                           │                                            │
│                                           ▼                                            │
│ [FASE L5-6] Single Binary CLI Integration & L5 Conformance Suite (`src/cli/`, `tests/`)│
│   ├── L5-TSK-601: CLI Subcommands: /bin/aurion l5 [mesh|worker|storage|query|bill]     │
│   ├── L5-TSK-602: L5 Conformance Test Harness (12 Pilar REQ-L5-01..12)                 │
│   └── L5-TSK-603: End-to-End Global Infrastructure Simulation Test                     │
│                                                                                        │
└────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## 2. Rincian Tugas & Kriteria Keberhasilan Per Fase

### Fase L5-0: Spesifikasi Protokol Node & Jaringan Mesh Service (Persiapan)
*Status: SELESAI / RATIFIED (Baseline 20.0%)*

| Task ID | Nama Tugas | Kriteria Keberhasilan (*Acceptance Criteria*) | Invariant |
| :--- | :--- | :--- | :---: |
| **L5-TSK-001** | Standar Node Layanan Off-Chain & Service Registry | Skema pendaftaran operator node (compute, storage, indexer, oracle) dengan jaminan stake AVM. | `AUR-L5-ARCH-002` |
| **L5-TSK-002** | Arsitektur Pemisahan Konsensus L1 & Non-Consensus | Dekopling mutlak: node konsensus L1 dilarang dibebani tugas komputasi atau hosting L5. | `AUR-L5-ARCH-001` |
| **L5-TSK-003** | Sinkronisasi Matriks Spesifikasi L5 & Gate 6 | Dokumen Rule 20 diratifikasi penuh; 12 requirement ID (`REQ-L5-01..12`) terpetakan 1:1 ke test plan. | `AUR-L5-RES-001` |

---

### Fase L5-1: Primitif Mesh Network & Service Envelope
*File Target: `src/l5/types.rs`, `src/l5/codec.rs`*

| Task ID | Nama Tugas | Kriteria Keberhasilan (*Acceptance Criteria*) | Invariant |
| :--- | :--- | :--- | :---: |
| **L5-TSK-101** | Primitif Data L5 (`ServiceTask`, `ProofOfExecution`) | Struktur data tugas komputasi, attestation chunk, receipt layanan, Zero-Float `Quantum(u128)`. | `AUR-ARCH-012`, `AUR-L5-COM-001` |
| **L5-TSK-102** | Wire Protocol P2P Khusus Layanan L5 | Transport framing berkinerja tinggi berbasis Zenoh/QUIC untuk penyebaran data dan tugas besar. | `AUR-WIRE-001` |
| **L5-TSK-103** | Canonical Codec & Golden Vectors Permintaan Layanan | Serialisasi biner kanonikal deterministik dengan toleransi nol-fabrikasi (*zero fabrication*). | `AUR-ARCH-005` |

---

### Fase L5-2: Decentralized Compute & Verifiable Execution Engine
*File Target: `src/l5/compute.rs`*

| Task ID | Nama Tugas | Kriteria Keberhasilan (*Acceptance Criteria*) | Invariant |
| :--- | :--- | :--- | :---: |
| **L5-TSK-201** | Sandboxed Compute Worker Daemon | Isolator runtime eksekusi komputasi berbasis WebAssembly (Wasm) deterministik aman. | `AUR-L5-COM-001` |
| **L5-TSK-202** | ZK & Fraud Proof of Compute Generation | Pembangkitan bukti eksekusi yang dapat diverifikasi oleh node verifier dalam waktu $O(1)$. | `AUR-L5-COM-002` |
| **L5-TSK-203** | Resource Quota & CPU/Memory Limiter | Pembatasan konsumsi memori dan instruksi waktu nyata untuk mencegah serangan kehabisan sumber daya. | `AUR-L5-RES-002` |

---

### Fase L5-3: Distributed Storage & Decentralized DA Mesh
*File Target: `src/l5/storage.rs`, `src/l5/da.rs`*

| Task ID | Nama Tugas | Kriteria Keberhasilan (*Acceptance Criteria*) | Invariant |
| :--- | :--- | :--- | :---: |
| **L5-TSK-301** | Content-Addressed Chunk Storage & Blake3 | Penyimpanan data terpecah dalam potongan-potongan terindeks hash Blake3 256-bit. | `AUR-L5-RES-001` |
| **L5-TSK-302** | Proof of Retrievability (PoR) & Spacetime (PoST) | Protokol tantangan kriptografis berkala membuktikan data benar-benar tersimpan di disk penyedia. | `AUR-L5-RES-001` |
| **L5-TSK-303** | Decentralized Data Availability (DA) Sampling | Mekanisme sampling ketersediaan data untuk blob besar rollup tanpa mengunduh seluruh isi data. | `AUR-L5-RES-001` |

---

### Fase L5-4: Distributed Indexing & Service Query Network
*File Target: `src/l5/indexing.rs`*

| Task ID | Nama Tugas | Kriteria Keberhasilan (*Acceptance Criteria*) | Invariant |
| :--- | :--- | :--- | :---: |
| **L5-TSK-401** | Distributed Ledger Event Indexer Worker | Ingestion transaksi dan mutasi state L1/L2 secara kontinu ke dalam database analitik terdesentralisasi. | `AUR-APP-08` |
| **L5-TSK-402** | High-Speed Query Node API (GraphQL/gRPC) | Antarmuka kueri data historis berlatensi rendah untuk dompet, penjelajah, dan klien eksternal. | `AUR-APP-02` |
| **L5-TSK-403** | Verifiable Query Response Attestation | Tanggapan kueri ditandatangani oleh operator indexer dengan bukti komitmen Merkle root L1. | `AUR-APP-08` |

---

### Fase L5-5: M2M Autonomous Economy & Streaming Billing
*File Target: `src/l5/billing.rs`, `src/l5/m2m.rs`*

| Task ID | Nama Tugas | Kriteria Keberhasilan (*Acceptance Criteria*) | Invariant |
| :--- | :--- | :--- | :---: |
| **L5-TSK-501** | Micro-Payment Streaming Channels | Saluran pembayaran mikro per detik/per komputasi dengan penyelesaian instan dalam unit `Quantum`. | `AUR-ARCH-012` |
| **L5-TSK-502** | Autonomous Machine Identity (M2M) | Otentikasi perangkat otonom dan agen AI menggunakan keypair Ed25519 berdaulat Aurion. | `AUR-L5-ARCH-002` |
| **L5-TSK-503** | Automated Slashing & Collateral Vault | Eksekusi pemotongan jaminan node secara otomatis pada kontrak L1 jika bukti kecurangan terverifikasi. | `AUR-L5-ARCH-002` |

---

### Fase L5-6: Single Binary CLI Integration & L5 Conformance Suite
*File Target: `src/cli/`, `tests/l5_conformance.rs`*

| Task ID | Nama Tugas | Kriteria Keberhasilan (*Acceptance Criteria*) | Invariant |
| :--- | :--- | :--- | :---: |
| **L5-TSK-601** | CLI Dispatcher Subcommands `/bin/aurion l5` | Integrasi single binary: `aurion l5 mesh`, `aurion l5 worker`, `aurion l5 storage`, `aurion l5 query`. | `AUR-ARCH-001`, `AUR-ARCH-009` |
| **L5-TSK-602** | L5 Conformance Test Harness (12 Pilar) | Pengujian kepatuhan 12 pilar (`REQ-L5-01..12`): PoR, compute sandbox, streaming billing. | `REQ-L5-*` |
| **L5-TSK-603** | End-to-End Global Infrastructure Simulation | Simulasi jaringan mesh global yang memproses komputasi, penyimpanan, dan kueri data terdistribusi. | Seluruh Invariant L5 |

---

## 3. Struktur Modul Kode Sumber (`src/l5/`)

```text
src/
├── l5/
│   ├── mod.rs          # Export publik aurion::l5
│   ├── types.rs        # L5-TSK-101 (ServiceTask, NodeAttestation, ProofOfExecution)
│   ├── codec.rs        # L5-TSK-102..103 (Canonical Mesh Wire Codec)
│   ├── compute.rs      # L5-TSK-201..203 (Wasm Sandboxed Verifiable Compute Engine)
│   ├── storage.rs      # L5-TSK-301..303 (Content-Addressed Storage & PoR/PoST)
│   ├── indexing.rs     # L5-TSK-401..403 (Distributed Ledger Indexer & Query API)
│   └── billing.rs      # L5-TSK-501..503 (M2M Streaming Billing & Slashing Vault)
```

---

## 4. Invariant Mutlak yang Wajib Dipertahankan
1. **`AUR-L5-ARCH-001` (Pemisahan Mutlak dari Konsensus L1):** Jaringan L5 beroperasi murni off-chain; kegagalan L5 tidak boleh memengaruhi konsensus L1 Aurion.
2. **`AUR-L5-RES-001` (Verifiable Storage & Compute):** Seluruh penyimpanan dan komputasi wajib dapat dibuktikan secara kriptografis tanpa kepercayaan buta.
3. **`AUR-ARCH-011` / `012`:** Zero unsafe code (`#![forbid(unsafe_code)]`) dan Zero floating-point arithmetic (seluruh tagihan dalam integer `Quantum`).
