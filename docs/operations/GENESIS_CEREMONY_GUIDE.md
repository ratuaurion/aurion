# AURION — Deterministic Genesis Ceremony Operator Guide (PRD-015)
> **Klasifikasi:** Panduan Operasional & Atestasi Kriptografis Multi-Pihak (Era VI: Production & Mainnet Readiness)  
> **Status:** RATIFIED & CANONICALLY SEALED  
> **Target Rilis:** `aurion v1.0.0-rc1`  
> **Akar Kedaulatan:** Invarian Moneter & Konsensus Imutabel (AUR-ARCH-001..012, 07-GENESIS)

---

## 1. Hakikat dan Prinsip Upacara Genesis (Ceremony Principles)

Upacara Genesis Aurion (*Aurion Deterministic Genesis Ceremony*) adalah upacara pembentukan primordial blok perdana ($H=0$) dan state awal ($\sigma_0$) yang dieksekusi secara deterministik murni tanpa perantara pihak ketiga (*trust-minimized*).

Upacara ini mengunci secara kriptografis:
1. **Konstitusi Moneter:**
   - Batas Suplai Tertinggi Permanen: **66.000.000 AUR** ($6.600.000.000.000.000\ \text{Quanta}$).
   - Alokasi Terbit pada Blok 0 (35%): **23.100.000 AUR** ($2.310.000.000.000.000\ \text{Quanta}$).
     - **Creator Vault:** 30% ($19.800.000\ \text{AUR}$).
     - **Developer Vault:** 5% ($3.300.000\ \text{AUR}$).
   - Cadangan Penambangan Komunitas (65%): $42.900.000\ \text{AUR}$ ditambang via BFT block rewards.
   - Zero-Float Arithmetic: Seluruh perhitungan menggunakan tipe data bilangan bulat pasti `Quantum(u128)`.
2. **Konsensus BFT Pemula ($\mathcal{V}_0$):**
   - 4 Genesis Validators masing-masing mengendalikan bobot voting $250.000$ (total $1.000.000$).
   - Ambang batas kuorum finalitas $>\frac{2}{3}$: **$666.667$ Suara**.
3. **Atestasi Multi-Pihak RFC 8032:**
   - Seluruh 6 pihak menandatangani digest pesan upacara dengan tanda tangan Ed25519 berstandar kepatuhan ketat RFC 8032 (*anti-malleability*).

---

## 2. Atestasi Kriptografis Resmi (Official Sealed Vectors)

Berikut adalah ringkasan hasil upacara kanonikal yang disegel secara permanen:

| Parameter Kriptografis | Nilai Hexadesimal / Angka | Keterangan |
| :--- | :--- | :--- |
| **Ceremony Transcript Hash** | `f88d06b37766746bcb9c9985c35ebf6448c441a578a576ee51bd7efbc64e3380` | Blake3 digest seluruh dokumen transkrip |
| **Genesis Block Hash ($H=0$)** | `d82f72ac1be185911bd803987660e624c0ed1c12d4a189b147de9c5b7f5635f9` | Blake3 ID Header Blok Nol ("AURION-BLOCK-ID-V1") |
| **Initial State Root ($\sigma_0$)** | `61e647706990a010ba95f781d506620cf69b2dc57a7b1b53ca96f0f1d07bb850` | Blake3 SMT Root dari 2 Vault Terbit Awal |
| **Network Chain ID** | `1001` | Aurion Mainnet Protocol ID |
| **Genesis Timestamp** | `1773532800` | 15 Maret 2026 00:00:00 UTC |
| **Total Validator Power** | `1.000.000` | $4 \times 250.000$ bobot voting |
| **Attested Validator Power** | `1.000.000` | Kuorum $100\% \ge 666.667$ tercapai |

---

## 3. Peserta dan Tanda Tangan Pengesahan (Signatories)

Pesan kanonikal 32-byte yang ditandatangani dihitung melalui:
$$\text{Msg} = \text{Blake3}\Big(\text{"AURION-GENESIS-CEREMONY-V1"} \mathbin{\Vert} \text{chain\_id} \mathbin{\Vert} \text{timestamp} \mathbin{\Vert} \text{block\_hash} \mathbin{\Vert} \text{state\_root}\Big)$$

### Rincian 6 Penandatangan:

