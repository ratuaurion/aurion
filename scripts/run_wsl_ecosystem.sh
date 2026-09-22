#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
AURION_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
BOOTNODE_ROOT="$(cd -- "${AURION_ROOT}/../aurion-bootnode" && pwd)"
RUNTIME_ROOT="${AURION_WSL_RUNTIME_DIR:-/tmp/aurion_wsl_test}"
LOG_ROOT="${RUNTIME_ROOT}/logs"
BOOTNODE_BIN="${BOOTNODE_ROOT}/target/release/aurion-bootnode"
AURION_BIN="${AURION_ROOT}/target/release/aurion"
TIMEOUT_SECONDS="${AURION_WSL_TIMEOUT_SECONDS:-90}"
KEEP_ALIVE=0
if [[ "${1:-}" == "--keep-alive" || "${1:-}" == "-k" ]]; then
    KEEP_ALIVE=1
fi

BOOTNODE_PID=""
VALIDATOR_PIDS=()

log() {
    printf '[%s] %s\n' "$(date '+%H:%M:%S')" "$*"
}

fail() {
    log "FAIL: $*"
    exit 1
}

cleanup() {
    local pid
    trap - EXIT INT TERM
    log "Cleaning up WSL ecosystem processes."
    for pid in "${VALIDATOR_PIDS[@]:-}"; do
        if [[ -n "${pid}" ]] && kill -0 "${pid}" 2>/dev/null; then
            kill -TERM "${pid}" 2>/dev/null || true
        fi
    done
    if [[ -n "${BOOTNODE_PID}" ]] && kill -0 "${BOOTNODE_PID}" 2>/dev/null; then
        kill -TERM "${BOOTNODE_PID}" 2>/dev/null || true
    fi
    for pid in "${VALIDATOR_PIDS[@]:-}" "${BOOTNODE_PID}"; do
        if [[ -n "${pid}" ]]; then
            wait "${pid}" 2>/dev/null || true
        fi
    done
}

trap cleanup EXIT INT TERM

command -v cargo >/dev/null 2>&1 || fail "cargo is required in WSL."
command -v curl >/dev/null 2>&1 || fail "curl is required in WSL."
[[ -d "${BOOTNODE_ROOT}" ]] || fail "aurion-bootnode repository not found at ${BOOTNODE_ROOT}."

mkdir -p "${LOG_ROOT}"
rm -rf "${RUNTIME_ROOT}/node0" "${RUNTIME_ROOT}/node1" "${RUNTIME_ROOT}/node2" "${RUNTIME_ROOT}/node3"

log "Building native Linux aurion-bootnode binary."
( cd "${BOOTNODE_ROOT}" && cargo build --release )
log "Building native Linux aurion binary."
( cd "${AURION_ROOT}" && cargo build --release --bin aurion )

[[ -x "${BOOTNODE_BIN}" ]] || fail "Missing bootnode binary: ${BOOTNODE_BIN}"
[[ -x "${AURION_BIN}" ]] || fail "Missing aurion binary: ${AURION_BIN}"

log "Starting bootnode on P2P 127.0.0.1:7000 and HTTP 127.0.0.1:8080."
"${BOOTNODE_BIN}" \
    --port 7000 \
    --bind-ip 0.0.0.0 \
    --http-port 8080 \
    --p2p-endpoint tcp/127.0.0.1:7000 \
    --identity-key "${RUNTIME_ROOT}/bootnode.key" \
    >"${LOG_ROOT}/bootnode.out.log" 2>"${LOG_ROOT}/bootnode.err.log" &
BOOTNODE_PID=$!

sleep 2
curl --fail --silent --show-error http://127.0.0.1:8080/healthz >/dev/null \
    || fail "Bootnode health endpoint did not become ready."

p2p_ports=(7001 7002 7003 7004)
rpc_ports=(8545 8546 8547 8548)
for index in 0 1 2 3; do
    log "Starting validator ${index} on P2P ${p2p_ports[${index}]} and RPC ${rpc_ports[${index}]}."
    "${AURION_BIN}" validator start \
        --dev \
        --index "${index}" \
        --p2p-bind "127.0.0.1:${p2p_ports[${index}]}" \
        --rpc-bind "127.0.0.1:${rpc_ports[${index}]}" \
        --data-dir "${RUNTIME_ROOT}/node${index}/validator.redb" \
        --bootnode tcp/127.0.0.1:7000 \
        >"${LOG_ROOT}/validator${index}.out.log" 2>"${LOG_ROOT}/validator${index}.err.log" &
    VALIDATOR_PIDS+=("$!")
done

rpc_height() {
    local port="$1"
    local response
    response="$(curl --fail --silent --show-error \
        -H 'Content-Type: application/json' \
        --data '{"jsonrpc":"2.0","method":"aur_blockHeight","params":[],"id":1}' \
        "http://127.0.0.1:${port}/rpc")" || return 1
    sed -n 's/.*"result":\([0-9][0-9]*\).*/\1/p' <<<"${response}" | head -n 1
}

log "Waiting for four validator RPC listeners and consensus height >= 2."
deadline=$((SECONDS + TIMEOUT_SECONDS))
while (( SECONDS < deadline )); do
    heights=()
    ready=1
    for port in "${rpc_ports[@]}"; do
        height="$(rpc_height "${port}" || true)"
        if [[ -z "${height}" ]]; then
            ready=0
            break
        fi
        heights+=("${height}")
    done
    if (( ready == 1 )); then
        log "Heights: ${heights[0]}/${heights[1]}/${heights[2]}/${heights[3]}"
        if (( heights[0] >= 2 && heights[1] >= 2 && heights[2] >= 2 && heights[3] >= 2 )); then
            log "PASS: bootnode discovery and four-validator BFT reached height >= 2."
            log "Explorer: open http://localhost:3000 after starting aurion-explorer with NEXT_PUBLIC_API_URL=http://localhost:8080."
            if (( KEEP_ALIVE == 1 )); then
                printf '\n%s\n' '================================================================='
                printf '%s\n' 'AURION LOCAL DEVNET IS LIVE (WSL2)'
                printf '%s\n' '================================================================='
                printf '%s\n' 'Bootnode P2P       : tcp://127.0.0.1:7000'
                printf '%s\n' 'Bootnode Telemetry : http://127.0.0.1:8080 (WS: ws://127.0.0.1:8080/ws/telemetry)'
                printf '%s\n' 'Validator 0 RPC    : http://127.0.0.1:8545'
                printf '%s\n' 'Explorer URL        : http://localhost:3000'
                printf '\n%s\n' 'Start Explorer in another terminal:'
                printf '%s\n' '  cd /mnt/c/Projects/aurion-explorer && npm run dev'
                printf '\n%s\n' 'Press ENTER or Ctrl+C to stop all nodes.'
                read -r _ < /dev/tty 2>/dev/null || wait
            fi
            exit 0
        fi
    fi
    sleep 1
done

fail "validator RPC readiness or consensus timeout; inspect ${LOG_ROOT}."
