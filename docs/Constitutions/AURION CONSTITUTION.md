# AURION CONSTITUTION
## Konstitusi Protokol Aurion (Aurion Blockchain Constitution)

> **Status:** RATIFIED & LOCKED CONSTITUTIONAL BASELINE  
> **Kategori:** Dokumen Tata Kelola Fundamental & Invarian Protokol Tertinggi  
> **Versi Protokol:** 1.0.0-BFT  
> **Jaringan:** Aurion Sovereign Mainnet (Chain ID: 1001)  
> **Cakupan:** Kedaulatan BFT, Kebijakan Moneter, Satuan Quantum, Provenance Aset, Topologi Jaringan, Storage ACID, dan Doktrin Zero-Mock

---

## BAB I: PRINSIP DASAR, IDENTITAS & KONSENSUS BFT

### Pasal 1: Kedaulatan & Konsensus BFT Deterministik

1. Jaringan Aurion beroperasi secara eksklusif di atas mekanisme konsensus **Byzantine Fault Tolerance (BFT)** berbasis ronde (*Round-Based BFT*) dengan finalitas kuorum. Seluruh terminologi dan mekanisme PoW diharamkan dari protokol konsensus.
   - **Finalitas bukan berbasis slot waktu tetap.** Finalitas diperoleh melalui **Sertifikat Kuorum (*Quorum Certificate / QC*)** dengan bobot suara $> \frac{2}{3}$ total validator aktif, yang dapat terbentuk pada ronde mana pun.
   - **Liveness dijaga oleh Round Timeout.** Bila Proposer tidak mengajukan blok dalam `round_timeout`, protokol menaikkan ronde dan membuka peluang proposing baru. Protokol karena itu tetap hidup meski Proposer offline.
   - **L-terminologi:** Istilah `miner` / `mining` dilarang muncul di lapisan produksi (kode, receipt, prompt pengguna, golden vector). Aturan ini ditegakkan otomatis oleh `tests/bft_purity_gate.rs`.
   - **L-skema fee:** Alokasi biaya transaksi bersifat tunggal — **100% ke validator BFT, 0% burn** (`FEE_VALIDATOR_PERCENTAGE = 100`, `FEE_BURN_PERCENTAGE = 0`). Skema historis "20% burn / 80% miner" telah dicabut dan tidak boleh divalidasi ulang.
   - Rujukan audit: `docs/operations/BFT_PURITY_AUDIT.md` (AUD-BFT-001).
2. **Penolakan Total Proof-of-Work:** Protokol Aurion tidak mengenal konsep penambangan (*mining*), kalkulasi tingkat kesulitan (*difficulty target*), maupun kompetisi komputasi hash untuk memproduksi blok.
   - **L-terminologi:** Istilah `miner` / `mining` dilarang muncul di lapisan produksi (kode, receipt, prompt pengguna, golden vector). Aturan ini ditegakkan otomatis oleh `tests/bft_purity_gate.rs`.
   - **L-skema fee:** Alokasi biaya transaksi bersifat tunggal — **100% ke validator BFT, 0% burn** (`FEE_VALIDATOR_PERCENTAGE = 100`, `FEE_BURN_PERCENTAGE = 0`). Skema historis "20% burn / 80% miner" telah dicabut dan tidak boleh divalidasi ulang.
   - Rujukan audit: `docs/operations/BFT_PURITY_AUDIT.md` (AUD-BFT-001).
3. **Penetapan Proposer — Opportunistik (*First-Valid-Block-Wins*):** Pada implementasi saat ini, **validator mana pun** dapat mengajukan blok untuk suatu ketinggian. Jaringan menerima proposal valid pertama yang berhasil mengumpulkan kuorum.
   - **Justifikasi:** Kesederhanaan dan latensi rendah untuk *validator set* tepercaya yang dipakai saat ini.
   - **Batas yang diakui:** Jadwal proposer *round-robin* deterministik **belum** ditegakkan di lapisan ledger. Untuk lingkungan publik tanpa kepercayaan, lihat Rencana Deterministic Leader Election pada `docs/operations/BFT_PURITY_AUDIT.md` §9.
