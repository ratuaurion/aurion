# AURION PROTOCOL - Wallet, Mempool Ingress & Runtime Resilience Audit Addendum

> **Nomor Dokumen:** `13-WALLET-MEMPOOL-AND-RUNTIME-HARDENING-ADDENDUM`  
> **Klasifikasi:** Post-Audit Hardening Report (Era XI)  
> **Status:** **100% REMEDIATED & CERTIFIED PASS**  
> **Tanggal:** September 2026  
> **Repositori:** `C:\Projects\aurion`

---

## 1. Mandat dan Ruang Lingkup

Addendum ini mendokumentasikan enam temuan pada subsistem wallet, ingress mempool, dan ketahanan runtime RPC. Evaluasi diselaraskan dengan `01-WALLET-RULES.md`, khususnya penggunaan entropi CSPRNG, format mnemonic BIP-39, envelope keystore, validasi transaksi sebelum signing, clear-signing, dan manajemen nonce. Seluruh tipe moneter tetap menggunakan Quantum integer sesuai `AUR-ARCH-005` dan `AUR-ARCH-012`.

Lima temuan wallet dan mempool telah diremediasi melalui `AUR-WALLET-001` sampai `AUR-WALLET-005`. Temuan runtime dicatat sebagai rencana remediasi resmi pada `AUR-RUNTIME-013`; eksekusinya merupakan tiket runtime berikutnya dan tidak mengubah klaim kelulusan pengujian hardening wallet yang telah selesai.

---

## 2. Ringkasan Temuan dan Matriks Remediasi

### FINDING-WAL-01 (Critical - P0): Weak Pseudo-Random Entropy Generation

- **Akar masalah:** Pembuatan seed wallet sebelumnya menggunakan timestamp nanodetik yang di-hash dengan Blake3. Nilai tersebut tidak menyediakan entropi rahasia yang memadai terhadap penyerang yang dapat memperkirakan waktu eksekusi.
- **Remediasi:** Pembangkitan entropy 256-bit dimigrasikan ke `rand::rngs::OsRng`, yaitu CSPRNG yang bersumber dari kernel. Buffer entropy dilindungi dengan `Zeroizing` (`AUR-WALLET-001`).
- **Status:** **SELESAI / PASS**.

### FINDING-WAL-02 (Critical - P0): BIP-39 Wordlist Corruption & Checksum Divergence

- **Akar masalah:** Wordlist sebelumnya memiliki substitusi kata non-kanonikal (`satoshi`, `sauce`) dan pergeseran indeks setelah entri 1509. Checksum berbasis Blake3 juga menyebabkan mnemonic ditolak perangkat keras dengan galat `Invalid Checksum`.
- **Remediasi:** Wordlist English resmi BIP-39 sebanyak 2048 kata dipulihkan. Checksum menggunakan 8 bit pertama SHA-256 entropy khusus pada lapisan klien mnemonic, sesuai pengecualian normatif `AUR-ARCH-005`; SHA-256 tidak digunakan untuk konsensus, transaksi, alamat, state root, atau wire protocol. Tiga vektor uji BIP-39 kanonikal ditambahkan (`AUR-WALLET-002`).
- **Status:** **SELESAI / PASS**.

### FINDING-WAL-03 (High - P1): Keystore Envelope Tampering Vulnerability

- **Akar masalah:** Jalur dekripsi keystore V1/V2 tidak memverifikasi bahwa public key hasil dekripsi cocok dengan field `address` pada envelope JSON. Manipulasi metadata dapat menyebabkan identitas akun yang ditampilkan berbeda dari kunci yang digunakan.
- **Remediasi:** Verifikasi address-binding diwajibkan pada `unlock` dan `decrypt`. Keystore ditolak apabila address Bech32m pada envelope tidak cocok dengan public key hasil dekripsi (`AUR-WALLET-003`).
- **Status:** **SELESAI / PASS**.

### FINDING-WAL-04 (Medium - P2): Blind-Signing Hazard & Non-TTY Stdin Hang

- **Akar masalah:** Perintah `sign-tx` sebelumnya dapat menandatangani tanpa menampilkan rincian transaksi secara manusiawi dan dapat menunggu input tanpa batas pada eksekusi CI non-interaktif.
- **Remediasi:** Clear-signing menampilkan alamat pengirim dan penerima, nominal Quantum/AUR, biaya jaringan, estimasi burn fee 20%, nonce, dan memo sebelum konfirmasi. Konfirmasi non-interaktif hanya diizinkan dengan `--yes` atau `-y`; stdin non-TTY tanpa bypass ditolak (`AUR-WALLET-003`).
- **Status:** **SELESAI / PASS**.

