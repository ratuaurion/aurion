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
use crate::gateway::contract_decode::{describe_contract_interaction, TxStatus};
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

/// Menghasilkan representasi JSON detail transaksi dari mempool atau riwayat
/// untuk `/explorer/tx/:hash`.
///
/// Mencakup dua sumber: mempool (`PENDING`) dan riwayat transaksi terkonfirmasi
/// (`FINALIZED`). Field kontrak (`tx_type`, `raw_payload`, `contract_interaction`)
/// ditambahkan agar Explorer dapat menampilkan interaksi kontrak ter-decode.
pub fn render_tx_by_hash(ctx: &RpcContext, hash_hex: &str) -> Option<String> {
    let clean_hash = hash_hex.strip_prefix("0x").unwrap_or(hash_hex);

    // Mempool lebih dulu: transaksi yang belum komit berstatus PENDING.
    {
        let mempool = ctx.mempool.lock().unwrap();
        for (tx_hash, entry) in &mempool.entries {
            if hex::encode(tx_hash.as_bytes()) == clean_hash {
                let tx = &entry.tx;
                let interaction =
                    describe_contract_interaction(ctx, tx, TxStatus::Pending);
                return Some(render_tx_detail_json(
                    clean_hash,
                    "PENDING",
                    tx,
                    None,
                    interaction,
                ));
            }
        }
    }

    // Riwayat transaksi yang sudah tercakup blok berstatus FINALIZED.
    let summaries: Vec<crate::gateway::rpc::methods::CommittedTxSummary> = {
        let recent = ctx.recent_transactions.lock().unwrap();
        recent
            .iter()
            .filter(|s| hex::encode(s.tx_id.as_bytes()) == clean_hash)
            .cloned()
            .collect()
    };
    let summary = summaries.first()?;
    let interaction = describe_contract_interaction(ctx, &summary.tx, TxStatus::Finalized);
    Some(render_tx_detail_json(
        clean_hash,
        "FINALIZED",
        &summary.tx,
        Some(summary.height),
        interaction,
    ))
}

/// Render JSON detail transaksi lengkap (explorer + dekoder kontrak).
///
/// Dibangun lewat `serde_json` (bukan `format!`) agar peng-escaping nilai
/// yang berasal dari jaringan (alamat, hex, reason) selalu valid dan aman.
/// `raw_payload` memakai versi terpotong dari dekoder agar respons tetap
/// berbatas untuk calldata besar.
fn render_tx_detail_json(
    clean_hash: &str,
    status: &str,
    tx: &crate::transaction::types::Transaction,
    height: Option<u64>,
    interaction: crate::gateway::contract_decode::ContractInteraction,
) -> String {
    use serde_json::json;

    let body = json!({
        "hash": format!("0x{clean_hash}"),
        "status": status,
        "chain_id": tx.chain_id,
        "block_height": height,
        "sender": encode_address_bech32m(&tx.sender, "aur").unwrap_or_default(),
        "recipient": encode_address_bech32m(&tx.recipient, "aur").unwrap_or_default(),
        "amount_quanta": tx.amount.as_u128().to_string(),
        "fee_quanta": tx.fee.as_u128().to_string(),
        "nonce": tx.nonce,
        "valid_until": tx.valid_until,
        "tx_type": tx_type_label(tx.tx_type),
        "raw_payload": format!("0x{}", interaction.raw_payload),
        "raw_payload_bytes": interaction.raw_payload_bytes,
        "raw_payload_truncated": interaction.raw_payload_truncated,
        "size_bytes": crate::transaction::types::transaction_wire_size(tx.payload.len()),
        "contract_interaction": interaction,
    });

    serde_json::to_string(&body).unwrap_or_else(|_| r#"{"status":"error"}"#.to_string())
}

/// Label tipe transaksi yang stabil untuk UI/API.
#[must_use]
pub fn tx_type_label(tx_type: crate::transaction::types::TxType) -> &'static str {
    use crate::transaction::types::TxType;
    match tx_type {
        TxType::Transfer => "transfer",
        TxType::Stake => "stake",
        TxType::Unstake => "unstake",
        TxType::GovernanceVote => "governance_vote",
        TxType::ContractDeploy => "contract_deploy",
        TxType::ContractCall => "contract_call",
    }
}

