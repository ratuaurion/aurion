# 01 — AURION WALLET APPLICATION RULES SPECIFICATION
## Standar Operasional, Manajemen Kunci, dan Kebijakan Interaksi Pengguna Dompet (Wallet)

> **Hierarki Dokumen:**  
> `AURION CONSTITUTIONS` $\longrightarrow$ `PROTOCOL SPECIFICATIONS` $\longrightarrow$ `00-APPLICATION-RULES` $\longrightarrow$ **`01-WALLET-RULES`**  
>
> **Status:** RATIFIED & LOCKED APPLICATION SPECIFICATION  
> **Klasifikasi:** Standar Dompet Kripto (Wallet Standard Specification)  
> **Versi:** 1.0.0-PROD  
> **Sifat Ketetapan:** Normatif Wajib (RFC 2119 / RFC 8174), Zero-Float, Anti-Malleability

---

## 1. Siklus Hidup Kunci Kriptografi (Key Lifecycle Management)

### 1.1 Pembangkitan Entropi Kunci (Entropy Generation)
1. Perangkat lunak dompet (wallet) **MUST** memperoleh entropi acak langsung dari sumber generator nomor acak kriptografis tingkat sistem operasi (*Kernel CSPRNG*):
   - Linux / Android: `getrandom(2)` dengan flag `GRND_RANDOM` atau `/dev/urandom`.
   - Windows: `BCryptGenRandom` dengan `BCRYPT_USE_SYSTEM_PREFERRED_RNG`.
   - macOS / iOS: `SecRandomCopyBytes`.
   - Web / Browser: `crypto.getRandomValues()`.
2. Wallet **MUST NOT** menggunakan generator pseudorandom yang tidak aman (misalnya `rand()`, `Math.random()`, atau seed berbasis timestamp lokal).
3. Entropi dasar **MUST** berukuran sekurang-kurangnya 128 bit (disarankan 256 bit).

### 1.2 Penyimpanan Kunci Aman (Key Storage Encryption)
1. Seluruh kunci privat dan seed frase mnemonik yang disimpan di media penyimpanan non-volatil (disk/flash) **MUST** dienkripsi menggunakan algoritma enkripsi terotentikasi standar industri:
   - **KDF (Key Derivation Function):** Argon2id dengan parameter minimum: memory $\ge 64\ \text{MB}$, iterations $\ge 3$, parallelism $\ge 4$; atau scrypt ($N=32768, r=8, p=1$).
   - **Enkripsi Simetris:** ChaCha20-Poly1305 (IETF RFC 8439) atau AES-256-GCM.
2. Wallet **MUST NOT** menyimpan kunci privat dalam format teks polos (*plaintext*) pada log file, clipboard, atau crash dump.
3. Seluruh variabel memori RAM yang menampung seed privat atau kunci penandatanganan **MUST** dibersihkan (*zeroized*) secara aman segera setelah selesai digunakan.

### 1.3 Jalur Derivasi Hierarkis (Hierarchical Derivation Path)
1. Wallet yang mengimplementasikan derivasi kunci hierarkis (BIP-32 / BIP-44 / SLIP-0010 untuk Ed25519) **MUST** menggunakan registered coin type resmi Aurion:
   $$\text{Derivation Path: } m / 44' / 9999' / \text{account}' / 0 / \text{address\_index}$$
   - `44'`: Purpose BIP-44.
   - `9999'`: Registered Coin Type resmi protokol Aurion.
   - `account'`: Nomor indeks akun (dimulai dari `0'`).
   - `0`: Change internal/external (selalu `0` untuk model akun Aurion).
   - `address_index`: Indeks alamat sekuensial (dimulai dari `0`).

---

## 2. Penanganan Alamat dan Validasi Antarmuka (Address Handling)

1. **Format Tampilan Kanonikal:** Wallet **MUST** menampilkan alamat akun dalam format resmi **Bech32m (BIP-350)** berpanjang tepat 62 karakter dengan Human-Readable Part (HRP):
   - Mainnet: `aur1...`
   - Testnet: `aurt1...`
