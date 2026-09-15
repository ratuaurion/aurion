# 13 — AURION COMPATIBILITY & VERSIONING FRAMEWORK
## Kerangka Kerja Kompatibilitas Ekosistem, Kebijakan Semantic Versioning, dan Siklus Depresiasi Terencana

> **Hierarki Dokumen:**  
> `AURION CONSTITUTIONS` $\longrightarrow` `PROTOCOL SPECIFICATIONS` $\longrightarrow$ `00-APPLICATION-RULES` $\longrightarrow$ **`13-COMPATIBILITY-VERSIONING`**  
>
> **Status:** RATIFIED & LOCKED APPLICATION SPECIFICATION  
> **Klasifikasi:** Tata Kelola Versi & Kompatibilitas Perangkat Lunak (Versioning Standard)  
> **Versi:** 1.0.0-PROD  
> **Sifat Ketetapan:** Normatif Wajib (RFC 2119 / RFC 8174), SemVer 2.0.0 Conforming

---

## 1. Empat Dimensi Versi Independen (Four-Dimensional Versioning)

Untuk mencegah kekacauan operasional ketika ekosistem berkembang, Aurion memisahkan versi secara tegas ke dalam empat domain independen:

```text
┌────────────────────────────────────────────────────────┐
│             EMPAT DOMAIN VERSI PROTOKOL AURION         │
├───────────────────┬────────────────────────────────────┤
│ DOMAIN VERSI      │ CAKUPAN DAN DAMPAK PERUBAHAN       │
├───────────────────┼────────────────────────────────────┤
│ Protocol Version  │ Aturan konsensus, struktur blok,   │
│ (Versi Konsensus) │ STF, dan validitas transaksi.      │
├───────────────────┼────────────────────────────────────┤
│ RPC API Version   │ Skema JSON-RPC, nama endpoint,     │
│ (Versi Antarmuka) │ parameter, dan kode galat gateway. │
├───────────────────┼────────────────────────────────────┤
│ Client SDK Version│ Pustaka pengembang (Rust, Go, TS,  │
│ (Versi Pustaka)   │ Python) yang membungkus RPC & data.│
├───────────────────┼────────────────────────────────────┤
│ Database Schema   │ Struktur tabel internal indexer,   │
│ (Versi Indeks)    │ explorer, dan database relasional. │
└───────────────────┴────────────────────────────────────┘
```

---

## 2. Aturan Semantic Versioning (SemVer 2.0.0 Mandate)

Seluruh komponen perangkat lunak Aurion **MUST** mengadopsi format versi `MAJOR.MINOR.PATCH`:

1. **`MAJOR` (Perubahan Melanggar Kompatibilitas - Breaking Changes):**
   - **Pada Protokol:** Hard Fork konsensus (memerlukan aktivasi kuorum validator).
   - **Pada RPC / SDK:** Penghapusan endpoint atau pengubahan tipe data kembalian yang mematahkan integrasi klien lama.
2. **`MINOR` (Penambahan Fitur Kompatibel ke Belakang - Backwards-Compatible):**
   - **Pada Protokol:** Penambahan tipe transaksi baru atau optimasi non-breaking.
   - **Pada RPC / SDK:** Penambahan endpoint atau parameter opsional baru.
3. **`PATCH` (Perbaikan Bug & Keamanan):**
   - Perbaikan celah keamanan, optimasi performa, atau refactoring kode tanpa mengubah antarmuka eksternal.

---

## 3. Matriks Kompatibilitas Ekosistem Resmi (Compatibility Matrix)

Setiap rilis SDK dan RPC **MUST** mendokumentasikan matriks dukungan versi:

| Versi Protokol | Versi RPC Didukung | Versi SDK Minimum | Kompatibilitas Transaksi Masa Lalu |
| :---: | :---: | :---: | :---: |
| **Protocol v1.0** | `RPC v1.x` | `SDK v1.0+` | **100% Valid Selamanya** |
| **Protocol v1.1** | `RPC v1.x`, `RPC v2.x` | `SDK v1.2+` | **100% Valid Selamanya** |
| **Protocol v2.0** | `RPC v2.x` | `SDK v2.0+` | **Transaksi v1 Tetap Terbaca Historis** |

---

## 4. Kebijakan Depresiasi Terencana (Deprecation Policy)

Pengelola simpul, pengembang SDK, dan penyedia RPC **MUST** mengikuti protokol masa tenggang depresiasi:

1. **Pemberitahuan Dini Minimum 180 Hari:**  
   Sebuah endpoint RPC atau fungsi SDK **MUST NOT** dihapus tanpa pemberitahuan formal sekurang-kurangnya **180 hari (6 bulan)** sebelumnya melalui dokumentasi rilis dan header respons HTTP (`Sunset: <Date>` / `Deprecation: true`).
2. **Kekebalan Transaksi Masa Lalu (Historical Immutability):**  
   Meskipun versi transaksi baru diperkenalkan, transaksi berformat lama (`Transaction V1`) yang telah tercatat di dalam blok final masa lalu **MUST** tetap dapat didekode, diverifikasi, dan dibaca selamanya tanpa batas kedaluwarsa.
