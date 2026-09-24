# AURION CONSENSUS SPECIFICATION
## Konstitusi dan Spesifikasi Formal Konsensus Aurion-BFT: Dari Blok Hingga Finalitas Deterministik

> **Dokumen Referensi:**  
> - [AURION CONSTITUTION.md](file:///c:/Projects/aurion/AURION%20CONSTITUTION.md)  
> - [AURION-MONETARY-POLICY-SPECIFICATION.md](file:///c:/Projects/aurion/AURION-MONETARY-POLICY-SPECIFICATION.md)  
>
> **Status:** RATIFIED & LOCKED SPECIFICATION  
> **Klasifikasi:** Protokol Konsensus Inti Layer 1 (Consensus Core)  
> **Versi:** 1.0.0-PROD  
> **Paradigma:** Accountable Byzantine Fault Tolerance (BFT) dengan Finalitas Instan Deterministik (*Zero-Probabilistic Reorganization*)

---

## 1. Model Konsensus dan Asumsi Byzantine

### 1.1 Paradigma Konsensus: Aurion-BFT
Aurion mengadopsi protokol konsensus **Aurion-BFT**, sebuah mesin replikasi state deterministik (*deterministic state-machine replication*) dengan finalitas absolut berbasis dua fase pemungutan suara (*two-phase voting*).

Berbeda dari konsensus probabilistik bergaya Nakamoto (yang bergantung pada kedalaman konfirmasi dan rentan terhadap reorganisasi rantai), Aurion-BFT menjamin bahwa **setiap blok yang telah memperoleh komitmen kuorum bersifat final secara permanen, mutlak, dan mustahil dibatalkan (*zero reorganization probability*)**.

### 1.2 Model Sinkronisasi Jaringan (Network Synchrony)
Aurion-BFT beroperasi di bawah model **Sinkronisasi Parsial (*Partial Synchrony*)** Dwork-Lynch-Stockmeyer (DLS):
1. **Keamanan Mutlak (*Safety Under Asynchrony*):** Protokol menjamin keamanan state dan pencegahan percabangan (*fork*) dalam segala kondisi keterlambatan jaringan (*arbitrary message delay*). Tidak ada dua blok sah yang dapat difinalisasi pada tinggi yang sama, bahkan jika jaringan mengalami asinkronisitas total atau serangan pemisahan jaringan (*network partition*).
2. **Ketersediaan Pasca-Stabilisasi (*Liveness After GST*):** Setelah batas waktu *Global Stabilization Time* (GST) tercapai—di mana pesan antar-simpul jujur tiba dalam batas waktu tertentu $\Delta$—protokol dijamin terus menghasilkan blok baru secara teratur.

### 1.3 Asumsi Kegagalan Byzantine (Byzantine Bound)
Misalkan $W_E$ adalah total bobot suara (*voting power*) sah dari himpunan validator pada era/epoch aktif $E$:
- Protokol mentoleransi hingga $f$ bobot suara yang bertindak jahat, korup, offline, atau mengalami anomali (*Byzantine faults*):
  $$f < \frac{W_E}{3} \implies W_E \ge 3f + 1$$
- **Ambang Batas Kuorum ($\mathcal{Q}$):** Keputusan konsensus (baik pada fase Pre-vote maupun Pre-commit) memerlukan persetujuan suara sah dengan bobot lebih dari dua pertiga:
  $$\mathcal{Q} = \left\lfloor \frac{2 W_E}{3} \right\rfloor + 1 \quad \left(> \frac{2}{3} W_E\right)$$
- **Pertanggungjawaban Kriptografis (*Cryptographic Accountability*):** Jika terjadi kolusi yang melibatkan lebih dari $\frac{1}{3} W_E$ untuk memaksakan percabangan ganda, protokol menyediakan bukti matematis yang tidak dapat disangkal (*undeniable cryptographic proof*) untuk mengidentifikasi dan memusnahkan (*slash*) seluruh pelaku.

---

## 2. Himpunan Validator, Epoch, dan Tata Kelola Peran

```text
┌────────────────────────────────────────────────────────────────────────┐
│                        EPOCH E (Panjang = L Blok)                      │
│                                                                        │
│   Tinggi H, Putaran R=0       Tinggi H, Putaran R=1      Tinggi H+1... │
│  ┌───────────────────────┐   ┌───────────────────────┐                 │
│  │ Proposer terpilih     │   │ Proposer fallback     │                 │
│  │ Broadcast Block       │   │ Broadcast Block       │                 │
│  │ Validasi Validator    │   │ Validasi Validator    │                 │
│  │ 2-Phase BFT Voting    │   │ 2-Phase BFT Voting    │                 │
│  │ Commit Certificate    │   │ Commit Certificate    │                 │
│  └───────────────────────┘   └───────────────────────┘                 │
│                                                                        │
│  Validator Set V_E tetap konstan sepanjang Epoch E                     │
│  Transisi ke V_{E+1} hanya dieksekusi pada batas blok akhir Epoch      │
└────────────────────────────────────────────────────────────────────────┘
```

### 2.1 Struktur Epoch dan Pergantian Himpunan Validator
1. **Panjang Epoch ($L_{\text{epoch}}$):** Ditetapkan konstan sebesar $10.000\ \text{blok}$.
   $$E(H) = \left\lfloor \frac{H - 1}{L_{\text{epoch}}} \right\rfloor$$
2. **Himpunan Validator Aktif ($\mathcal{V}_E$):**
   $$\mathcal{V}_E = \left\{ (v_i, \text{pk}_i, w_i) \mid i = 1, \dots, K \right\}$$
   Di mana:
   - $v_i$ adalah identifier/alamat publik validator;
   - $\text{pk}_i$ adalah kunci publik verifikasi tanda tangan konsensus;
   - $w_i$ adalah bobot suara (*voting power*) validator dalam bilangan bulat.
   - $W_E = \sum_{i=1}^K w_i$ adalah total bobot suara pada Epoch $E$.

### 2.2 Aktivasi dan Penonaktifan Validator (Activation & Removal)
1. **Jalur Pendaftaran (Ingress Pipeline):** Permintaan pendaftaran atau penambahan bobot yang diajukan pada Epoch $E$ tidak langsung aktif. Perubahan state validator diproses melalui penundaan dua epoch ($\text{Delay} = 2\ \text{Epoch}$) untuk mencegah serangan manipulasi instan himpunan validator:
   $$\text{Aktivasi Pendaftaran di } E \implies \text{Efektif Berlaku pada } E + 2$$
2. **Pelepasan Sukarela (Unbonding Period):** Validator yang mengajukan pengunduran diri dinonaktifkan dari pemungutan suara pada $E + 2$, namun agunan (*bond/stake*) validator tetap terkunci selama periode unbonding:
   $$\tau_{\text{unbond}} = 14\ \text{Epoch}\ (\approx 140.000\ \text{blok})$$
   Penguncian ini menjamin bahwa jika validator melakukan kecurangan masa lalu (*retrospective equivocation*), hukumannya tetap dapat dieksekusi.
3. **Pemberhentian Paksa (Involuntary Removal / Jailing):** Validator yang offline selama ambang batas toleransi kegagalan atau terbukti melakukan pelanggaran berat (*slashing fault*) dikeluarkan secara otomatis dari $\mathcal{V}_{E+1}$.

### 2.3 Pemilihan Pengusul Blok (Proposer Selection Function)
Untuk setiap tinggi blok $H$ dan nomor putaran $R$, terdapat tepat satu pengusul (*Proposer*) yang sah:

$$\text{Proposer}(H, R) = \text{SelectValidator}\Big( \mathcal{V}_E,\; \mathcal{H}_{\text{seed}}(H, R) \Big)$$

Di mana:
- $\mathcal{H}_{\text{seed}}(H, R)$ adalah seed deterministik yang diturunkan dari hash blok final sebelumnya $B_{H-1}$ dan putaran $R$.
- $\text{SelectValidator}$ adalah fungsi pemetaan deterministik terbobot (*weighted pseudo-random selection*) berdasarkan proporsi $w_i / W_E$.
- Pengusul blok tidak dapat memilih gilirannya sendiri atau memanipulasi jadwal giliran tanpa melanggar konsensus.

---

## 3. Pipa Konsensus Enam Tahap: Dari Blok Hingga Finalitas

Siklus hidup setiap blok Aurion wajib melewati enam tahap state machine berurutan:

```text
  [PROPOSAL]             Tahap 1: Proposer merumuskan & memancarkan Proposal Blok B
      ↓
 [VALIDATION]            Tahap 2: Setiap Validator memvalidasi struktur & hukum moneter
      ↓
   [VOTING]              Tahap 3: Dua fase pemungutan suara (Pre-vote & Pre-commit)
      ↓
   [QUORUM]              Tahap 4: Agregasi tanda tangan mencapai ambang > 2/3 bobot (Q)
      ↓
   [COMMIT]              Tahap 5: Pembentukan Commit Certificate & State Transition
      ↓
  [FINALITY]             Tahap 6: Blok terkunci permanen, abadi, & tak dapat diubah
```

---

### Tahap 1: Pengusulan Blok (Block Proposal)
1. Pada awal putaran $R$ untuk tinggi $H$, simpul yang terpilih sebagai $\text{Proposer}(H, R)$ mengumpulkan transaksi dari mempool, merakit blok kandidat $B$, dan menandatangani pesan proposal:
   $$\mathcal{M}_{\text{prop}} = \text{Sign}\Big( \text{sk}_{\text{proposer}},\; (\text{PROPOSAL}, H, R, \text{Hash}(B), \text{ValidRound}) \Big)$$
2. Proposer menyiarkan pasangan $(B, \mathcal{M}_{\text{prop}})$ ke seluruh jaringan validator.

### Tahap 2: Validasi Deterministik Blok (Block Validation)
Sebelum memberikan suara, setiap validator independen wajib memverifikasi secara ketat bahwa:
1. **Keabsahan Pengusul:** Pesan proposal ditandatangani secara sah oleh $\text{Proposer}(H, R)$.
2. **Keterkaitan Silsilah Rantai:** Hash blok induk tepat cocok dengan hash blok final sebelumnya:
   $$\text{ParentHash}(B) = \text{Hash}(B_{H-1})$$
3. **Validitas State Transisi Transaksi:** Setiap transaksi non-coinbase valid secara tanda tangan kriptografis, tidak melakukan *double-spend*, dan mematuhi konservasi nilai:
   $$\sum \text{Inputs} \ge \sum \text{Outputs}$$
4. **Validitas Moneter Konstitusional & Reward Protokol:**
   - Tidak ada transaksi coinbase eksternal yang diizinkan (*zero external coinbase transaction*).
   - Pencetakan reward blok tetap $R = 1\text{ AUR} = 1.000.000.000\text{ Quantum}\ (10^9\ Q)$ dieksekusi langsung pada level protokol (`aurion-execution`): 20% ke Proposer ($200.000.000\ Q$) dan 80% ke validator penandatangan Precommit QC ($800.000.000\ Q$).
   - Seluruh biaya gas transaksi dialirkan 100% ke validator pembuat proposal blok.
   - Mematuhi invarian moneter `[INV-MON-01]` hingga `[INV-MON-06]`.
   - Menggunakan kalkulasi integer murni Quantum (`u128`) tanpa floating-point.
5. **Validitas Header Konsensus:** Timestamp blok memenuhi aturan *Median Time Past* (MTP) dan tidak melompat ke masa depan:
   $$\text{MTP}(B_{H-1}) < \text{Timestamp}(B) \le \text{CurrentClock} + \text{MaxClockDrift}$$

### Tahap 3: Pemungutan Suara Dua Fase (Two-Phase BFT Voting)
Protokol membagi pemungutan suara menjadi dua fase ketat guna menjamin ketiadaan kebuntuan (*deadlock*) dan konsistensi partisi:

#### Fase 3A: Pre-vote
- Jika blok $B$ lolos seluruh validasi Tahap 2, validator memancarkan suara persetujuan:
  $$\mathcal{V}_{\text{prevote}}(v_i) = \text{Sign}\Big( \text{sk}_i,\; (\text{PREVOTE}, H, R, \text{Hash}(B)) \Big)$$
- Jika blok tidak valid atau waktu tunggu proposal habis (*TimeoutPropose*), validator memancarkan suara kosong (*nil*):
  $$\mathcal{V}_{\text{prevote}}(v_i) = \text{Sign}\Big( \text{sk}_i,\; (\text{PREVOTE}, H, R, \text{NIL}) \Big)$$

#### Sertifikat Persiapan (Polka / Prepare QC):
- Ketika validator mengumpulkan pesan Pre-vote untuk hash blok $B$ dari himpunan validator $\mathcal{J}$ dengan total bobot:
  $$\sum_{j \in \mathcal{J}} w_j \ge \mathcal{Q} = \left\lfloor \frac{2 W_E}{3} \right\rfloor + 1$$
  Maka tercipta sebuah **Polka / Prepare Quorum Certificate** pada $(H, R, \text{Hash}(B))$.
- Validator yang menyaksikan Polka wajib mengunci (*lock*) state internalnya pada blok $B$ dan putaran $R$.

#### Fase 3B: Pre-commit
- Validator yang mengamati Polka yang sah untuk blok $B$ memancarkan suara Pre-commit:
  $$\mathcal{V}_{\text{precommit}}(v_i) = \text{Sign}\Big( \text{sk}_i,\; (\text{PRECOMMIT}, H, R, \text{Hash}(B)) \Big)$$
- Jika validator mengamati Polka untuk *NIL* atau terjadi *TimeoutPrecommit*, validator memancarkan Pre-commit untuk *NIL*.

### Tahap 4: Pembentukan Sertifikat Komitmen (Commit Certificate)
Ketika himpunan suara Pre-commit untuk blok yang sama ($B \neq \text{NIL}$) dihimpun dari validator $\mathcal{K}$ hingga melampaui kuorum:

$$\sum_{k \in \mathcal{K}} w_k \ge \mathcal{Q} = \left\lfloor \frac{2 W_E}{3} \right\rfloor + 1$$

Maka terbentuklah **Sertifikat Komitmen Mutlak (*Commit Certificate* / $\mathcal{CC}(B)$)**:

$$\mathcal{CC}(B) = \left\{ H,\; R,\; \text{Hash}(B),\; \left[ (v_k, \sigma_k) \mid k \in \mathcal{K} \right] \right\}$$

### Tahap 5: Komitmen Ledger dan Transisi State (Commit & Execution)
1. Simpul menyematkan $\mathcal{CC}(B)$ ke dalam ledger lokal (`redb`) sebagai bukti keabsahan mutlak blok $B$.
2. State pohon akun (SMT) dan state moneter diperbarui secara atomik.
3. Eksekusi insentif tingkat protokol:
   - Pencetakan koin baru reward $R = 1\text{ AUR} = 10^9\text{ Q}$ dialokasikan: 20% ke Proposer dan 80% dibagi proporsional ke validator penandatangan Precommit pada $\mathcal{CC}(B)$.
   - 100% akumulasi gas fee transaksi dikreditkan langsung ke akun Proposer.
4. Nomor tinggi blok melangkah maju: $H \to H + 1$, putaran diatur ulang ke $R = 0$.

### Tahap 6: Finalitas Mutlak (Absolute Finality)
Blok $B$ sekarang berstatus **FINAL**. Tidak ada kondisi jaringan, serangan reorganisasi, atau putaran masa depan yang dapat membatalkan atau mencabut blok ini.

---

## 4. Jawaban Formal Tertinggi: Kapan Sebuah Blok Secara Matematis Dianggap Final?

> ### Teorema Finalitas Deterministik Aurion
> Sebuah blok $B$ pada tinggi $H$ dinyatakan **SECARA MATEMATIS FINAL** jika dan hanya jika terdapat tupel pembuktian formal $\langle B, \mathcal{CC}(B) \rangle$ yang memenuhi kelima syarat konjungtif berikut:
> 
> 1. **Konektivitas Rantai Sah (*Inductive State Ancestry*):**  
>    Blok induk $B_{H-1}$ telah berstatus final pada state lokal dan $\text{ParentHash}(B) = \text{Hash}(B_{H-1})$.
> 
> 2. **Kepatuhan Moneter dan Eksekusi Penuh (*Monetary Invariant Conformance*):**  
>    Seluruh transaksi di dalam $B$ memenuhi hukum konservasi nilai, larangan floating-point, dan batas emisi [INV-01] hingga [INV-06] pada [AURION-MONETARY-POLICY-SPECIFICATION.md](file:///c:/Projects/aurion/AURION-MONETARY-POLICY-SPECIFICATION.md).
> 
> 3. **Eksistensi Sertifikat Komitmen Kuorum (*Commit Certificate Existence*):**  
>    Tersedia sekumpulan tanda tangan Pre-commit yang valid pada tinggi $H$ dan putaran $R$ terhadap $\text{Hash}(B)$:
>    $$\mathcal{CC}(B) = \left\{ (v_i, \sigma_i) \mid v_i \in \mathcal{I} \subseteq \mathcal{V}_E \right\}$$
> 
> 4. **Pencapaian Ambang Supermayoritas Kuorum Kriptografis (*Quorum Threshold Satisfied*):**  
>    Total bobot validator yang menandatangani $\mathcal{CC}(B)$ melampaui dua pertiga total bobot voting aktif:
>    $$\sum_{i \in \mathcal{I}} w_i \ge \left\lfloor \frac{2 W_E}{3} \right\rfloor + 1$$
> 
> 5. **Batas Ketiadaan Percabangan (*Intersection Guarantee Under Byzantine Bound*):**  
>    Di bawah aksioma bahwa bobot validator jahat $f < \frac{W_E}{3}$, **tidak mungkin ada blok tandingan $B' \neq B$ pada tinggi $H$ yang dapat memperoleh Commit Certificate yang sah**.

### Bukti Matematis Ketiadaan Percabangan (Mathematical Proof of Non-Forking):
Misalkan secara kontradiksi terdapat dua blok berbeda $B$ dan $B'$ ($B \neq B'$) pada tinggi $H$ yang masing-masing berhasil mengumpulkan Commit Certificate dengan kuorum $\mathcal{I}$ dan $\mathcal{I}'$:
$$\sum_{i \in \mathcal{I}} w_i \ge \left\lfloor \frac{2W_E}{3} \right\rfloor + 1 \quad \text{dan} \quad \sum_{j \in \mathcal{I}'} w_j \ge \left\lfloor \frac{2W_E}{3} \right\rfloor + 1$$

Irisan bobot suara antara kedua kuorum tersebut memenuhi prinsip inklusi-eksklusi:
$$\text{Weight}(\mathcal{I} \cap \mathcal{I}') = \text{Weight}(\mathcal{I}) + \text{Weight}(\mathcal{I}') - \text{Weight}(\mathcal{I} \cup \mathcal{I}')$$
Karena $\text{Weight}(\mathcal{I} \cup \mathcal{I}') \le W_E$, maka:
$$\text{Weight}(\mathcal{I} \cap \mathcal{I}') \ge 2\left( \frac{2W_E}{3} \right) - W_E = \frac{4W_E}{3} - W_E = \frac{W_E}{3} + 2$$

Karena bobot anggota Byzantine dibatasi maksimum $f < \frac{W_E}{3}$, maka:
$$\text{Weight}(\text{Validator Jujur di dalam } \mathcal{I} \cap \mathcal{I}') \ge \left(\frac{W_E}{3} + 2\right) - f \ge 2 > 0$$

Artinya, sekurang-kurangnya terdapat validator jujur yang harus menandatangani **dua pesan Pre-commit yang berbeda untuk blok berbeda pada tinggi yang sama**. 

Namun, berdasarkan **Aturan Penguncian State (Locking Rule)** pada spesifikasi ini, validator jujur secara deterministik dilarang menandatangani dua Pre-commit yang bertentangan. 

$$\therefore \text{Percabangan ganda (fork) pada status FINAL adalah KONTRAKSI MATEMATIS MUTLAK. Q.E.D.}$$

---

## 5. Aturan Pemilihan Cabang (Fork Choice Rule)

1. **Prinsip Induk Final (*Root of Trust*):**  
   Titik awal (*anchor*) pohon konsensus adalah blok final tertinggi yang diketahui ($\text{Block}_{\text{final}}$). Semua blok sebelum dan pada tinggi $\text{Height}(\text{Block}_{\text{final}})$ adalah bagian dari kanonikal ledger dan kebal terhadap revisi apa pun.
2. **Kedalaman Reorganisasi Nol (*Reorganization Depth = 0*):**  
   Protokol Aurion **TIDAK MENGAKUI** adanya reorganisasi rantai terhadap blok yang telah memegang Commit Certificate.
   $$\text{MaxReorgDepth}(\text{Finalized Blocks}) = 0$$
3. **Penyelesaian Sengketa Putaran (Round Conflict Resolution):**  
   Jika pada tinggi $H$ yang belum final terdapat beberapa putaran $R_1 < R_2$, simpul wajib memilih cabang putaran tertinggi yang didukung oleh *Prepare Quorum Certificate (Polka)* yang sah.

---

## 6. Pelanggaran Byzantine, Ekuivokasi, dan Pemusnahan (Slashing)

Aurion menegakkan disiplin penalti tanpa kompromi terhadap segala bentuk pengkhianatan konsensus melalui mekanisme pemusnahan (*Slashing*).

```text
┌────────────────────────────────────────────────────────────────────────┐
│                        BUKTI EKUIVOKASI (FRAUD PROOF)                  │
│                                                                        │
│   Pesan A: Sign(sk_i, (PHASE, H, R, Hash(B_1)))                        │
│   Pesan B: Sign(sk_i, (PHASE, H, R, Hash(B_2)))                        │
│                                                                        │
│   Kondisi Pelanggaran: Hash(B_1) ≠ Hash(B_2)                           │
│   Verifikasi: Kunci Publik pk_i identik                                │
│                                                                        │
│   KONSEKUENSI PROTOKOL:                                                │
│   1. Agunan validator dimusnahkan 100% (Slashing)                      │
│   2. Dana hangus permanen (Burned), mengurangi pasokan beredar         │
│   3. Validator dikeluarkan permanen (Permanent Tombstoning)            │
└────────────────────────────────────────────────────────────────────────┘
```

### 6.1 Taksonomi Pelanggaran Kriptografis (Slashing Faults)
1. **Pengusulan Ganda (*Double Proposal*):**  
   Pengusul menandatangani dua header proposal blok yang berbeda $(B_1 \neq B_2)$ untuk tinggi $H$ dan putaran $R$ yang sama.
2. **Pemungutan Suara Ganda (*Double Voting*):**  
   Validator menandatangani dua pesan suara yang berbeda untuk tinggi $H$, putaran $R$, dan fase yang sama:
   $$\text{Sign}(\text{sk}_i, (\text{FASE}, H, R, X)) \quad \land \quad \text{Sign}(\text{sk}_i, (\text{FASE}, H, R, Y)) \quad \text{dengan } X \neq Y$$
3. **Pelanggaran Kunci Suara (*Surround / Out-of-Order Voting*):**  
   Validator memberikan suara Pre-commit pada putaran $R_2 > R_1$ tanpa adanya Polka sah yang melepaskan kuncian (*unlock*) dari putaran $R_1$.

### 6.2 Bukti Pelanggaran Tanpa Sanggahan (Fraud Proof Object)
Sebuah bukti ekuivokasi dikemas dalam struktur data pembuktian mandiri:
$$\mathcal{E} = \Big\langle \text{pk}_v,\; \mathcal{M}_1,\; \sigma_1,\; \mathcal{M}_2,\; \sigma_2 \Big\rangle$$
Di mana kedua pesan bertentangan ditandatangani oleh kunci privat yang sama. Setiap simpul yang menerima $\mathcal{E}$ dapat memverifikasi pelanggaran secara instan tanpa memerlukan asumsi kepercayaan.

### 6.3 Hukuman Pemusnahan (Slashing Execution)
Ketika bukti ekuivokasi $\mathcal{E}$ diverifikasi oleh konsensus:
1. **Penyitaan 100% Agunan (*Total Bond Annihilation*):** Seluruh dana agunan validator disita tanpa ampun.
2. **Pembakaran Suplai (*Supply Deflation*):** Dana agunan yang disita **DIBUANG DARI STATE UTXO (BURNED)** secara permanen, memperketat suplai sirkulasi dan memberikan keuntungan deflasi bagi seluruh pemegang AUR yang jujur.
3. **Pengusiran Abadi (*Permanent Tombstoning*):** Kunci publik validator dimasukkan ke dalam daftar hitam konsensus selamanya dan tidak dapat lagi didaftarkan pada epoch mana pun.

---

## 7. Waktu Habis (Timeout), Progresi Putaran, dan Pemulihan Partisi

### 7.1 Parameter Waktu Konsensus (Timeout Schedule)
Untuk memastikan liveness tetap terjaga ketika pengusul mengalami crash atau jaringan melambat, konsensus menggunakan jadwal timeout berjenjang:

| Parameter Timeout | Nilai Dasar Putaran 0 | Fungsi Pembesaran Putaran $R$ |
| :--- | :--- | :--- |
| $\Delta_{\text{propose}}$ | $10\ \text{detik}$ | $\Delta_{\text{propose}}(R) = 10\text{s} + (R \times 2\text{s})$ |
| $\Delta_{\text{prevote}}$ | $5\ \text{detik}$ | $\Delta_{\text{prevote}}(R) = 5\text{s} + (R \times 1\text{s})$ |
| $\Delta_{\text{precommit}}$ | $5\ \text{detik}$ | $\Delta_{\text{precommit}}(R) = 5\text{s} + (R \times 1\text{s})$ |

### 7.2 Progresi Putaran (Round Progression Invariant)
1. Jika simpul tidak menerima proposal valid dalam durasi $\Delta_{\text{propose}}(R)$, simpul memancarkan $\text{PREVOTE}(\text{NIL})$.
2. Jika simpul tidak berhasil melihat Polka dalam durasi $\Delta_{\text{prevote}}(R)$, simpul memancarkan $\text{PRECOMMIT}(\text{NIL})$.
3. Setelah mengumpulkan kuorum $\mathcal{Q}$ suara $\text{PRECOMMIT}(\text{NIL})$, seluruh validator melangkah ke putaran berikutnya:
   $$R \to R + 1$$
   Pengusul baru $\text{Proposer}(H, R+1)$ mengambil alih inisiatif perakitan proposal.

### 7.3 Pemulihan dari Pemisahan Jaringan (Network Partition Recovery)
Jika terjadi partisi jaringan fisik di mana simpul terpisah menjadi beberapa kelompok terisolasi:
1. **Perlindungan Pembekuan Aman (*Safety Halting*):** Karena setiap partisi memiliki bobot suara $< \mathcal{Q}$ ($< \frac{2}{3} W_E$), **tidak ada partisi yang dapat menghasilkan Commit Certificate**. Produksi blok berhenti secara otomatis tanpa ada resiko penciptaan rantai ganda.
2. **Penyambungan Kembali (*Reconnection & Catch-Up*):** Ketika partisi jaringan tersambung kembali, simpul saling bertukar pesan Pre-vote tertinggi, membentuk sertifikat Polka, memancarkan Pre-commit, dan secara langsung mencapai finalitas pada blok yang tertunda tanpa kehilangan riwayat data.

---

## 8. Hubungan Arsitektural Konsensus dengan Modul Asal-Usul Aset (Provenance Module)

Mengukuhkan mandat **Bab 5 [AURION CONSTITUTION.md](file:///c:/Projects/aurion/AURION%20CONSTITUTION.md)**, spesifikasi ini menegaskan pemisahan hierarkis antara proses pembentukan blok konsensus dan penetapan identitas silsilah aset:

```text
    ┌────────────────────────────────────────────────────────┐
    │                      KONSENSUS BFT                     │
    │  1. Proposer mengusulkan blok B                        │
    │  2. Validator menyetujui validitas blok                │
    │  3. Terbentuk Commit Certificate CC(B)                 │
    │  4. Blok dinyatakan SAH dan FINAL                      │
    └───────────────────────────┬────────────────────────────┘
                                │
                                ▼
    ┌────────────────────────────────────────────────────────┐
    │             ASSET IDENTIFICATION MODULE                │
    │  1. Menerima Block Final & Block Reward sah            │
    │  2. Mengisolasi input dari kendali arbitrer Proposer   │
    │  3. Menurunkan Reward Provenance Identifier (RPI)      │
    │  4. Menetapkan status keabsahan moneter absolut        │
    └────────────────────────────────────────────────────────┘
```

1. **Bukan Hak Proposer:** Proposer bertugas menyusun transaksi dan mengusulkan blok; Proposer **TIDAK BERWENANG** mendikte identitas silsilah koin yang diterbitkan.
2. **Katalisator Determinis:** Penetapan *Reward Provenance Identifier* (RPI) hanya dapat dipicu setelah blok memperoleh status **FINAL** berdasarkan kehadiran Commit Certificate $\mathcal{CC}(B)$ yang sah.
3. **Integritas Moneter:** Koin yang lahir dari blok yang belum final tidak dapat dimasukkan ke dalam modul provenance dan tidak diakui sebagai state sah oleh jaringan.

---

## 9. Rangkuman Aksioma Konsensus Tertinggi (Supreme Consensus Invariants)

Untuk memastikan konsensus Aurion-BFT tahan uji di masa depan tanpa bergantung pada bahasa pemrograman spesifik, empat aksioma logika ini dikunci secara permanen:

> ### Aksioma 1 (Deterministik Finalitas Tunggal):
> $$\forall H \ge 1, \quad \big| \left\{ B \mid B \text{ memiliki } \mathcal{CC}(B) \text{ sah pada tinggi } H \right\} \big| \le 1$$
> *"Pada setiap tinggi blok, hanya ada paling banyak satu blok yang dapat memperoleh Commit Certificate yang sah."*

> ### Aksioma 2 (Kekebalan Reorganisasi):
> $$\text{State}(\text{Ledger up to } H_{\text{final}}) \text{ is Permanently Immutable}$$
> *"Blok yang telah mengantongi Commit Certificate tidak dapat digeser, dibatalkan, atau direorganisasi oleh kekuatan komputasi maupun putaran konsensus berikutnya."*

> ### Aksioma 3 (Kekuatan Kuorum Supermayoritas):
> $$\mathcal{Q} = \left\lfloor \frac{2 W_E}{3} \right\rfloor + 1$$
> *"Tidak ada keputusan konsensus, baik pada fase Pre-vote maupun Pre-commit, yang diakui sah tanpa dukungan lebih dari dua pertiga bobot voting himpunan validator aktif."*

> ### Aksioma 4 (Pemusnahan Mutlak Terhadap Pengkhianatan):
> $$\text{Proof}(\mathcal{E}) \implies \text{Slash}(v, 100\%) \land \text{Burn}(\text{Bond}_v) \land \text{Tombstone}(v)$$
> *"Setiap bukti kriptografis ekuivokasi menghasilkan pemusnahan total agunan secara deflasi dan pengusiran abadi dari jaringan Aurion."*
