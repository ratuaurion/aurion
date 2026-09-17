#![forbid(unsafe_code)]

//! Layanan REST Explorer & Dashboard Komunitas Sandbox Aurion (NET-012).
//!
//! Mematuhi Invariant:
//! - AUR-ARCH-001: Single Sovereign Binary (/bin/aurion).
//! - AUR-ARCH-011: Absolute Zero Unsafe Code (#![forbid(unsafe_code)]).
//! - AUR-ARCH-012: Absolute Zero Float Arithmetic (Fixed Precision Quantum u128).
//!
//! Modul ini menyediakan antarmuka inspeksi blok, transaksi, akun, serta
//! Dashboard Web Interaktif Mandiri (/sandbox) dengan Faucet Testnet terintegrasi.

use std::sync::atomic::Ordering;

use crate::crypto::encode_address_bech32m;
use crate::gateway::rpc::methods::RpcContext;

/// Menghasilkan representasi JSON statistik jaringan untuk endpoint `/explorer/stats`.
pub fn render_explorer_stats(ctx: &RpcContext) -> String {
    let height = ctx.current_height.load(Ordering::SeqCst);
    let finalized = ctx.finalized_height.load(Ordering::SeqCst);
    let mempool_size = ctx.mempool.lock().unwrap().entries.len();
    let accounts_count = ctx.accounts.lock().unwrap().len();
    let headers_count = ctx.headers.lock().unwrap().len();

    let faucet_info = {
        let faucet_guard = ctx.faucet.lock().unwrap();
        if let Some(faucet) = faucet_guard.as_ref() {
            let addr_str = encode_address_bech32m(&faucet.address(), "aur").unwrap_or_default();
            format!(
                r#""faucet":{{"enabled":true,"address":"{}","dispense_amount_aur":"10.00000000","cooldown_secs":{}}}"#,
                addr_str, faucet.config.cooldown_secs
            )
        } else {
            r#""faucet":{{"enabled":false}}"#.to_string()
        }
    };

    format!(
        r#"{{"chain_id":{},"current_height":{},"finalized_height":{},"mempool_size":{},"accounts_count":{},"headers_count":{},{}}}"#,
        ctx.chain_id, height, finalized, mempool_size, accounts_count, headers_count, faucet_info
    )
}

/// Menghasilkan representasi JSON detail blok pada tinggi tertentu untuk `/explorer/block/:height`.
pub fn render_block_by_height(ctx: &RpcContext, height: u64) -> Option<String> {
    let headers = ctx.headers.lock().unwrap();
    let header = headers.get(&height)?;
    let block_hash = hex::encode(header.compute_block_hash().as_bytes());
    let prev_hash = hex::encode(header.prev_block_hash.as_bytes());
    let state_root = hex::encode(header.state_root.as_bytes());
    let tx_root = hex::encode(header.tx_merkle_root.as_bytes());

    let certs = ctx.certificates.lock().unwrap();
    let signatures_count = certs.get(&height).map(|c| c.precommits.len()).unwrap_or(0);

    Some(format!(
        r#"{{"height":{},"round":{},"block_hash":"0x{}","prev_block_hash":"0x{}","state_root":"0x{}","tx_merkle_root":"0x{}","timestamp":{},"signatures_count":{}}}"#,
        header.height, header.round, block_hash, prev_hash, state_root, tx_root, header.timestamp, signatures_count
    ))
}

