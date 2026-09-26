# Panduan Pengujian Endpoint Kontrak Cerdas (`aur_call`, `aur_estimateGas`, `aur_sendRawTransaction`)

> Berkas ini adalah panduan operasional untuk menguji endpoint kontrak Aurion
> pada simpul yang sedang berjalan. Referensi normatif: `Dokumen 02
> (02-RPC-API-RULES.md) Bagian 4.4`.
>
> Cakupan: `aur_call`, `aur_estimateGas`, `aur_getContractMetadata`,
> `aur_getCode`, `aur_sendContractMetadata`, dan `aur_sendRawTransaction`
> dengan validasi intent.

---

## 0. Prasyarat

1. Build binary tunggal Aurion:

   ```powershell
   cargo build --release
   ```

2. Jalankan simpul RPC pada port lokal (default devnet `:8545`):

   ```powershell
   .\target\release\aurion.exe rpc --bind 127.0.0.1:8545
   ```

   Atau validator penuh (RPC + BFT) bila ingin menguji alur broadcast sampai blok:

   ```powershell
   .\target\release\aurion.exe node --config .\devnet-validator-0.toml
   ```

3. Tetapkan variabel lingkungan (PowerShell):

   ```powershell
   $RPC = "http://127.0.0.1:8545/rpc"
   $SENDER_PUBKEY = "<64 hex pubkey Ed25519>"
   $CHAIN_ID = "1001"
   ```

4. Bangun payload transaksi **kanonik** (hex). Semua endpoint kontrak menerima
   **serialisasi kanonikal `Transaction`** (184 byte header + payload), bukan
   JSON objek. Cara tercepat adalah memakai Contract SDK di dalam binary yang
   sama (lihat `examples/contract_sdk.rs`).

   Layout kanonikal:

   ```text
   version(2) chain_id(4) tx_type(1) flags(1) sender(32) recipient(32)
   nonce(8) amount(16) fee(16) valid_until(8) payload_len(4) payload(..) signature(64)
   ```

   `$RAW_TX_HEX` di bawah adalah hex dari blob tersebut.

---

## 1. Sanity check koneksi

```powershell
curl.exe -s -X POST $RPC -H "Content-Type: application/json" `
  -d '{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"aur_chainId\",\"params\":[]}'
```

Ekspektasi: `{"jsonrpc":"2.0","id":1,"result":"1001"}`

Dengan `httpie`:

```powershell
http POST $RPC jsonrpc=2.0 id=1 method=aur_chainId params:=[]
```

---

## 2. `aur_estimateGas`

```powershell
curl.exe -s -X POST $RPC -H "Content-Type: application/json" `
  -d "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"aur_estimateGas\",\"params\":[\"$RAW_TX_HEX\"]}"
```

Ekspektasi sukses:

```json
{"jsonrpc":"2.0","id":2,"result":{"gas_used":96,"gas_limit":1000000,"success":true,"reason":null}}
```

Bila kontrak revert, **tetap HTTP 200** dengan `success:false` dan `reason` terisi.
Ini bukan galat JSON-RPC.

---

## 3. `aur_call` (dry-run jarak jauh)

```powershell
curl.exe -s -X POST $RPC -H "Content-Type: application/json" `
  -d "{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"aur_call\",\"params\":[\"$RAW_TX_HEX\"]}"
```

Ekspektasi:

```json
{
  "jsonrpc":"2.0","id":3,
  "result":{
    "success":true,
    "gas_used":96,
    "return_data":"0000...002a",
    "reason":null,
    "deployed_contract":null,
    "storage_changes":0,
    "gas_limit":1000000
  }
}
```

### 3.1 Bukti keras: `aur_call` tidak memutasi state

```powershell
#_before_
curl.exe -s -X POST $RPC -H "Content-Type: application/json" `
  -d "{\"jsonrpc\":\"2.0\",\"id\":4,\"method\":\"aur_getAccount\",\"params\":[\"$SENDER_BECH32M\"]}"

# jalankan 20x aur_call
1..20 | ForEach-Object { curl.exe -s -X POST $RPC -H "Content-Type: application/json" `
  -d "{\"jsonrpc\":\"2.0\",\"id\":5,\"method\":\"aur_call\",\"params\":[\"$RAW_TX_HEX\"]}" | Out-Null }

# _after_
curl.exe -s -X POST $RPC -H "Content-Type: application/json" `
  -d "{\"jsonrpc\":\"2.0\",\"id\":4,\"method\":\"aur_getAccount\",\"params\":[\"$SENDER_BECH32M\"]}"
```

`balance` dan `nonce` **wajib identik** sebelum dan sesudah. Simulasi berjalan pada
snapshot sandbox berukuran konstan; state on-chain tidak pernah disentuh.

