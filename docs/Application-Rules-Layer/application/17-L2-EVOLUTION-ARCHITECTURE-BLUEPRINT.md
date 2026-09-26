# Aurion Layer-2 (L2) Scaling & Settlement Architecture Blueprint

> **Status:** RATIFIED APPLICATION SPECIFICATION & ARCHITECTURAL BLUEPRINT (RULE 17)  
> **Parent Architecture:** [Aurion Master Architecture (README.md)](../../../README.md)  
> **Normative Framework:** RFC 2119 / RFC 8174 (`MUST`, `MUST NOT`, `REQUIRED`, `SHALL`, `SHOULD`, `MAY`)  
> **Klasifikasi:** Execution Layer Horizon & Settlement Specification  

---

## 1. Pendahuluan & Filosofi Desain

Dokumen ini menetapkan **Blueprint Arsitektur Layer-2 (L2) Scaling & Settlement** serta **Aurion Evolution Contract**. Dokumen ini bukan sekadar konsep teoritis, melainkan **standar kebutuhan terukur (*measurable specification*)** yang mengikat seluruh ekspansi horizontal rantai blok Aurion di masa depan.

### 1.1 Filosofi Inti: Ekstensi Kapasitas, Bukan Duplikasi Konsensus
Layer-2 Aurion **BUKAN** lapisan untuk "menambahkan smart contract", karena eksekusi smart contract telah menjadi bagian integral dari Layer-1 Aurion melalui **Aurion Native Virtual Machine (AVM)** (Dokumen Aturan 16). 

Sebaliknya, L2 didesain untuk menjawab kebutuhan berikut:
1. **Penskalaan Eksekusi Horizontal (*Horizontal Execution Scaling*):** Menangani ribuan transaksi per detik tanpa membebani node validator L1 dengan re-eksekusi transaksi individual.
2. **Domain Eksekusi Terspesialisasi (*Specialized Execution Domains*):** Menyediakan lingkungan komputasi khusus (misalnya: order-book DEX berkecepatan mikro-detik, transaksi mikro IoT, privasi zero-knowledge).
3. **Penyelesaian Kriptografis Berdaulat (*Sovereign Settlement*):** Seluruh mutasi state L2 diikat dan diverifikasi oleh konsensus L1 melalui komitmen state kanonikal (*State Commitments*) dan bukti keabsahan (*Validity Proofs*) atau jendela sanggahan (*Dispute Window*).

---

## 2. Pemisahan Tanggung Jawab Multi-Layer (L1 vs L2 vs L3)

Ekosistem Aurion mengadopsi pemisahan hierarkis yang ketat:

```text
                                   AURION ECOSYSTEM
                                          │
    ┌─────────────────────────────────────┼─────────────────────────────────────┐
    │                                     │                                     │
 LAYER 1 (L1)                          LAYER 2 (L2)                          LAYER 3 (L3)
 Sovereign Base Layer                  Execution & Scaling Layer             Hyper-Specialized Networks
    │                                     │                                     │
 ├── Sovereign Settlement              ├── High-Throughput Execution         ├── Micro-Execution Domains
 ├── Round-Based BFT Consensus         ├── Transaction Batch Compression     ├── App-Specific Rollups
 ├── Native State Ledger               ├── L1 State Commitments (Roots)      ├── Ultra-Low Latency Channels
 ├── Native AVM Smart Contracts        ├── Data Availability (DA) Posting    └── Settlement to L2
 ├── Validator Set & Staking           ├── Proof Generation (ZK/Fraud)
 ├── redb 4.3 ACID Persistence         ├── L1 ↔ L2 Cross-Layer Messaging
 └── Absolute Global Finality          └── Soft Finality & Anti-Censorship
```

