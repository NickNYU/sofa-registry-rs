use axum::Router;
use std::net::SocketAddr;
use tokio::sync::watch;
use tracing::info;

/// HTTP server wrapper around axum with lifecycle management.
pub struct AxumHttpServer {
    addr: SocketAddr,
    shutdown_tx: Option<watch::Sender<bool>>,
}

impl AxumHttpServer {
    pub fn new(port: u16) -> Self {
        let addr = SocketAddr::from(([0, 0, 0, 0], port));
        Self {
            addr,
            shutdown_tx: None,
        }
    }

    /// Start the HTTP server with the given router.
    pub async fn start(
        &mut self,
        router: Router,
    ) -> Result<tokio::task::JoinHandle<()>, Box<dyn std::error::Error>> {
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
