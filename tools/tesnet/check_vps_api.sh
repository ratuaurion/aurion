#!/bin/bash
curl -s 'http://127.0.0.1:8545/api/v1/transactions/recent?limit=3' | python3 -c "
import json, sys
d = json.load(sys.stdin)
txs = d.get('transactions', [])
print('count:', len(txs))
for t in txs[:3]:
    print()
    print('  hash    :', t.get('tx_hash','?')[:26], '...')
    print('  sender  :', t.get('sender', 'N/A'))
    print('  recipient:', t.get('recipient', 'N/A'))
    print('  amount  :', t.get('amount', 'N/A'), 'Quanta')
    print('  blok    :', t.get('block_height', 'N/A'))
"
