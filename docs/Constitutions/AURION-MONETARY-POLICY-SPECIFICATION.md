# AURION MONETARY POLICY SPECIFICATION
## Spesifikasi Kebijakan Moneter dan Konstitusi Matematika Protokol Aurion

> **Dokumen Referensi:** [CONSTITUTION.md](file:///c:/Projects/aurion/CONSTITUTION.md) & [AURION CONSTITUTION.md](file:///c:/Projects/aurion/docs/Constitutions/AURION%20CONSTITUTION.md)  
> **Status:** RATIFIED & LOCKED SPECIFICATION  
> **Klasifikasi:** Protokol Inti Konsensus Layer 1 (Monetary Core)  
> **Versi Protokol:** 1.0.0-BFT  
> **Sifat Ketetapan:** Kriptografis, Deterministik Matematis, Nol-Floating-Point (Zero-Float)

---

## 1. Unit Moneter, Atomisitas, dan Tata Nama Denominasi

### 1.1 Satuan Moneter Utama (Base Currency Unit)
Satuan moneter resmi ekosistem Aurion adalah **AUR**. Seluruh penetapan harga, valuasi makroekonomi, dan antarmuka pengguna luar menggunakan satuan dasar AUR sebagai unit referensi.

### 1.2 Satuan Atomik Moneter (Atomic Unit / Quantum)
Satuan atomik terkecil yang sah di dalam protokol konsensus Aurion disebut **Quantum** (bentuk jamak: **Quanta**).
- Quantum adalah unit fundamental yang bersifat **indivisible** (tidak dapat dibagi lagi).
- Protokol Aurion **tidak mengakui** adanya nilai moneter di bawah 1 Quantum.
- Simbol resmi Quantum dalam notasi matematika protokol adalah $\mathbf{Q}$.

### 1.3 Rasio Konversi dan Presisi Tetap (9 Fixed Decimal Places)
Rasio konversi antara satuan dasar AUR dan unit atomik Quantum ditetapkan secara permanen sebagai berikut:

$$\mathbf{1\ AUR = 1.000.000.000\ Quantum\ (10^9\ Q)}$$

Protokol Aurion mengunci secara mutlak **presisi tetap 9 angka di belakang koma (9 fixed decimal places)**.

### 1.4 Tabel Tata Nama Denominasi (Denomination Tiers)
Untuk memfasilitasi transaksi mikro hingga makro pada seluruh lapisan ekosistem, Aurion menetapkan tangga denominasi terstandarisasi:

| Nama Denominasi | Simbol | Nilai dalam Quantum ($Q$) | Nilai dalam AUR | Pangkat ($10^n$) | Peruntukan Utama |
| :--- | :--- | ---:| ---:| :--- | :--- |
| **Quantum** | $Q$ | $1\ Q$ | $0,000000001\ \text{AUR}$ | $10^0\ Q$ | Unit internal konsensus & fee gas terkecil |
| **Micro-AUR** | $\mu\text{AUR}$ | $1.000\ Q$ | $0,000001000\ \text{AUR}$ | $10^3\ Q$ | Transaksi mikro data & payload fees |
| **Milli-AUR** | $\text{mAUR}$ | $1.000.000\ Q$ | $0,001000000\ \text{AUR}$ | $10^6\ Q$ | Biaya transfer standar & smart contracts |
| **Cent-AUR** | $\text{cAUR}$ | $10.000.000\ Q$ | $0,010000000\ \text{AUR}$ | $10^7\ Q$ | Pembayaran komersial ritel mikro |
| **Deci-AUR** | $\text{dAUR}$ | $100.000.000\ Q$ | $0,100000000\ \text{AUR}$ | $10^8\ Q$ | Satuan intermediate pasar |
| **AUR (Base)** | $\text{AUR}$ | $1.000.000.000\ Q$ | $1,000000000\ \text{AUR}$ | $10^9\ Q$ | Satuan perdagangan & cadangan moneter |
| **Kilo-AUR** | $\text{kAUR}$ | $1.000.000.000.000\ Q$ | $1.000,000000000\ \text{AUR}$ | $10^{12}\ Q$ | Likuiditas validator & staking pool |
| **Mega-AUR** | $\text{MAUR}$ | $1.000.000.000.000.000\ Q$ | $1.000.000,000000000\ \text{AUR}$ | $10^{15}\ Q$ | Alokasi perbendaharaan & master treasury |

---

## 2. Pasokan Dasar Genesis (Blok 0) & Master Treasury

### 2.1 Pencetakan Awal Genesis
Pada Blok 0 (Genesis), protokol mencetak pasokan awal tepat:

$$\mathbf{S_{\text{genesis}} = 66.000.000\ AUR}$$

Dalam satuan internal konsensus Quantum (skala sembilan desimal):

$$\mathbf{S_{\text{genesis}}^{(Q)} = 66.000.000 \times 1.000.000.000 = 66.000.000.000.000.000\ Quantum\ (6,6 \times 10^{16}\ Q)}$$

### 2.2 Alokasi Eksklusif Master Treasury
Seluruh pasokan Blok 0 dialokasikan secara eksplisit dan tunggal ke dalam **Master Treasury Account** yang dideklarasikan pada file `genesis.json`:
- **Alamat Kanonikal:** `aur1jjtqrlqy9suehhltnzt2ml4zwsr8ukpyvvhm2gw899u0e0w22qusq0pjql`
- **Saldo Genesis ($\sigma_0$):** $66.000.000\ \text{AUR}$ ($66.000.000.000.000.000\ Q$)
- **Peruntukan:** Pengelolaan perbendaharaan berdaulat, operasional jaringan, stabilitas cadangan, dan penyediaan infrastruktur ekosistem resmi.

```text
                           BLOK 0 (GENESIS MINTING)
                    66.000.000 AUR (66.000.000.000.000.000 Q)
                                       │
                                       ▼
                       MASTER TREASURY ACCOUNT (100%)
             aur1jjtqrlqy9suehhltnzt2ml4zwsr8ukpyvvhm2gw899u0e0w22qusq0pjql
```

### 2.3 Aturan Moneter Faucet
1. Faucet resmi jaringan Aurion **WAJIB (MUST)** mengambil likuiditas secara eksklusif dari rekening operasional resmi di bawah Master Treasury.
2. Faucet **DILARANG KERAS (MUST NOT)** mencetak uang baru di luar mekanisme penerbitan resmi protokol.

---

## 3. Emisi Blok & Insentif Validator Berkelanjutan ($H > 0$)

### 3.1 Pencetakan Reward Blok Tingkat Protokol
Pada setiap blok baru ($H \ge 1$) yang berhasil memperoleh sertifikat kuorum kriptografis (*Quorum Certificate / QC*) dengan suara $> \frac{2}{3}$ validator aktif dan dituliskan secara permanen ke buku besar, protokol mengeksekusi pencetakan koin baru sejumlah nilai reward tetap $R$:

$$\mathbf{R = 1\ AUR = 1.000.000.000\ Quantum\ (10^9\ Q)}$$

Pencetakan reward blok ini dijalankan langsung oleh mesin eksekusi (`aurion-execution`) tanpa memerlukan transaksi eksternal (*zero external transaction*).

### 3.2 Distribusi Reward Blok
Nilai reward $R$ pada setiap blok didistribusikan secara deterministik:
1. **Porsi Proposer ($20\%$):**
   $$\mathcal{R}_{\text{proposer}} = \left\lfloor \frac{R \times 20}{100} \right\rfloor = 200.000.000\ \text{Quantum}\ (0,2\ \text{AUR})$$
   Dialokasikan kepada validator yang bertindak sebagai Proposer sah penyusun proposal blok tersebut.
2. **Porsi Voter Precommit QC ($80\%$):**
   $$\mathcal{R}_{\text{voters}} = R - \mathcal{R}_{\text{proposer}} = 800.000.000\ \text{Quantum}\ (0,8\ \text{AUR})$$
   Didistribusikan secara proporsional kepada seluruh validator aktif yang menandatangani pesan *Precommit* pada *Quorum Certificate* (QC) blok tersebut:
   $$\mathcal{R}_{\text{voter}}(v) = \left\lfloor \frac{\mathcal{R}_{\text{voters}} \times w_v}{\sum_{u \in \text{QC}} w_u} \right\rfloor$$
   Sisa fraksional dari pembagian floor dialokasikan kepada Proposer untuk menjaga konservasi nilai exact integer.

### 3.3 Biaya Gas Transaksi (Gas Fee Routing)
1. Setiap transaksi dalam blok membayar biaya gas $\text{Fee}(T_x) \ge \text{MinFee}$.
2. Seluruh akumulasi gas fee pada blok $H$, dinotasikan sebagai $\mathcal{F}_{\text{total}}(H) = \sum_{T_x \in B} \text{Fee}(T_x)$, dialirkan **100% ke akun validator pembuat blok (Proposer)** sebagai kompensasi perakitan komputasi:
   $$\mathcal{F}_{\text{proposer}}(H) = \mathcal{F}_{\text{total}}(H)$$

---

## 4. Fungsi Transisi State Moneter & Invarian Konsensus

### 4.1 Definisi Kuantitas Suplai
1. **Pasokan Awal Genesis ($S_{\text{genesis}}$):** Tepat $66.000.000.000.000.000\ Q$.
2. **Total Suplai Diterbitkan ($S_{\text{emitted}}(H)$):** Total seluruh Quantum yang telah dicetak sejak Blok 0 hingga blok $H$:
   $$S_{\text{emitted}}(H) = S_{\text{genesis}}^{(Q)} + (H \times R)$$
3. **Total Saldo Beredar ($S_{\text{circulating}}(H)$):** Total saldo yang tersimpan pada seluruh akun aktif di ledger:
   $$\sum_{\alpha \in \text{Accounts}} \mathcal{A}[\alpha].\text{balance} = S_{\text{emitted}}(H)$$

### 4.2 Invarian Moneter Tertinggi (The Supreme Monetary Invariants)
Setiap simpul penuh (*full node*) Aurion memvalidasi invarian moneter berikut secara atomik:

$$\begin{aligned}
\mathbf{[INV-MON-01]}\quad & S_{\text{genesis}} = 66.000.000.000.000.000\ Q \quad (\text{Locked at Genesis State } \sigma_0) \\
\mathbf{[INV-MON-02]}\quad & \mathcal{A}[\text{MasterTreasury}].\text{balance}_{\sigma_0} = 66.000.000.000.000.000\ Q \\
\mathbf{[INV-MON-03]}\quad & S_{\text{emitted}}(H) = S_{\text{emitted}}(H-1) + R, \quad \forall H \ge 1 \\
\mathbf{[INV-MON-04]}\quad & \Delta \text{LedgerBalances}(H) = R \quad (\text{Konservasi Mutasi State Blok}) \\
\mathbf{[INV-MON-05]}\quad & \text{Fee}(T_x) \text{ terdebet dari pengirim dan terkredit 100\% ke validator} \\
\mathbf{[INV-MON-06]}\quad & \forall \alpha, \quad \mathcal{A}[\alpha].\text{balance} \ge 0 \quad (\text{No Negative Balance, Strict } u128)
\end{aligned}$$

---

## 5. Aturan Aritmetika Integer Murni (Zero-Float Mandate)

1. **Larangan Floating-Point Mutlak:** Tipe data pecahan mengambang (`f32`, `f64`) diharamkan secara mutlak dari lapisan konsensus, STF, perhitungan reward, dan saldo.
2. **Representasi Unsigned Integer 128-bit (`u128`):** Seluruh nilai kuantitas moneter wajib menggunakan tipe primitif `u128`.
3. **Operasi Aritmetika Berpengecekan (*Checked Arithmetic*):** Semua operasi penambahan, pengurangan, dan perkalian wajib memanggil:
   - `checked_add`
   - `checked_sub`
   - `checked_mul`
   - `checked_div`
4. **Pembulatan ke Bawah (*Floor Truncation*):** Operasi pembagian integer selalu menggunakan pembulatan ke bawah. Larangan mutlak pembulatan ke atas (*ceiling*) yang berpotensi memunculkan unit moneter tanpa sandaran.

### 5.1 Aturan Presisi Input/Output Antarmuka
1. **Validasi Input Luar (Ingress):** Saat pengguna memasukkan desimal AUR, konversi ke Quantum dibatasi tepat 9 desimal. Digit ke-10 yang bukan nol wajib memicu error `InvalidPrecisionError`.
2. **Formulasi Konversi Ingress:**
   $$\text{Quantum} = \text{AUR\_Integer} \times 10^9 + \left\lfloor \text{AUR\_Fractional} \times 10^{9 - \text{len}} \right\rfloor$$
3. **Formulasi Konversi Egress (Display):**
   $$\text{Display} = \left\lfloor \frac{\text{Quantum}}{10^9} \right\rfloor \,.\, \left( \text{Quantum} \pmod{10^9} \right)_{[9\text{ digits, zero-padded}]}$$

---

## 6. Rangkuman Konstanta Kode Moneter

```rust
// ==============================================================================
// AURION PROTOCOL CONSTANTS: MONETARY CORE (1.0.0-BFT)
// ==============================================================================

/// Unit atomik per 1 AUR (10^9 Quantum, 9 desimal tetap)
pub const QUANTA_PER_AUR: u128 = 1_000_000_000;

/// Pasokan dasar Genesis dalam satuan AUR (66 Juta AUR)
pub const GENESIS_SUPPLY_AUR: u128 = 66_000_000;

/// Pasokan dasar Genesis dalam satuan Quantum (6,6 x 10^16 Quantum)
pub const GENESIS_SUPPLY_QUANTA: u128 = 66_000_000_000_000_000;

/// Alokasi awal Master Treasury di Blok 0 (100% Pasokan Genesis)
pub const MASTER_TREASURY_ALLOCATION_QUANTA: u128 = 66_000_000_000_000_000;

/// Reward emisi per blok BFT baru (H > 0) dalam satuan Quantum (1 AUR)
pub const BLOCK_REWARD_QUANTA: u128 = 1_000_000_000;

/// Persentase reward untuk Proposer pembuat blok (20%)
pub const PROPOSER_REWARD_PERCENTAGE: u128 = 20;

/// Persentase reward untuk Validator penandatangan Precommit QC (80%)
pub const VOTER_REWARD_PERCENTAGE: u128 = 80;

/// Alokasi fee transaksi ke validator pembuat blok (100%)
pub const FEE_VALIDATOR_PERCENTAGE: u128 = 100;
```
