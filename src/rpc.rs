//! The desktop RPC host owns its runtime, socket directory, and cancellation path.
use crate::AppEvent;
use std::{path::PathBuf, sync::Arc, time::Duration};
use tokio::{
    runtime::Runtime,
    sync::{mpsc, oneshot},
};
use well_ipc::rpc::{HermesAppCommand, HermesRpcServer, HermesService, ServerStats};
use winit::event_loop::EventLoopProxy;

pub const METHODS: &[&str] = &[
    "surface.read_text",
    "workspace.state",
    "workspace.new_tab",
    "workspace.switch_tab",
    "surface.send_text",
];
pub struct RpcHost {
    runtime: Option<Runtime>,
    pub commands: mpsc::Receiver<HermesAppCommand>,
    task: tokio::task::JoinHandle<()>,
    stop: Option<oneshot::Sender<()>>,
    pub path: PathBuf,
    pub stats: Arc<ServerStats>,
    pub error: Option<String>,
    _directory: Option<tempfile::TempDir>,
}
impl RpcHost {
    pub fn new(proxy: EventLoopProxy<AppEvent>) -> Result<Self, Box<dyn std::error::Error>> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()?;
        let (tx, commands) = mpsc::channel(64);
        let wake = proxy.clone();
        let service = Arc::new(HermesService::with_methods(tx, METHODS, move || {
            let _ = wake.send_event(AppEvent::RpcWake);
        })?);
        let (path, directory) = if let Some(path) = std::env::var_os("WELL_RPC_SOCKET") {
            (PathBuf::from(path), None)
        } else {
            let directory = tempfile::Builder::new()
                .prefix("well-rpc-")
                .tempdir_in("/tmp")?;
            (directory.path().join("rpc.sock"), Some(directory))
        };
        let server = runtime
            .block_on(HermesRpcServer::with_service(service).bind(&path))
            .map_err(|e| e.to_string())?;
        let stats = server.stats.clone();
        let (stop, stopped) = oneshot::channel();
        let task = runtime.spawn(async move {
            let result = server
                .serve_until(async {
                    let _ = stopped.await;
                })
                .await
                .map_err(|e| e.to_string());
            let _ = proxy.send_event(AppEvent::RpcStopped(result));
        });
        eprintln!("Well automation socket: {}", path.display());
        Ok(Self {
            runtime: Some(runtime),
            commands,
            task,
            stop: Some(stop),
            path,
            stats,
            error: None,
            _directory: directory,
        })
    }
    pub fn shutdown(&mut self) {
        self.commands.close();
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(runtime) = self.runtime.take() {
            if runtime
                .block_on(async {
                    tokio::time::timeout(Duration::from_secs(2), &mut self.task).await
                })
                .is_err()
            {
                self.task.abort();
                let _ = runtime.block_on(&mut self.task);
            }
            runtime.shutdown_timeout(Duration::from_secs(2));
        }
    }
}
impl Drop for RpcHost {
    fn drop(&mut self) {
        self.shutdown();
    }
}
