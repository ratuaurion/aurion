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
    Faucet(Vec<String>),
    Explorer(Vec<String>),
    Audit(Vec<String>),
    Metrics(Vec<String>),
    Governance(Vec<String>),
    Recovery(Vec<String>),
    Version,
    Help,
    /// Bantuan untuk **subperintah** tertentu, mis. `aurion node --help`.
    ///
    /// Dihasilkan hanya bila argumen help muncul SETELAH nama perintah, sehingga
    /// `aurion --help` (tanpa perintah) tetap memakai `Help` biasa.
    SubcommandHelp {
        command: String,
        args: Vec<String>,
    },
}

/// Perintah yang memiliki layar bantuan khusus (bukan master help).
///
/// Daftar ini eksplisit agar `aurion <apa-pun> --help` tidak pernah jatuh ke
/// penangan yang menjalankan proses sungguhan.
pub const HELP_CAPABLE_COMMANDS: &[&str] = &[
    "node",
    "validator",
    "wallet",
    "account",
    "block",
    "tx",
    "storage",
    "genesis",
    "rpc",
    "query",
    "network",
    "contract",
    "conformance",
    "l2",
    "specialized",
    "l3",
    "interop",
    "l4",
    "infra",
    "l5",
    "devnet",
    "testnet",
    "snapshot",
    "faucet",
    "explorer",
    "audit",
    "metrics",
    "governance",
    "gov",
    "recovery",
    "dr",
];

/// Deteksi permintaan bantuan pada tingkat subperintah.
///
/// Mengembalikan `true` bila `args` berisi `--help`/`-h`/`help` di posisi mana
/// pun setelah nama perintah. Ini yang mencegah `aurion node --help` menjalankan
/// node sungguhan — bug yang hanya ditemukan saat perintah "aman" tidak
/// berbahaya seperti yang diasumsikan.
#[must_use]
pub fn wants_subcommand_help(args: &[String]) -> bool {
    args.iter()
        .any(|a| a == "--help" || a == "-h" || a == "help")
}

impl CliCommand {
    pub fn parse(args: &[String]) -> Self {
        if args.is_empty() {
            return Self::Help;
        }

        // PERBAIKAN BUG: `--help`/`-h`/`help` setelah nama perintah harus
        // menampilkan bantuan, bukan menjalankan proses. Sebelumnya
        // `aurion node --help` jatuh ke `CliCommand::Node(["--help"])`, yang
        // default sub-nya "start" sehingga Full Node benar-benar dijalankan
        // (dan menyambung ke bootnode mainnet).
        if args[0] != "help" && args[0] != "-h" && args[0] != "--help" {
            let is_known = HELP_CAPABLE_COMMANDS.contains(&args[0].as_str());
            if is_known && wants_subcommand_help(&args[1..]) {
                return Self::SubcommandHelp {
                    command: args[0].clone(),
                    args: args[1..].to_vec(),
                };
            }
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
            "faucet" => Self::Faucet(args[1..].to_vec()),
            "explorer" => Self::Explorer(args[1..].to_vec()),
            "audit" => Self::Audit(args[1..].to_vec()),
            "metrics" => Self::Metrics(args[1..].to_vec()),
            "governance" | "gov" => Self::Governance(args[1..].to_vec()),
            "recovery" | "dr" => Self::Recovery(args[1..].to_vec()),
            "version" | "-v" | "--version" => Self::Version,
            "help" | "-h" | "--help" => Self::Help,
            _ => Self::Help,
        }
    }
}
