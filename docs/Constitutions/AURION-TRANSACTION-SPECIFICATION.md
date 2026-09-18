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
> **Sifat Ketetapan:** Biner Deterministik, Anti-Replay Lintas-Jaringan (`chain_id`), Zero-Float

---

## 1. Skema Objek Transaksi (Canonical Schema)

Setiap transaksi atomik di dalam protokol Aurion direpresentasikan oleh struktur data kanonikal:

```text
Transaction
├── version      : u16        (Format versi transaksi, wajib: 1)
├── chain_id     : u32        (Identifier rantai jaringan untuk proteksi replay)
├── tx_type      : u8         (Jenis transaksi: lihat TxType)
├── flags        : u8         (Bitmask opsi transaksi, default: 0x00)
├── sender       : Address    (32-byte Blake3 alamat publik pengirim)
├── recipient    : Address    (32-byte Blake3 alamat publik penerima)
├── nonce        : u64        (Penghitung sekuensial akun pengirim)
├── amount       : Quantum    (u128 integer bernilai > 0 dalam satuan atomik)
├── fee          : Quantum    (u128 integer biaya transaksi untuk jaringan)
├── valid_until  : u64        (Batas waktu kadaluarsa transaksi, 0 = tanpa batas)
├── payload      : ByteSeq    (Data tambahan/memo/smart script, panjang 0 s/d 24 KB)
└── signature    : Signature  (64-byte Ed25519 tanda tangan digital pengirim)
```

### 1.1 Tabel Spesifikasi Kolom Data (Field Layout)

| Nama Kolom | Tipe Data | Ukuran (Bytes) | Keterangan & Batasan |
| :--- | :--- | :--- | :--- |
| `version` | `u16` (BE) | $2\ \text{B}$ | Versi protokol transaksi. Saat ini wajib bernilai `0x0001`. |
| `chain_id` | `u32` (BE) | $4\ \text{B}$ | ID jaringan resmi. Mainnet = `1001` (`GENESIS_CHAIN_ID`). |
| `tx_type` | `u8` | $1\ \text{B}$ | `0x01` Transfer, `0x02` Stake, `0x03` Unstake, `0x04` GovernanceVote, `0x05` ContractDeploy, `0x06` ContractCall. |
| `flags` | `u8` | $1\ \text{B}$ | Bitmask opsi. Wajib `0x00` bila tidak ada opsi aktif. |
| `sender` | `[u8; 32]` | $32\ \text{B}$ | Alamat kanonikal 32-byte akun pembayar. |
| `recipient` | `[u8; 32]` | $32\ \text{B}$ | Alamat kanonikal 32-byte akun tujuan penerima. |
| `nonce` | `u64` (BE) | $8\ \text{B}$ | Wajib sama persis dengan `nonce` akun pengirim saat ini. |
| `amount` | `u128` (BE) | $16\ \text{B}$ | Nilai transfer dalam Quantum ($1\ \text{AUR} = 10^8\ Q$). |
| `fee` | `u128` (BE) | $16\ \text{B}$ | Biaya transaksi dalam Quantum. Wajib $\ge \text{MinFee}$. |
| `valid_until` | `u64` (BE) | $8\ \text{B}$ | Batas waktu kadaluarsa (Unix epoch detik). `0` = tanpa batas. |
| `payload_len` | `u32` (BE) | $4\ \text{B}$ | Panjang data payload (maksimum $24.576\ \text{bytes}$). |
| `payload` | `[u8; len]` | $\text{Var}\ (\le 24\ \text{KB})$ | Raw bytes memo, bukti, atau input smart-script. |
| `signature` | `[u8; 64]` | $64\ \text{B}$ | Tanda tangan Ed25519 atas preimage penandatanganan. |

Ukuran basis transaksi tanpa payload adalah $184\ \text{Bytes}$ (`TRANSACTION_BASE_BYTES`); setiap transaksi menambahkan prefiks panjang `payload_len` $4\ \text{B}$ sebelum `payload`.