### Tabel Komparasi Karakteristik Layer
| Dimensi | Layer-1 (L1 Base) | Layer-2 (L2 Rollup/Execution) | Layer-3 (L3 App-Specific) |
| :--- | :--- | :--- | :--- |
| **Fungsi Utama** | Settlement, Konsensus, Keamanan Inti | Throughput, Kompresi, Skalabilitas | Kustomisasi Aplikasi Ekstrem |
| **Konsensus** | Round-Based BFT Finality (>2/3 Quorum) | Sequencer BFT / Leader Rotation | Single Sequencer / State Channel |
| **Finalitas** | Hard Finality Mutlak ($\le 1$ detik) | Soft Finality (ms) $\to$ Hard via L1 | Instan $\to$ Soft via L2 $\to$ Hard via L1 |
| **Eksekusi VM** | Aurion Native VM (AVM) | AVM-Equivalent / Optimized Rollup VM | Domain-Specific Runtime / WASM / Custom |
| **Penyimpanan State** | `redb 4.3` ACID On-Disk | Rollup State Store + L1 State Root | Ephemeral / Micro State Store |
| **Basis Nilai Moneter** | Quantum ($u128$ integer murni) | Quantum ($u128$ integer murni) | Quantum ($u128$ integer murni) |

---

## 3. Arsitektur Komponen Layer-2

```text
+-------------------------------------------------------------------------+
|                           AURION LAYER-2 (L2)                           |
|                                                                         |
|  +-------------------+     +--------------------+     +---------------+ |
|  | L2 Client / User  | --> | Mempool & Sequencer| --> | L2 AVM Engine | |
|  +-------------------+     +--------------------+     +---------------+ |
|                                      │                        │         |
|                                      ▼                        ▼         |
|                             +-----------------+      +----------------+ |
|                             | Batch Assembler |      | L2 State Store | |
|                             +-----------------+      +----------------+ |
|                                      │                        │         |
|                                      ▼                        ▼         |
|                             +-----------------+      +----------------+ |
|                             |  Proof Engine   |      | State Root SMT | |
|                             |  (ZK / Fraud)   |      |  (Blake3 256)  | |
|                             +-----------------+      +----------------+ |
+--------------------------------------│────────────────────────│---------+
                                       │                        │
                          L1 CALLDATA  │                        │ STATE COMMITMENT
                          & PROOF POST │                        │ POST
                                       ▼                        ▼
+-------------------------------------------------------------------------+
|                           AURION LAYER-1 (L1)                           |
|                                                                         |
|  +-------------------------------------------------------------------+  |
|  |             L2SettlementBridge (Contract on AVM)                  |  |
|  |  - verify_state_transition(prev_root, next_root, proof, batch_hash)|  |
|  |  - process_deposit(l1_sender, l2_recipient, amount_quanta)         |  |
|  |  - process_withdrawal(l2_burn_proof, l1_recipient, amount_quanta)  |  |
|  |  - force_inclusion_queue(user_l2_tx)                              |  |
|  +-------------------------------------------------------------------+  |
|                                  │                                      |
|                                  ▼                                      |
|                     L1 State Transition & Storage                       |
+-------------------------------------------------------------------------+
```

---

## 4. Matriks Invariant Layer-2 (L2-*)

Seluruh rancangan, implementasi, dan pengujian L2 wajib mematuhi klausul normatif berikut:

### 4.1 Invariant Arsitektur Umum (`L2-ARCH`)
* **`L2-ARCH-001` (L1 Sovereignty Non-Degradation):** L2 `MUST NOT` mendegradasi keamanan, invariansi moneter, atau finalitas L1. Kegagalan, bug, atau serangan pada L2 `MUST NOT` memengaruhi integritas state L1.
* **`L2-ARCH-002` (Single Binary Alignment):** Komponen sequencer, node sinkronisasi, dan prover L2 `SHOULD` dapat dijalankan melalui dispatcher binary utama `/bin/aurion l2 <subcommand>`.
* **`L2-ARCH-003` (Zero Floating-Point Mandate):** Perhitungan fee, gas, kuantitas saldo, dan state transition L2 `MUST` menggunakan aritmetika integer presisi tetap Quantum ($u128$) murni. Tipe `f32` dan `f64` dilarang keras.
* **`L2-ARCH-004` (Strict Memory & Concurrency Safety):** Implementasi modul L2 `MUST` mematuhi `#![forbid(unsafe_code)]`.
* **`L2-ARCH-005` (Cryptographic Alignment):** L2 `MUST` menggunakan algoritma kriptografi yang selaras dengan L1: hashing Blake3 (256-bit) dan tanda tangan Ed25519 kanonikal.

