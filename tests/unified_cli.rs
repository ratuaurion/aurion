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

#[tokio::test]
async fn test_cli_dispatch_contract_deploy_and_inspect() {
    // Valid simple return bytecode: PUSH1 0, RETURN -> 6000F3
    let bytecode_hex = "6000f3".to_string();
    let res = dispatch(
        CliCommand::Contract(vec!["deploy".to_string(), bytecode_hex]),
        OutputFormat::Json,
    ).await;
    assert!(res.is_ok());

    // Inspect dummy contract address
    let hex_addr = "02".repeat(32);
    let res = dispatch(
        CliCommand::Contract(vec!["inspect".to_string(), hex_addr]),
        OutputFormat::Json,
    ).await;
    assert!(res.is_ok());
}

#[tokio::test]
async fn test_cli_dispatch_l2_subcommands_text_and_json() {
    // 1. L2 Node (Text & JSON)
    let res = dispatch(CliCommand::L2(vec!["node".to_string()]), OutputFormat::Text).await;
    assert!(res.is_ok());
    let res = dispatch(CliCommand::L2(vec!["node".to_string()]), OutputFormat::Json).await;
    assert!(res.is_ok());

    // 2. L2 Sequencer (Text & JSON)
    let res = dispatch(CliCommand::L2(vec!["sequencer".to_string()]), OutputFormat::Text).await;
    assert!(res.is_ok());
    let res = dispatch(CliCommand::L2(vec!["sequencer".to_string()]), OutputFormat::Json).await;
    assert!(res.is_ok());

    // 3. L2 Bridge (Text & JSON)
    let res = dispatch(CliCommand::L2(vec!["bridge".to_string()]), OutputFormat::Text).await;
    assert!(res.is_ok());
    let res = dispatch(CliCommand::L2(vec!["bridge".to_string()]), OutputFormat::Json).await;
    assert!(res.is_ok());

    // 4. L2 Tx Simulation (Text & JSON)
    let res = dispatch(
        CliCommand::L2(vec![
            "tx".to_string(),
            "--from".to_string(),
            "aur1000000000000000000000000000000000000000000000000000sqqqqqqqq".to_string(),
            "--to".to_string(),
            "aur1222222222222222222222222222222222222222222222222222sqqqqqqqq".to_string(),
            "--amount-quanta".to_string(),
            "500000000".to_string(),
        ]),
        OutputFormat::Text,
    ).await;
    assert!(res.is_ok());

    let res = dispatch(
        CliCommand::L2(vec![
            "tx".to_string(),
            "--from".to_string(),
            "aur1000000000000000000000000000000000000000000000000000sqqqqqqqq".to_string(),
            "--to".to_string(),
            "aur1222222222222222222222222222222222222222222222222222sqqqqqqqq".to_string(),
            "--amount-quanta".to_string(),
            "500000000".to_string(),
        ]),
        OutputFormat::Json,
    ).await;
    assert!(res.is_ok());

    // 5. L2 Help
    let res = dispatch(CliCommand::L2(vec!["help".to_string()]), OutputFormat::Text).await;
    assert!(res.is_ok());

    // 6. Test entrypoint parsing
    let res = run_cli(&[
        "l2".to_string(),
        "node".to_string(),
        "--output".to_string(),
        "json".to_string(),
    ]).await;
    assert!(res.is_ok());
}

#[tokio::test]
async fn test_cli_dispatch_l4_subcommands_text_and_json() {
    // 1. L4 Relay (Text & JSON)
    let res = dispatch(CliCommand::L4(vec!["relay".to_string()]), OutputFormat::Text).await;
    assert!(res.is_ok());
    let res = dispatch(CliCommand::L4(vec!["relay".to_string()]), OutputFormat::Json).await;
    assert!(res.is_ok());

    // 2. L4 Bridge (Text & JSON)
    let res = dispatch(CliCommand::L4(vec!["bridge".to_string()]), OutputFormat::Text).await;
    assert!(res.is_ok());
    let res = dispatch(CliCommand::L4(vec!["bridge".to_string()]), OutputFormat::Json).await;
    assert!(res.is_ok());

    // 3. L4 Verify (Text & JSON)
    let res = dispatch(CliCommand::L4(vec!["verify".to_string()]), OutputFormat::Text).await;
    assert!(res.is_ok());
    let res = dispatch(CliCommand::L4(vec!["verify".to_string()]), OutputFormat::Json).await;
    assert!(res.is_ok());

    // 4. L4 Circuit (Text & JSON)
    let res = dispatch(CliCommand::L4(vec!["circuit".to_string()]), OutputFormat::Text).await;
    assert!(res.is_ok());
    let res = dispatch(CliCommand::L4(vec!["circuit".to_string()]), OutputFormat::Json).await;
    assert!(res.is_ok());

    // 5. L4 Status (Text & JSON)
    let res = dispatch(CliCommand::L4(vec!["status".to_string()]), OutputFormat::Text).await;
    assert!(res.is_ok());
    let res = dispatch(CliCommand::L4(vec!["status".to_string()]), OutputFormat::Json).await;
    assert!(res.is_ok());

    // 6. L4 Help
    let res = dispatch(CliCommand::L4(vec!["help".to_string()]), OutputFormat::Text).await;
    assert!(res.is_ok());

    // 7. Test entrypoint parsing via run_cli with "interop" and "l4" alias
    let res = run_cli(&[
        "l4".to_string(),
        "status".to_string(),
        "--output".to_string(),
        "json".to_string(),
    ]).await;
    assert!(res.is_ok());

    let res = run_cli(&[
        "interop".to_string(),
        "relay".to_string(),
    ]).await;
    assert!(res.is_ok());
}

