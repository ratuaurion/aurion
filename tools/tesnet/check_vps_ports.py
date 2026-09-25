import subprocess

remote_code = '''
import urllib.request, json

def call_recent(port):
    req = urllib.request.Request(
        "http://127.0.0.1:" + str(port) + "/api/v1/transactions/recent?limit=5",
        headers={"Content-Type": "application/json"}
    )
    try:
        with urllib.request.urlopen(req, timeout=3) as resp:
            data = json.loads(resp.read().decode())
            print(f"Port {port} transactions: count={data.get('count')}")
            for tx in data.get('transactions', []):
                print(f"   H={tx.get('block_height')} {tx.get('tx_hash')[:16]}... amt={tx.get('amount')} sender={tx.get('sender')[:14]}...")
    except Exception as e:
        print(f"Port {port} error: {e}")

call_recent(18545)
call_recent(8545)
'''

cmd = ["wsl", "ssh", "-o", "StrictHostKeyChecking=no", "root@116.212.72.89", "python3"]
res = subprocess.run(cmd, input=remote_code, capture_output=True, text=True)
print(res.stdout)
if res.stderr:
    print("STDERR:", res.stderr)
