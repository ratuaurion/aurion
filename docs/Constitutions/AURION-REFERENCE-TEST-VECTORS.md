# 11 — AURION REFERENCE TEST VECTORS SPECIFICATION
## Vektor Uji Referensi Kanonikal Lintas Implementasi (Rust, Go, Python, C++)

> **Hierarki Dokumen:**  
> `00 — AURION CONSTITUTION` $\longrightarrow$ `01 — MONETARY` $\longrightarrow$ `02 — CONSENSUS` $\longrightarrow$ `03 — STATE TRANSITION` $\longrightarrow$ `04 — TRANSACTION` $\longrightarrow$ `05 — CRYPTOGRAPHY` $\longrightarrow$ `06 — SERIALIZATION & WIRE` $\longrightarrow$ `07 — GENESIS` $\longrightarrow$ `08 — VALIDATOR & STAKING` $\longrightarrow$ `09 — GOVERNANCE` $\longrightarrow$ `10 — SECURITY` $\longrightarrow$ **`11 — REFERENCE TEST VECTORS`**  
>
> **Status:** AUDITED & LOCKED SPECIFICATION  
> **Klasifikasi:** Standar Verifikasi & Validasi Uji Mesin Konsensus (Test Suite Anchor)  
> **Versi:** 1.0.0-PROD  
> **Sifat Ketetapan:** Deterministik Mutlak, Titik Acuan Emas (*Golden Test Vectors*)

---

## 1. Tujuan dan Protokol Verifikasi Uji

Dokumen ini menyediakan **Vektor Uji Emas (*Golden Test Vectors*)** independen bahasa untuk memastikan bahwa seluruh implementasi perangkat lunak simpul Aurion (baik dalam Rust, Go, Python, C++, atau bahasa lainnya) menghasilkan:
- Byte serialisasi yang **100% identik**;
- Nilai hash Blake3 yang **100% identik**;
- Alamat Bech32m dan tanda tangan Ed25519 yang **100% identik**;
- Hasil kalkulasi fungsi transisi state ($\text{STF}$) yang **100% identik**.

Setiap implementasi simpul **WAJIB (MUST)** lulus seluruh rangkaian pengujian pada dokumen ini sebelum diizinkan terhubung ke jaringan uji (*Testnet*) maupun jaringan utama (*Mainnet*).

---

## 2. Vektor Uji 1: Fungsi Hash BLAKE3 (256-Bit)

### 2.1 Test Vector 1.1: Hashing String Kosong (Empty Input)
- **Input (0 Bytes):** `""` (Empty string)
- **Expected Blake3 Digest (32 Bytes Hex):**
  ```text
  af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262
  ```

### 2.2 Test Vector 1.2: Hashing Kata Kunci Standar
- **Input ASCII:** `"aurion"` (6 Bytes ASCII: `0x617572696f6e`)
- **Expected Blake3 Digest (32 Bytes Hex):**
  ```text
  c81453c888cdd581753d4b72f466a585e98ffda6f68dbd2fabc5f43d5fee86c8
  ```

### 2.3 Test Vector 1.3: Mode Derivasi Kunci (Key Derivation Mode)
- **Context String (ASCII):** `"AURION-TEST-V1"`
- **Key Material (32 Bytes Hex):**
  `000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f`
- **Expected Blake3 DeriveKey Digest (32 Bytes Hex):**
  ```text
  d08662334d813cc3e872910df43129dd5dbf7ba49215352184def9f82ec5e195
  ```

---

## 3. Vektor Uji 2: Pembangkitan Kunci Ed25519 dan Tanda Tangan

### 3.1 Test Vector 2.1: Keypair Derivation
- **Private Seed (32 Bytes Hex):**
  `9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60`
- **Expected Compressed Public Key (32 Bytes Hex):**
  ```text
  d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a
  ```

