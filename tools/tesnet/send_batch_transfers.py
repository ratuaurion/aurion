#!/usr/bin/env python3
import subprocess
import json
import time
import urllib.request

RPC_URL = "http://127.0.0.1:18547"
PUBLIC_RPC = "https://bootnode.ratuaurion.store/rpc"
PUBLIC_RECENT = "https://bootnode.ratuaurion.store/api/v1/transactions/recent?limit=10"
WORKDIR = "/tmp/aurion_mutual_tx_test"
BINARY = "/mnt/c/Projects/aurion/target/release/aurion"

ALICE_PASS = "AliceSovereign2026!"
BOB_PASS   = "BobSovereign2026!"

ALICE_ADDR = "aur1rza97d5t79k992hg9464uke30vj8aknen5cvtdw2pw427sa9ze0s09kzjy"
BOB_ADDR   = "aur1c9042vydnnf20see9a0936rhn3lzsj4lujv7l4z9gk3khwdpdnpsw946h8"

def rpc_call(method, params=[]):
    req = urllib.request.Request(
        PUBLIC_RPC,
        data=json.dumps({"jsonrpc": "2.0", "id": 1, "method": method, "params": params}).encode("utf-8"),
        headers={"Content-Type": "application/json", "User-Agent": "Mozilla/5.0"}
    )
    with urllib.request.urlopen(req, timeout=5) as resp:
        return json.loads(resp.read().decode())

def get_bal_nonce(addr):
    b = int(rpc_call("aur_getBalance", [addr])["result"])
    n = int(rpc_call("aur_getNonce", [addr])["result"])
    return b, n

def run_wsl(cmd_str):
    res = subprocess.run(["wsl", "bash", "-c", cmd_str], capture_output=True, text=True)
    return res.stdout.strip(), res.stderr.strip(), res.returncode

def send_tx(sender, passw, keystore, recipient, amount_quanta):
    cmd = (
        f"cd {WORKDIR} && echo '{passw}' | "
        f"{BINARY} wallet send --to {recipient} --amount {amount_quanta} --fee 10000 "
        f"--keystore {keystore} --password-stdin --rpc {RPC_URL} --yes"
    )
    out, err, code = run_wsl(cmd)
    tx_hash = ""
    for line in out.splitlines():
        if "TxID" in line:
            tx_hash = line.split()[-1]
            break
    return tx_hash, out, err

def main():
    print("=" * 80)
    print("       PENGIRIMAN BATCH TRANSAKSI MUTUAL BARU (ALICE <-> BOB)")
    print("=" * 80)

    b_a, n_a = get_bal_nonce(ALICE_ADDR)
    b_b, n_b = get_bal_nonce(BOB_ADDR)
    print(f"[Kondisi Awal]")
    print(f"  Alice: {b_a / 1e8:.8f} AUR | Nonce: {n_a}")
    print(f"  Bob  : {b_b / 1e8:.8f} AUR | Nonce: {n_b}\n")

    # Transaksi 1: Bob -> Alice (0.25 AUR)
    print(">>> [Transaksi 1/3] Bob -> Alice (0.25000000 AUR)...")
    tx1, out1, _ = send_tx("bob", BOB_PASS, "bob.keystore.json", ALICE_ADDR, 25_000_000)
    print(f"    TxID 1: {tx1}")
    time.sleep(1.5)

    # Transaksi 2: Alice -> Bob (0.40 AUR)
    print("\n>>> [Transaksi 2/3] Alice -> Bob (0.40000000 AUR)...")
    tx2, out2, _ = send_tx("alice", ALICE_PASS, "alice.keystore.json", BOB_ADDR, 40_000_000)
    print(f"    TxID 2: {tx2}")
    time.sleep(1.5)

    # Transaksi 3: Bob -> Alice (0.10 AUR)
    print("\n>>> [Transaksi 3/3] Bob -> Alice (0.10000000 AUR)...")
    tx3, out3, _ = send_tx("bob", BOB_PASS, "bob.keystore.json", ALICE_ADDR, 10_000_000)
    print(f"    TxID 3: {tx3}")

    print("\n>>> Menunggu konfirmasi finalitas blok BFT...")
    time.sleep(2)

    # Cek Saldo Akhir
    b_a_end, n_a_end = get_bal_nonce(ALICE_ADDR)
    b_b_end, n_b_end = get_bal_nonce(BOB_ADDR)
    print("\n" + "=" * 80)
    print("                     VERIFIKASI SALDO TERBARU")
    print("=" * 80)
    print(f"  Alice: {b_a_end / 1e8:.8f} AUR | Nonce: {n_a_end}")
    print(f"  Bob  : {b_b_end / 1e8:.8f} AUR | Nonce: {n_b_end}")

    # Query Endpoint Publik Explorer
    req = urllib.request.Request(PUBLIC_RECENT, headers={"User-Agent": "Mozilla/5.0"})
    with urllib.request.urlopen(req) as resp:
        recent = json.loads(resp.read().decode())
        print("\n" + "=" * 80)
        print("    DAFTAR TRANSAKSI TERBARU DI https://bootnode.ratuaurion.store")
        print("=" * 80)
        for idx, t in enumerate(recent.get("transactions", []), 1):
            h = t.get("block_height")
            tx_id = t.get("tx_hash")
            amt = int(t.get("amount", 0)) / 1e8
            sender = t.get("sender")
            rcpt = t.get("recipient")
            s_name = "Alice" if sender == ALICE_ADDR else ("Bob" if sender == BOB_ADDR else sender[:10])
            r_name = "Alice" if rcpt == ALICE_ADDR else ("Bob" if rcpt == BOB_ADDR else rcpt[:10])
            print(f"  {idx}. [Blok #{h}] {tx_id[:16]}... | {amt:.8f} AUR | {s_name} -> {r_name}")

if __name__ == "__main__":
    main()
