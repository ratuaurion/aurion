# 09 — AURION GOVERNANCE SPECIFICATION
## Spesifikasi Formal Tata Kelola On-Chain, Hierarki Konstitusional, dan Aktivasi Peningkatan Protokol

> **Hierarki Dokumen:**  
> `00 — AURION CONSTITUTION` $\longrightarrow$ `01 — MONETARY` $\longrightarrow$ `02 — CONSENSUS` $\longrightarrow$ `03 — STATE TRANSITION` $\longrightarrow$ `04 — TRANSACTION` $\longrightarrow$ `05 — CRYPTOGRAPHY` $\longrightarrow$ `06 — SERIALIZATION & WIRE` $\longrightarrow$ `07 — GENESIS` $\longrightarrow$ `08 — VALIDATOR & STAKING` $\longrightarrow$ **`09 — GOVERNANCE SPECIFICATION`**  
>
> **Status:** AUDITED & LOCKED SPECIFICATION  
> **Klasifikasi:** Tata Kelola Protokol Layer 1 (On-Chain Governance & Upgrades)  
> **Versi:** 1.0.0-PROD  
> **Sifat Ketetapan:** Supremasi Konstitusi Absolut, Anti-Oligarki, Timelock Wajib

---

## 1. Doktrin Supremasi Konstitusional (Constitutional Supremacy Doctrine)

