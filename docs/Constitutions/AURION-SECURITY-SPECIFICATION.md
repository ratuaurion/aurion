# 10 — AURION SECURITY SPECIFICATION
## Spesifikasi Formal Model Ancaman Kedaulatan, Analisis Vektor Serangan, dan Protokol Pertahanan

> **Hierarki Dokumen:**  
> `00 — AURION CONSTITUTION` $\longrightarrow$ `01 — MONETARY` $\longrightarrow$ `02 — CONSENSUS` $\longrightarrow$ `03 — STATE TRANSITION` $\longrightarrow$ `04 — TRANSACTION` $\longrightarrow$ `05 — CRYPTOGRAPHY` $\longrightarrow$ `06 — SERIALIZATION & WIRE` $\longrightarrow$ `07 — GENESIS` $\longrightarrow$ `08 — VALIDATOR & STAKING` $\longrightarrow$ `09 — GOVERNANCE` $\longrightarrow$ **`10 — SECURITY SPECIFICATION`**  
>
> **Status:** AUDITED & LOCKED SPECIFICATION  
> **Klasifikasi:** Arsitektur Keamanan & Mitigasi Serangan Layer 0 - Layer 2 (Security Core)  
> **Versi:** 1.0.0-PROD  
> **Sifat Ketetapan:** Formal, Eksplisit, Berbasis Teorema Kriptografis

---

## 1. Model Musuh Formal (Formal Adversary Model)

Keamanan protokol Aurion dianalisis menggunakan model musuh gabungan **Dolev-Yao Network Adversary** dan **Byzantine Fault Adversary**:

1. **Kendali Jaringan Musuh (*Network Control*):**  
   Musuh dapat membaca, menunda, mengacak urutan, atau menduplikasi pesan jaringan pada lapisan P2P, namun tidak dapat memecahkan primitif kriptografi (Blake3 dan Ed25519) dalam batas komputasi polinomial.
2. **Kekuatan Byzantine Musuh ($f$):**  
   Musuh dapat mengendalikan sejumlah simpul validator korup dengan total bobot suara $f$:
   - *Batas Keamanan Konsensus:* $f < \frac{W_E}{3}$ menjamin keamanan (*safety*) dan ketiadaan percabangan ganda secara mutlak.
   - *Batas Ketersediaan Konsensus:* $f < \frac{W_E}{3}$ di bawah model sinkronisasi parsial pasca-GST menjamin kelangsungan (*liveness*).

---

## 2. Metodologi Analisis Ancaman Enam Tahap

Setiap vektor ancaman dalam spesifikasi ini dianalisis dan dimitigasi melalui kerangka kerja enam tahap deterministik:

$$\mathbf{Threat} \longrightarrow \mathbf{Assumption} \longrightarrow \mathbf{Defense} \longrightarrow \mathbf{Invariant} \longrightarrow \mathbf{Detection} \longrightarrow \mathbf{Recovery}$$

---

## 3. Matriks 13 Vektor Ancaman Utama (The 13 Threat Vectors)

### 3.1 Vektor 1: Byzantine Nodes ($f < \frac{1}{3} W_E$)
- **Threat:** Sejumlah validator jahat mengirimkan blok tidak valid, abstain dari voting, atau menunda propagasi pesan untuk mengganggu konsensus.
- **Assumption:** Total bobot suara Byzantine dibatasi ketat $f < \left\lfloor \frac{W_E}{3} \right\rfloor$.
- **Defense:** Skema dua fase voting Aurion-BFT (Pre-vote & Pre-commit) mewajibkan kuorum supermayoritas $\mathcal{Q} > \frac{2}{3} W_E$. Suara Byzantine yang minoritas tidak pernah mampu membentuk kuorum.
- **Invariant:** $\text{CommitCertificate}(B) \implies \text{Persetujuan Validator Jujur} > \frac{1}{3} W_E$.
- **Detection:** Timeout putaran otomatis mendeteksi ketiadaan proposal atau suara dari validator yang lambat/mati.
- **Recovery:** Progresi putaran ($R \to R+1$) memilih Proposer baru secara deterministik; validator offline dikenakan status Jailed jika absen $> 500$ blok.

