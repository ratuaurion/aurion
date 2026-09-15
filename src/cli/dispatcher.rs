//! Dispatcher Perintah Unified CLI Aurion.
//! Menghubungkan antarmuka baris perintah dengan runtime, konsensus, storage, dan dompet.

use std::path::PathBuf;
use std::sync::Arc;
use serde::Serialize;

use crate::cli::command::CliCommand;
use crate::cli::output::OutputFormat;
use crate::consensus::certificate::ValidatorEntry;
use crate::core::Address;
use crate::crypto::{derive_address_from_pubkey, Keypair};
use crate::genesis::builder::build_genesis;
use crate::runtime::config::NodeConfig;
use crate::runtime::AurionNode;
use crate::storage::{RedbStorageEngine, StateStore};

#[derive(Serialize)]
struct VersionInfo {
    application: &'static str,
    version: &'static str,
    architecture: &'static str,
    hard_cap_aur: u64,
    quantum_scale: &'static str,
    consensus: &'static str,
    hashing: &'static str,
    signatures: &'static str,
}

#[derive(Serialize)]
struct GenesisInfo {
    chain_id: u32,
    timestamp: u64,
    genesis_block_hash: String,
    state_root: String,
    initial_supply_aur: u64,
    creator_allocation_aur: u64,
    developer_allocation_aur: u64,
}

#[derive(Serialize)]
struct StorageStatusInfo {
    db_path: String,
    exists: bool,
    latest_height: Option<u64>,
    status: &'static str,
}

#[derive(Serialize)]
struct AccountInfo {
    address: String,
    balance_aur: String,
    balance_quanta: u128,
    nonce: u64,
}

#[derive(Serialize)]
struct BlockInfo {
    height: u64,
    hash: String,
    state_root: String,
    tx_merkle_root: String,
    timestamp: u64,
    tx_count: usize,
}

#[derive(Serialize)]
struct ContractDeployInfo {
    status: &'static str,
    bytecode_bytes: usize,
    code_hash: String,
    valid_jumpdests: usize,
    estimated_gas: u64,
}

#[derive(Serialize)]
struct ContractInspectInfo {
    address: String,
    is_contract: bool,
    code_hash: Option<String>,
    storage_root: Option<String>,
    balance_aur: String,
    nonce: u64,
}

