# AURION CONSTITUTION

> **Status:** RATIFIED & LOCKED CONSTITUTIONAL BASELINE  
> **Kategori:** Dokumen Tata Kelola Fundamental & Invarian Protokol  
> **Cakupan:** Prinsip Identitas, Kebijakan Moneter, Satuan Kuantum, Provenance Aset, dan Batas Arsitektural Konsensus

---

## 1. Identitas dan Prinsip Fundamental (Identity & Core Principles)

### 1.1 Hakikat Aurion
Aurion adalah sebuah **ekosistem cryptocurrency** yang dibangun di atas fondasi aturan konsensus yang ketat, deterministik, dan eksplisit mengenai penciptaan, kepemilikan, perpindahan, serta keabsahan aset.

Aurion menempatkan **akuntabilitas (accountability), keterbukaan audit (auditability), dan transparansi asal-usul aset (transparency of asset provenance)** sebagai pilar fundamental yang tidak dapat diganggu gugat.

### 1.2 Prinsip Kepemilikan dan Alur Asal-Usul Aset (Asset Ownership Principle)
Aurion tidak memandang kepemilikan aset hanya sebagai saldo akhir sesaat (*current ownership state*), melainkan sebagai **riwayat kepemilikan utuh yang dapat diverifikasi asal-usulnya** (*ownership provenance*).

Setiap aset di dalam ekosistem Aurion harus memiliki struktur data dan aturan protokol yang memungkinkan sistem konsensus menentukan secara definitif:
1. Siapa yang memiliki atau mengendalikan hak atas aset pada state saat ini;
2. Bagaimana hak kepemilikan tersebut pertama kali diperoleh secara sah;
3. Bagaimana hak kepemilikan tersebut berpindah dari satu pihak ke pihak lain;
4. Kapan dan pada blok/transaksi mana perpindahan kepemilikan terjadi;
5. Berdasarkan aturan protokol mana setiap perubahan state dinyatakan sah.

### 1.3 Akuntabilitas (Accountability)
Arsitektur Aurion mewajibkan seluruh kepemilikan dan mutasi aset dapat dipertanggungjawabkan berdasarkan aturan konsensus protokol. Setiap perubahan state wajib memiliki legitimasi bukti kriptografis yang dapat diverifikasi oleh seluruh simpul (*node*) jaringan secara independen.

### 1.4 Kemampuan Audit (Auditability)
Kemampuan audit menyeluruh terhadap pasokan moneter dan sirkulasi aset merupakan bagian intrinsik dari desain inti protokol, bukan sekadar fitur tambahan (*add-on*). Protokol wajib menyediakan data deterministik yang memungkinkan audit independen terhadap seluruh riwayat state kapan pun diperlukan.

### 1.5 Transparansi Objektif (Transparency of Provenance)
Transparansi dalam Aurion **tidak berarti membuka identitas pribadi atau privasi pemilik aset kepada publik**. 

Transparansi dimaknai sebagai **verifiability of asset legitimacy and provenance**—yakni kepastian matematis bahwa asal-usul, penciptaan, dan keabsahan suatu aset dapat diverifikasi secara publik dan independen berdasarkan aturan serta bukti kriptografis yang ditetapkan oleh protokol.

---

## 2. Kebijakan Moneter dan Alokasi Genesis (Monetary Origin & Allocation)

### 2.1 Asal-Usul Moneter (Monetary Origin)
Pada keadaan awal (*genesis*), **tidak terdapat aset AUR yang beredar sebelum mekanisme penerbitan yang sah dimulai**.

Aset AUR hanya dapat tercipta melalui mekanisme penciptaan yang secara eksplisit diatur, diakui, dan diizinkan oleh protokol Aurion. Sumber penciptaan AUR ditentukan secara formal oleh konsensus dan tidak boleh bergantung pada tindakan subjektif operator node, pengembang perangkat lunak, maupun entitas mana pun di luar aturan protokol.

### 2.2 Batas Suplai Maksimum (Maximum Supply Cap)
Aurion menetapkan batas mutlak jumlah seluruh aset AUR yang dapat diterbitkan sepanjang masa sebesar:

$$\mathbf{66.000.000\ AUR}$$