### 4.2 Invariant Settlement L1 (`L2-SETTLE`)
* **`L2-SETTLE-001` (Canonical Settlement Contract):** Settlement L2 `MUST` ditangani oleh smart contract resmi `L2SettlementBridge` yang dideploy di L1 AVM.
* **`L2-SETTLE-002` (State Commitment Integrity):** Setiap batch transaksi L2 `MUST` menyertakan `prev_state_root`, `new_state_root`, `batch_data_hash`, dan `block_number`.
* **`L2-SETTLE-003` (Atomic Settlement Commitment):** L1 `MUST` memperbarui state root L2 secara atomik dalam satu transaksi eksekusi STF L1.
* **`L2-SETTLE-004` (Dispute Window & Finalization):** Dalam mode optimistic, penyelesaian final `MUST` menunggu jendela sanggahan (*Dispute Window*) minimal 20.000 blok L1. Dalam mode ZK-Validity, finalitas terjadi segera setelah verifikasi bukti sukses pada blok L1 tersebut.
* **`L2-SETTLE-005` (Invariant Konservasi Nilai):** Jumlah total Quantum yang didepositkan ke kontrak bridge L1 `MUST` tepat sama dengan total Quantum yang dicetak/dikeluarkan di L2, dikurangi penarikan sah yang telah difinalisasi di L1.

### 4.3 Invariant Data Availability (`L2-DA`)
* **`L2-DA-001` (Guaranteed Data Availability):** Data transaksi L2 `MUST` dipublikasikan ke L1 baik melalui calldata transaksi L1 atau dedicated DA blobs yang di-hash dengan Blake3.
* **`L2-DA-002` (Deterministic Reconstruction):** Siapa pun `MUST` dapat merekonstruksi state lengkap L2 secara independen hanya dengan membaca data DA dari L1.
* **`L2-DA-003` (Pruning Safety):** Node L2 `MAY` melakukan pruning histori transaksi lama hanya jika state root telah difinalisasi di L1 dan bukti validitas telah diarsipkan.

### 4.4 Invariant Proof & Verification (`L2-PROOF`)
* **`L2-PROOF-001` (Deterministic Verification):** Verifikasi bukti (validity proof atau fraud proof) pada smart contract L1 `MUST` deterministik 100% dan bebas dari ketergantungan waktu eksekusi host.
* **`L2-PROOF-002` (Soundness & Completeness):** Verifier L1 `MUST NOT` menerima bukti yang merepresentasikan transisi state L2 yang tidak sah.
* **`L2-PROOF-003` (Trap Protection):** Kesalahan dalam bukti `MUST` menghasilkan eksekusi `REVERT` di AVM L1 tanpa menghentikan kelangsungan jaringan L1.

### 4.5 Invariant Messaging L1 $\leftrightarrow$ L2 (`L2-MSG`)
* **`L2-MSG-001` (Two-Way Merkle Proofs):** Penarikan dana dari L2 ke L1 `MUST` menyertakan bukti Merkle pohon penarikan (*Withdrawal Merkle Proof*) yang terikat ke state root L2 yang telah difinalisasi di L1.
* **`L2-MSG-002` (Anti-Replay Protection):** Setiap pesan lintas layer `MUST` memiliki nonce unik dan message ID yang di-nullifier saat dieksekusi untuk mencegah serangan replay.
* **`L2-MSG-003` (Forced Inclusion Anti-Censorship):** Pengguna `MUST` memiliki kemampuan mengajukan transaksi L2 langsung melalui kontrak L1 (`force_inclusion_queue`). Jika sequencer L2 tidak memproses transaksi tersebut dalam waktu $N$ blok L1, jaringan L2 masuk ke mode *halted sequencer* dan penarikan darurat (*escape hatch*) aktif.

