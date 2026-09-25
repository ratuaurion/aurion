#!/bin/bash
curl -s http://127.0.0.1:8545/api/v1/network/stats | python3 -c "
import json, sys
d = json.load(sys.stdin)
print('VPS peers      :', d.get('active_peers', 0))
print('TX relayed     :', d.get('total_tx_relayed', 0))
print('Block proposals:', d.get('total_proposals_relayed', 0))
print('Uptime secs    :', d.get('uptime_secs', 0))
"

echo ""
echo "=== Block height validator VPS ==="
for port in 18545 18546 18547 18548; do
  H=$(curl -s -m 2 -X POST http://127.0.0.1:$port/rpc \
    -H "Content-Type: application/json" \
    -d '{"jsonrpc":"2.0","id":1,"method":"aur_blockHeight","params":[]}' | \
    python3 -c "import json,sys; print(json.load(sys.stdin).get('result','?'))" 2>/dev/null || echo "offline")
  echo "  Port $port: height=$H"
done

echo ""
echo "=== Bootnode peer count ==="
curl -s http://127.0.0.1:8080/api/v1/network/stats 2>/dev/null | python3 -c "
import json, sys
try:
    d = json.load(sys.stdin)
    print('Bootnode peers:', d.get('active_peers', 0))
except:
    print('bootnode API not available on port 8080')
" 2>/dev/null || true
