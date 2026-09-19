# Temuan Isu Protokol Aurion

Dokumen ini mencatat temuan masalah pada kode, dokumentasi, artefak genesis,
wallet, dan protokol wire Aurion. Fokus dokumen adalah isu teknis dan keamanan;
temuan guardrail administratif tidak dibahas di sini.

## 1. Status Audit

Audit membandingkan source code, dokumen konstitusi, artefak genesis, wallet,
transport, dan test library Aurion.

Hasil pengujian library:

```text
150 passed
0 failed
```

Test tersebut hanya membuktikan test yang tersedia. Ia belum membuktikan
konsistensi lintas genesis, wallet, validator, mobile, bootnode, dan dokumentasi.

## 2. Ringkasan Isu

| ID | Isu | Prioritas | Status |
|---|---|---|---|
| AUR-ISSUE-001 | Genesis key deterministic tertanam di source | Critical | Open |
| AUR-ISSUE-002 | Chain ID berbeda antar komponen | Critical | Open |
| AUR-ISSUE-003 | Format transaksi kode berbeda dari spesifikasi | Critical | Open |
| AUR-ISSUE-004 | Keystore menggunakan kriptografi custom berisiko | Critical | Open |
| AUR-ISSUE-005 | Password default wallet lemah | High | Open |
| AUR-ISSUE-006 | Address Creator/Developer berupa placeholder | High | Open |
| AUR-ISSUE-007 | Ukuran transaksi tidak konsisten | High | Open |
| AUR-ISSUE-008 | Format CommitCertificate berbeda dari dokumentasi | High | Open |
| AUR-ISSUE-009 | Validasi frame belum menegakkan semua batas | High | Open |
| AUR-ISSUE-010 | Genesis artifact belum diverifikasi terhadap binary aktif | High | Open |

## 3. Temuan Detail

### AUR-ISSUE-001: Genesis Key Deterministic di Source

**Prioritas:** Critical
**Status:** Open
**Lokasi:** `src/primitives/genesis/ceremony.rs`

`CanonicalCeremonyKeypairs::new_deterministic()` membuat keypair Creator,
Developer, dan empat validator menggunakan seed tetap seperti `[0x01; 32]`,
`[0x02; 32]`, dan `[0x11; 32]` sampai `[0x14; 32]`.

Jika key tersebut digunakan untuk production, siapa pun yang membaca source
dapat merekonstruksi private key dan berpotensi:

- menandatangani transaksi Creator atau Developer;
- mengendalikan identity validator genesis;
- memalsukan attestation ceremony;
- mengganggu operasi validator.

**Tindakan wajib:**

1. Hentikan penggunaan key deterministic untuk production.
2. Perlakukan key lama sebagai compromised.
3. Buat key baru offline menggunakan CSPRNG atau HSM.
4. Pisahkan custody Creator, Developer, dan validator.
5. Jalankan ceremony ulang dengan transcript baru.
6. Regenerasi genesis state, state root, block hash, dan artifact.

### AUR-ISSUE-002: Chain ID Tidak Konsisten

**Prioritas:** Critical
**Status:** Open
**Lokasi:** genesis, runtime, transport, wallet, dan dokumentasi

Temuan nilai chain ID:

- `GENESIS_CHAIN_ID = 1001` pada `src/primitives/genesis/builder.rs`.
- runtime default menggunakan `1001` pada `src/platform/runtime/config.rs`.
- transport default menggunakan `1` pada `src/platform/wire/zenoh_transport.rs`.
- wallet CLI default menggunakan `1` pada `src/platform/wallet/cli.rs`.
- dokumentasi konstitusi masih menyebut mainnet `1`.
- mainnet guide dan artifact genesis menggunakan `1001`.

Dampaknya adalah transaksi dapat ditandatangani untuk network yang salah,
topic Zenoh dapat terpisah, handshake dapat gagal, dan replay protection tidak
memiliki satu sumber kebenaran.

**Tindakan wajib:**

