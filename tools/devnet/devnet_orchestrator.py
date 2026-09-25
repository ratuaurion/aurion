#!/usr/bin/env python3
"""
AURION DEVNET CONTINUOUS DEPLOYMENT ORCHESTRATOR (NET-010)
==========================================================
Automated cluster lifecycle management for Aurion local live staging devnet.

Architecture:
- 4 BFT Validator nodes (Single-slot finality, isolated ports & storage)
- 1 Sentry Node (Edge proxy, anti-DDoS filter)
- 1 Public JSON-RPC Gateway (Serving HTTP / WS endpoints)

Zero external dependencies: Pure Python standard library (socket, subprocess, json, urllib).
"""

import os
import sys
import json
import time
import socket
import signal
import argparse
import subprocess
import urllib.request
import urllib.error
from pathlib import Path

def _find_workspace_root() -> Path:
    for p in Path(__file__).resolve().parents:
        if (p / "Cargo.toml").exists():
            return p
    return Path(__file__).resolve().parent.parent.parent

WORKSPACE_ROOT = _find_workspace_root()
DEVNET_DIR = WORKSPACE_ROOT / "data" / "devnet"
PID_FILE = DEVNET_DIR / "cluster_pids.json"

NODES_TOPOLOGY = [
    {
        "id": "val-1",
        "role": "validator",
        "description": "BFT Validator Proposer",
        "p2p_port": 7447,
        "rpc_port": 8545,
        "metrics_port": 19601,
    },
    {
        "id": "val-2",
        "role": "validator",
        "description": "BFT Validator Peer",
        "p2p_port": 7448,
        "rpc_port": 8546,
        "metrics_port": 19602,
    },
    {
        "id": "val-3",
        "role": "validator",
        "description": "BFT Validator Peer",
        "p2p_port": 7449,
        "rpc_port": 8547,
        "metrics_port": 19603,
    },
    {
        "id": "val-4",
        "role": "validator",
        "description": "BFT Validator Peer",
        "p2p_port": 7450,
        "rpc_port": 8548,
        "metrics_port": 19604,
    },
    {
        "id": "sentry-1",
        "role": "sentry",
        "description": "Anti-DDoS Edge Sentry Node",
        "p2p_port": 7451,
        "rpc_port": 8549,
        "metrics_port": 19605,
    },
    {
        "id": "rpc-gateway",
        "role": "rpc",
        "description": "Public JSON-RPC & WS Gateway",
        "p2p_port": 7452,
        "rpc_port": 8550,
        "metrics_port": 19606,
    },
]

def ensure_dirs():
    DEVNET_DIR.mkdir(parents=True, exist_ok=True)
    for node in NODES_TOPOLOGY:
        node_dir = DEVNET_DIR / node["id"]
        node_dir.mkdir(parents=True, exist_ok=True)

def find_binary():
    # Check debug, release, or system PATH
    release_bin = WORKSPACE_ROOT / "target" / "release" / "aurion.exe"
    if release_bin.exists():
        return str(release_bin)
    
    release_bin_unix = WORKSPACE_ROOT / "target" / "release" / "aurion"
    if release_bin_unix.exists():
        return str(release_bin_unix)
    
    debug_bin = WORKSPACE_ROOT / "target" / "debug" / "aurion.exe"
    if debug_bin.exists():
        return str(debug_bin)

    debug_bin_unix = WORKSPACE_ROOT / "target" / "debug" / "aurion"
    if debug_bin_unix.exists():
        return str(debug_bin_unix)

    return "cargo"

def check_port_open(host, port, timeout=0.5):
    sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    sock.settimeout(timeout)
    result = sock.connect_ex((host, port))
    sock.close()
    return result == 0

def query_node_health(rpc_port):
    url = f"http://127.0.0.1:{rpc_port}/healthz"
    try:
        req = urllib.request.Request(url, method="GET")
        with urllib.request.urlopen(req, timeout=1.0) as resp:
            if resp.status == 200:
                body = resp.read().decode("utf-8")
                try:
                    return json.loads(body)
                except Exception:
                    return {"status": "UP", "raw": body.strip()}
    except Exception as e:
        return {"status": "DOWN", "error": str(e)}
    return {"status": "DOWN"}

def cmd_init(args):
    print("=" * 70)
    print("      AURION DEVNET CONTINUOUS DEPLOYMENT: INITIALIZATION (NET-010)  ")
    print("=" * 70)
    ensure_dirs()
    config_manifest = {
        "network": "aurion-devnet-live",
        "chain_id": 1001,
        "created_at": int(time.time()),
        "nodes": NODES_TOPOLOGY,
    }
    manifest_path = DEVNET_DIR / "devnet_manifest.json"
    with open(manifest_path, "w", encoding="utf-8") as f:
        json.dump(config_manifest, f, indent=2)
    
    print(f"[*] Devnet data directory: {DEVNET_DIR}")
    print(f"[*] Manifest created:      {manifest_path}")
    print(f"[*] Nodes provisioned:     {len(NODES_TOPOLOGY)} total")
    for n in NODES_TOPOLOGY:
        print(f"    - {n['id']:<12} | Role: {n['role']:<10} | P2P: 127.0.0.1:{n['p2p_port']} | RPC: 127.0.0.1:{n['rpc_port']}")
    print("=" * 70)
    print("[SUCCESS] Devnet initialized. Run 'python tools/devnet_orchestrator.py start' to launch.")

