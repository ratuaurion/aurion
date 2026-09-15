#![forbid(unsafe_code)]

//! Aurion Single Primary Executable (/bin/aurion).
//! Mematuhi Invariant AUR-ARCH-001, AUR-ARCH-009, & AUR-ARCH-010.

use aurion::conformance::{
    generate_golden_vectors_json, generate_json_report, generate_markdown_report,
    print_terminal_report, run_all_pillars, TestStatus,
};
use aurion::consensus::certificate::ValidatorEntry;
use aurion::core::Address;
use aurion::crypto::{derive_address_from_pubkey, Keypair};
use aurion::genesis::builder::build_genesis;
use aurion::runtime::config::NodeConfig;
use aurion::runtime::node::AurionNode;
use std::env;
use std::fs;
use std::process;

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();
    let command = if args.len() > 1 {
        args[1].as_str()
    } else {
        "help"
    };

    match command {
        "version" | "--version" | "-v" => {
            println!("aurion version 1.0.0 (Sovereign Single-Binary Architecture)");
            println!("Protocol Specification: V1 (RFC 2119 Normative)");
            println!("Cryptographic Engine: Blake3 + Ed25519 (Strict Anti-Malleability)");
            println!("Monetary Hard Cap: 66,000,000 AUR (Scale 10^8 Quantum)");
        }
        "conformance" => {
            handle_conformance_subcommand(&args[2..]);
        }
        "node" => {
            println!("==================================================================");
            println!("  [AURION NODE] Starting Sovereign Full Node Daemon...");
            println!("==================================================================");
            let mut rpc_bind = "127.0.0.1:8545".to_string();
            let mut i = 2;
            while i < args.len() {
                if (args[i] == "--rpc-bind" || args[i] == "-b") && i + 1 < args.len() {
                    rpc_bind = args[i + 1].clone();
                    i += 1;
                }
                i += 1;
            }

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

            println!("[AURION NODE] Genesis State initialized successfully.");
            println!("[AURION NODE] Chain ID: {}", node.config.chain_id);
            println!("[AURION NODE] Ledger Height: {}", node.ledger.lock().unwrap().latest_height());
            println!("[AURION NODE] P2P Wire Protocol: Magic AUR0 on {}", node.config.p2p_bind);
            println!("[AURION NODE] Serving JSON-RPC 2.0 and WebSocket on http://{}", node.config.rpc_bind);
            println!("[AURION NODE] Press Ctrl+C to stop.");

            if let Err(e) = node.run_rpc_server(None).await {
                eprintln!("[AURION NODE] Server error: {e}");
            }
        }
        "validator" => {
            println!("==================================================================");
            println!("  [AURION VALIDATOR] Starting BFT Consensus Validator Engine...");
            println!("==================================================================");
            let mut rpc_bind = "127.0.0.1:8545".to_string();
            let mut i = 2;
            while i < args.len() {
                if (args[i] == "--rpc-bind" || args[i] == "-b") && i + 1 < args.len() {
                    rpc_bind = args[i + 1].clone();
                    i += 1;
                }
                i += 1;
            }

            let config = NodeConfig {
                rpc_bind,
                ..NodeConfig::new_validator(Vec::new())
            };

            let val_key = Keypair::generate();
            let val_addr = derive_address_from_pubkey(&val_key.public_key_bytes());
            let dev_addr = Address::from_bytes([2u8; 32]);
            let val_entry = ValidatorEntry {
                validator_id: val_addr,
                consensus_pubkey: val_key.public_key_bytes(),
                voting_weight: 100,
            };
            let genesis = build_genesis(val_addr, dev_addr, vec![val_entry]);
            let node = AurionNode::new(config, genesis, Some(val_key), Some(0));

            println!("[AURION VALIDATOR] Validator consensus keypair active.");
            println!("[AURION VALIDATOR] Consensus Algorithm: Single-Slot BFT Finality (2/3+ Quorum).");
            println!("[AURION VALIDATOR] Serving JSON-RPC 2.0 & Status Gateway on http://{}", node.config.rpc_bind);
            println!("[AURION VALIDATOR] Press Ctrl+C to stop.");

            if let Err(e) = node.run_rpc_server(None).await {
                eprintln!("[AURION VALIDATOR] Server error: {e}");
            }
        }
        "wallet" => {
            aurion::wallet::cli::handle_wallet_subcommand(&args[2..]);
        }
        "rpc" => {
            println!("==================================================================");
            println!("  [AURION RPC] Starting Standalone Gateway JSON-RPC 2.0 Server...");
            println!("==================================================================");
            let mut rpc_bind = "127.0.0.1:8545".to_string();
            let mut i = 2;
            while i < args.len() {
                if (args[i] == "--bind" || args[i] == "-b") && i + 1 < args.len() {
                    rpc_bind = args[i + 1].clone();
                    i += 1;
                }
                i += 1;
            }

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
        }
        _ => {
            print_help();
        }
    }
}


