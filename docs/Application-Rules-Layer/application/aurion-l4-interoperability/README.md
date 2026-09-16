# Aurion L4 Interoperability Project (`aurion-l4-interoperability`)

> **Sub-Project:** Layer-4 (L4) Interoperability & Cross-Domain Ecosystem  
> **Parent Architecture:** [Rule 19: L4 Interoperability Architecture Blueprint](../19-L4-INTEROPERABILITY-ARCHITECTURE-BLUEPRINT.md)  
> **Master Framework:** [Application Rules Layer](../../README.md)

Direktori ini memuat perencanaan eksekusi teknis, spesifikasi protokol lintas-rantai, dan rincian tugas untuk pengembangan Layer-4 (L4) Aurion.

---

## Indeks Dokumen Proyek L4 Interoperability

1. **[AURION-L4-EXECUTION-PHASES.md](AURION-L4-EXECUTION-PHASES.md)**:
   - Peta Eksekusi Master Layer-4 (Fase L4-0 s/d L4-6)
   - Rincian 21 Tugas Teknis Terukur (`L4-TSK-001` s/d `L4-TSK-603`)
   - Syarat Selesai (*Acceptance Criteria*) dan Pemetaan Invariant (`AUR-L4-*`)
   - Blueprint Tata Letak Modul `src/l4/`
2. **[19-L4-INTEROPERABILITY-ARCHITECTURE-BLUEPRINT.md](../19-L4-INTEROPERABILITY-ARCHITECTURE-BLUEPRINT.md)**:
   - Blueprint Normatif Arsitektur L4 (Rule 19)
   - Hub Interoperabilitas Berdaulat, 8 Domain Kapabilitas, Model Keamanan Bridge
   - 12 Persyaratan Kanonikal (`REQ-L4-01..12`)

---

## Diagram Alur Eksekusi

```text
L4-0: Spesifikasi Protokol Interoperabilitas & Adapter Matrix (100% SPEC)
  │
  ▼
L4-1: Primitif Data Cross-Chain & Envelope Codec (`src/l4/types.rs`, `codec.rs`)
  │
  ▼
L4-2: Trust-Minimized Relayer & Light Client Verifiers (`src/l4/verifier.rs`)
  │
  ▼
L4-3: Cross-Chain Asset Bridge & Vault Management (`src/l4/vault.rs`)
  │
  ▼
L4-4: Cross-Domain State & Identity Interoperability (`src/l4/messaging.rs`)
  │
  ▼
L4-5: Multi-Prover Security & Circuit Breaker (`src/l4/security.rs`)
  │
  ▼
L4-6: Single Binary CLI Integration & L4 Conformance Suite (`src/cli/`, `tests/`)
```
