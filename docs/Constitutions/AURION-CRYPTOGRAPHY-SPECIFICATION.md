# AURION CRYPTOGRAPHY SPECIFICATION
## Spesifikasi Kriptografi Kedaulatan, Standar Primitif, dan Derivasi Alamat Protokol Aurion

> **Dokumen Referensi:**  
> - [AURION CONSTITUTION.md](file:///c:/Projects/aurion/AURION%20CONSTITUTION.md)  
> - [AURION-MONETARY-POLICY-SPECIFICATION.md](file:///c:/Projects/aurion/AURION-MONETARY-POLICY-SPECIFICATION.md)  
> - [AURION-CONSENSUS-SPECIFICATION.md](file:///c:/Projects/aurion/AURION-CONSENSUS-SPECIFICATION.md)  
> - [AURION-STATE-TRANSITION-SPECIFICATION.md](file:///c:/Projects/aurion/AURION-STATE-TRANSITION-SPECIFICATION.md)  
> - [AURION-TRANSACTION-SPECIFICATION.md](file:///c:/Projects/aurion/AURION-TRANSACTION-SPECIFICATION.md)  
>
> **Status:** RATIFIED & LOCKED SPECIFICATION  
> **Klasifikasi:** Fondasi Kriptografis Protokol Layer 0 (Cryptographic Foundation)  
> **Versi:** 1.0.0-PROD  
> **Sifat Ketetapan:** Baku, Universal, Anti-Malleability, Zero-Ambiguity

---

## 1. Matriks Standar Primitif Kriptografi Resmi

Untuk mencegah fragmentasi implementasi perangkat lunak pada berbagai bahasa pemrograman (Rust, Go, Python, C++), protokol Aurion menetapkan secara mengikat standar kriptografi tunggal untuk setiap fungsi:

```text
┌────────────────────────────────────────────────────────────────────────┐
│               MATRIKS STANDAR KRIPTOGRAFI RESMI AURION                 │
├─────────────────────────┬──────────────────────────────────────────────┤
│ FUNGSI KRIPTOGRAFIS     │ SPESIFIKASI & ALGORITMA RESMI                │
├─────────────────────────┼──────────────────────────────────────────────┤
│ Fungsi Hash Utama       │ BLAKE3 (Digest 256-bit / 32 Bytes)           │
├─────────────────────────┼──────────────────────────────────────────────┤
│ Tanda Tangan Digital    │ Ed25519 (RFC 8032 / Strict Edwards25519)     │
├─────────────────────────┼──────────────────────────────────────────────┤
│ Format Kunci Privat     │ 32-Byte Cryptographic Seed (CSPRNG)          │
├─────────────────────────┼──────────────────────────────────────────────┤
│ Format Kunci Publik     │ 32-Byte Compressed Edwards Point             │
├─────────────────────────┼──────────────────────────────────────────────┤
│ Format Tanda Tangan     │ 64-Byte (R || S) Canonical Scalar Bound      │
├─────────────────────────┼──────────────────────────────────────────────┤
│ Format Alamat Kanonikal │ 32-Byte Blake3 Key-Derived Digest            │
├─────────────────────────┼──────────────────────────────────────────────┤
│ Format Alamat Display   │ Bech32m (BIP-350, Prefix: "aur1" / "aurt1")  │
├─────────────────────────┼──────────────────────────────────────────────┤
│ Pohon Otentikasi State  │ 256-Bit Sparse Merkle Tree (Blake3 SMT)      │
└─────────────────────────┴──────────────────────────────────────────────┘
```

Setiap node atau library yang menggunakan algoritma alternatif (misalnya SHA-256, Keccak-256, secp256k1, atau RSA) diklasifikasikan sebagai **tidak kompatibel dengan protokol Aurion**.

---

## 2. Standar Fungsi Hash: BLAKE3 (256-Bit)

### 2.1 Parameter Fungsi Hash
1. **Panjang Keluaran Standar:** Tepat 32 bytes (256 bits).
2. **Kekuatan Keamanan:** Tingkat keamanan 128-bit terhadap serangan tabrakan (*collision*) dan 256-bit terhadap preimage.
3. **Kekebalan Serangan:** Kebal secara matematis terhadap serangan ekstensi panjang (*length extension attacks*).
4. **Perbandingan Konstan:** Semua evaluasi kesetaraan hash dalam kode program wajib dieksekusi dalam **waktu konstan (*constant-time equality check*)** untuk mencegah kebocoran *timing attack*.

### 2.2 Mode Operasi Blake3 Resmi
1. **Mode Standar (Standard Hashing):**
   Digunakan untuk hashing konten transaksi, daun Merkle, dan header blok:
   $$\text{Digest} = \text{Blake3}(\text{Data})$$
2. **Mode Derivasi Kunci (Key Derivation Mode):**
   Digunakan untuk derivasi alamat dan pembuatan identifier kriptografis dengan pemisahan konteks (*context string*):
   $$\text{Digest} = \text{Blake3\_DeriveKey}(\text{ContextString},\; \text{KeyMaterial})$$

---

## 3. Standar Tanda Tangan Digital: Ed25519 (RFC 8032)

### 3.1 Parameter Kurva dan Ukuran Kunci
Aurion menggunakan kurva Edwards yang ekuivalen birasional dengan Curve25519:
- **Persamaan Kurva:** $-x^2 + y^2 = 1 - \frac{121665}{121666} x^2 y^2$ di atas medan berhingga $\mathbb{F}_{2^{255}-19}$.
- **Ukuran Kunci Privat (Seed):** 32 bytes ($256\ \text{bits}$).
- **Ukuran Kunci Publik:** 32 bytes ($256\ \text{bits}$) titik terkompresi.
- **Ukuran Tanda Tangan:** 64 bytes ($512\ \text{bits}$), terdiri dari $(R \parallel S)$ di mana $R$ adalah titik terkompresi 32-byte dan $S$ adalah skalar integer 32-byte.

### 3.2 Verifikasi Ketat Anti-Malleability (Strict Verification Mandate)
Untuk menjamin bahwa sebuah transaksi tidak dapat dimanipulasi tanda tangannya tanpa mengubah isi:
1. **Batas Skalar $S$ (Scalar Range Check):**  
   Nilai skalar $S$ wajib berada dalam rentang kanonikal:
   $$0 \le S < \ell$$
   Di mana $\ell = 2^{252} + 27742317777372353535851937790883648493$ adalah orde grup dasar kurva. Tanda tangan dengan $S \ge \ell$ **WAJIB DITOLAK LANGSUNG**.
2. **Penolakan Titik Subgrup Kecil (Small-Order Points):**  
   Titik $R$ yang berada pada subgrup berorde kecil (titik berorde 1, 2, 4, atau 8) wajib ditolak secara eksplisit.

---

## 4. Arsitektur Derivasi Alamat Akun (Address Derivation)

Alamat akun Aurion diturunkan melalui tiga tahap transformasi deterministik:

```text
[Kunci Privat (Seed 32-B)]
           ↓ Ed25519 Derive
[Kunci Publik pk (32-B)]
           ↓ Blake3 Derive-Key (Konteks: "AURION-ADDRESS-V1")
[Alamat Kanonikal Raw (32-B)]
           ↓ Bech32m Encoding (BIP-350, Checksum 6 Karakter)
[Alamat String Display: aur1qw508d6qejxtdg4y5r3zarvary0c5xw7kxpjzs]
```

### 4.1 Langkah 1: Derivasi Alamat Biner Kanonikal (32 Bytes)
Diberikan kunci publik Ed25519 $\text{pk} \in [u8; 32]$:

$$\text{RawAddress} = \text{Blake3\_DeriveKey}\Big( \text{Context: "AURION-ADDRESS-V1"},\; \text{pk} \Big)$$

Alamat kanonikal internal protokol adalah array biner tepat 32 bytes.

### 4.2 Langkah 2: Format Presentasi Ramah Manusia (Bech32m)
Untuk antarmuka pengguna luar, alamat kanonikal dienkode menggunakan skema **Bech32m (BIP-350)** dengan batas toleransi checksum terhadap mutasi karakter:

1. **Human-Readable Part (HRP):**
   - **Mainnet:** `aur`
   - **Testnet:** `aurt`
2. **Separator Karakter:** `1`
3. **Konversi Data:** Array 32 bytes (8-bit) dikonversi ke representasi 5-bit array (panjang 52 digit alfabet base32).
4. **Perhitungan Checksum Bech32m:** Menghasilkan 6 karakter kontrol integritas.
5. **Panjang Total String Alamat:**
   $$\text{Panjang Alamat Mainnet} = 3\ (\text{HRP}) + 1\ (\text{sep}) + 52\ (\text{data}) + 6\ (\text{checksum}) = 62\ \text{Karakter}$$

Contoh Alamat Mainnet Resmi:
```text
aur1q06v2w8x8p6e9r7t2y4u1i3o5p7a9s1d3f5g7h9j1k3l5z7x9c1v3b5n7m
```

---

## 5. Kamus Resmi Pemisahan Domain Kriptografis (Domain Separation Tags / DST)

Untuk mencegah serangan *cross-protocol collision* dan *signature replay* antar-fungsi yang berbeda, seluruh input hashing dan penandatanganan wajib menggunakan string pemisahan domain statis berikut:

| Konteks Protokol | String Tag Pemisahan Domain (DST) | Deskripsi Penggunaan |
| :--- | :--- | :--- |
| **Transaksi Pengguna** | `"AURION-TX-V1"` | Preimage penandatanganan transaksi transfer/kontrak |
| **Derivasi Alamat** | `"AURION-ADDRESS-V1"` | Konteks derive-key pembuatan alamat akun dari public key |
| **Proposal Blok BFT** | `"AURION-BFT-PROPOSAL-V1"` | Penandatanganan proposal blok oleh Proposer terpilih |
| **Suara Pre-vote BFT** | `"AURION-BFT-PREVOTE-V1"` | Penandatanganan suara fase 1 konsensus |
| **Suara Pre-commit BFT** | `"AURION-BFT-PRECOMMIT-V1"`| Penandatanganan suara fase 2 konsensus (Commit Certificate) |
| **Identifier Provenance** | `"AURION-PROVENANCE-RPI-V1"`| Derivasi Reward Provenance Identifier pada blok reward |
| **Simpul Internal Merkle**| `"AURION-SMT-BRANCH-V1"` | Hashing pasangan cabang pada Sparse Merkle Tree |
| **Daun Akun Merkle** | `"AURION-SMT-LEAF-V1"` | Hashing pasangan key-value akun pada daun Merkle |
| **Hash ID Transaksi (TxID)**| `"AURION-TX-ID-V1"` | Derivasi identifier transaksi untuk pelacakan unik |
| **Hash ID Blok (BlockHash)**| `"AURION-BLOCK-ID-V1"` | Derivasi hash kanonikal header blok |

---

## 6. Konstruksi Identifier Kriptografis Protokol (Cryptographic Identifiers)

### 6.1 Identifier Transaksi (TxID)
$$\text{TxID} = \text{Blake3}\Big( \text{"AURION-TX-ID-V1"} \mathbin{\Vert} \text{CanonicalSerialize}(T_x) \Big)$$
TxID bersifat unik secara global dan menjadi referensi resmi di mempool dan riwayat explorer.

### 6.2 Identifier Blok (BlockHash)
$$\text{BlockHash} = \text{Blake3}\Big( \text{"AURION-BLOCK-ID-V1"} \mathbin{\Vert} \text{CanonicalSerialize}(B.\text{header}) \Big)$$
BlockHash mengikat seluruh struktur transaksi via Merkle Tree Root, State Root, dan tinggi blok.

### 6.3 Reward Provenance Identifier (RPI)
Mengukuhkan amanat Bab 5 Konstitusi mengenai silsilah aset terotentikasi:
$$\text{RPI} = \text{Blake3}\Big( \text{"AURION-PROVENANCE-RPI-V1"} \mathbin{\Vert} \text{ChainID} \mathbin{\Vert} \text{BlockHash} \mathbin{\Vert} \text{Height} \mathbin{\Vert} \mathcal{S}(H) \Big)$$
RPI diterbitkan oleh protokol dan disimpan di dalam state sebagai penanda sah penerbitan uang baru.

---

## 7. Standar Keacakan dan Keamanan Kunci (Entropy & Key Hygiene)

1. **Sumber Entropi Wajib:** Pembuatan kunci privat baru wajib mengambil entropi langsung dari generator nomor acak kriptografis tingkat kernel OS (*CSPRNG*):
   - Linux: `getrandom(2)` dengan flag `GRND_RANDOM` atau `/dev/urandom`.
   - Windows: `BCryptGenRandom` dengan flag `BCRYPT_USE_SYSTEM_PREFERRED_RNG`.
2. **Pembersihan Memori Konstan (*Zeroize on Drop*):** Seluruh variabel memori yang menampung kunci privat atau seed penandatanganan wajib dibersihkan (*zeroized*) secara aman dari RAM segera setelah selesai digunakan guna mencegah kebocoran melalui teknik *core dump analysis* atau *cold boot attack*.
