# 06 — AURION SERIALIZATION & WIRE PROTOCOL SPECIFICATION
## Spesifikasi Formal Representasi Biner Kanonikal dan Protokol Jaringan P2P Wire

> **Hierarki Dokumen:**  
> `00 — AURION CONSTITUTION` $\longrightarrow$ `01 — MONETARY` $\longrightarrow$ `02 — CONSENSUS` $\longrightarrow$ `03 — STATE TRANSITION` $\longrightarrow$ `04 — TRANSACTION` $\longrightarrow$ `05 — CRYPTOGRAPHY` $\longrightarrow$ **`06 — SERIALIZATION & WIRE PROTOCOL`**  
>
> **Status:** AUDITED & LOCKED SPECIFICATION  
> **Klasifikasi:** Protokol Enkode Data & Jaringan Layer 1 (Serialization & P2P Wire)  
> **Versi:** 1.0.0-PROD  
> **Sifat Ketetapan:** Biner Deterministik Tanpa Ambiguitas (*Zero-Ambiguity*), Anti-Malleability, Anti-DoS

---

# BAGIAN 1: KANONIKAL SERIALISASI BINER (CANONICAL BINARY CODEC)

## 1. Prinsip dan Aksioma Serialisasi Kanonikal

Untuk menjamin bahwa setiap objek di dalam Aurion menghasilkan representasi byte yang unik, identik, dan tidak dapat dimanipulasi (*tamper-proof*), seluruh struktur data wajib mengikuti empat aksioma serialisasi:

1. **Aksioma Keunikan Tunggal (*Bijective Mapping*):**  
   Setiap objek data $\mathcal{O}$ memiliki tepat satu representasi byte biner $\mathcal{B} = \text{Encode}(\mathcal{O})$, dan setiap representasi byte $\mathcal{B}$ hanya dapat didekode menjadi tepat satu objek $\mathcal{O} = \text{Decode}(\mathcal{B})$.
2. **Aksioma Big-Endian Murni:**  
   Seluruh tipe data numerik integer (`u16`, `u32`, `u64`, `u128`) diserialisasikan secara eksklusif dalam urutan byte Big-Endian (Most Significant Byte first).
3. **Aksioma Penolakan Sisa Byte (*Strict Zero-Trailing Bytes*):**  
   Fungsi dekoder wajib mengembalikan galat fatal `TrailingBytesError` jika setelah seluruh elemen objek selesai dibaca masih terdapat sisa byte dalam buffer.
4. **Aksioma Batas Alokasi Eksplisit (*Explicit Allocation Bounds*):**  
   Setiap struktur data dengan panjang dinamis (vektor/buffer byte) wajib diawali dengan prefiks panjang integer `u32` (Big-Endian) dan dibatasi oleh plafon memori maksimum sebelum buffer dialokasikan di RAM.

---

## 2. Format Primitif Dasar Biner

| Tipe Primitif | Panjang Byte | Representasi Biner | Batasan Validitas |
| :--- | :--- | :--- | :--- |
| `u8` / `bool` | $1\ \text{B}$ | `0x00` s/d `0xFF` (`bool`: `0x00` = false, `0x01` = true) | `bool` selain `0x00`/`0x01` ditolak |
| `u16` | $2\ \text{B}$ | Big-Endian byte order | Nilai $0$ s/d $2^{16}-1$ |
| `u32` | $4\ \text{B}$ | Big-Endian byte order | Nilai $0$ s/d $2^{32}-1$ |
| `u64` | $8\ \text{B}$ | Big-Endian byte order | Nilai $0$ s/d $2^{64}-1$ |
| `u128` (Quantum)| $16\ \text{B}$| Big-Endian byte order | Maksimum $\le S_{\max}^{(Q)}$ |
| `Hash256` | $32\ \text{B}$| Raw digest byte array | Tepat 32 bytes |
| `Address` | $32\ \text{B}$| Raw Blake3 derived digest | Tepat 32 bytes |
| `Signature` | $64\ \text{B}$| Ed25519 $(R \parallel S)$ canonical | $S < \ell$ wajib divalidasi |
| `ByteSeq` | $4 + N\ \text{B}$ | Prefiks `len: u32` (BE) diikuti $N$ bytes raw | $N \le \text{MaxPayloadBound}$ |

