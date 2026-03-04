use axum::Router;
use std::net::SocketAddr;
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tracing::info;

/// HTTP server wrapper around axum with lifecycle management.
pub struct AxumHttpServer {
    addr: SocketAddr,
    shutdown_tx: Option<watch::Sender<bool>>,
    handle: Option<JoinHandle<()>>,
}

impl AxumHttpServer {
    pub fn new(port: u16) -> Self {
        let addr = SocketAddr::from(([0, 0, 0, 0], port));
        Self {
            addr,
            shutdown_tx: None,
            handle: None,
        }
    }

    /// Start the HTTP server with the given router.
    pub async fn start(
        &mut self,
        router: Router,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let addr = self.addr;
        let (tx, mut rx) = watch::channel(false);
        self.shutdown_tx = Some(tx);

        let listener = tokio::net::TcpListener::bind(addr).await?;
        info!("HTTP server listening on {}", addr);

        let handle = tokio::spawn(async move {
            let result = axum::serve(listener, router)
                .with_graceful_shutdown(async move {
                    rx.changed().await.ok();
                })
                .await;
            if let Err(e) = result {
                tracing::error!("HTTP server error: {}", e);
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
