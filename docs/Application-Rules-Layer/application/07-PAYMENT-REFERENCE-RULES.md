# 07 — AURION PAYMENT REFERENCE & MEMO APPLICATION RULES
## Standar Format Referensi Pembayaran, Tag Destinasi Bursa, dan Integritas Payload Memo

> **Hierarki Dokumen:**  
> `AURION CONSTITUTIONS` $\longrightarrow$ `PROTOCOL SPECIFICATIONS` $\longrightarrow$ `00-APPLICATION-RULES` $\longrightarrow$ **`07-PAYMENT-REFERENCE-RULES`**  
>
> **Status:** RATIFIED & LOCKED APPLICATION SPECIFICATION  
> **Klasifikasi:** Standar Metadata Transaksi & Routing Bursa (Payment Reference Standard)  
> **Versi:** 1.0.0-PROD  
> **Sifat Ketetapan:** Normatif Wajib (RFC 2119 / RFC 8174), Machine-Readable, Anti-PII Leak

---

## 1. Urgensi Referensi Pembayaran dalam Ekosistem Aurion

Dalam transaksi perbankan dan bursa kripto terpusat (CEX), ribuan pengguna sering kali menyetorkan dana ke satu alamat deposit terpusat (*Omnibus Hot Wallet*). 

Oleh karena itu, penyertaan **Referensi Pembayaran (Memo / Destination Tag / Invoice ID)** pada field payload transaksi menjadi prasyarat mutlak agar sistem bursa dapat mengarahkan (*routing*) kredit saldo secara otomatis ke akun pengguna yang bersangkutan.

---

## 2. Pemanfaatan Field Payload Transaksi

1. **Lokasi Penyimpanan Protokol:** Referensi pembayaran disimpan di dalam field kanonikal transaksi:
   $$\text{Transaction.payload: ByteSeq (Panjang } N \le 65.536\ \text{Bytes)}$$
2. **Plafon Rekomendasi Aplikasi (Application Size Budget):**
   - Untuk transfer reguler, invoice merchant, dan tag bursa, ukuran memo **SHOULD NOT** melebihi **$64\ \text{Bytes}$** (dan **MUST NOT** melebihi $256\ \text{Bytes}$).
   - *Rasional:* Membatasi ukuran payload mencegah pemborosan ruang disk node jaringan (*chain bloat*) dan meminimalkan biaya fee transaksi bagi pengguna.

---

## 3. Taksonomi dan Format Standar Referensi (Standard Prefixes)

Untuk memungkinkan penguraian otomatis (*automated machine parsing*) lintas aplikasi, memo berbasis teks **SHOULD** mengadopsi skema prefiks standar 3-karakter:

```text
┌────────────────────────────────────────────────────────────────────────┐
│               PREFIKS STANDAR REFERENSI PEMBAYARAN AURION              │
├────────┬───────────────────┬───────────────────────────────────────────┤
│ PREFIKS│ PERUNTUKAN        │ CONTOH FORMAT                             │
├────────┼───────────────────┼───────────────────────────────────────────┤
│ USR:   │ Tag Akun Bursa    │ USR:98204123                              │
│        │ (Destination Tag) │ Mengarahkan deposit omnibus CEX ke user.  │
├────────┼───────────────────┼───────────────────────────────────────────┤
│ INV:   │ Tagihan Merchant  │ INV:2026-X9410                            │
│        │ (Invoice ID)      │ Menghubungkan transfer ke nomor pesanan.  │
├────────┼───────────────────┼───────────────────────────────────────────┤
│ REF:   │ Pengembalian Dana │ REF:7a9f82635921820d                      │
│        │ (Refund Original) │ Mengikat retur dana ke TxID sebelumnya.   │
├────────┼───────────────────┼───────────────────────────────────────────┤
│ TXT:   │ Pesan Bebas       │ TXT:Hadiah Ulang Tahun                    │
│        │ (Public Memo)     │ Catatan personal antar-pengguna.          │
└────────┴───────────────────┴───────────────────────────────────────────┘
```

---

## 4. Aturan Sanitasi Karakter dan Keamanan (Character Sanitization)

1. **Enkoding Karakter Baku:** Memo berbasis teks **MUST** dienkode menggunakan **UTF-8 valid murni**.
2. **Penolakan Karakter Kontrol Berbahaya:**
   - Aplikasi input dan wallet **MUST** menolak atau memotong karakter kontrol tak terlihat (*ASCII Control Characters* `0x00` s/d `0x1F` dan `0x7F`), kecuali karakter spasi biasa (`0x20`).
   - Khususnya, karakter `NULL` (`0x00`) **MUST NOT** disematkan di tengah string teks untuk mencegah serangan *Null-Byte Injection* pada sistem backend berbasis C/C++.
3. **Penyandian Biner Murni (Raw Binary Memos):**  
   Jika aplikasi menyematkan hash kriptografis atau bukti zk (bukan teks UTF-8), dua byte pertama dari payload **SHOULD** diawali dengan identifier byte `0x00 0x00` yang menandakan bahwa payload adalah biner murni, bukan teks yang dapat dibaca manusia.

---

## 5. Peringatan Perlindungan Privasi (Anti-PII Mandate)

> **PERINGATAN HUKUM & PRIVASI:**  
> Seluruh byte yang disematkan ke dalam field payload transaksi Aurion tercatat secara **terbuka, publik, dan permanen selamanya** di buku besar terdistribusi global.

1. **Larangan Informasi Pribadi Sensitif (PII Proscription):**  
   Wallet dan aplikasi merchant **MUST NOT** menyematkan data identitas pribadi (*Personally Identifiable Information*) ke dalam memo, seperti:
   - Nomor KTP / Paspor / NIK;
   - Nomor rekening bank atau nomor kartu kredit;
   - Alamat email pribadi atau nomor telepon;
   - Kata sandi atau kunci otentikasi.
2. **Kewajiban Peringatan Dompet:**  
   Jika pengguna mengetikkan teks memo pada antarmuka pengiriman, dompet **SHOULD** menyajikan teks pengingat: *"Catatan ini bersifat publik dan dapat dibaca oleh siapa saja di seluruh dunia."*

---

## 6. Penanganan Transaksi Deposit Tanpa Memo pada Bursa (Missing Tag Policy)

Bursa kripto (CEX) yang menggunakan sistem deposit omnibus **MUST** menerapkan kebijakan operasional formal untuk transaksi yang tiba tanpa memo `USR:`:

1. **Isolasi Dana Otomatis:** Dana tetap masuk ke hot wallet bursa pada level on-chain, namun **MUST NOT** dikreditkan ke saldo akun pengguna mana pun secara spekulatif.
2. **Alur Klaim Manual Terverifikasi:**  
   Bursa **MUST** menyediakan antarmuka klaim deposit mandiri (*Self-Service Deposit Recovery*) di mana pengguna wajib membuktikan kepemilikan alamat pengirim dengan menandatangani pesan kriptografis Ed25519 menggunakan kunci privat alamat terkait.
