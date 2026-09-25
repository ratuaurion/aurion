import urllib.request
import json
import time

now = time.time()
try:
    with urllib.request.urlopen('https://bootnode.ratuaurion.store/api/v1/peers') as resp:
        peers = json.loads(resp.read().decode())
    print(f"Total Peers Returned: {len(peers)}")
    for i, p in enumerate(peers, 1):
        age = int(now) - p.get('last_seen_unix_secs', 0)
        hex_id = "".join(f"{b:02x}" for b in p.get('peer_id', []))
        print(f"[{i}] Role: {p.get('role'):<10} Locator: {p.get('p2p_locator'):<30} LastSeen: {age}s ago  ID: {hex_id[:12]}...")
except Exception as e:
    print(f"Error: {e}")
