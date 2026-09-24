# 09 — AURION CLIENT SDK SPECIFICATION
## Arsitektur Standar Pustaka Klien (SDK), Modul Seragam, dan Persyaratan Kompatibilitas Lintas Bahasa

> **Hierarki Dokumen:**  
> `AURION CONSTITUTIONS` $\longrightarrow$ `PROTOCOL SPECIFICATIONS` $\longrightarrow$ `00-APPLICATION-RULES` $\longrightarrow$ **`09-SDK-RULES`**  
>
> **Status:** RATIFIED & LOCKED APPLICATION SPECIFICATION  
> **Klasifikasi:** Standar Pustaka Pengembang (Client Software Development Kit Standard)  
> **Versi:** 1.0.0-PROD  
> **Sifat Ketetapan:** Normatif Wajib (RFC 2119 / RFC 8174), Deterministik Lintas Bahasa

---

## 1. Tujuan dan Aksioma Keseragaman SDK

Untuk mencegah terjadinya fragmentasi logika bisnis antara pengembang yang menggunakan bahasa pemrograman berbeda (Rust, Go, Python, TypeScript, Java, C#), seluruh implementasi Aurion SDK **MUST** mematuhi hukum keseragaman:

> **AKSIOMA KESERAGAMAN BINER SDK (THE CANONICAL IDENTICALITY AXIOM):**  
> Diberikan kumpulan input transaksi yang identik (kunci privat, chain_id, nonce, penerima, amount, fee, dan memo), seluruh SDK resmi Aurion dalam bahasa apa pun **MUST** menghasilkan array byte serialisasi kanonikal yang **100% IDENTIK HINGGA KE TINGKAT BIT TERAKHIR** dan menghasilkan `TxID` yang sama.

---

## 2. Struktur Modul Baku SDK (Uniform Modular Hierarchy)

Setiap implementasi Aurion SDK **MUST** mengorganisasi kode ke dalam 9 modul fungsional standar:

```text
aurion-sdk/
├── client      (Transport RPC HTTP/WebSocket, retry logic, failover node)
├── wallet      (Pembangkitan kunci CSPRNG, mnemonik, derivasi alamat, keystore)
├── transaction (TransactionBuilder, validasi lokal, komputasi TxID)
├── signing     (Penandatangan Ed25519 kanonikal dengan Domain Separation Tag)
├── rpc         (Wrapper type-safe pemanggilan metode namespace aur_)
├── address     (Konektor Bech32m, konversi pubkey ke alamat, validasi HRP)
├── amount      (Objek Quantum zero-float, aritmetika aman integer u128)
├── fee         (Helper kalkulasi estimasi fee & alokasi validator 100%)
└── codec       (Serialisasi & deserialisasi biner Big-Endian murni)
```

---

## 3. Spesifikasi Rinci Modul SDK

### 3.1 Modul `amount` (Monetary Engine)
1. Modul `amount` **MUST NOT** menggunakan tipe data pecahan floating-point IEEE 754 (`f32`, `f64`, `float`, `double`) untuk kalkulasi moneter.
2. Seluruh representasi moneter **MUST** disimpan dalam unit atomik integer Quantum (u128 / BigInt), dengan skala kanonikal $1\text{ AUR} = 10^9\text{ Quantum}$ ($1.000.000.000\text{ Quanta}$).
3. SDK **MUST** menyediakan fungsi parsing string desimal aman:
   ```typescript
   // Contoh TypeScript
   const amount = Quantum.fromAurString("1.500000000"); // Menghasilkan 1_500_000_000n Quanta
   ```
   Jika string input memuat lebih dari 9 digit desimal di belakang koma, SDK **MUST** melempar pengecualian `InvalidPrecisionError`.

### 3.2 Modul `address` (Kriptografi Alamat)
1. Menghasilkan alamat kanonikal raw (32 bytes) dari kunci publik Ed25519 menggunakan:
   $$\text{RawAddress} = \text{Blake3\_DeriveKey}(\text{"AURION-ADDRESS-V1"},\; \text{PublicKey})$$
2. Mengenkode dan mendekode representasi display **Bech32m (BIP-350)**:
   - Validasi HRP aktif (`aur` vs `aurt`).
   - Panjang alamat tepat 62 karakter (Mainnet).

### 3.3 Modul `transaction` & `signing`
1. Menyediakan pola pembangunan transaksi fasih (*Fluent Builder Pattern*):
   ```rust
   // Contoh Rust
   let tx = TransactionBuilder::new()
       .chain_id(1)
       .nonce(current_nonce)
       .recipient(recipient_addr)
       .amount(Quantum::from_aur(10))
       .fee(Quantum::new(10_000))
       .payload(b"INV-9041")
       .build_and_sign(&keypair)?;
   ```
2. Injeksi Domain Separation Tag statis `"AURION-TX-V1"` wajib diterapkan pada setiap preimage sebelum diserahkan ke fungsi `ed25519_sign`.

### 3.4 Modul `client` & `rpc`
1. Pustaka klien **MUST** mendukung failover otomatis: Jika simpul RPC utama gagal merespons dalam batas waktu timeout (default: $10\ \text{detik}$), klien **SHOULD** mengalihkan permintaan ke endpoint cadangan secara transparan.
2. Membuka metode penyerahan transaksi dengan pelacakan hingga final:
   ```typescript
   // Menunggu hingga blok terverifikasi memiliki Commit Certificate
   const receipt = await client.sendTransactionAndWaitForFinality(tx);
   ```

---

## 4. Persyaratan Verifikasi Kepatuhan Suite Uji SDK

Sebelum sebuah pustaka SDK dirilis untuk publik:

1. Pengembang SDK **MUST** mengintegrasikan seluruh **Vektor Uji Emas Dokumen 11 ([AURION-REFERENCE-TEST-VECTORS.md](file:///c:/Projects/aurion/docs/Constitutions/AURION-REFERENCE-TEST-VECTORS.md))** ke dalam suite uji otomatis CI/CD (`cargo test`, `go test`, `pytest`, atau `npm test`).
2. Suite uji **MUST** memverifikasi:
   - Kesesuaian hash Blake3 string kosong, kata kunci, dan derive-key.
   - Pembangkitan kunci Ed25519 dan tanda tangan deterministik.
   - Alamat Bech32m Mainnet dan Testnet.
   - Serialisasi biner transaksi tepat 184 bytes (+ payload).
   - Perhitungan TxID dan BlockHash.