1. Tetapkan satu chain ID production melalui keputusan protokol.
2. Jadikan nilai tersebut satu-satunya sumber konfigurasi.
3. Hapus default yang berbeda dari transport dan wallet.
4. Regenerasi reference vector dan genesis artifact bila nilai berubah.
5. Tambahkan test lintas genesis, runtime, transport, dan wallet.

### AUR-ISSUE-003: Format Transaksi Berbeda dari Spesifikasi

**Prioritas:** Critical
**Status:** Open
**Lokasi:** `src/statemachine/transaction/types.rs` dan dokumen transaksi

Kode memakai field:

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

Dokumen transaksi mendeskripsikan `version: u32`, `chain_id: u64`, dan tidak
memuat `tx_type`, `flags`, atau `valid_until`.

Dampaknya:

- signature dapat tidak cocok;
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
setara dengan format keystore production yang diaudit.

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
**Status:** Open
**Lokasi:** `src/platform/wallet/cli.rs`

Wallet CLI memakai `password123` sebagai default pada beberapa operasi wallet.
Keystore yang dibuat tanpa perhatian operator dapat langsung ditebak.

**Tindakan wajib:**

- hapus password default;
- minta password melalui input aman atau secret terkontrol;
- tolak password kosong dan password umum;
- jangan menampilkan password di command line atau log;
- tambahkan test bahwa operasi berhenti tanpa password.

### AUR-ISSUE-006: Address Creator dan Developer Placeholder

**Prioritas:** High
**Status:** Open
**Lokasi:** `docs/Constitutions/AURION-GENESIS-SPECIFICATION.md`

Dokumen genesis menggunakan label seperti:

```text
aur1q_creator_vault_sovereign_mainnet_genesis_key_001
aur1q_developer_vault_r_and_d_faucet_source_key_002
```

Kode menghasilkan address dari public key melalui keypair ceremony. Label pada
dokumen bukan address Bech32m kriptografis yang dapat diverifikasi.

**Tindakan wajib:**

1. Ganti placeholder dengan address Bech32m yang benar.
2. Sertakan public key dan aturan derivasi.
3. Cocokkan address dengan genesis state root.
4. Publikasikan checksum artifact yang disetujui.

### AUR-ISSUE-007: Ukuran Transaksi Tidak Konsisten

**Prioritas:** High
**Status:** Open
**Lokasi:** codec, validator, wallet, wire limit, dan dokumentasi

Kode mendefinisikan `TRANSACTION_BASE_BYTES = 184` dan payload maksimum 24 KiB.
Dokumen transaksi mendeskripsikan `148 + N` byte dan payload maksimum 64 KiB.
Wire message memberi batas transaction gossip 68 KiB.

Dampaknya dapat berupa perbedaan fee calculation, penolakan transaksi valid,
perbedaan buffer bootnode, dan test vector yang tidak sesuai kode.

**Tindakan wajib:**

Tetapkan formula ukuran berdasarkan schema final, lalu sinkronkan codec,
validator, wallet, wire limits, bootnode limits, fee policy, dan dokumentasi.

### AUR-ISSUE-008: Format CommitCertificate Berbeda

**Prioritas:** High
**Status:** Open
**Lokasi:** consensus code dan wire specification

Dokumen wire menggambarkan certificate sebagai `height`, `round`, `block_hash`,
dan array `validator_index + signature`.

Implementasi consensus menggunakan `precommits: Vec<Vote>`, sedangkan setiap
`Vote` juga memuat phase, height, round, block hash, validator index, dan
signature.

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

Kode menyediakan `max_payload_bound(message_type)`, tetapi
`parse_network_frame()` hanya memeriksa batas global 8 MiB.

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

Repository memiliki beberapa sumber parameter genesis: builder, ceremony
transcript, JSON genesis, runtime configuration, dan dokumen konstitusi. Karena
chain ID dan schema berbeda antar sumber, belum ada satu langkah verifikasi
reproducible yang mengikat semuanya ke binary aktif.

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
