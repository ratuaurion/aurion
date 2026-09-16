# Aurion L5 Infrastructure Project (`aurion-l5-infrastructure`)

> **Sub-Project:** Layer-5 (L5) Global Distributed Infrastructure & Ecosystem Services  
> **Parent Architecture:** [Rule 20: L5 Global Infrastructure Blueprint](../20-L5-GLOBAL-INFRASTRUCTURE-BLUEPRINT.md)  
> **Master Framework:** [Application Rules Layer](../../README.md)

Direktori ini memuat perencanaan eksekusi teknis, spesifikasi protokol jaringan mesh, dan rincian tugas untuk pengembangan Layer-5 (L5) Aurion.

---

## Indeks Dokumen Proyek L5 Infrastructure

1. **[AURION-L5-EXECUTION-PHASES.md](AURION-L5-EXECUTION-PHASES.md)**:
   - Peta Eksekusi Master Layer-5 (Fase L5-0 s/d L5-6)
   - Rincian 21 Tugas Teknis Terukur (`L5-TSK-001` s/d `L5-TSK-603`)
   - Syarat Selesai (*Acceptance Criteria*) dan Pemetaan Invariant (`AUR-L5-*`)
   - Blueprint Tata Letak Modul `src/l5/`
2. **[20-L5-GLOBAL-INFRASTRUCTURE-BLUEPRINT.md](../20-L5-GLOBAL-INFRASTRUCTURE-BLUEPRINT.md)**:
   - Blueprint Normatif Arsitektur L5 (Rule 20)
   - Mandat Bukan-Konsensus, 8 Domain Layanan Global, Ekonomi Mesin (M2M)
   - 12 Persyaratan Kanonikal (`REQ-L5-01..12`)

---

## Diagram Alur Eksekusi

```text
L5-0: Spesifikasi Protokol Node & Jaringan Mesh Service (100% SPEC)
  │
  ▼
L5-1: Primitif Mesh Network & Service Envelope (`src/l5/types.rs`, `codec.rs`)
  │
  ▼
L5-2: Decentralized Compute & Verifiable Execution Engine (`src/l5/compute.rs`)
  │
  ▼
L5-3: Distributed Storage & Decentralized DA Mesh (`src/l5/storage.rs`, `da.rs`)
  │
  ▼
L5-4: Distributed Indexing & Service Query Network (`src/l5/indexing.rs`)
  │
  ▼
L5-5: M2M Autonomous Economy & Streaming Billing (`src/l5/billing.rs`, `m2m.rs`)
  │
  ▼
L5-6: Single Binary CLI Integration & L5 Conformance Suite (`src/cli/`, `tests/`)
```
