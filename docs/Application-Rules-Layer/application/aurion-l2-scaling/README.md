# Aurion L2 Scaling Project (`aurion-l2-scaling`)

> **Sub-Project:** Layer-2 (L2) Scaling & Horizon Expansion  
> **Parent Architecture:** [Rule 17: L2 Evolution Architecture Blueprint](../17-L2-EVOLUTION-ARCHITECTURE-BLUEPRINT.md)  
> **Master Framework:** [Application Rules Layer](../../README.md)

Direktori ini memuat seluruh perencanaan eksekusi teknis, spesifikasi antarmuka, dan rincian tugas untuk pengembangan Layer-2 (L2) Aurion.

---

## Indeks Dokumen Proyek L2 Scaling

1. **[AURION-L2-EXECUTION-PHASES.md](AURION-L2-EXECUTION-PHASES.md)**:
   - Peta Eksekusi Master Layer-2 (Fase L2-0 s/d L2-6)
   - Rincian 21 Tugas Teknis Terukur (`L2-TSK-001` s/d `L2-TSK-603`)
   - Syarat Selesai (*Acceptance Criteria*) dan Pemetaan Invariant (`L2-*`)
   - Blueprint Tata Letak Modul `src/l2/`
2. **[17-L2-EVOLUTION-ARCHITECTURE-BLUEPRINT.md](../17-L2-EVOLUTION-ARCHITECTURE-BLUEPRINT.md)**:
   - Blueprint Normatif Arsitektur L2 (Rule 17)
   - Invariant Kunci `L2-ARCH`, `L2-SETTLE`, `L2-DA`, `L2-PROOF`, `L2-MSG`, `L2-LIFE`
   - Kriteria Transisi Gerbang (Gate 2 & Gate 3)
   - 10 Persyaratan Kanonikal (`REQ-L2-01..10`)

---

## Diagram Alur Eksekusi

```text
L2-0: Spesifikasi Kontrak Bridge & Format Calldata (100% SPEC)
  │
  ▼
L2-1: Tipe Data Primitif & State Representation (`src/l2/types.rs`, `state.rs`)
  │
  ▼
L2-2: L2 Execution Engine & Rollup Runtime (`src/l2/vm.rs`)
  │
  ▼
L2-3: L2 Sequencer Engine & Batch Assembler (`src/l2/sequencer.rs`)
  │
  ▼
L2-4: Kontrak L1 Settlement Bridge & DA Ingestion (`src/l2/bridge.rs`)
  │
  ▼
L2-5: Two-Way Relayer & Anti-Censorship Protection (`src/l2/relayer.rs`)
  │
  ▼
L2-6: Single Binary CLI Integration & L2 Conformance Suite (`src/cli/`, `tests/`)
```