fn handle_conformance_subcommand(subargs: &[String]) {
    let subcmd = if !subargs.is_empty() {
        subargs[0].as_str()
    } else {
        "run"
    };

    match subcmd {
        "run" => {
            println!("\n[AURION CTS] Initializing 8-Pillar Protocol Conformance Audit...");
            let results = run_all_pillars();
            print_terminal_report(&results);

            // Handle optional exports
            let mut i = 1;
            while i < subargs.len() {
                match subargs[i].as_str() {
                    "--export" | "-e" if i + 1 < subargs.len() => {
                        let path = &subargs[i + 1];
                        let json = generate_json_report(&results);
                        if let Err(e) = fs::write(path, json) {
                            eprintln!("Error writing JSON report to {path}: {e}");
                        } else {
                            println!("  [OK] Exported machine-readable JSON report: {path}");
                        }
                        i += 1;
                    }
                    "--export-md" if i + 1 < subargs.len() => {
                        let path = &subargs[i + 1];
                        let md = generate_markdown_report(&results);
                        if let Err(e) = fs::write(path, md) {
                            eprintln!("Error writing Markdown report to {path}: {e}");
                        } else {
                            println!("  [OK] Exported Markdown report: {path}");
                        }
                        i += 1;
                    }
                    _ => {}
                }
                i += 1;
            }

            let all_passed = results.iter().all(|r| r.status == TestStatus::Passed);
            if !all_passed {
                process::exit(1);
            }
        }
        "export-vectors" => {
            let vectors_json = generate_golden_vectors_json();
            let mut output_path = None;

            let mut i = 1;
            while i < subargs.len() {
                if (subargs[i] == "--output" || subargs[i] == "-o") && i + 1 < subargs.len() {
                    output_path = Some(subargs[i + 1].clone());
                    i += 1;
                }
                i += 1;
            }

            if let Some(path) = output_path {
                if let Err(e) = fs::write(&path, &vectors_json) {
                    eprintln!("Error writing golden vectors to {path}: {e}");
                    process::exit(1);
                } else {
                    println!("  [OK] Successfully exported golden test vectors to: {path}");
                }
            } else {
                println!("{vectors_json}");
            }
        }
        _ => {
            println!("Usage: aurion conformance <run|export-vectors> [options]");
            println!("  run [--export report.json] [--export-md report.md]");
            println!("  export-vectors [--output vectors.json]");
        }
    }
}

fn print_help() {
    println!("==================================================================");
    println!("  AURION — Sovereign Cryptocurrency & Blockchain Ecosystem");
    println!("  Distribution: Single Authoritative Executable (/bin/aurion)");
    println!("==================================================================");
    println!("\nUsage: aurion <command> [options]\n");
    println!("Commands:");
    println!("  conformance  Execute the 8-pillar protocol conformance test suite");
    println!("               Subcommands: run, export-vectors");
    println!("  node         Start the decentralized P2P full node daemon");
    println!("  validator    Start the BFT consensus validator engine");
    println!("  wallet       Manage keys, addresses, and sign transactions");
    println!("  rpc          Start the public/private JSON-RPC 2.0 server");
    println!("  version      Display system version and constitutional parameters");
    println!("\nDocumentation: docs/Constitutions/ and docs/Application-Rules-Layer/");
}