---

## 3. Spesifikasi Biner Objek Protokol

```text
┌────────────────────────────────────────────────────────────────────────┐
│               ARSITEKTUR STRUKTUR DATA BINER KANONIKAL                 │
├───────────────────┬────────────────────────────────────────────────────┤
│ OBJEK PROTOKOL    │ PANJANG TETAP / FORMULA UKURAN                     │
├───────────────────┼────────────────────────────────────────────────────┤
│ Header Blok       │ Tepat 124 Bytes (Fixed-size)                       │
├───────────────────┼────────────────────────────────────────────────────┤
│ Suara (Vote)      │ Tepat 117 Bytes (Fixed-size)                       │
├───────────────────┼────────────────────────────────────────────────────┤
│ Validator Entry   │ Tepat 72 Bytes (Fixed-size)                        │
├───────────────────┼────────────────────────────────────────────────────┤
│ Transaksi         │ 184 Bytes + N Bytes Payload (Variable)             │
├───────────────────┼────────────────────────────────────────────────────┤
│ Blok Lengkap      │ 124 B (Header) + 8 B (Tx Count) + Σ(Tx) + CC(B)    │
├───────────────────┼────────────────────────────────────────────────────┤
│ Commit Certificate│ 48 Bytes + (K × 96 Bytes Signatures)               │
└───────────────────┴────────────────────────────────────────────────────┘
```

### 3.1 Header Blok (Tepat 124 Bytes)
Header blok adalah struktur berukuran tetap (*fixed-size*) tanpa overhead panjang dinamis:

```text
Offset  Panjang  Field             Tipe       Keterangan
--------------------------------------------------------------------------------
0x00    4 B      version           u32 (BE)   Protokol versi blok (0x00000001)
0x04    8 B      height            u64 (BE)   Tinggi blok sekuensial (H)
0x0C    8 B      round             u64 (BE)   Nomor putaran konsensus BFT (R)
0x14    8 B      timestamp         u64 (BE)   Unix epoch timestamp (detik)
0x1C    32 B     prev_block_hash   Hash256    Blake3 hash dari header blok H-1
0x3C    32 B     tx_merkle_root    Hash256    Blake3 Merkle root seluruh Tx
0x5C    32 B     state_root        Hash256    Blake3 Sparse Merkle Tree state root
--------------------------------------------------------------------------------
TOTAL: Tepat 124 Bytes
```

### 3.2 Suara Konsensus BFT / Vote (Tepat 117 Bytes)
Pesan pemungutan suara konsensus pada fase Pre-vote dan Pre-commit:

```text
Offset  Panjang  Field             Tipe       Keterangan
--------------------------------------------------------------------------------
0x00    1 B      phase             u8         0x01 = PREVOTE, 0x02 = PRECOMMIT
0x01    8 B      height            u64 (BE)   Tinggi blok (H)
0x09    8 B      round             u64 (BE)   Putaran konsensus (R)
0x11    32 B     block_hash        Hash256    Hash blok target (atau 0x00 untuk NIL)
0x31    4 B      validator_index   u32 (BE)   Indeks validator pada V_E
0x35    64 B     signature         Signature  Ed25519 tanda tangan atas DST + vote
--------------------------------------------------------------------------------
TOTAL: Tepat 117 Bytes
```

### 3.3 Entri Himpunan Validator (Tepat 72 Bytes)
```text
Offset  Panjang  Field             Tipe       Keterangan
--------------------------------------------------------------------------------
0x00    32 B     validator_id      Address    Alamat kanonikal validator
0x20    32 B     consensus_pubkey  [u8; 32]   Kunci publik Ed25519
0x40    8 B      voting_weight     u64 (BE)   Bobot voting integer
--------------------------------------------------------------------------------
TOTAL: Tepat 72 Bytes
```

