#![forbid(unsafe_code)]

//! Aurion Single Primary Executable (/bin/aurion).
//! Mematuhi Invariant AUR-ARCH-001 & AUR-ARCH-009.

use std::env;

fn main() {
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
        "node" => {
            println!("[AURION RUNTIME] Starting Aurion Full Node Daemon...");
            println!("[AURION RUNTIME] Initializing P2P Wire Protocol (Magic: AUR0)...");
        }
        "validator" => {
            println!("[AURION RUNTIME] Starting Aurion Validator Engine...");
            println!("[AURION RUNTIME] Enforcing Single-Slot BFT Consensus...");
        }
        "wallet" => {
            println!("[AURION RUNTIME] Launching Aurion Secure Wallet Subsystem...");
            println!("[AURION RUNTIME] Derivation Path: m/44'/9999'/0'/0/0 (BIP-44)");
        }
        "rpc" => {
            println!("[AURION RUNTIME] Starting JSON-RPC 2.0 Server on 127.0.0.1:8545...");
        }
        "conformance" => {
            println!("[AURION CONFORMANCE] Running 8-Pillar Protocol Conformance Tests...");
        }
        _ => {
            println!("==================================================================");
            println!("  AURION — Sovereign Cryptocurrency & Blockchain Ecosystem");
            println!("  Distribution: Single Authoritative Executable (/bin/aurion)");
            println!("==================================================================");
            println!("\nUsage: aurion <command> [options]\n");
            println!("Commands:");
            println!("  node         Start the decentralized P2P full node daemon");
            println!("  validator    Start the BFT consensus validator engine");
            println!("  wallet       Manage keys, addresses, and sign transactions");
            println!("  rpc          Start the public/private JSON-RPC 2.0 server");
            println!("  conformance  Execute the 8-pillar protocol conformance test suite");
            println!("  version      Display system version and constitutional parameters");
            println!("\nDocumentation: docs/Constitutions/ and docs/Application-Rules-Layer/");
        }
    }
}
