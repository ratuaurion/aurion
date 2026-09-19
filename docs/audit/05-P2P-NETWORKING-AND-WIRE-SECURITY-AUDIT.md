# AURION SECURITY AUDIT — P2P Networking & Wire Security Subsystem
> **Modul Diperiksa:** `src/platform/wire/`, `src/platform/gateway/`, `src/consensus/mempool/`  
> **Klasifikasi:** Evaluasi Protokol Wire Biner, Anti-DDoS, & Topologi Simpul  
> **Status:** **PASSED (100% CANONICAL)**

---

## 1. Ruang Lingkup Audit

Submateri Jaringan P2P dan Protokol Wire Aurion mencakup:
1. **Framing Biner Kanonikal `AUR0`:** Struktur header 52-byte dengan magic byte `0x41555230` dan checksum payload Blake3 32-byte.
2. **Batas Anti-DoS Payload:** Penolakan sebelum alokasi (*pre-allocation bound checking*) terhadap payload berukuran melebihi batas maksimal 8 MB (8.388.608 byte).
3. **Mutual Handshake Otentikasi:** Pertukaran pesan `Handshake` bertanda tangan Ed25519 dengan validasi Genesis Hash dan toleransi pergeseran jam maksimal 120 detik.
4. **Isolasi Simpul Sentry (Sentry Node Privilege Isolation):** Perlindungan perimeter di mana simpul validator tidak mengekspos port publik ke internet dan hanya berkomunikasi via simpul Sentry (`AUR-APP-12`).
5. **Mempool Anti-Spam & Mandat Replace-by-Fee (RBF):** Batas antrean mempool 10.000 transaksi dan kenaikan fee minimal 10% untuk penggantian transaksi.

---

## 2. Temuan & Analisis Teknis

### 2.1. Penolakan Injeksi Frame Oversize (Buffer Overflow DoS)
- **Vektor Ancaman:** Penyerang mengirim frame biner dengan field `payload_length` palsu yang sangat besar (misal: 2 GB) untuk memaksa simpul melakukan alokasi heap memori berlebih dan memicu crash *Out of Memory* (OOM).
- **Implementasi Aurion (`src/platform/wire/frame.rs`):**
  Parser menolak frame secara ketat sebelum payload dialokasikan: (a) `Message Type ID` wajib ada di katalog resmi (`WireError::UnknownMessageType`), (b) `Reserved`/`Reserved2` wajib nol (`WireError::NonZeroReserved`/`NonZeroReserved2`), (c) `payload_len` wajib $\le$ plafon khusus tipe pesan (`WireError::PayloadTooLarge`), dengan plafon global $8.388.608\ \text{B}$ sebagai *backstop* keras. Untuk `TX_GOSSIP` plafonnya `MAX_TX_WIRE_SIZE` $= 24.764\ \text{B}$.
- **Verifikasi Pengujian (`tests/security_hardening.rs`):**
  `test_strict_wire_frame_rejects_oversized_per_message_type` menyuntikkan `payload_len` melebihi plafon $24.764\ \text{B}$ dan memastikan penolakan sebelum alokasi; `test_strict_wire_frame_rejects_unknown_type_and_nonzero_reserved` memverifikasi penolakan tipe tak dikenal dan field cadangan non-zero. Injeksi header 16 MB tetap ditolak oleh *backstop* $8\ \text{MiB}$.

### 2.2. Penolakan Spam Mempool Sub-RBF
- **Vektor Ancaman:** Penyerang membanjiri mempool dengan transaksi duplikat nonce yang hanya menaikkan biaya 1 Quanta untuk menggeser transaksi valid tanpa biaya berarti.
- **Implementasi Aurion (`src/consensus/mempool/engine.rs`):**
  Mempool memberlakukan aturan RBF ketat di mana biaya transaksi baru wajib lebih tinggi minimal 10% dari biaya transaksi sebelumnya (`fee_new >= fee_old * 110 / 100`).
- **Verifikasi Pengujian (`tests/security_audit.rs:test_exploit_mempool_sub_rbf_spam_rejection`):**
  Transaksi pengganti dengan kenaikan fee hanya 5% ditolak seketika oleh mempool engine.

---

## 3. Kesimpulan Auditor
Lapisan jaringan Aurion memiliki pertahanan perimeter berlapis, framing biner kedap manipulasi, dan perlindungan anti-DoS yang tangguh di gerbang ingress.