### 3.4 Transaksi Kanonikal (184 Bytes + Payload Opsional)
```text
Offset       Panjang  Field         Tipe        Keterangan
--------------------------------------------------------------------------------
0x00         2 B      version       u16 (BE)    Format versi transaksi (wajib 1)
0x02         4 B      chain_id      u32 (BE)    Mainnet = 1001 (GENESIS_CHAIN_ID)
0x06         1 B      tx_type       u8          Jenis transaksi (0x01..0x06)
0x07         1 B      flags         u8          Bitmask opsi (default 0x00)
0x08         32 B     sender        Address     Alamat pengirim
0x28         32 B     recipient     Address     Alamat penerima
0x48         8 B      nonce         u64 (BE)    Penghitung sekuensial akun
0x50         16 B     amount        Quantum     u128 nilai transfer
0x60         16 B     fee           Quantum     u128 biaya transaksi
0x70         8 B      valid_until   u64 (BE)    Batas kadaluarsa (0 = tanpa batas)
0x78         4 B      payload_len   u32 (BE)    Panjang N (Maks: 24.576 B)
0x7C         N B      payload       [u8; N]     Raw bytes data/script
0x7C + N     64 B     signature     Signature   Ed25519 tanda tangan
--------------------------------------------------------------------------------
BASIS (tanpa payload): 184 Bytes; TOTAL: 188 + N Bytes
```

> **Preimage Penandatanganan:** Tanda tangan Ed25519 dibuat langsung atas `DST_TX ("AURION-TX-V1") || 0x00 || version || chain_id || tx_type || flags || sender || recipient || nonce || amount || fee || valid_until || payload_len || payload`. `TxID` dihitung terpisah dengan Blake3 derive-key konteks `AURION-TX-ID-V1` atas serialisasi kanonikal penuh (termasuk `signature`).

### 3.5 Sertifikat Komitmen (Commit Certificate)
Menampung bukti kuorum supermayoritas $> 2/3$ tanda tangan validator:
- `block_hash`: `Hash256` (32 B)
- `height`: `u64 (BE)` (8 B)
- `round`: `u64 (BE)` (8 B)
- `precommits_count`: `u32 (BE)` (4 B)
- `precommits`: Array $K$ elemen, masing-masing berupa `Vote` kanonikal:
  - `phase`: `u8` (1 B), wajib `0x02` (PRECOMMIT) di dalam sertifikat
  - `height`: `u64 (BE)` (8 B)
  - `round`: `u64 (BE)` (8 B)
  - `block_hash`: `Hash256` (32 B)
  - `validator_index`: `u32 (BE)` (4 B)
  - `signature`: `Signature` Ed25519 (64 B)
  *(Sub-total per precommit: 117 Bytes)*

Ukuran kanonikal sertifikat adalah `52 + (117 × K)` Bytes. `K` dibatasi
hingga `65.535` oleh decoder sebelum alokasi, dan buffer harus memuat tepat
`117 × K` Bytes yang tersisa. Verifikasi sertifikat juga mensyaratkan seluruh
precommit memiliki `block_hash`, `height`, dan `round` yang sama dengan header
sertifikat, tidak duplikat validator, tanda tangan valid, serta bobot total
lebih besar dari dua pertiga total bobot `V_E`.

---

# BAGIAN 2: PROTOKOL JARINGAN WIRE P2P (NETWORK WIRE PROTOCOL)

## 4. Arsitektur Jaringan dan Siklus Pemrosesan Pesan

Jaringan Aurion beroperasi sebagai jaringan peer-to-peer terdistribusi tanpa perantara terpusat. Setiap simpul memproses paket jaringan melalui pipa keamanan deterministik:

