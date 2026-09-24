# AURION STATE TRANSITION SPECIFICATION
## Spesifikasi Formal Fungsi Transisi State Deterministik Protokol Aurion

> **Dokumen Referensi:**  
> - [AURION CONSTITUTION.md](file:///c:/Projects/aurion/AURION%20CONSTITUTION.md)  
> - [AURION-MONETARY-POLICY-SPECIFICATION.md](file:///c:/Projects/aurion/AURION-MONETARY-POLICY-SPECIFICATION.md)  
> - [AURION-CONSENSUS-SPECIFICATION.md](file:///c:/Projects/aurion/AURION-CONSENSUS-SPECIFICATION.md)  
>
> **Status:** RATIFIED & LOCKED SPECIFICATION  
> **Klasifikasi:** Mesin State Inti Layer 1 (State Core)  
> **Versi:** 1.0.0-PROD  
> **Sifat Ketetapan:** Deterministik Mutlak, Zero-Float, Terotentikasi Merkle

---

## 1. Definisi Matematis Transisi State

Sistem buku besar Aurion dimodelkan sebagai **Mesin Replikasi State Deterministik (*Deterministic State-Machine*)**. 

Perubahan buku besar dari keadaan lama ke keadaan baru dikendalikan secara eksklusif oleh fungsi matematika tunggal: **State Transition Function ($\text{STF}$)**.

```text
       ┌─────────────────────────────────────────────────────────┐
       │                State Sebelumnya (σ_n)                   │
       └────────────────────────────┬────────────────────────────┘
                                    │
                                    ▼
       ┌─────────────────────────────────────────────────────────┐
       │               Blok Terverifikasi (B_{n+1})              │
       └────────────────────────────┬────────────────────────────┘
                                    │
                                    ▼
       ┌─────────────────────────────────────────────────────────┐
       │       STATE TRANSITION FUNCTION: STF(σ_n, B_{n+1})      │
       │                                                         │
       │  1. Validasi Pra-Eksekusi Header & Konsensus            │
       │  2. Eksekusi Sekuensial Transaksi & Konservasi Nilai    │
       │  3. Pemotongan & Alokasi Biaya (100% Validator Fee)     │
       │  4. Penerbitan Subsidi Blok R & Pembagian QC            │
       │  5. Pembaruan State Validator & Transisi Epoch          │
       │  6. Komputasi Root Pohon State Baru (StateRoot_{n+1})   │
       └────────────────────────────┬────────────────────────────┘
                                    │
                                    ▼
       ┌─────────────────────────────────────────────────────────┐
       │                 State Baru (σ_{n+1})                    │
       │               (Identik di Seluruh Simpul)               │
       └─────────────────────────────────────────────────────────┘
```

### 1.1 Formulasi Formal
Diberikan:
- $\Sigma$ adalah ruang seluruh keadaan state yang valid;
- $\mathcal{B}$ adalah ruang seluruh blok kandidat;
- $\bot$ adalah simbol kesalahan terminal (*Integrity Fault / Invalid State Transition*).

Fungsi transisi state didefinisikan sebagai:

$$\text{STF}: \Sigma \times \mathcal{B} \longrightarrow \Sigma \cup \{\bot\}$$

$$\sigma_{n+1} = \text{STF}(\sigma_n, B_{n+1})$$

### 1.2 Aksioma Determinisme Mutlak (Absolute Determinism Axiom)
Untuk setiap state awal $\sigma_n \in \Sigma$ dan blok yang sama $B_{n+1} \in \mathcal{B}$, fungsi $\text{STF}$ menjamin:

$$\forall \text{Node}_A, \text{Node}_B: \quad \text{STF}_A(\sigma_n, B_{n+1}) \equiv \text{STF}_B(\sigma_n, B_{n+1}) = \sigma_{n+1}$$

Tidak ada faktor lingkungan (arsitektur CPU x86/ARM, sistem operasi, urutan alokasi memori, atau clock lokal) yang diizinkan memengaruhi hasil evaluasi $\sigma_{n+1}$.

---

## 2. Model State Aurion: Account-Based State dengan Pohon Terotentikasi

Aurion mengadopsi **Model Akun Terotentikasi (*Authenticated Account State Model*)** yang dipadukan dengan struktur **Sparse Merkle Tree (SMT)** 256-bit berbasis Blake3.

State global $\sigma$ terdiri dari lima domain terisolasi:

$$\sigma = \Big\langle \mathcal{A},\; \mathcal{V},\; \mathcal{M},\; \mathcal{G},\; \mathcal{P} \Big\rangle$$

```text
                              STATE GLOBAL (σ)
                                     │
      ┌──────────────┬───────────────┼───────────────┬──────────────┐
      ▼              ▼               ▼               ▼              ▼
   ACCOUNT       VALIDATOR        MONETARY       GOVERNANCE     PROVENANCE
    STATE          STATE           STATE           STATE         REGISTRY
   (A[addr])        (V)             (M)             (G)            (P)
      │              │               │               │              │
      ├─ Balance     ├─ Active Set   ├─ Emitted Q    ├─ Proposals   └─ RPI Tree
      ├─ Nonce       ├─ Bonded Stake ├─ Circulating  ├─ Votes       (Issuance
      └─ Code/Data   ├─ Unbonding    ├─ Burned Fee   └─ Params       Lineage)
                     └─ Slashed      └─ Maturity Q
```

### 2.1 Domain State Akun ($\mathcal{A}$)
Setiap alamat akun $\alpha \in \{0, 1\}^{256}$ memetakan ke objek akun:

$$\mathcal{A}[\alpha] = \big\langle \text{Balance},\; \text{Nonce},\; \text{PayloadRoot} \big\rangle$$

1. **Balance ($\text{Quantum}$):** Saldo moneter aktif dalam satuan integer integer Quantum ($u128$). Wajib mematuhi $\text{Balance} \le S_{\max}^{(Q)}$.
2. **Nonce ($u64$):** Penghitung transaksi sekuensial akun. Dimulai dari $0$, bertambah tepat $+1$ untuk setiap transaksi sukses.
3. **PayloadRoot ($\text{Hash256}$):** Hash komitmen Blake3 atas data aplikasi/smart script yang terasosiasi dengan akun (bernilai nol untuk akun transfer standar).

### 2.2 Domain State Validator dan Staking ($\mathcal{V}$)
$$\mathcal{V} = \big\langle \mathcal{V}_{\text{active}},\; \mathcal{V}_{\text{pending}},\; \mathcal{U}_{\text{queue}},\; \mathcal{T}_{\text{tombstone}} \big\rangle$$
- $\mathcal{V}_{\text{active}}$: Himpunan validator aktif pada epoch berjalan beserta bobot voting $w_i$.
- $\mathcal{V}_{\text{pending}}$: Antrean registrasi validator yang menunggu aktivasi di $E+2$.
- $\mathcal{U}_{\text{queue}}$: Antrean unbonding agunan yang dikunci selama $\tau_{\text{unbond}} = 14\ \text{Epoch}$.
- $\mathcal{T}_{\text{tombstone}}$: Daftar hitam permanen validator yang telah dieksekusi pemusnahan (*slashed*).

### 2.3 Domain State Moneter Konsensus ($\mathcal{M}$)
Menyimpan state konservasi pasokan moneter sesuai [AURION-MONETARY-POLICY-SPECIFICATION.md](file:///c:/Projects/aurion/AURION-MONETARY-POLICY-SPECIFICATION.md):
- $S_{\text{emitted}}$: Total Quantum yang pernah dicetak sejak genesis.
- $S_{\text{circulating}}$: Total Quantum aktif dalam state akun saat ini.
- $S_{\text{burned}}$: Total kumulatif Quantum yang dimusnahkan via pembakaran fee dan pemusnahan agunan.
- $\mathcal{Q}_{\text{maturity}}$: Antrean kematangan coinbase 100 blok.

### 2.4 Domain Pendaftaran Asal-Usul Aset ($\mathcal{P}$)
Menyimpan pohon penanda silsilah penerbitan (*Provenance Registry*):
- Memetakan setiap tinggi blok $H$ ke *Reward Provenance Identifier* (RPI) resmi yang diterbitkan oleh modul protokol.

---

## 3. Tahapan Eksekusi Fungsi Transisi State (STF Pipeline)

Ketika sebuah blok $B$ pada tinggi $H$ dievaluasi terhadap state $\sigma_{H-1}$, fungsi $\text{STF}$ menjalankan 7 tahapan atomik:

```text
[TAHAP 1] Validasi Header & Verifikasi Kriptografis
    ↓
[TAHAP 2] Inisialisasi Alokasi Blok & Pool Biaya Transaksi
    ↓
[TAHAP 3] Eksekusi Sekuensial Seluruh Transaksi Tx_1 ... Tx_m
    ↓
[TAHAP 4] Alokasi Biaya Transaksi 100% ke Validator Proposer
    ↓
[TAHAP 5] Penerbitan Subsidi Blok R (20% Proposer, 80% Precommit Voters QC)
    ↓
[TAHAP 6] Pemrosesan Batas Epoch (Jika H mod L_epoch == 0)
    ↓
[TAHAP 7] Rekalkulasi StateRoot & Asersi Invarian Konsensus
```

---

### Tahap 1: Validasi Header & Verifikasi Kriptografis
1. **Pemeriksaan Induk:**
   $$B.\text{prev\_hash} == \text{Hash}(B_{H-1})$$
2. **Pemeriksaan Ketinggian Blok:**
   $$B.\text{height} == B_{H-1}.\text{height} + 1$$
3. **Pemeriksaan Timestamp MTP:**
   $$\text{MTP}(B_{H-11} \dots B_{H-1}) < B.\text{timestamp} \le \text{LocalClock} + \Delta_{\text{drift}}$$
4. **Pemeriksaan Pengusul Sah:**
   $$B.\text{proposer} == \text{Proposer}(H, B.\text{round})$$
5. **Verifikasi Tanda Tangan Konsensus:**
   Blok memuat bukti Commit Certificate $\mathcal{CC}(B_{H-1})$ yang valid dengan kuorum $\ge \mathcal{Q}$.

Jika salah satu syarat gagal, $\text{STF}$ mengembalikan $\bot$.

### Tahap 2: Inisialisasi Pool Blok
State sementara (*ephemeral execution context*) $\sigma'$ disalin dari $\sigma_{H-1}$.
- Inisialisasi pool biaya blok: $\mathcal{F}_{\text{total}} \leftarrow 0\ Q$.

### Tahap 3: Eksekusi Transaksi Tunggal ($\text{ApplyTx}$)
Untuk setiap transaksi $T_x \in B.\text{transactions}$ (diurutkan secara deterministik):

$$\sigma' \leftarrow \text{ApplyTx}(\sigma', T_x, H)$$

Sub-algoritma $\text{ApplyTx}(\sigma', T_x, H)$ mengeksekusi operasi berikut:

1. **Verifikasi Nonce:**
   $$\text{Assert}\big( T_x.\text{nonce} == \mathcal{A}[\text{sender}].\text{nonce} \big)$$
2. **Kalkulasi Beban Dana:**
   $$\text{TotalDebit} = T_x.\text{amount}.\text{checked\_add}(T_x.\text{fee}) \implies \text{Jika overflow } \to \bot$$
3. **Pemeriksaan Kecukupan Saldo:**
   $$\text{Assert}\big( \mathcal{A}[\text{sender}].\text{balance} \ge \text{TotalDebit} \big)$$
4. **Debet Saldo Pengirim:**
   $$\mathcal{A}[\text{sender}].\text{balance} \leftarrow \mathcal{A}[\text{sender}].\text{balance} - \text{TotalDebit}$$
   $$\mathcal{A}[\text{sender}].\text{nonce} \leftarrow \mathcal{A}[\text{sender}].\text{nonce} + 1$$
5. **Kredit Saldo Penerima:**
   $$\mathcal{A}[\text{recipient}].\text{balance} \leftarrow \mathcal{A}[\text{recipient}].\text{balance}.\text{checked\_add}(T_x.\text{amount})$$
6. **Akumulasi Pool Biaya Blok:**
   $$\mathcal{F}_{\text{total}} \leftarrow \mathcal{F}_{\text{total}}.\text{checked\_add}(T_x.\text{fee})$$

### Tahap 4: Pengaliran Biaya Transaksi (100% ke Validator Proposer)
Mengikuti mandat [CONSTITUTION.md](file:///c:/Projects/aurion/CONSTITUTION.md) dan [AURION-MONETARY-POLICY-SPECIFICATION.md](file:///c:/Projects/aurion/docs/Constitutions/AURION-MONETARY-POLICY-SPECIFICATION.md):
1. **Pengaliran Penuh ke Proposer (100%):**
   Seluruh biaya transaksi yang terkumpul dalam blok dialirkan langsung ke akun validator pembuat proposal blok:
   $$\mathcal{A}[B.\text{proposer}].\text{balance} \leftarrow \mathcal{A}[B.\text{proposer}].\text{balance}.\text{checked\_add}(\mathcal{F}_{\text{total}})$$
2. **Ketiadaan Pembakaran Rutin:** Seluruh gas fee dialokasikan untuk memberi kompensasi perakitan komputasi validator.

### Tahap 5: Penerbitan Reward Blok Protokol & Distribusi QC
Pencetakan reward blok tetap $R = 1\text{ AUR} = 1.000.000.000\text{ Quantum}\ (10^9\ Q)$ dieksekusi langsung oleh mesin protokol (`aurion-execution`) tanpa memerlukan transaksi eksternal (*zero external transaction*):
1. **Alokasi Porsi Proposer (20%):**
   $$\mathcal{R}_{\text{proposer}} = 200.000.000\ \text{Quantum}\ (0,2\ \text{AUR})$$
   $$\mathcal{A}[B.\text{proposer}].\text{balance} \leftarrow \mathcal{A}[B.\text{proposer}].\text{balance}.\text{checked\_add}(\mathcal{R}_{\text{proposer}})$$
2. **Alokasi Porsi Voter Precommit QC (80%):**
   $$\mathcal{R}_{\text{voters}} = 800.000.000\ \text{Quantum}\ (0,8\ \text{AUR})$$
   Didistribusikan secara proporsional kepada seluruh validator penandatangan Precommit pada sertifikat kuorum (QC):
   $$\forall v \in \text{QC}, \quad \mathcal{A}[v].\text{balance} \leftarrow \mathcal{A}[v].\text{balance}.\text{checked\_add}\left( \left\lfloor \frac{\mathcal{R}_{\text{voters}} \times w_v}{\sum_{u \in \text{QC}} w_u} \right\rfloor \right)$$
3. **Pembaruan Suplai Moneter Kumulatif:**
   $$\mathcal{M}.S_{\text{emitted}} \leftarrow \mathcal{M}.S_{\text{emitted}}.\text{checked\_add}(R)$$
   $$\mathcal{M}.S_{\text{circulating}} \leftarrow \mathcal{M}.S_{\text{circulating}}.\text{checked\_add}(R)$$
4. **Pencatatan RPI Modul Asal-Usul Aset:**
   $$\text{RPI} = \text{Blake3}\Big( \text{"AURION-PROVENANCE-RPI-V1"} \parallel B.\text{hash} \parallel H \parallel R \Big)$$
   $$\mathcal{P}.\text{Insert}(H, \text{RPI})$$

### Tahap 6: Transisi Batas Epoch (Epoch Boundary Transition)
Jika $H \pmod{10.000} == 0$:
1. **Pencairan Unbonding:**
   Untuk setiap agunan dalam $\mathcal{V}.\mathcal{U}_{\text{queue}}$ yang telah mencapai masa tunggu $\tau_{\text{unbond}} = 14\ \text{Epoch}$, kembalikan saldo agunan ke akun validator.
2. **Pemberlakuan Validator Baru:**
   Promosikan registrasi validator yang diajukan pada epoch $E-2$ menjadi anggota aktif $\mathcal{V}_{\text{active}}$ untuk epoch $E+1$.

### Tahap 7: Komputasi Root Pohon State & Asersi Invarian
1. **Komputasi Root Pohon State:**
   Hitung ulang root Merkle dari seluruh pohon akun, validator, moneter, dan provenance:
   $$\text{ComputedStateRoot} = \text{MerkleRoot}(\sigma')$$
2. **Pemeriksaan Kecocokan Header:**
   $$\text{Assert}\big( B.\text{state\_root} == \text{ComputedStateRoot} \big)$$
3. **Verifikasi Invarian Moneter Tertinggi:**
   Validasi pemenuhan formula `[INV-MON-01]` hingga `[INV-MON-06]`:
   $$\sum_{\alpha} \mathcal{A}[\alpha].\text{balance} == \mathcal{M}.S_{\text{circulating}}$$

Jika seluruh asersi terpenuhi, kembalikan state baru $\sigma_{H} = \sigma'$.

---

## 4. Penanganan Transisi State Tidak Sah (Invalid State Handling)

Jika terjadi pelanggaran selama eksekusi $\text{STF}$ (misalnya saldo tidak mencukupi, nonce tidak sesuai, hash state root tidak cocok, atau invarian suplai terlanggar):

1. **Isolasi Kegagalan (Atomic Rollback):** State sementara $\sigma'$ dibatalkan secara instan. State database tidak mengalami mutasi sedikit pun dan tetap pada $\sigma_{H-1}$.
2. **Penolakan Blok (*Block Rejection*):** Blok $B$ ditolak dari rantai kanonikal lokal.
3. **Penghentian Kegagalan Integritas (*Integrity Fail-Stop*):** Jika ketidakcocokan terjadi pada level komputasi *StateRoot* dari blok yang telah ditandatangani oleh Commit Certificate kuorum, simpul segera mengaktifkan status terkunci aman (*Terminal Fail-Stop Lockout*) untuk mencegah penyebaran state korup.

---

## 5. Teorema Integritas Konservasi State

> ### Teorema Konservasi Moneter STF
> Di bawah evaluasi fungsi $\text{STF}(\sigma_{H-1}, B_H)$, jumlah total seluruh aset Quantum di seluruh dunia Aurion selalu memenuhi hukum konservasi tertutup:
> 
> $$\Delta S_{\text{circulating}} = R = 1.000.000.000\ \text{Quantum}\ (1\ \text{AUR})$$
> 
> Tidak ada proses komputasi transaksi, transfer saldo, pemanggilan kontrak, atau penyesuaian staking yang dapat menciptakan atau memusnahkan 1 Quantum pun di luar hukum di atas.