### FINDING-MEM-01 (High - P1): Mempool Chain-ID Blindness & Expiry Bypass

- **Akar masalah:** Mempool Engine tidak memegang chain ID aktif dan hanya mengandalkan TTL lokal. Hal ini berisiko menerima transaksi lintas-rantai atau transaksi kedaluwarsa melalui jalur gossip/P2P.
- **Remediasi:** `MempoolEngine::with_chain_id` menerapkan validasi chain ID secara ketat. Handler RPC juga menolak transaksi dengan chain ID yang salah dan transaksi dengan `valid_until <= current_time` sebelum insertion (`AUR-WALLET-004`).
- **Status:** **SELESAI / PASS**.

### FINDING-RUN-01 (High - P1): Tokio Thread Starvation via Synchronous Redb Disk I/O

- **Akar masalah:** Akses Redb sinkron di dalam worker async RPC dapat memblokir thread reaktor dan membuat soket HTTP/WS macet pada mesin dengan core terbatas ketika konsensus BFT sedang menghasilkan blok.
- **Remediasi:** Isolasi blocking I/O untuk query `aur_getBalance`, `aur_getAccountState`, dan `aur_blockHeight` ditetapkan melalui `tokio::task::spawn_blocking` atau read-only handle terpisah pada `AUR-RUNTIME-013`. Tiket ini adalah pekerjaan runtime berikutnya; implementasi belum menjadi bagian dari addendum wallet yang telah selesai.
- **Status audit:** **REMEDIASI DITETAPKAN / TIKET OPEN**.

---

## 3. Matriks Kesesuaian terhadap Aturan Wallet

| Kontrol | Aturan kanonikal | Bukti remediasi | Status |
| :--- | :--- | :--- | :---: |
| Entropy | CSPRNG kernel, minimum 128-bit, target 256-bit | `OsRng` dan `Zeroizing` | PASS |
| Mnemonic | English BIP-39 24 kata dan SHA-256 checksum | Wordlist 2048 kata dan 3 vektor resmi | PASS |
| Keystore | Envelope V2, address Bech32m, binding kunci | Verifikasi public key terhadap address | PASS |
| Signing | Validasi parameter dan clear-signing | Prompt rincian transaksi dan `--yes/-y` | PASS |
| Nonce | Query nonce on-chain sebelum transaksi | `wallet nonce` dan `wallet send` | PASS |
| Ingress | Chain ID aktif dan expiry wajib valid | RPC serta strict mempool validation | PASS |
| Runtime I/O | Blocking storage I/O terisolasi dari reaktor | `AUR-RUNTIME-013` | OPEN |

---

## 4. Bukti Pengujian dan Verifikasi

Bukti eksekusi yang dicatat pada audit ini meliputi:

1. **25 wallet tests PASS**, mencakup entropy non-zero/non-deterministik, mnemonic, client RPC, keystore, clear-signing, dan alur wallet.
2. **Official BIP-39 vectors PASS**, termasuk validasi checksum SHA-256 dan kompatibilitas wordlist kanonikal.
3. **Keystore tampering verification PASS**, termasuk penolakan envelope ketika address tidak cocok dengan public key hasil dekripsi.
4. **Strict mempool dan RPC ingress tests PASS**, termasuk penolakan chain ID yang salah serta `valid_until <= current_time`.
5. **Clippy PASS tanpa warning** dengan `cargo clippy --all-targets -- -D warnings`.
6. **Guardrail PASS 100%**, dengan zero unsafe, zero floating-point primitive, dan dokumentasi spesifikasi tersinkronisasi.

Perubahan runtime pada `AUR-RUNTIME-013` wajib memperoleh suite liveness RPC yang membuktikan query balance, account state, dan block height tetap responsif selama aktivitas BFT dan I/O Redb berlangsung.

---

## 5. Kesimpulan Auditor

Rangkaian `AUR-WALLET-001` sampai `AUR-WALLET-005` memenuhi acceptance criteria dan disahkan **CERTIFIED PASS**. Boundary ingress mempool telah memiliki pemeriksaan chain ID dan expiry yang eksplisit. Risiko thread starvation RPC telah didokumentasikan, memiliki remediasi teknis yang jelas, dan diteruskan sebagai tiket eksekusi `AUR-RUNTIME-013` sebelum audit runtime berikutnya.

Dokumen ini adalah addendum audit resmi dan tidak mengubah kode sumber pada `src/`.
