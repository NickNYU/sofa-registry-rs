use async_trait::async_trait;
use parking_lot::RwLock;
use sofa_registry_core::error::{RegistryError, Result};
use sofa_registry_core::model::ReceivedData;
use sofa_registry_core::pb::sofa::registry::{BaseRegisterPb, DataBoxPb};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::Notify;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::api::observer::SubscriberDataObserver;
use crate::api::publisher::PublisherHandle;
use crate::api::registration::{PublisherRegistration, SubscriberRegistration};
use crate::api::registry_client::RegistryClient;
use crate::api::subscriber::SubscriberHandle;
use crate::auth::AuthManager;
use crate::config::RegistryClientConfig;
use crate::remoting::ClientConnection;
use crate::task::ReconnectTask;

/// Concrete publisher handle returned by DefaultRegistryClient.
struct DefaultPublisherHandle {
    data_id: String,
    regist_id: String,
    registered: AtomicBool,
    connection: Arc<ClientConnection>,
    base_register: BaseRegisterPb,
}

#[async_trait]
impl PublisherHandle for DefaultPublisherHandle {
    async fn republish(&self, data: &[&str]) -> Result<()> {
        if !self.registered.load(Ordering::SeqCst) {
            return Err(RegistryError::Refused(
                "publisher not registered".to_string(),
            ));
        }

        let data_list: Vec<DataBoxPb> = data
            .iter()
            .map(|d| DataBoxPb {
                data: d.to_string(),
            })
            .collect();

        let resp = self
            .connection
            .register_publisher(self.base_register.clone(), data_list)
            .await?;

        if !resp.success {
            return Err(RegistryError::Refused(resp.message));
        }
        Ok(())
    }

    async fn unregister(&self) -> Result<()> {
        if !self.registered.load(Ordering::SeqCst) {
            return Ok(());
        }

        let resp = self
            .connection
            .unregister(self.base_register.clone(), "PUB")
            .await?;

        self.registered.store(false, Ordering::SeqCst);

        if !resp.success {
            return Err(RegistryError::Refused(resp.message));
        }
        Ok(())
    }

    fn data_id(&self) -> &str {
        &self.data_id
    }

    fn regist_id(&self) -> &str {
        &self.regist_id
    }

    fn is_registered(&self) -> bool {
        self.registered.load(Ordering::SeqCst)
    }
}

/// Concrete subscriber handle returned by DefaultRegistryClient.
struct DefaultSubscriberHandle {
    data_id: String,
    registered: AtomicBool,
    latest_data: RwLock<Option<ReceivedData>>,
    observer: RwLock<Option<Arc<dyn SubscriberDataObserver>>>,
}

impl DefaultSubscriberHandle {
    fn notify_data(&self, data: ReceivedData) {
        let observer = self.observer.read().clone();
        *self.latest_data.write() = Some(data.clone());
        if let Some(obs) = observer {
            obs.handle_data(&self.data_id, data);
        }
    }
}

impl SubscriberHandle for DefaultSubscriberHandle {
    fn peek_data(&self) -> Option<ReceivedData> {
        self.latest_data.read().clone()
    }

    fn set_observer(&self, observer: Arc<dyn SubscriberDataObserver>) {
        *self.observer.write() = Some(observer);
    }

    fn data_id(&self) -> &str {
        &self.data_id
    }

    fn is_registered(&self) -> bool {
        self.registered.load(Ordering::SeqCst)
    }
}

/// Default implementation of [`RegistryClient`] that connects to a session server via gRPC.
pub struct DefaultRegistryClient {
    config: RegistryClientConfig,
    client_id: String,
    connection: Arc<ClientConnection>,
    auth_manager: Option<AuthManager>,
    subscribers: Arc<RwLock<HashMap<String, Vec<Arc<DefaultSubscriberHandle>>>>>,
    publishers: Arc<RwLock<Vec<Arc<DefaultPublisherHandle>>>>,
    shutdown: Arc<Notify>,
}

