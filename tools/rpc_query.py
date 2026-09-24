import urllib.request
import urllib.error
import json
import sys
import os

def rpc_call(method, params=[], port=8545):
    url = f"http://127.0.0.1:{port}/rpc"
    payload = {
        "jsonrpc": "2.0",
        "id": 1,
        "method": method,
        "params": params
    }
    req = urllib.request.Request(
        url,
        data=json.dumps(payload).encode("utf-8"),
        headers={"Content-Type": "application/json"}
    )
    with urllib.request.urlopen(req) as resp:
        return json.loads(resp.read().decode("utf-8"))

def bootnode_broadcast(raw_tx_hex, port=8080):
    url = f"http://127.0.0.1:{port}/api/v1/transactions/broadcast"
    payload = {
        "request_id": "cli-test",
        "raw_tx_hex": raw_tx_hex
    }
    req = urllib.request.Request(
        url,
        data=json.dumps(payload).encode("utf-8"),
        headers={"Content-Type": "application/json"}
    )
    try:
        with urllib.request.urlopen(req) as resp:
            return json.loads(resp.read().decode("utf-8"))
    except urllib.error.HTTPError as e:
        return {"http_code": e.code, "error_body": e.read().decode("utf-8")}

if __name__ == "__main__":
    if len(sys.argv) > 1 and sys.argv[1] == "broadcast_bootnode":
        raw_hex = sys.argv[2]
        res = bootnode_broadcast(raw_hex)
        print(json.dumps(res, indent=2))
    else:
        port = int(os.environ.get("RPC_PORT", "8545"))
        method = sys.argv[1] if len(sys.argv) > 1 else "aur_getNetworkStats"
        params = sys.argv[2:] if len(sys.argv) > 2 else []
        try:
            res = rpc_call(method, params, port)
            print(json.dumps(res, indent=2))
        except Exception as e:
            print(f"Error calling {method} on port {port}: {e}")