2. **Pencocokan Jaringan (Network Cross-Contamination Guard):**
   - Wallet yang terhubung ke Mainnet **MUST** menolak input alamat dengan HRP selain `aur`.
   - Wallet yang terhubung ke Testnet **MUST** menolak input alamat dengan HRP selain `aurt`.
3. **Validasi Checksum Seketika:** Antarmuka input pengguna **MUST** mengevaluasi checksum Bech32m secara *real-time*. Jika ditemukan kesalahan karakter atau manipulasi ketik, tombol kirim **MUST** dinonaktifkan seketika dengan pesan galat yang jelas.
4. **Pencegahan Normalisasi Karakter Campuran:** Seluruh alamat Bech32m **MUST** dinormalisasi ke huruf kecil (*lowercase*) sebelum diproses atau diverifikasi.

---

## 3. Konstruksi Transaksi dan Kebijakan Penandatanganan (Transaction Construction)

### 3.1 Validasi Parameter Sebelum Penandatanganan
Sebelum menandatangani transaksi, wallet **MUST** memvalidasi seluruh syarat berikut:
1. `amount > 0` dan `amount <= MAX_SUPPLY_QUANTA` ($6.6 \times 10^{15}\ Q$).
2. `fee >= MIN_TX_FEE_QUANTA` ($10.000\ Q = 0,0001\ \text{AUR}$).
3. `amount.checked_add(fee)` tidak meluap (*no arithmetic overflow*).
4. `sender != recipient` (kecuali untuk operasi pemusnahan dana yang disetujui pengguna secara eksplisit).
5. `payload.len() <= 65536` bytes.

### 3.2 Transparansi Penandatanganan (Clear Signing Mandate)
1. Wallet **MUST NOT** menandatangani hash biner abstrak (*blind signing*) tanpa menyajikan kepada pengguna rincian manusiawi:
   - Alamat pengirim dan penerima lengkap;
   - Nilai transfer dalam satuan AUR ($Q / 10^8$);
   - Biaya transaksi (Network Fee);
   - Rincian pembakaran biaya: Wallet **SHOULD** menginformasikan bahwa 20% dari fee akan dimusnahkan secara permanen oleh protokol;
   - Data memo/payload teks yang dapat dibaca.
2. Hardware wallet **MUST** mem-parsing seluruh field kanonikal pada layar independen perangkat sebelum meminta konfirmasi fisik dari pengguna.

---

## 4. Manajemen Nonce dan Antrean Transaksi (Nonce Management)

1. **Prinsip Nonce Sekuensial:** Setiap transaksi dari alamat yang sama membutuhkan nilai `nonce` tepat $N = \text{CurrentConfirmedNonce} + K$ di mana $K$ adalah urutan transaksi tertunda (*pending*).
2. **Sinkronisasi Nonce:**
   - Wallet **MUST** mengambil nilai nonce on-chain dari simpul melalui RPC sebelum membuat transaksi baru.
   - Jika terdapat transaksi yang masih berstatus `MEMPOOL`, wallet **MUST** menginkrementasi nonce lokal untuk transaksi berikutnya guna mencegah konflik nonce (*nonce collision*).
3. **Pencegahan Celah Nonce (*Nonce Gap*):** Wallet **MUST NOT** mengizinkan pengiriman transaksi dengan nonce $N+2$ jika transaksi dengan nonce $N+1$ belum pernah disiarkan, karena transaksi tersebut akan tertahan selamanya di mempool.

---

## 5. Penggantian Transaksi (Replace-By-Fee / RBF)