#[tokio::test]
async fn test_cli_dispatch_l5_subcommands_text_and_json() {
    // 1. L5 Node (Text & JSON)
    let res = dispatch(CliCommand::L5(vec!["node".to_string()]), OutputFormat::Text).await;
    assert!(res.is_ok());
    let res = dispatch(CliCommand::L5(vec!["node".to_string()]), OutputFormat::Json).await;
    assert!(res.is_ok());

    // 2. L5 Compute (Text & JSON)
    let res = dispatch(CliCommand::L5(vec!["compute".to_string()]), OutputFormat::Text).await;
    assert!(res.is_ok());
    let res = dispatch(CliCommand::L5(vec!["compute".to_string()]), OutputFormat::Json).await;
    assert!(res.is_ok());

    // 3. L5 Storage (Text & JSON)
    let res = dispatch(CliCommand::L5(vec!["storage".to_string()]), OutputFormat::Text).await;
    assert!(res.is_ok());
    let res = dispatch(CliCommand::L5(vec!["storage".to_string()]), OutputFormat::Json).await;
    assert!(res.is_ok());

    // 4. L5 DA (Text & JSON)
    let res = dispatch(CliCommand::L5(vec!["da".to_string()]), OutputFormat::Text).await;
    assert!(res.is_ok());
    let res = dispatch(CliCommand::L5(vec!["da".to_string()]), OutputFormat::Json).await;
    assert!(res.is_ok());

    // 5. L5 Pay (Text & JSON)
    let res = dispatch(CliCommand::L5(vec!["pay".to_string()]), OutputFormat::Text).await;
    assert!(res.is_ok());
    let res = dispatch(CliCommand::L5(vec!["pay".to_string()]), OutputFormat::Json).await;
    assert!(res.is_ok());

    // 6. L5 Agent (Text & JSON)
    let res = dispatch(CliCommand::L5(vec!["agent".to_string()]), OutputFormat::Text).await;
    assert!(res.is_ok());
    let res = dispatch(CliCommand::L5(vec!["agent".to_string()]), OutputFormat::Json).await;
    assert!(res.is_ok());

    // 7. L5 Status (Text & JSON)
    let res = dispatch(CliCommand::L5(vec!["status".to_string()]), OutputFormat::Text).await;
    assert!(res.is_ok());
    let res = dispatch(CliCommand::L5(vec!["status".to_string()]), OutputFormat::Json).await;
    assert!(res.is_ok());

    // 8. L5 Help
    let res = dispatch(CliCommand::L5(vec!["help".to_string()]), OutputFormat::Text).await;
    assert!(res.is_ok());

    // 9. Test entrypoint parsing via run_cli with "infra" and "l5" alias
    let res = run_cli(&[
        "l5".to_string(),
        "status".to_string(),
        "--output".to_string(),
        "json".to_string(),
    ]).await;
    assert!(res.is_ok());

    let res = run_cli(&[
        "infra".to_string(),
        "node".to_string(),
    ]).await;
    assert!(res.is_ok());
}

#[tokio::test]
async fn test_cli_dispatch_devnet_subcommands_text_and_json() {
    // 1. Devnet Init (Text & JSON)
    let res = dispatch(CliCommand::Devnet(vec!["init".to_string()]), OutputFormat::Text).await;
    assert!(res.is_ok());
    let res = dispatch(CliCommand::Devnet(vec!["init".to_string()]), OutputFormat::Json).await;
    assert!(res.is_ok());

    // 2. Devnet Status (Text & JSON)
    let res = dispatch(CliCommand::Devnet(vec!["status".to_string()]), OutputFormat::Text).await;
    assert!(res.is_ok());
    let res = dispatch(CliCommand::Devnet(vec!["status".to_string()]), OutputFormat::Json).await;
    assert!(res.is_ok());

    // 3. Devnet Help
    let res = dispatch(CliCommand::Devnet(vec!["help".to_string()]), OutputFormat::Text).await;
    assert!(res.is_ok());

    // 4. CLI entrypoint parsing for devnet
    assert_eq!(
        CliCommand::parse(&["devnet".to_string(), "init".to_string()]),
        CliCommand::Devnet(vec!["init".to_string()])
    );

    let res = run_cli(&[
        "devnet".to_string(),
        "status".to_string(),
        "--output".to_string(),
        "json".to_string(),
    ]).await;
    assert!(res.is_ok());
}
