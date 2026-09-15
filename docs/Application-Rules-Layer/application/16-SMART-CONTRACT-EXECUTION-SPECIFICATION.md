# DOKUMEN ATURAN APLIKASI 16: SMART CONTRACT & EXECUTION LAYER SPECIFICATION
> **Status:** RATIFIKASI RESMI  
> **Kategori:** Application Rules Layer & Protocol Execution Specification  
> **Kode Standar:** `AUR-APP-16` / `AUR-VM-001..010`  
> **Prinsip Utama:** Deterministik, Gas-Metered, Zero-Float, Zero-Unsafe, State Transition Integrated  

---

## 1. Posisi Smart Contract dalam Arsitektur Aurion

Smart contract di Aurion adalah komponen **first-class dalam Execution Layer**, sejajar dengan *Native Protocol Execution* (transfer, staking, tata kelola). Smart contract bukan ekstensi dompet atau antarmuka RPC tambahan, melainkan mesin komputasi terprogram yang tunduk pada aturan determinisme konsensus dan *State Transition Function* ($\sigma' = \Upsilon(\sigma, B)$).

```text
                         AURION
                           │
                    ┌──────┴──────┐
                    │ UNIFIED CLI │
                    └──────┬──────┘
                           │
                     AURION RUNTIME
                           │
        ┌──────────────────┼──────────────────┐
        │                  │                  │
     Network          Transaction          RPC
        │                  │                  │
        │             Execution Layer         │
        │                  │                  │
        │        ┌─────────┴─────────┐        │
        │        │                   │        │
        │   Native Execution    Smart Contract│
        │        │                   │        │
        │        │              Contract VM   │
        │        │                   │        │
        └────────┴──────────┬────────┘        │
                            │
                     STATE TRANSITION
                            │
                          STATE
                            │
                     STORAGE API
                            │
                  STORAGE ENGINE (redb 4.3)
                            │
                           DISK
```

---

## 2. Pilihan Arsitektur VM: Aurion Native VM (AVM)

Aurion menetapkan **Aurion Native Virtual Machine (AVM)** sebagai mesin eksekusi resmi untuk memenuhi seluruh invariant konstitusional:

1. **Absolute Zero Unsafe Code (`AUR-ARCH-011`):** Seluruh engine AVM ditulis dalam 100% Safe Rust (`#![forbid(unsafe_code)]`).
2. **Absolute Zero Floating-Point Arithmetic (`AUR-ARCH-012`):** AVM melarang instruksi floating-point. Seluruh komputasi operan berbasis kata 256-bit / integer deterministik.
3. **Static Bytecode Verification:** Sebelum kontrak dikomit ke state blockchain, bytecode melalui verifikasi statis ketat untuk menjamin ketiadaan instruksi terlarang atau lompatan tidak sah.
4. **Built-in Anti-Reentrancy & Call Depth Limit:** Eksekusi kontrak memiliki batas kedalaman rekursif maksimal 16 frame panggilan.
5. **ACID Storage Integration:** Penyimpanan kontrak diisolasi per alamat kontrak pada tabel state `redb 4.3` dan dibuktikan ke state root melalui Sparse Merkle Tree (SMT).

---

## 3. Invariant Konstitusional Smart Contract (`AUR-VM-001` s.d `AUR-VM-010`)

### AUR-VM-001: Deterministic Execution Mandate
Setiap instruksi AVM wajib menghasilkan mutasi state yang identik pada seluruh node jaringan tanpa divergensi waktu, arsitektur CPU, atau endianness.

### AUR-VM-002: Zero Floating-Point Opcode Mandate
AVM tidak memiliki opcode `f32`, `f64`, atau representasi pecahan floating-point. Seluruh pembagian adalah pembagian bulat dengan sisa (division with remainder), dan pembagian dengan nol (`DIV by 0`) menghasilkan nilai nol deterministik atau kegagalan transaksi terkontrol.

### AUR-VM-003: Strict Integer Gas Metering
Setiap eksekusi bytecode wajib mengonsumsi gas terukur secara linear dan kuadratik (pada ekspansi memori). Jika gas habis (`OutOfGas`), seluruh mutasi state dibatalkan (reverted) dan seluruh gas limit hangus ke jaringan (burn/miner).

### AUR-VM-004: Atomic State Rollback on Revert
Jika kontrak memanggil instruksi `REVERT` atau mengalami kesalahan eksekusi (stack overflow, illegal opcode, memory out-of-bounds), seluruh perubahan saldo dan penyimpanan kontrak pada frame tersebut wajib di-rollback secara atomik ke snapshot sebelum pemanggilan.

### AUR-VM-005: Pre-Deployment Bytecode Verification
Kontrak tidak boleh dideploy ke state jika gagal melewati `BytecodeVerifier`. Bytecode wajib memenuhi panjang maksimum (24 KB), memiliki opcode yang valid, dan instruksi `JUMP`/`JUMPI` hanya boleh mendarat pada penanda `JUMPDEST`.

### AUR-VM-006: Contract Account State Model
Akun kontrak memiliki struktur yang selaras dengan akun biasa namun diperluas:
$$\text{ContractAccount} = \langle \text{balance}, \text{nonce}, \text{code\_hash}, \text{storage\_root} \rangle$$

### AUR-VM-007: Cryptographic Sycall Isolation
Syscall kriptografi (seperti Blake3 hashing atau verifikasi tanda tangan Ed25519) dieksekusi melalui opcode kanonikal berbiaya gas proporsional terhadap ukuran data input.

### AUR-VM-008: Call Stack Depth Boundary
Kedalaman panggilan antar-kontrak (`CALL`) dibatasi maksimal 16 level. Upaya memanggil melebihi batas ini akan langsung memicu error stack overflow.

### AUR-VM-009: Immutable Code Storage
Setelah kontrak dideploy dan diverifikasi, bytecode kontrak bersifat *immutable* (tidak dapat dimodifikasi). Upgrade kontrak hanya dapat dilakukan melalui pola proxy eksplisit yang disepakati oleh tata kelola.

### AUR-VM-010: Machine-Readable Event Logs
Event log kontrak wajib dicatat secara deterministik dengan topik berukuran 32-byte (Blake3 hash) dan payload data biner, serta dapat diinspeksi melalui Unified CLI dan RPC (`--output json`).

---

## 4. Set Instruksi Aurion VM (AVM ISA)

Instruksi AVM dikelompokkan ke dalam kategori berikut:

| Rentang Opcode | Kategori | Contoh Instruksi |
| :--- | :--- | :--- |
| `0x00 - 0x0F` | **Stop & Aritmetika** | `STOP`, `ADD`, `SUB`, `MUL`, `DIV`, `MOD`, `NOT` |
| `0x10 - 0x1F` | **Logika & Bitwise** | `LT`, `GT`, `EQ`, `ISZERO`, `AND`, `OR`, `XOR`, `SHL`, `SHR` |
| `0x20 - 0x2F` | **Kriptografi & Syscall**| `BLAKE3`, `ED25519_VERIFY` |
| `0x30 - 0x3F` | **Konteks & Lingkungan** | `ADDRESS`, `CALLER`, `ORIGIN`, `CALLVALUE`, `GASLIMIT`, `BLOCKHEIGHT`, `TIMESTAMP` |
| `0x50 - 0x5F` | **Stack & Memori** | `POP`, `MLOAD`, `MSTORE`, `MSTORE8`, `SLOAD`, `SSTORE`, `JUMP`, `JUMPI`, `PC`, `MSIZE`, `GAS`, `JUMPDEST` |
| `0x60 - 0x7F` | **Konstanta Push** | `PUSH1` s.d `PUSH32` |
| `0x80 - 0x8F` | **Duplikasi Stack** | `DUP1` s.d `DUP16` |
| `0x90 - 0x9F` | **Penukaran Stack** | `SWAP1` s.d `SWAP16` |
| `0xA0 - 0xA4` | **Penerbitan Event** | `LOG0` s.d `LOG4` |
| `0xF0 - 0xFF` | **Sistem & Terminasi** | `RETURN`, `REVERT`, `INVALID` |

---

## 5. Model Ekonomi Gas AVM

1. **Base Transaction Fee:** 10.000 Quanta.
2. **Contract Deployment Surcharge:** 50.000 Quanta + 200 Quanta per byte kode.
3. **Opcode Step Cost:**
   - Operasi Stack/Aritmetika Dasar: 3 - 5 gas.
   - Operasi Memori (`MLOAD`/`MSTORE`): 3 gas + biaya ekspansi memori kuadratik.
   - Operasi Storage (`SLOAD`): 100 gas.
   - Operasi Storage Mutasi (`SSTORE`): 500 gas (fresh key) / 200 gas (update).
   - Syscall Kriptografi (`BLAKE3`): 30 gas + 6 gas per 32-byte input.
   - Syscall Kriptografi (`ED25519_VERIFY`): 500 gas.
4. **Gas Burn:** 20% dari seluruh biaya eksekusi gas dimusnahkan secara permanen (*burned*), dan 80% diberikan kepada validator/proposer pembuat blok.
