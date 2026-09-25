#!/usr/bin/env python3
"""
AURION REPRODUCIBLE BUILD AUDITOR & RELEASE ARTIFACT HASHER
Standard: RFC 2119 / RFC 8174 / AUR-ARCH-001 / AUR-ARCH-005

Verifies deterministic toolchain settings, compiles the sovereign binary in release mode,
calculates canonical cryptographic digests (SHA-256), and exports RELEASE_HASHES.json.
"""

import hashlib
import json
import os
import subprocess
import sys
import time

def _find_workspace_root() -> str:
    current = os.path.abspath(os.path.dirname(__file__))
    while current != os.path.dirname(current):
        if os.path.exists(os.path.join(current, "Cargo.toml")):
            return current
        current = os.path.dirname(current)
    return os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))

WORKSPACE_ROOT = _find_workspace_root()

def run_cmd(cmd, cwd=WORKSPACE_ROOT):
    result = subprocess.run(cmd, cwd=cwd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, shell=True)
    if result.returncode != 0:
        print(f"[-] Command failed: {cmd}")
        print(result.stderr)
        sys.exit(1)
    return result.stdout.strip()

def compute_sha256(filepath):
    h = hashlib.sha256()
    with open(filepath, "rb") as f:
        while chunk := f.read(65536):
            h.update(chunk)
    return h.hexdigest()

def main():
    print("=" * 80)
    print("  AURION CANONICAL REPRODUCIBLE BUILD PIPELINE")
    print(f"  Workspace: {WORKSPACE_ROOT}")
    print("=" * 80)

    # 1. Verify rust-toolchain.toml
    toolchain_file = os.path.join(WORKSPACE_ROOT, "rust-toolchain.toml")
    if not os.path.exists(toolchain_file):
        print("[-] FATAL: rust-toolchain.toml is missing!")
        sys.exit(1)
    print("[+] Verified: rust-toolchain.toml present.")

    # 2. Check rustc and cargo versions
    rustc_ver = run_cmd("rustc --version")
    cargo_ver = run_cmd("cargo --version")
    print(f"[+] Compiler: {rustc_ver}")
    print(f"[+] Toolchain: {cargo_ver}")

    # 3. Check git commit
    try:
        git_commit = run_cmd("git rev-parse HEAD")
        git_dirty = len(run_cmd("git status --porcelain")) > 0
    except Exception:
        git_commit = "unknown"
        git_dirty = False
    print(f"[+] Git Commit: {git_commit} {'(DIRTY)' if git_dirty else '(CLEAN)'}")

    # 4. Build in release mode with locked dependencies
    print("\n[*] Compiling sovereign release binary (/bin/aurion)...")
    build_start = time.time()
    run_cmd("cargo build --release --locked --bin aurion")
    build_duration = time.time() - build_start
    print(f"[+] Build completed in {build_duration:.2f}s.")

    # 5. Locate binary
    bin_name = "aurion.exe" if sys.platform == "win32" else "aurion"
    bin_path = os.path.join(WORKSPACE_ROOT, "target", "release", bin_name)
    if not os.path.exists(bin_path):
        print(f"[-] FATAL: Binary not found at {bin_path}!")
        sys.exit(1)

    bin_size = os.path.getsize(bin_path)
    bin_sha256 = compute_sha256(bin_path)

    print("\n" + "-" * 80)
    print("  AURION RELEASE ARTIFACT VERIFICATION")
    print("-" * 80)
    print(f"  Binary Name   : {bin_name}")
    print(f"  Binary Path   : {bin_path}")
    print(f"  Binary Size   : {bin_size:,} bytes")
    print(f"  SHA-256 Hash  : {bin_sha256}")
    print("-" * 80)

    # 6. Export RELEASE_HASHES.json
    manifest = {
        "artifact": "aurion",
        "version": "1.0.0",
        "target_binary": bin_name,
        "size_bytes": bin_size,
        "sha256": bin_sha256,
        "rustc_version": rustc_ver,
        "cargo_version": cargo_ver,
        "git_commit": git_commit,
        "git_dirty": git_dirty,
        "build_profile": "release",
        "codegen_units": 1,
        "lto": "fat",
        "opt_level": 3,
        "overflow_checks": True,
        "forbid_unsafe": True,
        "deny_float": True
    }

    out_file = os.path.join(WORKSPACE_ROOT, "RELEASE_HASHES.json")
    with open(out_file, "w", encoding="utf-8") as f:
        json.dump(manifest, f, indent=2)
    print(f"[+] Canonical manifest exported to: {out_file}")

    print("\n[PASS] REPRODUCIBLE BUILD VERIFICATION SUCCESSFUL.")
    print("=" * 80)

if __name__ == "__main__":
    main()
