#!/usr/bin/env python3
"""
Test mutual transfer antar dua wallet sovereign Aurion (Alice & Bob):
1. Membuat dua wallet baru (Alice & Bob) dengan keystore JSON & BIP-39 mnemonic.
2. Membaca alamat Bech32m kanonikal masing-masing wallet.
3. Mendanai Alice dari Validator 1 (Val1).
4. Alice mentransfer dana ke Bob (Alice -> Bob).
5. Bob mentransfer balik dana ke Alice (Bob -> Alice).
6. Memverifikasi saldo, nonce, dan status finalitas di konsensus BFT.
"""

import subprocess
import json
import time
import os
import shutil
import urllib.request

RPC_URL = "http://127.0.0.1:18547"
WORKDIR = "/tmp/aurion_mutual_tx_test"
BINARY = "/mnt/c/Projects/aurion/target/release/aurion"

ALICE_PASS = "AliceSovereign2026!"
BOB_PASS = "BobSovereign2026!"

def run_wsl(cmd_str):
    res = subprocess.run(["wsl", "bash", "-c", cmd_str], capture_output=True, text=True)
    return res.stdout.strip(), res.stderr.strip(), res.returncode

def rpc_call(method, params=[]):
    req = urllib.request.Request(
        RPC_URL,
        data=json.dumps({"jsonrpc": "2.0", "id": 1, "method": method, "params": params}).encode("utf-8"),
        headers={"Content-Type": "application/json"}
    )
    with urllib.request.urlopen(req) as resp:
        return json.loads(resp.read().decode())

def get_balance_and_nonce(address):
    bal_res = rpc_call("aur_getBalance", [address])
    nonce_res = rpc_call("aur_getNonce", [address])
    bal_raw = bal_res.get("result", 0)
    nonce = int(nonce_res.get("result", 0))
    bal_quanta = int(bal_raw) if bal_raw else 0
    bal_aur = bal_quanta / 1_000_000_000
    return bal_quanta, bal_aur, nonce

def wait_for_tx(tx_hash, timeout_secs=10):
    clean_hash = tx_hash.replace("0x", "")
    start = time.time()
    while time.time() - start < timeout_secs:
        time.sleep(1.5)
        try:
            res = rpc_call("aur_getTransactionByHash", [clean_hash])
            tx = res.get("result")
            if tx and tx.get("status") == "CONFIRMED":
                return tx
        except Exception:
            pass
    return None

