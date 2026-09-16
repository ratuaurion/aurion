# 01. Spesifikasi Antarmuka ABI Kontrak L2SettlementBridge (AVM L1)

> **Status:** RATIFIED APPLICATION SPECIFICATION (L2-SPEC-01)  
> **Sub-Project:** `aurion-l2-scaling`  
> **Execution Phase:** `FASE L2-0` (Task ID: `L2-TSK-001`)  
> **Target Runtime:** Aurion Virtual Machine (AVM) di Layer-1  
> **Standar Kata Kunci:** RFC 2119 / RFC 8174 (`MUST`, `MUST NOT`, `REQUIRED`, `SHALL`)

---

## 1. Ringkasan Eksekutif (*Executive Summary*)

Kontrak pintar `L2SettlementBridge` adalah jangkar kedaulatan utama (*sovereign anchor*) Layer-2 yang hidup di lingkungan eksekusi AVM Layer-1. Kontrak ini bertanggung jawab atas:
1. Mengamankan aset cadangan vault L1 yang didepositkan pengguna ke L2.
2. Memverifikasi transisi status atomik (*atomic state transition verification*) yang diposting oleh Sequencer L2.
3. Memfasilitasi penarikan dana (*withdrawal settlement*) dengan validasi bukti Merkle L2 yang sah.
4. Menerima antrean transaksi paksa (*forced inclusion queue*) sebagai mekanisme anti-sensor independen.

Dokumen ini mendefinisikan antarmuka biner (*Application Binary Interface / ABI*) resmi, skema selector fungsi 4-byte berbasis Blake3, tata letak parameter Big-Endian, kode kesalahan numerik, serta struktur log kejadian (*events*).

---

## 2. Invariant Kepatuhan Protokol (*Protocol Invariants*)

Seluruh implementasi kontrak `L2SettlementBridge` dan klien interaksinya `MUST` mematuhi:
* **`L2-SETTLE-001` (Canonical Settlement):** Penyelesaian akhir L2 bersifat deterministik dan hanya sah jika tercatat pada blok kanonikal Layer-1.
* **`L2-SETTLE-002` (State Commitment):** Setiap transisi state root wajib membentuk rantai kriptografis yang berkesinambungan (`prev_state_root == current_committed_root`).
* **`L2-SETTLE-005` (Konservasi Nilai Vault):** Total aset di dalam vault bridge `MUST` selalu mencukupi total klaim penarikan (`VaultBalance >= WithdrawableClaims`).
* **`AUR-ARCH-011` (Absolute Zero Unsafe):** Tidak boleh ada blok kode tidak aman (`unsafe`) pada modul parser maupun eksekutor ABI.
* **`AUR-ARCH-012` (Zero-Float Arithmetic):** Seluruh nominal moneter dinyatakan dalam bilangan bulat `Quantum` $u128$ ($10^{-8}$ AUR). Dilarang keras menggunakan floating-point (`f32`/`f64`).

---

## 3. Skema Identifikasi Fungsi (*Function Selectors*)

Sesuai konvensi Aurion VM, pemilih fungsi (*function selector*) berukuran 4 byte dan dihitung dari 4 byte pertama intisari Blake3 terhadap tanda tangan kanonikal metode:

$$\text{Selector} = \text{Blake3}(\text{"method_signature"})[0..4]$$

| Nama Metode | Tanda Tangan Kanonikal (*Signature*) | Selector Hex | Deskripsi Operasi |
| :--- | :--- | :---: | :--- |
| `deposit` | `deposit(Address,Quantum)` | `0x5D437F01` | Menerima transfer AUR dari akun L1 ke dalam vault bridge dan menerbitkan event deposit ke L2. |
| `verify_state_transition` | `verify_state_transition(u64,Hash256,Hash256,u64,u64,Hash256)` | `0x8A2C19E4` | Memvalidasi dan mengomitmenkan batch transisi status L2 berikutnya ke rantai L1. |
| `withdraw` | `withdraw(Address,Quantum,u32,bytes)` | `0x3B79F418` | Mencairkan dana vault L1 kepada penerima dengan verifikasi bukti cabang Merkle withdrawal L2. |
| `enqueue_forced_tx` | `enqueue_forced_tx(bytes)` | `0x1F80E74C` | Memasukkan payload transaksi ke antrean paksa di L1 untuk dieksekusi sequencer dalam batas timeout. |
| `escape_hatch_claim` | `escape_hatch_claim(Address,Quantum,bytes)` | `0x7C9246D3` | Penarikan darurat pengguna jika sequencer membeku melampaui batas toleransi jendela sengketa. |

---

## 4. Tata Letak Parameter Masukan (*Parameter Layout & Encoding*)

Seluruh nilai skalar dikodekan dalam format biner kanonikal **Big-Endian**. Tidak diperbolehkan adanya bantalan (*padding*) non-standar.

### 4.1 `deposit(Address,Quantum)`
* Selector: `0x5D437F01` (4 byte)
* Parameter:
  1. `recipient_l2` (Address): 32 byte alamat tujuan di Layer-2.
  2. `amount` (Quantum): 16 byte integer tanpa tanda (`u128` Big-Endian).
