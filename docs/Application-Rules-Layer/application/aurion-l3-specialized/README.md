# Aurion L3 Specialized Networks Project (`aurion-l3-specialized`)

> **Sub-Project:** Layer-3 (L3) Ecosystem Expansion & Specialized Domains  
> **Parent Architecture:** [Rule 18: L3 Ecosystem Expansion Blueprint](../18-L3-ECOSYSTEM-EXPANSION-BLUEPRINT.md)  
> **Master Framework:** [Application Rules Layer](../../README.md)

Direktori ini memuat perencanaan eksekusi teknis, spesifikasi domain runtime, dan rincian tugas untuk pengembangan Layer-3 (L3) Aurion.

---

## Indeks Dokumen Proyek L3 Specialized

1. **[AURION-L3-EXECUTION-PHASES.md](AURION-L3-EXECUTION-PHASES.md)**:
   - Peta Eksekusi Master Layer-3 (Fase L3-0 s/d L3-6)
   - Rincian 21 Tugas Teknis Terukur (`L3-TSK-001` s/d `L3-TSK-603`)
   - Syarat Selesai (*Acceptance Criteria*) dan Pemetaan Invariant (`AUR-L3-*`)
   - Blueprint Tata Letak Modul `src/l3/`
2. **[18-L3-ECOSYSTEM-EXPANSION-BLUEPRINT.md](../18-L3-ECOSYSTEM-EXPANSION-BLUEPRINT.md)**:
   - Blueprint Normatif Arsitektur L3 (Rule 18)
   - 5 Model Keamanan L3, Taksonomi Domain, dan Protokol Perpesanan Hierarkis
   - 12 Persyaratan Kanonikal (`REQ-L3-01..12`)

---

## Diagram Alur Eksekusi

```text
L3-0: Spesifikasi Domain & Inter-Layer Hierarchy (100% SPEC)
  │
  ▼
L3-1: Primitif Data L3 & State Tree (`src/l3/types.rs`, `state.rs`)
  │
  ▼
L3-2: L3 Specialized Runtime Engine (`src/l3/runtime.rs`, `src/l3/vm.rs`)
  │
  ▼
L3-3: L3-to-L2 Settlement & Checkpointing (`src/l3/settlement.rs`)
  │
  ▼
L3-4: Hierarchical Messaging & Relayers L1↔L2↔L3 (`src/l3/messaging.rs`)
  │
  ▼
L3-5: Domain Adapters: App-Chains, DeFi & Privacy (`src/l3/domains/`)
  │
  ▼
L3-6: Single Binary CLI Integration & L3 Conformance Suite (`src/cli/`, `tests/`)
```