```text
[Jaringan P2P] ──(Koneksi TCP/Multiplexed Wire)
       │
       ▼
 ┌───────────┐      1. Baca 4-Byte Magic (0x41555230)
 │ WIRE IN   │      2. Baca Header Paket (52 Bytes)
 └─────┬─────┘      3. Verifikasi Blake3 Checksum Payload
       │
       ▼
 ┌───────────┐      1. Periksa Batas Ukuran Memori Sesuai MsgType
 │ ALLOC-CHK │      2. Tolak Jika Melebihi Plafon (Anti-OOM)
 └─────┬─────┘
       │
       ▼
 ┌───────────┐      1. Dekode Biner Kanonikal (Zero-Trailing Check)
 │  DECODE   │      2. Validasi Format & Struktur Internal
 └─────┬─────┘
       │
       ▼
 ┌───────────┐      1. Validasi Kriptografis (Tanda Tangan & Hash)
 │ VALIDATE  │      2. Evaluasi Konsensus / Aturan Mempool
 └─────┬─────┘
       │
       ▼
 ┌───────────┐      1. Eksekusi STF (Jika Blok)
 │  PROCESS  │      2. Masukkan Mempool (Jika Tx)
 └─────┬─────┘      3. Relay / Gossip ke Peer Tetangga yang Memenuhi Syarat
```

---

## 5. Struktur Frame Wire Protokol (52-Byte Wire Frame Header)

Setiap pesan yang ditransmisikan melalui wire jaringan wajib dibungkus oleh **Wire Frame Header** standar berukuran tetap 52 bytes:

```text
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                       Magic: 0x41555230 ("AUR0")              |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|       Message Type ID (u16)   |          Reserved (0x0000)    |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                     Payload Length (u32, BE)                  |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|               Reserved2 (8 Bytes, Wajib 0x00...)              |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                                                               |
+                                                               +
|                   Blake3 Checksum (32 Bytes)                  |
+                     Payload Integrity Digest                  +
|                                                               |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                      Payload (Length Bytes...)                |
```

### 5.1 Rincian Kolom Wire Header
1. **Magic Bytes (4 B):** `0x41555230` (ASCII: `"AUR0"`). Simpul memutuskan koneksi seketika jika 4 byte pertama tidak cocok.
2. **Message Type ID (2 B):** Identifier numerik Big-Endian untuk membedakan rute pesan. Nilai yang tidak ada di katalog Bagian 6 **WAJIB DITOLAK** (`WireError::UnknownMessageType`); tidak ada rute fallback implisit.
3. **Reserved (2 B):** Bit cadangan untuk perluasan protokol masa depan. Wajib bernilai `0x0000`; nilai lain ditolak (`WireError::NonZeroReserved`).
4. **Payload Length (4 B):** Panjang payload biner dalam bytes. Wajib $\le$ plafon khusus `message_type` (Bagian 6).
5. **Reserved2 (8 B):** Bit cadangan lapis kedua. Wajib seluruhnya `0x00`; jika tidak, ditolak (`WireError::NonZeroReserved2`).
6. **Blake3 Checksum (32 B):** Hash Blake3 langsung dari payload:
   $$\text{Checksum} = \text{Blake3}(\text{Payload})$$
   Jika checksum tidak cocok, payload dibuang seketika tanpa didekode.

### 5.2 Validasi Ketat Parser Frame (Strict Wire Validation)
`parse_network_frame()` menolak frame **sebelum** payload dialokasikan/disalin bila salah satu kondisi berikut terpenuhi:
1. Magic bytes $\ne$ `0x41555230` $\to$ `WireError::InvalidMagic`.
2. `Message Type ID` tidak ada di katalog Bagian 6 $\to$ `WireError::UnknownMessageType`.
3. `Reserved != 0x0000` $\to$ `WireError::NonZeroReserved`.
4. `Reserved2` mengandung byte non-zero $\to$ `WireError::NonZeroReserved2`.
5. `Payload Length` melebihi plafon khusus tipe pesan $\to$ `WireError::PayloadTooLarge`. Untuk `TX_GOSSIP` plafonnya adalah `MAX_TX_WIRE_SIZE` $= 24.764\ \text{B}$.
6. Panjang byte frame tidak konsisten dengan `Payload Length` $\to$ `CodecError::UnexpectedEof`.
7. Checksum Blake3 tidak cocok $\to$ `WireError::ChecksumMismatch`.

