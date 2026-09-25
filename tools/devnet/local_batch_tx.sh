#!/bin/bash
BINARY=/mnt/c/Projects/aurion/target/release/aurion
VAL1=aur167tyqu5xttfwlppvvytv3avpsunqd7vtln3euwtsqvk0eermrq3swdtq8x
VAL2=aur1h9ey8dgxw5454djnc9ugjlayd49gz2rnzpukq0txvqjnax8g49qqm7wfqr
RPC=http://127.0.0.1:18545

echo "=== LOCAL BATCH TX SENDER (Val3, Val4, Val1) ==="

echo ""
echo "[LOKAL] 10 transaksi Val3 -> Val1..."
for i in 1 2 3 4 5 6 7 8 9 10; do
  RESULT=$($BINARY wallet send --val-sender 3 --to $VAL1 --amount 200000000 --fee 10000 -y --rpc $RPC 2>&1)
  TXID=$(echo "$RESULT" | grep "TxID" | awk '{print $3}')
  if echo "$RESULT" | grep -q "berhasil"; then
    echo "  [TX #$i] OK -> $TXID"
  else
    echo "  [TX #$i] $(echo "$RESULT" | grep ERROR | head -1)"
  fi
  sleep 1
done

echo ""
echo "[LOKAL] 10 transaksi Val4 -> Val2..."
for i in 1 2 3 4 5 6 7 8 9 10; do
  RESULT=$($BINARY wallet send --val-sender 4 --to $VAL2 --amount 150000000 --fee 10000 -y --rpc $RPC 2>&1)
  TXID=$(echo "$RESULT" | grep "TxID" | awk '{print $3}')
  if echo "$RESULT" | grep -q "berhasil"; then
    echo "  [TX #$i] OK -> $TXID"
  else
    echo "  [TX #$i] $(echo "$RESULT" | grep ERROR | head -1)"
  fi
  sleep 1
done

echo ""
echo "[LOKAL] 10 transaksi Val1 -> Val2 (amount beda)..."
for i in 1 2 3 4 5 6 7 8 9 10; do
  AMT=$((100000000 + i * 10000000))
  RESULT=$($BINARY wallet send --val-sender 1 --to $VAL2 --amount $AMT --fee 10000 -y --rpc $RPC 2>&1)
  TXID=$(echo "$RESULT" | grep "TxID" | awk '{print $3}')
  if echo "$RESULT" | grep -q "berhasil"; then
    echo "  [TX #$i] OK (${AMT} Quanta) -> $TXID"
  else
    echo "  [TX #$i] $(echo "$RESULT" | grep ERROR | head -1)"
  fi
  sleep 1
done

echo ""
echo "=== SELESAI ==="
