# 02. Spesifikasi Skema Serialisasi Canonical L2 Batch & Kompresi Calldata (DA)

> **Status:** RATIFIED APPLICATION SPECIFICATION (L2-SPEC-02)  
> **Sub-Project:** `aurion-l2-scaling`  
> **Execution Phase:** `FASE L2-0` (Task ID: `L2-TSK-002`)  
> **Target Domain:** Layer-1 Data Availability (DA) & Rollup Batch Framing  
> **Standar Kata Kunci:** RFC 2119 / RFC 8174 (`MUST`, `MUST NOT`, `REQUIRED`, `SHALL`)

---

## 1. Ringkasan Eksekutif (*Executive Summary*)

Sebagai rollup berdaulat, Aurion Layer-2 memanfaatkan ledger Layer-1 sebagai lapisan ketersediaan data (*Data Availability / DA*). Seluruh data transaksi yang dieksekusi di L2 wajib dipublikasikan ke L1 dalam bentuk *calldata* yang ringkas, deterministik, dan terkompresi.

Dokumen ini menetapkan spesifikasi format pembingkaian biner kanonikal (*canonical binary framing*) beridentitas `AUL2`, mekanisme serialisasi metadata batch, teknik kompresi transaksi, serta tata cara verifikasi integritas data sebelum diterima oleh kontrak penyelesaian di L1.

---

## 2. Invariant Ketersediaan Data (*Data Availability Invariants*)

* **`L2-DA-001` (Kanonikalitas Calldata L1):** Rekonstruksi status L2 dari blok genesis `MUST` dapat dilakukan secara mandiri hanya dengan membaca riwayat calldata batch yang diposting ke L1.
* **`L2-DA-002` (Komitmen Integritas Blake3):** Intisari Blake3 dari keseluruhan calldata batch `MUST` sama persis dengan `calldata_hash` yang didaftarkan pada saat pemanggilan metode `verify_state_transition`.
* **`AUR-ARCH-011` (Zero Unsafe):** Seluruh kode parser dan serializer biner calldata wajib berjalan dalam mode aman murni (`#![forbid(unsafe_code)]`).
* **`AUR-ARCH-012` (Zero Float):** Parameter nominal dan biaya transaksi dalam payload tetap menggunakan bilangan bulat presisi tetap `Quantum` $u128$.

---

## 3. Format Pembingkaian Biner Kanonikal (*Binary Frame Layout*)

Setiap paket calldata batch yang diposting ke Layer-1 wajib diawali dengan header tetap (*fixed-size header*) sepanjang **102 byte**:

```text
+-----------------------------------------------------------------------+
|                      AURION L2 BATCH FRAME HEADER                     |
+-----------------------------------+-----------------------------------+
| Field                             | Size / Type                       |
+-----------------------------------+-----------------------------------+
| Magic Bytes ("AUL2")              | 4 Bytes (0x41, 0x55, 0x4C, 0x32)  |
| Protocol Version                  | 1 Byte  (0x01)                    |
| Compression Flags                 | 1 Byte  (Bitmap)                  |
| Batch Index                       | 8 Bytes (u64 Big-Endian)          |
| Prev State Root                   | 32 Bytes (Hash256)                |
| New State Root                    | 32 Bytes (Hash256)                |
| Start Block Number                | 8 Bytes (u64 Big-Endian)          |
| End Block Number                  | 8 Bytes (u64 Big-Endian)          |
| Transaction Count                 | 4 Bytes (u32 Big-Endian)          |
| Payload Length (N)                | 4 Bytes (u32 Big-Endian)          |
+-----------------------------------+-----------------------------------+
| Compressed Transactions Payload   | N Bytes                           |
+-----------------------------------------------------------------------+
```

