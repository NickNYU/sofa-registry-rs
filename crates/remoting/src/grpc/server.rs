use std::net::SocketAddr;
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tokio_stream::wrappers::TcpListenerStream;
use tracing::info;

/// A wrapper around tonic gRPC server with lifecycle management.
pub struct GrpcServer {
    addr: SocketAddr,
    shutdown_tx: Option<watch::Sender<bool>>,
    handle: Option<JoinHandle<()>>,
}

impl GrpcServer {
    pub fn new(port: u16) -> Self {
        let addr = SocketAddr::from(([0, 0, 0, 0], port));
        Self {
            addr,
            shutdown_tx: None,
            handle: None,
        }
    }

    /// Start the gRPC server with a pre-configured tonic Router.
    /// Pre-binds the TCP listener so the port is released promptly on stop.
    pub async fn start(
        &mut self,
        router: tonic::transport::server::Router,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let addr = self.addr;
        let listener = tokio::net::TcpListener::bind(addr).await?;
        let incoming = TcpListenerStream::new(listener);
        let (tx, mut rx) = watch::channel(false);
        self.shutdown_tx = Some(tx);

        let handle = tokio::spawn(async move {
            info!("gRPC server listening on {}", addr);
            let result = router
                .serve_with_incoming_shutdown(incoming, async move {
                    rx.changed().await.ok();
                })
                .await;
            if let Err(e) = result {
                tracing::error!("gRPC server error: {}", e);
            }
        });

        self.handle = Some(handle);
        Ok(())
    }

    pub fn stop(&self) {
        if let Some(tx) = &self.shutdown_tx {
            let _ = tx.send(true);
        }
    }

    /// Stop and wait for the server task to finish, ensuring the port is released.
    pub async fn stop_and_wait(&mut self) {
        self.stop();
        if let Some(h) = self.handle.take() {
            let _ = h.await;
        }
    }

    pub fn addr(&self) -> SocketAddr {
        self.addr
    }
}
