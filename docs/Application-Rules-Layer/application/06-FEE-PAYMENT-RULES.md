# 06 — AURION FEE & PAYMENT APPLICATION RULES
## Standar Estimasi Biaya Transaksi, Transparansi Pembakaran 20%, dan Kebijakan Pembayaran Pedagang (Merchant)

> **Hierarki Dokumen:**  
> `AURION CONSTITUTIONS` $\longrightarrow$ `PROTOCOL SPECIFICATIONS` $\longrightarrow$ `00-APPLICATION-RULES` $\longrightarrow$ **`06-FEE-PAYMENT-RULES`**  
>
> **Status:** RATIFIED & LOCKED APPLICATION SPECIFICATION  
> **Klasifikasi:** Standar Biaya & Logika Pembayaran Komersial (Fee & Payment Logic)  
> **Versi:** 1.0.0-PROD  
> **Sifat Ketetapan:** Normatif Wajib (RFC 2119 / RFC 8174), Zero-Float, Anti-Overcharging

---

## 1. Perbedaan Definisi Biaya: Protokol vs Aplikasi

Untuk mencegah kebingungan pengguna dan kesalahan estimasi perangkat lunak, ekosistem Aurion mendefinisikan tiga tingkatan biaya secara presisi:

```text
┌────────────────────────────────────────────────────────────────────────┐
│               TAKSONOMI BIAYA TRANSAKSI APLIKASI                       │
├──────────────────┬─────────────────────────────────────────────────────┤
│ ISTILAH          │ DEFINISI & PENERAPAN PADA APLIKASI                  │
├──────────────────┼─────────────────────────────────────────────────────┤
│ Estimated Fee    │ Nilai rekomendasi dinamis dari simpul RPC           │
│ (Biaya Estimasi) │ berdasarkan kepadatan mempool saat ini.             │
├──────────────────┼─────────────────────────────────────────────────────┤
│ Maximum Fee      │ Batas tertinggi yang disetujui pengguna (plafon)    │
│ (Batas Maksimum) │ untuk mencegah lonjakan biaya mendadak.             │
├──────────────────┼─────────────────────────────────────────────────────┤
│ Actual Fee       │ Nilai pasti integer Quantum yang didebet on-chain   │
│ (Biaya Aktual)   │ dan tercatat secara permanen pada header transaksi. │
└──────────────────┴─────────────────────────────────────────────────────┘
```

1. **Batas Minimum Protokol:** `Actual Fee` **MUST** $\ge 10.000\ \text{Quantum}$ ($0,0001\ \text{AUR}$). Wallet **MUST NOT** mengizinkan pembuatan transaksi dengan fee di bawah ambang ini.
2. **Ketiadaan Konsep Gas Tersembunyi:** Aurion beroperasi dengan model biaya langsung (*Deterministic Byte/Execution Fee*) tanpa ketidakpastian gas limit ala EVM. Seluruh nilai `fee` didebet secara pasti dari akun pengirim.

---

## 2. Kewajiban Transparansi Pembakaran Biaya (20% Burn Disclosure)

