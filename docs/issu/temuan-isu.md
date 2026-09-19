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
| AUR-ISSUE-001 | Genesis key deterministic tertanam di source | Critical | Closed | ✅ Terverifikasi & diremediasi (gating dev/prod + keystore validator) |
| AUR-ISSUE-002 | Chain ID berbeda antar komponen | Critical | Closed | ✅ Terverifikasi & diremediasi (u32 seragam, Mainnet=1001) |
| AUR-ISSUE-003 | Format transaksi kode berbeda dari spesifikasi | Critical | Closed | ✅ Terverifikasi & diremediasi (dokumen mengikuti kode + golden test) |
| AUR-ISSUE-004 | Keystore menggunakan kriptografi custom berisiko | Critical | Closed | ✅ Terverifikasi & diremediasi (Argon2id + ChaCha20Poly1305, envelope V2 + migrasi) |
| AUR-ISSUE-005 | Password default wallet lemah | High | Closed | ✅ Terverifikasi & sudah diremediasi |
| AUR-ISSUE-006 | Address Creator/Developer berupa placeholder | High | Open | ✅ Terverifikasi (dokumen) |
| AUR-ISSUE-007 | Ukuran transaksi tidak konsisten | High | Closed | ✅ Terverifikasi & diremediasi (`MAX_TX_WIRE_SIZE` = 24.764 B) |
| AUR-ISSUE-008 | Format CommitCertificate berbeda dari dokumentasi | High | Open | ✅ Terverifikasi |
| AUR-ISSUE-009 | Validasi frame belum menegakkan semua batas | High | Closed | ✅ Terverifikasi & diremediasi (strict wire validation + tests) |
| AUR-ISSUE-010 | Genesis artifact belum diverifikasi terhadap binary aktif | High | Open | ✅ Terverifikasi |

## 3. Temuan Detail

### AUR-ISSUE-001: Genesis Key Deterministic di Source

**Prioritas:** Critical
**Status:** Closed (Remediasi selesai, diverifikasi)
**Lokasi:** `src/primitives/genesis/ceremony.rs:153-160` dan `src/platform/cli/dispatcher.rs` (ceremony & validator start)

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

**Remediasi (diterapkan):**

- `aurion validator start` kini menolak key deterministic pada mode produksi:
  - deterministic (seed `[0x11;32]`–`[0x14;32]`) hanya diizinkan bila flag
    eksplisit `--dev` / `--insecure-deterministic-keys` diberikan, atau pada
    `status` / `--dry-run`;
  - mode produksi wajib `--validator-key-file <PATH>` (keystore terenkripsi
    Blake3-stream) yang dibuka lewat resolver 3-tier (issue 005):
    `--validator-password-stdin` / env `AURION_VALIDATOR_PASSWORD` /
    prompt interaktif `rpassword` no-echo;
- leak argv `--creator-password` / `--developer-password` ditutup: nilai argv
  diabaikan dengan warning; pembukaan keystore Creator/Developer kini lewat
  resolver 3-tier dengan env `AURION_CREATOR_PASSWORD` / `AURION_DEVELOPER_PASSWORD`;
- guardrail 100% canonical, unit suite 153 passed, integration mainnet/genesis
  ceremony 5 passed.

**Sisa terdokumentasi:**

- `aurion genesis ceremony run` tanpa opsi `--creator-keystore` /
  `--developer-keystore` masih menghasilkan transcript dengan key deterministic
  (path khusus pengujian/dev; prod harus menyediakan keystore nyata).

### AUR-ISSUE-002: Chain ID Tidak Konsisten

**Prioritas:** Critical
**Status:** Closed (Remediasi selesai, diverifikasi)
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

**Remediasi (diterapkan):**

- Keputusan protokol: **satu type kanonikal `chain_id: u32`** dan **Mainnet =
  `1001`** (`GENESIS_CHAIN_ID`), diambil dari kesesuaian kode + `CONTEXT_ANCHOR`.
- Seluruh 13 titik `u64` diseragamkan menjadi `u32`: `runtime/config.rs`,
  `wire/zenoh_transport.rs`, `wire/handshake.rs` (HandshakeHello 188 B → 184 B),
  `state/snapshot.rs`, `telemetry/metrics.rs`, `telemetry/health.rs`,
  `gateway/explorer.rs`, `gateway/rpc/methods.rs`, dan DTO `cli/dispatcher.rs`.