`serialize_network_frame()` menegakkan aturan (2) dan (5) yang sama, sehingga frame di luar katalog tidak dapat diproduksi. Plafon global $8\ \text{MiB}$ tetap dipertahankan sebagai *backstop* keras di atas plafon per-tipe.

---

## 6. Katalog Tipe Pesan Resmi (Message Types Catalog)

| ID Tipe Pesan | Nama Pesan | Plafon Ukuran Payload | Keterangan Fungsi |
| :---: | :--- | ---:| :--- |
| `0x0001` | `HANDSHAKE_HELLO` | $512\ \text{B}$ | Inisiasi jabat tangan & pertukaran versi |
| `0x0002` | `HANDSHAKE_ACK` | $512\ \text{B}$ | Konfirmasi koneksi & status sinkronisasi |
| `0x0003` | `PING` | $8\ \text{B}$ | Heartbeat & pengukuran latensi jaringan |
| `0x0004` | `PONG` | $8\ \text{B}$ | Jawaban heartbeat |
| `0x0010` | `GET_PEERS` | $0\ \text{B}$ | Permintaan daftar alamat peer aktif |
| `0x0011` | `PEERS_ADDR` | $64\ \text{KB}$ | Daftar IP:Port peer tetangga (PEX) |
| `0x0020` | `BFT_PROPOSAL` | $4\ \text{MB}$ | Siaran proposal blok oleh Proposer |
| `0x0021` | `BFT_PREVOTE` | $128\ \text{B}$ | Siaran suara fase 1 konsensus |
| `0x0022` | `BFT_PRECOMMIT` | $128\ \text{B}$ | Siaran suara fase 2 konsensus |
| `0x0023` | `BFT_COMMIT_CERT`| $256\ \text{KB}$ | Siaran sertifikat komitmen final blok |
| `0x0030` | `TX_GOSSIP` | $24.764\ \text{B}$ | Propagasi transaksi tunggal mempool (184 B basis + 4 B prefiks + 24.576 B payload) |
| `0x0031` | `MEMPOOL_INV` | $1\ \text{MB}$ | Daftar hash transaksi yang dimiliki |
| `0x0040` | `SYNC_GET_HEADERS`| $128\ \text{B}$ | Permintaan rentang header untuk sinkronisasi |
| `0x0041` | `SYNC_HEADERS` | $256\ \text{KB}$ | Kumpulan hingga 2.000 header blok |
| `0x0042` | `SYNC_GET_BLOCK` | $32\ \text{B}$ | Permintaan data lengkap sebuah blok |
| `0x0043` | `SYNC_BLOCK` | $4\ \text{MB}$ | Pengiriman data blok penuh beserta CC(B) |

---

## 7. Handshake, Autentikasi Node, dan Pencegahan Pemisahan Genesis

Setiap koneksi antar-simpul wajib menuntaskan protokol **Mutual Handshake** sebelum diizinkan bertukar data konsensus atau transaksi:

```text
Node A (Inisiator)                             Node B (Penerima)
       │                                              │
       ├────────────── HANDSHAKE_HELLO ──────────────►│
       │   - Protocol Version: 1                      │ (Verifikasi Genesis & Drift)
        │   - Chain ID: 1001                           │
       │   - Genesis Hash: 0xABCD...                  │
       │   - Best Height: 1250                        │
       │   - Timestamp: 1773532800                    │
       │   - Node ID (pk_A) & Challenge Sig           │
       │                                              │
       │◄────────────── HANDSHAKE_ACK ────────────────┤
       │   - Status: SUCCESS                          │
       │   - Best Height: 1300                        │
       │   - Node ID (pk_B) & Challenge Sig           │
       │                                              │
[Koneksi Terotentikasi & Masuk Mesh P2P]
```

### Syarat Pemutusan Hubungan Seketika pada Handshake:
1. **Ketidakcocokan Genesis Hash:** Jika `GenesisHash` peer tidak identik dengan genesis lokal, peer berasal dari jaringan berbeda $\to$ **Putuskan Langsung**.
2. **Ketidakcocokan Versi Protokol:** Jika versi protokol tidak didukung $\to$ **Putuskan Langsung**.
3. **Perbedaan Jam Melampaui Batas (Clock Drift):** Jika $|\text{Timestamp}_{\text{peer}} - \text{Timestamp}_{\text{local}}| > 15\ \text{detik}$ $\to$ **Putuskan Langsung**.