### 3.2 Parameter gas limit opsional

```powershell
# gas limit melebihi batas kanonik -> -32602
curl.exe -s -X POST $RPC -H "Content-Type: application/json" `
  -d "{\"jsonrpc\":\"2.0\",\"id\":6,\"method\":\"aur_call\",\"params\":[\"$RAW_TX_HEX\",\"1000001\"]}"
```

Ekspektasi: `{"error":{"code":-32602,"message":"gas limit 1000001 di luar rentang 1..=1000000"}}`

---

## 4. `aur_getCode` (binding kontrak)

```powershell
curl.exe -s -X POST $RPC -H "Content-Type: application/json" `
  -d "{\"jsonrpc\":\"2.0\",\"id\":7,\"method\":\"aur_getCode\",\"params\":[\"$CONTRACT_BECH32M\"]}"
```

Ekspektasi:

```json
{"jsonrpc":"2.0","id":7,"result":{"address":"aur1...","code_hash":"<64 hex>","storage_root":"<64 hex>","is_contract":true,"code_available":false,"note":"..."}}
```

`code_available` selalu `false` karena Aurion tidak menyimpan bytecode on-chain,
hanya `code_hash` (AUR-VM-006).

---

## 5. Metadata kontrak (off-chain, opsional)

### 5.1 Belum terdaftar -> `-32002`

```powershell
curl.exe -s -X POST $RPC -H "Content-Type: application/json" `
  -d "{\"jsonrpc\":\"2.0\",\"id\":8,\"method\":\"aur_getContractMetadata\",\"params\":[\"$CODE_HASH_HEX\"]}"
```

Ekspektasi: `{"error":{"code":-32002,"message":"Metadata for code_hash ... is not registered..."}}`

### 5.2 Daftarkan lalu ambil

```powershell
$metaJson = Get-Content .\metadata\echo.json -Raw
curl.exe -s -X POST $RPC -H "Content-Type: application/json" `
  -d "{\"jsonrpc\":\"2.0\",\"id\":9,\"method\":\"aur_sendContractMetadata\",\"params\":[\"$CODE_HASH_HEX\",$(ConvertTo-Json $metaJson -Compress)]}"
curl.exe -s -X POST $RPC -H "Content-Type: application/json" `
  -d "{\"jsonrpc\":\"2.0\",\"id\":10,\"method\":\"aur_getContractMetadata\",\"params\":[\"$CODE_HASH_HEX\"]}"
```

Metadata yang `code_hash`-nya berbeda dari parameter akan ditolak `-32602`.
Registry bersifat in-memory per simpul: hilang saat restart, tidak tercermin
dalam konsensus.

---

## 6. Broadcast + validasi intent

### 6.1 Bentuk lama (tetap didukung, backward compatible)

```powershell
curl.exe -s -X POST $RPC -H "Content-Type: application/json" `
  -d "{\"jsonrpc\":\"2.0\",\"id\":11,\"method\":\"aur_sendRawTransaction\",\"params\":[\"$SIGNED_TX_HEX\",\"$SENDER_PUBKEY\"]}"
```

### 6.2 Bentuk dengan intent binding (anti-blind-signing)

```text
params = [ signed_tx_hex, sender_pubkey_hex, intent_payload_hash, intent_code_hash ]
```

* `intent_payload_hash` = `blake3(tx.payload)`.
* Deploy: `intent_code_hash` = `blake3(tx.payload)`.
* Call: `intent_code_hash` = `code_hash` kontrak on-chain.

```powershell
curl.exe -s -X POST $RPC -H "Content-Type: application/json" `
  -d "{\"jsonrpc\":\"2.0\",\"id\":12,\"method\":\"aur_sendRawTransaction\",\"params\":[\"$SIGNED_TX_HEX\",\"$SENDER_PUBKEY\",\"$PAYLOAD_HASH_HEX\",\"$CODE_HASH_HEX\"]}"
