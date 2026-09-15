#!/usr/bin/env python3
"""
AURION SOFTWARE BILL OF MATERIALS (SBOM) GENERATOR
Extracts complete dependency tree, versions, and source hashes from Cargo.lock.
"""

import json
import os
import sys

WORKSPACE_ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))

def parse_cargo_lock(lock_path):
    packages = []
    current_pkg = {}
    with open(lock_path, "r", encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if line == "[[package]]":
                if current_pkg and "name" in current_pkg:
                    packages.append(current_pkg)
                current_pkg = {}
            elif "=" in line and current_pkg is not None:
                k, v = line.split("=", 1)
                k = k.strip()
                v = v.strip().strip('"')
                if k in ("name", "version", "source", "checksum"):
                    current_pkg[k] = v
        if current_pkg and "name" in current_pkg:
            packages.append(current_pkg)
    return packages

def parse_cargo_toml_deps(toml_path):
    direct_deps = set()
    in_deps = False
    with open(toml_path, "r", encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if line.startswith("[dependencies]"):
                in_deps = True
                continue
            elif line.startswith("[") and in_deps:
                in_deps = False
                continue
            if in_deps and "=" in line and not line.startswith("#"):
                dep_name = line.split("=", 1)[0].strip()
                direct_deps.add(dep_name)
    return direct_deps

def main():
    lock_file = os.path.join(WORKSPACE_ROOT, "Cargo.lock")
    toml_file = os.path.join(WORKSPACE_ROOT, "Cargo.toml")

    if not os.path.exists(lock_file):
        print("[-] Cargo.lock not found!")
        sys.exit(1)

    packages = parse_cargo_lock(lock_file)
    direct_deps = parse_cargo_toml_deps(toml_file)

    direct_list = []
    transitive_list = []

    for pkg in packages:
        if pkg["name"] == "aurion":
            continue
        entry = {
            "name": pkg.get("name"),
            "version": pkg.get("version"),
            "checksum": pkg.get("checksum", "workspace/local"),
            "source": pkg.get("source", "local")
        }
        if pkg["name"] in direct_deps:
            direct_list.append(entry)
        else:
            transitive_list.append(entry)

    direct_list.sort(key=lambda x: x["name"])
    transitive_list.sort(key=lambda x: x["name"])

    sbom = {
        "bomFormat": "Aurion-SBOM",
        "specVersion": "1.0",
        "component": {
            "name": "aurion",
            "version": "1.0.0",
            "type": "application",
            "description": "Aurion Sovereign Cryptocurrency & Blockchain Ecosystem (Single Binary)"
        },
        "stats": {
            "total_dependencies": len(direct_list) + len(transitive_list),
            "direct_dependencies": len(direct_list),
            "transitive_dependencies": len(transitive_list)
        },
        "direct_dependencies": direct_list,
        "transitive_dependencies": transitive_list
    }

    out_file = os.path.join(WORKSPACE_ROOT, "SBOM.json")
    with open(out_file, "w", encoding="utf-8") as f:
        json.dump(sbom, f, indent=2)

    print("=" * 80)
    print(f"  AURION SBOM GENERATED SUCCESSFULLY")
    print(f"  Direct Dependencies     : {len(direct_list)}")
    print(f"  Transitive Dependencies : {len(transitive_list)}")
    print(f"  Total Packages          : {len(direct_list) + len(transitive_list)}")
    print(f"  Output File             : {out_file}")
    print("=" * 80)

if __name__ == "__main__":
    main()
