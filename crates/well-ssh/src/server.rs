//! Authenticated SSH terminal hosting, isolated from the system SSH daemon.
use portable_pty::{CommandBuilder, MasterPty, PtySize};
use russh::{
    server::{self, Handler, Server, Session},
    Channel, ChannelId, Pty,
};
use std::{
    collections::HashMap,
    io::{Read, Write},
    net::SocketAddr,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};
use tokio::sync::oneshot;

type Error = Box<dyn std::error::Error + Send + Sync>;
#[derive(Clone, Debug)]
pub struct HostConfig {
    pub address: SocketAddr,
    pub public_key_auth: bool,
    pub password_auth: bool,
    pub home: PathBuf,
    pub user: String,
}
#[derive(Debug)]
pub struct SshHost {
    stop: Mutex<Option<oneshot::Sender<()>>>,
    pub running: AtomicBool,
    pub message: Mutex<String>,
}
impl SshHost {
    pub fn start(config: HostConfig) -> Arc<Self> {
        let (stop_tx, stop_rx) = oneshot::channel();
        let state = Arc::new(Self {
            stop: Mutex::new(Some(stop_tx)),
            running: AtomicBool::new(false),
            message: Mutex::new("Starting SSH host…".into()),
        });
        let weak = Arc::downgrade(&state);
        std::thread::spawn(move || {
            let result = (|| -> Result<(), Error> {
                let runtime = tokio::runtime::Runtime::new()?;
                let weak = weak.clone();
                runtime.block_on(async move {
                    let user = config.user;
                    let home = config.home;
                    let authorized = if config.public_key_auth {
                        std::fs::read_to_string(home.join(".ssh/authorized_keys"))?
                            .lines()
                            .filter_map(|line| russh::keys::PublicKey::from_openssh(line).ok())
                            .collect()
                    } else {
                        Vec::new()
                    };
                    let password = if config.password_auth {
                        Some(std::env::var("WELL_SSH_PASSWORD").map_err(|_| {
                            "Password auth requires WELL_SSH_PASSWORD in the launching environment"
                        })?)
                    } else {
                        None
                    };
                    if authorized.is_empty() && password.is_none() {
                        return Err("No authorized keys or password configured".into());
                    }
                    let dir = home.join(".local/state/well/ssh-host");
                    std::fs::create_dir_all(&dir)?;
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::PermissionsExt;
                        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700))?;
                    }
                    let key_path = dir.join("host_ed25519");
                    if !key_path.exists() {
                        let status = std::process::Command::new("ssh-keygen")
                            .args(["-q", "-t", "ed25519", "-N", "", "-f"])
                            .arg(&key_path)
                            .stdin(std::process::Stdio::null())
                            .status()?;
                        if !status.success() {
                            return Err("Host key generation failed".into());
                        }
                    }
                    let key = russh::keys::load_secret_key(&key_path, None)?;
                    let fingerprint = key
                        .public_key()
                        .fingerprint(russh::keys::HashAlg::Sha256)
                        .to_string();
                    let conf = Arc::new(server::Config {
                        keys: vec![key],
                        auth_rejection_time: std::time::Duration::from_secs(1),
                        inactivity_timeout: Some(std::time::Duration::from_secs(3600)),
                        ..Default::default()
                    });
                    let listener = tokio::net::TcpListener::bind(config.address).await?;
                    let password = password.map(|p| Password::new(&p));
                    let mut factory = Factory {
                        auth: Arc::new(Auth {
                            user,
                            authorized,
                            password,
                            home,
                        }),
                    };
                    if let Some(state) = weak.upgrade() {
                        state.running.store(true, Ordering::Release);
                        *state.message.lock().unwrap() = format!(
                            "SSH listening on {} · {fingerprint}",
                            listener.local_addr()?
                        );
                    }
                    let running = factory.run_on_socket(conf, &listener);
                    tokio::select! {result=running=>{result?;},_=stop_rx=>{}}
                    Ok(())
                })
            })();
            if let Some(state) = weak.upgrade() {
                state.running.store(false, Ordering::Release);
                *state.message.lock().unwrap() = match result {
                    Ok(()) => "SSH host stopped".into(),
                    Err(e) => format!("SSH host failed: {e}"),
                };
            }
        });
        state
    }
    pub fn stop(&self) {
        if let Some(tx) = self.stop.lock().unwrap().take() {
            let _ = tx.send(());
        }
    }
}
impl Drop for SshHost {
    fn drop(&mut self) {
        self.stop();
    }
}
struct Password {
    salt: [u8; 16],
    hash: [u8; 32],
}
impl Password {
    fn new(value: &str) -> Self {
        use ring::{
            pbkdf2,
            rand::{SecureRandom, SystemRandom},
        };
        let mut p = Self {
            salt: [0; 16],
            hash: [0; 32],
        };
        SystemRandom::new()
            .fill(&mut p.salt)
            .expect("OS randomness");
        pbkdf2::derive(
            pbkdf2::PBKDF2_HMAC_SHA256,
            std::num::NonZeroU32::new(100_000).unwrap(),
            &p.salt,
            value.as_bytes(),
            &mut p.hash,
        );
        p
    }
    fn matches(&self, value: &str) -> bool {
        ring::pbkdf2::verify(
            ring::pbkdf2::PBKDF2_HMAC_SHA256,
            std::num::NonZeroU32::new(100_000).unwrap(),
            &self.salt,
            value.as_bytes(),
            &self.hash,
        )
        .is_ok()
    }
}
struct Auth {
    user: String,
    authorized: Vec<russh::keys::PublicKey>,
    password: Option<Password>,
    home: PathBuf,
}
struct Factory {
    auth: Arc<Auth>,
}
impl Server for Factory {
    type Handler = TerminalHandler;
    fn new_client(&mut self, _: Option<SocketAddr>) -> Self::Handler {
        TerminalHandler {
            auth: self.auth.clone(),
            channels: HashMap::new(),
            size: PtySize::default(),
        }
    }
}
struct Terminal {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    killer: Box<dyn portable_pty::ChildKiller + Send + Sync>,
}
impl Drop for Terminal {
    fn drop(&mut self) {
        let _ = self.killer.kill();
    }
}
struct TerminalHandler {
    auth: Arc<Auth>,
    channels: HashMap<ChannelId, Terminal>,
    size: PtySize,
}
impl TerminalHandler {
    fn start_shell(
        &mut self,
        id: ChannelId,
        command: Option<&[u8]>,
        session: &mut Session,
    ) -> Result<(), Error> {
        if self.channels.contains_key(&id) {
            session.channel_failure(id)?;
            return Ok(());
        }
        let pair = portable_pty::native_pty_system().openpty(self.size)?;
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into());
        let mut cmd = CommandBuilder::new(shell);
        if let Some(command) = command {
            cmd.arg("-c");
            cmd.arg(std::str::from_utf8(command)?);
        } else {
            cmd.arg("-l");
        }
        cmd.cwd(&self.auth.home);
        cmd.env("TERM", "xterm-256color");
        cmd.env("HOME", &self.auth.home);
        cmd.env_remove("WELL_SSH_PASSWORD");
        let mut child = pair.slave.spawn_command(cmd)?;
        let killer = child.clone_killer();
        drop(pair.slave);
        let mut reader = pair.master.try_clone_reader()?;
        let writer = pair.master.take_writer()?;
        let handle = session.handle();
        let (tx, mut rx) = tokio::sync::mpsc::channel::<Option<Vec<u8>>>(16);
        let (exit_tx, exit_rx) = oneshot::channel();
        std::thread::spawn(move || {
            let mut data = [0; 8192];
            loop {
                match reader.read(&mut data) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        if tx.blocking_send(Some(data[..n].to_vec())).is_err() {
                            break;
                        }
                    }
                }
            }
            let _ = exit_tx.send(child.wait().map(|s| s.exit_code()).unwrap_or(1));
        });
        tokio::spawn(async move {
            while let Some(data) = rx.recv().await {
                if let Some(data) = data {
                    if handle.data(id, data).await.is_err() {
                        break;
                    }
                }
            }
            let code = exit_rx.await.unwrap_or(1);
            let _ = handle.exit_status_request(id, code).await;
            let _ = handle.eof(id).await;
            let _ = handle.close(id).await;
        });
        self.channels.insert(
            id,
            Terminal {
                master: pair.master,
                writer,
                killer,
            },
        );
        session.channel_success(id)?;
        Ok(())
    }
}
impl Handler for TerminalHandler {
    type Error = Error;
    async fn auth_publickey(
        &mut self,
        user: &str,
        key: &russh::keys::PublicKey,
    ) -> Result<server::Auth, Error> {
        Ok(
            if user == self.auth.user
                && self
                    .auth
                    .authorized
                    .iter()
                    .any(|allowed| allowed.key_data() == key.key_data())
            {
                server::Auth::Accept
            } else {
                server::Auth::reject()
            },
        )
    }
    async fn auth_password(&mut self, user: &str, password: &str) -> Result<server::Auth, Error> {
        Ok(
            if user == self.auth.user
                && self
                    .auth
                    .password
                    .as_ref()
                    .is_some_and(|p| p.matches(password))
            {
                server::Auth::Accept
            } else {
                server::Auth::reject()
            },
        )
    }
    async fn channel_open_session(
        &mut self,
        _: Channel<server::Msg>,
        reply: server::ChannelOpenHandle,
        _: &mut Session,
    ) -> Result<(), Error> {
        reply.accept().await;
        Ok(())
    }
    async fn pty_request(
        &mut self,
        id: ChannelId,
        _: &str,
        cols: u32,
        rows: u32,
        _: u32,
        _: u32,
        _: &[(Pty, u32)],
        session: &mut Session,
    ) -> Result<(), Error> {
        self.size = PtySize {
            rows: rows.clamp(1, 1000) as u16,
            cols: cols.clamp(1, 1000) as u16,
            pixel_width: 0,
            pixel_height: 0,
        };
        session.channel_success(id)?;
        Ok(())
    }
    async fn shell_request(&mut self, id: ChannelId, session: &mut Session) -> Result<(), Error> {
        self.start_shell(id, None, session)
    }
    async fn exec_request(
        &mut self,
        id: ChannelId,
        data: &[u8],
        session: &mut Session,
    ) -> Result<(), Error> {
        self.start_shell(id, Some(data), session)
    }
    async fn window_change_request(
        &mut self,
        id: ChannelId,
        cols: u32,
        rows: u32,
        _: u32,
        _: u32,
        _: &mut Session,
    ) -> Result<(), Error> {
        if let Some(pty) = self.channels.get(&id) {
            pty.master.resize(PtySize {
                rows: rows.clamp(1, 1000) as u16,
                cols: cols.clamp(1, 1000) as u16,
                pixel_width: 0,
                pixel_height: 0,
            })?;
        }
        Ok(())
    }
    async fn data(&mut self, id: ChannelId, data: &[u8], _: &mut Session) -> Result<(), Error> {
        if let Some(pty) = self.channels.get_mut(&id) {
            pty.writer.write_all(data)?;
            pty.writer.flush()?;
        }
        Ok(())
    }
    async fn channel_close(&mut self, id: ChannelId, _: &mut Session) -> Result<(), Error> {
        self.channels.remove(&id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn real_openssh_authentication_exec_and_shutdown() {
        let root = tempfile::tempdir().unwrap();
        let home = root.path().to_path_buf();
        std::fs::create_dir(home.join(".ssh")).unwrap();
        let key = home.join("identity");
        assert!(std::process::Command::new("ssh-keygen")
            .args(["-q", "-t", "ed25519", "-N", "", "-f"])
            .arg(&key)
            .status()
            .unwrap()
            .success());
        std::fs::copy(home.join("identity.pub"), home.join(".ssh/authorized_keys")).unwrap();
        let probe = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = probe.local_addr().unwrap();
        drop(probe);
        let host = SshHost::start(HostConfig {
            address,
            public_key_auth: true,
            password_auth: false,
            home: home.clone(),
            user: "well-test".into(),
        });
        let until = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while !host.running.load(Ordering::Acquire) {
            assert!(
                std::time::Instant::now() < until,
                "{}",
                host.message.lock().unwrap()
            );
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let result = std::process::Command::new("ssh")
            .args([
                "-T",
                "-oBatchMode=yes",
                "-oStrictHostKeyChecking=accept-new",
                "-oIdentitiesOnly=yes",
                "-oConnectTimeout=5",
                "-o",
            ])
            .arg(format!(
                "UserKnownHostsFile={}",
                home.join("known_hosts").display()
            ))
            .arg("-i")
            .arg(key)
            .arg("-p")
            .arg(address.port().to_string())
            .arg("well-test@127.0.0.1")
            .arg("printf WELL_SSH_EXEC_VERIFIED")
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(String::from_utf8_lossy(&result.stdout).contains("WELL_SSH_EXEC_VERIFIED"));
        let denied = std::process::Command::new("ssh")
            .args([
                "-T",
                "-oBatchMode=yes",
                "-oStrictHostKeyChecking=yes",
                "-oIdentitiesOnly=yes",
                "-oIdentityFile=none",
                "-o",
            ])
            .arg(format!(
                "UserKnownHostsFile={}",
                home.join("known_hosts").display()
            ))
            .arg("-p")
            .arg(address.port().to_string())
            .arg("wrong-user@127.0.0.1")
            .arg("true")
            .output()
            .unwrap();
        assert!(!denied.status.success());
        host.stop();
        let until = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while host.running.load(Ordering::Acquire) {
            assert!(std::time::Instant::now() < until);
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }
}
