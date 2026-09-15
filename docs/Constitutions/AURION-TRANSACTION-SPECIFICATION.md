# AURION TRANSACTION SPECIFICATION
## Spesifikasi Skema, Serialisasi, dan Validasi Transaksi Protokol Aurion

> **Dokumen Referensi:**  
> - [AURION CONSTITUTION.md](file:///c:/Projects/aurion/AURION%20CONSTITUTION.md)  
> - [AURION-MONETARY-POLICY-SPECIFICATION.md](file:///c:/Projects/aurion/AURION-MONETARY-POLICY-SPECIFICATION.md)  
> - [AURION-STATE-TRANSITION-SPECIFICATION.md](file:///c:/Projects/aurion/AURION-STATE-TRANSITION-SPECIFICATION.md)  
>
> **Status:** RATIFIED & LOCKED SPECIFICATION  
> **Klasifikasi:** Protokol Eksekusi & Jaringan Layer 1 (Transaction Wire & Mempool)  
> **Versi:** 1.0.0-PROD  
> **Sifat Ketetapan:** Biner Deterministik, Anti-Replay EIP-155, Zero-Float

---

## 1. Skema Objek Transaksi (Canonical Schema)

Setiap transaksi atomik di dalam protokol Aurion direpresentasikan oleh struktur data kanonikal:

```text
Transaction
├── version      : u32        (Format versi transaksi, default: 1)
├── chain_id     : u64        (Identifier rantai jaringan untuk proteksi replay)
├── nonce        : u64        (Penghitung sekuensial akun pengirim)
├── sender       : Address    (32-byte Blake3 alamat publik pengirim)
├── recipient    : Address    (32-byte Blake3 alamat publik penerima)
├── amount       : Quantum    (u128 integer bernilai > 0 dalam satuan atomik)
├── fee          : Quantum    (u128 integer biaya transaksi untuk jaringan)
├── payload      : ByteSeq    (Data tambahan/memo/smart script, panjang 0 s/d 64 KB)
└── signature    : Signature  (64-byte Ed25519 tanda tangan digital pengirim)
```

### 1.1 Tabel Spesifikasi Kolom Data (Field Layout)

| Nama Kolom | Tipe Data | Ukuran (Bytes) | Keterangan & Batasan |
| :--- | :--- | :--- | :--- |
| `version` | `u32` (BE) | $4\ \text{B}$ | Versi protokol transaksi. Saat ini wajib bernilai `0x00000001`. |
| `chain_id` | `u64` (BE) | $8\ \text{B}$ | ID jaringan resmi: `1` untuk Mainnet, `2` untuk Testnet. |
| `nonce` | `u64` (BE) | $8\ \text{B}$ | Wajib sama persis dengan `nonce` akun pengirim saat ini. |
| `sender` | `[u8; 32]` | $32\ \text{B}$ | Alamat kanonikal 32-byte akun pembayar. |
| `recipient` | `[u8; 32]` | $32\ \text{B}$ | Alamat kanonikal 32-byte akun tujuan penerima. |
| `amount` | `u128` (BE) | $16\ \text{B}$ | Nilai transfer dalam Quantum ($1\ \text{AUR} = 10^8\ Q$). |
| `fee` | `u128` (BE) | $16\ \text{B}$ | Biaya transaksi dalam Quantum. Wajib $\ge \text{MinFee}$. |
| `payload_len` | `u32` (BE) | $4\ \text{B}$ | Panjang data payload (maksimum $65.536\ \text{bytes}$). |
| `payload` | `[u8; len]` | $\text{Var}\ (\le 64\ \text{KB})$ | Raw bytes memo, bukti, atau input smart-script. |
| `signature` | `[u8; 64]` | $64\ \text{B}$ | Tanda tangan Ed25519 atas hash preimage penandatanganan. |

---

## 2. Serialisasi Biner Kanonikal (Canonical Codec)

Untuk memastikan bahwa tanda tangan digital bersifat unik dan kebal terhadap serangan pemalsuan tanda tangan (*signature malleability*), serialisasi biner wajib memenuhi aturan **Canonical Strict Encoding**:

1. **Urutan Byte Big-Endian:** Seluruh bilangan bulat (`u32`, `u64`, `u128`) diserialisasikan dalam format Big-Endian murni.
2. **Ketiadaan Padding Tersembunyi:** Setiap field dikemas secara langsung tanpa byte penyelarasan (*alignment padding*).
3. **Penetapan Batas Panjang Payload:** Data `payload` diawali dengan prefiks panjang 4-byte `u32` Big-Endian. Jika panjang yang dibaca tidak cocok dengan sisa buffer, serialisasi dianggap cacat.
4. **Larangan Sisa Byte (*Zero Trailing Bytes*):** Dekoder wajib menolak transaksi jika terdapat byte sisa setelah field tanda tangan 64-byte terakhir diproses.

---

## 3. Preimage Penandatanganan dan Proteksi Replay (Signing & Verification)

### 3.1 Konstruksi Preimage Penandatanganan
Tanda tangan digital **TIDAK** mencakup field `signature` itu sendiri. 

Preimage yang ditandatangani adalah serialisasi kanonikal dari seluruh field transaksi (dari `version` hingga `payload`), yang diawali dengan **Domain Separation Tag (DST)**:

```text
SigningPayload =
    "AURION-TX-V1" (12 Bytes ASCII)
    || version     (4 Bytes)
    || chain_id    (8 Bytes)
    || nonce       (8 Bytes)
    || sender      (32 Bytes)
    || recipient   (32 Bytes)
    || amount      (16 Bytes)
    || fee         (16 Bytes)
    || payload_len (4 Bytes)
    || payload     (N Bytes)
```

Hash ringkasan penandatanganan dihitung menggunakan Blake3:

$$\mathcal{H}_{\text{tx}} = \text{Blake3}(\text{SigningPayload})$$

### 3.2 Pembuatan dan Verifikasi Tanda Tangan
1. **Penandatanganan (Signing):**
   Pengirim menandatangani digest $\mathcal{H}_{\text{tx}}$ menggunakan kunci privat Ed25519 miliknya:
   $$\sigma = \text{Ed25519\_Sign}(\text{sk}_{\text{sender}},\; \mathcal{H}_{\text{tx}})$$
2. **Verifikasi Tanda Tangan (Verification):**
   Setiap simpul memverifikasi keabsahan tanda tangan secara ketat (*strict verification*):
   $$\text{Ed25519\_VerifyStrict}(\text{pk}_{\text{sender}},\; \mathcal{H}_{\text{tx}},\; \sigma) == \mathbf{TRUE}$$
3. **Pencocokan Alamat Pengirim:**
   $$\text{DeriveAddress}(\text{pk}_{\text{sender}}) == T_x.\text{sender}$$

### 3.3 Proteksi Serangan Replay (Dual-Tier Replay Protection)
Protokol mengeliminasi seluruh kemungkinan penyiaran ulang transaksi (*replay attacks*):
1. **Proteksi Lintas Jaringan (Cross-Chain Replay Protection):**  
   Penyertaan `chain_id` secara eksplisit pada preimage memastikan transaksi Mainnet tidak dapat disiarkan di Testnet atau fork jaringan lainnya.
2. **Proteksi Waktu & Urutan (In-Chain Replay Protection):**  
   Penyertaan `nonce` sekuensial akun memastikan transaksi yang telah dieksekusi sekali tidak dapat pernah dieksekusi kedua kalinya pada rantai yang sama.

---

## 4. Validasi Transaksi: Stateless vs Stateful

Proses validasi transaksi dibagi menjadi dua gerbang keamanan berurutan:

```text
Transaksi Masuk
      │
      ▼
┌────────────────────────────────────────────────────────┐
│             GERBANG 1: VALIDASI STATELESS              │
│  (Dapat divalidasi secara paralel tanpa akses state)  │
│                                                        │
│  1. Ukuran transaksi ≤ 65.536 bytes                    │
│  2. Format biner kanonikal tanpa trailing bytes       │
│  3. version == 1                                       │
│  4. chain_id == CURRENT_CHAIN_ID                       │
│  5. amount > 0                                         │
│  6. fee ≥ MinFeePerByte × UkuranTransaksi              │
│  7. Tanda tangan Ed25519 valid secara kriptografis     │
└───────────────────────────┬────────────────────────────┘
                            │ (Lolos)
                            ▼
┌────────────────────────────────────────────────────────┐
│              GERBANG 2: VALIDASI STATEFUL              │
│          (Memerlukan akses ke State Terkini)           │
│                                                        │
│  1. Akun pengirim (sender) terdaftar pada state        │
│  2. tx.nonce == account.nonce                          │
│  3. account.balance ≥ tx.amount + tx.fee (Checked)     │
│  4. amount + fee tidak mengalami luapan u128           │
└───────────────────────────┬────────────────────────────┘
                            │ (Lolos)
                            ▼
               Diterima Ke Dalam Mempool
```

---

## 5. Aturan Mempool dan Kebijakan Penggantian Biaya (Mempool & RBF)

### 5.1 Kriteria Penerimaan Mempool (Mempool Admission Rules)
Untuk melindungi simpul dari serangan kehabisan memori (*DoS Memory Exhaustion*):
1. **Kapasitas Maksimum Mempool:** Dibatasi maksimum $50.000$ transaksi atau $128\ \text{MB}$.
2. **Batas Rentang Nonce Masa Depan:** Mempool hanya menerima transaksi dengan:
   $$T_x.\text{nonce} \in \big[ \text{Account}.\text{nonce},\; \text{Account}.\text{nonce} + 16 \big]$$
   Transaksi dengan gap nonce $> 16$ ditolak langsung untuk mencegah spamming antrean memori.

### 5.2 Kebijakan Penggantian Biaya (Replace-By-Fee / RBF)
Jika sebuah transaksi $T_x'$ diterima dengan pengirim dan `nonce` yang identik dengan transaksi $T_x$ yang telah berada di dalam mempool:
1. Transaksi lama $T_x$ hanya digantikan jika biaya transaksi baru $T_x'.\text{fee}$ lebih tinggi sekurang-kurangnya $10\%$:
   $$T_x'.\text{fee} \ge \left\lfloor \frac{T_x.\text{fee} \times 110}{100} \right\rfloor$$
2. Jika syarat di atas tidak terpenuhi, transaksi baru $T_x'$ ditolak dengan alasan `FeeBumpInsufficient`.

---

## 6. Pengurutan Deterministik Transaksi dalam Blok (Transaction Ordering)

Ketika seorang Proposer merangkai blok kandidat, transaksi yang dimasukkan wajib diurutkan mengikuti aturan deterministik konsensus:

1. **Pengelompokan Berdasarkan Akun Pengirim:** Semua transaksi dari akun pengirim yang sama **WAJIB** disusun berurutan menaik berdasarkan `nonce` ($0, 1, 2, \dots$).
2. **Prioritas Pasar Biaya (Fee Density Sorting):** Antar pengirim yang berbeda, transaksi diurutkan menurun berdasarkan kepadatan biaya per byte (*fee density*):
   $$\text{FeeRate}(T_x) = \left\lfloor \frac{T_x.\text{fee}}{\text{SizeInBytes}(T_x)} \right\rfloor$$
   Transaksi dengan $\text{FeeRate}$ tertinggi diproses lebih awal.
3. **Penyelesaian Sengketa Determinis (Tie-Breaker):** Jika terdapat dua transaksi dengan $\text{FeeRate}$ yang persis sama, urutan ditentukan secara leksikografis berdasarkan $\text{Hash}(T_x)$.

---

## 7. Siklus Hidup Transaksi (Transaction Lifecycle)

```text
[DIBUAT]        Dompet membentuk transaksi & menandatangani secara offline
    ↓
[DISIARKAN]     Diserahkan ke RPC Node melalui P2P Mesh
    ↓
[MEMPOOL]       Lolos validasi Stateless & Stateful; menunggu giliran blok
    ↓
[TERPILIH]      Dimasukkan ke dalam proposal blok oleh Proposer terpilih
    ↓
[KOMITMEN]      Blok divalidasi dan mengumpulkan Commit Certificate (CC)
    ↓
[FINALITAS]     Blok final; saldo berpindah secara permanen; transaksi dihapus dari mempool
```

Sebuah transaksi yang telah mencapai tahap **FINALITAS** kebal terhadap segala bentuk pembatalan, revisi, maupun reorganisasi.
