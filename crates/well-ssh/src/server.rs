use russh::server::{Handler, Server};
use russh::Error;
use std::sync::Arc;
use tokio::net::TcpListener;

#[derive(Clone, Default)]
pub struct SSHServer {}

impl Handler for SSHServer {
    type Error = Error;
}

impl Server for SSHServer {
    type Handler = Self;
    fn new_client(&mut self, _peer_addr: Option<std::net::SocketAddr>) -> Self::Handler {
        SSHServer {}
    }
}

pub async fn run_server(port: u16) -> Result<(), Error> {
    let config = russh::server::Config {
        inactivity_timeout: Some(std::time::Duration::from_secs(300)),
        ..Default::default()
    };
    let config = Arc::new(config);
    let mut server = SSHServer {};
    let listener = TcpListener::bind(("127.0.0.1", port))
        .await
        .map_err(Error::IO)?;

    while let Ok((stream, peer_addr)) = listener.accept().await {
        let handler = server.new_client(Some(peer_addr));
        let conf = config.clone();
        tokio::spawn(async move {
            let _ = russh::server::run_stream(conf, stream, handler).await;
        });
    }

    Ok(())
}