- Default divergen dihapus: transport (`1`) dan wallet CLI (`1`) kini memakai
  `GENESIS_CHAIN_ID`; cast `GENESIS_CHAIN_ID as u64` dihapus.
- Dokumentasi konstitusi/operasional disinkronkan: `AURION-GENESIS-SPECIFICATION.md`,
  `AURION-TRANSACTION-SPECIFICATION.md`, `AURION-SERIALIZATION-AND-WIRE-PROTOCOL.md`,
  `AURION-REFERENCE-TEST-VECTORS.md`, dan `MULTI_REGION_TESTNET_GUIDE.md`.
- Golden test byte-level `tests/golden_vectors.rs` mengunci `chain_id = 1001`
  pada transaksi, preimage, TxID, header, dan BlockHash.

### AUR-ISSUE-003: Format Transaksi Berbeda dari Spesifikasi

**Prioritas:** Critical
**Status:** Closed (Remediasi selesai, diverifikasi)
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

**Remediasi (diterapkan):**

- Keputusan protokol: **kode aktual + `CONTEXT_ANCHOR.md` menjadi skema resmi**
  (`version u16`, `chain_id u32`, `tx_type u8`, `flags u8`, `sender`,
  `recipient`, `nonce u64`, `amount u128`, `fee u128`, `valid_until u64`,
  `payload`, `signature`; basis 184 Bytes + prefiks `payload_len` u32).
- Preimage penandatanganan didokumentasikan sesuai kode: `DST_TX ("AURION-TX-V1")
  || 0x00 ||` serialisasi kanonikal seluruh field (kecuali `signature`), dan
  Ed25519 dibuat langsung atas preimage. `TxID` memakai
  `Blake3DeriveKey("AURION-TX-ID-V1", EncodeCanonical(Tx))`.
- `AURION-TRANSACTION-SPECIFICATION.md` dan
  `AURION-SERIALIZATION-AND-WIRE-PROTOCOL.md` diperbarui ke Header 8B dan
  payload maksimum 24 KiB.
- `AURION-REFERENCE-TEST-VECTORS.md` diregenerasi (transaksi 188 B, preimage
  137 B, TxID, header 124 B, BlockHash) dari kode.
- Golden test byte-level `tests/golden_vectors.rs` mengunci canonical bytes,
  preimage, hash preimage, tanda tangan, TxID, header, dan BlockHash, serta
  memverifikasi signature lintas wallet/validator.
- Drift timestamp genesis (dokumen `1773570000` vs kode & artifact
  `1773532800`) dihilangkan: `AURION-GENESIS-SPECIFICATION.md` dan Vektor 6
  (`AURION-REFERENCE-TEST-VECTORS.md`, header 124 B + GenesisHash)
  diregenerasi ke `1773532800` (15 Maret 2026 00:00:00 UTC), konsisten dengan
  `MAINNET_GENESIS_BLOCK.json`, `GENESIS_CEREMONY.json`, `build_genesis`, dan
  `tests/mainnet_launch.rs`.

### AUR-ISSUE-004: Keystore Menggunakan Kriptografi Custom

**Prioritas:** Critical
**Status:** Closed (Remediasi selesai, diverifikasi)
**Lokasi:** `src/platform/wallet/keystore.rs`, `src/platform/wallet/keystore/legacy.rs`

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

**Remediasi (diterapkan):**

- Dependensi kanonik ditambahkan: `argon2 0.5`, `chacha20poly1305 0.10`,
  `rand 0.8` (CSPRNG OS).
- Envelope **V2** diimplementasikan di `keystore.rs`: `version: 2` + `address`
  + blok `kdf` (`algorithm: "argon2id"`, `m_cost: 65536` KiB = 64 MiB,
  `t_cost: 3`, `p_cost: 4`, `salt` 16 byte acak) + blok `cipher`
  (`algorithm: "chacha20poly1305"`, `nonce` 12 byte acak, `ciphertext` =
  32-byte secret + 16-byte Poly1305 tag). Salt & nonce berasal dari `OsRng`.
- `Keystore::encrypt` kini selalu menghasilkan V2; kegagalan verifikasi AEAD
  (password salah / ciphertext dimanipulasi) dipetakan ke
  `KeystoreError::InvalidPassword` tanpa membocorkan plaintext.
- Jalur legacy `blake3-stream-v1` diisolasi ke
  `src/platform/wallet/keystore/legacy.rs` dan hanya dipakai untuk membaca.