1. Wallet **MAY** menyediakan fitur percepatan transaksi (*Transaction Speed-up / Cancel*) melalui mekanisme Replace-By-Fee (RBF).
2. **Aturan RBF Dompet:**
   - Transaksi pengganti **MUST** memiliki nilai `nonce` yang identik dengan transaksi yang hendak digantikan.
   - Transaksi pengganti **MUST** menetapkan biaya transaksi baru:
     $$\text{NewFee} \ge \text{OldFee} \times 1,10 \quad (\text{Kenaikan minimum } +10\%)$$
   - Transaksi pembatalan (*Cancellation*) dikonstruksi dengan mengirimkan transfer bernilai $0\ Q$ kembali ke alamat pengirim sendiri dengan fee yang lebih tinggi.

---

## 6. Klasifikasi Status Konfirmasi dan Finalitas

Untuk melindungi pengguna dari risiko pembatalan blok (*reorg*), wallet **MUST** membedakan tiga tingkatan status transaksi secara tegas pada antarmuka pengguna:

```text
┌────────────────────────────────────────────────────────────────────────┐
│             STATUS TRANSAKSI RESMI PADA ANTARMUKA WALLET               │
├─────────────┬───────────────────┬──────────────────────────────────────┤
│ STATUS      │ KONDISI PROTOKOL  │ ARTI PADA TAMPILAN PENGGUNA          │
├─────────────┼───────────────────┼──────────────────────────────────────┤
│ PENDING     │ Berada di Mempool │ "Menunggu Validasi Jaringan"         │
│             │ Belum masuk blok  │ Belum dianggap pembayaran sah.       │
├─────────────┼───────────────────┼──────────────────────────────────────┤
│ INCLUDED    │ Termasuk di Blok  │ "Diproses (Menunggu Finalitas BFT)"  │
│             │ CC(B) belum hadir │ Risiko kegagalan sangat rendah, tapi │
│             │                   │ belum mutlak secara hukum kripto.    │
├─────────────┼───────────────────┼──────────────────────────────────────┤
│ FINALIZED   │ Blok memiliki     │ "Selesai & Final Mutlak"             │
│             │ CC(B) kuorum >2/3 │ Transaksi tidak dapat dibatalkan     │
│             │                   │ atau diubah oleh kekuatan apa pun.   │
└─────────────┴───────────────────┴──────────────────────────────────────┘
```

> **Mandat Finalitas Dompet (Wallet Finality Mandate):**  
> Wallet **MUST NOT** menampilkan status transaksi sebagai *"Confirmed / Selesai / Sukses"* sebelum blok tempat transaksi tersebut berada telah memiliki **Commit Certificate (Finalitas Aurion-BFT)** yang terverifikasi.

---

## 7. Model Perhitungan Saldo Pengguna (Balance Presentation)

Wallet **MUST** menyajikan rincian saldo dengan pemisahan transparan:
1. **Confirmed Balance ($\mathcal{B}_{\text{confirmed}}$):** Saldo total yang berada pada state finalitas protocol.
2. **Pending Outgoing ($\mathcal{D}_{\text{pending}}$):** Total debet (amount + fee) dari seluruh transaksi yang dikirim namun masih berstatus `PENDING` atau `INCLUDED`.
3. **Pending Incoming ($\mathcal{C}_{\text{pending}}$):** Total kredit dari transaksi masuk yang belum berstatus `FINALIZED`.
4. **Spendable Balance ($\mathcal{B}_{\text{spendable}}$):**
   $$\mathcal{B}_{\text{spendable}} = \mathcal{B}_{\text{confirmed}} - \mathcal{D}_{\text{pending}}$$
   Wallet **MUST NOT** mengizinkan pembuatan transaksi baru jika `TotalDebit > \mathcal{B}_{\text{spendable}}`.

---

## 8. Akun Pantau (Watch-Only Accounts)

1. Wallet **MAY** mendukung fitur *Watch-Only Account* untuk audit saldo dan riwayat transaksi tanpa memegang kunci privat.
2. Watch-only account **MUST** dikonfigurasi secara eksklusif menggunakan alamat publik kanonikal (`Address`).
3. Pada watch-only account, seluruh fungsi penandatanganan dan pengiriman transaksi **MUST** dinonaktifkan secara permanen di tingkat logika aplikasi.