4. **Finalitas Blok:** Sebuah blok dinyatakan sah, permanen, dan tidak dapat dibatalkan (*irreversible*) jika dan hanya jika telah memperoleh sertifikat kuorum kriptografis (*Quorum Certificate / QC*) dengan bobot suara lebih dari dua pertiga ($> \frac{2}{3}$) total validator aktif.

### Pasal 2: Hakikat Kedaulatan & Akuntabilitas

1. **Hakikat Ekosistem:** Aurion adalah ekosistem mata uang kripto berdaulat (*sovereign blockchain ecosystem*) yang bertumpu pada aturan konsensus deterministik mengenai penciptaan, kepemilikan, perpindahan, serta keabsahan aset.
2. **Prinsip Alur Asal-Usul Aset (*Asset Ownership Provenance*):** Aurion tidak memandang kepemilikan aset hanya sebagai saldo akhir sesaat (*current ownership state*), melainkan sebagai riwayat kepemilikan utuh yang dapat diverifikasi silsilahnya (*ownership provenance*) dari penerbitan awal.
3. **Akuntabilitas Kriptografis (*Cryptographic Accountability*):** Seluruh mutasi state wajib memiliki legitimasi bukti kriptografis Ed25519 dan Blake3 yang dapat diverifikasi oleh setiap simpul (*full node*) secara independen dan tanpa kepercayaan pihak ketiga (*trustless*).
4. **Transparansi Objektif:** Transparansi dalam Aurion dimaknai sebagai kepastian matematis bahwa asal-usul dan keabsahan suatu aset dapat diaudit secara independen tanpa mengekspos privasi pemilik aset kepada publik.

---

## BAB II: KEBIJAKAN MONETER & SUPLAI AUR

### Pasal 3: Pasokan Dasar Genesis (Blok 0)

1. **Pencetakan Awal Genesis:** Tepat **66.000.000 AUR** dicetak pada Blok 0 (Genesis).
2. **Alokasi Master Treasury:** Seluruh 66.000.000 AUR pada Blok 0 dialokasikan secara eksplisit ke dalam alamat dompet perbendaharaan master (*Master Treasury Account*) yang dideklarasikan secara kanonikal di dalam artefak `genesis.json`:
   - Alamat Kanonikal Master Treasury: `aur1jjtqrlqy9suehhltnzt2ml4zwsr8ukpyvvhm2gw899u0e0w22qusq0pjql`
3. **Presisi Desimal Tetap (9 Fixed Decimal Places):**
   - Satuan dasar terkecil protokol dikunci pada **9 desimal** ($1\text{ AUR} = 10^9\text{ unit terkecil}$).
   - Unit atomik terkecil disebut **Quantum** ($Q$).
   - Konversi kanonikal:
     $$\mathbf{1\ AUR = 1.000.000.000\ Quantum\ (10^9\ Q)}$$
4. **Skala Pasokan Genesis Internal:**
   $$\mathbf{S_{\text{genesis}}^{(Q)} = 66.000.000 \times 1.000.000.000 = 66.000.000.000.000.000\ Quantum\ (6,6 \times 10^{16}\ Q)}$$
5. **Larangan Aritmetika Floating-Point (Absolute Integer Invariant):**
   Seluruh kalkulasi saldo, transfer, fee, dan reward diwajibkan menggunakan integer murni (`u128`) untuk mencegah celah ketidakakuratan dan nondeterminisme aritmetika floating point (`f32`/`f64`).

### Pasal 4: Emisi Blok & Insentif Validator

1. **Pencetakan Berkelanjutan (*Block Reward*):** Pada setiap blok baru ($H > 0$) yang berhasil mencapai kuorum BFT dan ditulis ke ledger fisik, protokol secara otomatis mengeksekusi pencetakan koin baru sejumlah nilai reward tetap $R$:
   $$\mathbf{R = 1\ AUR = 1.000.000.000\ Quantum\ (10^9\ Q)}$$