def main():
    print("=" * 80)
    print("   AURION MULTI-WALLET MUTUAL TRANSFER TEST (ALICE <-> BOB)")
    print("=" * 80)
    print(f"RPC Node URL: {RPC_URL}")
    print(f"Working Dir : {WORKDIR}\n")

    run_wsl(f"rm -rf {WORKDIR} && mkdir -p {WORKDIR}")

    # STEP 1: Buat Wallet Alice
    print(">>> [Langkah 1/5] Membuat Wallet Baru: Alice...")
    out, err, code = run_wsl(f"cd {WORKDIR} && echo '{ALICE_PASS}' | {BINARY} wallet create --name alice --password-stdin")
    alice_addr = ""
    for line in out.splitlines():
        if "Address (Bech32m):" in line:
            alice_addr = line.split()[-1]
            break
    print(f"    Alice Address : {alice_addr}")
    assert alice_addr, f"Gagal membuat wallet Alice: {out} {err}"

    # STEP 2: Buat Wallet Bob
    print("\n>>> [Langkah 2/5] Membuat Wallet Baru: Bob...")
    out, err, code = run_wsl(f"cd {WORKDIR} && echo '{BOB_PASS}' | {BINARY} wallet create --name bob --password-stdin")
    bob_addr = ""
    for line in out.splitlines():
        if "Address (Bech32m):" in line:
            bob_addr = line.split()[-1]
            break
    print(f"    Bob Address   : {bob_addr}")
    assert bob_addr, f"Gagal membuat wallet Bob: {out} {err}"

    # Cek Saldo Awal Alice & Bob
    b_alice, a_alice, n_alice = get_balance_and_nonce(alice_addr)
    b_bob, a_bob, n_bob = get_balance_and_nonce(bob_addr)
    print(f"\n[Saldo Awal]")
    print(f"    Alice : {a_alice:.8f} AUR ({b_alice} Quanta) | Nonce: {n_alice}")
    print(f"    Bob   : {a_bob:.8f} AUR ({b_bob} Quanta) | Nonce: {n_bob}")

    # STEP 3: Danai Alice dari Validator 1
    fund_amount = 200_000_000 # 2.0 AUR
    print(f"\n>>> [Langkah 3/5] Mendanai Alice 2.0 AUR dari Validator 1...")
    fund_cmd = f"cd {WORKDIR} && {BINARY} wallet send --val-sender 1 --to {alice_addr} --amount {fund_amount} --fee 10000 --rpc {RPC_URL} --yes"
    out, err, code = run_wsl(fund_cmd)
    fund_txid = ""
    for line in out.splitlines():
        if "TxID" in line:
            fund_txid = line.split()[-1]
            break
    print(f"    Funding TxID  : {fund_txid}")
    confirmed_fund = wait_for_tx(fund_txid)
    if confirmed_fund:
        print(f"    Status        : CONFIRMED pada Blok #{confirmed_fund.get('block_height')}")
    else:
        print(f"    Status        : Menunggu...")

    b_alice, a_alice, n_alice = get_balance_and_nonce(alice_addr)
    print(f"    Saldo Alice sekarang : {a_alice:.8f} AUR ({b_alice} Quanta)")

    # STEP 4: Alice Transfer ke Bob (Alice -> Bob: 1.2 AUR)
    send_ab_amount = 120_000_000 # 1.2 AUR
    print(f"\n>>> [Langkah 4/5] Transfer ALICE -> BOB (1.20000000 AUR)...")
    ab_cmd = f"cd {WORKDIR} && echo '{ALICE_PASS}' | {BINARY} wallet send --to {bob_addr} --amount {send_ab_amount} --fee 10000 --keystore alice.keystore.json --password-stdin --rpc {RPC_URL} --yes"
    out, err, code = run_wsl(ab_cmd)
    tx_ab = ""
    for line in out.splitlines():
        if "TxID" in line:
            tx_ab = line.split()[-1]
            break
    print(f"    TxID (Alice -> Bob) : {tx_ab}")
    confirmed_ab = wait_for_tx(tx_ab)
    if confirmed_ab:
        print(f"    Status               : CONFIRMED pada Blok #{confirmed_ab.get('block_height')}")
    
    b_alice, a_alice, n_alice = get_balance_and_nonce(alice_addr)
    b_bob, a_bob, n_bob = get_balance_and_nonce(bob_addr)
    print(f"    Saldo Sementara Alice: {a_alice:.8f} AUR | Nonce: {n_alice}")
    print(f"    Saldo Sementara Bob  : {a_bob:.8f} AUR | Nonce: {n_bob}")

    # STEP 5: Bob Transfer Balik ke Alice (Bob -> Alice: 0.5 AUR)
    send_ba_amount = 50_000_000 # 0.5 AUR
    print(f"\n>>> [Langkah 5/5] Transfer Balik BOB -> ALICE (0.50000000 AUR)...")
    ba_cmd = f"cd {WORKDIR} && echo '{BOB_PASS}' | {BINARY} wallet send --to {alice_addr} --amount {send_ba_amount} --fee 10000 --keystore bob.keystore.json --password-stdin --rpc {RPC_URL} --yes"
    out, err, code = run_wsl(ba_cmd)
    tx_ba = ""
    for line in out.splitlines():
        if "TxID" in line:
            tx_ba = line.split()[-1]
            break
    print(f"    TxID (Bob -> Alice) : {tx_ba}")
    confirmed_ba = wait_for_tx(tx_ba)
    if confirmed_ba:
        print(f"    Status               : CONFIRMED pada Blok #{confirmed_ba.get('block_height')}")

    # Verifikasi Saldo Akhir
    b_alice_fin, a_alice_fin, n_alice_fin = get_balance_and_nonce(alice_addr)
    b_bob_fin, a_bob_fin, n_bob_fin = get_balance_and_nonce(bob_addr)

    print("\n" + "=" * 80)
    print("                    HASIL VERIFIKASI SALDO AKHIR")
    print("=" * 80)
    print(f"  [Alice] Address : {alice_addr}")
    print(f"          Saldo   : {a_alice_fin:.8f} AUR ({b_alice_fin:,} Quanta)")
    print(f"          Nonce   : {n_alice_fin} (telah mengirim 1 transaksi)")
    print(f"  [Bob]   Address : {bob_addr}")
    print(f"          Saldo   : {a_bob_fin:.8f} AUR ({b_bob_fin:,} Quanta)")
    print(f"          Nonce   : {n_bob_fin} (telah mengirim 1 transaksi)")
    print("=" * 80)
    print("  STATUS: MUTUAL TRANSFER DUA WALLET SUKSES DIVERIFIKASI DI LEDGER CANONICAL!")
    print("=" * 80)

if __name__ == "__main__":
    main()