def cmd_start(args):
    print("=" * 70)
    print("       AURION DEVNET CONTINUOUS DEPLOYMENT: START CLUSTER           ")
    print("=" * 70)
    ensure_dirs()
    binary = find_binary()
    print(f"[*] Target executable: {binary}")

    pids = {}
    if PID_FILE.exists():
        try:
            with open(PID_FILE, "r") as f:
                pids = json.load(f)
        except Exception:
            pids = {}

    for node in NODES_TOPOLOGY:
        nid = node["id"]
        # Check if already running
        if nid in pids:
            old_pid = pids[nid]
            if check_port_open("127.0.0.1", node["rpc_port"]):
                print(f"[!] Node '{nid}' is already running with PID {old_pid} on port {node['rpc_port']}.")
                continue

        db_path = str(DEVNET_DIR / nid / "storage.redb")
        log_path = DEVNET_DIR / nid / "node.log"

        if binary == "cargo":
            cmd = [
                "cargo", "run", "--quiet", "--",
                "devnet", "start",
                "--node-id", nid,
                "--role", node["role"],
                "--p2p-bind", f"127.0.0.1:{node['p2p_port']}",
                "--rpc-bind", f"127.0.0.1:{node['rpc_port']}",
                "--data-dir", db_path,
            ]
        else:
            cmd = [
                binary,
                "devnet", "start",
                "--node-id", nid,
                "--role", node["role"],
                "--p2p-bind", f"127.0.0.1:{node['p2p_port']}",
                "--rpc-bind", f"127.0.0.1:{node['rpc_port']}",
                "--data-dir", db_path,
            ]

        with open(log_path, "w", encoding="utf-8") as log_f:
            proc = subprocess.Popen(
                cmd,
                cwd=str(WORKSPACE_ROOT),
                stdout=log_f,
                stderr=subprocess.STDOUT,
            )
            pids[nid] = proc.pid
            print(f"[+] Started '{nid}' (PID: {proc.pid}) -> http://127.0.0.1:{node['rpc_port']} [Log: {log_path.name}]")

    with open(PID_FILE, "w", encoding="utf-8") as f:
        json.dump(pids, f, indent=2)

    print("=" * 70)
    print("[*] Waiting for cluster nodes to stabilize...")
    time.sleep(1.0)
    cmd_status(args)

def cmd_status(args):
    print("=" * 70)
    print("       AURION DEVNET CONTINUOUS DEPLOYMENT: CLUSTER STATUS          ")
    print("=" * 70)
    pids = {}
    if PID_FILE.exists():
        try:
            with open(PID_FILE, "r") as f:
                pids = json.load(f)
        except Exception:
            pids = {}

    online_count = 0
    print(f"{'Node ID':<12} | {'Role':<10} | {'PID':<8} | {'RPC Port':<9} | {'Port Status':<12} | {'Health'}")
    print("-" * 70)
    for node in NODES_TOPOLOGY:
        nid = node["id"]
        pid = pids.get(nid, "N/A")
        is_port_open = check_port_open("127.0.0.1", node["rpc_port"])
        port_status = "LISTENING" if is_port_open else "CLOSED"
        
        health_info = query_node_health(node["rpc_port"])
        health_str = health_info.get("status", "DOWN")
        if is_port_open:
            online_count += 1

        print(f"{nid:<12} | {node['role']:<10} | {str(pid):<8} | {node['rpc_port']:<9} | {port_status:<12} | {health_str}")

    print("=" * 70)
    print(f"Summary: {online_count}/{len(NODES_TOPOLOGY)} nodes online and active.")
    if online_count == len(NODES_TOPOLOGY):
        print("[SUCCESS] Devnet cluster 100% HEALTHY and continuous.")
    else:
        print("[NOTE] Some nodes are starting or stopped.")

def cmd_stop(args):
    print("=" * 70)
    print("       AURION DEVNET CONTINUOUS DEPLOYMENT: STOP CLUSTER            ")
    print("=" * 70)
    if not PID_FILE.exists():
        print("[*] No cluster_pids.json found. No cluster running.")
        return

    try:
        with open(PID_FILE, "r") as f:
            pids = json.load(f)
    except Exception:
        pids = {}

    for nid, pid in pids.items():
        try:
            if os.name == 'nt':
                subprocess.run(["taskkill", "/F", "/T", "/PID", str(pid)], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            else:
                os.kill(pid, signal.SIGTERM)
            print(f"[-] Terminated '{nid}' (PID: {pid})")
        except Exception as e:
            print(f"[!] Node '{nid}' (PID: {pid}) was not running: {e}")

    try:
        PID_FILE.unlink()
    except Exception:
        pass
    print("=" * 70)
    print("[SUCCESS] All devnet processes stopped.")

def main():
    parser = argparse.ArgumentParser(description="Aurion Devnet Continuous Deployment Orchestrator")
    subparsers = parser.add_subparsers(dest="command", help="Devnet command")

    subparsers.add_parser("init", help="Initialize devnet directory and topology")
    subparsers.add_parser("start", help="Start all nodes in devnet cluster")
    subparsers.add_parser("status", help="Inspect status of devnet nodes")
    subparsers.add_parser("stop", help="Stop all nodes in devnet cluster")

    args = parser.parse_args()

    if args.command == "init":
        cmd_init(args)
    elif args.command == "start":
        cmd_start(args)
    elif args.command == "status":
        cmd_status(args)
    elif args.command == "stop":
        cmd_stop(args)
    else:
        parser.print_help()

if __name__ == "__main__":
    main()
