# AURION APPLICATION RULE 14: STORAGE ARCHITECTURE & PERSISTENCE SPECIFICATION
**Normative Standard:** RFC 2119 / RFC 8174 / AUR-ARCH-001 / AUR-ARCH-005 / AUR-ARCH-011  
**Status:** CANONICAL RATIFIED  
**Pillar:** State Storage, Embedded Persistence & Crash Recovery

---

## 1. Scope & Architectural Principles

Storage pada Aurion bukanlah modul eksternal yang disisipkan belakangan, melainkan subsistem infrastruktur inti di bawah lapisan *State & Execution*.

```text
                    AURION
                       │
                 SINGLE BINARY (/bin/aurion)
                       │
                    RUNTIME
                       │
        ┌──────────────┼──────────────┐
        │              │              │
    Consensus       Execution      Network
        │              │              │
        └──────────────┼──────────────┘
                       │
                     STATE
                       │
                ┌──────┴──────┐
                │   STORAGE   │ (StateStore Trait)
                │   ENGINE    │
                └──────┬──────┘
                       │
                 redb 4.3 (ACID, MVCC, WAL, Pure Rust)
                       │
                     DISK
```

### Prinsip Fundamental:
1. **Pemisahan Semantik & Fisik:** Protokol Aurion (STF, konsensus, moneter) tidak boleh bergantung langsung pada API database konkret. Protokol berinteraksi secara murni melalui trait abstraksi `StateStore`.
2. **Pure-Rust Mandate (`redb` bukan `RocksDB`):** Aurion secara ketat menolak database yang memerlukan runtime C/C++ eksternal (seperti RocksDB atau LevelDB) untuk menjaga invariant:
   - Kompilasi silang (*cross-compilation*) 100% deterministik dan bebas dependensi compiler C++.
   - Zero unsafe di lapisan aplikasi Aurion (`#![forbid(unsafe_code)]`).
   - Keamanan memori bawaan Rust.
3. **Atomisitas Transaksional Penuh (Multi-Table Atomic Commit):** State perubahan akun, blok baru, indeks hash, dan sertifikat konsensus WAJIB dikomit bersamaan dalam satu `WriteTransaction` ACID yang tunggal.

---

## 2. Abstraksi Database (`trait StateStore`)

Protokol mengekspos trait penyimpanan netral:

```rust
pub trait StateStore: Send + Sync {
    fn get_account(&self, address: &Address) -> Result<Option<Account>, StorageError>;
    fn get_block_by_height(&self, height: u64) -> Result<Option<Block>, StorageError>;
    fn get_block_by_hash(&self, hash: &Hash256) -> Result<Option<Block>, StorageError>;
    fn get_certificate(&self, height: u64) -> Result<Option<CommitCertificate>, StorageError>;
    fn get_latest_height(&self) -> Result<Option<u64>, StorageError>;
    fn get_all_accounts(&self) -> Result<HashMap<Address, Account>, StorageError>;

    fn commit_block_atomic(
        &self,
        block: &Block,
        certificate: &CommitCertificate,
        updated_accounts: &[(Address, Account)],
    ) -> Result<(), StorageError>;
}
```

---

## 3. Canonical Storage Layout (Keyspace & Tables)

Engine fisik `redb` menggunakan tabel bertipe (*strongly-typed tables*):

| Nama Tabel | Tipe Kunci | Tipe Nilai | Deskripsi |
| :--- | :--- | :--- | :--- |
| `accounts` | `&[u8; 32]` (Address bytes) | `&[u8]` (Canonical Account) | Keadaan saldo dan nonce akun terkini. |
| `blocks_by_height` | `u64` (Big-Endian Height) | `&[u8]` (Canonical Block) | Data blok kanonikal per tinggi blok. |
| `blocks_by_hash` | `&[u8; 32]` (Block Hash) | `u64` (Height) | Indeks pencarian tinggi blok dari hash. |
| `certificates` | `u64` (Big-Endian Height) | `&[u8]` (Canonical Certificate) | Bukti konsensus BFT kuorum >2/3. |
| `metadata` | `&str` (Key Name) | `&[u8]` (Value Bytes) | Informasi rantai, versi protokol, genesis hash, latest height. |

---

## 4. Atomic Block Commit & WAL Recovery

### 4.1 Siklus Commit Blok
Ketika blok $H$ selesai divalidasi dan disetujui konsensus:
1. Membuka `db.begin_write()`.
2. Menyimpan seluruh akun yang berubah pada tabel `accounts`.
3. Menyimpan payload blok pada tabel `blocks_by_height`.
4. Mendaftarkan hash blok pada tabel `blocks_by_hash`.
5. Menyimpan sertifikat finalitas pada tabel `certificates`.
6. Memperbarui `latest_height` pada tabel `metadata`.
7. Menjalankan `write_txn.commit()`. Jika terjadi kegagalan/crash sebelum titik ini, disk tetap bersih tanpa perubahan parsial.

### 4.2 Prosedur Crash Recovery
Saat node dinyalakan kembali (*startup*):
1. Membuka database `redb`.
2. Membaca `latest_height` dari tabel `metadata`.
3. Jika database kosong: menginisialisasi blok Genesis ($H=0$) dan alokasi 35% hard cap.
4. Jika database berisi:
   - Memuat blok terbaru pada `latest_height`.
   - Merekonstruksi state akun dari tabel `accounts`.
   - Menghitung akar state sparse Merkle tree (`compute_accounts_state_root`).
   - Memverifikasi bahwa akar state yang dihitung **COCOK PERSIS 100%** dengan `state_root` pada header blok terakhir.
   - Jika terdapat diskrepansi: node menolak booting demi integritas konsensus (*fail-fast*).

---

## 5. Pruning & Archive Policy

- **Full/Archive Mode:** Menyimpan seluruh blok dan sertifikat dari Genesis hingga terkini tanpa pemangkasan.
- **Snapshot Support:** Ekspor state root dan tabel akun pada tinggi terfinalisasi tertentu untuk sinkronisasi cepat peer baru (*fast sync*).
