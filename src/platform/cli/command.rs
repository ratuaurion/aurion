//! Taksonomi perintah Unified CLI Aurion.
//! Mematuhi Dokumen Aturan Aplikasi 15 (15-UNIFIED-CLI-SPECIFICATION.md).

#[derive(Debug, PartialEq, Eq)]
pub enum CliCommand {
    Node(Vec<String>),
    Validator(Vec<String>),
    Wallet(Vec<String>),
    Account(Vec<String>),
    Block(Vec<String>),
    Tx(Vec<String>),
    Storage(Vec<String>),
    Genesis(Vec<String>),
    Rpc(Vec<String>),
    Query(Vec<String>),
    Network(Vec<String>),
    Contract(Vec<String>),
    Conformance(Vec<String>),
    L2(Vec<String>),
    Specialized(Vec<String>),
    L4(Vec<String>),
    L5(Vec<String>),
    Devnet(Vec<String>),
    Testnet(Vec<String>),
    Snapshot(Vec<String>),
    Version,
    Help,
}

impl CliCommand {
    pub fn parse(args: &[String]) -> Self {
        if args.is_empty() {
            return Self::Help;
        }

        match args[0].as_str() {
            "node" => Self::Node(args[1..].to_vec()),
            "validator" => Self::Validator(args[1..].to_vec()),
            "wallet" => Self::Wallet(args[1..].to_vec()),
            "account" => Self::Account(args[1..].to_vec()),
            "block" => Self::Block(args[1..].to_vec()),
            "tx" => Self::Tx(args[1..].to_vec()),
            "storage" => Self::Storage(args[1..].to_vec()),
            "genesis" => Self::Genesis(args[1..].to_vec()),
            "rpc" => Self::Rpc(args[1..].to_vec()),
            "query" => Self::Query(args[1..].to_vec()),
            "network" => Self::Network(args[1..].to_vec()),
            "contract" => Self::Contract(args[1..].to_vec()),
            "conformance" => Self::Conformance(args[1..].to_vec()),
            "l2" => Self::L2(args[1..].to_vec()),
            "specialized" | "l3" => Self::Specialized(args[1..].to_vec()),
            "interop" | "l4" => Self::L4(args[1..].to_vec()),
            "infra" | "l5" => Self::L5(args[1..].to_vec()),
            "devnet" => Self::Devnet(args[1..].to_vec()),
            "testnet" => Self::Testnet(args[1..].to_vec()),
            "snapshot" => Self::Snapshot(args[1..].to_vec()),
            "version" | "-v" | "--version" => Self::Version,
            "help" | "-h" | "--help" => Self::Help,
            _ => Self::Help,
        }
    }
}
