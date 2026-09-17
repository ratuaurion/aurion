#!/usr/bin/env python3
"""
Aurion Public Testnet & Community Sandbox CLI Orchestrator (NET-012).
Pure Python Standard Library — Zero External Dependencies.

Usage:
  python tools/public_testnet.py status [--rpc http://127.0.0.1:8545]
  python tools/public_testnet.py faucet <address> [--rpc http://127.0.0.1:8545]
  python tools/public_testnet.py balance <address> [--rpc http://127.0.0.1:8545]
  python tools/public_testnet.py block [height|latest] [--rpc http://127.0.0.1:8545]
"""

import sys
import json
import argparse
import urllib.request
import urllib.error

DEFAULT_RPC = "http://127.0.0.1:8545"

def rpc_call(url: str, method: str, params: list = None) -> dict:
    if params is None:
        params = []
    payload = json.dumps({
        "jsonrpc": "2.0",
        "method": method,
        "params": params,
        "id": 1
    }).encode("utf-8")

    req = urllib.request.Request(
        url,
        data=payload,
        headers={"Content-Type": "application/json"}
    )
    with urllib.request.urlopen(req, timeout=5) as response:
        return json.loads(response.read().decode("utf-8"))

def http_get(url: str) -> dict:
    req = urllib.request.Request(url, headers={"Accept": "application/json"})
    with urllib.request.urlopen(req, timeout=5) as response:
        return json.loads(response.read().decode("utf-8"))

def cmd_status(args):
    rpc_url = args.rpc
    print("==================================================================")
    print("           AURION PUBLIC TESTNET COMMUNITY STATUS                 ")
    print("==================================================================")
    try:
        stats_url = f"{rpc_url.rstrip('/')}/explorer/stats"
        stats = http_get(stats_url)
        print(f"  RPC Endpoint:        {rpc_url}")
        print(f"  Chain ID:            {stats.get('chain_id')}")
        print(f"  Current Height:      {stats.get('current_height')}")
        print(f"  Finalized Height:    {stats.get('finalized_height')}")
        print(f"  Mempool Pending Txs: {stats.get('mempool_size')}")
        print(f"  Accounts On-Chain:   {stats.get('accounts_count')}")
        faucet = stats.get('faucet', {})
        if faucet.get('enabled'):
            print(f"  Faucet Status:       ACTIVE (Quota: {faucet.get('dispense_amount_aur')} AUR, Cooldown: {faucet.get('cooldown_secs')}s)")
            print(f"  Faucet Address:      {faucet.get('address')}")
        else:
            print("  Faucet Status:       NOT CONFIGURED")
        print("  Web Sandbox URL:     " + f"{rpc_url.rstrip('/')}/sandbox")
    except Exception as e:
        print(f"  Error querying testnet status: {e}")
    print("==================================================================")

def cmd_faucet(args):
    recipient = args.address
    rpc_url = args.rpc
    print(f"[*] Requesting 10 AUR from Testnet Faucet for: {recipient}...")
    try:
        resp = rpc_call(rpc_url, "aur_requestFaucet", [recipient])
        if "error" in resp:
            print(f"[-] Faucet Error: {resp['error'].get('message')}")
        else:
            tx_hash = resp.get("result")
            print(f"[+] SUCCESS! 10 AUR dispensed. Transaction Hash: {tx_hash}")
    except Exception as e:
        print(f"[-] Request failed: {e}")

def cmd_balance(args):
    address = args.address
    rpc_url = args.rpc
    print(f"[*] Querying balance for: {address}...")
    try:
        resp = rpc_call(rpc_url, "aur_getBalance", [address])
        if "error" in resp:
            print(f"[-] Error: {resp['error'].get('message')}")
        else:
            quanta_str = resp.get("result", "0")
            quanta = int(quanta_str)
            whole = quanta // 100000000
            frac = quanta % 100000000
            print(f"[+] Balance: {whole}.{frac:08d} AUR ({quanta:,} Quanta)")
    except Exception as e:
        print(f"[-] Query failed: {e}")

def cmd_block(args):
    target = args.target
    rpc_url = args.rpc
    url = f"{rpc_url.rstrip('/')}/explorer/block/{target}"
    print(f"[*] Querying block: {target} from {url}...")
    try:
        data = http_get(url)
        print(json.dumps(data, indent=2))
    except Exception as e:
        print(f"[-] Query failed: {e}")

def main():
    parser = argparse.ArgumentParser(description="Aurion Public Testnet Orchestrator")
    parser.add_argument("--rpc", default=DEFAULT_RPC, help="Aurion RPC endpoint URL")
    subparsers = parser.add_subparsers(dest="command", required=True)

    # status
    p_status = subparsers.add_parser("status", help="Display public testnet status")
    p_status.set_defaults(func=cmd_status)

    # faucet
    p_faucet = subparsers.add_parser("faucet", help="Request tokens from faucet")
    p_faucet.add_argument("address", help="Recipient Aurion bech32m address (aur1...)")
    p_faucet.set_defaults(func=cmd_faucet)

    # balance
    p_balance = subparsers.add_parser("balance", help="Check account balance")
    p_balance.add_argument("address", help="Aurion bech32m address")
    p_balance.set_defaults(func=cmd_balance)

    # block
    p_block = subparsers.add_parser("block", help="Query block by height or 'latest'")
    p_block.add_argument("target", default="latest", nargs="?", help="Block height or 'latest'")
    p_block.set_defaults(func=cmd_block)

    args = parser.parse_args()
    args.func(args)

if __name__ == "__main__":
    main()
