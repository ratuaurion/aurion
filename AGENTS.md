# AGENTS.md — ANGGARAN VISI & ATURAN KERAS AURION

> **BACA FILE INI PADA AWAL SETIAP SESI.**
> Ini adalah sumber kebenaran pertama. Bila kamu hanya membaca satu berkas repo
> ini sebelum menulis kode, bacalah yang ini.

---

## 1. VISI BESAR (THE GRAND VISION)

Aurion adalah **Single Ecosystem & Single Binary**: satu pohon sumber, satu
identitas versi, satu model data, satu executable — `/bin/aurion`.

L1 hingga L5 **BUKAN** chain terpisah dan **BUKAN** binary terpisah. L1–L5 adalah
**Evolution Domains** yang berbagi *shared core* yang sama persis:

| Primitif inti (dipakai semua layer) | Lokasi |
| :--- | :--- |
| Blake3 (hash + KDF) | `src/primitives/crypto/` |
| Ed25519 (RFC 8032, non-malleable) | `src/primitives/crypto/ed25519.rs` |
| `Quantum(u128)` presisi 9 desimal | `src/primitives/core/quantum.rs` |
| SMT (state root) | `src/statemachine/state/smt.rs` |
| STF (σ' = Υ(σ,B)) | `src/statemachine/state/stf.rs` |
| AVM (mesin kontrak) | `src/statemachine/vm/` |
| BFT Reactor | `src/consensus/bft/reactor.rs` |
| Codepage kanonik | `src/codec/` |

**Semua kapabilitas dijalankan lewat `/bin/aurion`**: node, validator, wallet,
contract, faucet, explorer, rpc, metrics, governance, recovery, audit.

---

## 2. INVARIAN ARSITEKTUR — TIDAK BISA DILANGGAR

### I-1. ZERO FLOAT — WAJIB
Seluruh aritmetika moneter memakai `Quantum(u128)` dengan 9 desimal.
`1 AUR = 1_000_000_000 Q`. Dilarang `f32`/`f64` di mana pun, termasuk konstanta.
Pakai `checked_add`/`checked_sub`/`checked_mul` — jangan `+`/`*` telanjang.
Ditegakkan oleh `tools/guardrail.py`.

### I-2. SINGLE BINARY
Satu `Cargo.toml`, satu `src/main.rs`, satu `lib.rs`. Tidak menambah crate atau
repo terpisah untuk kapabilitas protokol baru. Repositori Aurora lain
(`aur-wallet`, `aurion-bootnode`, `aurion-explorer`, `aurion-market`) adalah
**backend terpisah**, bukan bagian dari binary konsensus.

### I-3. BFT MULTI-ROUND DENGAN TIMEOUT-DRIVEN LIVENESS
Aurion adalah **BFT berbasis ronde**, 2-fase (Prevote → Precommit).
**BUKAN** single-slot. Finalitas diperoleh dari **Sertifikat Kuorum (QC)**
dengan bobot suara **> 2/3** validator aktif — bukan dari slot waktu tetap.
Bila proposer diam melebihi `round_timeout`, protokol menaikkan ronde
(`reactor.rs:393`) sehingga jaringan tetap hidup.
Dilarang memakai istilah "single-slot" — ada gerbang yang memindai seluruh repo.

### I-4. PROPOSER OPPORTUNISTIC (First-Valid-Block-Wins)
Saat ini **validator mana pun** dapat mengajukan blok untuk suatu ketinggian.
`validate_block_proposal` memeriksa height, parent hash, timestamp, merkle root —
**belum** memeriksa hak proposer terjadwal.
Rencana **Deterministic Leader Election** dijadwalkan sebagai *hard-fork*
terpisah (lihat `docs/operations/BFT_PURITY_AUDIT.md` §9.4).

### I-5. ZERO UNSAFE
`#![forbid(unsafe_code)]` di setiap file produksi.

### I-6. CLEAR SIGNING — WAJIB
Tidak ada penandatanganan buta. Semua mutasi (deploy/call kontrak) wajib:
1. **Dry-run / simulasi** dulu (`aur_call`, `ContractInstance`),
2. **Intent terikat** — `ContractIntent` diverifikasi terhadap transaksi
   (`verify_against`); isi yang ditandatangani adalah byte yang disimulasikan,
3. **Persetujuan manusia** sebelum tanda tangan Ed25519 dibuat.

Intent mismatch diblokir **sebelum** masuk mempool (`-32001`).
`query` tidak pernah menandatangani atau menyiarkan.

### I-7. FAUCET KONSTITUSIONAL
Faucet **WAJIB** menarik likuiditas secara eksklusif dari rekening operasional
**Master Treasury** dan **DILARANG** mencetak koin (AUR-MON §2.3).
`fund_from_treasury` membangun transaksi `Transfer` bertanda tangan Treasury
yang disiarkan lewat mempool — bukan menambah saldo secara diam-diam.
Ada *reserve floor*: saldo faucet tidak boleh turun di bawahnya.

---

## 3. ATURAN KERAS UNTUK AGEN

### A. DILARANG MENEBAK
Jangan menulis berdasarkan asumsi tentang apa yang "mungkin" ada. **Verifikasi
dulu dengan perintah:**

```powershell
# mencari referensi di kode
Get-ChildItem src -Recurse -Filter *.rs | Select-String -Pattern 'pola_apa'
# mencari nama file
Get-ChildItem -Recurse -File -Filter '*nama*'
# apakah sebuah file benar-benar dipakai (sebelum menyebut "dipakai")
Get-ChildItem src,tests,tools -Recurse -File | Select-String -SimpleMatch 'file.json'
```

Kegagalan nyata yang pernah terjadi di repo ini — jangan diulang:

- CLI `faucet` pernah mencetak nilai **palsu** (`status: "DISPENSED"` plus tx
  hash rekaan) tanpa menyentuh jaringan. Test-nya "lulus" justru karena
  kebohongannya.
- `tests/wallet.rs` pernah **meng-assert** pembagian fee 20/80 yang salah, sehingga
  bug tersembunyi di balik test yang hijau. Test harus mengukur hal yang benar.
- Skema fee 20/80 sempat divalidasi oleh `audit/runner.rs` padahal
  `MonetaryState::split_fee` sudah 0% burn / 100% validator — validasi menguji
  skema yang sudah tidak berlaku.

### B. SEBELUM MENGUBAH FILE
1. Baca `AGENTS.md` (file ini).
2. Baca `SESSION_HANDOFF.md` untuk status terkini dan utang teknis.
3. Baca dokumen relevan di `docs/Constitutions/` bila menyangkut aturan.
4. `grep` dulu apa yang benar-benar ada sebelum mengedit.

### C. SETELAH MENGUBAH — WAJIB VERIFIKASI

```bash
cargo clippy --offline --all-targets -- -D warnings   # wajib hijau
cargo test --offline --no-fail-fast                    # tidak boleh ada FAIL
python tools/guardrail.py                               # ALL INVARIANTS SATISFIED
```

Bila ada test gagal, sebutkan. Jangan menyatakan hijau bila tidak.

### D. JANGAN UBAH YANG TIDAK DIMINTA
Hanya ubah apa yang diminta; jangan ikut refactor di luar scope.
Dilarang mengubah logika konsensus kecuali pemilik proyek memintanya eksplisit.
Jangan menghapus berkas inti: `GENESIS_CEREMONY.json` dan
`MAINNET_GENESIS_BLOCK.json` di-embed lewat `include_str!` di
`src/primitives/genesis/ceremony.rs`; menghapusnya merusak build.

### E. JUJUR SOAL KEADAAN
- Bila menemukan dokumen bertentangan dengan implementasi, **laporkan** —
  jangan diperbaiki diam-diam tanpa izin.
- Bila tidak yakin, katakan tidak yakin. Jangan mengarang nomor, nama file,
  atau hasil pengujian.

---

## 4. PINTUAN VERIFIKASI CEPAT

```bash
# konsensus benar-benar multi-round (bukan single-slot)
cargo test --offline --test bft_purity_gate

# tidak ada kontaminasi terminologi mining
Get-ChildItem src -Recurse -Filter *.rs | Select-String -Pattern 'miner|mining'
```

---

## 5. DOKUMEN YANG WAJIB DIBACA SAAT RELEVAN

| Topik | Berkas |
| :--- | :--- |
| Aturan protokol | `docs/Constitutions/AURION CONSTITUTION.md` |
| Anatomy transaksi kontrak | `docs/Application-Rules-Layer/application/02-RPC-API-RULES.md` |
| CLI terpadu | `docs/Application-Rules-Layer/application/15-UNIFIED-CLI-SPECIFICATION.md` |
| Eksekusi kontrak | `docs/Application-Rules-Layer/application/16-SMART-CONTRACT-EXECUTION-SPECIFICATION.md` |
| Panduan CLI kontrak | `docs/operations/CONTRACT_CLI_GUIDE.md` |
| Audit kemurnian BFT & proposer | `docs/operations/BFT_PURITY_AUDIT.md` |
| Status sesi berjalan | `SESSION_HANDOFF.md` |

**Jangan mengubahnya diam-diam** — itu keputusan pemilik proyek.
