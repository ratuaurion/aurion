#!/usr/bin/env bash
set -e

AURION_BIN="/usr/local/bin/aurion"
RPC_URL="${1:-http://127.0.0.1:18545}"
WORKDIR="/tmp/aurion_wallet_test_$$"
mkdir -p "$WORKDIR"
cd "$WORKDIR"

echo "================================================================================"
echo "          AURION SOVEREIGN WALLET COMPREHENSIVE TEST SUITE                     "
echo "================================================================================"
echo "Working directory: $WORKDIR"
echo "Binary path      : $AURION_BIN"
echo "RPC endpoint     : $RPC_URL"
echo ""

# ------------------------------------------------------------------------------
# TEST 1: CREATE WALLET ALICE
# ------------------------------------------------------------------------------
echo ">>> [TEST 1/8] Membuat Wallet Baru (Alice)..."
ALICE_PASS="AliceSovereignPass2026!"

# Jalankan aurion wallet create dengan password via stdin
CREATE_OUTPUT=$(echo "$ALICE_PASS" | $AURION_BIN wallet create --name alice --password-stdin)
echo "$CREATE_OUTPUT"

# Ekstrak Address dari output
ALICE_ADDR=$(echo "$CREATE_OUTPUT" | grep "Address (Bech32m):" | awk '{print $3}')
# Ekstrak 24 kata Mnemonic
ALICE_MNEMONIC=$(echo "$CREATE_OUTPUT" | tail -n 2 | head -n 1 | sed 's/^[ \t]*//')

echo ""
echo "[TEST 1 RESULT] Alice Address : $ALICE_ADDR"
echo "[TEST 1 RESULT] Alice Mnemonic: $ALICE_MNEMONIC"

if [ -f "alice.keystore.json" ]; then
    echo "[TEST 1 RESULT] Keystore file 'alice.keystore.json' berhasil dibuat."
else
    echo "[TEST 1 ERROR] Keystore file tidak ditemukan!"
    exit 1
fi

# ------------------------------------------------------------------------------
# TEST 2: VERIFIKASI ALAMAT DARI KEYSTORE
# ------------------------------------------------------------------------------
echo ""
echo ">>> [TEST 2/8] Verifikasi Alamat dari Keystore JSON..."
CHECK_ADDR=$($AURION_BIN wallet address --keystore alice.keystore.json)
echo "Alamat terbaca dari file: $CHECK_ADDR"

if [ "$CHECK_ADDR" = "$ALICE_ADDR" ]; then
    echo "[TEST 2 PASS] Alamat cocok 100% dengan Bech32m kanonikal."
else
    echo "[TEST 2 ERROR] Alamat tidak cocok! ($CHECK_ADDR != $ALICE_ADDR)"
    exit 1
fi

# ------------------------------------------------------------------------------
# TEST 3: RECOVERY / IMPORT WALLET DARI 24 KATA MNEMONIC
# ------------------------------------------------------------------------------
echo ""
echo ">>> [TEST 3/8] Uji Pemulihan / Import dari 24 Kata Mnemonic..."
RECOVER_PASS="NewStrongRecoveryPass2026!"

# Jalankan import dengan mem-pipe mnemonic dan password
printf "%s\n%s\n" "$ALICE_MNEMONIC" "$RECOVER_PASS" | $AURION_BIN wallet import --name alice_recovered --mnemonic-stdin --password-stdin
RECOVERED_ADDR=$($AURION_BIN wallet address --keystore alice_recovered.keystore.json)

echo "Alamat hasil pemulihan: $RECOVERED_ADDR"
if [ "$RECOVERED_ADDR" = "$ALICE_ADDR" ]; then
    echo "[TEST 3 PASS] Deterministic derivation sukses! Alamat hasil import identik dengan alamat asli."
else
    echo "[TEST 3 ERROR] Alamat pemulihan berbeda! ($RECOVERED_ADDR != $ALICE_ADDR)"
    exit 1
fi

