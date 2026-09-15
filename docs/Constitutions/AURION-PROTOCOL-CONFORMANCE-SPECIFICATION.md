# 12 — AURION PROTOCOL CONFORMANCE SPECIFICATION
## Spesifikasi Formal Standar Kepatuhan Protokol, Sertifikasi Kompatibilitas Node, dan Protokol Uji Otomatis

> **Hierarki Dokumen:**  
> `00 — AURION CONSTITUTION` $\longrightarrow$ `01 — MONETARY` $\longrightarrow$ `02 — CONSENSUS` $\longrightarrow$ `03 — STATE TRANSITION` $\longrightarrow$ `04 — TRANSACTION` $\longrightarrow$ `05 — CRYPTOGRAPHY` $\longrightarrow$ `06 — SERIALIZATION & WIRE` $\longrightarrow$ `07 — GENESIS` $\longrightarrow$ `08 — VALIDATOR & STAKING` $\longrightarrow$ `09 — GOVERNANCE` $\longrightarrow$ `10 — SECURITY` $\longrightarrow$ `11 — TEST VECTORS` $\longrightarrow$ **`12 — PROTOCOL CONFORMANCE SPECIFICATION`**  
>
> **Status:** AUDITED & LOCKED SPECIFICATION  
> **Klasifikasi:** Standar Sertifikasi & Akreditasi Perangkat Lunak Simpul (Conformance Anchor)  
> **Versi:** 1.0.0-PROD  
> **Sifat Ketetapan:** Wajib Mutlak (*Strictly Normative*), Tolok Ukur Kesiapan Mainnet

---

## 1. Definisi dan Tujuan Standar Kepatuhan

Spesifikasi ini menetapkan kriteria pengujian mutlak yang harus dipenuhi oleh setiap implementasi perangkat lunak simpul (*node implementation*)—baik yang dibangun menggunakan bahasa Rust, Go, Python, C++, maupun bahasa lainnya—sebelum dapat dinyatakan dan diakreditasi sebagai **Aurion-Compatible Node**.

