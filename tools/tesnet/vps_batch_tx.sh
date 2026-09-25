#!/bin/bash
# Kirim batch transaksi ke VPS via SSH
BINARY="/usr/local/bin/aurion"
RPC="http://127.0.0.1:18545"

VAL1="aur167tyqu5xttfwlppvvytv3avpsunqd7vtln3euwtsqvk0eermrq3swdtq8x"
VAL2="aur1h9ey8dgxw5454djnc9ugjlayd49gz2rnzpukq0txvqjnax8g49qqm7wfqr"

echo "=== VPS BATCH TX SENDER ==="
echo ""

# Cek block height dulu
echo "[VPS] Block height saat ini:"
curl -s -X POST $RPC -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"aur_blockHeight","params":[]}' | python3 -c "import json,sys; d=json.load(sys.stdin); print('  Height:', d.get('result', d.get('error','?')))"

echo ""
echo "[VPS] Mengirim 8 transaksi Val1 -> Val2..."
for i in 1 2 3 4 5 6 7 8; do
  RESULT=$($BINARY wallet send --val-sender 1 --to "$VAL2" --amount 100000000 --fee 10000 -y --rpc "$RPC" 2>&1)
  TXID=$(echo "$RESULT" | grep "TxID" | awk '{print $3}')
  if echo "$RESULT" | grep -q "berhasil"; then
    echo "  [TX #$i] OK -> $TXID"
  else
    ERR=$(echo "$RESULT" | grep "ERROR" | head -1)
    echo "  [TX #$i] $ERR"
  fi
  sleep 1
done

echo ""
echo "[VPS] Mengirim 8 transaksi Val2 -> Val1..."
for i in 1 2 3 4 5 6 7 8; do
  RESULT=$($BINARY wallet send --val-sender 2 --to "$VAL1" --amount 50000000 --fee 10000 -y --rpc "$RPC" 2>&1)
  TXID=$(echo "$RESULT" | grep "TxID" | awk '{print $3}')
  if echo "$RESULT" | grep -q "berhasil"; then
    echo "  [TX #$i] OK -> $TXID"
  else
    ERR=$(echo "$RESULT" | grep "ERROR" | head -1)
    echo "  [TX #$i] $ERR"
  fi
  sleep 1
done

echo ""
echo "[VPS] Mengirim 8 transaksi Val3 -> Val4..."
VAL3_ADDR=$(curl -s -X POST $RPC -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"aur_blockHeight","params":[]}' 2>/dev/null | head -1)
for i in 1 2 3 4 5 6 7 8; do
  RESULT=$($BINARY wallet send --val-sender 3 --to "$VAL2" --amount 75000000 --fee 10000 -y --rpc "$RPC" 2>&1)
  TXID=$(echo "$RESULT" | grep "TxID" | awk '{print $3}')
  if echo "$RESULT" | grep -q "berhasil"; then
    echo "  [TX #$i] OK -> $TXID"
  else
    ERR=$(echo "$RESULT" | grep "ERROR" | head -1)
    echo "  [TX #$i] $ERR"
  fi
  sleep 1
done

echo ""
echo "=== SELESAI ==="
