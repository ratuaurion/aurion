#!/bin/bash
echo "=== Genesis Block VPS ==="
curl -s -X POST http://127.0.0.1:18545 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"aur_getBlockByHeight","params":["0"]}' | \
  python3 -c "import json,sys; d=json.load(sys.stdin); print(json.dumps(d.get('result','ERROR'), indent=2))"
