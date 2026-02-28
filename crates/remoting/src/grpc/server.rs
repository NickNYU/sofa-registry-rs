use std::net::SocketAddr;
use tokio::sync::watch;
use tracing::info;

/// A wrapper around tonic gRPC server with lifecycle management.
pub struct GrpcServer {
    addr: SocketAddr,
    shutdown_tx: Option<watch::Sender<bool>>,
}

impl GrpcServer {
    pub fn new(port: u16) -> Self {
        let addr = SocketAddr::from(([0, 0, 0, 0], port));
        Self {
            addr,
            shutdown_tx: None,
        }
    }

    /// Start the gRPC server with a pre-configured tonic Router.
    /// Returns a JoinHandle for the server task.
    pub async fn start(
        &mut self,
        router: tonic::transport::server::Router,
    ) -> Result<tokio::task::JoinHandle<()>, Box<dyn std::error::Error>> {
        let addr = self.addr;
        let (tx, mut rx) = watch::channel(false);
        self.shutdown_tx = Some(tx);

        let handle = tokio::spawn(async move {
            info!("gRPC server listening on {}", addr);
            let result = router
                .serve_with_shutdown(addr, async move {
                    rx.changed().await.ok();
                })
                .await;
            if let Err(e) = result {
                tracing::error!("gRPC server error: {}", e);
            }
        });

        Ok(handle)
    }

    pub fn stop(&self) {
        if let Some(tx) = &self.shutdown_tx {
            let _ = tx.send(true);
        }
    }

    pub fn addr(&self) -> SocketAddr {
        self.addr
    }
}
