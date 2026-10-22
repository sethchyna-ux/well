//! Structured process output with bounded backpressure and owned PTY sessions.
use crate::keyboard::KeyEncoder;
use crate::pty::{PtySession, ScreenSnapshot};
use std::io;
use std::path::Path;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::Command;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
struct NativeJob {
    task: JoinHandle<()>,
    stop: CancellationToken,
}
use well_ipc::ring_buffer::Producer;
use well_ipc::{KittyKeyboardInput, TypedBlock};

type Error = Box<dyn std::error::Error + Send + Sync>;

pub struct ExecutionRouter<const N: usize> {
    pub producer: Arc<Producer<TypedBlock, N>>,
    interactive: Mutex<Option<PtySession>>,
    interactive_bridge: Mutex<Option<JoinHandle<()>>>,
    interactive_cancel: Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
    jobs: Mutex<Vec<NativeJob>>,
    serial: Arc<tokio::sync::Mutex<()>>,
}
impl<const N: usize> ExecutionRouter<N> {
    pub fn new(producer: Producer<TypedBlock, N>) -> Self {
        Self {
            producer: Arc::new(producer),
            interactive: Mutex::new(None),
            interactive_bridge: Mutex::new(None),
            interactive_cancel: Mutex::new(None),
            jobs: Mutex::new(Vec::new()),
            serial: Arc::new(tokio::sync::Mutex::new(())),
        }
    }
    pub fn handle_input(
        &self,
        input: KittyKeyboardInput,
        pty: Option<&mut PtySession>,
    ) -> Result<(), Error> {
        let Some(bytes) = KeyEncoder::encode_legacy(&input) else {
            return Ok(());
        };
        if let Some(pty) = pty {
            pty.write_all(&bytes)?;
        } else if let Some(pty) = &*self.interactive.lock().unwrap_or_else(|p| p.into_inner()) {
            pty.write_all(&bytes)?;
        } else {
            return Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "No active interactive session",
            )
            .into());
        }
        Ok(())
    }
    /// Compatibility classification; callers can select either execution mode explicitly.
    pub fn is_interactive(cmd: &str) -> bool {
        let Ok(parts) = shell_words::split(cmd) else {
            return false;
        };
        let Some(program) = parts
            .first()
            .and_then(|s| Path::new(s).file_name())
            .and_then(|s| s.to_str())
        else {
            return false;
        };
        matches!(
            program,
            "htop"
                | "top"
                | "less"
                | "more"
                | "ssh"
                | "vim"
                | "vi"
                | "nvim"
                | "nano"
                | "tmux"
                | "screen"
        )
    }
    pub async fn execute(&self, cmd: &str, rows: u16, cols: u16) -> Result<bool, Error> {
        if Self::is_interactive(cmd) {
            self.execute_interactive(cmd, rows, cols)?;
            Ok(true)
        } else {
            self.execute_native(cmd).await?;
            Ok(false)
        }
    }
    pub fn execute_interactive(&self, cmd: &str, rows: u16, cols: u16) -> Result<(), Error> {
        self.start_interactive(cmd, rows, cols, None, true)
    }
    fn start_interactive(
        &self,
        cmd: &str,
        rows: u16,
        cols: u16,
        shell: Option<&str>,
        login: bool,
    ) -> Result<(), Error> {
        let _serial = self.serial.try_lock().map_err(|_| {
            io::Error::new(io::ErrorKind::WouldBlock, "A native batch is still running")
        })?;
        let runtime = tokio::runtime::Handle::try_current()?;

        let mut active = self.interactive.lock().unwrap_or_else(|p| p.into_inner());
        if active.as_ref().is_some_and(PtySession::is_alive) {
            active
                .as_ref()
                .unwrap()
                .write_all(format!("{cmd}\r").as_bytes())?;
            return Ok(());
        }
        if let Some(bridge) = self
            .interactive_bridge
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .take()
        {
            bridge.abort();
        }
        self.interactive_cancel
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .take();
        active.take();
        let (tx, mut rx) = tokio::sync::mpsc::channel(32);
        let (cancel, mut cancelled_rx) = tokio::sync::oneshot::channel();
        let callback_runtime = runtime.clone();
        let mut cancelled = false;
        let session = PtySession::start(
            rows,
            cols,
            shell,
            login,
            move |bytes| {
                if cancelled {
                    return;
                }
                // Block only the PTY worker; cancellation can wake it even when
                // the runtime's UI thread is synchronously joining this session.
                cancelled = callback_runtime.block_on(async {
                    tokio::select! {
                        _ = &mut cancelled_rx => true,
                        result = tx.send(bytes.to_vec()) => result.is_err(),
                    }
                });
            },
            |_| {},
        )?;
        let producer = Arc::clone(&self.producer);
        let bridge = runtime.spawn(async move {
            while let Some(bytes) = rx.recv().await {
                if producer.push(TypedBlock::StdoutChunk(bytes)).await.is_err() {
                    break;
                }
            }
        });
        if let Err(error) = session.write_all(format!("{cmd}\r").as_bytes()) {
            let _ = cancel.send(());
            bridge.abort();
            return Err(error.into());
        }
        *self
            .interactive_cancel
            .lock()
            .unwrap_or_else(|p| p.into_inner()) = Some(cancel);
        *self
            .interactive_bridge
            .lock()
            .unwrap_or_else(|p| p.into_inner()) = Some(bridge);
        *active = Some(session);
        Ok(())
    }
    pub fn interactive_snapshot(&self) -> Option<ScreenSnapshot> {
        self.interactive
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .as_ref()
            .map(PtySession::snapshot)
    }
    pub fn resize_interactive(&self, rows: u16, cols: u16) -> Result<(), Error> {
        if let Some(session) = &*self.interactive.lock().unwrap_or_else(|p| p.into_inner()) {
            session.resize(rows, cols)?;
        }
        Ok(())
    }
    pub fn close_interactive(&self) {
        if let Some(cancel) = self
            .interactive_cancel
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .take()
        {
            let _ = cancel.send(());
        }
        if let Some(bridge) = self
            .interactive_bridge
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .take()
        {
            bridge.abort();
        }
        self.interactive
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .take();
    }
    /// Run POSIX shell syntax with byte streams transported through Hermes.
    /// Native batches are serialized because TypedBlock identifies a single stream.
    pub async fn execute_native(&self, text: &str) -> Result<(), Error> {
        if text.trim().is_empty() {
            return Ok(());
        }
        #[cfg(unix)]
        let command = {
            let mut command = Command::new("/bin/sh");
            command.arg("-c").arg(text);
            command
        };
        #[cfg(windows)]
        let command = {
            let mut command = Command::new("cmd.exe");
            command.arg("/D").arg("/S").arg("/C").arg(text);
            command
        };
        self.start_native(command).await
    }
    /// Deny network access; unsupported systems fail rather than run unsandboxed.
    pub async fn execute_sandboxed(&self, cmd: &str, args: &[String]) -> Result<(), Error> {
        #[cfg(target_os = "macos")]
        let mut command = {
            let mut c = Command::new("/usr/bin/sandbox-exec");
            c.arg("-p")
                .arg("(version 1) (allow default) (deny network*)")
                .arg(cmd);
            c
        };
        #[cfg(target_os = "linux")]
        let mut command = {
            let mut c = Command::new("unshare");
            c.arg("--net").arg("--").arg(cmd);
            c
        };
        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Network-isolated execution is unsupported on this platform",
        )
        .into());
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        {
            command.args(args);
            self.start_native(command).await
        }
    }
    async fn start_native(&self, mut command: Command) -> Result<(), Error> {
        let serial = Arc::clone(&self.serial).lock_owned().await;
        if self
            .interactive
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .is_some()
        {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "Close the interactive stream before starting a native batch",
            )
            .into());
        }
        command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        #[cfg(unix)]
        command.process_group(0);
        let mut child = command.spawn()?;
        #[cfg(unix)]
        let mut group = ProcessGroup(child.id().map(|pid| pid as i32));
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| io::Error::other("Missing stdout pipe"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| io::Error::other("Missing stderr pipe"))?;
        let producer = Arc::clone(&self.producer);
        let stop = CancellationToken::new();
        let cancelled = stop.clone();
        let job = tokio::spawn(async move {
            let _serial = serial;
            let outcome = tokio::select! {
                _ = cancelled.cancelled() => None,
                result = async {
                    tokio::try_join!(stream(stdout, &producer, false), stream(stderr, &producer, true))?;
                    child.wait().await.map(status_code)
                } => Some(result),
            };
            let code = match outcome {
                Some(Ok(code)) => {
                    #[cfg(unix)]
                    group.disarm();
                    code
                }
                outcome => {
                    #[cfg(unix)]
                    group.terminate();
                    let _ = child.kill().await;
                    let _ = child.wait().await;
                    let Some(Err(error)) = outcome else {
                        return;
                    };
                    if error.kind() != io::ErrorKind::BrokenPipe {
                        tokio::select! {
                            _ = cancelled.cancelled() => return,
                            _ = producer.push(TypedBlock::StderrChunk(format!("process: {error}\n").into_bytes())) => {},
                        }
                    }
                    -1
                }
            };
            tokio::select! {
                _ = cancelled.cancelled() => {},
                _ = producer.push(TypedBlock::ExitStatus(code)) => {},
            }
        });
        let mut jobs = self.jobs.lock().unwrap_or_else(|p| p.into_inner());
        jobs.retain(|job| !job.task.is_finished());
        jobs.push(NativeJob { task: job, stop });
        Ok(())
    }
    pub async fn shutdown(&self) {
        self.close_interactive();
        let jobs = std::mem::take(&mut *self.jobs.lock().unwrap_or_else(|p| p.into_inner()));
        for job in &jobs {
            job.stop.cancel();
        }
        for job in jobs {
            let _ = job.task.await;
        }
    }
}
impl<const N: usize> Drop for ExecutionRouter<N> {
    fn drop(&mut self) {
        self.close_interactive();
        for job in self.jobs.get_mut().unwrap_or_else(|p| p.into_inner()) {
            job.stop.cancel();
        }
    }
}
async fn stream<R: AsyncRead + Unpin, const N: usize>(
    mut reader: R,
    producer: &Producer<TypedBlock, N>,
    stderr: bool,
) -> io::Result<()> {
    let mut bytes = [0; 16384];
    loop {
        let n = reader.read(&mut bytes).await?;
        if n == 0 {
            return Ok(());
        }
        let chunk = if stderr {
            TypedBlock::StderrChunk(bytes[..n].to_vec())
        } else {
            TypedBlock::StdoutChunk(bytes[..n].to_vec())
        };
        producer.push(chunk).await.map_err(|_| {
            io::Error::new(io::ErrorKind::BrokenPipe, "Output consumer disconnected")
        })?;
    }
}
fn status_code(status: std::process::ExitStatus) -> i32 {
    if let Some(code) = status.code() {
        return code;
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        return 128 + status.signal().unwrap_or(0);
    }
    #[cfg(not(unix))]
    -1
}
#[cfg(unix)]
struct ProcessGroup(Option<i32>);
#[cfg(unix)]
impl ProcessGroup {
    fn disarm(&mut self) {
        self.0 = None;
    }
    fn terminate(&mut self) {
        if let Some(pid) = self.0.take() {
            // Command::process_group(0) creates this group for this job alone.
            if pid > 1 {
                unsafe {
                    libc::kill(-pid, libc::SIGKILL);
                }
            }
        }
    }
}
#[cfg(unix)]
impl Drop for ProcessGroup {
    fn drop(&mut self) {
        self.terminate();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use well_ipc::ring_buffer::RingBuffer;
    #[tokio::test]
    async fn backpressure_preserves_both_streams_and_exit_order() {
        let ring = RingBuffer::<TypedBlock, 1>::new();
        let (producer, consumer) = ring.split();
        let router = ExecutionRouter::new(producer);
        router.execute_native("printf '%s\\n' 'a b' | tr ' ' '_'; i=0; while [ $i -lt 2500 ]; do printf 'out-'; printf 'err-' >&2; i=$((i+1)); done; exit 7").await.unwrap();
        let mut out = Vec::new();
        let mut err = Vec::new();
        let collect = async {
            loop {
                match consumer.pop().await.unwrap() {
                    TypedBlock::StdoutChunk(bytes) => out.extend(bytes),
                    TypedBlock::StderrChunk(bytes) => err.extend(bytes),
                    TypedBlock::ExitStatus(code) => {
                        assert_eq!(code, 7);
                        break;
                    }
                }
            }
        };
        tokio::time::timeout(Duration::from_secs(5), collect)
            .await
            .unwrap();
        assert!(out.starts_with(b"a_b\n"));
        assert_eq!(out.len(), 4 + 2500 * 4);
        assert_eq!(err, b"err-".repeat(2500));
        assert!(consumer.try_pop().is_none());
        router.shutdown().await;
    }
    #[tokio::test]
    async fn receiver_disconnect_does_not_hang_job_shutdown() {
        let (producer, consumer) = RingBuffer::<TypedBlock, 1>::new().split();
        let router = ExecutionRouter::new(producer);
        router
            .execute_native("while :; do printf 'lots of output'; done")
            .await
            .unwrap();
        drop(consumer);
        tokio::time::timeout(Duration::from_secs(2), router.shutdown())
            .await
            .unwrap();
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn shutdown_waits_for_native_child_reaping() {
        let (producer, consumer) = RingBuffer::<TypedBlock, 2>::new().split();
        let router = ExecutionRouter::new(producer);
        router
            .execute_native("printf '%s\\n' $$; sleep 30")
            .await
            .unwrap();
        let Some(TypedBlock::StdoutChunk(bytes)) =
            tokio::time::timeout(Duration::from_secs(2), consumer.pop())
                .await
                .unwrap()
        else {
            panic!("missing child PID");
        };
        let pid: i32 = std::str::from_utf8(&bytes).unwrap().trim().parse().unwrap();
        tokio::time::timeout(Duration::from_secs(2), router.shutdown())
            .await
            .unwrap();
        assert_eq!(
            unsafe { libc::kill(pid, 0) },
            -1,
            "native child was not reaped"
        );
    }
    #[test]
    fn detects_program_basename_after_quote_parsing() {
        assert!(ExecutionRouter::<8>::is_interactive(
            "'/usr/bin/vim' -Nu NONE"
        ));
        assert!(ExecutionRouter::<8>::is_interactive("ssh host"));
        assert!(!ExecutionRouter::<8>::is_interactive("printf x | cat"));
        assert!(!ExecutionRouter::<8>::is_interactive("'unfinished"));
    }
    #[tokio::test]
    async fn interactive_session_remains_owned_and_can_be_closed_under_backpressure() {
        let (producer, _consumer) = RingBuffer::<TypedBlock, 1>::new().split();
        let router = ExecutionRouter::new(producer);
        router
            .start_interactive(
                "while :; do printf 'abcdefghijklmnopqrstuvwxyz'; done",
                24,
                80,
                Some("/bin/sh"),
                false,
            )
            .unwrap();
        assert!(router.interactive_snapshot().is_some());
        router.resize_interactive(30, 100).unwrap();
        assert_eq!(router.interactive_snapshot().unwrap().cols, 100);
        tokio::time::sleep(Duration::from_millis(50)).await;
        tokio::time::timeout(Duration::from_secs(2), router.shutdown())
            .await
            .unwrap();
        assert!(router.interactive_snapshot().is_none());
    }
}
