use russh::client::Handler;
use russh::ChannelId;
use std::future::Future;
use std::sync::Arc;

pub struct ArgusClient {}

impl Handler for ArgusClient {
    type Error = russh::Error;

    // Default implementation handles check_server_key safely or we override
    fn check_server_key(
        &mut self,
        _server_public_key: &russh::keys::PublicKeyOrCertificate,
    ) -> impl Future<Output = Result<bool, Self::Error>> + Send {
        async { Ok(true) }
    }
}

pub async fn connect_argus_subsystem(target: &str, _user: &str) -> Result<(), russh::Error> {
    let config = russh::client::Config {
        inactivity_timeout: Some(std::time::Duration::from_secs(60)),
        ..Default::default()
    };
    let config = Arc::new(config);
    let handler = ArgusClient {};

    let mut _session = russh::client::connect(config, target, handler).await?;

    Ok(())
}
