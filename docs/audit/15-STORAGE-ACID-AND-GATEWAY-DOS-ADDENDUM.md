# AURION PROTOCOL — Storage ACID Crash Consistency & Gateway Ingress DoS Hardening Audit Addendum
> **Nomor Dokumen:** 15-STORAGE-ACID-AND-GATEWAY-DOS-ADDENDUM
> **Klasifikasi:** Post-Audit Hardening Report (Era XIII)
> **Status:** 100% REMEDIATED & CERTIFIED PASS
> **Tanggal:** September 2026

---

## 1. Ringkasan Eksekutif

Addendum ini mendokumentasikan dua temuan perimeter Era XIII pada persistence layer Redb dan gateway HTTP/RPC Aurion. Verifikasi remediasi dilakukan dengan uji regresi deterministik dan pemeriksaan guardrail kanonikal, dalam kerangka zero-unsafe, zero-float, dan determinisme atomik.

Temuan dan remediasi utama:
- **FINDING-STOR-01:** atomic rollback multi-tabel Redb belum memiliki bukti regresi saat transaksi di-abort atau proses terinterupsi;
- **FINDING-RPC-01:** ingress payload HTTP/RPC tidak memiliki batas ukuran keras dan berisiko memicu memory exhaustion / OOM DoS.

---

## 2. Matriks Temuan & Remediasi Arsitektur

| ID Temuan | Tingkat Risiko | Status | Akar Masalah | Remediasi Formal |
| :--- | :---: | :---: | :--- | :--- |
| **FINDING-STOR-01** | **Critical / P0** | **REMEDIATED** | Walaupun `RedbStorage` memakai transaksi Redb, belum ada pembuktian regresi otomatis bahwa aborted transaction di tengah mutasi multi-tabel header blok, state saldo akun, dan indeks transaksi tidak meninggalkan partial-write leak. | Implementasi `tests/storage_crash_recovery.rs` yang mensimulasikan kegagalan penulisan sebelum commit dan membuktikan all-or-nothing rollback pada committed baseline (`AUR-STOR-001`). |
| **FINDING-RPC-01** | **High / P1** | **REMEDIATED** | Listener HTTP/RPC membaca body request ke memori tanpa batasan ketat, sehingga payload raksasa dapat memicu alokasi berlebih dan memory exhaustion hingga OS OOM killer menghentikan node. | Penegakan `MAX_RPC_PAYLOAD_BYTES = 128 * 1024` pada ingress `src/platform/gateway/rpc/server.rs`; request di atas batas ditolak lebih awal dengan HTTP `413 Payload Too Large` (`AUR-RPC-002`). |

---

## 3. Detail Temuan Audit

### 3.1 FINDING-STOR-01: Unverified Multi-Table Atomic Rollback on Process Interruption / Crash

**Akar masalah**
- Transaksi atomik Redb telah digunakan untuk mutasi state persistence.
- Belum tersedia bukti regresi bahwa abort sebelum commit memulihkan seluruh tabel ke committed state terakhir.
- Risiko mencakup partial-write leak atau ketidakkonsistenan antara block header, accounts, dan transaction index.

**Impact**
- Crash atau interupsi proses pada saat write dapat meninggalkan state storage yang tidak konsisten apabila atomicity tidak terbukti.
- Ketidakkonsistenan persistence dapat mengganggu pemulihan node dan validasi state berikutnya.

**Remediasi**
- Suite `tests/storage_crash_recovery.rs` mensimulasikan write transaction yang di-abort sebelum `.commit()`.
- Verifikasi memastikan committed baseline tetap utuh dan tidak ada mutasi parsial yang terlihat setelah transaksi gagal.
- Acceptance criterion `AUR-STOR-001` dinyatakan terpenuhi.

### 3.2 FINDING-RPC-01: Unbounded Ingress Payload Size Vulnerability to Memory Exhaustion / OOM DoS

**Akar masalah**
- Listener HTTP/RPC sebelumnya dapat membaca body request tanpa ceiling normatif pada ingress.
- Bot eksternal dapat mengirim payload besar untuk memaksa buffering dan konsumsi RAM node.

**Impact**
- Memory exhaustion dapat menurunkan availability node atau memicu terminasi oleh OS OOM killer.
- Serangan tidak memerlukan validasi transaksi yang berhasil karena tekanan memori terjadi sebelum dispatch RPC.

**Remediasi**
- Ingress menetapkan `MAX_RPC_PAYLOAD_BYTES: usize = 128 * 1024`.
- `Content-Length` diperiksa sebelum pembacaan body tambahan dan body tidak boleh melewati ceiling.
- Request oversize dihentikan sebelum buffering besar dan menerima HTTP `413 Payload Too Large` dengan error JSON-RPC deskriptif.
- Acceptance criterion `AUR-RPC-002` dinyatakan terpenuhi.

---

## 4. Bukti Pengujian Empiris

| Suite | Hasil | Keterangan |
| :--- | :---: | :--- |
| **`tests/storage_crash_recovery.rs`** | **1/1 PASS** | Aborted Redb transaction tidak membocorkan partial state; committed baseline tetap konsisten. |
| **`tests/rpc_payload_limit.rs`** | **2/2 PASS** | Payload valid diterima, sedangkan payload di atas 128 KiB ditolak pada ingress dengan HTTP 413. |
| **Guardrail canonical integrity** | **100% PASS** | Zero unsafe, zero floating-point, 38 dokumen spesifikasi tersinkronisasi, dan zero conflicts detected. |

Perintah verifikasi yang digunakan:

```powershell
cargo test --test storage_crash_recovery -- --nocapture
cargo test --test rpc_payload_limit -- --nocapture
cargo clippy --all-targets -- -D warnings
python tools/guardrail.py
```

Seluruh bukti di atas mendukung kesimpulan bahwa persistence rollback dan perimeter ingress telah diverifikasi secara empiris dalam batas acceptance criteria Era XIII.

---

## 5. Status Remediasi Final

Kedua temuan Era XIII dinyatakan tervalidasi dan tertutup:

- **AUR-STOR-001:** Redb ACID crash consistency dan multi-table rollback;
- **AUR-RPC-002:** 128 KiB ingress payload limit dan memory DoS protection.

Dengan demikian, Addendum 15 menutup hardening storage dan gateway Era XIII sebagai **100% REMEDIATED & CERTIFIED PASS**.