### 4.6 Invariant Lifecycle & Sequencer (`L2-LIFE`)
* **`L2-LIFE-001` (Sequencer Soft Finality):** Konfirmasi sequencer L2 bersifat *soft finality*. Pengguna berisiko tinggi (misalnya: transfer nilai besar) `MUST` menunggu komitmen state difinalisasi di L1.
* **`L2-LIFE-002` (Decentralized Sequencer Set):** Sequencer L2 `SHOULD` berevolution dari sequencer tunggal (fase bootstrap) menjadi rotasi multi-sequencer terikat staking L1.
* **`L2-LIFE-003` (Emergency Exit / Escape Hatch):** Jika sequencer berhenti beroperasi lebih dari 72 jam, pengguna `MUST` dapat menarik aset mereka kembali ke L1 secara sepihak menggunakan bukti kepemilikan state terakhir yang sah (*valid state proof*).

---

## 5. Aurion Evolution Contract & Transition Gates

Peralihan dari satu layer ke layer berikutnya diatur oleh kriteria gerbang (*transition gates*) yang ketat:

```text
     ERA I - III (Implementation)
                  │
                  ▼
          ERA IV: VERIFICATION
         (L1 Hardening & Tests)
                  │
        [GATE 1: L1 FREEZE & AUDIT]
                  │
                  ▼
             L1 MAINNET
                  │
       [GATE 2: L2 INITIATION GATE]
                  │
                  ▼
            L2 DEVELOPMENT
    (Design 100% -> Impl -> Test -> CTS)
                  │
       [GATE 3: L2 MAINNET DEPLOYMENT]
                  │
                  ▼
            L3 EXPANSION
```

### Kriteria Masuk Gerbang 2 (L1 Production $\to$ L2 Implementation Initiation)
1. **L1 Mainnet Stable:** L1 telah beroperasi di mainnet minimal 90 hari tanpa insiden konsensus atau fork tak terduga.
2. **AVM Verification:** Smart contract AVM telah memverifikasi $\ge 1.000.000$ transaksi eksekusi tanpa anomali determinisme.
3. **L2 Specification Complete (100%):** Dokumen Aturan 17 telah diratifikasi penuh dan seluruh requirement ID telah dipetakan ke rencana pengujian.

### Kriteria Keluar Gerbang 3 (L2 Implementation $\to$ L2 Mainnet Deployment)
1. **L2 Conformance Suite:** Lolos 100% pengujian kepatuhan `L2-CTS` (Data availability, dispute, bridge, rollback).
2. **Deterministic Settlement Test:** Minimal 100.000 batch L2 berhasil diverifikasi dan diselesaikan di L1 devnet/testnet.
3. **Anti-Censorship Test:** Mekanisme *forced inclusion queue* dan *escape hatch* telah diuji secara adversarial dan terbukti berhasil menarik dana pengguna ke L1 saat sequencer dimatikan.
4. **Independent Security Audit:** Laporan audit pihak ketiga terhadap kontrak L1 bridge dan prover rollup L2 dengan 0 temuan kritis/tinggi.

---

## 6. Model Pengukuran Kuantitatif (Progress Measurement Model)

Untuk mencegah klaim progres yang subjektif, seluruh status evolusi rantai blok Aurion dihitung dengan formula verifikasi berbobot berikut:

### 6.1 Formula Perhitungan Progres Per Layer
$$\text{Progress}_{\text{Layer}} = \frac{\sum_{i=1}^{N} \Big( W_{\text{SPEC}} \cdot S_i + W_{\text{IMPL}} \cdot I_i + W_{\text{TEST}} \cdot T_i + W_{\text{CTS}} \cdot C_i + W_{\text{AUD}} \cdot A_i \Big)}{N}$$