/// Menghasilkan representasi JSON detail transaksi dari mempool atau riwayat untuk `/explorer/tx/:hash`.
pub fn render_tx_by_hash(ctx: &RpcContext, hash_hex: &str) -> Option<String> {
    let clean_hash = hash_hex.strip_prefix("0x").unwrap_or(hash_hex);
    let mempool = ctx.mempool.lock().unwrap();

    for (tx_hash, entry) in &mempool.entries {
        if hex::encode(tx_hash.as_bytes()) == clean_hash {
            let tx = &entry.tx;
            let sender_bech = encode_address_bech32m(&tx.sender, "aur").unwrap_or_default();
            let recipient_bech = encode_address_bech32m(&tx.recipient, "aur").unwrap_or_default();
            return Some(format!(
                r#"{{"hash":"0x{}","status":"PENDING_IN_MEMPOOL","chain_id":{},"sender":"{}","recipient":"{}","amount_quanta":{},"fee_quanta":{},"nonce":{},"valid_until":{}}}"#,
                clean_hash, tx.chain_id, sender_bech, recipient_bech, tx.amount.as_u128(), tx.fee.as_u128(), tx.nonce, tx.valid_until
            ));
        }
    }
    None
}

/// Menyajikan dokumen HTML5 mandiri untuk Community Sandbox Dashboard (/sandbox).
pub fn render_sandbox_html(chain_id: u64) -> String {
    format!(r##"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Aurion Testnet — Community Sandbox & Explorer</title>
  <style>
    :root {{
      --bg: #07090e;
      --card-bg: rgba(18, 24, 38, 0.75);
      --card-border: rgba(0, 229, 255, 0.15);
      --accent: #00e5ff;
      --accent-glow: rgba(0, 229, 255, 0.35);
      --text: #f0f6fc;
      --text-muted: #8b949e;
      --success: #00f090;
      --error: #ff3366;
    }}
    * {{ box-sizing: border-box; margin: 0; padding: 0; font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, monospace; }}
    body {{ background: var(--bg); color: var(--text); padding: 24px; min-height: 100vh; }}
    header {{ display: flex; justify-content: space-between; align-items: center; border-bottom: 1px solid var(--card-border); padding-bottom: 16px; margin-bottom: 24px; }}
    .brand {{ font-size: 24px; font-weight: 800; color: var(--accent); letter-spacing: 1px; text-shadow: 0 0 12px var(--accent-glow); }}
    .badge {{ background: rgba(0, 229, 255, 0.1); border: 1px solid var(--accent); color: var(--accent); padding: 4px 12px; border-radius: 99px; font-size: 12px; font-weight: 600; }}
    .grid {{ display: grid; grid-template-columns: repeat(auto-fit, minmax(280px, 1fr)); gap: 20px; margin-bottom: 24px; }}
    .card {{ background: var(--card-bg); border: 1px solid var(--card-border); border-radius: 12px; padding: 20px; backdrop-filter: blur(10px); box-shadow: 0 8px 32px rgba(0,0,0,0.3); }}
    .card h3 {{ font-size: 14px; text-transform: uppercase; color: var(--text-muted); margin-bottom: 10px; letter-spacing: 0.5px; }}
    .stat-val {{ font-size: 32px; font-weight: 700; color: var(--text); }}
    .input-group {{ margin-top: 14px; display: flex; gap: 8px; }}
    input {{ flex: 1; background: #0d111a; border: 1px solid #30363d; border-radius: 6px; padding: 10px 14px; color: #fff; font-size: 14px; outline: none; }}
    input:focus {{ border-color: var(--accent); box-shadow: 0 0 8px var(--accent-glow); }}
    button {{ background: var(--accent); color: #07090e; font-weight: 700; border: none; border-radius: 6px; padding: 10px 18px; cursor: pointer; transition: 0.2s; }}
    button:hover {{ filter: brightness(1.15); box-shadow: 0 0 12px var(--accent-glow); }}
    .result-box {{ margin-top: 12px; font-size: 13px; padding: 10px; border-radius: 6px; background: rgba(0,0,0,0.4); border-left: 3px solid var(--accent); display: none; word-break: break-all; }}
  </style>
</head>
<body>
  <header>
    <div class="brand">⚡ AURION PUBLIC TESTNET</div>
    <div class="badge">CHAIN ID: {chain_id}</div>
  </header>

  <div class="grid">
    <div class="card">
      <h3>Latest Block Height</h3>
      <div class="stat-val" id="stat-height">--</div>
    </div>
    <div class="card">
      <h3>Finalized Height</h3>
      <div class="stat-val" id="stat-finalized">--</div>
    </div>
    <div class="card">
      <h3>Mempool Pending Txs</h3>
      <div class="stat-val" id="stat-mempool">--</div>
    </div>
  </div>

  <div class="grid">
    <div class="card">
      <h3>Testnet Faucet Dispenser (10 AUR)</h3>
      <p style="font-size: 13px; color: var(--text-muted);">Request free testnet tokens for development and smart contract testing.</p>
      <div class="input-group">
        <input type="text" id="faucet-address" placeholder="Enter Aurion Address (aur1...)" />
        <button onclick="requestFaucet()">Claim 10 AUR</button>
      </div>
      <div id="faucet-res" class="result-box"></div>
    </div>

    <div class="card">
      <h3>Account Balance Lookup</h3>
      <p style="font-size: 13px; color: var(--text-muted);">Query account balance and nonce state on-chain.</p>
      <div class="input-group">
        <input type="text" id="acc-address" placeholder="Enter Aurion Address (aur1...)" />
        <button onclick="checkBalance()">Check State</button>
      </div>
      <div id="acc-res" class="result-box"></div>
    </div>
  </div>

  <script>
    async function updateStats() {{
      try {{
        const res = await fetch('/explorer/stats');
        const data = await res.json();
        document.getElementById('stat-height').innerText = data.current_height;
        document.getElementById('stat-finalized').innerText = data.finalized_height;
        document.getElementById('stat-mempool').innerText = data.mempool_size;
      }} catch (e) {{
        console.error("Stats poll failed", e);
      }}
    }}

    async function requestFaucet() {{
      const addr = document.getElementById('faucet-address').value.trim();
      const resBox = document.getElementById('faucet-res');
      resBox.style.display = 'block';
      if (!addr) {{ resBox.innerText = 'Please enter a valid address'; return; }}
      resBox.innerText = 'Submitting faucet request...';
      try {{
        const rpcPayload = {{
          jsonrpc: "2.0",
          method: "aur_requestFaucet",
          params: [addr],
          id: 1
        }};
        const res = await fetch('/', {{
          method: 'POST',
          headers: {{ 'Content-Type': 'application/json' }},
          body: JSON.stringify(rpcPayload)
        }});
        const data = await res.json();
        if (data.error) {{
          resBox.style.color = 'var(--error)';
          resBox.innerText = 'Faucet Error: ' + data.error.message;
        }} else {{
          resBox.style.color = 'var(--success)';
          resBox.innerText = 'SUCCESS! Dispensed 10 AUR. Tx Hash: ' + data.result;
          updateStats();
        }}
      }} catch (e) {{
        resBox.style.color = 'var(--error)';
        resBox.innerText = 'Network error: ' + e;
      }}
    }}

    async function checkBalance() {{
      const addr = document.getElementById('acc-address').value.trim();
      const resBox = document.getElementById('acc-res');
      resBox.style.display = 'block';
      if (!addr) {{ resBox.innerText = 'Please enter an address'; return; }}
      try {{
        const rpcPayload = {{
          jsonrpc: "2.0",
          method: "aur_getBalance",
          params: [addr],
          id: 2
        }};
        const res = await fetch('/', {{
          method: 'POST',
          headers: {{ 'Content-Type': 'application/json' }},
          body: JSON.stringify(rpcPayload)
        }});
        const data = await res.json();
        if (data.error) {{
          resBox.style.color = 'var(--error)';
          resBox.innerText = 'Error: ' + data.error.message;
        }} else {{
          resBox.style.color = 'var(--accent)';
          resBox.innerText = 'Balance: ' + data.result + ' Quanta';
        }}
      }} catch (e) {{
        resBox.style.color = 'var(--error)';
        resBox.innerText = 'Network error: ' + e;
      }}
    }}

    updateStats();
    setInterval(updateStats, 3000);
  </script>
</body>
</html>"##, chain_id = chain_id)
}