Batas maksimum 66.000.000 AUR bersifat mengikat secara permanen dan tidak dapat dilampaui melalui proses penambangan (*mining*), *block reward*, penyesuaian perangkat lunak simpul, maupun intervensi pihak mana pun, kecuali melalui amandemen protokol resmi yang diizinkan oleh mekanisme tata kelola (*governance*) konstitusional Aurion.

### 2.3 Pembagian Alokasi Genesis (Genesis Allocation)
Sejak blok genesis, Aurion menetapkan distribusi hak atas total suplai 66.000.000 AUR secara transparan dan tertulis di dalam *genesis state*:

| Alokasi | Persentase | Jumlah Mutlak (AUR) | Mekanisme Distribusi |
|---|---:|---:|---|
| **Creator** | 30% | 19.800.000 AUR | Alokasi Genesis Hak Awal |
| **Developer / Development** | 5% | 3.300.000 AUR | Alokasi Genesis Hak Awal (termasuk Faucet) |
| **Community / Public** | 65% | 42.900.000 AUR | Pure Mining Konsensus |
| **TOTAL** | **100%** | **66.000.000 AUR** | **Batas Suplai Maksimum Absolut** |

#### 2.3.1 Alokasi Creator (30% / 19.800.000 AUR)
Alokasi sebesar 19.800.000 AUR ditetapkan sejak genesis untuk mendukung keberlangsungan, operasional, dan stabilitas ekosistem jangka panjang, mencakup:
- Penyediaan dan pemeliharaan infrastruktur jaringan (VPS, bootnodes, seed infrastructure);
- Pembiayaan operasional jaringan terdesentralisasi;
- Kemitraan strategis dan pendanaan ekosistem;
- Keperluan lain yang secara sah menunjang keberlanjutan hidup Aurion.

Penggunaan alokasi Creator tunduk pada integritas suplai dan tidak menambah pasokan total AUR.

#### 2.3.2 Alokasi Developer / Development (5% / 3.300.000 AUR) & Aturan Faucet
Alokasi sebesar 3.300.000 AUR ditujukan untuk mendukung riset, pengembangan rekayasa sistem, pengujian perangkat lunak, testnet, audit keamanan, dan pengembangan infrastruktur teknis.

> **Aturan Wajib Faucet (Strict Faucet Invariant):**  
> Seluruh fasilitas faucet jaringan (baik pada fase pengujian maupun publik) **wajib mengambil sumber dana secara eksklusif dari alokasi Developer**.  
> Faucet **dilarang keras mengambil dana dari alokasi Community/Public** dan **dilarang keras mencetak AUR baru** di luar batas suplai yang telah ditetapkan. Setiap pengeluaran faucet dicatat sebagai pengurangan saldo alokasi Developer.

#### 2.3.3 Alokasi Community / Public — Pure Mining (65% / 42.900.000 AUR)
Sebanyak 42.900.000 AUR diperuntukkan sepenuhnya bagi publik dan komunitas melalui **mekanisme penambangan murni (*pure mining*)** yang ditentukan oleh protokol konsensus Aurion.

Pada alokasi ini berlaku larangan mutlak:
- Dilarang adanya alokasi awal tersembunyi (*hidden pre-allocation*);
- Dilarang adanya pencetakan manual (*manual minting*);
- Dilarang adanya distribusi privat di luar aturan konsensus mining;
- Dilarang adanya pemotongan untuk faucet atau subsidi lain di luar aturan *block reward*.

```text
Alokasi Community/Public (65%)
             ↓
     42.900.000 AUR
             ↓
   Pure Mining Protokol
             ↓
     Community / Public
```

### 2.4 Integritas Suplai dan Pemisahan Domain Alokasi
Protokol wajib memberlakukan pemisahan isolasi yang ketat (*strict domain separation*) di antara ketiga alokasi:

```text
Suplai Maksimum: 66.000.000 AUR
  ├── Creator Allocation (30% = 19.800.000 AUR)
  │     └── Operasional, Infrastruktur & Keberlanjutan
  ├── Developer Allocation (5% = 3.300.000 AUR)
  │     ├── Rekayasa Sistem & Audit
  │     ├── Lingkungan Testnet
  │     └── Sumber Faucet
  └── Community / Public (65% = 42.900.000 AUR)
        └── Pure Mining Terbuka
```

