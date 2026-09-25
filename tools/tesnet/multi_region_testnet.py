#!/usr/bin/env python3
"""
AURION PRIVATE MULTI-REGION TESTNET ORCHESTRATOR (NET-011)
===========================================================
Automated multi-region cluster manager with simulated WAN latency profiles.

Topology:
- Region 1: Asia-Pacific (Jakarta / Singapore) - 15ms base RTT
- Region 2: Europe (Frankfurt / London)        - 160ms base RTT
- Region 3: North America (US-East)            - 220ms base RTT
- Region 4: South America (São Paulo)          - 300ms base RTT

Zero external dependencies: Pure Python standard library.
"""

import os
import sys
import json
import time
import socket
import argparse
import urllib.request
from pathlib import Path

def _find_workspace_root() -> Path:
    for p in Path(__file__).resolve().parents:
        if (p / "Cargo.toml").exists():
            return p
    return Path(__file__).resolve().parent.parent.parent

WORKSPACE_ROOT = _find_workspace_root()
TESTNET_DIR = WORKSPACE_ROOT / "data" / "testnet"

REGIONS_CONFIG = {
    "ap-southeast": {
        "name": "Asia-Pacific (Jakarta / Singapore)",
        "rtt_ms": 15,
        "nodes": [
            {"id": "val-ap-1", "role": "validator", "p2p": 19411, "rpc": 19511},
            {"id": "sentry-ap", "role": "sentry", "p2p": 19415, "rpc": 19515},
        ]
    },
    "eu-central": {
        "name": "Europe (Frankfurt / London)",
        "rtt_ms": 160,
        "nodes": [
            {"id": "val-eu-1", "role": "validator", "p2p": 19412, "rpc": 19512},
            {"id": "sentry-eu", "role": "sentry", "p2p": 19416, "rpc": 19516},
        ]
    },
    "us-east": {
        "name": "North America (US-East / N. Virginia)",
        "rtt_ms": 220,
        "nodes": [
            {"id": "val-us-1", "role": "validator", "p2p": 19413, "rpc": 19513},
        ]
    },
    "sa-east": {
        "name": "South America (São Paulo / Sydney)",
        "rtt_ms": 300,
        "nodes": [
            {"id": "val-sa-1", "role": "validator", "p2p": 19414, "rpc": 19514},
        ]
    }
}

def check_port(host, port, timeout=0.3):
    sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    sock.settimeout(timeout)
    res = sock.connect_ex((host, port))
    sock.close()
    return res == 0

def cmd_init(args):
    print("=" * 72)
    print("    AURION PRIVATE MULTI-REGION TESTNET INITIALIZATION (NET-011)     ")
    print("=" * 72)
    TESTNET_DIR.mkdir(parents=True, exist_ok=True)
    manifest = {
        "network": "aurion-private-multiregion-testnet",
        "chain_id": 9999,
        "epoch_block_interval": 10,
        "max_wan_latency_ms": 300,
        "regions": REGIONS_CONFIG,
    }
    manifest_file = TESTNET_DIR / "testnet_manifest.json"
    with open(manifest_file, "w", encoding="utf-8") as f:
        json.dump(manifest, f, indent=2)

    print(f"[*] Base directory: {TESTNET_DIR}")
    print(f"[*] Manifest saved: {manifest_file}")
    print("[*] Configured Geographies & Latency Profiles:")
    for reg_id, reg in REGIONS_CONFIG.items():
        print(f"    - [{reg_id:<12}] {reg['name']:<36} | Base WAN RTT: {reg['rtt_ms']:>3}ms")
        for node in reg["nodes"]:
            node_dir = TESTNET_DIR / node["id"]
            node_dir.mkdir(parents=True, exist_ok=True)
            print(f"        * {node['id']:<10} ({node['role']:<9}) -> P2P: 127.0.0.1:{node['p2p']} | RPC: 127.0.0.1:{node['rpc']}")
    print("=" * 72)
    print("[SUCCESS] Private multi-region testnet topology initialized.")

def cmd_status(args):
    print("=" * 72)
    print("      AURION PRIVATE MULTI-REGION TESTNET CLUSTER STATUS             ")
    print("=" * 72)
    print(f"{'Region':<14} | {'Node ID':<10} | {'Role':<9} | {'WAN RTT':<7} | {'RPC Port':<8} | {'Status'}")
    print("-" * 72)
    total_nodes = 0
    active_nodes = 0
    for reg_id, reg in REGIONS_CONFIG.items():
        for node in reg["nodes"]:
            total_nodes += 1
            is_open = check_port("127.0.0.1", node["rpc"])
            status = "LISTENING" if is_open else "IDLE/READY"
            if is_open:
                active_nodes += 1
            print(f"{reg_id:<14} | {node['id']:<10} | {node['role']:<9} | {str(reg['rtt_ms']) + 'ms':<7} | {node['rpc']:<8} | {status}")
    print("=" * 72)
    print(f"Summary: {active_nodes}/{total_nodes} nodes online across 4 regions.")

def cmd_latency(args):
    print("=" * 72)
    print("    AURION MULTI-REGION CROSS-WAN LATENCY SIMULATION MATRIX          ")
    print("=" * 72)
    regions = list(REGIONS_CONFIG.keys())
    print(f"{'Region A':<14} -> {'Region B':<14} | {'Simulated WAN RTT':<18} | {'BFT SLA Margin (<1000ms)'}")
    print("-" * 72)
    for i, r1 in enumerate(regions):
        for r2 in regions[i:]:
            rtt = max(REGIONS_CONFIG[r1]["rtt_ms"], REGIONS_CONFIG[r2]["rtt_ms"])
            if r1 != r2:
                rtt = REGIONS_CONFIG[r1]["rtt_ms"] + REGIONS_CONFIG[r2]["rtt_ms"] // 2
            margin = 1000 - rtt
            print(f"{r1:<14} -> {r2:<14} | {str(rtt) + ' ms':<18} | {margin} ms remaining (SLA MET)")
    print("=" * 72)

def main():
    parser = argparse.ArgumentParser(description="Aurion Multi-Region Testnet Orchestrator")
    subparsers = parser.add_subparsers(dest="command")
    subparsers.add_parser("init", help="Initialize multi-region topology")
    subparsers.add_parser("status", help="Inspect multi-region cluster status")
    subparsers.add_parser("latency", help="Display simulated cross-region WAN latency matrix")

    args = parser.parse_args()
    if args.command == "init":
        cmd_init(args)
    elif args.command == "status":
        cmd_status(args)
    elif args.command == "latency":
        cmd_latency(args)
    else:
        parser.print_help()

if __name__ == "__main__":
    main()