| Entitas / Peran | Kunci Publik Ed25519 (Hex) | Alamat Akun (Hex) | Bobot | Tanda Tangan Digital RFC 8032 (Hex Prefix) |
| :--- | :--- | :--- | ---:| :--- |
| **Creator Sovereign Vault** | `8a88e3dd7409f195...` | `cb095697ccc5acbf...` | $0$ | `0430cbb9d4e92c59...` |
| **Developer Core Vault** | `8139770ea87d175f...` | `708ab607c168ebfe...` | $0$ | `f5451947d0c65376...` |
| **Genesis Val 1 (Alpha)** | `d04ab232742bb4ab...` | `d7964072865ad2ef...` | $250.000$ | `b3adbf3b66970ece...` |
| **Genesis Val 2 (Beta)** | `204040e364c10f2b...` | `b97243b506752b4a...` | $250.000$ | `8b9b950c713e88c9...` |
| **Genesis Val 3 (Gamma)** | `66cd608b928b88e5...` | `0c74846bba5614f8...` | $250.000$ | `2a600841d537542e...` |
| **Genesis Val 4 (Delta)** | `20828bf5c5bdcacb...` | `459dcf2383f92dce...` | $250.000$ | `a093f79bd5398123...` |

---

## 4. Panduan Verifikasi Mandiri bagi Validator & Bursa

Setiap operator simpul dapat memverifikasi integritas upacara Genesis secara mandiri menggunakan single binary `/bin/aurion`:

### 4.1 Pemeriksaan Transkrip Upacara
```bash
# Format Teks Human-Readable
aurion genesis ceremony inspect

# Format JSON Otomasi Mesin
aurion genesis ceremony inspect --output json
```

### 4.2 Verifikasi Kriptografis Penuh (Full Cryptographic Verification)
Perintah ini mengeksekusi pemeriksaan mendalam terhadap:
- Kecocokan hash blok genesis $H=0$.
- Keabsahan seluruh tanda tangan Ed25519 via algoritma RFC 8032 strict non-malleability.
- Pemenuhan kuorum BFT $\ge 666.667$.
- Kepatuhan invarian moneter 35% terbit awal dan 66M hard cap.

```bash
# Verifikasi Berkas Default (GENESIS_CEREMONY.json)
aurion genesis ceremony verify

# Verifikasi Berkas Khusus
aurion genesis ceremony verify --file /path/to/GENESIS_CEREMONY.json --output json
```

Contoh keluaran sukses:
```json
{
  "overall_status": "VERIFIED_CANONICAL",
  "ceremony_hash": "f88d06b37766746bcb9c9985c35ebf6448c441a578a576ee51bd7efbc64e3380",
  "genesis_block_hash": "d82f72ac1be185911bd803987660e624c0ed1c12d4a189b147de9c5b7f5635f9",
  "state_root": "61e647706990a010ba95f781d506620cf69b2dc57a7b1b53ca96f0f1d07bb850",
  "chain_id": 1001,
  "genesis_timestamp": 1773532800,
  "total_attestations": 6,
  "attested_validator_power": 1000000,
  "quorum_threshold": 666667,
  "quorum_status": "PASSED (1000000/1000000 >= 666667)",
  "monetary_audit_status": "PASSED (100% Invariant Compliant: 35% Genesis, Zero-Float)",
  "verified_at": 1773532800
}
```

### 4.3 Pembuatan Ulang Transkrip (Reproducible Ceremony Re-Run)
```bash
aurion genesis ceremony run --export GENESIS_CEREMONY.json --output json
```

---

## 5. Inisialisasi Simpul Produksi dari Sealed Genesis

Untuk memulai simpul validator atau full node baru:
1. Pastikan berkas [`GENESIS_CEREMONY.json`](file:///c:/Projects/aurion/GENESIS_CEREMONY.json) tersedia di direktori kerja simpul.
2. Jalankan inisialisasi simpul:
   ```bash
   aurion node --config node_config.json
   ```
3. Runtime secara otomatis menginisialisasi database `redb` (`data/storage/aurion.redb`), mengkomit Blok 0 secara atomik, dan mengunci State Root $\sigma_0$ (`61e647706990a010ba95f781d506620cf69b2dc57a7b1b53ca96f0f1d07bb850`).
4. Jaringan siap memproses blok pertama $H=1$ setelah BFT validator online dan bertukar proposal.

---

## 6. Surat Ratifikasi Konstitusional
Upacara pembentukan Genesis ini mengikat seluruh validator, bursa, dan pengembang protokol Aurion. Perubahan terhadap blok genesis dianggap sebagai pembuatan rantai cabang yang tidak sah (*invalid hard-fork universe*) dan ditolak secara otomatis oleh seluruh simpul konsensus yang taat aturan.
