use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::time::Duration;
use well_ipc::TypedBlock;

#[test]
fn test_well_server_headless_execution() {
    // Determine the path to the cargo binary
    let server_bin = assert_cmd::cargo::cargo_bin("well-server");

    // Spawn the well-server child process
    let mut child = Command::new(server_bin)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to spawn well-server");

    let mut stdin = child.stdin.take().expect("Failed to open stdin");
    let mut stdout = child.stdout.take().expect("Failed to open stdout");

    // Send a command to the well-server
    // well-server creates an ExecutionRouter which delegates to bash or zsh.
    let cmd = b"echo 'headless well-server working'\n";
    stdin.write_all(cmd).expect("Failed to write to stdin");
    stdin.flush().expect("Failed to flush stdin");

    // Give the server time to process the command and push bytes out
    std::thread::sleep(Duration::from_millis(1500));

    // Read the serialized TypedBlock length from stdout (4 bytes)
    let mut len_buf = [0u8; 4];
    match stdout.read_exact(&mut len_buf) {
        Ok(_) => {}
        Err(e) => {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("Failed to read length from stdout. Error: {}", e);
        }
    }

    let payload_len = u32::from_be_bytes(len_buf) as usize;
    if payload_len == 0 || payload_len > 10 * 1024 * 1024 {
        child.kill().unwrap();
        child.wait().unwrap();
        panic!("Invalid payload length received: {}", payload_len);
    }

    // Read the payload
    let mut payload = vec![0u8; payload_len];
    stdout
        .read_exact(&mut payload)
        .expect("Failed to read payload from stdout");

    // Deserialize into TypedBlock
    let block: TypedBlock =
        bincode::deserialize(&payload).expect("Failed to deserialize TypedBlock");

    // Extract the content
    let content = match block {
        TypedBlock::StdoutChunk(data) => String::from_utf8_lossy(&data).into_owned(),
        TypedBlock::StderrChunk(data) => {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!(
                "Expected StdoutChunk, got StderrChunk: {}",
                String::from_utf8_lossy(&data)
            );
        }
        _ => {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("Expected StdoutChunk block");
        }
    };

    assert!(
        content.contains("headless well-server working"),
        "Output did not contain expected string, got: {}",
        content
    );

    // Clean up
    child.kill().expect("Failed to kill child process");
    child.wait().expect("Failed to reap child process");
}