2. **Eksekusi Tingkat Protokol:** Pencetakan reward blok dijalankan langsung oleh mesin eksekusi (`aurion-execution`) tanpa memerlukan transaksi eksternal (*zero external coinbase transaction*).
3. **Distribusi Reward Blok:**
   - **Proposer Porsi (20%):** Validator pembuat blok menerima bagian tetap sebesar $20\%$ dari $R$ ($200.000.000\ Q$) sebagai insentif perakitan transaksi dan proposal blok.
   - **Voter Porsi (80%):** Sisa $80\%$ dari $R$ ($800.000.000\ Q$) didistribusikan secara proporsional kepada seluruh validator aktif yang menandatangani *Precommit* pada *Quorum Certificate* (QC).
4. **Biaya Gas Transaksi (*Gas Fee*):** Biaya transaksi yang dibayarkan oleh pengguna dialirkan 100% ke validator pembuat blok untuk melengkapi insentif emisi.
5. **Aturan Faucet:** Seluruh fasilitas faucet jaringan wajib mengambil dana dari saldo operasional Master Treasury / Developer Allocation, dan dilarang mencetak koin baru di luar aturan emisi protokol.

---

## BAB III: TOPOLOGI JARINGAN & PEMISAHAN PERAN NODE

```text
┌──────────────────┐               ┌────────────────────────┐               ┌────────────────────┐
│ Validator Nodes  │ <---(P2P)---> │  P2P Anchor / Bootnode │ <---(P2P)---> │ Gateway / Explorer │
│  (Ed25519 BFT)   │               │   (TCP Port 7447)      │               │ (HTTP/WS Port 8080)│
└──────────────────┘               └────────────────────────┘               └────────────────────┘
                                               │                                       │
                                    Alamat: 116.212.72.89                   Nginx SSL Reverse Proxy
                                    (Isolasi Signing Key)                  bootnode.ratuaurion.store
```

### Pasal 5: Node Jangkar / P2P Bootnode (`116.212.72.89`)

1. **Fungsi Utama:** Bertindak sebagai *discovery anchor* dan *routing relay* transaksi serta pesan konsensus BFT antar validator.
2. **Koneksi Jaringan:** Wajib membuka soket pendengar TCP (`TCP Listener`) pada **Port 7447** secara persisten untuk menerima *inbound handshake* dari validator luar.
3. **Isolasi Kunci:** Bootnode dilarang keras menyimpan kunci privat penandatangan konsensus (*consensus signing key*). Kompromi pada bootnode tidak boleh membahayakan kuorum BFT.

### Pasal 6: Node Validator

1. **Fungsi Utama:** Menjalankan mesin status konsensus BFT (*Proposal*, *Prevote*, *Precommit*, *Commit*).
2. **Kunci Rahasia:** Menyimpan kunci privat Ed25519 untuk menandatangani proposal blok dan suara konsensus secara aman.
3. **Keamanan Port:** Port RPC/Gateway validator wajib ditutup dari akses publik dan hanya berkomunikasi melalui jaringan P2P ke Bootnode atau sesama peer terverifikasi.

### Pasal 7: Gateway API & Telemetri

1. **Fungsi Utama:** Melayani kueri status ledger, data blok, histori transaksi, dan stream WebSocket telemetri ke penjelajah blok (`aurion-explorer`).
2. **Integrasi Nginx:** Gateway lokal mengikat port internal **127.0.0.1:8080**, yang diteruskan oleh Nginx dengan sertifikat SSL ke domain publik `bootnode.ratuaurion.store`.

---

## BAB IV: PENYIMPANAN LEDGER, STATE TRANSITION & PROVENANCE

### Pasal 8: Mesin Status & Database Fisik

