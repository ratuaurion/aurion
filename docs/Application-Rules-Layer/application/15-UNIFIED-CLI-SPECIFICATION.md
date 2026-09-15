# AURION APPLICATION RULE 15: UNIFIED CLI & APPLICATION CONTROL PLANE SPECIFICATION
**Normative Standard:** RFC 2119 / RFC 8174 / AUR-ARCH-001 / AUR-ARCH-009 / AUR-CLI-001..007  
**Status:** CANONICAL RATIFIED  
**Pillar:** Unified Ecosystem Interface, Operational Control Plane & Machine Automation

---

## 1. Scope & Architectural Philosophy

Dalam ekosistem Aurion, antarmuka baris perintah (Command-Line Interface / CLI) **bukanlah sekadar fitur dompet (wallet)** atau pembungkus parsial. Sesuai prinsip **Single Sovereign Ecosystem / Single Primary Binary (`/bin/aurion`)**, CLI adalah **Control Plane Terpadu (Unified Application Control Plane)** untuk seluruh kapabilitas sistem.

```text
                         AURION
                           │
                    ONE EXECUTABLE (/bin/aurion)
                           │
                    ┌──────┴──────┐
                    │  UNIFIED CLI │
                    └──────┬──────┘
                           │
        ┌──────────────────┼──────────────────┐
        │                  │                  │
      Node              Wallet            Validator
        │                  │                  │
     Consensus          Keys/Tx          Staking
        │                  │                  │
      Network           Payment          Consensus
        │                  │                  │
      Storage             RPC             Governance
        │                  │                  │
        └──────────────────┼──────────────────┘
                           │
                     Aurion Runtime
```

Seluruh perintah operasional dipanggil melalui satu biner:
```bash
aurion <subsystem> <action> [options]
```

DILARANG MEMBUAT BINARY TERPISAH seperti `aurion-wallet`, `aurion-node`, `aurion-cli`, atau `aurion-validator`.

---

## 2. Invariant Unified CLI (AUR-CLI)

* **`AUR-CLI-001` (Unified Interface Mandate):** Aurion WAJIB menyediakan antarmuka baris perintah tunggal dan terpadu melalui biner `/bin/aurion`.
* **`AUR-CLI-002` (Ecosystem Operational Coverage):** Seluruh kemampuan operasional pihak pertama (node, validator, wallet, storage, genesis, RPC, kueri) WAJIB dapat diakses melalui CLI terpadu.
* **`AUR-CLI-003` (Zero Separate Wallet Binary):** Fungsionalitas dompet (pembuatan kunci, derivasi, tanda tangan transaksi, keystore) DILARANG memerlukan biner eksekusi terpisah.
* **`AUR-CLI-004` (Strict Protocol Decoupling):** CLI WAJIB menggunakan pustaka protokol kanonikal dan DILARANG mendefinisikan semantik konsensus atau aturan moneter secara independen.
* **`AUR-CLI-005` (Invariant Enforcement on Mutating Commands):** Seluruh perintah CLI yang memodifikasi state persisten WAJIB melalui aturan validasi dan otorisasi yang sama dengan STF dan konsensus.
* **`AUR-CLI-006` (No State Bypass):** Perintah administratif CLI DILARANG memiliki jalan pintas (*backdoor*) untuk memodifikasi saldo, akun, atau riwayat blok tanpa melalui transaksi yang sah dan STF.
* **`AUR-CLI-007` (Machine-Readable Automation Output):** Seluruh perintah kueri dan inspeksi CLI WAJIB menyediakan format keluaran yang dapat dibaca mesin (`--output json` atau `--format json`) untuk kebutuhan otomasi CI/CD dan monitoring.

---

## 3. Taksonomi & Pohon Perintah Resmi

```text
aurion
├── node          -> Pengelolaan simpul publik & gateway (start, info, status)
├── validator     -> Pengoperasian simpul konsensus BFT (start, status, info)
├── wallet        -> Manajemen kunci, derivasi BIP-39/BIP-44, dan clear-signing (create, import, address, sign-tx)
├── account       -> Kueri informasi saldo, nonce, dan verifikasi akun (balance, info)
├── block         -> Inspeksi data blok kanonikal (get, latest, verify)
├── tx            -> Pembuatan, penandatanganan, dan pengiriman transaksi (build, sign, submit, status)
├── network       -> Informasi topologi P2P wire framing dan peer (peers, status)
├── storage       -> Manajemen dan verifikasi integritas database redb (status, verify)
├── genesis       -> Inspeksi parameter dan hash blok genesis (inspect, hash, validate)
├── rpc           -> Gateway JSON-RPC 2.0 & WebSocket mandiri (start, status)
├── query         -> Kueri cepat terhadap simpul lokal/remote (block, tx, account)
├── conformance   -> Runner pengujian kepatuhan protokol 8-Pilar CTS (run, export)
└── version       -> Informasi versi atomik SemVer 2.0.0, commit git, dan flag kompilasi
```

---

## 4. Klasifikasi Operasi: Read vs Mutating

| Kelas Operasi | Karakteristik | Contoh Perintah | Persyaratan Keamanan |
| :--- | :--- | :--- | :--- |
| **Read Operations** | Idempoten, tanpa efek samping, aman dijalankan tanpa izin khusus. | `query`, `block get`, `account balance`, `storage status`, `version` | Dapat dijalankan langsung tanpa kunci privat / otentikasi. |
| **Mutating Operations** | Mengubah state, menandatangani payload, atau mengonsumsi sumber daya. | `node start`, `wallet create`, `tx submit`, `validator start` | Memerlukan kunci kriptografi, password keystore, atau isolasi hak akses sistem operasi. |

---

## 5. Standar Format Keluaran Mesin (JSON Automation)

Seluruh perintah CLI mendukung flag `--output json` atau `-o json`:

```bash
aurion account balance aur1... --output json
```
Keluaran JSON terstruktur:
```json
{
  "address": "aur1...",
  "balance_aur": "100.00000000",
  "balance_quanta": 10000000000,
  "nonce": 0,
  "status": "success"
}
```

Hal ini memungkinkan skrip shell, pipeline CI/CD, dan subagent berinteraksi secara deterministik dengan Aurion tanpa *parsing* teks rapuh.
