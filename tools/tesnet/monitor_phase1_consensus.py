#!/usr/bin/env python3
"""
Monitor Fase 1 BFT Testnet Aurion:
- 2 Validator di VPS (Val 1: Alpha, Val 2: Beta)
- 2 Validator di Docker Laptop (Val 3: Gamma, Val 4: Delta)
- 1 FullNode di VPS & 1 FullNode di Docker (tidak diubah)
"""

import urllib.request
import json
import subprocess
import time

def query_rpc(url, method="aur_blockHeight", params=None):
    if params is None:
        params = []
    data = json.dumps({"jsonrpc": "2.0", "id": 1, "method": method, "params": params}).encode()
    req = urllib.request.Request(url, data=data, headers={"Content-Type": "application/json"})
    try:
        with urllib.request.urlopen(req, timeout=3) as resp:
            res = json.loads(resp.read().decode())
            return res.get("result")
    except Exception as e:
        return f"ERR: {e}"

def get_vps_status():
    vps_py = """
import urllib.request, json

def q(port):
    try:
        data = json.dumps({"jsonrpc":"2.0","id":1,"method":"aur_blockHeight","params":[]}).encode()
        req = urllib.request.Request(f"http://127.0.0.1:{port}/rpc", data=data, headers={"Content-Type":"application/json"})
        with urllib.request.urlopen(req, timeout=2) as r:
            return json.loads(r.read().decode()).get("result")
    except Exception as e:
        return f"ERR: {e}"

print(json.dumps({
    "val1": q(18545),
    "val2": q(18546),
    "fullnode": q(8545)
}))
"""
    cmd = ["wsl.exe", "ssh", "-o", "StrictHostKeyChecking=no", "root@116.212.72.89", "python3"]
    try:
        proc = subprocess.run(cmd, input=vps_py, text=True, capture_output=True, timeout=8)
        return json.loads(proc.stdout.strip())
    except Exception as e:
        return {"val1": f"ERR: {e}", "val2": f"ERR: {e}", "fullnode": f"ERR: {e}"}

def main():
    print("=" * 70)
    print("  AURION BFT TESTNET - MONITOR FASE 1 (MULTI-DEVICE WAN CONSENSUS)")
    print("=" * 70)
    
    # Docker nodes
    v3 = query_rpc("http://127.0.0.1:18547/rpc")
    v4 = query_rpc("http://127.0.0.1:18548/rpc")
    docker_fn = query_rpc("http://127.0.0.1:18549/rpc")
    
    # VPS nodes
    vps = get_vps_status()
    
    print(f" [VPS Cloud - 116.212.72.89]")
    print(f"   * Val 1 (Alpha - :18545) : Height {vps.get('val1')}")
    print(f"   * Val 2 (Beta  - :18546) : Height {vps.get('val2')}")
    print(f"   * FullNode RPC  (:8545)  : Height {vps.get('fullnode')}")
    print()
    print(f" [Docker Laptop - WSL2]")
    print(f"   * Val 3 (Gamma - :18547) : Height {v3}")
    print(f"   * Val 4 (Delta - :18548) : Height {v4}")
    print(f"   * FullNode RPC  (:18549) : Height {docker_fn}")
    print("=" * 70)
    
    # Quorum check
    heights = [vps.get('val1'), vps.get('val2'), v3, v4]
    valid_heights = [h for h in heights if isinstance(h, int)]
    if len(valid_heights) == 4 and len(set(valid_heights)) == 1:
        print(f" STATUS: 100% IN SYNC! Quorum 4/4 tercapai pada blok {valid_heights[0]}.")
        print(f" Bukti Konsensus: Blok bergerak secara sinkron antar VPS dan Laptop via WAN!")
    elif len(valid_heights) >= 3:
        print(f" STATUS: QUORUM TERCAPAI ({len(valid_heights)}/4 validator aktif).")
    else:
        print(" STATUS: Menunggu konvergensi validator...")
    print("=" * 70)

if __name__ == "__main__":
    main()