### 3.2 Test Vector 2.2: Penandatanganan Pesan (Ed25519 Sign & Verify)
- **Message Payload ASCII:** `"AURION-CONSENSUS-TEST-MESSAGE"`
- **Expected Canonical Signature (64 Bytes Hex):**
  ```text
  cb8dba50a61f5269904f7ea575769cfb4d6916af4ef4c984edeca8fbe36194f500b8373535e9806ad93d04921365f9820d8561ef050f2ff6d8e35f3da3452209
  ```
- **Strict Verification:** `Ed25519_VerifyStrict(PublicKey, Message, Signature) == TRUE`.

---

## 4. Vektor Uji 3: Derivasi Alamat Kanonikal & Bech32m

### 4.1 Test Vector 3.1: Alamat Raw dan Alamat Bech32m Mainnet
- **Input Public Key (32 Bytes Hex):**
  `d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a`
- **Context String DST:** `"AURION-ADDRESS-V1"`
- **Expected Raw Address (32 Bytes Hex via Blake3 DeriveKey):**
  ```text
  7acb7a9e77ef27c92b8049ec96ea40901febbf3861e97a97f01e7a02f0170253
  ```
- **Expected Mainnet Display Address (Bech32m, Prefix: `aur1`):**
  ```text
  aur10t9h48nhaunuj2uqf8kfd6jqjq07h0ecv85h49lsreaq9uqhqffshf0p6h
  ```
- **Expected Testnet Display Address (Bech32m, Prefix: `aurt1`):**
  ```text
  aurt10t9h48nhaunuj2uqf8kfd6jqjq07h0ecv85h49lsreaq9uqhqffsjazk8c
  ```

---

## 5. Vektor Uji 4: Serialisasi Transaksi Kanonikal dan TxID

### 5.1 Parameter Transaksi Uji
```text
version      : 1 (0x0001)
chain_id     : 1001 (0x000003e9)
tx_type      : 0x01 (Transfer)
flags        : 0x00
nonce        : 0 (0x0000000000000000)
sender       : cb095697ccc5acbf23e176ee4f05e7e77ddfe54236bfe8c8b42e78c3d5a95aad
recipient    : 0202020202020202020202020202020202020202020202020202020202020202
amount       : 500.000.000 Q (5 AUR) -> 0x0000000000000000000000001dcd6500
fee          : 10.000.000 Q (0.1 AUR) -> 0x00000000000000000000000000989680
valid_until  : 1000 (0x00000000000003e8)
payload_len  : 0 (0x00000000)
payload      : [] (Empty)
ed25519_seed : 0101010101010101010101010101010101010101010101010101010101010101
```

### 5.2 Preimage Penandatanganan Transaksi (Signing Preimage)
- **Prefiks Domain (DST):** `"AURION-TX-V1"` (12 Bytes ASCII: `415552494f4e2d54582d5631`) diikuti pemisah `0x00`.
- **Total Panjang Preimage:** 12 + 1 + 124 = 137 Bytes.
- **Signing Preimage (137 Bytes Hex):**
  ```text
  415552494f4e2d54582d5631000001000003e90100cb095697ccc5acbf23e176ee4f05e7e77ddfe54236bfe8c8b42e78c3d5a95aad020202020202020202020202020202020202020202020202020202020202020200000000000000000000000000000000000000001dcd65000000000000000000000000000098968000000000000003e800000000
  ```
- **Expected Preimage Hash (Blake3 Hex):**
  ```text
  807fa6f843afc7e23cbc927aaf2d86556d05ca9690a304d8e2c1a293430fb7ff
  ```
- **Tanda Tangan Dihasilkan (Ed25519 Signature, 64 Bytes Hex):**
  ```text
  77506b9564af9209c9f67b12874cb13b760cc19137143f211b9f3c3d127b0b889a20edd754de317da90631a755f41c8dc83fe3a27b415ed0397b73a4d87f0c09
  ```

### 5.3 Serialisasi Lengkap Transaksi (188 Bytes Hex)
```text
0001000003e90100cb095697ccc5acbf23e176ee4f05e7e77ddfe54236bfe8c8b42e78c3d5a95aad020202020202020202020202020202020202020202020202020202020202020200000000000000000000000000000000000000001dcd65000000000000000000000000000098968000000000000003e80000000077506b9564af9209c9f67b12874cb13b760cc19137143f211b9f3c3d127b0b889a20edd754de317da90631a755f41c8dc83fe3a27b415ed0397b73a4d87f0c09
```