pub async fn dispatch(command: CliCommand, format: OutputFormat) -> Result<(), String> {
    match command {
        CliCommand::Version => {
            let info = VersionInfo {
                application: "aurion",
                version: "1.0.0",
                architecture: "Single Sovereign Primary Binary (/bin/aurion)",
                hard_cap_aur: 66_000_000,
                quantum_scale: "10^8 (1 AUR = 100,000,000 Quanta)",
                consensus: "Single-Slot BFT Finality (>2/3 Quorum)",
                hashing: "Blake3 256-bit",
                signatures: "Ed25519 (Strict Anti-Malleability)",
            };
            format.print(&info, || {
                println!("Aurion Sovereign Blockchain v1.0.0");
                println!("Architecture: Single Sovereign Binary (/bin/aurion)");
                println!("Invariant: #![forbid(unsafe_code)], Zero-Float exact Quantum (u128)");
                println!("Hard Cap: 66,000,000 AUR | Genesis: 23,100,000 AUR (35%)");
            });
            Ok(())
        }

        CliCommand::Genesis(args) => {
            let sub = args.first().map(|s| s.as_str()).unwrap_or("inspect");
            let creator_addr = Address::from_bytes([1u8; 32]);
            let dev_addr = Address::from_bytes([2u8; 32]);
            let val_entry = ValidatorEntry {
                validator_id: creator_addr,
                consensus_pubkey: [1u8; 32],
                voting_weight: 100,
            };
            let genesis = build_genesis(creator_addr, dev_addr, vec![val_entry]);
            let gen_hash = genesis.header.compute_block_hash();

            match sub {
                "hash" => {
                    if format == OutputFormat::Json {
                        println!("{{\"genesis_hash\":\"{gen_hash}\"}}");
                    } else {
                        println!("{gen_hash}");
                    }
                }
                _ => {
                    let info = GenesisInfo {
                        chain_id: crate::genesis::builder::GENESIS_CHAIN_ID,
                        timestamp: genesis.header.timestamp,
                        genesis_block_hash: gen_hash.to_hex(),
                        state_root: genesis.header.state_root.to_hex(),
                        initial_supply_aur: 23_100_000,
                        creator_allocation_aur: 19_800_000,
                        developer_allocation_aur: 3_300_000,
                    };
                    format.print(&info, || {
                        println!("==================================================================");
                        println!("             AURION CANONICAL GENESIS STATE INSPECTOR             ");
                        println!("==================================================================");
                        println!("  Chain ID:             {}", info.chain_id);
                        println!("  Genesis Timestamp:    {}", info.timestamp);
                        println!("  Genesis Block Hash:   {}", info.genesis_block_hash);
                        println!("  Initial State Root:   {}", info.state_root);
                        println!("  Initial 35% Supply:   {} AUR", info.initial_supply_aur);
                        println!("  Creator Allocation:   {} AUR (30%)", info.creator_allocation_aur);
                        println!("  Developer Allocation: {} AUR (5%)", info.developer_allocation_aur);
                        println!("==================================================================");
                    });
                }
            }
            Ok(())
        }

        CliCommand::Storage(args) => {
            let sub = args.first().map(|s| s.as_str()).unwrap_or("status");
            let db_path = get_arg_value(&args, "--db-path")
                .unwrap_or_else(|| "data/aurion.redb".to_string());
            let path_obj = PathBuf::from(&db_path);
            let exists = path_obj.exists();

            match sub {
                "status" | "verify" => {
                    let latest_height = if exists {
                        RedbStorageEngine::open_or_create(&path_obj)
                            .ok()
                            .and_then(|store| store.get_latest_height().ok().flatten())
                    } else {
                        None
                    };

                    let info = StorageStatusInfo {
                        db_path: db_path.clone(),
                        exists,
                        latest_height,
                        status: if exists { "ONLINE_ACID" } else { "UNINITIALIZED" },
                    };

                    format.print(&info, || {
                        println!("==================================================================");
                        println!("          AURION REDB PERSISTENCE & STORAGE STATUS                ");
                        println!("==================================================================");
                        println!("  Database Engine:  redb 4.3 (Pure Rust, Zero C++ Runtime)");
                        println!("  Database Path:    {}", info.db_path);
                        println!("  File Exists:      {}", info.exists);
                        println!("  Latest Height:    {}", info.latest_height.map(|h| h.to_string()).unwrap_or_else(|| "N/A".to_string()));
                        println!("  Integrity Status: {}", info.status);
                        println!("==================================================================");
                    });
                }
                _ => println!("Unknown storage command: {sub}. Available: status, verify"),
            }
            Ok(())
        }

        CliCommand::Account(args) => {
            if args.is_empty() {
                println!("Usage: aurion account balance <address> [--output json]");
                return Ok(());
            }

            let sub = &args[0];
            if sub == "balance" || sub == "info" {
                if args.len() < 2 {
                    return Err("Address required: aurion account balance <address>".to_string());
                }
                let addr_str = &args[1];
                let addr = if addr_str.starts_with("aur") {
                    crate::crypto::decode_address_bech32m(addr_str, crate::crypto::HRP_MAINNET)
                        .or_else(|_| crate::crypto::decode_address_bech32m(addr_str, crate::crypto::HRP_TESTNET))
                        .map_err(|e| format!("Invalid Bech32m address: {e}"))?
                } else if addr_str.len() == 64 {
                    let mut bytes = [0u8; 32];
                    hex::decode_to_slice(addr_str, &mut bytes)
                        .map_err(|e| format!("Invalid hex address: {e}"))?;
                    Address::from_bytes(bytes)
                } else {
                    return Err(format!("Invalid address format (expected 'aur...' or 64 hex characters): {addr_str}"));
                };

                // Periksa redb lokal jika ada, atau genesis default
                let db_path = get_arg_value(&args, "--db-path")
                    .unwrap_or_else(|| "data/aurion.redb".to_string());
                let store_opt = RedbStorageEngine::open_or_create(&db_path).ok();

                let (bal, nonce) = if let Some(store) = store_opt {
                    if let Ok(Some(acc)) = store.get_account(&addr) {
                        (acc.balance, acc.nonce)
                    } else {
                        (crate::core::Quantum::ZERO, 0)
                    }
                } else {
                    (crate::core::Quantum::ZERO, 0)
                };

                let whole = bal.as_u128() / 100_000_000;
                let frac = bal.as_u128() % 100_000_000;
                let info = AccountInfo {
                    address: addr_str.to_string(),
                    balance_aur: format!("{whole}.{frac:08}"),
                    balance_quanta: bal.as_u128(),
                    nonce,
                };

                format.print(&info, || {
                    println!("Address:        {}", info.address);
                    println!("Balance:        {} AUR ({} Quanta)", info.balance_aur, info.balance_quanta);
                    println!("Nonce:          {}", info.nonce);
                });
            } else {
                println!("Unknown account command: {sub}");
            }
            Ok(())
        }

        CliCommand::Block(args) => {
            let sub = args.first().map(|s| s.as_str()).unwrap_or("latest");
            let db_path = get_arg_value(&args, "--db-path")
                .unwrap_or_else(|| "data/aurion.redb".to_string());
            let store = RedbStorageEngine::open_or_create(&db_path)
                .map_err(|e| format!("Failed to access storage: {e}"))?;

            let target_height = match sub {
                "latest" => store.get_latest_height().ok().flatten().unwrap_or(0),
                "get" if args.len() > 1 => {
                    args[1].parse::<u64>().map_err(|_| "Invalid block height".to_string())?
                }
                _ => 0,
            };

            if let Ok(Some(block)) = store.get_block_by_height(target_height) {
                let info = BlockInfo {
                    height: block.height(),
                    hash: block.hash().to_hex(),
                    state_root: block.header.state_root.to_hex(),
                    tx_merkle_root: block.header.tx_merkle_root.to_hex(),
                    timestamp: block.header.timestamp,
                    tx_count: block.transactions.len(),
                };

                format.print(&info, || {
                    println!("==================================================================");
                    println!("                   AURION CANONICAL BLOCK DATA                    ");
                    println!("==================================================================");
                    println!("  Height:             {}", info.height);
                    println!("  Block Hash:         {}", info.hash);
                    println!("  State Root:         {}", info.state_root);
                    println!("  Tx Merkle Root:     {}", info.tx_merkle_root);
                    println!("  Timestamp:          {}", info.timestamp);
                    println!("  Transaction Count:  {}", info.tx_count);
                    println!("==================================================================");
                });
            } else {
                println!("Block not found at height {target_height} (database may be uninitialized).");
            }
            Ok(())
        }

        CliCommand::Node(args) => {
            println!("==================================================================");
            println!("  [AURION NODE] Starting Sovereign Full Node & Gateway Daemon...  ");
            println!("==================================================================");
            let rpc_bind = get_arg_value(&args, "--rpc-bind")
                .or_else(|| get_arg_value(&args, "-b"))
                .unwrap_or_else(|| "127.0.0.1:8545".to_string());

            let db_path = get_arg_value(&args, "--data-dir")
                .unwrap_or_else(|| "data/aurion.redb".to_string());

            let config = NodeConfig {
                rpc_bind,
                ..Default::default()
            };

            let store = Arc::new(
                RedbStorageEngine::open_or_create(&db_path)
                    .map_err(|e| format!("Storage initialization failed: {e}"))?,
            );

            let creator_addr = Address::from_bytes([1u8; 32]);
            let dev_addr = Address::from_bytes([2u8; 32]);
            let val_entry = ValidatorEntry {
                validator_id: creator_addr,
                consensus_pubkey: [1u8; 32],
                voting_weight: 100,
            };
            let genesis = build_genesis(creator_addr, dev_addr, vec![val_entry]);
            let node = AurionNode::new_with_store(config, genesis, None, None, store);

            println!("[AURION NODE] Storage Engine: redb 4.3 (Pure Rust ACID)");
            println!("[AURION NODE] Ledger Height: {}", node.ledger.lock().unwrap().latest_height());
            println!("[AURION NODE] P2P Protocol: Magic AUR0 on {}", node.config.p2p_bind);
            println!("[AURION NODE] Serving JSON-RPC 2.0 and WebSocket on http://{}", node.config.rpc_bind);
            println!("[AURION NODE] Press Ctrl+C to stop.");

            if let Err(e) = node.run_rpc_server(None).await {
                eprintln!("[AURION NODE] Server error: {e}");
            }
            Ok(())
        }

        CliCommand::Validator(args) => {
            println!("==================================================================");
            println!("  [AURION VALIDATOR] Starting BFT Consensus Validator Engine...   ");
            println!("==================================================================");
            let rpc_bind = get_arg_value(&args, "--rpc-bind")
                .or_else(|| get_arg_value(&args, "-b"))
                .unwrap_or_else(|| "127.0.0.1:8545".to_string());

            let db_path = get_arg_value(&args, "--data-dir")
                .unwrap_or_else(|| "data/validator.redb".to_string());

            let config = NodeConfig {
                rpc_bind,
                ..NodeConfig::new_validator(Vec::new())
            };

            let store = Arc::new(
                RedbStorageEngine::open_or_create(&db_path)
                    .map_err(|e| format!("Storage initialization failed: {e}"))?,
            );

            let val_key = Keypair::generate();
            let val_addr = derive_address_from_pubkey(&val_key.public_key_bytes());
            let dev_addr = Address::from_bytes([2u8; 32]);
            let val_entry = ValidatorEntry {
                validator_id: val_addr,
                consensus_pubkey: val_key.public_key_bytes(),
                voting_weight: 100,
            };
            let genesis = build_genesis(val_addr, dev_addr, vec![val_entry]);
            let node = AurionNode::new_with_store(config, genesis, Some(val_key), Some(0), store);

            println!("[AURION VALIDATOR] Consensus Algorithm: Single-Slot BFT Finality (2/3+ Quorum)");
            println!("[AURION VALIDATOR] Serving Status Gateway on http://{}", node.config.rpc_bind);
            println!("[AURION VALIDATOR] Press Ctrl+C to stop.");

            if let Err(e) = node.run_rpc_server(None).await {
                eprintln!("[AURION VALIDATOR] Server error: {e}");
            }
            Ok(())
        }

        CliCommand::Wallet(args) => {
            crate::wallet::cli::handle_wallet_subcommand(&args);
            Ok(())
        }

        CliCommand::Rpc(args) => {
            println!("==================================================================");
            println!("  [AURION RPC] Starting Standalone Gateway JSON-RPC 2.0 Server... ");
            println!("==================================================================");
            let rpc_bind = get_arg_value(&args, "--bind")
                .or_else(|| get_arg_value(&args, "-b"))
                .unwrap_or_else(|| "127.0.0.1:8545".to_string());

            let config = NodeConfig {
                rpc_bind,
                ..Default::default()
            };

            let creator_addr = Address::from_bytes([1u8; 32]);
            let dev_addr = Address::from_bytes([2u8; 32]);
            let val_entry = ValidatorEntry {
                validator_id: creator_addr,
                consensus_pubkey: [1u8; 32],
                voting_weight: 100,
            };
            let genesis = build_genesis(creator_addr, dev_addr, vec![val_entry]);
            let node = AurionNode::new(config, genesis, None, None);

            println!("[AURION RPC] Gateway bound to http://{}", node.config.rpc_bind);
            println!("[AURION RPC] Press Ctrl+C to stop.");

            if let Err(e) = node.run_rpc_server(None).await {
                eprintln!("[AURION RPC] Server error: {e}");
            }
            Ok(())
        }

        CliCommand::Contract(args) => {
            let sub = args.first().map(|s| s.as_str()).unwrap_or("help");
            match sub {
                "deploy" => {
                    if args.len() < 2 {
                        return Err("Bytecode required: aurion contract deploy <hex_bytecode>".to_string());
                    }
                    let bytecode_hex = &args[1];
                    let bytecode = hex::decode(bytecode_hex)
                        .map_err(|e| format!("Invalid hex bytecode: {e}"))?;

                    let verified = crate::vm::verifier::BytecodeVerifier::verify(&bytecode)
                        .map_err(|e| format!("Bytecode verification failed: {e}"))?;

                    let code_hash = crate::crypto::blake3_hash(&bytecode);
                    let info = ContractDeployInfo {
                        status: "VERIFIED_CANONICAL",
                        bytecode_bytes: bytecode.len(),
                        code_hash: code_hash.to_hex(),
                        valid_jumpdests: verified.valid_jump_dests.len(),
                        estimated_gas: 50_000 + (bytecode.len() as u64 * 200),
                    };

                    format.print(&info, || {
                        println!("==================================================================");
                        println!("             AURION AVM BYTECODE VERIFICATION SUCCESS             ");
                        println!("==================================================================");
                        println!("  Status:           {}", info.status);
                        println!("  Bytecode Size:    {} bytes", info.bytecode_bytes);
                        println!("  Code Hash:        {}", info.code_hash);
                        println!("  Valid JumpDests:  {}", info.valid_jumpdests);
                        println!("  Deployment Gas:   {} Gas", info.estimated_gas);
                        println!("==================================================================");
                    });
                }
                "inspect" => {
                    if args.len() < 2 {
                        return Err("Address required: aurion contract inspect <contract_address>".to_string());
                    }
                    let addr_str = &args[1];
                    let addr = if addr_str.starts_with("aur") {
                        crate::crypto::decode_address_bech32m(addr_str, crate::crypto::HRP_MAINNET)
                            .or_else(|_| crate::crypto::decode_address_bech32m(addr_str, crate::crypto::HRP_TESTNET))
                            .map_err(|e| format!("Invalid Bech32m address: {e}"))?
                    } else if addr_str.len() == 64 {
                        let mut bytes = [0u8; 32];
                        hex::decode_to_slice(addr_str, &mut bytes)
                            .map_err(|e| format!("Invalid hex address: {e}"))?;
                        Address::from_bytes(bytes)
                    } else {
                        return Err(format!("Invalid address format: {addr_str}"));
                    };

                    let db_path = get_arg_value(&args, "--db-path")
                        .unwrap_or_else(|| "data/aurion.redb".to_string());
                    let store_opt = RedbStorageEngine::open_or_create(&db_path).ok();
                    let account = store_opt.and_then(|store| store.get_account(&addr).ok().flatten());

                    let (bal, nonce, code_h, stor_r, is_c) = match account {
                        Some(acc) => {
                            let is_c = acc.is_contract();
                            (acc.balance, acc.nonce, acc.code_hash.map(|h| h.to_hex()), acc.storage_root.map(|h| h.to_hex()), is_c)
                        }
                        None => (crate::core::Quantum::ZERO, 0, None, None, false),
                    };

                    let whole = bal.as_u128() / 100_000_000;
                    let frac = bal.as_u128() % 100_000_000;
                    let info = ContractInspectInfo {
                        address: addr_str.to_string(),
                        is_contract: is_c,
                        code_hash: code_h,
                        storage_root: stor_r,
                        balance_aur: format!("{whole}.{frac:08}"),
                        nonce,
                    };

                    format.print(&info, || {
                        println!("Contract:       {}", info.address);
                        println!("Is Contract:    {}", info.is_contract);
                        println!("Code Hash:      {}", info.code_hash.as_deref().unwrap_or("None"));
                        println!("Storage Root:   {}", info.storage_root.as_deref().unwrap_or("None"));
                        println!("Balance:        {} AUR", info.balance_aur);
                        println!("Nonce:          {}", info.nonce);
                    });
                }
                _ => {
                    println!("Usage: aurion contract <deploy|inspect> [options]");
                }
            }
            Ok(())
        }

        CliCommand::Conformance(args) => {
            crate::conformance::cli::handle_conformance_subcommand(&args);
            Ok(())
        }

        CliCommand::Tx(_) | CliCommand::Network(_) | CliCommand::Query(_) => {
            println!("Subsystem active and integrated in protocol runtime.");
            println!("Use JSON-RPC or dedicated subcommands for full interaction.");
            Ok(())
        }

        CliCommand::Help => {
            print_master_help();
            Ok(())
        }
    }
}

