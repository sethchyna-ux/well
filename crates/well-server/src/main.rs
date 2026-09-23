use tokio::io::{AsyncReadExt, AsyncWriteExt};
use well_ipc::ring_buffer::RingBuffer;
use well_ipc::TypedBlock;
use well_shell::engine::ExecutionRouter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // The headless well-server daemon running on a remote node over SSH
    let ring_buffer = RingBuffer::<TypedBlock, 1024>::new();
    let (producer, mut consumer) = ring_buffer.split();
    let router = ExecutionRouter::new(producer);

    // Read from standard input (SSH channel data sent by the client)
    let mut stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();

    let mut buf = [0u8; 4096];

    // Simple command dispatch loop
    loop {
        let block_opt = consumer.try_pop();

        if let Some(block) = block_opt {
            // Serialize TypedBlocks and stream them over SSH stdout
            let encoded = bincode::serialize(&block)?;
            let len = (encoded.len() as u32).to_be_bytes();
            stdout.write_all(&len).await?;
            stdout.write_all(&encoded).await?;
            stdout.flush().await?;
            continue;
        }

        tokio::select! {
            res = stdin.read(&mut buf) => {
                match res {
                    Ok(0) => break, // EOF
                    Ok(n) => {
                        let cmd_str = String::from_utf8_lossy(&buf[..n]);
                        // Execute the command locally on the server using dual-path ExecutionRouter
                        if let Err(err) = router.execute(&cmd_str, 24, 80).await {
                            eprintln!("well-server command execution failed: {err}");
                        }
                    }
                    Err(_) => break,
                }
            }
            _ = tokio::time::sleep(std::time::Duration::from_millis(1)) => {
                // Yield to allow router tasks to execute and populate the ring buffer
            }
        }
    }

    Ok(())
}
