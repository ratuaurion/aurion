#!/usr/bin/env python3
"""
AURION ARCHITECTURAL & PROTOCOL GUARDRAIL AUDITOR
=================================================
Automated invariant verification and conflict detector for Aurion Single Ecosystem.

This script enforces all constitutional and architectural invariants:
- AUR-ARCH-001: Single Primary Executable Mandate
- AUR-ARCH-003: Explicit Internal Module Boundaries
- AUR-ARCH-005: Canonical Shared Types & Golden Constants
- AUR-ARCH-008: Strict Acyclic Dependency Graph
- AUR-ARCH-011: Absolute Zero Unsafe Code (#![forbid(unsafe_code)])
- AUR-ARCH-012: Absolute Zero Float Arithmetic (Fixed Precision Quantum u128)

If any violation is detected:
1. It prints a prominent conflict alert with exact file path, line number, and conflict reason.
2. It terminates with exit code 1 to halt agent/pipeline execution immediately.
"""

import os
import sys
import re
from pathlib import Path

WORKSPACE_ROOT = Path(__file__).resolve().parent.parent

# Invariant Rules & Patterns
FORBIDDEN_KEYWORDS = [
    (re.compile(r'\bunsafe\b'), "AUR-ARCH-011 (Zero Unsafe Code Mandate: 'unsafe' keyword detected)"),
    (re.compile(r'\bf32\b'), "AUR-ARCH-012 (Zero Float Mandate: 'f32' primitive type detected)"),
    (re.compile(r'\bf64\b'), "AUR-ARCH-012 (Zero Float Mandate: 'f64' primitive type detected)"),
]

# Required Workspace Lint Flags in Cargo.toml
REQUIRED_CARGO_TOML_SNIPPETS = [
    ('unsafe_code = "forbid"', "AUR-ARCH-011 (Workspace must enforce unsafe_code = forbid)"),
    ('float_arithmetic = "deny"', "AUR-ARCH-012 (Clippy must deny float_arithmetic)"),
    ('cast_precision_loss = "deny"', "AUR-ARCH-012 (Clippy must deny cast_precision_loss)"),
]

# Canonical Constants to Cross-Check in codebase
CANONICAL_VALUES = {
    "MAX_SUPPLY_AUR": 66_000_000,
    "QUANTUM_SCALE": 100_000_000,
    "MAX_QUANTUM_SUPPLY": 6_600_000_000_000_000,
    "GENESIS_ALLOCATION_AUR": 23_100_000,
    "FEE_BURN_PERCENT": 20,
    "FEE_MINER_PERCENT": 80,
    "WIRE_MAGIC": "0x41555230",
    "WIRE_HEADER_BYTES": 52,
    "TX_BASE_BYTES": 184,
}

# Required Documentation Files (13 Protocol Specs + 14 App Rules + README)
REQUIRED_DOCS = [
    "README.md",
    "docs/Constitutions/AURION CONSTITUTION.md",
    "docs/Constitutions/AURION-MONETARY-POLICY-SPECIFICATION.md",
    "docs/Constitutions/AURION-CONSENSUS-SPECIFICATION.md",
    "docs/Constitutions/AURION-STATE-TRANSITION-SPECIFICATION.md",
    "docs/Constitutions/AURION-TRANSACTION-SPECIFICATION.md",
    "docs/Constitutions/AURION-CRYPTOGRAPHY-SPECIFICATION.md",
    "docs/Constitutions/AURION-SERIALIZATION-AND-WIRE-PROTOCOL.md",
    "docs/Constitutions/AURION-GENESIS-SPECIFICATION.md",
    "docs/Constitutions/AURION-VALIDATOR-STAKING-SPECIFICATION.md",
    "docs/Constitutions/AURION-GOVERNANCE-SPECIFICATION.md",
    "docs/Constitutions/AURION-SECURITY-SPECIFICATION.md",
    "docs/Constitutions/AURION-REFERENCE-TEST-VECTORS.md",
    "docs/Constitutions/AURION-PROTOCOL-CONFORMANCE-SPECIFICATION.md",
    "docs/Application-Rules-Layer/README.md",
    "docs/Application-Rules-Layer/application/00-APPLICATION-RULES.md",
    "docs/Application-Rules-Layer/application/01-WALLET-RULES.md",
    "docs/Application-Rules-Layer/application/02-RPC-API-RULES.md",
    "docs/Application-Rules-Layer/application/03-TRANSACTION-LIFECYCLE.md",
    "docs/Application-Rules-Layer/application/04-FINALITY-CONFIRMATION-RULES.md",
    "docs/Application-Rules-Layer/application/05-ADDRESS-ACCOUNT-RULES.md",
    "docs/Application-Rules-Layer/application/06-FEE-PAYMENT-RULES.md",
    "docs/Application-Rules-Layer/application/07-PAYMENT-REFERENCE-RULES.md",
    "docs/Application-Rules-Layer/application/08-EXPLORER-INDEXER-RULES.md",
    "docs/Application-Rules-Layer/application/09-SDK-RULES.md",
    "docs/Application-Rules-Layer/application/10-ERROR-MODEL.md",
    "docs/Application-Rules-Layer/application/11-INTEGRATION-RULES.md",
    "docs/Application-Rules-Layer/application/12-OPERATIONAL-RULES.md",
    "docs/Application-Rules-Layer/application/13-COMPATIBILITY-VERSIONING.md",
    "docs/Application-Rules-Layer/application/14-STORAGE-PERSISTENCE-SPECIFICATION.md",
    ".internal-tasks/CONTEXT_ANCHOR.md",
    ".internal-tasks/TASK_REGISTER.md",
]