---

### 3.2 Vektor 2: Serangan Sybil (Sybil Attacks)
- **Threat:** Penyerang menciptakan jutaan identitas simpul palsu untuk mendominasi voting konsensus atau menguasai topologi P2P.
- **Assumption:** Penyerang memiliki sumber daya komputasi dan alamat IP tak terbatas, namun modal koin AUR miliknya terbatas.
- **Defense:** Hak voting konsensus tidak didasarkan pada jumlah IP/node, melainkan pada **Agunan Mandiri Minimum (*Self-Bond*) sebesar $10.000\ \text{AUR}$ ($10^{13}\ Q$)** per validator.
- **Invariant:** $\forall v \in \mathcal{V}_E, \quad \text{SelfBond}(v) \ge 10.000.000.000.000\ \text{Quantum}$.
- **Detection:** Mesin STF menolak pendaftaran validator tanpa transfer agunan $10.000\ \text{AUR}$ yang sah.
- **Recovery:** Node palsu tanpa agunan hanya menjadi peer relai pasif tanpa hak suara konsensus.

---

### 3.3 Vektor 3: Serangan Gerhana (Eclipse Attacks)
- **Threat:** Penyerang mengisolasi simpul korban dengan memenuhi seluruh slot koneksi peer korban menggunakan simpul jahat penyerang.
- **Assumption:** Penyerang mengontrol sebagian rentang subnet IP tetangga korban.
- **Defense:**
  1. *Diversitas Subnet IP:* Simpul membatasi maksimum 1 koneksi peer keluar per subnet `/16` IPv4 atau `/32` IPv6.
  2. *Anchor Bootnodes:* Simpul memelihara koneksi persisten ke sekurang-kurangnya 4 Genesis Bootnodes terverifikasi.
  3. *Peer Exchange (PEX):* Pengacakan daftar peer secara berkala.
- **Invariant:** $\text{SubnetDiversity}(\text{OutboundPeers}) \ge 8\ \text{Subnets Unik}$.
- **Detection:** Pemantauan laju kedatangan blok: jika sebuah simpul tertinggal dari Median Time Past jaringan, simpul memicu penemuan peer paksa (*Forced Discovery*).
- **Recovery:** Simpul menyambungkan ulang koneksi keluar ke daftar alamat *Hardcoded Genesis Seeds*.

---

### 3.4 Vektor 4: Pemisahan Jaringan Fisik (Network Partition)
- **Threat:** Jalur komunikasi antar-wilayah terputus sehingga jaringan terbelah menjadi dua atau lebih partisi terisolasi.
- **Assumption:** Tidak ada partisi tunggal yang memiliki akses ke $\ge \mathcal{Q}$ ($> \frac{2}{3} W_E$) bobot suara.
- **Defense:** Prinsip **Safety over Liveness**. Karena kuorum $\mathcal{Q} > \frac{2}{3}$ tidak dapat dicapai pada kedua belah partisi, produksi blok berhenti secara aman (*halt*). Tidak ada percabangan ganda (*split-brain*) yang dapat terjadi.
- **Invariant:** $\big| \left\{ \text{Partisi dengan Bobot } \ge \mathcal{Q} \right\} \big| \le 1$.
- **Detection:** Putaran berulang kali mengalami *TimeoutPrecommit* tanpa pembentukan Commit Certificate.
- **Recovery:** Begitu partisi fisik tersambung kembali, simpul saling bertukar sertifikat putaran tertinggi dan konsensus langsung melanjutkan produksi blok normal.

---

### 3.5 Vektor 5: Serangan Penyiaran Ulang (Replay Attacks)
- **Threat:** Penyerang mencegat transaksi sah dan menyiarkannya kembali pada rantai lain (Testnet/Fork) atau berulang kali pada rantai yang sama.
- **Assumption:** Kunci privat pengirim tidak bocor; data transaksi dapat dibaca publik.
- **Defense:** **Dual-Tier Replay Protection** sesuai Dokumen 04:
  1. `chain_id` mengikat tanda tangan ke rantai spesifik.
  2. `nonce` sekuensial mengikat transaksi ke riwayat akun spesifik.