```

Ekspektasi sukses: `{"jsonrpc":"2.0","id":12,"result":"0x<txid>"}`

Ekspektasi penolakan intent salah:

```json
{"error":{"code":-32001,"message":"Transaction Rejected: Intent mismatch: payload_hash does not match the signed transaction","data":{...}}}
```

Perhatikan `-32001` (TX_REJECTED) dan mempool **tetap kosong**.

### 6.3 Bytecode rusak ditolak sebelum mempool

Kirim `ContractCall` dengan payload `0xFF` (bukan opcode AVM):

```json
{"error":{"code":-32001,"message":"Transaction Rejected: Bytecode kontrak gagal verifikasi statis (AUR-VM-005)"}}
```

---

## 7. Uji ketahanan DoS

Token bucket: 20 permintaan/detik, burst 40. Kirim 60 permintaan `aur_call`
berturutan tanpa jeda:

```powershell
1..60 | ForEach-Object {
  curl.exe -s -X POST $RPC -H "Content-Type: application/json" `
    -d "{\"jsonrpc\":\"2.0\",\"id\":9,\"method\":\"aur_call\",\"params\":[\"$RAW_TX_HEX\"]}"
} | Select-String '32004'
http POST $RPC jsonrpc=2.0 id=5 method=aur_sendRawTransaction  params:="[\"$SIGNED_TX_HEX\",\"$SENDER_PUBKEY\"]"
```

---

## 9. Checklist verifikasi

Centang setiap butir sebelum menyatakan sebuah simpul "layak kontrak".

### 9.1 Kontrak endpoint dengan sisi klien (`RpcProvider`)

| # | Verifikasi | Endpoint / cara cek |
| :---: | :--- | :--- |
| 1 | `aur_call` mengembalikan `success`, `gas_used`, `return_data`, `reason`, `deployed_contract`, `storage_changes`, `gas_limit` | Bagian 3 |
| 2 | `aur_estimateGas` `gas_used` **identik** dengan simulasi lokal SDK (kode sama: `state::sandbox::estimate_gas`) | Bagian 2 + `cargo test --test contract_rpc` |
| 3 | `aur_getAccount` mengembalikan `code_hash` + `is_contract` | `aur_getAccount` |
| 4 | `aur_getCode` mengembalikan `code_hash` dengan `code_available: false` | Bagian 4 |
| 5 | `aur_getContractMetadata` mengembalikan `-32002` bila belum terdaftar | Bagian 5.1 |
| 6 | `aur_sendRawTransaction` menerima transaksi bertanda tangan `KeystoreSigner` | Bagian 6.1 |
| 7 | Intent mismatch ditolak `-32001` **sebelum** masuk mempool | Bagian 6.2 |
| 8 | `aur_call` tidak memutasi `balance`/`nonce` on-chain | Bagian 3.1 |
| 9 | Galat `-32004` muncul saat kuota simulasi habis, simpul tetap sehat | Bagian 7 |
| 10 | Seluruh endpoint lama tetap berfungsi (kompatibilitas mundur) | `cargo test --test rpc_api` |

### 9.2 Gerbang kualitas repositori

```powershell
cargo clippy --all-targets -- -D warnings   # harus 0 error, 0 warning
cargo test --no-fail-fast                   # harus 100% hijau
python tools/guardrail.py                    # harus ALL INVARIANTS SATISFIED
```

### 9.3 Catatan khusus testnet/mainnet nyata

- [ ] Ganti `aur` dengan HRP `aurt` pada alamat Bech32m bila jaringan testnet.
- [ ] Registry metadata bersifat **in-memory**: metadata hilang saat restart simpul.
      Untuk produksi, jalankan ulang `aur_sendContractMetadata` untuk setiap
      kontrak, atau simpan metadata JSON di repositori klien dan andalkan
      `verify_binding` terhadap `code_hash` on-chain.
- [ ] `aur_call` menguji state **latest**. Transaksi berdekatan bisa mengubah
      hasil; selalu jalankan `aur_call` tepat sebelum signing, lalu verifikasi
      terhadap blok target bila konfirmasi akhir dibutuhkan.
- [ ] Bila simpul menjalankan versi binary yang lebih lama, `RpcProvider` otomatis
      jatuh ke simulasi lokal saat endpoint mengembalikan `-32601`. Naikkan versi
      simpul agar dry-run mencerminkan state simpul sebenarnya.

```

Harus muncul galat `-32004` (`RATE_LIMIT_EXCEEDED`) setelah kuota habis.
Simpul **tetap sehat** dan melayani `aur_blockHeight` normal.

---

## 8. Ringkasan cepat (httpie)

```bash
http POST $RPC jsonrpc=2.0 id=1 method=aur_estimateGas         params:="[\"$RAW_TX_HEX\"]"
http POST $RPC jsonrpc=2.0 id=2 method=aur_call                params:="[\"$RAW_TX_HEX\"]"
http POST $RPC jsonrpc=2.0 id=3 method=aur_getCode             params:="[\"$CONTRACT_BECH32M\"]"
http POST $RPC jsonrpc=2.0 id=4 method=aur_getContractMetadata params:="[\"$CODE_HASH_HEX\"]"
http POST $RPC jsonrpc=2.0 id=5 method=aur_sendRawTransaction  params:="[\"$SIGNED_TX_HEX\",\"$SENDER_PUBKEY\"]"
```