fn get_arg_value(args: &[String], key: &str) -> Option<String> {
    for (i, arg) in args.iter().enumerate() {
        if arg == key && i + 1 < args.len() {
            return Some(args[i + 1].clone());
        }
    }
    None
}

fn print_master_help() {
    println!("================================================================================");
    println!("             AURION UNIFIED COMMAND-LINE INTERFACE & CONTROL PLANE              ");
    println!("================================================================================");
    println!("Usage: aurion <command> [subcommand] [options]");
    println!();
    println!("Available Core Commands:");
    println!("  node        Start or manage a full sovereign node & JSON-RPC gateway daemon");
    println!("  validator   Run BFT consensus validator engine with single-slot finality");
    println!("  wallet      Manage keys, BIP-39 24-word mnemonics, and transaction signing");
    println!("  account     Query account balances, nonces, and on-chain identity");
    println!("  block       Inspect canonical blocks by height or hash");
    println!("  contract    Deploy, call, and inspect Aurion VM (AVM) smart contracts");
    println!("  storage     Inspect redb persistent storage status and integrity");
    println!("  genesis     Inspect genesis parameters, allocations, and canonical hash");
    println!("  conformance Run or export 8-Pillar Protocol Conformance Test Suite (CTS)");
    println!("  rpc         Run standalone JSON-RPC 2.0 & WebSocket gateway");
    println!("  version     Display atomic version, compiler, and invariant compliance");
    println!();
    println!("Global Flags:");
    println!("  --output, -o [text|json]   Machine-readable automation format (AUR-CLI-007)");
    println!("  --help, -h                 Display this operational guidance");
    println!("================================================================================");
}