/// Menyajikan dokumen HTML5 mandiri untuk Community Sandbox Dashboard (/sandbox).
pub fn render_sandbox_html(chain_id: u32) -> String {
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
    /* --- Contract Interaction card (Smart Contract Visualization) --- */
    .grid-wide {{ grid-column: 1 / -1; }}
    .kv {{ display: grid; grid-template-columns: 180px 1fr; gap: 6px 14px; font-size: 13px; margin-top: 10px; }}
    .kv dt {{ color: var(--text-muted); }}
    .kv dd {{ margin: 0; word-break: break-all; }}
    .tag {{ display: inline-block; padding: 2px 8px; border-radius: 99px; font-size: 11px; font-weight: 700; letter-spacing: 0.4px; }}
    .tag-ok {{ background: rgba(0,240,144,0.12); border: 1px solid var(--success); color: var(--success); }}
    .tag-warn {{ background: rgba(255,204,0,0.12); border: 1px solid #ffcc00; color: #ffcc00; }}
    .tag-mute {{ background: rgba(139,148,158,0.12); border: 1px solid var(--text-muted); color: var(--text-muted); }}
    table.args {{ width: 100%; border-collapse: collapse; margin-top: 10px; font-size: 13px; }}
    table.args th, table.args td {{ text-align: left; padding: 6px 8px; border-bottom: 1px solid var(--card-border); word-break: break-all; }}
    table.args th {{ color: var(--text-muted); font-weight: 600; font-size: 11px; text-transform: uppercase; }}
    details.raw {{ margin-top: 12px; }}
    details.raw summary {{ cursor: pointer; color: var(--text-muted); font-size: 12px; }}
    details.raw pre {{ white-space: pre-wrap; word-break: break-all; font-size: 11px; background: rgba(0,0,0,0.45); padding: 10px; border-radius: 6px; max-height: 220px; overflow: auto; }}
    .hidden {{ display: none !important; }}
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
    <div class="card">
      <h3>Transaction Inspector — Smart Contract Visualization</h3>
      <p style="font-size: 13px; color: var(--text-muted);">
        Paste a transaction TxID to decode contract calls into human-readable
        method names and arguments. Non-contract transactions render normally.
      </p>
      <div class="input-group">
        <input type="text" id="tx-hash" placeholder="Transaction TxID (hex, with or without 0x)" />
        <button onclick="inspectTx()">Inspect</button>
      </div>
      <div id="tx-meta" class="result-box"></div>
      <div id="contract-card" class="hidden" style="margin-top:16px;">
        <div style="display:flex;align-items:center;gap:10px;flex-wrap:wrap;">
          <strong style="font-size:14px;">Contract Interaction</strong>
          <span id="ci-kind" class="tag tag-mute">—</span>
          <span id="ci-status" class="tag tag-mute">—</span>
          <span id="ci-decode" class="tag tag-mute">—</span>
        </div>
        <dl class="kv">
          <dt>Contract Address</dt><dd id="ci-address">—</dd>
          <dt>Contract Name</dt><dd id="ci-name">—</dd>
          <dt>Method</dt><dd id="ci-method">—</dd>
          <dt>Selector</dt><dd id="ci-selector">—</dd>
          <dt>Code Hash</dt><dd id="ci-codehash">—</dd>
        </dl>
        <div id="ci-args-wrap">
          <table class="args">
            <thead><tr><th>#</th><th>Parameter</th><th>Type</th><th>Value</th></tr></thead>
            <tbody id="ci-args"></tbody>
          </table>
        </div>
        <div id="ci-reason" style="display:none;margin-top:10px;font-size:12px;color:#ffcc00;"></div>
        <details class="raw">
          <summary>Raw Calldata (advanced)</summary>
          <pre id="ci-raw">—</pre>
        </details>
      </div>
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

    function setTag(id, text, cls) {{
      const el = document.getElementById(id);
      el.textContent = text;
      el.className = 'tag ' + cls;
    }}

    function renderArgs(args) {{
      const tbody = document.getElementById('ci-args');
      tbody.innerHTML = '';
      const list = Array.isArray(args) ? args : [];
      document.getElementById('ci-args-wrap').style.display = list.length ? 'block' : 'none';
      list.forEach(function (a) {{
        const tr = document.createElement('tr');
        // textContent (bukan innerHTML): data kontrak berasal dari jaringan,
        // sehingga tidak boleh pernah di-parse sebagai HTML.
        [String(a.index), a.name || '-', a.abi_type || '-', a.value || '-'].forEach(function (cell) {{
          const td = document.createElement('td');
          td.textContent = cell;
          tr.appendChild(td);
        }});
        tbody.appendChild(tr);
      }});
    }}

    function renderContract(ci) {{
      const card = document.getElementById('contract-card');
      if (!ci || ci.kind === 'none') {{ card.classList.add('hidden'); return; }}
      card.classList.remove('hidden');

      setTag('ci-kind', ci.kind === 'deploy' ? 'DEPLOYMENT' : 'CALL',
             ci.kind === 'deploy' ? 'tag-warn' : 'tag-ok');
      setTag('ci-status', String(ci.status || '').toUpperCase(),
             ci.status === 'finalized' ? 'tag-ok' : 'tag-warn');
      setTag('ci-decode', ci.decode_status === 'decoded' ? 'DECODED' : 'UNKNOWN',
             ci.decode_status === 'decoded' ? 'tag-ok' : 'tag-warn');

      document.getElementById('ci-address').textContent = ci.contract_address || '—';
      document.getElementById('ci-name').textContent = ci.contract_name || '—';
      document.getElementById('ci-method').textContent = ci.method || 'Unknown Method';
      document.getElementById('ci-selector').textContent = ci.selector || '—';
      document.getElementById('ci-codehash').textContent = ci.code_hash || '—';

      renderArgs(ci.arguments);

      const reason = document.getElementById('ci-reason');
      if (ci.reason) {{
        reason.style.display = 'block';
        reason.textContent = ci.reason;
      }} else {{
        reason.style.display = 'none';
      }}

      const raw = (ci.raw_payload || '') +
        (ci.raw_payload_truncated ? '... [truncated, ' + ci.raw_payload_bytes + ' bytes total]' : '');
      document.getElementById('ci-raw').textContent = raw || '—';
    }}

    async function inspectTx() {{
      const hash = document.getElementById('tx-hash').value.trim();
      const meta = document.getElementById('tx-meta');
      const card = document.getElementById('contract-card');
      if (!hash) {{
        meta.style.display = 'block';
        meta.style.color = 'var(--error)';
        meta.textContent = 'Please enter a transaction TxID';
        return;
      }}
      meta.style.display = 'block';
      meta.style.color = 'var(--accent)';
      meta.textContent = 'Loading transaction...';
      card.classList.add('hidden');
      try {{
        const res = await fetch('/api/v1/transactions/' + encodeURIComponent(hash));
        const data = await res.json();
        if (data.status === 'error') {{
          meta.style.color = 'var(--error)';
          meta.textContent = 'Not found: ' + (data.error || 'unknown error');
          return;
        }}
        meta.style.color = 'var(--success)';
        meta.textContent = [
          'TxID: ' + data.hash,
          'Status: ' + data.status,
          'Type: ' + data.tx_type,
          'Block: ' + (data.block_height === null ? 'pending' : data.block_height),
          'From: ' + data.sender,
          'Amount: ' + data.amount_quanta + ' Quanta',
          'Fee: ' + data.fee_quanta + ' Quanta'
        ].join('  |  ');
        renderContract(data.contract_interaction);
      }} catch (e) {{
        meta.style.color = 'var(--error)';
        meta.textContent = 'Network error: ' + e;
      }}
    }}

    updateStats();
    setInterval(updateStats, 3000);
  </script>
</body>
</html>"##, chain_id = chain_id)
}
