# 08 — AURION VALIDATOR & STAKING SPECIFICATION
## Spesifikasi Formal Siklus Hidup Validator, Mekanisme Agunan (Staking), Delegasi, dan Penegakan Sanksi

> **Hierarki Dokumen:**  
> `00 — AURION CONSTITUTION` $\longrightarrow$ `01 — MONETARY` $\longrightarrow$ `02 — CONSENSUS` $\longrightarrow$ `03 — STATE TRANSITION` $\longrightarrow$ `04 — TRANSACTION` $\longrightarrow$ `05 — CRYPTOGRAPHY` $\longrightarrow$ `06 — SERIALIZATION & WIRE` $\longrightarrow$ `07 — GENESIS` $\longrightarrow$ **`08 — VALIDATOR & STAKING SPECIFICATION`**  
>
> **Status:** AUDITED & LOCKED SPECIFICATION  
> **Klasifikasi:** Protokol Tata Kelola Operator Konsensus Layer 1 (Staking & Validator Engine)  
> **Versi:** 1.0.0-PROD  
> **Sifat Ketetapan:** Kriptografis, Anti-Sentralisasi, Zero-Float, Eksekusi Penalti Tanpa Kompromi

---

## 1. Arsitektur Agunan dan Model Hak Suara (Staking & Voting Power Model)

Protokol Aurion menggunakan agunan koin kedaulatan AUR (dalam satuan integer Quantum) sebagai jangkar keamanan ekonomi (*economic security bond*) untuk konsensus Aurion-BFT.

```text
┌────────────────────────────────────────────────────────────────────────┐
│                   ARSITEKTUR AKUMULASI BOBOT SUARA                     │
│                                                                        │
│   Agunan Mandiri Validator (Self-Bond ≥ 10.000 AUR)                    │
│                        │                                               │
│                        ▼ (+)                                           │
│   Agunan Delegasi Komunitas (Delegated Stake)                          │
│                        │                                               │
│                        ▼ (=)                                           │
│   Total Agunan Terikat (Bonded Quanta: B_v)                            │
│                        │                                               │
│                        ▼ Integer Division (B_v / 10^9)                 │
│   Bobot Suara Mentah (Raw Voting Power)                                │
│                        │                                               │
│                        ▼ Plafon Batas Maksimum (Cap: Min(W, 10% W_E))  │
│   BOBOT SUARA KONSENSUS EFEKTIF (w_v)                                  │
└────────────────────────────────────────────────────────────────────────┘
```

### 1.1 Persyaratan Agunan Mandiri Minimum (Self-Bond Minimum)
Untuk mencegah serangan *Sybil Attack* dan memastikan operator simpul memiliki kepentingan finansial langsung terhadap kelangsungan jaringan (*skin in the game*):

$$\text{MinSelfBond} = 10.000\ \text{AUR} = 10.000.000.000.000\ \text{Quantum}\ (10^{13}\ Q)$$

Validator yang saldo agunan mandirinya turun di bawah $\text{MinSelfBond}$ akan dinonaktifkan secara otomatis dari proses konsensus pada batas epoch berikutnya.

### 1.2 Formulasi Perhitungan Bobot Suara (Voting Power Formula)
Bobot voting validator $v$, dinotasikan sebagai $w_v$, dihitung secara deterministik menggunakan pembagian bilangan bulat:

$$W_{\text{raw}}(v) = \left\lfloor \frac{\text{SelfBond}(v) + \sum_{d \in \text{Delegators}} \text{DelegatedBond}(d, v)}{10^9} \right\rfloor$$

### 1.3 Plafon Batas Anti-Sentralisasi (Voting Power Cap)
Untuk mencegah dominasi oligarki dan menjaga desentralisasi jaringan:
- Tidak ada validator tunggal yang diizinkan memegang bobot suara konsensus melebihi **10% dari total bobot suara aktif**:
  $$w_v = \min\left( W_{\text{raw}}(v),\; \left\lfloor \frac{W_E \times 10}{100} \right\rfloor \right)$$
