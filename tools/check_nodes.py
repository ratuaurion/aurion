import subprocess
import sys

script = """
import urllib.request
import json

ports = [8545, 18545, 18546, 18547, 18548]
labels = ["FullNode RPC", "Val 1 (Alpha)", "Val 2 (Beta)", "Val 3 (Gamma)", "Val 4 (Delta)"]

for port, label in zip(ports, labels):
    url = f"http://127.0.0.1:{port}/rpc"
    data = json.dumps({"jsonrpc": "2.0", "id": 1, "method": "aur_blockHeight", "params": []}).encode()
    req = urllib.request.Request(url, data=data, headers={"Content-Type": "application/json"})
    try:
        with urllib.request.urlopen(req, timeout=3) as resp:
            text = resp.read().decode()
            print(f"[{port}] {label}: {text}")
    except Exception as e:
        print(f"[{port}] {label}: FAILED -> {e}")
"""

cmd = ["wsl.exe", "ssh", "-o", "StrictHostKeyChecking=no", "root@116.212.72.89", "python3"]
subprocess.run(cmd, input=script, text=True)