impl DefaultRegistryClient {
    /// Create a new client with the given configuration.
    /// Call [`connect`] to establish the gRPC channel before registering publishers/subscribers.
    pub fn new(config: RegistryClientConfig) -> Self {
        let addr = config
            .session_server_addresses
            .first()
            .cloned()
            .unwrap_or_else(|| "127.0.0.1:9601".to_string());

        let connection = Arc::new(ClientConnection::new(
            &addr,
            config.connect_timeout_ms,
            config.request_timeout_ms,
        ));

        let auth_manager = if config.auth_enabled {
            match (&config.access_key, &config.secret_key) {
                (Some(ak), Some(sk)) => Some(AuthManager::new(ak, sk)),
                _ => None,
            }
        } else {
            None
        };

        let client_id = Uuid::new_v4().to_string();

        Self {
            config,
            client_id,
            connection,
            auth_manager,
            subscribers: Arc::new(RwLock::new(HashMap::new())),
            publishers: Arc::new(RwLock::new(Vec::new())),
            shutdown: Arc::new(Notify::new()),
        }
    }

    /// Connect to the session server.
    pub async fn connect(&self) -> Result<()> {
        self.connection.connect().await
    }

    /// Start the background reconnection task and push subscription stream.
    /// Returns a join handle for the reconnect task.
    pub fn start_background_tasks(&self) -> tokio::task::JoinHandle<()> {
        let reconnect = ReconnectTask::new(
            self.connection.clone(),
            self.config.reconnect_delay_ms,
            self.config.max_reconnect_delay_ms,
        );
        let shutdown = self.shutdown.clone();
        let connection = self.connection.clone();
        let client_id = self.client_id.clone();
        let zone = self.config.zone.clone();
        let data_center = self.config.data_center.clone();
        let subscribers = self.subscribers.clone();
        let publishers = self.publishers.clone();

        tokio::spawn(async move {
            // Start reconnect loop in background
            let reconnect_shutdown = reconnect.shutdown_handle();
            let reconnect_handle = tokio::spawn(async move {
                reconnect.run().await;
            });

            // Start subscribe stream processor
            let stream_handle = tokio::spawn(async move {
                loop {
                    if !connection.is_connected() {
                        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                        continue;
                    }
                    match connection
                        .subscribe_stream(&client_id, &zone, &data_center)
                        .await
                    {
                        Ok(mut rx) => {
                            info!("Subscribe stream established");
                            while let Some(data) = rx.recv().await {
                                let data_info_id =
                                    format!("{}#{}#{}", data.data_id, data.instance_id, data.group);
                                let subs = subscribers.read();
                                if let Some(sub_list) = subs.get(&data_info_id) {
                                    for sub in sub_list {
                                        sub.notify_data(data.clone());
                                    }
                                } else {
                                    debug!(
                                        "Received data for unknown subscriber: {}",
                                        data_info_id
                                    );
                                }
                            }
                            warn!("Subscribe stream ended, will reconnect");
                        }
                        Err(e) => {
                            warn!("Failed to open subscribe stream: {}", e);
                            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                        }
                    }
                }
            });

            // Wait for shutdown
            shutdown.notified().await;

            // Unregister all publishers so the server can clean up
            // and notify remaining subscribers.
            let active_pubs: Vec<Arc<DefaultPublisherHandle>> = publishers
                .read()
                .iter()
                .filter(|h| h.is_registered())
                .cloned()
                .collect();
            for handle in &active_pubs {
                if let Err(e) = handle.unregister().await {
                    debug!("Failed to unregister publisher on shutdown: {}", e);
                }
            }

            reconnect_shutdown.notify_one();
            reconnect_handle.abort();
            stream_handle.abort();
        })
    }

    /// Shut down background tasks.
    pub fn shutdown(&self) {
        self.shutdown.notify_one();
    }

