#!/usr/bin/env python3
import subprocess
import json
import time
import urllib.request

RPC_URL = "http://127.0.0.1:18547"
WORKDIR = "/tmp/aurion_mutual_tx_test"
BINARY = "/mnt/c/Projects/aurion/target/release/aurion"
ALICE_PASS = "AliceSovereign2026!"
BOB_PASS = "BobSovereign2026!"

def rpc_call(method, params=[]):
    req = urllib.request.Request(
        "https://bootnode.ratuaurion.store/rpc",
        data=json.dumps({"jsonrpc": "2.0", "id": 1, "method": method, "params": params}).encode("utf-8"),
        headers={"Content-Type": "application/json", "User-Agent": "Mozilla/5.0"}
    )
    with urllib.request.urlopen(req, timeout=5) as resp:
        return json.loads(resp.read().decode())

def run_wsl(cmd_str):
    res = subprocess.run(["wsl", "bash", "-c", cmd_str], capture_output=True, text=True)
    return res.stdout.strip(), res.stderr.strip(), res.returncode

def main():
    print("=" * 70)
    print("  SENDING LIVE TEST TRANSACTION ON AURION TESTNET")
    print("=" * 70)

    alice_addr = "aur1rza97d5t79k992hg9464uke30vj8aknen5cvtdw2pw427sa9ze0s09kzjy"
    bob_addr   = "aur1c9042vydnnf20see9a0936rhn3lzsj4lujv7l4z9gk3khwdpdnpsw946h8"

    print(f"Alice Address: {alice_addr}")
    print(f"Bob Address  : {bob_addr}")

    bal_res = rpc_call("aur_getBalance", [alice_addr])
    nonce_res = rpc_call("aur_getNonce", [alice_addr])
    alice_bal = int(bal_res.get("result", 0)) / 100_000_000
    nonce = int(nonce_res.get("result", 0))
    print(f"Alice Saldo : {alice_bal:.8f} AUR | Nonce: {nonce}")

    # Transfer 0.15 AUR dari Alice ke Bob via RPC public
    amount_quanta = 15_000_000 # 0.15 AUR
    print(f"\n>>> Mengirim 0.15000000 AUR dari Alice ke Bob...")
    send_cmd = f"cd {WORKDIR} && echo '{ALICE_PASS}' | {BINARY} wallet send --to {bob_addr} --amount {amount_quanta} --fee 10000 --keystore alice.keystore.json --password-stdin --rpc https://bootnode.ratuaurion.store/rpc --yes"
    out_tx, err_tx, code = run_wsl(send_cmd)
    tx_hash = ""
    for line in out_tx.splitlines():
        if "TxID" in line:
            tx_hash = line.split()[-1]
            break

    print(f"CLI Output : {out_tx}")
    print(f"TxID Hash  : {tx_hash}")

    # Tunggu finalitas blok
    print("\n>>> Menunggu konfirmasi BFT Single-Slot...")
    time.sleep(2)

    # Verifikasi langsung pada Public Bootnode Endpoint
    req = urllib.request.Request("https://bootnode.ratuaurion.store/api/v1/transactions/recent?limit=6", headers={"User-Agent": "Mozilla/5.0"})
    with urllib.request.urlopen(req) as resp:
        recent = json.loads(resp.read().decode())
        print("\n>>> DAFTAR TRANSAKSI TERBARU DARI https://bootnode.ratuaurion.store:")
        for t in recent.get("transactions", []):
            print(f"   [Block #{t.get('block_height')}] Tx: {t.get('tx_hash')[:18]}... Amount: {int(t.get('amount',0))/1e8} AUR | From: {t.get('sender')[:14]}... -> To: {t.get('recipient')[:14]}...")

if __name__ == "__main__":
    main()
