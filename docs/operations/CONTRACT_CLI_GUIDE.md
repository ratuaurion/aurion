# Panduan CLI Kontrak Cerdas Aurion (`aurion contract`)

> Panduan operasional untuk deploying, memanggil, dan menginspeksi kontrak AVM
> langsung dari terminal — memakai Contract SDK dan RPC Provider yang sama
> dengan library, dalam satu binary (`/bin/aurion`, AUR-ARCH-001).
>
> Referensi normatif: `Dokumen 02 (02-RPC-API-RULES.md) Bagian 4.4` dan
> `Dokumen 16 (16-SMART-CONTRACT-EXECUTION-SPECIFICATION.md)`.

---

## 1. Peta perintah

| Perintah | Jaringan | Keystore | Menyiarkan transaksi |
| :--- | :---: | :---: | :---: |
| `contract verify` | tidak | tidak | tidak |
| `contract deploy` (tanpa `--keystore`) | tidak | tidak | tidak |
| `contract deploy --keystore` | ya | ya | ya |
| `contract call` | ya | ya | ya (setelah konfirmasi) |
| `contract query` | ya | **tidak** | **tidak pernah** |
| `contract metadata` | ya | tidak | tidak |
| `contract publish-metadata` | ya | tidak | tidak |
| `contract inspect` | tidak (redb lokal) | tidak | tidak |

---

## 2. Alur lengkap (end-to-end)

### 2.1 Verifikasi bytecode (offline, tanpa jaringan)

```bash
aurion contract verify ./build/echo.avm
```

```text
======================================================================
  AURION AVM BYTECODE VERIFICATION
======================================================================
  Status          : VERIFIED_CANONICAL
  Bytecode Size   : 10 bytes
  Code Hash       : c571836a21ae2432a6b2fe2927eb2d0354b5093ef5a037099e5fe1211b7cd76b
  Valid JumpDests : 0
  Estimated Gas   : 52000

  Mode offline: tidak ada transaksi disiarkan.
  Tambahkan --keystore <path> untuk deploy sungguhan.
```

### 2.2 Deploy (dry-run dulu, lalu broadcast)

```bash
aurion contract deploy ./build/echo.avm \
  --name Echo \
  --runtime ./build/echo-runtime.avm \
  --metadata ./build/echo.json \
  --keystore my.keystore.json \
  --rpc http://127.0.0.1:8545
```

Pipeline lengkap Contract SDK:

```text
1. Verifikasi statis bytecode        (AUR-VM-005)
2. Dry-run STF pada state sandbox    (LANGKAH 1/2, belum disiarkan)
3. Prompt clear-signing              (butuh persetujuan [y/N])
4. Tanda tangan Ed25519 + broadcast  (LANGKAH 2/2)
```

```text
  Contract Address : aur1yg3zyg3...gaky4y
  Transaction Hash : 3f2a...c91d
  Code Hash        : c571836a21ae2432a6b2fe2927eb2d0354b5093ef5a037099e5fe1211b7cd76b
  Nonce            : 0
  Fee              : 52000 Quanta
  Gas (simulasi)   : 42
```

### 2.3 Publish metadata ke registry simpul (opsional)

Metadata bersifat **off-chain**. Agar CLI di simpul lain dapat memanggil kontrak
tanpa berkas lokal:

```bash
aurion contract publish-metadata <code_hash> --metadata ./build/echo.json \
  --rpc http://127.0.0.1:8545
```

> Registry **in-memory**: hilang saat restart simpul. Jalankan ulang perintah ini
> setelah restart, atau andalkan berkas metadata lokal (`--metadata`).

### 2.4 Query read-only (tanpa keystore)

```bash
aurion contract query aur1yg3zyg3...gaky4y ping --rpc http://127.0.0.1:8545
```

Menjalankan `aur_call` di simpul **tanpa menandatangani dan tanpa menyiarkan**.
Tidak ada keystore yang dibuka.

### 2.5 Call (mengubah state)

```bash
aurion contract call aur1yg3zyg3...gaky4y transfer 1000000000 \
  --keystore my.keystore.json \
  --rpc http://127.0.0.1:8545
```

```text
======================================================================
  LANGKAH 1/2 - HASIL SIMULASI (DRY-RUN, TIDAK DISIARKAN)
======================================================================
  Contract     : aur1yg3zyg3...gaky4y
  Method        : transfer(u64)
  Gas Used      : 42
  Return Data   : 0x0000...0000
  Decoded       : 1.000000000 AUR (1000000000 Q)

======================================================================
  LANGKAH 2/2 - BROADCAST
======================================================================
  TxID          : 0x9f3c...
```

---

## 2.6 Melihat hasil di Explorer (Smart Contract Visualization)

Explorer menerjemahkan calldata kontrak menjadi **metode + argumen terbaca
manusia**, bukan hex mentah.