Tujuan standar ini adalah menjamin bahwa seluruh simpul di seluruh dunia berperilaku secara identik pada tingkat matematika konsensus, meniadakan celah percabangan rantai (*chain split*), dan melindungi kedaulatan moneter yang telah diratifikasi dalam **[AURION CONSTITUTION.md](file:///c:/Projects/aurion/AURION%20CONSTITUTION.md)**.

### Terminologi Normatif (RFC 2119 / RFC 8174)
- **WAJIB / HARUS (MUST / SHALL):** Persyaratan mutlak. Pelanggaran terhadap poin ini menyebabkan simpul otomatis ditolak dan diblokir dari jaringan.
- **DILARANG KERAS (MUST NOT / SHALL NOT):** Larangan mutlak. Pelanggaran memicu pemutusan hubungan P2P atau kegagalan konsensus.
- **SEHARUSNYA (SHOULD):** Rekomendasi kuat terkait efisiensi atau performa, namun bukan penyebab pemutusan konsensus jika diabaikan dengan alasan sah.

---

## 2. Delapan Pilar Kepatuhan Protokol Aurion (The 8 Conformance Pillars)

```text
┌────────────────────────────────────────────────────────────────────────┐
│               DELAPAN PILAR SERTIFIKASI NODE AURION-COMPATIBLE         │
├────────────────────────────────┬───────────────────────────────────────┤
│ PILAR KEPATUHAN                │ TARGET PENEGAKAN WAJIB (MUST)         │
├────────────────────────────────┼───────────────────────────────────────┤
│ 1. Kriptografi Murni           │ Blake3 (256-bit) & Ed25519 Strict     │
├────────────────────────────────┼───────────────────────────────────────┤
│ 2. Serialisasi Kanonikal       │ Big-Endian, Zero-Trailing Bytes       │
├────────────────────────────────┼───────────────────────────────────────┤
│ 3. Verifikasi Genesis          │ State σ_0, StateRoot_0, GenesisHash   │
├────────────────────────────────┼───────────────────────────────────────┤
│ 4. Integritas Moneter Konstitusi│ Zero-Float, 66M Cap, Invarian [INV]   │
├────────────────────────────────┼───────────────────────────────────────┤
│ 5. Validasi Transaksi          │ Stateless & Stateful Dual-Gate        │
├────────────────────────────────┼───────────────────────────────────────┤
│ 6. Mesin State Transition (STF)│ Eksekusi Determinis, SMT State Root   │
├────────────────────────────────┼───────────────────────────────────────┤
│ 7. Konsensus Aurion-BFT        │ 2-Phase Vote, > 2/3 Kuorum, Anti-Fork │
├────────────────────────────────┼───────────────────────────────────────┤
│ 8. Protokol Wire P2P & Anti-DoS│ Frame 52-B, Plafon Memori, Skor Peer  │
└────────────────────────────────┴───────────────────────────────────────┘
```

---

### Pilar 1: Kepatuhan Kriptografi (Cryptographic Conformance)
Sebuah simpul Aurion yang sah **WAJIB**:
1. Mengimplementasikan fungsi hash **BLAKE3 (256-bit output / 32 bytes)** secara murni;
2. Mengimplementasikan skema tanda tangan digital **Ed25519 (RFC 8032)** dengan verifikasi ketat (*Strict Verification*);
3. **DILARANG KERAS** menerima tanda tangan dengan skalar non-kanonikal ($S \ge \ell$) atau titik kurva subgrup berorde kecil;
4. Mengimplementasikan enkode alamat **Bech32m (BIP-350)** dengan HRP resmi (`aur1` untuk Mainnet, `aurt1` untuk Testnet);
5. Menerapkan seluruh string pemisahan domain (*Domain Separation Tags*) sesuai Dokumen 05.

---

### Pilar 2: Kepatuhan Serialisasi Kanonikal (Serialization Conformance)
Sebuah simpul Aurion yang sah **WAJIB**:
1. Menghasilkan representasi byte biner yang tepat cocok dengan spesifikasi Dokumen 06;
2. Menggunakan representasi integer Big-Endian murni untuk seluruh tipe numerik;
3. Mengembalikan galat fatal `TrailingBytesError` seketika jika terdapat kelebihan byte dalam buffer;
4. Menolak struktur data dinamis yang panjangnya melampaui plafon yang ditentukan.

---

### Pilar 3: Kepatuhan Genesis (Genesis Conformance)
Sebuah simpul Aurion yang sah **WAJIB**:
1. Menginisialisasi state awal $\sigma_0$ dengan alokasi tepat $30\%$ Creator ($1.980.000.000.000.000\ Q$), $5\%$ Developer ($330.000.000.000.000\ Q$), dan $65\%$ Cadangan Penambangan ($4.290.000.000.000.000\ Q$);
2. Memverifikasi bahwa hash header Blok 0 tepat identik dengan **GenesisHash** resmi:
   ```text
   e972418a0928b12e6945a0b3687311d402947b5921855e34789012a43b174092
   ```
3. Memutus koneksi P2P seketika dari peer yang memancarkan `GenesisHash` yang berbeda saat fase Handshake.

---

### Pilar 4: Kepatuhan Moneter Konstitusional (Monetary Conformance)
Sebuah simpul Aurion yang sah **WAJIB**:
1. Mengoperasikan seluruh logika moneter dan saldo secara eksklusif menggunakan integer Quantum ($u128$);
2. **DILARANG KERAS** menggunakan tipe floating-point (`f32`, `f64`) pada lapisan konsensus dan transisi state;
3. Menegakkan batas total suplai absolut $S_{\max} = 66.000.000\ \text{AUR}$ ($6,6 \times 10^{15}\ Q$);
4. Menegakkan pembakaran $20\%$ biaya transaksi ($\mathcal{F}_{\text{burned}}$) dan pembagian $80\%$ ke produser blok;
5. Menegakkan masa kematangan hadiah coinbase $100\ \text{blok}$ (*Coinbase Maturity*);
6. Memvalidasi keenam invarian suplai **[INV-01] hingga [INV-06]** pada setiap blok.

---

### Pilar 5: Kepatuhan Transaksi (Transaction Conformance)
Sebuah simpul Aurion yang sah **WAJIB**:
1. Menolak transaksi dengan ukuran $> 65.536\ \text{bytes}$;
2. Menolak transaksi dengan `chain_id` yang tidak cocok dengan rantai aktif;
3. Menolak transaksi dengan `nonce` yang tidak sesuai dengan urutan akun pengirim;
4. Menolak transaksi jika $\text{balance} < \text{amount} + \text{fee}$;
5. Menolak transaksi dengan `fee` di bawah tarif minimum $\text{MinFeePerByte} \times \text{Size}$.

---

### Pilar 6: Kepatuhan State Transition Function (STF Conformance)
Sebuah simpul Aurion yang sah **WAJIB**:
1. Mengeksekusi transisi state secara atomik: jika terjadi kegagalan pada salah satu transaksi, seluruh state blok dibatalkan secara bersih (*atomic rollback*);
2. Menghitung root hash Sparse Merkle Tree (SMT) atas state baru $\sigma_{H}$;
3. Memverifikasi kesetaraan mutlak: $\text{ComputedStateRoot} == B.\text{state\_root}$;
4. Mengaktifkan mode penghentian aman (*Terminal Fail-Stop Lockout*) jika terjadi ketidakcocokan state root dari blok yang memiliki Commit Certificate kuorum.

---

### Pilar 7: Kepatuhan Konsensus Aurion-BFT (Consensus Conformance)
Sebuah simpul Aurion yang sah **WAJIB**:
1. Menolak blok yang diusulkan oleh simpul di luar hak giliran $\text{Proposer}(H, R)$;
2. Hanya memancarkan suara Pre-commit jika telah mengamati Polka / Prepare QC sah dari $\ge \mathcal{Q}$ suara Pre-vote;
3. Hanya menganggap sebuah blok berstatus **FINAL** jika memiliki Commit Certificate $\mathcal{CC}(B)$ dari $\ge \mathcal{Q}$ bobot validator sah;
4. Menolak secara mutlak segala bentuk reorganisasi terhadap blok yang telah difinalisasi ($\text{Reorganization Depth} = 0$);
5. Menolak dan memproses bukti kecurangan (*fraud proof*) ekuivokasi dengan menyita $100\%$ agunan validator jahat.

---

### Pilar 8: Kepatuhan Protokol Jaringan Wire P2P (Wire Conformance)
Sebuah simpul Aurion yang sah **WAJIB**:
1. Membungkus seluruh paket jaringan dengan Wire Frame Header 52 bytes ber-magic `0x41555230` ("AUR0");
2. Memvalidasi Blake3 checksum payload sebelum memproses isi pesan;
3. Membatasi alokasi memori sebelum dekode (*Pre-allocation Check*) guna mencegah serangan kehabisan memori (Anti-OOM);
4. Menerapkan sistem skor reputasi peer dan memblokir IP peer yang skornya $\le 0$ selama minimal 24 jam.

---

## 3. Tiga Tingkatan Akreditasi Simpul (Node Certification Tiers)

Protokol Aurion mendefinisikan tiga tingkatan sertifikasi implementasi:

```text
┌────────────────────────────────────────────────────────────────────────┐
│                   TINGKATAN SERTIFIKASI SIMPUL AURION                  │
├─────────────────────────┬──────────────────────────────────────────────┤
│ TINGKAT SERTIFIKASI     │ CAKUPAN TANGGUNG JAWAB PROTOKOL              │
├─────────────────────────┼──────────────────────────────────────────────┤
│ Tier 1: Light Node      │ Verifikasi Header, SPV Proof, Bech32m, Wire  │
├─────────────────────────┼──────────────────────────────────────────────┤
│ Tier 2: Full Node       │ Sync Ledger, STF Lengkap, Mempool, Verifier  │
├─────────────────────────┼──────────────────────────────────────────────┤
│ Tier 3: Validator Node  │ BFT Voting, Proposer, Staking, Anti-Equiv    │
└─────────────────────────┴──────────────────────────────────────────────┘
```

1. **Tier 1 (Light Node / SPV):**  
   Diperuntukkan bagi dompet seluler dan aplikasi klien ringan. Wajib lulus pengujian Kriptografi, Serialisasi Header, dan Verifikasi Bukti Merkle.
2. **Tier 2 (Full Node / Archive Node):**  
   Diperuntukkan bagi relay jaringan, block explorer, dan bursa kripto. Wajib lulus seluruh 8 pilar kepatuhan, mengeksekusi penuh seluruh riwayat transaksi sejak Genesis Blok 0 hingga blok terkini.
3. **Tier 3 (Validator Sovereign Node):**  
   Diperuntukkan bagi operator konsensus penghasil blok. Memenuhi Tier 2 ditambah modul penandatanganan suara BFT aman, isolasi kunci privat via HSM (*Hardware Security Module*), dan sistem pencegahan ekuivokasi lokal (*Double-Sign Protection Guard*).

---

## 4. Rangkaian Pengujian Kepatuhan Protokol (Conformance Test Suite / CTS)

Setiap implementasi simpul wajib melewati tiga fase pengujian otomatis:

```text
[FASE 1: VEKTOR UJI EMAS] ───► Lulus 100% Vektor Uji Dokumen 11
            │
            ▼
[FASE 2: STATE MACHINE FUZZ] ─► Lulus 1.000.000 Transaksi Acak Tanpa Crash
            │
            ▼
[FASE 3: ADVERSARIAL NETWORK] ─► Simulasi Partisi, Byzantine Node, & DoS
            │
            ▼
     [AKREDITASI RESMI: AURION-COMPATIBLE CERTIFIED]
```

### 4.1 Fase 1: Validasi Vektor Uji Statis (Golden Vector Tests)
Implementasi simpul wajib mengintegrasikan seluruh test vector pada **[11 — REFERENCE TEST VECTORS SPECIFICATION](file:///c:/Projects/aurion/AURION-REFERENCE-TEST-VECTORS.md)** ke dalam unit test internal. Tingkat kelulusan yang dapat diterima adalah **tepat 100% (Zero Tolerance for Failure)**.

### 4.2 Fase 2: Pengujian Fuzzing Diferensial (Differential STF Fuzzing)
Simpul diuji secara diferensial terhadap implementasi referensi resmi dengan menyuntikkan jutaan mutasi transaksi acak (*malformed, boundary, underflow, overflow*). Hasil *StateRoot* akhir wajib identik bit-demi-bit.

### 4.3 Fase 3: Simulasi Jaringan Adversarial (Chaos Network Testing)
Simpul diuji di dalam lingkungan testbed privat dengan kondisi:
- $30\%$ node bertindak sebagai Byzantine offline / pengirim data acak;
- Simulasi pemisahan jaringan fisik (partisi);
- Serangan pembanjiran mempool dan payload wire raksasa.
- *Kriteria Kelulusan:* Simpul tidak mengalami *panic crash*, tidak mengalami *memory leak*, dan tidak pernah mengadopsi rantai yang bertentangan dengan konsensus mayoritas jujur.

---

## 5. Surat Deklarasi Kepatuhan Protokol (Protocol Conformance Seal)

Sebuah implementasi perangkat lunak hanya berhak menyematkan label **"Aurion Protocol Certified Node"** jika seluruh persyaratan dalam dokumen ini telah terverifikasi dan terdokumentasi dalam laporan uji terbuka.

Dengan diratifikasinya spesifikasi ini, **seluruh fondasi arsitektur spesifikasi resmi Aurion Protocol (Dokumen 00 hingga 12) telah resmi selesai, terkunci, dan siap menjadi cetak biru implementasi rekayasa sistem (*Mainnet Engineering*)**.