1. **Storage Engine:** Protokol Aurion menggunakan mesin basis data terdistribusi bertipe ACID (`redb`) dalam format biner murni murni Rust (*zero C++ dependency*).
2. **Invarian Integritas:** Database dilarang keras menyimpan blok yang tidak memiliki validasi *Quorum Certificate* yang sah.
3. **Keterpisahan State:** Struktur data dipisahkan secara tegas:
   - *State Tree:* Saldo akun, nonce, dan penyimpanan kontrak dalam struktur 256-bit Sparse Merkle Tree (SMT) berbasis Blake3.
   - *Block Store:* Header blok, body transaksi, dan sertifikat kuorum (QC) penutupan blok.

### Pasal 9: Provenance dan Modul Identifikasi Aset

1. **Reward Provenance Identifier (RPI):** Penerbitan AUR dari block reward menghasilkan penanda asal-usul yang diturunkan secara deterministik dari konteks blok yang sah:
   $$\text{RPI} = \text{Blake3}(\text{ChainID} \parallel \text{BlockHash} \parallel \text{BlockHeight} \parallel \text{ValidatorPubkey})$$
2. **Ketahanan Validitas Sepanjang Masa:** Mutasi kepemilikan memperbarui *current state*, tetapi tidak pernah memutus rantai silsilah penerbitan aset (*issuance provenance*).
3. **Asset Identification Module:** Protokol menolak identifier arbitrer yang diajukan secara sepihak di luar fungsi derivasi konsensus resmi.

---

## BAB V: DOKTRIN INTEGRITAS KODE & ANTI-CACAT PRODUK

### Pasal 10: Larangan Mutlak Mode Tiruan (*Zero-Mock Policy*)

1. **Eliminasi Kode Tiruan:** Segala bentuk implementasi mode tiruan (`--dev`), *mock consensus*, kluster proses simulasi dalam satu server fisik publik, dan kunci privat hardcoded **diharamkan secara mutlak** dari alur rilis biner resmi.
2. **Penegakan Lingkungan Publik:** Lingkungan publik VPS (`116.212.72.89`) hanya boleh mengeksekusi biner dengan arsitektur jaringan P2P nyata dan konfigurasi `genesis.json` resmi.
3. **Architectural Contamination:** Pelanggaran terhadap pasal ini dianggap sebagai cacat integritas produk (*architectural contamination*) yang membatalkan validitas rilis.

---

## BAB VI: ASAS-ASAS TERTINGGI (SUPREME INVARIANTS)

Konstitusi Aurion mengikat seluruh simpul, pengembang, validator, dan pengguna melalui asas-asas tertinggi yang tidak dapat dilanggar:

> ### Invarian Keabsahan State:
> **"Tidak ada satu unit AUR pun yang dapat diakui sebagai bagian dari state protokol yang sah, kecuali penciptaan dan asal-usulnya (provenance) dapat dibuktikan secara matematis berdasarkan aturan konsensus Aurion."**

> ### Invarian Sumber Otoritas:
> **"Perangkat lunak simpul (node) tidak menentukan apa itu aset yang sah; Protokol konsensus Aurion-lah yang menentukan apa itu aset yang sah."**

> ### Invarian Integritas Moneter Genesis & Emisi:
> **"Tepat 66.000.000 AUR (setara dengan 66.000.000.000.000.000 Quantum pada skala sembilan desimal 10^9) dicetak pada Blok 0 ke Master Treasury Account. Pada setiap blok baru, protokol menerbitkan reward tetap R sebesar 1 AUR kepada validator kuorum BFT. Segala bentuk pencetakan aset di luar batasan ini adalah pembatalan langsung terhadap konsensus Aurion."**

> ### Invarian Determinisme Aritmetika:
> **"Konsensus moneter Aurion beroperasi secara eksklusif dengan bilangan bulat Quantum (u128). Penggunaan floating-point pada lapisan konsensus adalah pelanggaran konstitusional terhadap determinisme jaringan."**

> ### Invarian Integritas Kode:
> **"Biner resmi Aurion dilarang memuat mode tiruan, mock consensus, atau kunci privat hardcoded. Jaringan produksi wajib beroperasi murni di atas protokol P2P dan konsensus nyata."**