### 3.1 Definisi Field Header
1. **Magic Bytes (`[u8; 4]`):** Karakter ASCII `A`, `U`, `L`, `2` (`[0x41, 0x55, 0x4C, 0x32]`). Frame dengan magic byte selain ini `MUST` ditolak seketika (*fail-fast*).
2. **Protocol Version (`u8`):** Versi skema serialisasi. Versi awal adalah `0x01`.
3. **Compression Flags (`u8`):**
   - `0x00`: *Raw Canonical* (tanpa kompresi tambahan).
   - `0x01`: *Compact Bit-Packed* (pengurangan redundansi field transaksi).
   - `0x02`: *Dictionary Compressed* (menggunakan tabel kamus alamat aktif L2).
4. **Batch Index (`u64`):** Nomor urut batch monotoik naik.
5. **Prev State Root (`[u8; 32]`):** Komitmen status L2 sebelum transaksi dalam batch ini dieksekusi.
6. **New State Root (`[u8; 32]`):** Komitmen status L2 setelah seluruh transaksi dalam batch berhasil dieksekusi.
7. **Start & End Block (`u64`):** Rentang blok L2 yang dicakup oleh batch.
8. **Transaction Count (`u32`):** Jumlah transaksi individual yang terkandung dalam payload.
9. **Payload Length (`u32`):** Ukuran byte ($N$) dari isi transaksi yang terkompresi.

---

## 4. Skema Kompresi Transaksi (*Transaction Payload Packing*)

Dalam mode `0x00` (Raw Canonical), setiap transaksi dikemas secara sekuensial:

```text
Per Transaksi (Raw):
- Sender Address       : 32 Bytes
- Recipient Address    : 32 Bytes
- Amount (Quantum)     : 16 Bytes (u128 BE)
- Fee (Quantum)        : 16 Bytes (u128 BE)
- Nonce                : 8 Bytes (u64 BE)
- Signature (Ed25519)  : 64 Bytes
- Payload Length (M)   : 2 Bytes (u16 BE)
- Payload Data         : M Bytes
Total Basis Per Tx     : 170 + M Bytes
```

Dalam mode `0x01` (Compact Bit-Packed), efisiensi ditingkatkan hingga >40%:
- Eliminasi byte nol pada field `amount` dan `fee` menggunakan skema *Length-Prefixed Varint*.
- Delta encoding untuk `nonce` (selisih dari transaksi sebelumnya jika akun sama).

---

## 5. Tata Cara Validasi Integritas Data (*Integrity Verification*)

Sebelum memanggil `verify_state_transition` pada kontrak L1:
1. **Pemeriksaan Panjang Minimum:** Calldata `MUST` memiliki panjang $\ge 102$ byte.
2. **Pemeriksaan Magic Header:** Byte 0..4 `MUST` tepat bernilai `[0x41, 0x55, 0x4C, 0x32]`.
3. **Pemeriksaan Konsistensi Panjang:** Total panjang calldata `MUST` tepat sama dengan $102 + \text{Payload Length}$.
4. **Pemeriksaan Intisari DA:**
   $$\text{calldata\_hash} = \text{Blake3}(\text{Entire Calldata Frame})$$
   Intisari ini wajib diserahkan sebagai argumen terakhir pada pemanggilan metode verifikasi di kontrak `L2SettlementBridge`.

---

## 6. Penanganan Kesalahan (*Error Conditions*)

Parser framing calldata wajib mengembalikan galat deskriptif jika ditemukan pelanggaran:
- `InvalidMagicHeader`: Magic byte tidak cocok dengan `AUL2`.
- `UnsupportedVersion`: Versi protokol tidak didukung oleh node pemroses.
- `IncompleteHeader`: Ukuran data kurang dari 102 byte.
- `PayloadLengthMismatch`: Panjang data aktual berbeda dengan nilai `Payload Length` di header.
- `CorruptedTransactionPayload`: Kegagalan membaca transaksi dari payload.

---

## 7. Referensi Kode Terkait

- Kode biner dan pengujian unit: [`src/l2/codec.rs`](../../../src/l2/codec.rs).
- Penggunaan dalam perakitan batch: [`src/l2/sequencer.rs`](../../../src/l2/sequencer.rs).