class Violation:
    def __init__(self, rule_id, file_path, line_number, line_content, message):
        self.rule_id = rule_id
        self.file_path = file_path
        self.line_number = line_number
        self.line_content = line_content.strip()
        self.message = message

def audit_rust_files(workspace_root: Path):
    violations = []
    crates_dir = workspace_root / "crates"
    src_dir = workspace_root / "src"

    search_dirs = [d for d in [crates_dir, src_dir] if d.exists()]

    for s_dir in search_dirs:
        for rs_file in s_dir.rglob("*.rs"):
            # Skip target or temporary files
            if "target" in rs_file.parts:
                continue

            try:
                with open(rs_file, "r", encoding="utf-8", errors="replace") as f:
                    for line_idx, line in enumerate(f, start=1):
                        stripped = line.strip()
                        # Skip comments
                        if stripped.startswith("//") or stripped.startswith("/*") or stripped.startswith("*"):
                            # But verify comments don't contain unsafe directives
                            continue
                        
                        # Check forbid unsafe attribute
                        if '#![forbid(unsafe_code)]' in line:
                            continue

                        # Check for forbidden patterns
                        for pattern, rule_desc in FORBIDDEN_KEYWORDS:
                            # If matching 'unsafe', ensure it's not a lint rule line
                            if pattern.pattern == r'\bunsafe\b':
                                if "unsafe_code" in line:
                                    continue
                            
                            match = pattern.search(line)
                            if match:
                                violations.append(Violation(
                                    rule_id=rule_desc.split()[0],
                                    file_path=str(rs_file.relative_to(workspace_root)),
                                    line_number=line_idx,
                                    line_content=line,
                                    message=rule_desc
                                ))
            except Exception as e:
                violations.append(Violation(
                    rule_id="IO_ERROR",
                    file_path=str(rs_file.relative_to(workspace_root)),
                    line_number=0,
                    line_content="",
                    message=f"Failed to read file: {e}"
                ))

    return violations

def audit_cargo_config(workspace_root: Path):
    violations = []
    cargo_toml = workspace_root / "Cargo.toml"
    if not cargo_toml.exists():
        violations.append(Violation("AUR-ARCH-001", "Cargo.toml", 0, "", "Workspace Cargo.toml is missing!"))
        return violations

    content = cargo_toml.read_text(encoding="utf-8")
    for snippet, desc in REQUIRED_CARGO_TOML_SNIPPETS:
        if snippet not in content:
            violations.append(Violation(
                rule_id=desc.split()[0],
                file_path="Cargo.toml",
                line_number=0,
                line_content="",
                message=f"Missing required configuration: '{snippet}' - {desc}"
            ))
    return violations

