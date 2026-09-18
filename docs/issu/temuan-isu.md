# Temuan Isu Protokol Aurion

Dokumen ini mencatat temuan masalah pada kode, dokumentasi, artefak genesis,
wallet, dan protokol wire Aurion. Fokus dokumen adalah isu teknis dan keamanan;
temuan guardrail administratif tidak dibahas di sini.

Setiap temuan pada dokumen ini telah diverifikasi ulang terhadap isi source code,
dokumen konstitusi, dan artefak pada commit terakhir `dd34f7c` (09-2026-09-19).
Kolom **verifikasi** menjelaskan bukti lokasi dan status akurasi temuan.

## 1. Status Audit

Audit membandingkan source code, dokumen konstitusi, artefak genesis, wallet,
transport, dan test library Aurion.

Hasil pengujian library (verifikasi `cargo test --lib` pada commit `dd34f7c`):

```text
150 passed
0 failed
```

Test tersebut hanya membuktikan test yang tersedia. Ia belum membuktikan
konsistensi lintas genesis, wallet, validator, mobile, bootnode, dan dokumentasi.

Guardrail arsitektur (`python tools/guardrail.py`) tetap 100% PASS: zero unsafe
code, zero floating-point, 38/38 dokumen spesifikasi hadir.

## 2. Ringkasan Isu

| ID | Isu | Prioritas | Status | Verifikasi |
|---|---|---|---|---|
| AUR-ISSUE-001 | Genesis key deterministic tertanam di source | Critical | Open | ✅ Terverifikasi (terbatas pada validator) |
| AUR-ISSUE-002 | Chain ID berbeda antar komponen | Critical | Open | ✅ Terverifikasi |
| AUR-ISSUE-003 | Format transaksi kode berbeda dari spesifikasi | Critical | Open | ✅ Terverifikasi |
| AUR-ISSUE-004 | Keystore menggunakan kriptografi custom berisiko | Critical | Open | ✅ Terverifikasi |
| AUR-ISSUE-005 | Password default wallet lemah | High | Closed | ✅ Terverifikasi & sudah diremediasi |
| AUR-ISSUE-006 | Address Creator/Developer berupa placeholder | High | Open | ✅ Terverifikasi (dokumen) |
| AUR-ISSUE-007 | Ukuran transaksi tidak konsisten | High | Open | ✅ Terverifikasi |
| AUR-ISSUE-008 | Format CommitCertificate berbeda dari dokumentasi | High | Open | ✅ Terverifikasi |
| AUR-ISSUE-009 | Validasi frame belum menegakkan semua batas | High | Open | ✅ Terverifikasi |
| AUR-ISSUE-010 | Genesis artifact belum diverifikasi terhadap binary aktif | High | Open | ✅ Terverifikasi |

## 3. Temuan Detail

### AUR-ISSUE-001: Genesis Key Deterministic di Source

**Prioritas:** Critical
**Status:** Open
**Lokasi:** `src/primitives/genesis/ceremony.rs:153-160` dan `src/platform/cli/dispatcher.rs:876-881`

`CanonicalCeremonyKeypairs::new_deterministic()` membuat keypair Creator,
Developer, dan empat validator menggunakan seed tetap seperti `[0x01; 32]`,
`[0x02; 32]`, dan `[0x11; 32]` sampai `[0x14; 32]`.

**Koreksi verifikasi (kondisi per commit `dd34f7c`):**

- `GENESIS_CEREMONY.json` terbaru **tidak lagi** memakai key deterministic
  untuk Creator dan Developer. Ceremony telah dijalankan ulang dengan wallet
  keypair nyata pada commit `d3d54a4`:
  - Creator: `aur1jjtqrlqy9suehhltnzt2ml4zwsr8ukpyvvhm2gw899u0e0w22qusq0pjql`
  - Developer: `aur1eaj265jvs5wzgdyr9d9p2elkgckx07r0gqc9kejwcznyplw2zlqqlxdu7y`
  - Genesis Block H=0: `d82f72ac1be185911bd803987660e624c0ed1c12d4a189b147de9c5b7f5635f9`