```bash
# Dapatkan TxID dari output broadcast
aurion contract call <addr> transfer 1000000000 --yes \
  --keystore my.keystore.json --rpc http://127.0.0.1:8545 --json

# API: detail transaksi + interaksi kontrak ter-decode
curl -s http://127.0.0.1:8545/api/v1/transactions/<TxID> | jq .contract_interaction

# UI: buka http://127.0.0.1:8545/explorer lalu tempel TxID ke
#     kartu "Transaction Inspector"
```

Contoh keluaran `jq .contract_interaction`:

```json
{
  "kind": "call",
  "status": "finalized",
  "decode_status": "decoded",
  "contract_name": "Echo",
  "method": "transfer(u64,address)",
  "selector": "0x3056d944",
  "arguments": [
    { "index": 0, "name": "amount", "abi_type": "U64",     "value": "42" },
    { "index": 1, "name": "to",     "abi_type": "Address", "value": "aur14w46h2…" }
  ],
  "raw_payload_truncated": false
}
```

> **Penting:** tampilan "Unknown Method" muncul bila metadata kontrak belum
> terdaftar pada simpul yang melayani Explorer. Registry metadata bersifat
> **off-chain dan per-simpul** (hilang saat restart), jadi jalankan
> `aurion contract publish-metadata <code_hash> --metadata <file.json>`
> pada simpul tujuan agar argumen dapat ter-decode. Tanpa itu, Explorer tetap
> menampilkan selector dan calldata mentah sebagai fallback audit.

---

## 3. Format argumen

Argumen metode mengikuti tipe ABI dari metadata:

| Tipe | Format diterima | Contoh |
| :--- | :--- | :--- |
| `address` | Bech32m `aur`/`aurt` atau hex 64 | `aur1...`, `2222...` |
| `quantum` | bilangan bulat **Quanta**, atau desimal **AUR** | `1000000000`, `1.5` |
| `u64` / `u32` | bilangan bulat desimal | `42`, `7` |
| `bool` | `true`/`false`/`1`/`0`/`yes`/`no` | `true` |
| `hash256` | hex 64 | `abab...` |

Tiga cara penulisan argumen (boleh dikombinasikan):

```bash
# 1. JSON array
aurion contract call <addr> transfer --args '[1000000000]'

# 2. Flag berulang
aurion contract call <addr> transfer --args 1000000000

# 3. Posisional
aurion contract call <addr> transfer 1000000000
```

Semua nilai moneter diproses sebagai integer `Quantum` (u128) — **tanpa
arithmetic floating-point** (AUR-ARCH-012). Angka desimal seperti `1.5`
dikonversi lewat `Quantum::from_aur_str` presisi 9 desimal.

---

## 4. Opsi umum

| Flag | Default | Keterangan |
| :--- | :--- | :--- |
| `--rpc <url>` | `http://127.0.0.1:8545` | URL JSON-RPC simpul |
| `--keystore <path>` | `default.keystore.json` | Keystore Argon2 |
| `--password-stdin` | – | Ambil password dari stdin |
| `--metadata <file>` | – | Metadata kontrak JSON |
| `--yes`, `-y`, `--auto-approve` | – | Persetujuan otomatis (CI) |
| `--output json` | `text` | Keluaran JSON (AUR-CLI-007) |
| `--value <quanta>` | `0` | Nilai untuk metode payable |
| `--fee <quanta>` | otomatis | Override fee |
| `--nonce <n>` | dari simpul | Override nonce |

### Password

Mengikuti 3-tier `AUR-APP-01` (password tidak pernah diterima lewat argv):

1. `--password-stdin` atau stdin non-TTY
2. Environment `AURION_WALLET_PASSWORD`
3. Prompt interaktif (`rpassword`, tanpa echo)

```bash
export AURION_WALLET_PASSWORD='rahasia123'
aurion contract call <addr> ping --keystore my.keystore.json
```

---

## 5. Automasi / CI

```bash
aurion contract call <addr> transfer 1000000000 \
  --keystore ci.keystore.json \
  --password-stdin --yes \
  --output json | jq -r '.tx_id'
```

- `--yes` menonaktifkan prompt. Tanpa `--yes` pada lingkungan **non-TTY**, CLI
  menolak dengan pesan jelas (tidak hang, tidak diam-diam menandatangani).
- `--output json` menghasilkan JSON valid tanpa escape ANSI.
- Keluar dengan kode status **0** sukses, **1** gagal.

---

## 6. Warna terminal

| Warna | Makna |
| :--- | :--- |
| Hijau | Berhasil / terverifikasi |
| Merah | Revert / gagal |
| Kuning | Peringatan (dibatalkan, registry sementara) |
| Cyan | Informasi metadata |
| Redup | Catatan / nilai sekunder |

