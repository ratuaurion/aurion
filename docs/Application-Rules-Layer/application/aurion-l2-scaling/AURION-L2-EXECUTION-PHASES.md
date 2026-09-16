# Aurion Layer-2 (L2) Scaling Execution Phases & Task Register

> **Status:** RATIFIED EXECUTION ROADMAP  
> **Parent Specification:** [17-L2-EVOLUTION-ARCHITECTURE-BLUEPRINT.md](../17-L2-EVOLUTION-ARCHITECTURE-BLUEPRINT.md)  
> **Sub-Project Name:** `aurion-l2-scaling`  
> **Target Module:** `src/l2/` (Modular Monolith under `/bin/aurion`)  
> **Standard:** RFC 2119 / RFC 8174 (`MUST`, `MUST NOT`, `REQUIRED`, `SHALL`)

---

## 1. Peta Eksekusi Master Layer-2 (*Aurion L2 Execution Phases*)

```text
┌────────────────────────────────────────────────────────────────────────────────────────┐
│                          AURION LAYER-2 EXECUTION PHASES                               │
├────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                        │
│ [FASE L2-0] Spesifikasi Kontrak Bridge & Format Calldata (Status: 100% SPEC)          │
│   ├── L2-TSK-001: Definisi Antarmuka ABI L2SettlementBridge (AVM L1)                   │
│   ├── L2-TSK-002: Skema Serialisasi Canonical L2 Batch & Calldata Compression          │
│   └── L2-TSK-003: Sinkronisasi Matriks Spesifikasi L1 ↔ L2                             │
│                                           │                                            │
│                                           ▼                                            │
│ [FASE L2-1] Tipe Data Primitif & State Representation L2 (`src/l2/types.rs`, `state.rs`)│
│   ├── L2-TSK-101: Primitif Data L2 (L2Transaction, L2Block, L2Batch, L2Receipt)       │
│   ├── L2-TSK-102: L2 Account & SMT Blake3 256-bit (L2StateRoot)                       │
│   └── L2-TSK-103: Canonical Encode/Decode & Golden Test Vectors L2                     │
│                                           │                                            │
│                                           ▼                                            │
│ [FASE L2-2] L2 Execution Engine & Rollup Runtime (`src/l2/vm.rs`)                      │
│   ├── L2-TSK-201: Mesin Eksekusi Transaksi Throughput Tinggi (STF L2)                  │
│   ├── L2-TSK-202: Gas Metering & Fee Calculation L2 (Exact Quantum u128)               │
│   └── L2-TSK-203: Revert Semantics & Atomic Batch Rollback                             │
│                                           │                                            │
│                                           ▼                                            │
│ [FASE L2-3] L2 Sequencer Engine & Batch Assembler (`src/l2/sequencer.rs`)              │
│   ├── L2-TSK-301: L2 Mempool & In-Memory Transaksi dengan Proteksi DoS                │
│   ├── L2-TSK-302: Batch Assembler & Kompresi Transaksi Calldata L1                     │
│   └── L2-TSK-303: Sequencer Runtime & Soft Finality (<50ms)                            │
│                                           │                                            │
│                                           ▼                                            │
│ [FASE L2-4] Kontrak L1 Settlement Bridge & DA Ingestion (`src/l2/bridge.rs`)           │
│   ├── L2-TSK-401: Kontrak AVM L2SettlementBridge di Layer-1 (Vault & State Roots)      │
│   ├── L2-TSK-402: Calldata DA Posting ke Ledger L1                                     │
│   └── L2-TSK-403: Verifikasi Transisi State Atomik di L1                               │
│                                           │                                            │
│                                           ▼                                            │
│ [FASE L2-5] Two-Way Relayer & Anti-Censorship Protection (`src/l2/relayer.rs`)         │
│   ├── L2-TSK-501: Relayer Dua Arah L1 ↔ L2 (Deposit & Withdrawal via Merkle Proofs)    │
│   ├── L2-TSK-502: Forced Inclusion Queue di L1 (Anti-Censorship)                       │
│   └── L2-TSK-503: Emergency Exit / Escape Hatch Mechanism (Jika Sequencer Mati)        │
│                                           │                                            │
│                                           ▼                                            │
│ [FASE L2-6] Single Binary CLI Integration & L2 Conformance Suite (`src/cli/`, `tests/`)│
│   ├── L2-TSK-601: CLI Subcommands: /bin/aurion l2 [node|sequencer|bridge|tx]           │
│   ├── L2-TSK-602: L2 Conformance Test Harness (10 Pilar REQ-L2-01..10)                 │
│   └── L2-TSK-603: End-to-End L2 Lifecycle Integration Test                             │
│                                                                                        │
└────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## 2. Rincian Tugas & Kriteria Keberhasilan Per Fase

### Fase L2-0: Spesifikasi Kontrak Bridge & Format Calldata (Persiapan Arsitektur)
*Status: 100% SELESAI (Spesifikasi Formal & Implementasi Codec/ABI Terverifikasi)*  
*Dokumen Spesifikasi:*  
- [01-L2-SETTLEMENT-BRIDGE-ABI-SPECIFICATION.md](01-L2-SETTLEMENT-BRIDGE-ABI-SPECIFICATION.md)  
- [02-L2-BATCH-CALLDATA-COMPRESSION-SPECIFICATION.md](02-L2-BATCH-CALLDATA-COMPRESSION-SPECIFICATION.md)  
*File Kode Target:* [`src/l2/abi.rs`](../../../src/l2/abi.rs), [`src/l2/codec.rs`](../../../src/l2/codec.rs), [`src/l2/bridge.rs`](../../../src/l2/bridge.rs)

| Task ID | Nama Tugas | Kriteria Keberhasilan (*Acceptance Criteria*) | Invariant | Status |
| :--- | :--- | :--- | :---: | :---: |
| **L2-TSK-001** | Definisi Antarmuka ABI `L2SettlementBridge` | Antarmuka ABI smart contract AVM di L1 (`01-L2-SETTLEMENT-BRIDGE-ABI-SPECIFICATION.md`), 4-byte Blake3 selector, decoder calldata kanonikal di `src/l2/abi.rs`. | `L2-SETTLE-001..005` | **DONE** |
| **L2-TSK-002** | Skema Serialisasi Canonical L2 Batch & DA | Format serialisasi biner pembingkaian batch `AUL2` (`02-L2-BATCH-CALLDATA-COMPRESSION-SPECIFICATION.md`), header 102 byte, kompresi calldata di `src/l2/codec.rs`. | `L2-DA-001..002` | **DONE** |
| **L2-TSK-003** | Sinkronisasi Matriks Spesifikasi L1 $\leftrightarrow$ L2 | Dokumen Rule 17 dan spesifikasi teknis L2-01 & L2-02 diratifikasi penuh; 10 requirement ID (`REQ-L2-01..10`) terpetakan 1:1; 88 unit test lulus 100%. | `AUR-ARCH-001`, `L2-ARCH-001` | **DONE** |

---

### Fase L2-1: Tipe Data Primitif & State Representation L2
*Status: 100% SELESAI (Tipe Data Kanonikal, SMT 256-bit, & Vektor Pengujian Terverifikasi)*  
*File Target:* [`src/l2/types.rs`](../../../src/l2/types.rs), [`src/l2/state.rs`](../../../src/l2/state.rs)

| Task ID | Nama Tugas | Kriteria Keberhasilan (*Acceptance Criteria*) | Invariant | Status |
| :--- | :--- | :--- | :---: | :---: |
| **L2-TSK-101** | Primitif Data L2 (`L2Transaction`, `L2Block`, `L2Batch`, `L2Receipt`) | Struktur data transaksi L2, batch header, receipt, nonce tracking, Zero-Float `Quantum(u128)`, verifikasi tanda tangan Ed25519, `compute_txs_root`. | `L2-ARCH-003`, `L2-ARCH-005` | **DONE** |
| **L2-TSK-102** | L2 Account & SMT Blake3 (`L2StateRoot`) | `L2Account` dengan `storage_root`, Sparse Merkle Tree (SMT) deterministik Blake3 256-bit, dan pembangkitan/verifikasi bukti keanggotaan `L2AccountProof`. | `L2-SETTLE-002`, `L2-MSG-001` | **DONE** |
| **L2-TSK-103** | Canonical Encode/Decode & Golden Test Vectors L2 | Serialisasi byte kanonikal big-endian roundtrip 100% identik (`encode_canonical`/`decode_canonical`) dan pengujian vektor emas deterministik (92 total tes lolos). | `AUR-ARCH-005`, `L2-ARCH-005` | **DONE** |

---

### Fase L2-2: L2 Execution Engine & Rollup Runtime
*Status: 100% SELESAI (STF Throughput Tinggi, Zero-Float Gas Metering, Fee Split 80/20, & Atomic Rollback)*  
*File Target:* [`src/l2/vm.rs`](../../../src/l2/vm.rs), [`src/l2/state.rs`](../../../src/l2/state.rs)

| Task ID | Nama Tugas | Kriteria Keberhasilan (*Acceptance Criteria*) | Invariant | Status |
| :--- | :--- | :--- | :---: | :---: |
| **L2-TSK-201** | Mesin Eksekusi Transaksi Throughput Tinggi L2 | State Transition Function (STF) L2: $\sigma_{L2}' = \Upsilon_{L2}(\sigma_{L2}, B_{L2})$ untuk transfer berkecepatan tinggi dengan validasi komitmen state root deterministik. | `L2-ARCH-001`, `L2-EXEC-001` | **DONE** |
| **L2-TSK-202** | Gas Metering & Fee Calculation L2 | Perhitungan gas integer exact (10.000 gas dasar + 4 gas/byte payload), harga minimum 1 Quanta, pembagian fee 80% Sequencer & 20% L1 settlement cost. | `L2-ARCH-003`, `AUR-ARCH-012` | **DONE** |
| **L2-TSK-203** | Revert Semantics & Atomic Batch Rollback | Proteksi atomik mutasi state L2 melalui mekanisme checkpoint & snapshot jika salah satu transaksi dalam batch gagal memenuhi invariant. | `AUR-VM-005`, `L2-PROOF-003` | **DONE** |


---

### Fase L2-3: L2 Sequencer Engine & Batch Assembler
*Status: 100% SELESAI (Mempool Prioritisasi Fee, Anti-DoS, Canonical BatchFrame Assembler, & Soft Finality <50ms)*  
*File Target:* [`src/l2/sequencer.rs`](../../../src/l2/sequencer.rs)

| Task ID | Nama Tugas | Kriteria Keberhasilan (*Acceptance Criteria*) | Invariant | Status |
| :--- | :--- | :--- | :---: | :---: |
| **L2-TSK-301** | L2 Mempool & In-Memory Transaksi | Antrean transaksi mempool L2 dengan pengurutan prioritas fee tertinggi (`drain_prioritized`), pencegahan nonce ganda per akun, dan batas anti-DoS 10.000 transaksi (`MAX_L2_MEMPOOL_CAPACITY`). | `AUR-APP-03`, `L2-LIFE-001` | **DONE** |
| **L2-TSK-302** | Batch Assembler & Kompresi Transaksi | Penggabungan transaksi dari blok-blok aktif ke dalam framing biner kanonikal `L2BatchFrame` (`AUL2` 102-byte header) dengan metadata batch hash dan Blake3 DA commitment. | `L2-DA-001`, `L2-SETTLE-002` | **DONE** |
| **L2-TSK-303** | Sequencer Runtime & Soft Finality (<50ms) | Daemon sequencer memproduksi blok L2 deterministik dengan STF atomik dan menerbitkan pengesahan konfirmasi instan `L2SoftFinalityReceipt` sebelum commit L1. | `L2-LIFE-001`, `L2-LIFE-002` | **DONE** |


---

### Fase L2-4: Kontrak L1 Settlement Bridge & DA Ingestion
*Status: 100% SELESAI (Vault Conservation, Blake3 DA Posting Commitment, & Atomic State Transition)*  
*File Target:* [`src/l2/bridge.rs`](../../../src/l2/bridge.rs), [`src/l2/abi.rs`](../../../src/l2/abi.rs)

| Task ID | Nama Tugas | Kriteria Keberhasilan (*Acceptance Criteria*) | Invariant | Status |
| :--- | :--- | :--- | :---: | :---: |
| **L2-TSK-401** | Kontrak AVM `L2SettlementBridge` di Layer-1 | Kontrak settlement L1 dengan pengelolaan deposit vault, konservasi nilai (`vault_balance`), pelacakan state root, dan emisi log kejadian (`BridgeEvent`). | `L2-SETTLE-001`, `L2-SETTLE-005` | **DONE** |
| **L2-TSK-402** | Calldata DA Posting ke Ledger L1 | Verifikasi integritas komitmen DA Blake3 atas seluruh frame calldata (`verify_state_transition_with_da`) dan penyimpanan riwayat komitmen per batch. | `L2-DA-001`, `L2-DA-002` | **DONE** |
| **L2-TSK-403** | Verifikasi Transisi State Atomik di L1 | Evaluasi berkesinambungan `prev_root` vs `next_root`, penomoran batch sekuensial, pemutakhiran status atomik, dan verifikasi penarikan via cabang Merkle. | `L2-SETTLE-003`, `L2-PROOF-001` | **DONE** |


---

### Fase L2-5: Two-Way Relayer & Anti-Censorship Protection
*Status: 100% SELESAI (Two-Way Messaging Relayer, Forced Inclusion Queue, & Escape Hatch Unilateral Claims)*  
*File Target:* [`src/l2/relayer.rs`](../../../src/l2/relayer.rs)

| Task ID | Nama Tugas | Kriteria Keberhasilan (*Acceptance Criteria*) | Invariant | Status |
| :--- | :--- | :--- | :---: | :---: |
| **L2-TSK-501** | Relayer Dua Arah L1 $\leftrightarrow$ L2 | Mekanisme deposit L1 $\to$ L2 (kunci vault L1 & cetak di L2) dan penarikan L2 $\to$ L1 (bakar di L2 & buka kunci vault L1 via Merkle proof `WithdrawalProof`). | `L2-MSG-001`, `L2-MSG-002` | **DONE** |
| **L2-TSK-502** | Forced Inclusion Queue di L1 | Pengguna dapat mengirim transaksi L2 langsung ke kontrak L1 jika sequencer melakukan sensor, dengan batas waktu toleransi blok sebelum freeze. | `L2-MSG-003` | **DONE** |
| **L2-TSK-503** | Emergency Exit / Escape Hatch Mechanism | Penarikan dana mandiri sepihak oleh pengguna jika sequencer L2 berhenti atau membeku, dibuktikan dengan bukti keanggotaan SMT Blake3 256-bit `L2AccountProof`. | `L2-LIFE-003` | **DONE** |

---

### Fase L2-6: Single Binary CLI Integration & L2 Conformance Suite
*Status: 100% SELESAI (Single Binary CLI `/bin/aurion l2`, 10-Pillar L2-CTS Harness, & Full Lifecycle E2E Suite)*  
*File Target:* [`src/cli/`](../../../src/cli/), [`tests/l2_conformance.rs`](../../../tests/l2_conformance.rs), [`tests/l2_lifecycle_e2e.rs`](../../../tests/l2_lifecycle_e2e.rs)

| Task ID | Nama Tugas | Kriteria Keberhasilan (*Acceptance Criteria*) | Invariant | Status |
| :--- | :--- | :--- | :---: | :---: |
| **L2-TSK-601** | CLI Dispatcher Subcommands `/bin/aurion l2` | Integrasi single binary: `aurion l2 node`, `aurion l2 sequencer`, `aurion l2 bridge`, `aurion l2 tx` dengan output human-readable dan format mesin `--output json`. | `AUR-ARCH-001`, `L2-ARCH-002` | **DONE** |
| **L2-TSK-602** | L2 Conformance Test Harness (L2-CTS) | 10 pilar pengujian kepatuhan (`REQ-L2-01..10`) mencakup ABI selectors, batch frame codec, SMT state roots, DA calldata posting, STF atomic rollback, two-way messaging, forced queue, sequencer soft finality, escape hatch, dan zero-float/zero-unsafe. | `L2-ARCH-001`, `REQ-L2-*` | **DONE** |
| **L2-TSK-603** | End-to-End L2 Lifecycle Integration Suite | Uji siklus hidup utuh (`tests/l2_lifecycle_e2e.rs`): Deposit L1 $\to$ L2 Tx $\to$ Soft Finality $\to$ Batch DA Commit $\to$ L2 Burn $\to$ Merkle Withdrawal $\to$ Vault Conservation $\to$ Forced Inclusion $\to$ Escape Hatch Claim. | Seluruh Invariant L2 | **DONE** |

---

## 3. Struktur Kode Sumber Domain-Driven Architecture

Seluruh kode sumber Aurion distrukturkan ke dalam 5 Domain Fungsional Mandiri (Domain-Driven Architecture) bebas label numerik layer (`L1/L2`):

```text
src/
├── primitives/         # Fondasi protokol bersama (core, crypto, codec, genesis)
├── statemachine/       # Mesin status & eksekusi STF/AVM (state, vm, transaction)
├── consensus/          # Konsensus BFT & antrean prioritas (bft, mempool)
├── scaling/            # Ekstensi throughput tinggi rollup & bridge (types, state, vm, sequencer, bridge, relayer, abi, codec)
├── platform/           # Infrastruktur sistem & klien (storage, wire, gateway, wallet, cli, runtime, conformance)
├── lib.rs              # Re-export kanonikal transparan untuk seluruh ruang kerja
└── main.rs             # Bootstrap single binary (/bin/aurion)
```

---

## 4. Invariant Mutlak yang Wajib Dipertahankan
1. **`L2-ARCH-001` (Kedaulatan L1):** Kegagalan L2 tidak boleh memengaruhi integritas state atau konsensus L1.
2. **`L2-ARCH-003` (Zero-Float):** Dilarang keras menggunakan `f32`/`f64`. Seluruh kuantitas menggunakan integer `Quantum(u128)`.
3. **`L2-ARCH-004` (Zero-Unsafe):** Seluruh crate mematuhi `#![forbid(unsafe_code)]`.
4. **`L2-ARCH-005` (Kriptografi Identik):** Menggunakan hashing Blake3 256-bit dan tanda tangan Ed25519 kanonikal.