- **Invariant:** $\text{Nonce}(T_x) == \mathcal{A}[\text{sender}].\text{nonce}$.
- **Detection:** Validasi stateful menolak transaksi dengan `nonce != account.nonce` atau `chain_id != CURRENT_CHAIN_ID`.
- **Recovery:** Transaksi duplikat dibuang seketika oleh mempool tanpa mutasi saldo.

---

### 3.6 Vektor 6: Pengeluaran Ganda (Double Spend Attacks)
- **Threat:** Penyerang mencoba membelanjakan koin yang sama untuk dua penerima berbeda dalam waktu bersamaan.
- **Assumption:** Penyerang tidak memiliki $\ge \frac{2}{3} W_E$ bobot konsensus.
- **Defense:**
  1. *Account Nonce Serialization:* Hanya satu transaksi dengan nonce $N$ yang dapat dieksekusi dari akun yang sama.
  2. *Round-Based Finality:* Transaksi yang telah masuk ke dalam blok final berstatus abadi dan tidak dapat digantikan oleh blok alternatif.
- **Invariant:** $\forall \alpha, \forall N, \quad \big| \left\{ T_x \mid T_x.\text{sender} == \alpha \land T_x.\text{nonce} == N \text{ dieksekusi} \right\} \big| \le 1$.
- **Detection:** Mesin STF menolak transaksi kedua yang menggunakan nonce yang telah terpakai.
- **Recovery:** Saldo pengirim didebet tepat satu kali; percobaan transaksi kedua gugur.

---

### 3.7 Vektor 7: Sensor Transaksi (Transaction Censorship)
- **Threat:** Proposer jahat secara sengaja menolak memasukkan transaksi pengguna tertentu ke dalam proposal blok.
- **Assumption:** Sedikitnya $\ge \frac{2}{3} W_E$ validator adalah simpul jujur.
- **Defense:**
  1. *Rotasi Pengusul Dinamis:* Proposer berganti setiap putaran dan setiap blok secara terbobot pseudo-random. Proposer yang menyensor hanya dapat menahan transaksi selama gilirannya sendiri.
  2. *Mempool Gossip Mesh:* Transaksi disebarkan ke seluruh validator melalui jaringan gossip, sehingga proposer jujur berikutnya akan memasukkan transaksi tersebut.
- **Invariant:** $\text{Probabilitas Sensor Selama } K \text{ Blok} \le \left( \frac{f}{W_E} \right)^K \longrightarrow 0 \quad (\text{untuk } K \to \infty)$.
- **Detection:** Transaksi dengan biaya memadai bertahan di mempool melebihi batas waktu wajar.
- **Recovery:** Transaksi dimasukkan ke dalam rantai oleh proposer jujur berikutnya.

---

### 3.8 Vektor 8: Serangan Jarak Jauh (Long-Range Attacks)
- **Threat:** Mantan validator yang telah menarik seluruh agunannya menggunakan kunci privat lamanya untuk membuat rantai alternatif panjang dari blok masa lalu.
- **Assumption:** Kunci lama validator telah tidak memiliki nilai ekonomi di masa kini.
- **Defense:**
  1. *Periode Unbonding Panjang:* $\tau_{\text{unbond}} = 14\ \text{Epoch}$ ($\approx 97\ \text{hari}$).
  2. *Weak Subjectivity Checkpoint:* Simpul baru yang melakukan sinkronisasi wajib memverifikasi *StateRoot Checkpoint* yang tidak lebih tua dari $\tau_{\text{unbond}}$. Rantai alternatif yang bercabang sebelum checkpoint ditolak langsung tanpa evaluasi.
- **Invariant:** $\text{ForkDepth}(\text{AlternativeChain}) \le \tau_{\text{unbond}}$.
- **Detection:** Simpul menerima proposal blok dengan titik percabangan lebih tua dari checkpoint subjektivitas lemah.
- **Recovery:** Simpul membuang rantai alternatif dan memutus koneksi peer yang menyiarkannya.