- Namun `new_deterministic()` masih dipakai sebagai **default** pada jalur
  `aurion validator start --index <0..3>` (`dispatcher.rs:876-881`): key signing
  validator 1-4 diambil dari seed `[0x11;32]`–`[0x14;32]`. Jalur ini tidak
  menawarkan opsi keystore validator untuk mengganti key signing saat startup.

Dampak sisa yang valid:

- siapa pun yang membaca source dapat merekonstruksi private key validator
  genesis dan mengendalikan identity validator;
- `aurion genesis ceremony run` tanpa opsi `--creator-keystore` /
  `--developer-keystore` kembali menghasilkan transcript dengan key deterministic.

**Tindakan wajib:**

1. Hentikan penggunaan key deterministic untuk production (khusus validator).
2. Tambahkan dukungan keystore untuk key signing validator 1-4 pada
   `aurion validator start`.
3. Perlakukan key deterministic lama sebagai compromised untuk seluruh peran.
4. Pisahkan custody Creator, Developer, dan validator.
5. Jalankan ceremony ulang dengan transcript baru dan publikasikan checksum.

### AUR-ISSUE-002: Chain ID Tidak Konsisten

**Prioritas:** Critical
**Status:** Open
**Lokasi:** genesis, runtime, transport, wallet, dan dokumentasi

Temuan nilai chain ID (terverifikasi pada commit `dd34f7c`):

- `GENESIS_CHAIN_ID = 1001` pada `src/primitives/genesis/builder.rs:12`.
- runtime default menggunakan `1001` pada `src/platform/runtime/config.rs:43`.
- transport default menggunakan `1` pada `src/platform/wire/zenoh_transport.rs:39`.
- wallet CLI default menggunakan `1` pada `src/platform/wallet/cli.rs:178`.
- P2P bootnode hardcode `1001` pada `src/platform/cli/dispatcher.rs:820`.
- `MAINNET_CONFIG.toml:9` menggunakan `1001`.
- dokumentasi konstitusi masih menyebut mainnet `1`
  (`AURION-TRANSACTION-SPECIFICATION.md:38`, `AURION-SERIALIZATION-AND-WIRE-PROTOCOL.md:118`).
- **Ketidaksesuaian tipe:** genesis/ceremony/wallet memakai `chain_id: u32`,
  sedangkan handshake, transport, snapshot, telemetry, dan health memakai
  `u64`. Tidak ada satu tipe kanonikal tunggal.

Dampaknya adalah transaksi dapat ditandatangani untuk network yang salah,
topic Zenoh dapat terpisah, handshake dapat gagal, dan replay protection tidak
memiliki satu sumber kebenaran.

**Tindakan wajib:**

1. Tetapkan satu chain ID production melalui keputusan protokol.
2. Jadikan nilai tersebut satu-satunya sumber konfigurasi.
3. Hapus default yang berbeda dari transport dan wallet.
4. Seragamkan tipe `chain_id` menjadi satu tipe kanonikal.
5. Regenerasi reference vector dan genesis artifact bila nilai berubah.
6. Tambahkan test lintas genesis, runtime, transport, dan wallet.

### AUR-ISSUE-003: Format Transaksi Berbeda dari Spesifikasi

**Prioritas:** Critical
**Status:** Open
**Lokasi:** `src/statemachine/transaction/types.rs` dan dokumen transaksi

Kode memakai field (terverifikasi pada `types.rs:44-58`):

```text
version: u16
chain_id: u32
tx_type
flags
sender
recipient
nonce
amount
fee
valid_until
payload
signature
```

Dokumen `AURION-TRANSACTION-SPECIFICATION.md:20-46` mendeskripsikan
`version: u32`, `chain_id: u64`, dan tidak
memuat `tx_type`, `flags`, atau `valid_until`; urutannya juga berbeda
(versi, chain id, nonce, sender, recipient, amount, fee, payload_len,
payload, signature).

Dampaknya:

- signature dapat tidak cocok (signing preimage berbeda antara wallet dan validator);
- TxID dapat berbeda;
- transaksi lintas SDK tidak kompatibel;
- validator dan mobile dapat membaca byte secara berbeda;
- bootnode tidak memiliki kontrak payload yang pasti.