---

## 8. Mekanisme Propagasi Jaringan (Propagation Pipelines)

### 8.1 Propagasi Suara Konsensus BFT (Zero-Delay Mesh Gossip)
Pesan konsensus (`BFT_PROPOSAL`, `BFT_PREVOTE`, `BFT_PRECOMMIT`) memiliki prioritas jaringan tertinggi:
- **Jalur Cepat (*Fast-Path Forwarding*):** Begitu format dan tanda tangan suara dinyatakan sah, simpul langsung memancarkan suara tersebut ke seluruh peer yang terhubung tanpa menunggu verifikasi state lengkap.
- **Deduplikasi Pesan (*Seen-Message Filter*):** Setiap simpul memelihara *Bloom Filter / LRU Cache* berisi hash pesan konsensus untuk mencegah pengiriman pesan yang sama berulang kali.

### 8.2 Propagasi Transaksi (Inventory Announcement)
Transaksi mempool disebarkan menggunakan mekanisme hemat bandwidth:
1. Simpul menyiarkan paket `MEMPOOL_INV` (berisi hash `TxID`).
2. Peer yang belum memiliki transaksi tersebut merespons dengan `GET_DATA(TxID)`.
3. Simpul mengirimkan transaksi penuh via `TX_GOSSIP`.

---

## 9. Sistem Skor Reputasi Peer dan Ketahanan Serangan (Peer Scoring & Anti-DoS)

Setiap simpul secara independen memantau perilaku peer tetangga menggunakan matriks penalti skor integer deterministik (Skor awal peer = $100$):

```text
┌────────────────────────────────────────────────────────────────────────┐
│                   MATRIKS PENALTI SKOR PEER (PEER SCORING)             │
├───────────────────────────────────────────────────────┬────────────────┤
│ JENIS PELANGGARAN PEER                                │ PENALTI SKOR   │
├───────────────────────────────────────────────────────┼────────────────┤
│ Magic bytes salah / tidak dikenal                     │ -100 (Ban 24h) │
│ Checksum Blake3 tidak cocok                           │ -50            │
│ Mengirim transaksi dengan tanda tangan palsu          │ -40            │
│ Mengirim transaksi invalid / double-spend             │ -20            │
│ Mengirim bukti ekuivokasi palsu                       │ -100 (Ban 24h) │
│ Melakukan spamming pesan di atas batas laju (*rate*)  │ -30            │
│ Payload melebihi plafon maksimum memori               │ -100 (Ban 24h) │
│ Mengirim proposal blok yang tidak valid               │ -80            │
└───────────────────────────────────────────────────────┴────────────────┘
```

### 9.1 Tindakan Berdasarkan Ambang Batas Skor:
- $\text{Skor} \le 50$: Peer dimasukkan ke mode pembatasan ketat (*Traffic Throttling*).
- $\text{Skor} \le 0$: Koneksi TCP diputus secara sepihak dan alamat IP peer diblokir (*Banned*) selama $86.400\ \text{detik}$ ($24\ \text{jam}$).

### 9.2 Ketahanan Serangan DoS / Alokasi Memori Aman:
1. **Pre-Allocation Cap:** Dilarang mengalokasikan memori berdasarkan nilai field `payload_len` sebelum byte data benar-benar tiba di socket buffer.
2. **Buffer Streaming Parsing:** Parser membaca frame secara streaming per-bagian kecil untuk mencegah serangan alokasi memori kosong (*zero-byte allocation attack*).
3. **Batas Laju Pesan (*Token Bucket Rate Limiter*):** Setiap koneksi peer dibatasi maksimum menerima $200\ \text{pesan/detik}$ untuk transaksi dan $50\ \text{pesan/detik}$ untuk blok sync. Pesan berlebih dibuang langsung pada tingkat socket.