---

### 3.9 Vektor 9: Kolusi Validator Tingkat Tinggi ($\frac{1}{3} \le f < \frac{2}{3}$)
- **Threat:** Penyerang menguasai lebih dari sepertiga namun kurang dari dua pertiga bobot voting untuk menghentikan konsensus (*Liveness Halting Attack*).
- **Assumption:** Penyerang rela kehilangan pendapatan hadiah blok demi melumpuhkan jaringan.
- **Defense:**
  1. *Safety Tetap Terjaga:* Kolusi $< \frac{2}{3}$ mustahil menciptakan rantai ganda atau mencuri dana.
  2. *Inactivity Leak / Slashing Window:* Validator yang mogok voting secara otomatis dikenakan sanksi denda dan dikeluarkan ke status *Jailed* setelah jendela toleransi habis, sehingga total bobot efektif menyusut dan validator jujur kembali menguasai $> \frac{2}{3}$ suara aktif.
- **Invariant:** $\text{Safety}(f < 2/3) \equiv \mathbf{TRUE}$.
- **Detection:** Akumulasi suara Pre-commit gagal mencapai kuorum $\mathcal{Q}$ selama beberapa putaran berturut-turut.
- **Recovery:** Penurunan bobot validator mogok memulihkan kuorum bagi validator jujur.

---

### 3.10 Vektor 10: Ekuivokasi Kriptografis (Double Proposing & Double Voting)
- **Threat:** Validator jahat menandatangani dua pesan konsensus bertentangan pada tinggi dan putaran yang sama untuk memicu kebuntuan atau percabangan.
- **Assumption:** Tanda tangan digital Ed25519 tidak dapat dipalsukan tanpa kunci privat.
- **Defense:** **Hukuman Pemusnahan Agunan Mutlak (100% Slashing & Tombstone)** sesuai Dokumen 08.
- **Invariant:** $\text{VerifiedFraudProof}(\mathcal{E}) \implies \text{Slash}(v, 100\%) \land \text{Tombstone}(v)$.
- **Detection:** Transaksi `MsgSubmitEvidence` menyerahkan dua tanda tangan sah dari validator yang sama atas hash blok berbeda.
- **Recovery:** State machine menyita 100% agunan validator, membakar 95% koin secara deflasi, dan memasukkan validator ke daftar hitam abadi.

---

### 3.11 Vektor 11: Serangan Denial of Service (DoS & Memory Exhaustion)
- **Threat:** Penyerang membanjiri simpul dengan transaksi sampah, paket wire raksasa, atau spam koneksi TCP untuk memicu kehabisan memori (*OOM Crash*).
- **Assumption:** Penyerang memiliki bandwidth tinggi.
- **Defense:**
  1. *Plafon Alokasi Memori Wire:* Maksimum payload dibatasi ketat (Maks 4 MB untuk blok, 64 KB untuk transaksi).
  2. *Anti-Spam Transaction Fee:* Setiap transaksi wajib membayar biaya $\ge \text{MinFeePerByte} \times \text{Size}$.
  3. *Token Bucket Rate Limiting:* Pembatasan 200 pesan/detik per peer.
- **Invariant:** $\text{AllocMemory}(\text{InboundMessage}) \le \text{MaxPayloadCeiling}$.
- **Detection:** Firewall aplikasi mendeteksi peer yang melanggar batas laju atau mengirim payload melebihi plafon.
- **Recovery:** Peer langsung dikenakan penalti skor $-100$ dan diblokir IP-nya selama 24 jam.

---

### 3.12 Vektor 12: Kerusakan State & Non-Determinisme (State Corruption & Non-Determinism)
- **Threat:** Kesalahan implementasi kode (bug compiler, perbedaan pembulatan float, race condition) menyebabkan simpul berbeda menghasilkan saldo berbeda dari blok yang sama.
- **Assumption:** Perangkat keras simpul bebas dari kerusakan fisik ekstrem (*bit-rot* ditangani checksum).
- **Defense:**
  1. *Zero-Float Mandate Konstitusional:* Larangan mutlak tipe `f32`/`f64`. Seluruh kalkulasi wajib integer Quantum berpengecekan (`checked_*`).
  2. *Kriptografis State Root:* Setiap blok memuat `state_root` (Blake3 Sparse Merkle Tree). Simpul dengan state berbeda tidak akan cocok dengan hash header blok.