    /// Build a `BaseRegisterPb` with common fields populated.
    fn build_base_pb(
        &self,
        data_id: &str,
        group: &str,
        instance_id: &str,
        regist_id: &str,
        app_name: Option<&str>,
    ) -> BaseRegisterPb {
        let data_info_id = format!("{}#{}#{}", data_id, instance_id, group);
        let now = chrono::Utc::now().timestamp_millis();

        let mut attributes = HashMap::new();
        if let Some(ref auth) = self.auth_manager {
            attributes.insert("accessKey".to_string(), auth.access_key().to_string());
            attributes.insert("timestamp".to_string(), now.to_string());
            attributes.insert("signature".to_string(), auth.sign(now));
        }

        BaseRegisterPb {
            instance_id: instance_id.to_string(),
            zone: self.config.zone.clone(),
            app_name: app_name
                .or(Some(self.config.app_name.as_str()))
                .unwrap_or_default()
                .to_string(),
            data_id: data_id.to_string(),
            group: group.to_string(),
            process_id: String::new(),
            regist_id: regist_id.to_string(),
            client_id: self.client_id.clone(),
            data_info_id,
            ip: String::new(),
            port: 0,
            event_type: "REGISTER".to_string(),
            version: 0,
            timestamp: now,
            attributes,
        }
    }
}

#[async_trait]
impl RegistryClient for DefaultRegistryClient {
    async fn register_publisher(
        &self,
        reg: PublisherRegistration,
        data: &[&str],
    ) -> Result<Arc<dyn PublisherHandle>> {
        let regist_id = Uuid::new_v4().to_string();
        let base = self.build_base_pb(
            &reg.data_id,
            &reg.group,
            &reg.instance_id,
            &regist_id,
            reg.app_name.as_deref(),
        );

        let data_list: Vec<DataBoxPb> = data
            .iter()
            .map(|d| DataBoxPb {
                data: d.to_string(),
            })
            .collect();

        let resp = self
            .connection
            .register_publisher(base.clone(), data_list)
            .await?;

        if !resp.success {
            if resp.refused {
                return Err(RegistryError::Refused(resp.message));
            }
            let msg = if resp.message.is_empty() {
                "registration failed".to_string()
            } else {
                resp.message
            };
            return Err(RegistryError::Remoting(msg));
        }

        let actual_regist_id = if resp.regist_id.is_empty() {
            regist_id
        } else {
            resp.regist_id
        };

        let handle = Arc::new(DefaultPublisherHandle {
            data_id: reg.data_id.clone(),
            regist_id: actual_regist_id,
            registered: AtomicBool::new(true),
            connection: self.connection.clone(),
            base_register: base,
        });

        // Track the publisher so we can unregister on shutdown.
        self.publishers.write().push(handle.clone());

        info!("Publisher registered: data_id={}", reg.data_id);
        Ok(handle)
    }

    async fn register_subscriber(
        &self,
        reg: SubscriberRegistration,
    ) -> Result<Arc<dyn SubscriberHandle>> {
        let regist_id = Uuid::new_v4().to_string();
        let base = self.build_base_pb(
            &reg.data_id,
            &reg.group,
            &reg.instance_id,
            &regist_id,
            reg.app_name.as_deref(),
        );

        let resp = self
            .connection
            .register_subscriber(base, reg.scope.to_string())
            .await?;

        if !resp.success {
            if resp.refused {
                return Err(RegistryError::Refused(resp.message));
            }
            let msg = if resp.message.is_empty() {
                "registration failed".to_string()
            } else {
                resp.message
            };
            return Err(RegistryError::Remoting(msg));
        }

        let handle = Arc::new(DefaultSubscriberHandle {
            data_id: reg.data_id.clone(),
            registered: AtomicBool::new(true),
            latest_data: RwLock::new(None),
            observer: RwLock::new(None),
        });

        let data_info_id = reg.data_info_id();
        self.subscribers
            .write()
            .entry(data_info_id)
            .or_default()
            .push(handle.clone());

        info!("Subscriber registered: data_id={}", reg.data_id);
        Ok(handle)
    }
}