Setiap koin AUR yang beredar wajib dapat diverifikasi silsilahnya kembali ke salah satu dari tiga sumber sah di atas:
$$\text{Suplai Sah} = \text{Genesis Creator} + \text{Genesis Developer} + \text{Mining Publik} \le 66.000.000\ \text{AUR}$$

---

## 3. Satuan Moneter, Presisi, dan Determinisme Matematis (Quantum)

### 3.1 Satuan Moneter — AUR dan Quantum
1. **AUR** adalah satuan moneter primer dalam ekosistem Aurion.
2. **Quantum** (bentuk jamak: *Quanta*) adalah unit moneter terkecil (*atomic unit*) yang sah dalam protokol.
3. Rasio konversi dasar:
   $$\mathbf{1\ AUR = 100.000.000\ Quantum\ (10^8\ Quantum)}$$
4. Quantum adalah unit fundamental yang bersifat atomik dan tidak dapat dibagi lagi (*indivisible*). Tidak diakui adanya unit nilai di bawah 1 Quantum.

### 3.2 Presisi Moneter Tetap (8 Decimal Places)
Aurion menetapkan presisi moneter tetap sebesar **8 angka di belakang koma (8 decimal places)**.

### 3.3 Skala Pasokan Internal Jaringan
Seluruh penghitungan suplai internal protokol beroperasi pada skala bilangan bulat Quantum:
$$\mathbf{66.000.000\ AUR \times 100.000.000 = 6.600.000.000.000.000\ Quantum}$$
Ekonomi Aurion beroperasi secara internal pada skala tepat **6,6 kuadriliun Quantum**.

### 3.4 Larangan Aritmetika Floating-Point (Absolute Integer Invariant)
1. **Integer Only:** Seluruh operasi yang memengaruhi konsensus, state transition, saldo rekening/UTXO, transfer, biaya transaksi (*fees*), hadiah penambangan (*rewards*), maupun staking **WAJIB (MUST)** dieksekusi menggunakan representasi bilangan bulat Quantum.
2. **Strict Floating-Point Prohibition:** Operasi floating-point, termasuk namun tidak terbatas pada tipe data `f32` dan `f64` pada perangkat lunak, **DILARANG KERAS (MUST NOT)** digunakan di dalam lapisan konsensus dan penentuan state moneter Aurion.
3. **Alasan Ketetapan:** Larangan ini merupakan prasyarat mutlak untuk menjamin determinisme konsensus lintas platform perangkat keras/arsitektur prosesor dan meniadakan ketidaksinkronan akibat kesalahan pembulatan (*rounding errors*).

### 3.5 Pemisahan Lapisan Konsensus dan Presentasi (Presentation Layer Separation)
1. Representasi berbasis desimal (misalnya `1.5 AUR`) adalah representasi murni pada lapisan presentasi (*presentation layer*) yang ditujukan untuk kemudahan manusia.
2. Lapisan konsensus, storage database state, dan propagasi jaringan internal **MUST** secara eksklusif menyimpan dan memproses bilangan bulat Quantum.
3. Konversi nilai ke bentuk desimal AUR hanya diizinkan pada antarmuka pengguna luar, seperti CLI, API gateway, block explorer, dan frontend aplikasi dompet (*wallet*).

---

## 4. Keabsahan, Asal-Usul, dan Autentikasi Aset (Provenance & Authentication)

### 4.1 Hakikat Validitas dan Provenance
Keabsahan unit AUR tidak ditentukan oleh siapa penyimpannya saat ini, berapa lama disimpan, atau berapa kali berpindah tangan. Keabsahan AUR ditentukan semata-mata oleh **keterhubungannya dengan proses penerbitan yang sah menurut aturan konsensus Aurion**.

Setiap unit atau representasi aset AUR yang diakui jaringan wajib memiliki asal-usul (*provenance*) yang dapat dibuktikan secara independen oleh setiap simpul untuk memastikan bahwa aset:
1. Berasal dari peristiwa penciptaan atau penerbitan blok yang sah menurut aturan Aurion;
2. Memiliki alur riwayat transaksi yang valid tanpa cacat logika konsensus;
3. Tidak pernah dimanipulasi atau dicetak di luar aturan moneter protokol;
4. Tidak mengalami anomali mutasi state yang melanggar hukum konservasi suplai.