> **Catatan:** Basis transaksi tanpa payload adalah 184 Bytes (`TRANSACTION_BASE_BYTES`); vektor 188 Bytes di atas menyertakan prefiks `payload_len` 4 Bytes untuk payload kosong.

### 5.4 Expected Transaction Identifier (TxID)
- **Rumus:** $\text{TxID} = \text{Blake3DeriveKey}(\texttt{"AURION-TX-ID-V1"},\; \text{EncodeCanonical}(T_x))$
- **Expected TxID (32 Bytes Hex):**
  ```text
  9da0583445cf37016058bb104b83e80626e3bcf6c92629eada1ea7fcc8e6e2f9
  ```

---

## 6. Vektor Uji 5: Header Blok dan BlockHash

### 6.1 Parameter Header Blok 1
```text
version         : 1 (0x00000001)
height          : 1 (0x0000000000000001)
round           : 0 (0x0000000000000000)
timestamp       : 1773570060 (0x0000000069b6880c)
prev_block_hash : [0u8; 32] (Genesis Hash Blok 0)
tx_merkle_root  : 9da0583445cf37016058bb104b83e80626e3bcf6c92629eada1ea7fcc8e6e2f9 (TxID di atas)
state_root      : af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262 (Blake3 empty)
```

### 6.2 Serialisasi Biner Header (Tepat 124 Bytes Hex)
```text
00000001000000000000000100000000000000000000000069b6880c00000000000000000000000000000000000000000000000000000000000000009da0583445cf37016058bb104b83e80626e3bcf6c92629eada1ea7fcc8e6e2f9af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262
```

### 6.3 Expected BlockHash
- **Rumus:** $\text{BlockHash} = \text{Blake3DeriveKey}(\texttt{"AURION-BLOCK-ID-V1"},\; \text{HeaderBytes})$
- **Expected BlockHash (32 Bytes Hex):**
  ```text
  eed74fb70c5cf061b51688acc25f7f08d3445d08e4b40a0c24eccefa56024701
  ```

---

## 7. Vektor Uji 6: Header Blok Nol (Genesis Block) dan GenesisHash

### 7.1 Parameter Header Blok 0 (Mainnet)
```text
version         : 1 (0x00000001)
height          : 0 (0x0000000000000000)
round           : 0 (0x0000000000000000)
timestamp       : 1773532800 (0x0000000069b5f680)
prev_block_hash : [0u8; 32] (32 Zero Bytes)
tx_merkle_root  : [0u8; 32] (32 Zero Bytes)
state_root      : 61e647706990a010ba95f781d506620cf69b2dc57a7b1b53ca96f0f1d07bb850 (StateRoot_0, SMT Root 2 Vault Terbit Awal)
```

### 7.2 Serialisasi Biner Header Blok 0 (Tepat 124 Bytes Hex)
```text
00000001000000000000000000000000000000000000000069b5f6800000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000061e647706990a010ba95f781d506620cf69b2dc57a7b1b53ca96f0f1d07bb850
```

### 7.3 Expected GenesisHash Resmi Mainnet
- **Rumus:** $\text{GenesisHash} = \text{Blake3DeriveKey}(\texttt{"AURION-BLOCK-ID-V1"},\; \text{HeaderBytes}_0)$
- **Expected GenesisHash (32 Bytes Hex):**
  ```text
  d82f72ac1be185911bd803987660e624c0ed1c12d4a189b147de9c5b7f5635f9
  ```

---

## 8. Vektor Uji 7: Konsensus Suara BFT (Canonical Vote)