- Migrasi otomatis: `Keystore::unlock_and_migrate` dan
  `unlock_and_migrate_to_file` mengenkripsi ulang keystore V1 menjadi V2 dan
  menuliskannya kembali; dipakai oleh `wallet sign-tx` serta jalur
  `genesis ceremony` (creator/developer) dan `validator start`.
- Test ditambahkan (total suite lib 161 passed): roundtrip V2, password salah,
  tamper ciphertext (Poly1305 tag menolak), dekripsi legacy V1, migrasi
  V1→V2, tidak ada upgrade untuk V2, dan disiplin zeroize. `cargo clippy -D
  warnings` bersih; guardrail 100% canonical.

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
- Residu argv pada `import` ditutup: `--mnemonic "<24 words>"` kini diabaikan
  dengan warning keras; mnemonic hanya diterima via `--mnemonic-stdin`, env
  `AURION_WALLET_MNEMONIC`, atau prompt interaktif tanpa echo melalui
  `resolve_mnemonic` di `password.rs`.

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

**Catatan remediasi:**

- Sebagai bagian remediasi ISSUE-003: dokumentasi disinkronkan ke `184 + N`
  Bytes + `payload_len` (bukan `148 + N`), dan hardcode `148` di
  `src/consensus/mempool/engine.rs` diganti `TRANSACTION_BASE_BYTES + 4 + payload`.
- Audit wire diselesaikan bersamaan dengan ISSUE-009: `MAX_TRANSACTION_WIRE_BYTES`
  / `MAX_TX_WIRE_SIZE` = `184 + 4 + 24.576` = **24.764 B** ditambahkan sebagai
  konstanta kanonikal; `MSG_TX_GOSSIP` kini memakai plafon ini (bukan 68 KiB),
  dan `src/consensus/mempool/engine.rs` memakai helper `transaction_wire_size()`.
- **Kebijakan fee:** STF hanya menegakkan `fee > 0` (flat, tanpa komponen ukuran),
  sehingga tidak ada kebijakan fee berbasis ukuran yang perlu disinkronkan.
  Keselarasan tercapai *by construction*: validator menolak payload > 24 KiB,
  wire menolak frame > 24.764 B, dan mempool memakai formula ukuran yang sama.
  Tidak ada perubahan aturan konsensus.
- Dokumentasi `AURION-SERIALIZATION-AND-WIRE-PROTOCOL.md` Bagian 6 diperbarui
  (`TX_GOSSIP` = 24.764 B). Status **Closed**.

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
**Status:** Closed
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

**Catatan remediasi:**

- `max_payload_bound()` kini benar-benar ditegakkan: `MSG_TX_GOSSIP` =
  `MAX_TX_WIRE_SIZE` (24.764 B), tipe dikenal lain memakai plafon katalog, dan
  tipe tak dikenal mengembalikan `0` (tanpa fallback implisit). Ditambah
  predikat `is_known_message_type()` di `messages.rs`.
- `frame.rs` menambahkan varian error `UnknownMessageType(u16)`,
  `NonZeroReserved(u16)`, `NonZeroReserved2([u8; 8])` dan helper
  `validate_message_type()` / `validate_payload_bound()`. Urutan validasi
  `parse_network_frame()`: magic $\to$ tipe dikenal $\to$ reserved $\to$ reserved2
  $\to$ plafon per tipe $\to$ backstop global 8 MiB $\to$ panjang $\to$ checksum.
  `serialize_network_frame()` menegakkan aturan tipe + plafon yang sama.
- Decoder transaksi (`Transaction::decode_canonical`) memakai
  `decode_length_prefixed_bytes()` sehingga `payload_len` divalidasi terhadap
  `MAX_TRANSACTION_PAYLOAD_BYTES` sebelum alokasi (`CodecError::ExcessiveAllocation`
  kini memuat `{ max, requested }`).
- Pengujian: `tests/security_hardening.rs` menambah
  `test_strict_wire_frame_rejects_oversized_per_message_type`,
  `test_strict_wire_frame_rejects_unknown_type_and_nonzero_reserved`, dan
  `test_transaction_decoder_rejects_oversized_payload_before_allocation`.
  `tests/property_tests.rs` membatasi roundtrip ke tipe dikenal + plafon tipe.
  `tests/conformance.rs` dan `src/platform/conformance/runner.rs` memakai
  `MSG_TX_GOSSIP` (0x0030). Status **Closed**.

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