- Kelebihan agunan di atas plafon 10% tetap berhak menerima bagi hasil reward, namun tidak menambah kekuatan hak suara voting BFT.

---

## 2. Mesin State Siklus Hidup Validator (Validator Lifecycle State Machine)

Setiap entitas validator berada pada salah satu dari lima status resmi:

```text
               ┌────────────────┐
               │  UNREGISTERED  │
               └───────┬────────┘
                       │ MsgRegisterValidator (Self-Bond ≥ 10.000 AUR)
                       ▼
               ┌────────────────┐
               │   CANDIDATE    │ ◄──────────────────────────┐
               └───────┬────────┘                            │
                       │ Pipeline Delay (E → E+2)            │ MsgUnjail
                       ▼                                     │ (Pasca Denda)
               ┌────────────────┐  Inactivity Fault          │
               │     ACTIVE     ├────────────────────►┌──────┴─────────┐
               └───────┬────────┘                     │     JAILED     │
                       │                              └──────┬─────────┘
                       │ MsgUnbond / Sukarela                │
                       ▼                                     │
               ┌────────────────┐                            │
               │   UNBONDING    │                            │
               └───────┬────────┘                            │
                       │ Bukti Ekuivokasi (Fraud Proof)      │ Bukti Ekuivokasi
                       └──────────────────┬──────────────────┘
                                          │
                                          ▼
                               ┌─────────────────────┐
                               │     TOMBSTONED      │
                               │ (Pemusnahan Abadi)  │
                               └─────────────────────┘
```

### 2.1 Definisi Status Validator:
1. **UNREGISTERED:** Akun reguler yang belum mendaftar sebagai validator.
2. **CANDIDATE:** Akun telah menyerahkan agunan mandiri sah dan menunggu aktivasi antrean epoch ($E \to E+2$).
3. **ACTIVE:** Validator berhak mengusulkan blok (*Proposer*) dan memberikan suara Pre-vote/Pre-commit dalam konsensus Aurion-BFT.
4. **JAILED:** Validator dinonaktifkan sementara akibat kegagalan keaktifan (*liveness fault*). Tidak berhak mengusulkan blok atau memberi suara.
5. **TOMBSTONED:** Validator dimusnahkan secara permanen akibat pengkhianatan konsensus (*equivocation*). Kunci publik diblokir selamanya dari jaringan.

---

## 3. Mekanisme Delegasi dan Akuntansi Komisi (Delegation & Rewards)

### 3.1 Hak dan Batasan Delegasi
1. Setiap pemilik koin AUR berhak mendelegasikan koinnya kepada validator aktif tanpa menyerahkan hak kustodi kunci privat (*non-custodial delegation*).
2. Koin yang didelegasikan dikunci di dalam state staking protokol dan tidak dapat ditransfer sebelum melewati masa unbonding.

### 3.2 Struktur Komisi Validator (Commission Rate)
Validator berhak memungut komisi dari reward blok yang diperoleh sebelum dibagikan kepada para delegator:
- Komisi dinyatakan dalam **Basis Points (bps)** di mana $1\ \text{bps} = 0,01\%$.
- Rentang komisi yang diizinkan protokol:
  $$0\ \text{bps}\ (0\%) \le \text{CommissionRate} \le 2.000\ \text{bps}\ (20\%)$$
- Penyesuaian komisi dibatasi maksimum pergeseran $100\ \text{bps}$ ($1\%$) per epoch untuk melindungi stabilitas delegator.

### 3.3 Formulasi Pembagian Hadiah Blok Bebas Floating-Point
Setiap blok yang final menghasilkan total imbalan validator pengusul $\mathcal{R}_{\text{total}} = \mathcal{R}_{\text{proposer}}(H) + \mathcal{F}_{\text{validator}}(H)$, di mana $\mathcal{R}_{\text{proposer}}(H) = \lfloor (R \times 20) / 100 \rfloor$ dari subsidi blok $R = 1\text{ AUR}$, dan $\mathcal{F}_{\text{validator}}(H) = \mathcal{F}_{\text{total}}$ (100% total biaya transaksi blok dialirkan ke validator pengusul).