- **Invariant:** $\text{LocalComputedStateRoot} == B.\text{state\_root}$.
- **Detection:** Perbedaan hash state root saat validasi Tahap 7 STF.
- **Recovery:** Simpul seketika mengaktifkan mode **Terminal Fail-Stop Lockout**, menolak memperbarui database lokal untuk mencegah penyebaran state korup.

---

### 3.13 Vektor 13: Asumsi Kegagalan Kriptografi Masa Depan (Cryptographic Failure Contingency)
- **Threat:** Penemuan cacat matematis teoritis pada kurva Edwards25519 atau algoritma kompresi Blake3.
- **Assumption:** Degradasi kriptografi terdeteksi oleh komunitas riset sebelum eksploitasi massal terjadi.
- **Defense:**
  1. *Field Ekstensi Cadangan Header:* Header blok Aurion menyediakan kolom cadangan masa depan (*reserved padding*).
  2. *Agility Pemisahan Domain:* Seluruh primitif dipisahkan oleh Domain Separation Tag (DST) eksplisit, memfasilitasi migrasi versi bertahap via proposal tata kelola Tipe B.
- **Invariant:** $\text{CryptoSecurityMargin} \ge 128\ \text{Bits}$.
- **Detection:** Pemantauan publikasi kriptoanalisis internasional.
- **Recovery:** Peluncuran upgrade *Protocol Version Signal* terencana menuju algoritma pasca-kuantum (*Post-Quantum Cryptography*).

---

### 3.14 Vektor 14: Pencemaran Simulasi & Mode Tiruan (Zero-Mock Policy Violation)
- **Threat:** Penggunaan flag mode tiruan (`--dev`), mesin *mock consensus*, kluster proses simulasi multi-node dalam satu server fisik publik, atau kunci privat hardcoded pada rilis biner produksi.
- **Assumption:** Biner resmi Aurion wajib steril 100% dari kode tiruan (*architectural contamination*).
- **Defense:**
  1. *Pemberlakuan Doktrin Zero-Mock:* Biner resmi rilis menolak flag `--dev`, simulasi konsensus palsu, dan kunci privat bawaan (*hardcoded keys*).
  2. *Topologi P2P Nyata:* Lingkungan publik VPS (`116.212.72.89`) hanya boleh mengeksekusi biner dengan soket TCP nyata (Port 7447) dan `genesis.json` resmi.
- **Invariant:** $\text{IsProductionBinary} \implies \text{MockEngineForbidden} \land \text{HardcodedKeysForbidden}$.
- **Detection:** Pemeriksaan runtime validator menolak konfigurasi tiruan saat inisialisasi node.
- **Recovery:** Simpul seketika menghentikan eksekusi (*panic halt*) jika mendeteksi parameter mock di jaringan produksi.

---

## 4. Arsitektur Penghentian Kegagalan Integritas (Fail-Stop Architecture)

Prinsip tertinggi keamanan Aurion adalah:

> **"Sebuah simpul yang mati (*fail-stop*) jauh lebih berharga dan aman bagi kedaulatan jaringan daripada sebuah simpul yang diam-diam menjalankan state yang salah."**

Jika terjadi pelanggaran terhadap salah satu dari Invarian Moneter `[INV-MON-01]` hingga `[INV-MON-06]`, pelanggaran konservasi nilai, atau manipulasi state root yang lolos verifikasi lokal:
1. Simpul wajib mengunci database `ledger` (`redb`) ke mode *Read-Only*;
2. Memancarkan log audit darurat berkategori `CRITICAL_SECURITY_PANIC`;
3. Menghentikan proses eksekusi seketika (*abort runtime*) tanpa melakukan rollback otomatis yang berisiko.