**Tindakan wajib:**

1. Pilih satu transaction schema resmi.
2. Pilih satu canonical encoding dan signing preimage.
3. Buat test vector byte-level.
4. Regenerasi wallet signing dan validator decoding.
5. Jadikan bootnode relay opaque sampai schema final tersedia.

### AUR-ISSUE-004: Keystore Menggunakan Kriptografi Custom

**Prioritas:** Critical
**Status:** Open
**Lokasi:** `src/platform/wallet/keystore.rs`

Keystore menggunakan KDF Blake3 custom, XOR stream cipher custom, nonce yang
diturunkan dari salt, dan salt pseudo-random berbasis waktu. Format ini belum
setara dengan format keystore production yang diaudit. Dokumen resmi
`01-WALLET-RULES.md:27-28` justru mewajibkan **Argon2id** (memory >= 64 MB)
atau scrypt untuk KDF dan **ChaCha20-Poly1305** atau AES-256-GCM untuk enkripsi;
implementasi `keystore.rs` menggunakan cipher `blake3-stream-v1`.

Risikonya mencakup perlindungan password yang tidak memadai, tidak adanya AEAD
standar, parameter yang sulit diaudit, dan interoperabilitas wallet yang buruk.

**Tindakan wajib:**

1. Hentikan format ini untuk private key production baru.
2. Gunakan AEAD standar seperti AES-GCM atau ChaCha20-Poly1305.
3. Gunakan KDF password seperti Argon2id atau scrypt.
4. Tetapkan parameter KDF, nonce, salt, version, dan migration policy.
5. Tambahkan test vector dan test tampering.
6. Sediakan migrasi tanpa mencetak private key ke log atau disk sementara.

### AUR-ISSUE-005: Password Default Wallet Lemah

**Prioritas:** High
**Status:** Closed (Remediasi selesai, diverifikasi)
**Lokasi:** `src/platform/wallet/cli.rs`, `src/platform/wallet/password.rs`

Wallet CLI memakai `password123` sebagai default pada operasi `create`,
`import`, dan `sign-tx`. Keystore yang dibuat tanpa perhatian operator dapat
langsung ditebak. CLI juga menerima password melalui argumen command line
(terekspos di history shell/process list).

**Remediasi (diterapkan):**

- password default `password123` dihapus; `handle_create`, `handle_import`,
  dan `handle_sign_tx` kini memerlukan password eksplisit;
- resolver baru `src/platform/wallet/password.rs` dengan 3-tier precedence:
  1. stdin (flag `--password-stdin`);
  2. environment `AURION_WALLET_PASSWORD`;
  3. prompt interaktif via `rpassword` (no echo), dengan konfirmasi ganda
     saat `create`;
- opsi `--password`/`--passphrase` di argv tidak didukung dan diabaikan
  dengan warning (mencegah eksposure ke process list / shell history);
- password kosong ditolak, konfirmasi mismatch ditolak;
- test ditambahkan pada `password.rs`, total suite lib menjadi 153 passed.
  Guardrail: 100% canonical, zero conflicts.

### AUR-ISSUE-006: Address Creator dan Developer Placeholder

**Prioritas:** High
**Status:** Open
**Lokasi:** `docs/Constitutions/AURION-GENESIS-SPECIFICATION.md:88-89`

Dokumen genesis masih menggunakan label placeholder:

```text
aur1q_creator_vault_sovereign_mainnet_genesis_key_001
aur1q_developer_vault_r_and_d_faucet_source_key_002
```

Label tersebut bukan address Bech32m kriptografis yang dapat diverifikasi.
**Koreksi verifikasi:** nilai aktual sudah tersedia pada `GENESIS_CEREMONY.json`
dan `MAINNET_GENESIS_BLOCK.json`:
- Creator: `aur1jjtqrlqy9suehhltnzt2ml4zwsr8ukpyvvhm2gw899u0e0w22qusq0pjql`
- Developer: `aur1eaj265jvs5wzgdyr9d9p2elkgckx07r0gqc9kejwcznyplw2zlqqlxdu7y`

