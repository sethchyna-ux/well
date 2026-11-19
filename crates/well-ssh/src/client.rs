//! Native Argus connector. Unknown or changed host keys fail closed; the desktop
//! OpenSSH path supplies the interactive first-use confirmation flow.
use russh::client::Handler;
use std::sync::Arc;

pub struct ArgusClient {
    pub host: String,
    pub port: u16,
}
impl Handler for ArgusClient {
    type Error = russh::Error;
    async fn check_server_key(
        &mut self,
        key: &russh::keys::PublicKeyOrCertificate,
    ) -> Result<bool, Self::Error> {
        Ok(key.certificate().is_none()
            && russh::keys::check_known_hosts(&self.host, self.port, &key.public_key())
                .unwrap_or(false))
    }
}
pub async fn connect_argus_subsystem(
    target: &str,
    user: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (host, port) = target
        .rsplit_once(':')
        .and_then(|(h, p)| p.parse::<u16>().ok().map(|p| (h, p)))
        .unwrap_or((target, 22));
    let handler = ArgusClient {
        host: host.trim_matches(['[', ']']).into(),
        port,
    };
    let config = Arc::new(russh::client::Config {
        inactivity_timeout: Some(std::time::Duration::from_secs(60)),
        ..Default::default()
    });
    let mut session = russh::client::connect(config, (host, port), handler).await?;
    let path = std::path::PathBuf::from(std::env::var_os("HOME").ok_or("HOME unavailable")?)
        .join(".ssh/id_ed25519");
    let key = Arc::new(russh::keys::load_secret_key(path, None)?);
    let auth = session
        .authenticate_publickey(user, russh::keys::PrivateKeyWithHashAlg::new(key, None))
        .await?;
    if !auth.success() {
        return Err("Argus SSH authentication failed".into());
    }
    let mut channel = session.channel_open_session().await?;
    channel.request_subsystem(true, "argus").await?;
    match channel.wait().await {
        Some(russh::ChannelMsg::Success) => Ok(()),
        _ => Err("Argus subsystem was not accepted by the remote host".into()),
    }
}
