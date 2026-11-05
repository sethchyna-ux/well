//! Integration tests for headless `well-cli` binary execution.

use std::process::Command;

fn well_cli_bin() -> &'static str {
    env!("CARGO_BIN_EXE_well-cli")
}

#[test]
fn test_cli_help_flag_exits_zero_and_prints_usage() {
    let out = Command::new(well_cli_bin())
        .arg("--help")
        .output()
        .expect("Failed to execute well-cli --help");
    assert!(out.status.success(), "Expected exit code 0 for --help");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("USAGE:"));
    assert!(stdout.contains("translate"));
}

#[test]
fn test_cli_version_flag_exits_zero() {
    let out = Command::new(well_cli_bin())
        .arg("--version")
        .output()
        .expect("Failed to execute well-cli --version");
    assert!(out.status.success(), "Expected exit code 0 for --version");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.starts_with("well-cli "));
}

#[test]
fn test_cli_translate_list_files_with_mock_llm() {
    let out = Command::new(well_cli_bin())
        .env("WELL_MOCK_LLM", "1")
        .args(["translate", "list files"])
        .output()
        .expect("Failed to execute well-cli translate");
    assert!(out.status.success(), "Expected exit code 0 for translate");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.trim().starts_with("ls"),
        "Expected ls command, got: {stdout}"
    );
}

#[test]
fn test_cli_translate_empty_query_returns_nonzero_exit() {
    let out = Command::new(well_cli_bin())
        .arg("translate")
        .output()
        .expect("Failed to execute well-cli translate with empty query");
    assert!(
        !out.status.success(),
        "Expected non-zero exit code when translate query is missing"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("Usage: well-cli translate"));
}