Mengukuhkan mandat **Bab 3 [AURION CONSTITUTION.md](file:///c:/Projects/aurion/docs/Constitutions/AURION%20CONSTITUTION.md)** dan **[AURION-MONETARY-POLICY-SPECIFICATION.md](file:///c:/Projects/aurion/docs/Constitutions/AURION-MONETARY-POLICY-SPECIFICATION.md)**:

1. **Pengungkapan pada Antarmuka Dompet:**  
   Setiap perangkat lunak dompet (wallet) **MUST** menyajikan rincian pembagian biaya transaksi sebelum pengguna menekan tombol persetujuan kirim:
   - Total Biaya Jaringan: $\mathcal{F}_{\text{total}}$
   - Bagian Dimusnahkan Permanen (Burn): $\mathcal{F}_{\text{burned}} = \lfloor (\mathcal{F}_{\text{total}} \times 20) / 100 \rfloor$
   - Bagian Hadiah Produser Blok (Miner): $\mathcal{F}_{\text{miner}} = \mathcal{F}_{\text{total}} - \mathcal{F}_{\text{burned}}$
2. **Rasional Pendidikan Moneter:**  
   Wallet **SHOULD** menyertakan keterangan edukasi singkat: *"20% dari biaya ini dimusnahkan secara permanen untuk menjaga kelangkaan absolut 66 Juta AUR."*

---

## 3. Tingkatan Kecepatan Biaya Transaksi (Fee Tiers)

Simpul RPC dan pustaka SDK **SHOULD** menyediakan tiga tingkatan estimasi biaya dinamis:

| Tingkat Kecepatan | Formula Estimasi Biaya | Target Waktu Masuk Blok | Skenario Penggunaan |
| :--- | :--- | :--- | :--- |
| **Ekonomis (Low)** | $\text{BaseFee} = 10.000\ Q$ | $\le 3\ \text{Blok}$ ($\approx 3\ \text{menit}$) | Transfer terjadwal, rebalancing internal. |
| **Standar (Normal)**| $\text{BaseFee} \times 1,25 = 12.500\ Q$| Blok Berikutnya ($H+1$) | Pembayaran harian, transfer personal. |
| **Prioritas (High)**| $\text{BaseFee} \times 2,00 = 20.000\ Q$| Prioritas Puncak $H+1$ | Arbitrase bursa, transaksi mendesak. |

---

## 4. Logika Pemrosesan Pembayaran Pedagang (Merchant Payment Rules)

Gerbang pembayaran (*Payment Gateway*), kasir ritel, dan e-commerce **MUST** mematuhi aturan penanganan pembayaran berikut:

### 4.1 Pembayaran Pas (Exact Payment)
- **Kondisi:** $\text{ReceivedAmount} == \text{InvoiceAmount}$.
- **Tindakan:** Begitu transaksi mencapai status `FINALIZED`, pesanan ditandai sebagai **LUNAS (*PAID*)** secara otomatis.

### 4.2 Pembayaran Kurang (Underpayment)
- **Kondisi:** $\text{ReceivedAmount} < \text{InvoiceAmount}$.
- **Tindakan:**
  - Gerbang pembayaran **MUST NOT** menandai pesanan sebagai lunas.
  - Sistem **MUST** memperbarui sisa tagihan:
    $$\text{RemainingDue} = \text{InvoiceAmount} - \text{ReceivedAmount}$$
  - Antarmuka **MUST** menampilkan sisa tagihan yang harus dibayar oleh pembeli beserta timer kedaluwarsa yang diperbarui.

### 4.3 Pembayaran Lebih (Overpayment)
- **Kondisi:** $\text{ReceivedAmount} > \text{InvoiceAmount}$.
- **Tindakan:**
  - Pesanan ditandai sebagai **LUNAS**.
  - Sistem **MUST** mencatat kelebihan pembayaran (*Excess Balance*):
    $$\text{ExcessQuanta} = \text{ReceivedAmount} - \text{InvoiceAmount}$$
  - Sistem **SHOULD** menyediakan mekanisme pengembalian dana (*refund reference*) ke alamat pengirim atau menyimpan saldo lebih pada akun kredit pembeli.

### 4.4 Batas Waktu Kedaluwarsa Tagihan (Invoice Expiration)
1. Setiap invoice pembayaran **SHOULD** memiliki masa berlaku antara $15$ hingga $60\ \text{menit}$ sejak diterbitkan.
2. Jika pembayaran baru tiba setelah invoice kedaluwarsa (*Late Payment*):
   - Sistem pembayaran **MUST NOT** membatalkan transaksi pada level blockchain (karena transaksi yang final tidak dapat dibatalkan).
   - Sistem **MUST** menandai pesanan sebagai *Expired-Paid* dan mengarahkan ke alur rekonsiliasi manual atau pengembalian dana otomatis (*Automated Refund*).

---

## 5. Standar Pengembalian Dana Otomatis (Automated Refunds)

Jika merchant perlu mengembalikan dana kepada pembeli:
1. Dana pengembalian **MUST** dikirimkan kembali ke alamat `sender` yang tertera pada transaksi asli.
2. Transaksi pengembalian **SHOULD** menyertakan invoice ID atau hash transaksi asli pada field memo/payload untuk memudahkan rekonsiliasi otomatis pembeli:
   $$\text{RefundMemo} = \text{"REFUND:"} \parallel \text{OriginalTxID}[0..16]$$
3. Biaya transaksi jaringan untuk pengembalian dana **MUST NOT** dibebankan melebihi nilai transfer pengembalian itu sendiri.