Tata kelola (*governance*) di dalam Aurion bukanlah kekuasaan mutlak tanpa batas. Tata kelola tunduk secara ketat di bawah **[AURION CONSTITUTION.md](file:///c:/Projects/aurion/AURION%20CONSTITUTION.md)**.

```text
┌────────────────────────────────────────────────────────────────────────┐
│                   HIERARKI KEDAULATAN PROTOKOL AURION                  │
├────────────────────────────────────────────────────────────────────────┤
│                       AURION CONSTITUTION                              │
│              (Akar Kedaulatan Tertinggi & Hukum Abadi)                 │
│                                  │                                     │
│                                  ▼                                     │
│                     PROTOCOL SPECIFICATIONS                            │
│           (Moneter, Konsensus, STF, Transaksi, Kriptografi)            │
│                                  │                                     │
│                                  ▼                                     │
│                      ON-CHAIN GOVERNANCE                               │
│           (Penyetelan Parameter & Sinyal Upgrade Perangkat Lunak)      │
│                                  │                                     │
│                                  ▼                                     │
│                      PERANGKAT LUNAK SIMPUL                            │
│                 (Implementasi Kode Mesin Konsensus)                    │
└────────────────────────────────────────────────────────────────────────┘
```

### 1.1 Batasan Kekuasaan Tata Kelola (Negative Governance Powers)
Tata kelola on-chain **DILARANG KERAS DAN TIDAK MEMILIKI KEMAMPUAN IMPLISIT** untuk:
1. Menambah batas suplai maksimum melampaui $66.000.000\ \text{AUR}$;
2. Mengubah rasio alokasi genesis (Creator 30%, Developer 5%, Community 65%);
3. Mengizinkan aritmetika floating-point pada lapisan konsensus;
4. Mencetak koin baru dari ketiadaan di luar aturan subsidi blok;
5. Membatalkan keabsahan koin yang memiliki *Reward Provenance Identifier* (RPI) sah;
6. Menyita saldo akun pengguna yang jujur tanpa bukti kriptografis ekuivokasi.

> **Invarian Pelindung Konstitusi:**  
> Setiap proposal tata kelola yang disetujui secara on-chain namun melanggar salah satu poin di atas dianggap **cacat hukum (*void ab initio*)**. Mesin State Transition Function ($\text{STF}$) simpul wajib menolak proposal tersebut secara otomatis sebagai *Invalid State Transition* ($\bot$).

---

## 2. Taksonomi Proposal Tata Kelola (Proposal Categories)

| Kategori Proposal | Objek Penyetelan | Ambang Kuorum | Ambang Kelulusan | Timelock Wajib |
| :--- | :--- | :---: | :---: | :---: |
| **Tipe A: Parameter Update (PUP)** | Parameter konsensus/jaringan yang diizinkan | $40\%$ | $> 66,67\%$ | $2\ \text{Epoch}$ ($\approx 14$ hari) |
| **Tipe B: Protocol Version Signal (PVS)** | Sinyal kesiapan migrasi versi software | $50\%$ | $\ge 80,00\%$ | $4\ \text{Epoch}$ ($\approx 28$ hari) |
| **Tipe C: Emergency Security Action (ESA)** | Pembekuan sementara peer DoS / hotfix bug | $66,67\%$ | $\ge 90,00\%$ | $1\ \text{Epoch}$ ($\approx 7$ hari) |

### 2.1 Ruang Lingkup Parameter Update Proposal (PUP)
Hanya parameter operasional berikut yang dapat disetel melalui proposal Tipe A:
- `min_fee_per_byte` (Batas minimum biaya transaksi antispam);
- `max_mempool_size` (Plafon kapasitas memori transaksi);
- `slashing_downtime_blocks` (Ambang batas toleransi blok offline);
- `max_commission_rate` (Plafon batas atas komisi validator).

---

## 3. Pipa Siklus Hidup Proposal (Proposal Lifecycle Pipeline)

Setiap proposal wajib melewati enam tahap state machine berurutan:

```text
[PENGAJUAN]    Akun menyerahkan naskah proposal + Deposit Minimal
     ↓
 [DEPOSIT]     Deposit mencapai ambang batas aktivasi (1.000 AUR)
     ↓
  [VOTING]     Jendela pemungutan suara dibuka selama 1 Epoch (10.000 Blok)
     ↓
 [EVALUASI]    Evaluasi Kuorum (≥ 40%) & Threshold Kelulusan (> 66,67%)
     ↓
 [TIMELOCK]    Masa tunda pengamanan selama 2 Epoch (20.000 Blok)
     ↓
[EKSEKUSI]     State mesin menerapkan parameter baru secara otomatis
```

---

## 4. Mekanisme Deposit dan Pencegahan Spam Proposal

Untuk mencegah pembanjiran proposal sampah (*proposal spamming*):

### 4.1 Persyaratan Deposit Minimum
$$\text{MinProposalDeposit} = 1.000\ \text{AUR} = 100.000.000.000\ \text{Quantum}\ (10^{11}\ Q)$$

1. **Jendela Akumulasi Deposit:** Pengusul memiliki waktu maksimum $2.000\ \text{blok}$ ($\approx 33\ \text{jam}$) untuk mengumpulkan deposit $\text{MinProposalDeposit}$.
2. **Pengembalian Deposit (*Refund*):** Jika proposal berhasil mencapai kuorum minimum $40\%$ pada akhir masa voting, deposit dikembalikan $100\%$ kepada para penyetor.
3. **Penyitaan dan Pembakaran (*Deposit Burning*):**  
   Deposit disita dan **DIBUANG PERMANEN (BURNED)** dari peredaran jika:
   - Proposal gagal mengumpulkan $\text{MinProposalDeposit}$ dalam batas waktu;
   - Proposal gagal mencapai kuorum partisipasi minimal $40\%$;
   - Proposal ditolak melalui suara Veto ($\text{NO\_WITH\_VETO} \ge 33,34\%$).

---

## 5. Mekanisme Pemungutan Suara (Voting Period & Rules)

### 5.1 Durasi Periode Voting
$$\text{VotingPeriod} = 1\ \text{Epoch} = 10.000\ \text{blok}\ (\approx 6,94\ \text{hari})$$

### 5.2 Hak Suara dan Opsi Pilihan
Hak suara dihitung berdasarkan bobot suara validator aktif ($w_v$) pada saat proposal memasuki masa voting. Delegator yang tidak setuju dengan pilihan validatornya berhak melakukan *vote overriding* dengan memberikan suara secara mandiri menggunakan kunci akun pribadinya.

Terdapat empat opsi pilihan suara resmi:
1. **YES (Setuju):** Mendukung pemberlakuan proposal.
2. **NO (Tidak Setuju):** Menolak proposal tanpa sanksi pembakaran deposit.
3. **NO_WITH_VETO (Penolakan Keras / Veto):** Menolak proposal karena dianggap merusak ekosistem atau melanggar etika konstitusi. Membakar deposit pengusul jika mencapai ambang batas veto.
4. **ABSTAIN (Netral):** Dihitung ke dalam pencapaian kuorum kehadiran, namun tidak memengaruhi persentase kelulusan threshold YES/NO.

---

## 6. Formula Matematika Kelulusan Proposal (Quorum & Threshold Formulas)

Misalkan $W_E$ adalah total bobot voting aktif pada epoch proposal, dan $V_{\text{YES}}, V_{\text{NO}}, V_{\text{VETO}}, V_{\text{ABSTAIN}}$ adalah akumulasi bobot suara untuk masing-masing opsi:

$$\text{TotalVotesCast} = V_{\text{YES}} + V_{\text{NO}} + V_{\text{VETO}} + V_{\text{ABSTAIN}}$$

$$\text{DecisiveVotes} = V_{\text{YES}} + V_{\text{NO}} + V_{\text{VETO}}$$

### Syarat 1: Pencapaian Kuorum Partisipasi
Proposal hanya dapat dievaluasi jika kuorum terpenuhi:

$$\text{TotalVotesCast} \ge \left\lfloor \frac{W_E \times 40}{100} \right\rfloor \quad (\ge 40\%)$$

Jika syarat ini tidak terpenuhi, proposal **GUGUR (*REJECTED_INSUFFICIENT_QUORUM*)** dan deposit dibakar.

### Syarat 2: Pemeriksaan Batas Veto (Veto Gate)
Jika kuorum terpenuhi, evaluasi batas veto:

$$\text{Jika } V_{\text{VETO}} \ge \left\lfloor \frac{\text{TotalVotesCast} \times 3.334}{10.000} \right\rfloor \quad (\ge 33,34\%)$$

Maka proposal **GUGUR DENGAN VETO (*REJECTED_VETOED*)** dan deposit dibakar permanen.

### Syarat 3: Ambang Batas Kelulusan Supermayoritas (Passing Threshold)
Proposal dinyatakan **LULUS (*PASSED*)** jika dan hanya jika suara `YES` melampaui dua pertiga suara penentu:

$$V_{\text{YES}} > \left\lfloor \frac{2 \times \text{DecisiveVotes}}{3} \right\rfloor \quad (> 66,6667\%)$$

---

## 7. Jeda Pengamanan Waktu dan Eksekusi (Timelock & Execution)

### 7.1 Durasi Timelock
Proposal yang dinyatakan lulus tidak langsung dieksekusi, melainkan memasuki masa tunda wajib (*Timelock Delay*):

$$\text{TimelockDelay} = 2\ \text{Epoch} = 20.000\ \text{blok}\ (\approx 13,88\ \text{hari})$$

### 7.2 Fungsi Perlindungan Timelock
1. **Pencegahan Serangan Kilat (*Flash-Loan / Sudden Takeover Defense*):** Memberikan jendela waktu transparansi publik untuk memantau perubahan parameter state.
2. **Hak Keluar Ekosistem (*Ragequit Window*):** Memberikan waktu yang cukup bagi pengguna atau validator yang tidak menyetujui parameter baru untuk melepas agunan (*unbonding*) atau melikuidasi posisi sebelum aturan baru aktif.

### 7.3 Eksekusi Deterministik State Machine
Tepat pada blok aktivasi:
$$H_{\text{exec}} = H_{\text{passed}} + \text{TimelockDelay}$$
Fungsi State Transition Function ($\text{STF}$) simpul memperbarui tabel konfigurasi parameter secara atomik tanpa intervensi manual.

---

## 8. Sinyal Peningkatan Versi Protokol (Protocol Upgrade Signaling)

Untuk perubahan besar yang melibatkan pembaruan logika biner perangkat lunak simpul:
1. Proposal Tipe B (`Protocol Version Signal`) menentukan nomor target versi `version_{N+1}` dan tinggi aktivasi masa depan $H_{\text{fork}}$.
2. Setelah proposal lulus dan melewati masa timelock 4 Epoch, validator yang telah meng-upgrade software simpulnya memancarkan bit versi baru pada header blok proposal:
   $$B.\text{version} == \text{TargetVersion}$$
3. Jika sekurang-kurangnya **90% blok dalam satu epoch utuh** memancarkan versi baru, transisi konsensus diaktifkan secara mulus pada blok awal epoch berikutnya.
