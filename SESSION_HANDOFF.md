# SESSION_HANDOFF.md — STATUS TERKINI & LANGKAH BERIKUTNYA

> **Tujuan file ini.** Deployment VPS kemungkinan memakan banyak sesi dan mungkin
> menghabiskan context window. Baca file ini untuk mengambil alih pekerjaan
> tanpa perlu membaca ulang seluruh riwayat percakapan.
>
> **Pasca-baca, baca juga `AGENTS.md`** — jangkar visi dan aturan keras.

---

## 1. POSISI TERKINI

| Aspek | Nilai |
| :--- | :--- |
| Commit dasar | `16f5a4a` |
| Branch | `main` (sinkron dengan `origin/main`) |
| Working tree | bersih |
| Chain ID | `1001` |
| Master Treasury (kanonik) | `aur1jjtqrlqy9suehhltnzt2ml4zwsr8ukpyvvhm2gw899u0e0w22qusq0pjql` |
| Host VPS (target) | `116.212.72.89` |

---

## 2. SUDAH SELESAI

| Komponen | Lokasi |
| :--- | :--- |
| Core STF (σ' = Υ(σ,B)) | `src/statemachine/state/stf.rs` |
| AVM (mesin kontrak) | `src/statemachine/vm/` |
| BFT Reactor (multi-round) | `src/consensus/bft/reactor.rs` |
| Contract SDK (Wallet + VM menyatu) | `src/platform/contract/` |
| Wallet (keystore Argon2, BIP-39, clear signing) | `src/platform/wallet/` |
| CLI terpadu `/bin/aurion` | `src/platform/cli/` |
| Faucet produksi (dana dari Treasury) | `src/platform/gateway/faucet.rs` |
| Explorer decoder (visualisasi kontrak) | `src/platform/gateway/contract_decode.rs` |
| Server RPC (`aur_*`) | `src/platform/gateway/rpc/` |
| Perbaikan bug `node --help` | `src/platform/cli/command.rs` |
| Audit kemurnian BFT (dokumen selaras kode) | commit `16f5a4a` |

**Pembersihan yang sudah dilakukan:**
- Kontaminasi terminologi mining dibersihkan dari lapisan produksi (receipt STF,
  prompt wallet, audit runner, conformance vectors).
- Skema fee lama 20% burn / 80% miner dihapus. Kanonik sekarang **0% burn /
  100% validator**, diturunkan dari `FEE_*_PERCENTAGE`.
- 5 artefak tanpa referensi dihapus; 4 panduan dokumentasi diperbarui agar tidak
  menunjuk ke berkas yang hilang.
- Klaim "single-slot finality" diselaraskan dengan implementasi sebenarnya
  (round-based BFT) di konstitusi, README, 20 dokumen, dan 16 berkas sumber.

---

## 3. STATUS VERIFIKASI

| Gerbang | Hasil |
| :--- | :--- |
| `cargo test --offline --no-fail-fast` | **517 lulus, 0 gagal**, 55 suite |
| `cargo clippy --offline --all-targets -- -D warnings` | hijau |
| `python tools/guardrail.py` | ALL INVARIANTS SATISFIED |

---

## 4. UTANG TEKNIS (WAJIB DIBACA)

### U-1. Proposer selection belum deterministik
`reactor.rs` mengambil `proposer_index` dari envelope proposal itu sendiri.
`validate_block_proposal` (`chain.rs`) memverifikasi height, parent hash,
timestamp, dan merkle root — **tidak** memverifikasi siapa yang berhak
mengajukan pada `(height, round)` tersebut.

Dampak untuk lingkungan publik tanpa kepercayaan:
- validator mana pun dapat mengirim proposal berulang (*proposer spam*);
- bonus proposer 20% dari block reward diberikan kepada siapa pun yang menang
  balasan proposing pertama, bukan kepada validator yang "jadwalnya" giliran.

**Rencana:** *Deterministic Leader Election* sebagai upgrade *hard-fork* terpisah;
empat langkahnya ada di `docs/operations/BFT_PURITY_AUDIT.md` §9.4.
**Jangan diimplementasikan tanpa persetujuan pemilik proyek.**

### U-2. Kunci seremoni masih dapat direkonstruksi
`CanonicalCeremonyKeypairs::new_deterministic()` memakai seed konstan
(`[0x01; 32]`, `[0x02; 32]`, `[0x11; 32]` … `[0x14; 32]`) yang **terbaca di
source publik**. Seed `[0x01; 32]` menghasilkan address `cb095697…95aad` yang
cocok dengan `creator_address_hex` di `GENESIS_CEREMONY.json`.

Selama hanya berjalan di localhost, tidak ada pihak ketiga yang dirugikan.
Sebelum go-public dengan genesis yang sama, private key Master Treasury
**harus dianggap bocor**.

### U-3. Tiga angka alokasi Treasury tidak konsisten

| Sumber | Angka |
| :--- | :--- |
| `AURION CONSTITUTION.md` | 66.000.000 AUR |
| `GENESIS_CEREMONY.json` → `initial_supply_aur` | 23.100.000 AUR |
| `GENESIS_CEREMONY.json` → `creator_allocation_aur` | 19.800.000 AUR |

Angka yang benar harus ditetapkan pemilik **sebelum** go-public, saat belum ada
blok yang mengikat siapa pun.

### U-4. Gerbang kemurnian hanya mencakup repo `aurion`
`tests/bft_purity_gate.rs` hanya memindai `aurion/src`. Repositori backend lain
(`aur-wallet`, `aurion-bootnode`, `aurion-explorer`, `aurion-market`) belum

---

## 5. LANGKAH BERIKUTNYA — TAHAP B (DEPLOYMENT VPS)

Target: `116.212.72.89`. Tiga fokus, dikerjakan berurutan.

### B-1. Generate Master Treasury Key
- Buat keystore Argon2 **nyata** (bukan `new_deterministic()`).
- Simpan mnemonic di luar repo. **Jangan pernah** menaruhnya di source.
- Catat address barunya dan cocokkan dengan yang dipakai saat inisialisasi faucet.
- Perhatikan U-2: bila memakai genesis yang sama, key lama dianggap bocor.

### B-2. Inisialisasi Faucet dari Treasury

```bash
aurion faucet init \
  --treasury-keystore <treasury.keystore.json> \
  --faucet-keystore  <faucet.keystore.json> \
  --fund 1000 \
  --password-stdin
```

- Perintah ini membangun transaksi `Transfer` bertanda tangan Treasury dan
  menyiarkannya ke mempool (AUR-MON §2.3) — dana faucet **wajib** mengalir lewat
  consensus reguler, bukan ditambah diam-diam.
- Verifikasi saldo Treasury berkurang dan rekening faucet bertambah.
- Jalankan simpul dengan faucet aktif:
  `aurion node --faucet-key <faucet.keystore.json> --password-stdin`

### B-3. Uji alur E2E di Testnet

Urut: **Claim → Deploy Contract → Call**, lalu verifikasi di Explorer.

```bash
# 1. Claim
aurion faucet claim <alamat-pengguna> --faucet-keystore <faucet.keystore.json>

# 2. Deploy kontrak
aurion contract deploy ./build/echo.avm \
  --keystore <user.keystore.json> --rpc http://<vps>:8545 --yes

# 3. Call
aurion contract call <alamat-kontrak> transfer 1000000000 \
  --keystore <user.keystore.json> --rpc http://<vps>:8545 --yes

# 4. Verifikasi visual di Explorer
curl -s http://<vps>:8545/api/v1/transactions/<TxID> | jq .contract_interaction
```

**Yang harus terbukti di Fase B:** faucet ter-dana dari Treasury, claim membuat
user mampu membayar fee, deploy menghasilkan kontrak dengan `code_hash` on-chain,
call ter-decode di Explorer, dan reserve floor faucet tetap terjaga.

### B-4. Setelah Fase B
- Tutup U-2 dan U-3 (kunci Treasury + angka alokasi).
- Usulkan Deterministic Leader Election sebagai hard-fork terpisah (U-1).

---

## 6. CATATAN OPERASIONAL

- **`--help` aman di mana pun** setelah nama perintah. Dulu `aurion node --help`
  memulai node sungguhan; sudah diperbaiki.
- **Jangan pakai `git add -A`.**INS pernah menghapus 15 aset gambar di
  `tools/assets/` di luar scope. Stagekan path secara spesifik.
- **Jangan menghapus** `GENESIS_CEREMONY.json` / `MAINNET_GENESIS_BLOCK.json`;
  keduanya di-embed lewat `include_str!`.
- **`.internal-tasks/` di-ignore git** — entri di sana tidak ter-push ke GitHub.
- **PowerShell + `git commit -m`** merusak commit message yang memuat tanda kutip.
  Tulis pesan ke berkas, lalu `git commit -F <berkas>`.
- **Teks non-ASCII** (CJK) sempat menyelinap ke beberapa komentar. Periksa
  berkasnya bila menambah teks Indonesia panjang.

tercakup aturan yang sama.
