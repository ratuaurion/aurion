import urllib.request
import json

validators = [
    ('Val1', 'aur167tyqu5xttfwlppvvytv3avpsunqd7vtln3euwtsqvk0eermrq3swdtq8x'),
    ('Val2', 'aur1h9ey8dgxw5454djnc9ugjlayd49gz2rnzpukq0txvqjnax8g49qqm7wfqr'),
    ('Val3', 'aur1rj4pfa5s2lga3l0a4j5m9gcfqy4xzwfkktjxzl8rxdnrkge3s8stw7q5q'),
    ('Val4', 'aur1a09zx3tl6gf9caj98mvjx6v64cmxn9u2v8wqakwh4yuzl9f2qysp0v7vq')
]

for name, addr in validators:
    req = urllib.request.Request(
        'http://127.0.0.1:18547',
        data=json.dumps({'jsonrpc': '2.0', 'id': 1, 'method': 'aur_getBalance', 'params': [addr]}).encode('utf-8'),
        headers={'Content-Type': 'application/json'}
    )
    with urllib.request.urlopen(req) as resp:
        res = json.loads(resp.read().decode())
        val = res.get('result', 0)
        aur = val / 1_000_000_000 if isinstance(val, (int, float)) else val
        print(f"{name} ({addr[:14]}...): {val} Quanta ({aur} AUR)")
