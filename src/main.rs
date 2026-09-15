#![forbid(unsafe_code)]

//! Aurion Single Primary Executable (/bin/aurion).
//! Mematuhi Invariant AUR-ARCH-001, AUR-ARCH-009, AUR-ARCH-010, & AUR-CLI-001 s.d AUR-CLI-007.

use std::env;
use std::process;

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();
    let cli_args = if args.len() > 1 {
        &args[1..]
    } else {
        &[]
    };

    if let Err(err) = aurion::cli::run_cli(cli_args).await {
        eprintln!("[AURION CLI ERROR] {err}");
        process::exit(1);
    }
}