### 8.1 Parameter Suara Pre-commit
```text
phase           : 0x02 (PRECOMMIT)
height          : 1 (0x0000000000000001)
round           : 0 (0x0000000000000000)
block_hash      : 9b456209c11874296df4425b42d1396a58231d8e1329a43587b1c42e54308821
validator_index : 0 (0x00000000)
signature       : 64 Bytes Signature valid
```
- **Total Panjang Biner Vote:** Tepat **$117\ \text{Bytes}$**.

### 8.2 Vektor Sertifikat Komitmen (Satu Precommit)
Vektor struktural berikut mengunci urutan field `CommitCertificate` dan ukuran
elemen vote:
```text
block_hash      : 11 repeated 32 times
height          : 7 (0x0000000000000007)
round           : 3 (0x0000000000000003)
precommits_count: 1 (0x00000001)
vote.phase      : 0x02 (PRECOMMIT)
vote.height     : 7
vote.round      : 3
vote.block_hash : 11 repeated 32 times
vote.validator_index: 2 (0x00000002)
vote.signature  : 22 repeated 64 times
```
- **Ukuran tetap:** `52 + (117 × 1) = 169 Bytes`.
- Decoder wajib menolak `precommits_count > 65.535` sebelum alokasi dan wajib
  memastikan tersedia `117 × precommits_count` Bytes untuk seluruh vote.
- Verifier wajib memeriksa signature, duplikasi validator, kesamaan
  `block_hash`/`height`/`round`, serta quorum `> 2/3` berdasarkan bobot `V_E`.

---

## 9. Vektor Uji 8: Eksekusi Fungsi Transisi State (STF Execution)

### 9.1 Skenario Input Transisi
- **State Awal ($\sigma$):**
  - Akun A: $\text{Balance} = 1.000.000.000\ Q$ ($1\ \text{AUR}$ pada skala $10^9$ Quantum), $\text{Nonce} = 0$.
  - Akun B: $\text{Balance} = 0\ Q$, $\text{Nonce} = 0$.
- **Transaksi $T_x$:**
  - Pengirim: Akun A
  - Penerima: Akun B
  - Nilai Transfer: $250.000.000\ Q$ ($0,25\ \text{AUR}$)
  - Biaya Transaksi: $10.000\ Q$ ($0,00001\ \text{AUR}$)

### 9.2 Perhitungan Deterministik Output ($\sigma'$)
1. **Total Debet Akun A:** $250.000.000 + 10.000 = 250.010.000\ Q$.
2. **Saldo Akhir Akun A:** $1.000.000.000 - 250.010.000 = \mathbf{749.990.000\ Q}\ (0,749990000\ \text{AUR})$.
3. **Nonce Baru Akun A:** $0 + 1 = \mathbf{1}$.
4. **Saldo Akhir Akun B:** $0 + 250.000.000 = \mathbf{250.000.000\ Q}\ (0,250000000\ \text{AUR})$.
5. **Alokasi Biaya Transaksi (100% Validator, 0% Burn):**
   - $\mathcal{F}_{\text{validator}} = \mathbf{10.000\ Q}$ (100% dialokasikan langsung kepada validator pembuat blok).
   - $\mathcal{F}_{\text{burned}} = \mathbf{0\ Q}$ (Skema usang pembakaran fee telah dihapuskan).
6. **Verifikasi Konservasi Nilai:**
   $$749.990.000 + 250.000.000 + 10.000 = 1.000.000.000\ Q\ (\text{Presisi Sempurna 100\%})$$

---

## 10. Vektor Uji 9: Format Frame Jaringan Wire (Wire Frame Codec)

### 10.1 Parameter Frame Pesan `BFT_PREVOTE`
```text
Magic        : 0x41555230 ("AUR0")
Message Type : 0x0021 (BFT_PREVOTE)
Reserved     : 0x0000
Payload Len  : 117 (0x00000075)
Payload      : 117 Bytes Canonical Vote
Blake3 Check : 32 Bytes Blake3 Hash atas Payload 117 Bytes
```
- **Total Panjang Frame Wire:** $52\ \text{B (Header)} + 117\ \text{B (Payload)} = \mathbf{169\ \text{Bytes}}$.