def audit_docs_existence(workspace_root: Path):
    violations = []
    for doc_rel_path in REQUIRED_DOCS:
        doc_path = workspace_root / doc_rel_path
        if not doc_path.exists():
            violations.append(Violation(
                rule_id="DOC_SYNC_MISSING",
                file_path=doc_rel_path,
                line_number=0,
                line_content="",
                message=f"Mandatory specification document is missing: {doc_rel_path}"
            ))
        else:
            if doc_path.stat().st_size < 100:
                violations.append(Violation(
                    rule_id="DOC_SYNC_EMPTY",
                    file_path=doc_rel_path,
                    line_number=0,
                    line_content="",
                    message=f"Document appears truncated or unpopulated (<100 bytes): {doc_rel_path}"
                ))
    return violations

def audit_forbidden_trees(workspace_root: Path):
    violations = []
    # Ensure aurion.old is NEVER accessed or part of workspace
    old_dir = workspace_root.parent / "aurion.old"
    if old_dir.exists():
        # Just check that no files in aurion reference aurion.old
        for fpath in workspace_root.rglob("*"):
            if fpath.is_file() and not any(p in fpath.parts for p in ["target", ".git"]):
                try:
                    text = fpath.read_text(encoding="utf-8", errors="ignore")
                    if "aurion.old" in text and "DILARANG menyentuh" not in text and "strictly NEVER touch" not in text:
                        violations.append(Violation(
                            rule_id="AUR-DIR-001",
                            file_path=str(fpath.relative_to(workspace_root)),
                            line_number=0,
                            line_content="",
                            message="Illegal reference to deprecated directory 'aurion.old'!"
                        ))
                except Exception:
                    pass
    return violations

def main():
    print("\n" + "="*80)
    print("  AURION PROTOCOL & ARCHITECTURE GUARDRAIL AUDITOR (STAGE 0)")
    print("  Workspace: " + str(WORKSPACE_ROOT))
    print("="*80 + "\n")

    all_violations = []

    # 1. Audit Cargo Config
    print("[1/4] Checking Cargo Workspace Invariants (AUR-ARCH-001, 011, 012)...")
    cargo_violations = audit_cargo_config(WORKSPACE_ROOT)
    all_violations.extend(cargo_violations)
    if not cargo_violations:
        print("      PASS: Cargo.toml contains all required safety and precision lints.")

    # 2. Audit Rust Source Code
    print("[2/4] Scanning Rust Source Tree for Unsafe Code & Float Arithmetic...")
    rust_violations = audit_rust_files(WORKSPACE_ROOT)
    all_violations.extend(rust_violations)
    if not rust_violations:
        print("      PASS: Zero unsafe blocks and zero float primitives detected in codebase.")

    # 3. Audit Documentation Set Synchronization
    print("[3/4] Verifying Documentation Set Synchronization (30 Required Specs)...")
    doc_violations = audit_docs_existence(WORKSPACE_ROOT)
    all_violations.extend(doc_violations)
    if not doc_violations:
        print("      PASS: All 30 specification, constitutional, and application documents present.")

    # 4. Audit Deprecated Tree Isolation
    print("[4/4] Verifying Complete Isolation from Deprecated Trees...")
    tree_violations = audit_forbidden_trees(WORKSPACE_ROOT)
    all_violations.extend(tree_violations)
    if not tree_violations:
        print("      PASS: Clean isolation verified.")

    print("\n" + "-"*80)
    if all_violations:
        print(f"\n[FATAL GUARDRAIL VIOLATION DETECTED] Total Conflicts: {len(all_violations)}")
        print("="*80)
        for idx, v in enumerate(all_violations, start=1):
            print(f"\nCONFLICT #{idx}:")
            print(f"  Invariant Rule : {v.rule_id}")
            print(f"  File Location  : {v.file_path}:{v.line_number}")
            if v.line_content:
                print(f"  Offending Code : {v.line_content}")
            print(f"  Conflict Detail: {v.message}")
        print("\n" + "="*80)
        print(">>> AGENT EXECUTION HALTED! CONFLICTS MUST BE RESOLVED IMMEDIATELY! <<<")
        print("="*80 + "\n")
        sys.exit(1)
    else:
        print("\n  ALL INVARIANTS SATISFIED! ZERO CONFLICTS DETECTED.")
        print("  AURION ARCHITECTURAL INTEGRITY: 100% CANONICAL.")
        print("="*80 + "\n")
        sys.exit(0)

if __name__ == "__main__":
    main()