### 4.2 Ketahanan Validitas Sepanjang Masa (Persistent Asset Validity)
Setiap aset AUR yang diterbitkan secara sah mempertahankan ikatan kriptografis dengan peristiwa penerbitan asalnya sepanjang umur jaringan:
- Validitas tidak kedaluwarsa meski aset disimpan bertahun-tahun (*dormant*);
- Validitas tidak memudar walau aset telah melewati ribuan transaksi pemindahan kepemilikan;
- Mutasi kepemilikan hanya memperbarui *current ownership state*, tetapi **tidak pernah menghapus atau memutuskan issuance provenance**.

```text
Issuance Sah
     ↓
  AUR #X ── (Tersimpan / Berpindah)
     │
     ├── Pemilik A
     ├── Pemilik B
     ├── Pemilik C
     └── Pemilik N
           │
           ↓
     Provenance Tetap Utuh & Terverifikasi
```

### 4.3 Kewajiban Mekanisme Autentikasi Aset (Protocol-Verifiable Authentication)
Aurion **wajib memiliki mekanisme autentikasi aset** yang memungkinkan setiap node membedakan secara instan dan tanpa ragu antara:
- Aset sah yang bersumber dari konsensus Aurion;
- Aset palsu/tiruan yang disuntikkan di luar aturan konsensus;
- State transisi yang sah vs state yang dimanipulasi secara ilegal.

Verifikasi ini wajib bersifat independen tanpa perlu mempercayai (*trustless*) operator node mana pun.

### 4.4 Larangan Mutlak Penciptaan Aset Ilegal (No Artificial Assets)
Tidak ada entitas—baik penambang, validator, pengembang perangkat lunak, maupun pendiri—yang memiliki hak istimewa untuk menciptakan AUR di luar mekanisme konsensus.

Modifikasi pada perangkat lunak node lokal tidak boleh dapat menghasilkan AUR yang diakui sah oleh simpul lain yang menjalankan protokol resmi Aurion. **Perangkat lunak implementasi bukanlah sumber otoritas; aturan protokol konsensus adalah satu-satunya sumber otoritas.**

### 4.5 Domain Desain Terbuka (Open Specification Domain)
Pilihan primitif kriptografis spesifik untuk pembuktian keabsahan aset (seperti *cryptographic commitment, authenticated state trees, UTXO lineage proofs, Zero-Knowledge proofs*, atau kombinasi tanda tangan Merkle) ditetapkan sebagai domain **OPEN** yang diratifikasi secara terpisah dalam dokumen spesifikasi teknis (*Technical Specifications*).

---

## 5. Arsitektur Pemisahan Peran dan Modul Identifikasi Aset

### 5.1 Pemisahan Tanggung Jawab Penambang dan Identitas Aset
Protokol Aurion secara tegas memisahkan proses **produksi blok (*mining/block production*)** dari proses **penetapan identitas dan asal-usul aset (*asset identification & provenance generation*)**.

Penambang bertanggung jawab atas komputasi konsensus dan pengusulan blok yang valid. **Penambang TIDAK memiliki wewenang untuk memilih, menetapkan, merekayasa, atau memanipulasi identitas maupun identifier aset yang lahir dari hadiah blok (*block reward*).**

```text
Penambang / Block Producer
             ↓
    Produksi Kandidat Blok
             ↓
     Validasi Konsensus
             ↓
  Block Reward Dinyatakan Sah
             ↓
 Asset Identification Module
             ↓
 Deterministic Provenance Identifier
```

### 5.2 Modul Identifikasi Aset (Asset Identification Module)
Setelah suatu blok dan penerbitan reward dinyatakan sah oleh konsensus, proses penandaan diteruskan kepada komponen protokol terisolasi: **Asset Identification Module**.

