#![forbid(unsafe_code)]

//! Aurion Single Primary Executable (/bin/aurion).
//! Mematuhi Invariant AUR-ARCH-001, AUR-ARCH-009, AUR-ARCH-010, & AUR-CLI-001 s.d AUR-CLI-007.

use std::env;
use std::process;

// Runtime Tokio eksplisit dengan alokasi worker yang memadai dan blocking pool
// yang cukup (AUR-ISSUE-011). Worker starvation pada runtime default (2-4 thread
// di container/WSL2) dapat membekukan accept loop gateway & konsensus; guard ini
// memberi headroom sehingga handler blocking tidak menghabiskan seluruh worker.
fn main() {
    let args: Vec<String> = env::args().collect();
    let cli_args = if args.len() > 1 { &args[1..] } else { &[] };

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(std::cmp::max(
            4,
            std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(4),
        ))
        .max_blocking_threads(64)
        .build()
        .expect("failed to build Aurion tokio runtime");

    if let Err(err) = runtime.block_on(aurion::cli::run_cli(cli_args)) {
        eprintln!("[AURION CLI ERROR] {err}");
        process::exit(1);
    }
}
