#![forbid(unsafe_code)]

//! Aurion Unified Command-Line Interface & Application Control Plane.
//! Sesuai Invariant AUR-ARCH-001, AUR-ARCH-010, dan Dokumen Aturan Aplikasi 15 (AUR-CLI-001 s.d AUR-CLI-007).

pub mod command;
pub mod dispatcher;
pub mod output;

pub use command::CliCommand;
pub use dispatcher::dispatch;
pub use output::OutputFormat;

/// Pintu masuk eksekusi terpadu Aurion CLI (/bin/aurion).
pub async fn run_cli(args: &[String]) -> Result<(), String> {
    let (format, filtered_args) = OutputFormat::parse_and_strip(args);
    let cmd = CliCommand::parse(&filtered_args);
    dispatch(cmd, format).await
}