Otomatis dimatikan bila stdout bukan TTY atau environment `NO_COLOR` disetel
(https://no-color.org). `--output json` tidak pernah memuat escape ANSI.

---

## 7. Penanganan galat

Tidak ada panic pada kesalahan jaringan/parsing; CLI selalu mencetak pesan
ramah pengguna dan keluar dengan kode status 1.

| Situasi | Perilaku |
| :--- | :--- |
| Simpul tidak terjangkau | `Gagal membaca akun kontrak dari simpul: ... No connection could be made` |
| Alamat bukan kontrak | `<addr> bukan kontrak on-chain (tidak ada code_hash)` |
| Metadata belum terdaftar | Menyarankan `--metadata <file>` atau `publish-metadata` |
| Binding `code_hash` tidak cocok | Ditolak **sebelum** simulasi dijalankan |
| Metode tidak ada | `Metode 'x' tidak ada di metadata. Tersedia: a, b, c` |
| Jumlah argumen salah | `Metode 'f' memerlukan 2 argumen, diberikan 1` |
| Bytecode rusak | `Verifikasi bytecode gagal: ...` |
| Non-TTY tanpa `--yes` | `Lingkungan non-TTY: gunakan --yes/-y ...` |

---

## 8. Pengujian terhadap simpul lokal

### 8.1 Jalankan simpul

```bash
# Gateway RPC saja
./target/release/aurion rpc --bind 127.0.0.1:8545

# atau validator penuh dengan dana genesis
./target/release/aurion node --config devnet-validator-0.toml
```

### 8.2 Smoke test

```bash
# 1. Offline: verifikasi bytecode
aurion contract verify 6000f3

# 2. Offline: deploy tanpa keystore hanya memverifikasi
aurion contract deploy 6000f3 --output json

# 3. Online: metadata dari registry simpul
aurion contract metadata <code_hash> --rpc http://127.0.0.1:8545

# 4. Online: query read-only
aurion contract query <addr> ping --rpc http://127.0.0.1:8545

# 5. Online: broadcast (butuh keystore berdompet)
aurion contract call <addr> ping --keystore my.keystore.json --yes
```

### 8.3 Suite otomatis

```bash
cargo test --test contract_cli      # 17 test CLI kontrak
cargo test --test contract_rpc      # 15 test endpoint RPC kontrak
cargo test --test unified_cli       # regresi kompatibilitas CLI
```

`query_end_to_end_against_live_rpc_server` menjalankan CLI `query` terhadap
server RPC in-process sungguhan dan memverifikasi state on-chain tidak berubah.

---

## 9. Catatan desain penting

### Metadata off-chain

Aurion **tidak** menyimpan metadata/ABI on-chain. `Account` hanya memuat
`balance`, `nonce`, `code_hash`, dan `storage_root` (AUR-VM-006). Menambah
metadata ke state akan mengubah `state_root` setiap blok — yaitu mengubah
consensus, yang dilarang.

Pertahanan anti-penipuan tetap utuh: CLI **wajib** menjalankan `verify_binding()`
— metadata ditolak bila `code_hash`-nya berbeda dengan akun kontrak on-chain,
sebelum simulasi apa pun dijalankan.

### View-caller pada `query`

`query` tidak membuka keystore. Field `CALLER`/`ORIGIN` pada dry-run diisi
dengan alamat view-caller deterministik. Kontrak yang bergantung pada identitas
pemanggil akan melihat alamat tersebut, bukan alamat operator. Gunakan
`--keystore` bila membutuhkan `CALLER`/`ORIGIN` yang sebenarnya — tetap tanpa
penandatanganan.

### Backward compatibility

- `contract deploy <hex>` tanpa `--keystore` tetap hanya memverifikasi bytecode,
  persis seperti sebelumnya.
- `contract inspect` tetap membaca penyimpanan redb lokal.
- Tidak ada dependensi eksternal baru; CLI memakai parser internal yang sama
  dengan `aurion wallet` (AUR-ARCH-001: single binary).

  Belum ada transaksi yang disiarkan.

  Setujui pemanggilan 'transfer(u64)' dengan fee 10000 Quanta? Lanjutkan? [y/N]:

======================================================================
  LANGKAH 2/2 - HASIL BROADCAST
======================================================================
  Transaction Hash : 9ab4...77de
  Nonce            : 1
  Fee              : 10000 Quanta
  Gas (simulasi)   : 42
```

**Pemisahan simulasi vs broadcast bersifat tegas**: tidak ada penandatanganan
sebelum pengguna melihat hasil dry-run.

### 2.6 Metadata & inspect

```bash
aurion contract metadata aur1yg3zyg3...gaky4y --rpc http://127.0.0.1:8545
aurion contract inspect aur1yg3zyg3...gaky4y --db-path data/aurion.redb
```