Dampak langsung adalah dokumentasi konstitusi tidak lagi sinkron dengan
artifact genesis yang diratifikasi, sehingga konsumen dokumen dapat memakai
label yang salah sebagai alamat.

**Tindakan wajib:**

1. Ganti placeholder dengan address Bech32m yang benar (nilai sudah tersedia).
2. Sertakan public key dan aturan derivasi (`m/44'/9999'/0'/0'/0'`).
3. Cocokkan address dengan genesis state root.
4. Publikasikan checksum artifact yang disetujui.

### AUR-ISSUE-007: Ukuran Transaksi Tidak Konsisten

**Prioritas:** High
**Status:** Open
**Lokasi:** codec, validator, wallet, wire limit, dan dokumentasi

Kode mendefinisikan `TRANSACTION_BASE_BYTES = 184` dan payload maksimum 24 KiB
(`src/statemachine/transaction/types.rs:6-7`).
Dokumen `AURION-SERIALIZATION-AND-WIRE-PROTOCOL.md:113-128` mendeskripsikan
`148 + N` byte dan payload maksimum 64 KiB (65.536 B).
Wire message `MSG_TX_GOSSIP` dibatasi 68 KiB pada
`src/platform/wire/messages.rs:49`.

Dampaknya dapat berupa perbedaan fee calculation, penolakan transaksi valid,
perbedaan buffer bootnode, dan test vector yang tidak sesuai kode.

**Tindakan wajib:**

Tetapkan formula ukuran berdasarkan schema final, lalu sinkronkan codec,
validator, wallet, wire limits, bootnode limits, fee policy, dan dokumentasi.

### AUR-ISSUE-008: Format CommitCertificate Berbeda

**Prioritas:** High
**Status:** Open
**Lokasi:** consensus code dan wire specification

Dokumen `AURION-SERIALIZATION-AND-WIRE-PROTOCOL.md:131-140` menggambarkan
certificate sebagai `height`, `round`, `block_hash`, `signatures_count`, dan
array `validator_index (u32) + signature (64B)` (sub-total 68 bytes/signature).

Implementasi `CommitCertificate` pada
`src/consensus/bft/certificate.rs:91-96` memakai
`block_hash, height, round, precommits: Vec<Vote>`, dan setiap `Vote`
(`src/consensus/bft/vote.rs:27-34`) juga memuat phase (u8), height, round,
block hash, validator index, dan signature (total 117 bytes/vote).

Dampaknya bootnode dan mobile dapat salah melakukan decoding atau filtering
commitment.

**Tindakan wajib:**

1. Tetapkan format certificate berdasarkan validator implementation.
2. Buat canonical test vectors.
3. Definisikan validator set reference dan quorum calculation.
4. Bootnode meneruskan certificate opaque sampai format final dikunci.
5. Mobile melakukan verifikasi signature dan quorum secara independen.

### AUR-ISSUE-009: Validasi Frame Belum Strict

**Prioritas:** High
**Status:** Open
**Lokasi:** `src/platform/wire/frame.rs` dan `messages.rs`

Kode menyediakan `max_payload_bound(message_type)` pada `messages.rs:40`, tetapi
`parse_network_frame()` (`frame.rs:114`) hanya memeriksa batas global 8 MiB.

Parser juga belum menolak secara umum:

- `reserved != 0`;
- `reserved2` non-zero;
- message type tidak dikenal;
- payload yang melewati batas khusus message type.

Dampaknya aturan anti-DoS dan strict wire validation belum sepenuhnya
terlaksana.

**Tindakan wajib:**

1. Tegakkan batas per message type sebelum alokasi lanjutan.
2. Tolak reserved field yang tidak nol.
3. Tolak message type tidak dikenal atau definisikan rute fallback eksplisit.
4. Tambahkan malformed-frame dan oversized-message tests.
5. Terapkan aturan yang sama pada bootnode atau gunakan relay opaque dengan limit aman.

### AUR-ISSUE-010: Genesis Artifact Belum Terikat ke Binary Aktif

**Prioritas:** High
**Status:** Open
**Lokasi:** source genesis, transcript, JSON artifact, runtime, dan dokumen