---

## 2. Serialisasi Biner Kanonikal (Canonical Codec)

Untuk memastikan bahwa tanda tangan digital bersifat unik dan kebal terhadap serangan pemalsuan tanda tangan (*signature malleability*), serialisasi biner wajib memenuhi aturan **Canonical Strict Encoding**:

1. **Urutan Byte Big-Endian:** Seluruh bilangan bulat (`u16`, `u32`, `u64`, `u128`) diserialisasikan dalam format Big-Endian murni.
2. **Ketiadaan Padding Tersembunyi:** Setiap field dikemas secara langsung tanpa byte penyelarasan (*alignment padding*).
3. **Penetapan Batas Panjang Payload:** Data `payload` diawali dengan prefiks panjang 4-byte `u32` Big-Endian. Jika panjang yang dibaca tidak cocok dengan sisa buffer, serialisasi dianggap cacat.
4. **Larangan Sisa Byte (*Zero Trailing Bytes*):** Dekoder wajib menolak transaksi jika terdapat byte sisa setelah field tanda tangan 64-byte terakhir diproses.

---

## 3. Preimage Penandatanganan dan Proteksi Replay (Signing & Verification)

### 3.1 Konstruksi Preimage Penandatanganan
Tanda tangan digital **TIDAK** mencakup field `signature` itu sendiri. 

Preimage yang ditandatangani adalah serialisasi kanonikal dari seluruh field transaksi (dari `version` hingga `payload`), yang diawali dengan **Domain Separation Tag (DST)** `AURION-TX-V1` dan satu byte pemisah `0x00`:

```text
SigningPayload =
    "AURION-TX-V1" (12 Bytes ASCII, DST_TX)
    || 0x00         (1 Byte pemisah domain)
    || version     (2 Bytes)
    || chain_id    (4 Bytes)
    || tx_type     (1 Byte)
    || flags       (1 Byte)
    || sender      (32 Bytes)
    || recipient   (32 Bytes)
    || nonce       (8 Bytes)
    || amount      (16 Bytes)
    || fee         (16 Bytes)
    || valid_until (8 Bytes)
    || payload_len (4 Bytes)
    || payload     (N Bytes)
```

Tanda tangan Ed25519 dibuat langsung atas byte `SigningPayload` (bukan atas hash-nya):

$$\sigma = \text{Ed25519\_Sign}(\text{sk}_{\text{sender}},\; \text{SigningPayload})$$

Untuk identitas transaksi, `TxID` dihitung secara terpisah dengan Blake3 derive-key atas serialisasi kanonikal penuh (termasuk `signature`) memakai konteks `AURION-TX-ID-V1`:

$$\text{TxID} = \text{Blake3DeriveKey}\big(\texttt{"AURION-TX-ID-V1"},\; \text{EncodeCanonical}(T_x)\big)$$

### 3.2 Pembuatan dan Verifikasi Tanda Tangan
1. **Penandatanganan (Signing):**
   Pengirim menandatangani byte `SigningPayload` menggunakan kunci privat Ed25519 miliknya:
   $$\sigma = \text{Ed25519\_Sign}(\text{sk}_{\text{sender}},\; \text{SigningPayload})$$
2. **Verifikasi Tanda Tangan (Verification):**
   Setiap simpul memverifikasi keabsahan tanda tangan secara ketat (*strict verification*):
   $$\text{Ed25519\_VerifyStrict}(\text{pk}_{\text{sender}},\; \text{SigningPayload},\; \sigma) == \mathbf{TRUE}$$
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
│  1. Panjang payload ≤ 24.576 bytes                     │
│  2. Format biner kanonikal tanpa trailing bytes       │
│  3. version == 1                                       │
│  4. chain_id == GENESIS_CHAIN_ID (1001)                │
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
