//! Asynchronous shell service backed by one persistent, owned PTY session.
use crate::{MetisHistory, PtySession};
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, Mutex};
use tokio::task::JoinHandle;

type Error = Box<dyn std::error::Error + Send + Sync>;

#[derive(Debug)]
pub struct ShellHandle {
    pub history: Arc<Mutex<MetisHistory>>,
    pub tx: mpsc::Sender<String>,
    /// Real terminal bytes, preserving UTF-8 fragments and control sequences.
    pub rx: mpsc::Receiver<Vec<u8>>,
    worker: JoinHandle<()>,
}
impl ShellHandle {
    pub async fn send_command(&self, command: String) -> Result<(), String> {
        self.tx.send(command).await.map_err(|e| e.to_string())
    }
    pub async fn suggest(&self, prefix: &str) -> Vec<String> {
        self.history.lock().await.suggestions(prefix, 5)
    }
    pub async fn shutdown(self) {
        self.worker.abort();
        // Await through the handle by reference because ShellHandle owns Drop.
        let mut this = self;
        let _ = (&mut this.worker).await;
    }
}
impl Drop for ShellHandle {
    fn drop(&mut self) {
        self.worker.abort();
    }
}

struct OwnedShell {
    session: PtySession,
    cancel: Option<oneshot::Sender<()>>,
}
impl Drop for OwnedShell {
    fn drop(&mut self) {
        if let Some(cancel) = self.cancel.take() {
            let _ = cancel.send(());
        }
        self.session.shutdown();
    }
}

pub fn init_shell() -> Result<ShellHandle, Error> {
    init_with_shell(None, true)
}
fn init_with_shell(shell: Option<&str>, login: bool) -> Result<ShellHandle, Error> {
    let runtime = tokio::runtime::Handle::try_current()?;
    let callback_runtime = runtime.clone();
    let (tx, mut commands) = mpsc::channel::<String>(32);
    let (output, rx) = mpsc::channel::<Vec<u8>>(32);
    let (cancel, mut cancelled) = oneshot::channel();
    let mut closed = false;
    let session = PtySession::start(
        24,
        80,
        shell,
        login,
        move |bytes| {
            if closed {
                return;
            }
            closed = callback_runtime.block_on(async {
                tokio::select! {
                    _ = &mut cancelled => true,
                    result = output.send(bytes.to_vec()) => result.is_err(),
                }
            });
        },
        |_| {},
    )?;
    let shell = OwnedShell {
        session,
        cancel: Some(cancel),
    };
    let history = Arc::new(Mutex::new(MetisHistory::new()));
    let worker_history = history.clone();
    let worker = runtime.spawn(async move {
        let shell = shell;
        while let Some(command) = commands.recv().await {
            worker_history.lock().await.learn_command(&command);
            if shell
                .session
                .write_all(format!("{command}\r").as_bytes())
                .is_err()
            {
                break;
            }
        }
    });
    Ok(ShellHandle {
        history,
        tx,
        rx,
        worker,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    #[tokio::test]
    async fn executes_real_commands_and_preserves_directory_state() {
        let mut shell = init_with_shell(Some("/bin/sh"), false).unwrap();
        shell
            .send_command(
                "stty -echo; PS1=''; cd /tmp; printf '\\137ACTUAL\\137:%s\\n' \"$PWD\"".into(),
            )
            .await
            .unwrap();
        let output = tokio::time::timeout(Duration::from_secs(3), async {
            let mut output = Vec::new();
            while let Some(bytes) = shell.rx.recv().await {
                output.extend(bytes);
                if String::from_utf8_lossy(&output).contains("_ACTUAL_:/tmp") {
                    break;
                }
            }
            output
        })
        .await
        .unwrap();
        assert!(String::from_utf8_lossy(&output).contains("_ACTUAL_:/tmp"));
        shell
            .send_command("printf '\\137SECOND\\137:%s\\n' \"$PWD\"".into())
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_secs(3), async {
            let mut output = Vec::new();
            while let Some(bytes) = shell.rx.recv().await {
                output.extend(bytes);
                if String::from_utf8_lossy(&output).contains("_SECOND_:/tmp") {
                    return;
                }
            }
            panic!("shell output ended before the second command");
        })
        .await
        .unwrap();
        assert!(!shell.suggest("printf").await.is_empty());
        shell.shutdown().await;
    }
    #[tokio::test]
    async fn full_output_queue_can_be_cancelled() {
        let shell = init_with_shell(Some("/bin/sh"), false).unwrap();
        shell
            .send_command("while :; do printf 'abcdefghijklmnopqrstuvwxyz'; done".into())
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;
        tokio::time::timeout(Duration::from_secs(2), shell.shutdown())
            .await
            .unwrap();
    }
}