* Total Ukuran Calldata: 52 byte.

### 4.2 `verify_state_transition(u64,Hash256,Hash256,u64,u64,Hash256)`
* Selector: `0x8A2C19E4` (4 byte)
* Parameter:
  1. `batch_index` (`u64`): 8 byte indeks urut batch.
  2. `prev_state_root` (`Hash256`): 32 byte hash root state sebelumnya.
  3. `new_state_root` (`Hash256`): 32 byte hash root state sesudahnya.
  4. `start_block` (`u64`): 8 byte nomor blok awal L2 yang dicakup batch.
  5. `end_block` (`u64`): 8 byte nomor blok akhir L2 yang dicakup batch.
  6. `calldata_hash` (`Hash256`): 32 byte komitmen intisari Blake3 data ketersediaan (DA).
* Total Ukuran Calldata: 124 byte.

### 4.3 `withdraw(Address,Quantum,u32,bytes)`
* Selector: `0x3B79F418` (4 byte)
* Parameter:
  1. `recipient_l1` (`Address`): 32 byte alamat penerima di Layer-1.
  2. `amount` (`Quantum`): 16 byte nilai penarikan (`u128` Big-Endian).
  3. `leaf_index` (`u32`): 4 byte indeks posisi daun penarikan pada pohon Merkle.
  4. `merkle_branch_len` (`u16`): 2 byte jumlah saudara (*siblings*) cabang Merkle ($K$).
  5. `merkle_branch` (`[Hash256; K]`): $K \times 32$ byte hash cabang verifikasi.
* Total Ukuran Calldata: $58 + (K \times 32)$ byte.

### 4.4 `enqueue_forced_tx(bytes)`
* Selector: `0x1F80E74C` (4 byte)
* Parameter:
  1. `payload_len` (`u32`): 4 byte panjang data biner transaksi L2.
  2. `payload` (`bytes`): Data mentah transaksi L2 yang wajib disertakan oleh Sequencer.
* Total Ukuran Calldata: $8 + \text{payload\_len}$ byte.

---

## 5. Kode Status Eksekusi (*Execution Status Codes*)

Hasil eksekusi panggilan kontrak mengembalikan kode status numerik `u8` pada awal byte keluaran:

| Kode Hex | Nama Kesalahan | Kondisi Pemicu |
| :---: | :--- | :--- |
| `0x00` | `SUCCESS` | Eksekusi berhasil penuh tanpa anomali. |
| `0x01` | `ERR_INSUFFICIENT_VAULT` | Saldo cadangan vault bridge L1 tidak mencukupi untuk memenuhi klaim penarikan. |
| `0x02` | `ERR_INVALID_PREV_ROOT` | `prev_state_root` batch tidak sama dengan komitmen state root L1 terkini. |
| `0x03` | `ERR_NON_SEQUENTIAL_BATCH`| `batch_index` tidak berurutan tepat $\text{index}_{\text{current}} + 1$. |
| `0x04` | `ERR_INVALID_MERKLE_PROOF`| Perhitungan traversal cabang Merkle tidak menghasilkan root state penarikan yang sah. |
| `0x05` | `ERR_TIMEOUT_NOT_EXPIRED` | Upaya klaim emergency exit dilakukan sebelum jendela sengketa terlampaui. |
| `0x06` | `ERR_ARITHMETIC_OVERFLOW` | Terdeteksi luapan (*overflow*) atau underflow pada perhitungan saldo `Quantum`. |
| `0x07` | `ERR_MALFORMED_CALLDATA` | Ukuran calldata atau panjang variadik tidak memenuhi batas skema ABI. |
| `0x08` | `ERR_UNKNOWN_SELECTOR` | 4 byte selector fungsi tidak dikenali oleh dispatch table AVM. |

---

## 6. Definisi Event Log (*Contract Event Emission*)

Kontrak `L2SettlementBridge` menerbitkan log kejadian kanonikal untuk dimonitor oleh node Sequencer dan Relayer:

```text
Event 1: DepositEvent
Topic[0] = Blake3("Deposit(Address,Address,Quantum,u64)")
Data     = [sender_l1: 32B, recipient_l2: 32B, amount: 16B, deposit_nonce: 8B]

Event 2: StateTransitionVerifiedEvent
Topic[0] = Blake3("StateTransitionVerified(u64,Hash256,Hash256)")
Data     = [batch_index: 8B, prev_state_root: 32B, new_state_root: 32B]

Event 3: WithdrawalExecutedEvent
Topic[0] = Blake3("WithdrawalExecuted(Address,Quantum,Hash256)")
Data     = [recipient_l1: 32B, amount: 16B, withdrawal_hash: 32B]

Event 4: ForcedTransactionEnqueuedEvent
Topic[0] = Blake3("ForcedTransactionEnqueued(Hash256,u64)")
Data     = [tx_hash: 32B, enqueued_l1_block: 8B]
```

---

## 7. Rangkuman Kesiapan Implementasi

Spesifikasi ABI ini menjadi landasan formal bagi modul kode Rust [`src/l2/abi.rs`](../../../src/l2/abi.rs) dan implementasi kontrak bridge di [`src/l2/bridge.rs`](../../../src/l2/bridge.rs).
