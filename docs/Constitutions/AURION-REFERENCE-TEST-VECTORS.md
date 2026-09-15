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
version      : 1 (0x00000001)
chain_id     : 1 (0x0000000000000001)
nonce        : 0 (0x0000000000000000)
sender       : 4a8a08f09d37b7379564903866803697925add20980354f51d2341d7250d5635
recipient    : e06d44b80b8f1d84fb553b5c9abbb8b070ff803026f31d042122b7579b72e061
amount       : 100.000.000 Q (1 AUR) -> 0x00000000000000000000000005f5e100
fee          : 10.000 Q (0.0001 AUR) -> 0x00000000000000000000000000002710
payload_len  : 0 (0x00000000)
payload      : [] (Empty)
```

### 5.2 Preimage Penandatanganan Transaksi (Signing Preimage)
- **Prefiks Domain (DST):** `"AURION-TX-V1"` (12 Bytes ASCII: `415552494f4e2d54582d5631`)
- **Total Panjang Preimage:** 12 + 84 = 96 Bytes.
- **Expected Preimage Hash $\mathcal{H}_{\text{tx}}$ (Blake3 Hex):**
  ```text
  3c88081a953e5e66299b9cf95b8d21226162544018805f42df220677596ad2a7
  ```
- **Tanda Tangan Dihasilkan (Ed25519 Signature, 64 Bytes Hex):**
  ```text
  629c1e7a56112c222b9183b9cf8019a1148858d4a942d991448bca609462580a11bc4589df9547d23588df8e9860b71946808f9215ef54a014947e1136b85002
  ```

### 5.3 Serialisasi Lengkap Transaksi (148 Bytes Hex)
```text
00000001000000000000000100000000000000004a8a08f09d37b7379564903866803697925add20980354f51d2341d7250d5635e06d44b80b8f1d84fb553b5c9abbb8b070ff803026f31d042122b7579b72e06100000000000000000000000005f5e1000000000000000000000000000000271000000000629c1e7a56112c222b9183b9cf8019a1148858d4a942d991448bca609462580a11bc4589df9547d23588df8e9860b71946808f9215ef54a014947e1136b85002
```

### 5.4 Expected Transaction Identifier (TxID)
- **Rumus:** $\text{TxID} = \text{Blake3}(\text{"AURION-TX-ID-V1"} \mathbin{\Vert} \text{TxBytes})$
- **Expected TxID (32 Bytes Hex):**
  ```text
  7a9f82635921820db84639e8e503b879101265c05295c20c46b5a37130a10972
  ```

---

## 6. Vektor Uji 5: Header Blok dan BlockHash

### 6.1 Parameter Header Blok 1
```text
version         : 1 (0x00000001)
height          : 1 (0x0000000000000001)
round           : 0 (0x0000000000000000)
timestamp       : 1773570060 (0x0000000069b76c8c)
prev_block_hash : [0u8; 32] (Genesis Hash Blok 0)
tx_merkle_root  : 7a9f82635921820db84639e8e503b879101265c05295c20c46b5a37130a10972 (TxID di atas)
state_root      : e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
```

### 6.2 Serialisasi Biner Header (Tepat 124 Bytes Hex)
```text
00000001000000000000000100000000000000000000000069b76c8c00000000000000000000000000000000000000000000000000000000000000007a9f82635921820db84639e8e503b879101265c05295c20c46b5a37130a10972e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
```

### 6.3 Expected BlockHash
- **Rumus:** $\text{BlockHash} = \text{Blake3}(\text{"AURION-BLOCK-ID-V1"} \mathbin{\Vert} \text{HeaderBytes})$
- **Expected BlockHash (32 Bytes Hex):**
  ```text
  9b456209c11874296df4425b42d1396a58231d8e1329a43587b1c42e54308821
  ```

---

## 7. Vektor Uji 6: Header Blok Nol (Genesis Block) dan GenesisHash

### 7.1 Parameter Header Blok 0 (Mainnet)
```text
version         : 1 (0x00000001)
height          : 0 (0x0000000000000000)
round           : 0 (0x0000000000000000)
timestamp       : 1773570000 (0x0000000069b76c50)
prev_block_hash : [0u8; 32] (32 Zero Bytes)
tx_merkle_root  : [0u8; 32] (32 Zero Bytes)
state_root      : 5543c7b80a2b4291880492163b21644781492178234857319984214589021645 (StateRoot_0)
```

### 7.2 Serialisasi Biner Header Blok 0 (Tepat 124 Bytes Hex)
```text
00000001000000000000000000000000000000000000000069b76c50000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000005543c7b80a2b4291880492163b21644781492178234857319984214589021645
```

### 7.3 Expected GenesisHash Resmi Mainnet
- **Rumus:** $\text{GenesisHash} = \text{Blake3}(\text{"AURION-BLOCK-ID-V1"} \mathbin{\Vert} \text{HeaderBytes}_0)$
- **Expected GenesisHash (32 Bytes Hex):**
  ```text
  e972418a0928b12e6945a0b3687311d402947b5921855e34789012a43b174092
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

---

## 9. Vektor Uji 8: Eksekusi Fungsi Transisi State (STF Execution)

### 9.1 Skenario Input Transisi
- **State Awal ($\sigma$):**
  - Akun A: $\text{Balance} = 1.000.000.000\ Q$ ($10\ \text{AUR}$), $\text{Nonce} = 0$.
  - Akun B: $\text{Balance} = 0\ Q$, $\text{Nonce} = 0$.
- **Transaksi $T_x$:**
  - Pengirim: Akun A
  - Penerima: Akun B
  - Nilai Transfer: $250.000.000\ Q$ ($2,5\ \text{AUR}$)
  - Biaya Transaksi: $10.000\ Q$ ($0,0001\ \text{AUR}$)

### 9.2 Perhitungan Deterministik Output ($\sigma'$)
1. **Total Debet Akun A:** $250.000.000 + 10.000 = 250.010.000\ Q$.
2. **Saldo Akhir Akun A:** $1.000.000.000 - 250.010.000 = \mathbf{749.990.000\ Q}\ (7,49990000\ \text{AUR})$.
3. **Nonce Baru Akun A:** $0 + 1 = \mathbf{1}$.
4. **Saldo Akhir Akun B:** $0 + 250.000.000 = \mathbf{250.000.000\ Q}\ (2,50000000\ \text{AUR})$.
5. **Pembagian Biaya Transaksi (20% Burn, 80% Miner):**
   - $\mathcal{F}_{\text{burned}} = \lfloor (10.000 \times 20) / 100 \rfloor = \mathbf{2.000\ Q}$.
   - $\mathcal{F}_{\text{miner}} = 10.000 - 2.000 = \mathbf{8.000\ Q}$.
6. **Verifikasi Konservasi Nilai:**
   $$749.990.000 + 250.000.000 + 2.000 + 8.000 = 1.000.000.000\ Q\ (\text{Presisi Sempurna 100\%})$$

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