Jika blok diusulkan oleh validator $v$ yang memiliki kumpulan agunan $B_v = \text{SelfBond} + \text{DelegatedStake}$:

1. **Pemotongan Komisi Validator:**
   $$\text{CommissionQuanta} = \left\lfloor \frac{\mathcal{R}_{\text{total}} \times \text{CommissionRate}}{10.000} \right\rfloor$$
2. **Pool Hadiah Bersih Delegator:**
   $$\mathcal{R}_{\text{pool}} = \mathcal{R}_{\text{total}} - \text{CommissionQuanta}$$
3. **Distribusi per Delegator ($d$):**
   $$\text{Reward}(d) = \left\lfloor \frac{\mathcal{R}_{\text{pool}} \times \text{Bond}(d)}{B_v} \right\rfloor$$
4. **Konservasi Sisa Pembagian (Dust Conservation):**
   Sisa pembagian fraksional integer dialokasikan kembali ke agunan mandiri validator pengusul:
   $$\text{Remainder} = \mathcal{R}_{\text{pool}} - \sum_{d} \text{Reward}(d) \implies \text{Dikreditkan ke Validator}$$
   Hal ini menjamin konservasi nilai mutlak tanpa pembulatan mengambang.

---

## 4. Pipeline Pelepasan Agunan (Unbonding Pipeline)

Untuk melindungi jaringan dari serangan reorganisasi jarak jauh (*Long-Range Attacks*), protokol memberlakukan periode unbonding yang ketat:

### 4.1 Parameter Unbonding
$$\tau_{\text{unbond}} = 14\ \text{Epoch} = 140.000\ \text{blok}\ (\approx 97,2\ \text{hari pada interval 60 detik})$$

### 4.2 Status Koin Selama Masa Unbonding
Ketika validator atau delegator memicu transaksi `MsgUnbond`:
1. Koin seketika dikeluarkan dari penghitungan bobot suara konsensus;
2. Koin berhenti menghasilkan hadiah blok;
3. Koin dimasukkan ke dalam antrean $\mathcal{U}_{\text{queue}}$ pada state staking;
4. **Kewajiban Sanksi Tetap Berlaku:** Jika terbukti bahwa validator melakukan kecurangan pada blok-blok di mana koin tersebut masih aktif, dana di dalam antrean unbonding tetap disita (*slashed*).

---

## 5. Klasifikasi Pelanggaran dan Matriks Penegakan Hukum (Slashing & Penalties)

Aurion membedakan secara tegas antara **Kelalaian Operasional (*Liveness Fault*)** dan **Pengkhianatan Konsensus Kriptografis (*Byzantine Fault*)**:

```text
┌────────────────────────────────────────────────────────────────────────┐
│                   MATRIKS PENEGAKAN SANKSI PROTOKOL                    │
├──────────────────────────┬────────────────────┬────────────────────────┤
│ KATEGORI PELANGGARAN     │ AKSI PROTOKOL      │ DAMPAK FINANSIAL       │
├──────────────────────────┼────────────────────┼────────────────────────┤
│ Downtime / Liveness      │ Jailed (1 Epoch)   │ Denda 0,1% Agunan      │
├──────────────────────────┼────────────────────┼────────────────────────┤
│ Double Proposing         │ Tombstoned (Abadi) │ 100% Agunan Dimusnahkan│
├──────────────────────────┼────────────────────┼────────────────────────┤
│ Double Voting            │ Tombstoned (Abadi) │ 100% Agunan Dimusnahkan│
├──────────────────────────┼────────────────────┼────────────────────────┤
│ Surround Voting          │ Tombstoned (Abadi) │ 100% Agunan Dimusnahkan│
└──────────────────────────┴────────────────────┴────────────────────────┘
```

