# KONSTITUSI PROTOKOL AURION (AURION BLOCKCHAIN CONSTITUTION)

**Status:** Draf Spesifikasi Fondasi

**Versi Protokol:** 1.0.0-BFT

**Jaringan:** Aurion Sovereign Mainnet (Chain ID: 1001)

---

## BAB I: PRINSIP DASAR & INVARIAN PROTOKOL

### Pasal 1: Kedaulatan & Konsensus BFT Deterministik

1. Jaringan Aurion beroperasi di atas mekanisme konsensus **Byzantine Fault Tolerance (BFT)** deterministik dengan *single-slot finality*.
2. **Penolakan Total Proof-of-Work:** Protokol Aurion tidak mengenal konsep penambangan (*mining*), kalkulasi tingkat kesulitan (*difficulty target*), maupun kompetisi komputasi hash untuk memproduksi blok.
   - **L-terminologi:** Istilah `miner` / `mining` dilarang muncul di lapisan produksi (kode, receipt, prompt pengguna, golden vector). Aturan ini ditegakkan otomatis oleh `tests/bft_purity_gate.rs`.
   - **L-skema fee:** Alokasi biaya transaksi bersifat tunggal — **100% ke validator BFT, 0% burn** (`FEE_VALIDATOR_PERCENTAGE = 100`, `FEE_BURN_PERCENTAGE = 0`). Skema historis "20% burn / 80% miner" telah dicabut dan tidak boleh divalidasi ulang.
   - Rujukan audit: `docs/operations/BFT_PURITY_AUDIT.md` (AUD-BFT-001).
3. **Pencetakan Blok:** Blok hanya dapat diajukan oleh satu validator (*Proposer*) yang ditunjuk secara bergilir berdasarkan jadwal deterministik pada ketinggian ($H$) dan ronde ($R$) tertentu.
4. **Finalitas Blok:** Sebuah blok dinyatakan sah, permanen, dan tidak dapat dibatalkan (*irreversible*) jika dan hanya jika telah memperoleh sertifikat kuorum kriptografis (*Quorum Certificate / QC*) dengan bobot suara lebih dari dua pertiga ($> \frac{2}{3}$) total validator aktif.

---

## BAB II: KEBIJAKAN MONETER & SUPLAI AUR

### Pasal 2: Pasokan Dasar Genesis (Blok 0)

1. **Pencetakan Awal:** Tepat **66.000.000 AUR** dicetak pada Blok 0 (Genesis).
2. **Alokasi Awal:** Seluruh 66.000.000 AUR pada Blok 0 dialokasikan secara eksplisit ke dalam alamat dompet perbendaharaan/master (*Master Treasury Account*) yang dideklarasikan di dalam file `genesis.json`.
3. **Presisi Desimal:** Satuan dasar terkecil protokol dikunci pada **9 desimal** ($1\text{ AUR} = 10^9\text{ unit terkecil}$). Seluruh kalkulasi saldo diwajibkan menggunakan integer murni (`u128`) untuk mencegah celah ketidakakuratan aritmatika floating point.

### Pasal 3: Emisi Blok & Insentif Validator

1. **Pencetakan Berkelanjutan (*Block Reward*):** Pada setiap blok baru ($H > 0$) yang berhasil mencapai kuorum BFT dan ditulis ke ledger, protokol secara otomatis mengeksekusi pencetakan koin baru sejumlah nilai reward tetap $R$.
2. **Eksekusi Tingkat Protokol:** Pencetakan reward blok dijalankan langsung oleh mesin eksekusi (`aurion-execution`) tanpa memerlukan transaksi eksternal.
3. **Distribusi Reward:**
   * **Proposer Porsi:** Validator pembuat blok menerima persentase tetap sebagai upah perakitan transaksi.
   * **Voter Porsi:** Sisa reward didistribusikan secara proporsional kepada seluruh validator yang menandatangani *Precommit* pada *Quorum Certificate*.
4. **Biaya Gas Transaksi:** Biaya transaksi (*gas fee*) yang dibayarkan oleh pengguna dialirkan ke validator blok tersebut untuk melengkapi insentif emisi.

---

## BAB III: TOPOLOGI JARINGAN & PEMISAHAN PERAN NODE

```
[ Validator Nodes ] <---> [ P2P Anchor / Bootnode ] <---> [ Gateway / Explorer ]
  (Port P2P)                   (TCP Port 7447)              (HTTP/WS Port 8080)
```

### Pasal 4: Node Jangkar / P2P Bootnode (`116.212.72.89`)

1. **Fungsi Utama:** Bertindak sebagai *discovery anchor* dan *routing relay* transaksi serta pesan konsensus BFT.
2. **Koneksi Jaringan:** Wajib membuka soket pendengar TCP (`TCP Listener`) pada **Port 7447** secara persisten untuk menerima *inbound handshake* dari validator luar.
3. **Isolasi Kunci:** Bootnode dilarang menyimpan kunci privat penandatangan konsensus (*consensus signing key*). Kompromi pada bootnode tidak boleh membahayakan kuorum BFT.

### Pasal 5: Node Validator

1. **Fungsi Utama:** Menjalankan mesin status konsensus BFT (*Proposal*, *Prevote*, *Precommit*, *Commit*).
2. **Kunci Rahasia:** Menyimpan kunci privat kriptografis untuk menandatangani proposal dan suara blok.
3. **Keamanan Port:** Port RPC/Gateway validator harus ditutup dari akses publik dan hanya berkomunikasi melalui jaringan P2P ke Bootnode atau sesama peer terverifikasi.

### Pasal 6: Gateway API & Telemetri

1. **Fungsi Utama:** Melayani kueri status ledger, data blok, histori transaksi, dan stream WebSocket telemetri ke penjelajah blok (`aurion-explorer`).
2. **Integrasi Nginx:** Gateway lokal mengikat port internal **127.0.0.1:8080**, yang diteruskan oleh Nginx dengan sertifikat SSL ke domain publik `bootnode.ratuaurion.store`.

---

## BAB IV: PENYIMPANAN LEDGER & STATE TRANSITION

### Pasal 7: Mesin Status & Database Fisik

1. **Storage Engine:** Protokol Aurion menggunakan mesin basis data terdistribusi bertipe ACID (`redb`) dalam format biner murni.
2. **Invarian Integritas:** Database dilarang menyimpan blok yang tidak memiliki validasi *Quorum Certificate* yang sah.
3. **Keterpisahan State:** Struktur data dipisahkan secara tegas:
   * *State Tree:* Saldo akun, nonce, dan penyimpanan kontrak.
   * *Block Store:* Header blok, body transaksi, dan QC penutupan blok.

---

## BAB V: DOKTRIN INTEGRITAS KODE & ANTI-CACAT PRODUK

### Pasal 8: Larangan Mutlak Mode Tiruan (*Zero-Mock Policy*)

1. Segala bentuk implementasi mode tiruan (`--dev`), *mock consensus*, kluster proses simulasi dalam satu server fisik publik, dan kunci privat hardcoded **diharamkan secara mutlak** dari alur rilis biner resmi.
2. Lingkungan publik VPS (`116.212.72.89`) hanya boleh mengeksekusi biner dengan arsitektur jaringan P2P nyata dan konfigurasi `genesis.json` resmi.
3. Pelanggaran terhadap pasal ini dianggap sebagai cacat integritas produk (*architectural contamination*).
