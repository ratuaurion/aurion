#![forbid(unsafe_code)]

//! Test Integrasi Unified CLI & Application Control Plane (/bin/aurion).
//! Mematuhi Dokumen 15 (15-UNIFIED-CLI-SPECIFICATION.md) dan Invariant AUR-CLI-001 s.d AUR-CLI-007.

use aurion::cli::{dispatch, run_cli, CliCommand, OutputFormat};

#[test]
fn test_cli_command_parser_taxonomy() {
    let args = vec!["node".to_string(), "start".to_string(), "--rpc-bind".to_string(), "127.0.0.1:9000".to_string()];
    assert_eq!(
        CliCommand::parse(&args),
        CliCommand::Node(vec!["start".to_string(), "--rpc-bind".to_string(), "127.0.0.1:9000".to_string()])
    );

    let args = vec!["validator".to_string(), "status".to_string()];
    assert_eq!(
        CliCommand::parse(&args),
        CliCommand::Validator(vec!["status".to_string()])
    );

    let args = vec!["wallet".to_string(), "create".to_string(), "--name".to_string(), "alice".to_string()];
    assert_eq!(
        CliCommand::parse(&args),
        CliCommand::Wallet(vec!["create".to_string(), "--name".to_string(), "alice".to_string()])
    );

    let args = vec!["storage".to_string(), "status".to_string()];
    assert_eq!(
        CliCommand::parse(&args),
        CliCommand::Storage(vec!["status".to_string()])
    );

    let args = vec!["genesis".to_string(), "inspect".to_string()];
    assert_eq!(
        CliCommand::parse(&args),
        CliCommand::Genesis(vec!["inspect".to_string()])
    );

    let args = vec!["network".to_string(), "peers".to_string()];
    assert_eq!(
        CliCommand::parse(&args),
        CliCommand::Network(vec!["peers".to_string()])
    );

    let args = vec!["version".to_string()];
    assert_eq!(CliCommand::parse(&args), CliCommand::Version);

    let args = vec!["--version".to_string()];
    assert_eq!(CliCommand::parse(&args), CliCommand::Version);

    let args = vec!["help".to_string()];
    assert_eq!(CliCommand::parse(&args), CliCommand::Help);

    let empty: Vec<String> = vec![];
    assert_eq!(CliCommand::parse(&empty), CliCommand::Help);
}

#[test]
fn test_output_format_parser() {
    let args = vec!["genesis".to_string(), "inspect".to_string(), "--output".to_string(), "json".to_string()];
    let (fmt, filtered) = OutputFormat::parse_and_strip(&args);
    assert_eq!(fmt, OutputFormat::Json);
    assert_eq!(filtered, vec!["genesis".to_string(), "inspect".to_string()]);

    let args = vec!["-o".to_string(), "json".to_string(), "version".to_string()];
    let (fmt, filtered) = OutputFormat::parse_and_strip(&args);
    assert_eq!(fmt, OutputFormat::Json);
    assert_eq!(filtered, vec!["version".to_string()]);

    let args = vec!["storage".to_string(), "status".to_string(), "--format=json".to_string()];
    let (fmt, filtered) = OutputFormat::parse_and_strip(&args);
    assert_eq!(fmt, OutputFormat::Json);
    assert_eq!(filtered, vec!["storage".to_string(), "status".to_string()]);

    let args = vec!["block".to_string(), "latest".to_string()];
    let (fmt, filtered) = OutputFormat::parse_and_strip(&args);
    assert_eq!(fmt, OutputFormat::Text);
    assert_eq!(filtered, vec!["block".to_string(), "latest".to_string()]);
}

#[tokio::test]
async fn test_cli_dispatch_version_text_and_json() {
    // Human readable text
    let res = dispatch(CliCommand::Version, OutputFormat::Text).await;
    assert!(res.is_ok());

    // Machine-readable JSON (AUR-CLI-007)
    let res = dispatch(CliCommand::Version, OutputFormat::Json).await;
    assert!(res.is_ok());
}

#[tokio::test]
async fn test_cli_dispatch_genesis_inspect_and_hash() {
    // Text inspect
    let res = dispatch(CliCommand::Genesis(vec!["inspect".to_string()]), OutputFormat::Text).await;
    assert!(res.is_ok());

    // JSON inspect
    let res = dispatch(CliCommand::Genesis(vec!["inspect".to_string()]), OutputFormat::Json).await;
    assert!(res.is_ok());

    // Hash text
    let res = dispatch(CliCommand::Genesis(vec!["hash".to_string()]), OutputFormat::Text).await;
    assert!(res.is_ok());

    // Hash JSON
    let res = dispatch(CliCommand::Genesis(vec!["hash".to_string()]), OutputFormat::Json).await;
    assert!(res.is_ok());
}

#[tokio::test]
async fn test_cli_dispatch_storage_status() {
    let res = dispatch(CliCommand::Storage(vec!["status".to_string()]), OutputFormat::Text).await;
    assert!(res.is_ok());

    let res = dispatch(CliCommand::Storage(vec!["status".to_string()]), OutputFormat::Json).await;
    assert!(res.is_ok());
}

#[tokio::test]
async fn test_cli_dispatch_account_validation() {
    // Missing address
    let res = dispatch(CliCommand::Account(vec!["balance".to_string()]), OutputFormat::Text).await;
    assert!(res.is_err());

    // Invalid address
    let res = dispatch(
        CliCommand::Account(vec!["balance".to_string(), "invalid_address".to_string()]),
        OutputFormat::Text,
    ).await;
    assert!(res.is_err());

    // Valid 64-hex address
    let hex_addr = "01".repeat(32);
    let res = dispatch(
        CliCommand::Account(vec!["balance".to_string(), hex_addr]),
        OutputFormat::Json,
    ).await;
    assert!(res.is_ok());
}

#[tokio::test]
async fn test_run_cli_entrypoint() {
    let args = vec!["version".to_string(), "--output".to_string(), "json".to_string()];
    let res = run_cli(&args).await;
    assert!(res.is_ok());

    let args = vec!["help".to_string()];
    let res = run_cli(&args).await;
    assert!(res.is_ok());
}
