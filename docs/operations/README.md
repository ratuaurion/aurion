# AURION — Operational Runbooks & Operations Manuals
> **Klasifikasi:** Panduan Operasional Jaringan, Validator, Operator Sentry, & Tim Keamanan  
> **Status:** Produksi Resmi (Production-Ready)  
> **Acuan Tertinggi:** [Konstitusi Protokol Aurion (`CONSTITUTION.md`)](file:///c:/Projects/aurion/CONSTITUTION.md) | [Dokumen 12: Aturan Operasional](file:///c:/Projects/aurion/docs/Application-Rules-Layer/application/12-OPERATIONAL-RULES.md)

---

## Direktori Dokumen Operasional

Koleksi panduan operasional, deployment, pengujian jaringan bertahap, dan spesifikasi kapasitas untuk seluruh fase daur hidup rantai blok berdaulat Aurion.

| No | Dokumen Panduan | Era Terkait | Deskripsi & Fokus Operasional |
| :---: | :--- | :---: | :--- |
| **01** | [`DEVNET_DEPLOYMENT_GUIDE.md`](file:///c:/Projects/aurion/docs/operations/DEVNET_DEPLOYMENT_GUIDE.md) | **Era V (NET-010)** | Panduan continuous deployment kluster 6-simpul devnet lokal dan kontainerisasi Docker disk D. |
| **02** | [`MULTI_REGION_TESTNET_GUIDE.md`](file:///c:/Projects/aurion/docs/operations/MULTI_REGION_TESTNET_GUIDE.md) | **Era V (NET-011)** | Panduan testnet privat 4 region geografis lintas-benua, rotasi validator per epoch, dan sinkronisasi snapshot `.auss`. |
| **03** | [`PUBLIC_TESTNET_GUIDE.md`](file:///c:/Projects/aurion/docs/operations/PUBLIC_TESTNET_GUIDE.md) | **Era V (NET-012)** | Panduan partisipasi publik, Community Sandbox web dashboard, penggunaan Faucet rate-limited, dan Explorer REST API. |
| **04** | [`RELEASE_CANDIDATE_GUIDE.md`](file:///c:/Projects/aurion/docs/operations/RELEASE_CANDIDATE_GUIDE.md) | **Era VI (PRD-014)** | Prosedur verifikasi integritas binary release candidate `v1.0.0-rc1`, validasi SBOM, dan atestasi checksum SHA-256. |
| **05** | [`GENESIS_CEREMONY_GUIDE.md`](file:///c:/Projects/aurion/docs/operations/GENESIS_CEREMONY_GUIDE.md) | **Era VI (PRD-015)** | Tata cara upacara genesis multi-pihak, pengumpulan tanda tangan Ed25519 validator $\mathcal{V}_0$, dan verifikasi kuorum $>2/3$. |
| **06** | [`MAINNET_LAUNCH_GUIDE.md`](file:///c:/Projects/aurion/docs/operations/MAINNET_LAUNCH_GUIDE.md) | **Era VI (PRD-016)** | Runbook inisialisasi simpul Mainnet produksi, bootstrap 4 validator kanonikal, dan startup daemon `/bin/aurion`. |
| **07** | [`POST_MAINNET_OPERATIONS_GUIDE.md`](file:///c:/Projects/aurion/docs/operations/POST_MAINNET_OPERATIONS_GUIDE.md) | **Era VI (PRD-017)** | SOP insiden darurat Circuit Breaker, pemulihan bencana fast-sync, rotasi kunci validator, dan pensinyalan fork on-chain. |
| **08** | [`CAPACITY_MODEL.md`](file:///c:/Projects/aurion/docs/operations/CAPACITY_MODEL.md) | **Era IV (VER-008)** | Model pengukuran empiris throughput TPS, latensi BFT <1000ms SLA, jejak memori, dan amplifikasi I/O storage `redb 4.3`. |
| **09** | [`CONFORMANCE_MATRIX.md`](file:///c:/Projects/aurion/docs/operations/CONFORMANCE_MATRIX.md) | **Era IV (VER-002)** | Matriks audit 54 pilar kepatuhan multi-layer L1 s/d L5 terhadap automated test suite protokol. |
| **10** | [`METRICS_SPECIFICATION.md`](file:///c:/Projects/aurion/docs/operations/METRICS_SPECIFICATION.md) | **Era VI (PRD-017)** | Kamus metrik Prometheus OpenMetrics format v0.0.4, deskripsi label, tipe data, dan scraping configuration. |

---

## Panduan Penggunaan Singkat

Seluruh panduan operasional di atas dapat diakses langsung oleh operator simpul atau diuji secara otomatis melalui single sovereign primary executable:

```powershell
# Menjalankan simpul FullNode Mainnet
aurion node start --config MAINNET_CONFIG.toml

# Memeriksa status telemetri operasional
aurion metrics status --output json

# Menjalankan verifikasi matriks kepatuhan
aurion conformance matrix
```