### 5.1 Pelanggaran Keaktifan (Liveness Fault / Downtime)
- **Kriteria:** Validator gagal menandatangani sekurang-kurangnya $500\ \text{blok}$ dalam jendela geser $10.000\ \text{blok}$ terakhir.
- **Konsekuensi:**
  1. Validator seketika dimasukkan ke status **JAILED**;
  2. Dikenakan denda operasional sebesar $0,1\%$ ($10\ \text{bps}$) dari total agunan;
  3. Dana denda dibakar (*burned*) secara permanen;
  4. Validator hanya dapat aktif kembali setelah minimal $1\ \text{Epoch}$ dengan mengirimkan transaksi `MsgUnjail` yang sah.

### 5.2 Pengkhianatan Konsensus (Byzantine Equivocation Fault)
- **Kriteria:** Penyerahan bukti otentik pengusulan ganda (*Double Proposal*) atau pemungutan suara ganda (*Double Voting* / *Surround Voting*).
- **Konsekuensi Eksekusi:**
  1. **Penyitaan 100% Agunan:** Seluruh agunan mandiri validator disita seketika. Agunan delegator yang mempercayakan koinnya pada validator jahat turut dipotong $50\%$ sebagai penegakan kehati-hatian delegasi (*delegation risk enforcement*).
  2. **Pembakaran Mayoritas (95% Burned):** Sebesar $95\%$ dana yang disita dibakar permanen dari state, memicu deflasi masif bagi pemegang koin jujur.
  3. **Hadiah Pelapor (5% Bounty):** Sebesar $5\%$ dana sitaan diserahkan kepada simpul yang menyiarkan bukti kecurangan (`MsgSubmitEvidence`).
  4. **Tombstoning Permanen:** Kunci publik validator dimasukkan ke dalam daftar `Tombstone` selamanya.

---

## 6. Penyerahan dan Verifikasi Bukti Kecurangan (Evidence Processing)

Pesan bukti kecurangan (*Fraud Proof*) ditransmisikan melalui transaksi khusus `MsgSubmitEvidence`.

Struktur data bukti:
```text
EvidencePayload
├── validator_pubkey  : [u8; 32]
├── height            : u64
├── round             : u64
├── phase             : u8
├── vote_a            : CanonicalVote
└── vote_b            : CanonicalVote
```

### Aturan Verifikasi Bukti oleh Simpul:
1. `vote_a` dan `vote_b` ditandatangani oleh `validator_pubkey` yang sama;
2. `height`, `round`, dan `phase` pada kedua suara identik;
3. `block_hash_a != block_hash_b`;
4. Bukti diserahkan dalam jendela waktu sah (tidak lebih tua dari $\tau_{\text{unbond}}$);
5. Jika kelima syarat terpenuhi, sanksi pemusnahan dieksekusi seketika di dalam blok tempat transaksi bukti dimasukkan.

---

## 7. Transisi Himpunan Validator Antar-Epoch (Epoch Boundary Set Transition)

Tepat pada blok akhir epoch ($H \pmod{10.000} == 0$), protokol mengeksekusi algoritma pembaruan himpunan validator:

```text
[Batas Akhir Epoch H]
        │
        ▼
  1. Keluarkan validator yang berstatus JAILED atau TOMBSTONED
  2. Cairkan unbonding yang telah mencapai masa tunggu 14 Epoch
  3. Masukkan kandidat baru yang mendaftar pada Epoch E-2
  4. Hitung ulang bobot voting w_v untuk setiap validator aktif
  5. Terapkan batas plafon 10% voting power cap
  6. Hitung total bobot baru W_{E+1} = Σ(w_v)
  7. Kunci Kuorum Baru: Q_{E+1} = ⌊(2 * W_{E+1}) / 3⌋ + 1
        │
        ▼
[Himpunan V_{E+1} Resmi Bertugas untuk Blok H+1 s/d H+10.000]
```

Transisi ini bersifat atomik, deterministik, dan bebas dari interpretasi subjektif node.