# ------------------------------------------------------------------------------
# TEST 4: MEMBUAT WALLET BOB (PENERIMA TRANSFER)
# ------------------------------------------------------------------------------
echo ""
echo ">>> [TEST 4/8] Membuat Wallet Kedua (Bob)..."
BOB_PASS="BobSovereignPass2026!"
BOB_CREATE=$(echo "$BOB_PASS" | $AURION_BIN wallet create --name bob --password-stdin)
BOB_ADDR=$(echo "$BOB_CREATE" | grep "Address (Bech32m):" | awk '{print $3}')
echo "[TEST 4 RESULT] Bob Address: $BOB_ADDR"

# ------------------------------------------------------------------------------
# TEST 5: CEK SALDO AWAL & NONCE VIA RPC
# ------------------------------------------------------------------------------
echo ""
echo ">>> [TEST 5/8] Pengecekan Saldo & Nonce Awal via RPC Node..."
echo "Pengecekan Alice:"
$AURION_BIN wallet balance --address "$ALICE_ADDR" --rpc "$RPC_URL" || true
$AURION_BIN wallet nonce --address "$ALICE_ADDR" --rpc "$RPC_URL" || true

echo "Pengecekan Bob:"
$AURION_BIN wallet balance --address "$BOB_ADDR" --rpc "$RPC_URL" || true
$AURION_BIN wallet nonce --address "$BOB_ADDR" --rpc "$RPC_URL" || true

# ------------------------------------------------------------------------------
# TEST 6: TRANSFER DANA DARI GENESIS DEVELOPER KE ALICE (--dev-sender)
# ------------------------------------------------------------------------------
echo ""
echo ">>> [TEST 6/8] Uji Transfer On-Chain dari Genesis Dev ke Alice (100,000,000 Quanta = 0.1 AUR)..."
SEND_DEV_OUTPUT=$($AURION_BIN wallet send --to "$ALICE_ADDR" --amount 100000000 --fee 10000 --rpc "$RPC_URL" --yes --dev-sender)
echo "$SEND_DEV_OUTPUT"

# Tunggu konsensus BFT memproses blok
echo "Menunggu konsensus BFT memproses transaksi ke ledger (5 detik)..."
sleep 5

echo "Status Saldo Alice setelah pendanaan Genesis:"
$AURION_BIN wallet balance --address "$ALICE_ADDR" --rpc "$RPC_URL"
$AURION_BIN wallet nonce --address "$ALICE_ADDR" --rpc "$RPC_URL"

# ------------------------------------------------------------------------------
# TEST 7: TRANSFER DANA DARI ALICE KE BOB MENGGUNAKAN KEYSTORE ALICE
# ------------------------------------------------------------------------------
echo ""
echo ">>> [TEST 7/8] Uji Transfer Peer-to-Peer (Alice -> Bob: 25,000,000 Quanta = 0.025 AUR)..."
SEND_ALICE_OUTPUT=$(echo "$ALICE_PASS" | $AURION_BIN wallet send --to "$BOB_ADDR" --amount 25000000 --fee 10000 --keystore alice.keystore.json --rpc "$RPC_URL" --password-stdin --yes)
echo "$SEND_ALICE_OUTPUT"

echo "Menunggu konsensus BFT memproses transaksi Alice -> Bob (5 detik)..."
sleep 5

echo "Saldo Akhir Alice:"
$AURION_BIN wallet balance --address "$ALICE_ADDR" --rpc "$RPC_URL"
echo "Saldo Akhir Bob:"
$AURION_BIN wallet balance --address "$BOB_ADDR" --rpc "$RPC_URL"

# ------------------------------------------------------------------------------
# TEST 8: OFFLINE SIGNING (AIR-GAPPED TRANSACTION)
# ------------------------------------------------------------------------------
echo ""
echo ">>> [TEST 8/8] Uji Penandatanganan Offline (Air-Gapped Raw Tx)..."
SIGN_OFFLINE_OUTPUT=$(echo "$ALICE_PASS" | $AURION_BIN wallet sign-tx --to "$BOB_ADDR" --amount 5000000 --fee 10000 --nonce 1 --keystore alice.keystore.json --password-stdin --yes)
echo "$SIGN_OFFLINE_OUTPUT"

echo ""
echo "================================================================================"
echo "          SELURUH 8 PENGUJIAN WALLET AURION SELESAI DENGAN SUKSES!             "
echo "================================================================================"

# Cleanup
rm -rf "$WORKDIR"