Di mana bobot standar adalah:
* **$W_{\text{SPEC}}$ (Specification Ratified):** 0.20 (20%)
* **$W_{\text{IMPL}}$ (Code Implemented):** 0.30 (30%)
* **$W_{\text{TEST}}$ (Unit/Integration Tests Passing):** 0.25 (25%)
* **$W_{\text{CTS}}$ (Conformance Test Verified):** 0.15 (15%)
* **$W_{\text{AUD}}$ (Security Audit Cleared):** 0.10 (10%)

### 6.2 Matriks Status Kebutuhan L2 (Initial Baseline)
| ID Kebutuhan | Deskripsi Singkat | Spec (0.2) | Impl (0.3) | Test (0.25) | CTS (0.15) | Audit (0.1) | Skor Terbobot |
| :--- | :--- | :---: | :---: | :---: | :---: | :---: | :---: |
| **REQ-L2-01** | L1 Bridge Contract Interface Specification | 1.0 | 0.0 | 0.0 | 0.0 | 0.0 | **20.0%** |
| **REQ-L2-02** | Rollup Batch Serialization & Compression | 1.0 | 0.0 | 0.0 | 0.0 | 0.0 | **20.0%** |
| **REQ-L2-03** | Blake3 Sparse Merkle Tree (SMT) State Roots | 1.0 | 0.0 | 0.0 | 0.0 | 0.0 | **20.0%** |
| **REQ-L2-04** | Calldata Data Availability Posting to L1 | 1.0 | 0.0 | 0.0 | 0.0 | 0.0 | **20.0%** |
| **REQ-L2-05** | ZK / Fraud Proof Verification on AVM | 1.0 | 0.0 | 0.0 | 0.0 | 0.0 | **20.0%** |
| **REQ-L2-06** | Two-Way Cross-Layer Message Relayer | 1.0 | 0.0 | 0.0 | 0.0 | 0.0 | **20.0%** |
| **REQ-L2-07** | Forced Inclusion Queue on L1 | 1.0 | 0.0 | 0.0 | 0.0 | 0.0 | **20.0%** |
| **REQ-L2-08** | L2 Sequencer Engine & Soft Finality BFT | 1.0 | 0.0 | 0.0 | 0.0 | 0.0 | **20.0%** |
| **REQ-L2-09** | Emergency Exit / Escape Hatch Mechanism | 1.0 | 0.0 | 0.0 | 0.0 | 0.0 | **20.0%** |
| **REQ-L2-10** | L2 Conformance & Simulation Test Harness | 1.0 | 0.0 | 0.0 | 0.0 | 0.0 | **20.0%** |
| **RATA-RATA**| **Status Keseluruhan L2 Blueprint Baseline**| **1.0** | **0.0** | **0.0** | **0.0** | **0.0** | **20.0% (Desain Terkunci)** |

---

## 7. Kesimpulan Arsitektural

Dengan diratifikasinya Dokumen Aturan 17 ini:
1. **L2 Memiliki Identitas yang Jelas:** L2 adalah lapisan skalabilitas komputasi dan kompresi transaksi yang menyerahkan konsensus, kedaulatan moneter, dan penyelesaian mutlak kepada L1.
2. **Roadmap Menjadi Terukur:** Tidak ada ambiguitas mengenai apa yang harus dikerjakan setelah L1 selesai; seluruh spesifikasi, antarmuka, dan kriteria gerbang transisi telah terdefinisi secara matematis dan formal.
3. **Integritas Konstitusi Terjaga:** Seluruh lapisan ekstensi (L2 dan L3) terikat oleh prinsip invariant yang sama: zero unsafe code, zero floating-point arithmetic, dan integritas deterministik berbasis single ecosystem.
