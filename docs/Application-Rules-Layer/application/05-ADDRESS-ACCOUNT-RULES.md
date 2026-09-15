# 05 — AURION ADDRESS & ACCOUNT APPLICATION RULES
## Standar Presentasi Alamat, Normalisasi Bech32m, Representasi QR Code, dan Spesifikasi Skema URI

> **Hierarki Dokumen:**  
> `AURION CONSTITUTIONS` $\longrightarrow$ `PROTOCOL SPECIFICATIONS` $\longrightarrow$ `00-APPLICATION-RULES` $\longrightarrow$ **`05-ADDRESS-ACCOUNT-RULES`**  
>
> **Status:** RATIFIED & LOCKED APPLICATION SPECIFICATION  
> **Klasifikasi:** Standar Alamat & Antarmuka Interoperabilitas (Address & URI Standard)  
> **Versi:** 1.0.0-PROD  
> **Sifat Ketetapan:** Normatif Wajib (RFC 2119 / RFC 8174), BIP-350 / BIP-21 Conforming

---

## 1. Standar Format Alamat Kanonikal

1. **Format Presentasi Baku:** Seluruh aplikasi Aurion **MUST** merepresentasikan alamat akun kepada pengguna dalam format **Bech32m (BIP-350)**.
2. **Panjang dan Komposisi Karakter:**
   - **Mainnet (HRP: `aur`):** Tepat **62 karakter**:
     ```text
     aur1 [52 Karakter Data Base32] [6 Karakter Checksum]
     Contoh: aur10t9h48nhaunuj2uqf8kfd6jqjq07h0ecv85h49lsreaq9uqhqffshf0p6h
     ```
   - **Testnet (HRP: `aurt`):** Tepat **63 karakter**:
     ```text
     aurt1 [52 Karakter Data Base32] [6 Karakter Checksum]
     Contoh: aurt10t9h48nhaunuj2uqf8kfd6jqjq07h0ecv85h49lsreaq9uqhqffsjazk8c
     ```
3. **Larangan Format Usang:** Aplikasi **MUST NOT** menggunakan format Bech32 versi awal (BIP-173) untuk alamat Aurion, karena rentan terhadap mutasi penambahan karakter '1'. Aurion mewajibkan varian **Bech32m** dengan konstanta checksum $M = \text{0x2bc830a3}$.

---

## 2. Aturan Normalisasi dan Validasi Input (Address Normalization)

Aplikasi klien dan antarmuka web **MUST** menerapkan prosedur sanitasi berikut sebelum memproses alamat:

1. **Penghapusan Spasi Tambahan:** Memotong (*trim*) seluruh karakter spasi, tab, atau baris baru di awal dan akhir string input.
2. **Konversi Huruf Kecil (Case Lowercasing):**
   - Secara internal, alamat Bech32m disimpan dan diproses dalam format huruf kecil murni (*lowercase*).
   - Jika pengguna memasukkan alamat dalam format huruf besar murni (*uppercase*), aplikasi **MAY** mengonversinya secara otomatis ke huruf kecil.
   - Jika input memuat campuran huruf besar dan kecil (*mixed-case*), aplikasi **MUST** menolak input tersebut sebagai format yang tidak sah sebelum normalisasi.
3. **Pencocokan HRP Terhadap Jaringan Aktif:**
   ```text
   IF active_network == MAINNET AND hrp != "aur" THEN ERROR(INVALID_NETWORK_HRP)
   IF active_network == TESTNET AND hrp != "aurt" THEN ERROR(INVALID_NETWORK_HRP)
   ```
4. **Verifikasi Checksum Polinomial:** Checksum 6 karakter **MUST** dievaluasi. Jika checksum tidak cocok, transaksi tidak boleh dibuat.

---

## 3. Representasi Kode QR (QR Code Encoding Standard)

Untuk memaksimalkan keterbacaan scanner kamera dan meminimalkan kepadatan modul QR:

1. **Mode Alfanumerik QR (Uppercase Encoding):**
   - Ketika alamat Bech32m diubah menjadi QR Code tanpa parameter URI, aplikasi **SHOULD** mengubah string alamat menjadi **HURUF BESAR SEMUA (*UPPERCASE*)**:
     ```text
     AUR10T9H48NHAUNUJ2UQF8KFD6JQJQ07H0ECV85H49LSREAQ9UQHQFFSHF0P6H
     ```
   - *Rasional:* Mode alfanumerik QR Code mengompresi karakter huruf besar dengan efisiensi jauh lebih tinggi dibandingkan mode biner/byte UTF-8, menghasilkan kode QR berdensitas rendah yang 20% lebih cepat dipindai oleh kamera beresolusi rendah.
2. **Level Koreksi Galat (Error Correction Level):**  
   QR code untuk pembayaran Aurion **SHOULD** menggunakan **Level M (15% Recovery)** atau **Level Q (25% Recovery)** untuk memastikan pemindaian tetap berhasil meskipun permukaan layar tergores.

---

## 4. Skema URI Pembayaran Universal (`aurion:`)

Mengikuti filosofi BIP-21, protokol Aurion menetapkan skema URI universal untuk interaksi transfer dan pembayaran instan:

### 4.1 Sintaksis Tata Bahasa Formal (BNF)
```text
aurion-uri      = "aurion:" aurion-address [ "?" query-params ]
aurion-address  = ( mainnet-addr | testnet-addr )
query-params    = param [ "&" query-params ]
param           = ( amount-param | memo-param | label-param | custom-param )
amount-param    = "amount=" 1*DIGIT [ "." 1*8DIGIT ]
memo-param      = "memo=" *pchar
label-param     = "label=" *pchar
custom-param    = ( [ "req-" ] 1*pchar "=" *pchar )
```

### 4.2 Parameter URI Standar
- **`amount`:** Nilai transfer. 
  - Jika ditulis dalam desimal (misal `amount=2.5`), ditafsirkan sebagai $2,5\ \text{AUR}$ ($250.000.000\ Q$).
  - Angka di belakang koma **MUST NOT** melebihi 8 digit desimal.
- **`memo`:** Teks referensi pembayaran atau invoice ID (maksimum 64 bytes UTF-8 ter-URL-encoded).
- **`label`:** Nama penerima atau entitas yang ramah manusia (misal: `label=Toko%20Kopi%20Aurion`).

### 4.3 Contoh URI Resmi
```text
aurion:aur10t9h48nhaunuj2uqf8kfd6jqjq07h0ecv85h49lsreaq9uqhqffshf0p6h?amount=1.50000000&memo=INV-2026-9041&label=Warung%20Sovereign
```

### 4.4 Aturan Penerimaan Parameter Wajib (`req-`)
Jika URI memuat parameter tambahan yang diawali dengan prefiks `req-` (misal `req-expiry=1773573600`) dan aplikasi dompet pengguna tidak mengenali parameter tersebut, dompet **MUST** menolak URI tersebut secara menyeluruh dan tidak boleh mengeksekusi transfer parsial.