Repository memiliki beberapa sumber parameter genesis: builder
(`src/primitives/genesis/builder.rs`), kanonikal `GENESIS_CEREMONY.json`
(diimbuhkan via `include_str!` pada `ceremony.rs:567-568` sekaligus dibaca dari
disk pada `canonical_mainnet_genesis()` `ceremony.rs:571-586`), runtime
configuration, dan dokumen konstitusi. `canonical_mainnet_genesis()` justru
**memprioritaskan file `GENESIS_CEREMONY.json` di disk** jika ada dan valid,
sehingga binary dapat memproduksi genesis berbeda dari yang disegel.

Karena chain ID dan schema berbeda antar sumber, belum ada satu langkah
verifikasi reproducible yang mengikat semuanya ke binary aktif; startup juga
belum menolak genesis mismatch terhadap nilai yang diratifikasi.

**Tindakan wajib:**

1. Buat satu manifest genesis resmi.
2. Masukkan chain ID, genesis hash, state root, validator set, dan public keys.
3. Verifikasi manifest terhadap binary dari clean build.
4. Simpan checksum binary dan artifact.
5. Jadikan startup menolak genesis mismatch.

## 4. Urutan Perbaikan

### Tahap 0: Freeze

- hentikan penggunaan genesis key deterministic;
- hentikan deployment wallet production dengan password default;
- jangan aktifkan bootnode terhadap genesis yang belum dikunci;
- tandai artifact saat ini sebagai development atau non-production.

### Tahap 1: Ratifikasi Protokol

- tetapkan chain ID;
- tetapkan transaction schema;
- tetapkan certificate schema;
- tetapkan canonical codec dan test vectors;
- tetapkan address Creator dan Developer yang valid.

### Tahap 2: Rekonstruksi Genesis

- generate key offline;
- lakukan ceremony multisaksi;
- bangun state genesis;
- hitung state root dan block hash;
- buat manifest dan transcript;
- verifikasi oleh validator independen.

### Tahap 3: Perbaikan Wallet

- ganti keystore dengan format standar yang diaudit;
- hapus password default;
- sinkronkan chain ID;
- sinkronkan transaction encoder dan signing preimage;
- tambahkan migration dan recovery test.

### Tahap 4: Perbaikan Wire dan Bootnode

- kunci message limits;
- perbaiki strict frame validation;
- implementasikan handshake berdasarkan genesis hash;
- gunakan relay opaque untuk transaksi dan certificate sampai schema final;
- tambahkan failover dan observability.

### Tahap 5: Verifikasi dan Release

- jalankan clean build;
- jalankan unit dan conformance tests;
- jalankan cross-language transaction vectors;
- jalankan staging validator dan bootnode;
- lakukan security review;
- publikasikan release manifest.

## 5. Kriteria Penutupan Isu

Isu hanya boleh ditutup jika tersedia:

- perubahan kode atau keputusan formal yang menyelesaikan akar masalah;
- test yang gagal sebelum perbaikan dan lulus setelah perbaikan;
- dokumen protokol yang diperbarui;
- artifact checksum atau test vector yang dapat direproduksi;
- review minimal oleh dua pihak untuk genesis dan cryptography;
- catatan compatibility dan migration bila format berubah.

## 6. Keputusan Wajib Sebelum Bootnode Production

1. Chain ID mainnet final: `1` atau `1001`.
2. Transaction layout final dan field version.
3. Batas payload transaksi final.
4. Format `CommitCertificate` final.
5. Algoritma dan format Merkle proof.
6. Address Creator dan Developer final.
7. Status genesis key lama: revoked, compromised, atau development-only.
8. Format keystore production.
9. Validator set dan public key yang diratifikasi.
10. Genesis hash yang wajib dipin oleh bootnode.

## 7. Kesimpulan

Alokasi nominal Creator dan Developer sesuai dokumen, tetapi custody key, chain
identity, wallet encoding, keystore, dan artifact genesis belum dapat dianggap
konsisten atau production-safe. Bootnode harus menunggu keputusan protokol dan
genesis baru sebelum dijadikan relay mainnet.
