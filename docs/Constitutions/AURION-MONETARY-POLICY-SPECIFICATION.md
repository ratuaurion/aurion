# AURION MONETARY POLICY SPECIFICATION
## Spesifikasi Kebijakan Moneter dan Konstitusi Matematika Protokol Aurion

> **Dokumen Referensi:** [AURION CONSTITUTION.md](file:///c:/Projects/aurion/AURION%20CONSTITUTION.md)  
> **Status:** RATIFIED & LOCKED SPECIFICATION  
> **Klasifikasi:** Protokol Inti Konsensus Layer 1 (Monetary Core)  
> **Versi:** 1.0.0-PROD  
> **Sifat Ketetapan:** Kriptografis, Deterministik Matematis, Nol-Floating-Point

---

## 1. Unit Moneter, Atomisitas, dan Tata Nama Denominasi

### 1.1 Satuan Moneter Utama (Base Currency Unit)
Satuan moneter resmi ekosistem Aurion adalah **AUR**. Seluruh penetapan harga, valuasi makroekonomi, dan antarmuka pengguna luar menggunakan satuan dasar AUR sebagai unit referensi.

### 1.2 Satuan Atomik Moneter (Atomic Unit / Quantum)
Satuan atomik terkecil yang sah di dalam protokol konsensus Aurion disebut **Quantum** (bentuk jamak: **Quanta**).
- Quantum adalah unit fundamental yang bersifat **indivisible** (tidak dapat dibagi lagi).
- Protokol Aurion **tidak mengakui** adanya nilai moneter di bawah 1 Quantum.
- Simbol resmi Quantum dalam notasi matematika protokol adalah $\mathbf{Q}$.

### 1.3 Rasio Konversi dan Presisi Tetap
Rasio konversi antara satuan dasar AUR dan unit atomik Quantum ditetapkan secara permanen sebagai berikut:

$$\mathbf{1\ AUR = 100.000.000\ Quantum\ (10^8\ Q)}$$

Protokol Aurion menggunakan **presisi tetap 8 angka di belakang koma (8 fixed decimal places)**.

### 1.4 Tabel Tata Nama Denominasi (Denomination Tiers)
Untuk memfasilitasi transaksi mikro hingga makro pada seluruh lapisan ekosistem, Aurion menetapkan tangga denominasi terstandarisasi:

| Nama Denominasi | Simbol | Nilai dalam Quantum ($Q$) | Nilai dalam AUR | Pangkat ($10^n$) | Peruntukan Utama |
| :--- | :--- | ---:| ---:| :--- | :--- |
| **Quantum** | $Q$ | $1\ Q$ | $0,00000001\ \text{AUR}$ | $10^0\ Q$ | Unit internal konsensus & fee granularity |
| **Micro-AUR** | $\mu\text{AUR}$ | $100\ Q$ | $0,00000100\ \text{AUR}$ | $10^2\ Q$ | Transaksi mikro data & payload fees |
| **Milli-AUR** | $\text{mAUR}$ | $100.000\ Q$ | $0,00100000\ \text{AUR}$ | $10^5\ Q$ | Biaya transfer standar & smart contracts |
| **Cent-AUR** | $\text{cAUR}$ | $1.000.000\ Q$ | $0,01000000\ \text{AUR}$ | $10^6\ Q$ | Pembayaran komersial ritel mikro |
| **Deci-AUR** | $\text{dAUR}$ | $10.000.000\ Q$ | $0,10000000\ \text{AUR}$ | $10^7\ Q$ | Satuan intermediate pasar |
| **AUR (Base)** | $\text{AUR}$ | $100.000.000\ Q$ | $1,00000000\ \text{AUR}$ | $10^8\ Q$ | Satuan perdagangan & cadangan moneter |
| **Kilo-AUR** | $\text{kAUR}$ | $100.000.000.000\ Q$ | $1.000,00000000\ \text{AUR}$ | $10^{11}\ Q$ | Likuiditas validator & staking pool |
| **Mega-AUR** | $\text{MAUR}$ | $100.000.000.000.000\ Q$ | $1.000.000,00000000\ \text{AUR}$ | $10^{14}\ Q$ | Alokasi perbendaharaan & genesis vaults |

---

## 2. Batas Suplai Maksimum dan Alokasi Genesis

### 2.1 Batas Suplai Maksimum Absolut (Hard Cap)
Protokol Aurion mengunci secara permanen batas total suplai moneter yang dapat diciptakan sepanjang keberadaan jaringan:

$$\mathbf{S_{\max} = 66.000.000\ AUR}$$

Dalam satuan internal konsensus Quantum:

$$\mathbf{S_{\max}^{(Q)} = 66.000.000 \times 100.000.000 = 6.600.000.000.000.000\ Quantum}$$

$$\mathbf{S_{\max}^{(Q)} = 6,6 \times 10^{15}\ Q\ \text{(Enam koma enam kuadriliun Quantum)}}$$

Setiap penambahan suplai yang menyebabkan total koin yang pernah diterbitkan melebihi $S_{\max}^{(Q)}$ adalah pelanggaran konsensus fatal yang wajib ditolak oleh setiap node.

### 2.2 Alokasi Genesis Terverifikasi (Genesis Allocation)
Pada blok genesis ($H=0$), hak atas suplai dialokasikan menjadi tiga pilar tertutup yang saling terisolasi:

```text
                               TOTAL SUPLAI MAKSIMUM
                           66.000.000 AUR (100,00000000%)
                     6.600.000.000.000.000 Quantum (10^8 Scale)
                                         │
        ┌────────────────────────────────┼────────────────────────────────┐
        ▼                                ▼                                ▼
  CREATOR ALLOCATION           DEVELOPER ALLOCATION            COMMUNITY / PUBLIC
19.800.000 AUR (30,00%)        3.300.000 AUR (5,00%)        42.900.000 AUR (65,00%)
1.980.000.000.000.000 Q          330.000.000.000.000 Q        4.290.000.000.000.000 Q
 (Infrastruktur & Ops)       (R&D, Testnet, Sumber Faucet)      (Pure PoW Block Mining)
```

### 2.3 Rincian Pembagian Genesis

| Domain Alokasi | Rasio (%) | Jumlah dalam AUR | Jumlah dalam Quantum ($Q$) | Mekanisme Penerbitan |
| :--- | ---:| ---:| ---:| :--- |
| **Creator Vault** | $30,00\%$ | $19.800.000\ \text{AUR}$ | $1.980.000.000.000.000\ Q$ | Terbit pada State Genesis ($H=0$) |
| **Developer Vault** | $5,00\%$ | $3.300.000\ \text{AUR}$ | $330.000.000.000.000\ Q$ | Terbit pada State Genesis ($H=0$) |
| **Community / Public** | $65,00\%$ | $42.900.000\ \text{AUR}$ | $4.290.000.000.000.000\ Q$ | Terbit bertahap via Block Reward ($H \ge 1$) |
| **TOTAL KONSENSUS** | $\mathbf{100,00\%}$ | $\mathbf{66.000.000\ \text{AUR}}$ | $\mathbf{6.600.000.000.000.000\ Q}$ | **Konservasi Nilai Matematika Tertutup** |

### 2.4 Aturan Moneter Faucet (The Faucet Monetary Invariant)
1. **Sumber Eksklusif:** Faucet resmi jaringan Aurion **WAJIB (MUST)** mengambil likuiditas secara eksklusif dari *Developer Vault* ($3.300.000\ \text{AUR}$).
2. **Larangan Pencetakan Tambahan:** Faucet **DILARANG KERAS (MUST NOT)** mencetak uang baru atau memanfaatkan fungsi minting apa pun di luar alokasi developer yang ada.
3. **Isolasi Komunitas:** Faucet **DILARANG KERAS** memotong atau mengalihkan bagian dari alokasi 65% Community/Public.
4. **Persamaan Konservasi Faucet:**
   $$\text{Saldo Faucet}(t) + \text{Total Disalurkan Faucet}(t) \le \text{Developer Vault Initial} = 330.000.000.000.000\ Q$$

---

## 3. Jadwal Emisi dan Desain Subsidi Blok (Emission Schedule)

### 3.1 Parameter Waktu dan Periode Halving
Alokasi Community/Public sebesar $42.900.000\ \text{AUR}$ didistribusikan melalui algoritma subsidi penambangan murni (*pure mining*).
- **Target Interval Waktu Blok:** $T_{\text{target}} = 60\ \text{detik}$ ($1\ \text{menit}$).
- **Interval Halving Konsensus ($N_{\text{era}}$):** Tepat **$2.145.000$ blok**.
- **Estimasi Durasi per Era Halving:**
  $$\text{Durasi} = \frac{2.145.000 \times 60\ \text{detik}}{86.400 \times 365,25\ \text{hari}} \approx 4,0786\ \text{tahun}\ (\approx 4\ \text{tahun})$$

### 3.2 Subsidi Awal Blok ($R_0$)
Subsidi awal untuk setiap blok pada Era 0 ($H \in [1, 2.145.000]$) adalah:

$$\mathbf{R_0 = 10\ AUR = 1.000.000.000\ Quantum\ (10^9\ Q)}$$

### 3.3 Formulasi Matematis Subsidi Blok
Untuk setiap tinggi blok $H \ge 1$, indeks era halving $e(H)$ dihitung menggunakan pembagian bilangan bulat (*floor division*):

$$e(H) = \left\lfloor \frac{H - 1}{N_{\text{era}}} \right\rfloor = \left\lfloor \frac{H - 1}{2.145.000} \right\rfloor$$

Subsidi blok pada tinggi $H$, dinotasikan sebagai $\mathcal{S}(H)$, dihitung secara deterministik menggunakan operasi *binary right shift*:

$$\mathcal{S}(H) = \begin{cases} 
R_0 \gg e(H) = \left\lfloor \dfrac{1.000.000.000}{2^{e(H)}} \right\rfloor \text{ Quantum}, & \text{jika } e(H) < 30 \\
0\ \text{Quantum}, & \text{jika } e(H) \ge 30 
\end{cases}$$

### 3.4 Tabel Emisi Multi-Era (Era-by-Era Emission Schedule)

| Era ($e$) | Rentang Blok ($H$) | Subsidi per Blok (AUR) | Subsidi per Blok ($Q$) | Total Emisi Era (AUR) | Total Emisi Era ($Q$) | Kumulatif Mined ($Q$) | % Mined |
| :---: | :---: | ---:| ---:| ---:| ---:| ---:| ---:|
| **0** | $1 - 2.145.000$ | $10,00000000$ | $1.000.000.000$ | $21.450.000$ | $2.145.000.000.000.000$ | $2.145.000.000.000.000$ | $50,000\%$ |
| **1** | $2.145.001 - 4.290.000$ | $5,00000000$ | $500.000.000$ | $10.725.000$ | $1.072.500.000.000.000$ | $3.217.500.000.000.000$ | $75,000\%$ |
| **2** | $4.290.001 - 6.435.000$ | $2,50000000$ | $250.000.000$ | $5.362.500$ | $536.250.000.000.000$ | $3.753.750.000.000.000$ | $87,500\%$ |
| **3** | $6.435.001 - 8.580.000$ | $1,25000000$ | $125.000.000$ | $2.681.250$ | $268.125.000.000.000$ | $4.021.875.000.000.000$ | $93,750\%$ |
| **4** | $8.580.001 - 10.725.000$ | $0,62500000$ | $62.500.000$ | $1.340.625$ | $134.062.500.000.000$ | $4.155.937.500.000.000$ | $96,875\%$ |
| **5** | $10.725.001 - 12.870.000$ | $0,31250000$ | $31.250.000$ | $670.312,5$ | $67.031.250.000.000$ | $4.222.968.750.000.000$ | $98,438\%$ |
| **6** | $12.870.001 - 15.015.000$ | $0,15625000$ | $15.625.000$ | $335.156,25$ | $33.515.625.000.000$ | $4.256.484.375.000.000$ | $99,219\%$ |
| **7** | $15.015.001 - 17.160.000$ | $0,07812500$ | $7.812.500$ | $167.578,125$ | $16.757.812.500.000$ | $4.273.242.187.500.000$ | $99,609\%$ |
| **8** | $17.160.001 - 19.305.000$ | $0,03906250$ | $3.906.250$ | $83.789,0625$ | $8.378.906.250.000$ | $4.281.621.093.750.000$ | $99,805\%$ |
| **9** | $19.305.001 - 21.450.000$ | $0,01953125$ | $1.953.125$ | $41.894,53125$ | $4.189.453.125.000$ | $4.285.810.546.875.000$ | $99,902\%$ |
| **10** | $21.450.001 - 23.595.000$ | $0,00976562$ | $976.562$ | $20.947,2549$ | $2.094.725.490.000$ | $4.287.905.272.365.000$ | $99,951\%$ |
| **...** | ... | ... | ... | ... | ... | ... | ... |
| **29** | $62.205.001 - 64.350.000$ | $0,00000001$ | $1$ | $0,02145$ | $2.145.000$ | $4.289.999.999.989.275$ | $\approx 100\%$ |
| **$\ge 30$** | $\ge 64.350.001$ | $0,00000000$ | $0$ | $0$ | $0$ | $\le 4.290.000.000.000.000$ | $100,000\%$ |

### 3.5 Bukti Ketidaklampauan Batas (Proof of Finite Convergence)
Karena sifat pembagian bilangan bulat yang membuang sisa pecahan (*truncation toward zero*), jumlah kumulatif seluruh emisi subsidi penambangan memenuhi batas ketat:

$$\sum_{H=1}^{\infty} \mathcal{S}(H) = \sum_{e=0}^{29} \left( \left\lfloor \frac{10^9}{2^e} \right\rfloor \times 2.145.000 \right) = 4.289.999.999.989.275\ Q$$

$$\sum_{H=1}^{\infty} \mathcal{S}(H) < 4.290.000.000.000.000\ Q\ (42.900.000\ \text{AUR})$$

$$\text{Selisih Sisa Unmintable} = 4.290.000.000.000.000 - 4.289.999.999.989.275 = 10.725\ Q\ (0,00010725\ \text{AUR})$$

Selisih sebesar $10.725\ Q$ adalah sisa fraksional yang secara matematis tidak pernah dicetak (*permanently unminted dust*). Hal ini membuktikan secara formal bahwa suplai penambangan **mustahil melampaui $42.900.000\ \text{AUR}$**.

---

## 4. Struktur Hadiah Blok, Biaya Transaksi, dan Pembakaran Fee

### 4.1 Struktur Transaksi Coinbase
Setiap blok yang valid ($H \ge 1$) wajib memuat tepat **satu transaksi Coinbase** yang ditempatkan pada indeks transaksi pertama (`index = 0`).
1. **Input Coinbase:** Wajib memiliki tepat satu input kosong (`null outpoint`) dengan identifier hash bernilai nol dan indeks `0xFFFFFFFF`.
2. **Kematangan Hadiah (Coinbase Maturity):** Output transaksi coinbase **TIDAK DAPAT DIBELANJAKAN** sebelum melewati ambang konfirmasi:
   $$\text{Maturity Threshold} = 100\ \text{blok}$$
   Sebuah output coinbase pada blok $H$ hanya sah digunakan sebagai input pada blok $H' \ge H + 100$.

### 4.2 Formulasi Hadiah Produser Blok (Miner / Validator Reward)
Total nilai output yang sah pada transaksi coinbase blok $H$, dinotasikan sebagai $\mathcal{V}_{\text{coinbase}}(H)$, dibatasi oleh jumlah subsidi resmi ditambah biaya transaksi yang dialokasikan:

$$\mathcal{V}_{\text{coinbase}}(H) \le \mathcal{S}(H) + \mathcal{F}_{\text{miner}}(H)$$

Di mana:
- $\mathcal{S}(H)$ adalah subsidi blok resmi pada tinggi $H$.
- $\mathcal{F}_{\text{miner}}(H)$ adalah total biaya transaksi blok yang menjadi hak produser blok.

> **Aturan Subsidi yang Tidak Diklaim (Unclaimed Subsidy Rule):**  
> Jika produser blok membuat output coinbase dengan nilai lebih kecil dari hak maksimum ($\mathcal{V}_{\text{coinbase}} < \mathcal{S}(H) + \mathcal{F}_{\text{miner}}$), selisih yang tidak diklaim dianggap **hangus secara permanen (*permanently unminted*)** dan tidak dapat diklaim pada blok-blok berikutnya.

### 4.3 Biaya Transaksi (Transaction Fee Accounting)
Untuk setiap transaksi non-coinbase $T_x$, biaya transaksi $\text{Fee}(T_x)$ dihitung secara ketat melalui prinsip konservasi input-output:

$$\text{Fee}(T_x) = \sum_{i \in \text{Inputs}} \text{Value}(i) - \sum_{j \in \text{Outputs}} \text{Value}(j)$$

Syarat Validitas Transaksi:
$$\sum_{i \in \text{Inputs}} \text{Value}(i) \ge \sum_{j \in \text{Outputs}} \text{Value}(j) \implies \text{Fee}(T_x) \ge 0$$
Transaksi dengan $\text{Fee}(T_x) < 0$ dianggap tidak sah dan ditolak langsung oleh konsensus.

### 4.4 Mekanisme Pembakaran Biaya (Fee Burning Architecture)
Aurion mengadopsi kebijakan deflasi berbasis pembakaran biaya transaksi parsial untuk mengimbangi laju sirkulasi dan memberikan tekanan deflasi proporsional terhadap volume aktivitas jaringan:

1. **Rasio Pembakaran Protokol ($\beta_{\text{burn}}$):**  
   Ditetapkan sebesar parameter konsensus:
   $$\beta_{\text{burn}} = 20\%\ (\text{default baseline})$$
2. **Kompensasi Produser Blok ($\beta_{\text{miner}}$):**
   $$\beta_{\text{miner}} = 100\% - \beta_{\text{burn}} = 80\%$$
3. **Kalkulasi Pembagian Biaya Tiap Blok:**
   Untuk total fee blok $\mathcal{F}_{\text{total}}(H) = \sum_{T_x \in \text{Block}} \text{Fee}(T_x)$:
   $$\mathcal{F}_{\text{burned}}(H) = \left\lfloor \frac{\mathcal{F}_{\text{total}}(H) \times 20}{100} \right\rfloor$$
   $$\mathcal{F}_{\text{miner}}(H) = \mathcal{F}_{\text{total}}(H) - \mathcal{F}_{\text{burned}}(H)$$

> **Prinsip Pembakaran Nyata:**  
> Nilai $\mathcal{F}_{\text{burned}}(H)$ tidak dialihkan ke akun apa pun. Nilai tersebut ditiadakan dari state UTXO jaringan, sehingga mengurangi pasokan beredar secara absolut tanpa melanggar konservasi total koin yang pernah diterbitkan.

---

## 5. Fungsi Transisi State Moneter dan Invarian Konsensus

### 5.1 Definisi Kuantitas Suplai Moneter
Protokol Aurion membedakan secara tegas tiga terminologi suplai:
1. **Total Suplai Maksimum ($S_{\max}$):** Batas plafon absolut yaitu $6.600.000.000.000.000\ Q$.
2. **Suplai Diterbitkan Kumulatif ($S_{\text{emitted}}(H)$):** Total seluruh Quantum yang pernah diciptakan secara sah sejak genesis hingga blok $H$:
   $$S_{\text{emitted}}(H) = S_{\text{creator}} + S_{\text{developer}} + \sum_{h=1}^{H} \mathcal{S}_{\text{claimed}}(h)$$
3. **Suplai Beredar Aktif ($S_{\text{circulating}}(H)$):** Total seluruh Quantum yang ada dan dapat dibelanjakan pada state UTXO aktif saat blok $H$:
   $$S_{\text{circulating}}(H) = S_{\text{emitted}}(H) - \sum_{h=1}^{H} \mathcal{F}_{\text{burned}}(h)$$

### 5.2 Fungsi Transisi State Blok (Block Monetary State Transition)
Perubahan state moneter dari blok $H-1$ ke blok $H$ dinyatakan sebagai pemetaan deterministik:

$$\begin{pmatrix} S_{\text{emitted}}(H) \\ S_{\text{circulating}}(H) \end{pmatrix} = \begin{pmatrix} S_{\text{emitted}}(H-1) + \mathcal{S}_{\text{actual}}(H) \\ S_{\text{circulating}}(H-1) + \mathcal{S}_{\text{actual}}(H) - \mathcal{F}_{\text{burned}}(H) \end{pmatrix}$$

### 5.3 Enam Invarian Suplai Tertinggi (The Six Supreme Monetary Invariants)
Setiap simpul penuh (*full node*) Aurion wajib memvalidasi keenam invarian berikut pada setiap blok yang diterima:

$$\begin{aligned}
\mathbf{[INV-01]}\quad & S_{\text{emitted}}(H) \le S_{\max} = 6.600.000.000.000.000\ Q, \quad \forall H \ge 0 \\
\mathbf{[INV-02]}\quad & S_{\text{circulating}}(H) \le S_{\text{emitted}}(H), \quad \forall H \ge 0 \\
\mathbf{[INV-03]}\quad & S_{\text{creator}} = 1.980.000.000.000.000\ Q \quad (\text{Locked at Genesis}) \\
\mathbf{[INV-04]}\quad & S_{\text{developer}} = 330.000.000.000.000\ Q \quad (\text{Locked at Genesis}) \\
\mathbf{[INV-05]}\quad & \sum_{h=1}^{H} \mathcal{S}_{\text{actual}}(h) \le 4.290.000.000.000.000\ Q, \quad \forall H \ge 1 \\
\mathbf{[INV-06]}\quad & \text{Balance}(\text{UTXO Set at } H) = S_{\text{circulating}}(H)
\end{aligned}$$

Jika salah satu dari keenam invarian di atas dilanggar, blok dinyatakan mengalami **Integrity Fault** dan seluruh simpul wajib melakukan penolakan serta penghentian eksekusi (*fail-stop*).

---

## 6. Aturan Aritmetika, Overflow/Underflow, dan Pembulatan

### 6.1 Larangan Total Aritmetika Pecahan Mengambang (Zero-Float Mandate)
1. Seluruh komputasi konsensus, biaya, saldo, reward, pembagian fee, dan transisi state **DILARANG KERAS** menggunakan tipe data floating-point IEEE 754 (seperti `f32`, `f64`, atau tipe float arsitektural lainnya).
2. Representasi kode perangkat lunak wajib menggunakan integer tanpa tanda (*unsigned integer*) dengan presisi minimal 128-bit (`u128`).
3. Kapasitas tipe `u128` ($2^{128} - 1 \approx 3,4028 \times 10^{38}$) memberikan ruang penyangga sebesar $\approx 5,15 \times 10^{22}$ kali lebih besar dari $S_{\max}^{(Q)}$ ($6,6 \times 10^{15}\ Q$), menjamin keamanan representasi terhadap luapan normal.

### 6.2 Aturan Penanganan Overflow dan Underflow (Checked Arithmetic)
1. **Checked Operations Mandatory:** Semua operasi penambahan, pengurangan, dan perkalian nilai moneter pada lapisan konsensus wajib menggunakan operasi berpengecekan:
   - `checked_add`
   - `checked_sub`
   - `checked_mul`
   - `checked_div`
2. **Larangan Wrapping dan Saturasi:** Operasi aritmetika moneter konsensus **DILARANG KERAS** menggunakan aritmetika wrapping (`wrapping_*`) maupun saturasi (`saturating_*`) pada mutasi saldo rekening/UTXO.
3. **Konsekuensi Kegagalan Aritmetika:**
   - Kegagalan `checked_sub` pada saldo mengindikasikan percobaan *double-spend* atau penarikan dana melebihi saldo $\rightarrow$ **Transaksi Ditolak**.
   - Terjadinya luapan `checked_add` pada total suplai mengindikasikan kerusakan integritas state $\rightarrow$ **Node Fail-Stop Lockout**.

### 6.3 Aturan Pembulatan (Deterministic Rounding Rules)
1. **Floor Rounding Only (Truncation toward Zero):** Seluruh operasi pembagian integer dalam kalkulasi moneter wajib menggunakan pembulatan ke bawah (*floor division*).
2. **Larangan Pembulatan ke Atas:** Tidak diperbolehkan menggunakan metode pembulatan ke atas (*ceiling*) atau pembulatan setengah ke atas (*round-half-up*) yang berpotensi menghasilkan 1 Quantum tambahan dari ketiadaan (*inflationary rounding*).
3. **Penetapan Sisa Hasil Bagi:** Setiap sisa hasil bagi (*remainder*) dari pembagian alokasi atau fee dialokasikan mengikuti aturan deterministik berikut:
   $$\text{Remainder} = A - \left( \left\lfloor \frac{A}{B} \right\rfloor \times B \right)$$
   Sisa pembagian pada kalkulasi reward atau fee dialokasikan kepada produser blok untuk menjamin konservasi nilai:
   $$\mathcal{F}_{\text{miner}} = \mathcal{F}_{\text{total}} - \mathcal{F}_{\text{burned}}$$

### 6.4 Aturan Presisi Input/Output Antarmuka (Precision Ingress/Egress)
1. **Validasi Input Luar (Ingress):** Saat pengguna memasukkan nilai dalam desimal AUR melalui CLI/API, konversi ke Quantum wajib mematuhi batas maksimum 8 desimal. Nilai dengan digit ke-9 di belakang koma yang bukan nol **WAJIB DITOLAK** dengan galat `InvalidPrecisionError`, bukan dibulatkan diam-diam.
2. **Formulasi Konversi Ingress:**
   $$\text{Quantum} = \text{AUR\_Integer} \times 10^8 + \left\lfloor \text{AUR\_Fractional} \times 10^{8 - \text{len}} \right\rfloor$$
3. **Formulasi Konversi Egress (Display):**
   $$\text{Display} = \left\lfloor \frac{\text{Quantum}}{10^8} \right\rfloor \,.\, \left( \text{Quantum} \pmod{10^8} \right)_{[8\text{ digits, zero-padded}]}$$

---

## 7. Rangkuman Spesifikasi Parameter Konstan Moneter

Untuk implementasi kode program, parameter moneter resmi protokol Aurion didefinisikan secara konstan:

```rust
// ==============================================================================
// AURION PROTOCOL CONSTANTS: MONETARY CORE
// ==============================================================================

/// Unit atomik per 1 AUR (10^8)
pub const QUANTA_PER_AUR: u128 = 100_000_000;

/// Batas maksimum suplai dalam satuan AUR (66 Juta)
pub const MAX_SUPPLY_AUR: u128 = 66_000_000;

/// Batas maksimum suplai dalam satuan Quantum (6,6 Kuadriliun)
pub const MAX_SUPPLY_QUANTA: u128 = 6_600_000_000_000_000;

/// Alokasi Creator dalam satuan Quantum (30%)
pub const CREATOR_ALLOCATION_QUANTA: u128 = 1_980_000_000_000_000;

/// Alokasi Developer dalam satuan Quantum (5%)
pub const DEVELOPER_ALLOCATION_QUANTA: u128 = 330_000_000_000_000;

/// Alokasi Community / Public (Pure Mining) dalam satuan Quantum (65%)
pub const COMMUNITY_MINING_QUANTA: u128 = 4_290_000_000_000_000;

/// Subsidi blok awal (Era 0) dalam satuan Quantum (10 AUR)
pub const INITIAL_BLOCK_SUBSIDY_QUANTA: u128 = 1_000_000_000;

/// Interval blok per era halving (~4 tahun pada target 60 detik)
pub const HALVING_INTERVAL_BLOCKS: u64 = 2_145_000;

/// Batas maksimum era halving sebelum subsidi menjadi nol
pub const MAX_HALVING_ERAS: u64 = 30;

/// Waktu target produksi per blok (dalam detik)
pub const TARGET_BLOCK_TIME_SECONDS: u64 = 60;

/// Periode kematangan transaksi coinbase (dalam blok)
pub const COINBASE_MATURITY_BLOCKS: u64 = 100;

/// Rasio pembakaran fee transaksi (20%)
pub const FEE_BURN_PERCENTAGE: u128 = 20;

/// Rasio fee untuk produser blok (80%)
pub const FEE_MINER_PERCENTAGE: u128 = 80;
```

---

## 8. Surat Ratifikasi Konstitusi Moneter

Spesifikasi Kebijakan Moneter ini merupakan penjabaran hukum dan matematika langsung dari **Bab 2 dan Bab 3 [AURION CONSTITUTION.md](file:///c:/Projects/aurion/AURION%20CONSTITUTION.md)**. 

Setiap modifikasi terhadap konstanta, formula emisi, rasio pembakaran, alokasi genesis, maupun aturan aritmetika bilangan bulat pada dokumen ini diklasifikasikan sebagai **Hard Fork Fundamental** yang memerlukan konsensus bulat (*unanimous constitutional consensus*) dari seluruh jaringan kedaulatan Aurion.
