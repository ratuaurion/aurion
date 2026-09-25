#!/bin/bash
# Script kirim batch transaksi uji ke node Aurion lokal dan VPS
# Menggunakan kunci validator kanonik (--val-sender)
BINARY="/mnt/c/Projects/aurion/target/release/aurion"
RPC_LOCAL="http://127.0.0.1:18545"
RPC_VPS="https://bootnode.ratuaurion.store/rpc"

# Alamat validator dari kunci kanonik
VAL1="aur167tyqu5xttfwlppvvytv3avpsunqd7vtln3euwtsqvk0eermrq3swdtq8x"
VAL2="aur1h9ey8dgxw5454djnc9ugjlayd49gz2rnzpukq0txvqjnax8g49qqm7wfqr"
VAL3="aur1rj4pfa5s2lga3l0a4j5m9gcfqy4xzwfkktjxzl8rxdnrkge3s8stw7q5q"
VAL4="aur1a09zx3tl6gf9caj98mvjx6v64cmxn9u2v8wqakwh4yuzl9f2qysp0v7vq"

cd /tmp

echo "========================================"
echo " AURION BATCH TX SENDER - LOKAL & VPS"
echo "========================================"

# === LOKAL: Val1 -> Val2 (5 tx) ===
echo ""
echo "[LOKAL] Mengirim 5 transaksi Val1 -> Val2 ke $RPC_LOCAL..."
for nonce_offset in 3 4 5 6 7; do
  RESULT=$($BINARY wallet send --val-sender 1 --to "$VAL2" --amount 100000000 --fee 10000 -y --rpc "$RPC_LOCAL" 2>&1)
  TXID=$(echo "$RESULT" | grep "TxID" | awk '{print $3}')
  STATUS=$(echo "$RESULT" | grep -E "ERROR|berhasil")
  echo "  [TX nonce~$nonce_offset] $TXID | $STATUS"
  sleep 0.3
done

# === LOKAL: Val2 -> Val3 (5 tx) ===
echo ""
echo "[LOKAL] Mengirim 5 transaksi Val2 -> Val1 ke $RPC_LOCAL..."
for i in 1 2 3 4 5; do
  RESULT=$($BINARY wallet send --val-sender 2 --to "$VAL1" --amount 50000000 --fee 10000 -y --rpc "$RPC_LOCAL" 2>&1)
  TXID=$(echo "$RESULT" | grep "TxID" | awk '{print $3}')
  STATUS=$(echo "$RESULT" | grep -E "ERROR|berhasil")
  echo "  [TX #$i] $TXID | $STATUS"
  sleep 0.3
done

# === VPS: Val1 -> Val2 (5 tx) ===
echo ""
echo "[VPS] Mengirim 5 transaksi Val1 -> Val2 ke $RPC_VPS..."
for i in 1 2 3 4 5; do
  RESULT=$($BINARY wallet send --val-sender 1 --to "$VAL2" --amount 200000000 --fee 10000 -y --rpc "$RPC_VPS" 2>&1)
  TXID=$(echo "$RESULT" | grep "TxID" | awk '{print $3}')
  STATUS=$(echo "$RESULT" | grep -E "ERROR|berhasil")
  echo "  [TX #$i] $TXID | $STATUS"
  sleep 0.3
done

# === VPS: Val2 -> Val1 (5 tx) ===
echo ""
echo "[VPS] Mengirim 5 transaksi Val2 -> Val1 ke $RPC_VPS..."
for i in 1 2 3 4 5; do
  RESULT=$($BINARY wallet send --val-sender 2 --to "$VAL1" --amount 100000000 --fee 10000 -y --rpc "$RPC_VPS" 2>&1)
  TXID=$(echo "$RESULT" | grep "TxID" | awk '{print $3}')
  STATUS=$(echo "$RESULT" | grep -E "ERROR|berhasil")
  echo "  [TX #$i] $TXID | $STATUS"
  sleep 0.3
done

echo ""
echo "========================================"
echo " SELESAI — Tunggu ~1 blok untuk CONFIRMED"
echo "========================================"