Karakteristik Modul Identifikasi Aset:
1. Menjalankan fungsi derivasi murni deterministik berdasarkan aturan konsensus;
2. Hanya menerima masukan dari peristiwa penerbitan (*issuance event*) yang telah lolos validasi konsensus;
3. Menolak identifier arbitrer yang diajukan secara sepihak oleh penambang;
4. Dapat diverifikasi secara identik oleh seluruh simpul penuh (*full nodes*);
5. Tidak bertindak sebagai produsen moneter independen di luar aturan suplai.

### 5.3 Hirarki Otoritas Konsensus (Authority Boundary)
Keabsahan aset berasal dari pemenuhan hukum konsensus, bukan semata-mata dari keberadaan identifier. Alur otoritas berlaku satu arah:

```text
Aturan Konsensus Protokol (Protocol Rules)
                 ↓
      Valid State Transition
                 ↓
       Valid Issuance Event
                 ↓
    Asset Identification Module
                 ↓
   Reward Provenance Identifier (RPI)
                 ↓
       Tercatat Pada State Sah
```

Identifier berfungsi sebagai **bukti/penanda silsilah kriptografis (provenance marker)**, bukan sebagai sumber pembuat legitimasi aset secara mandiri.

### 5.4 Derivasi Kriptografis Deterministik (Reward Provenance Identifier - RPI)
Penerbitan AUR dari block reward menghasilkan penanda asal-usul yang disebut **Reward Provenance Identifier (RPI)**.

RPI diturunkan secara deterministik dari konteks blok yang sah:
$$\text{RPI} = \mathcal{H}_{\text{prov}}(\text{ChainID} \parallel \text{BlockHash} \parallel \text{BlockHeight} \parallel \text{IssuanceContext})$$

Identifier ini wajib:
- Bersifat unik untuk setiap peristiwa penerbitan (*collision-resistant*);
- Terikat secara permanen pada rantai blok dan nomor tinggi blok (*block height*);
- Mustahil dipalsukan tanpa merusak validitas konsensus blok itu sendiri;
- Terisolasi dari data manipulatif penambang.

### 5.5 Batas Konstitusi vs Spesifikasi Teknis (Architecture Boundary)
Konstitusi Aurion mengatur invarian dan prinsip mutlak yang wajib dipatuhi. Detail implementasi rekayasa sistem berikut diserahkan kepada dokumen spesifikasi arsitektur (*Architecture Specifications*):
- Format serialisasi RPI (apakah numerik desimal, heksadesimal, atau byte array biner);
- Granularitas penandaan (apakah melekat pada tingkat blok reward, UTXO output, atau unit akun);
- Pemilihan algoritma hashing dan struktur data penampung state;
- Penanganan reorganisasi rantai pendek (*chain reorganization*);
- Struktur pembungkus tipe data internal pada kode program (seperti representasi `u128` integer wrapper dalam Rust).

---

## 6. Prinsip Utama dan Invarian Tertinggi (Supreme Invariants)

Konstitusi Aurion mengikat seluruh simpul, pengembang, penambang, dan pengguna melalui asas-asas tertinggi yang tidak dapat dilanggar:

> ### Invarian Keabsahan State:
> **"Tidak ada satu unit AUR pun yang dapat diakui sebagai bagian dari state protokol yang sah, kecuali penciptaan dan asal-usulnya (provenance) dapat dibuktikan secara matematis berdasarkan aturan konsensus Aurion."**

> ### Invarian Sumber Otoritas:
> **"Perangkat lunak simpul (node) tidak menentukan apa itu aset yang sah; Protokol konsensus Aurion-lah yang menentukan apa itu aset yang sah."**

> ### Invarian Integritas Moneter:
> **"Sejak blok genesis, Aurion menetapkan suplai maksimum sebesar 66.000.000 AUR (setara dengan 6.600.000.000.000.000 Quantum). Pembagian hak 30% Creator, 5% Developer, dan 65% Pure Mining Komunitas bersifat mengikat. Faucet wajib dibiayai dari alokasi Developer. Segala bentuk pencetakan aset di luar batasan ini adalah pembatalan langsung terhadap konsensus Aurion."**

> ### Invarian Determinisme Aritmetika:
> **"Konsensus moneter Aurion beroperasi secara eksklusif dengan bilangan bulat Quantum. Penggunaan floating-point pada lapisan konsensus adalah pelanggaran konstitusional terhadap determinisme jaringan."**
