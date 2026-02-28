use std::sync::Arc;

use sofa_registry_core::model::ReceivedData;
use sofa_registry_core::pb::sofa::registry::ReceivedDataPb;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

use sofa_registry_server_shared::metrics as srv_metrics;

use super::StreamRegistry;

/// A task representing data that should be pushed to a set of subscribers.
pub struct PushTask {
    pub data_info_id: String,
    pub subscriber_regist_ids: Vec<String>,
    pub data: ReceivedData,
}

/// Sender side of the push pipeline.
pub struct PushService {
    tx: mpsc::Sender<PushTask>,
}

impl PushService {
    /// Create a new push service with the given buffer size.
    /// Returns the service (sender) and the receiver that processes tasks.
    pub fn new(buffer_size: usize, stream_registry: Arc<StreamRegistry>) -> (Self, PushReceiver) {
        let (tx, rx) = mpsc::channel(buffer_size);
        (
            Self { tx },
            PushReceiver {
                rx,
                stream_registry,
            },
        )
    }

    /// Enqueue a push task. If the channel is full, logs a warning and drops.
    pub async fn push(&self, task: PushTask) {
        if let Err(e) = self.tx.try_send(task) {
            match e {
                mpsc::error::TrySendError::Full(_) => {
                    warn!("Push task channel full, dropping push task");
                }
                mpsc::error::TrySendError::Closed(_) => {
                    warn!("Push task channel closed");
                }
            }
        }
    }
}

/// Receiver side of the push pipeline.
pub struct PushReceiver {
    rx: mpsc::Receiver<PushTask>,
    stream_registry: Arc<StreamRegistry>,
}

impl PushReceiver {
    /// Process push tasks until the cancellation token fires.
    pub async fn run(mut self, cancel: CancellationToken) {
        info!("PushReceiver started");
        loop {
            tokio::select! {
                _ = cancel.cancelled() => {
                    info!("PushReceiver shutting down");
                    break;
                }
                task = self.rx.recv() => {
                    match task {
                        Some(push_task) => {
                            self.process_push_task(push_task).await;
                        }
                        None => {
                            info!("Push task channel closed, exiting");
                            break;
                        }
                    }
                }
            }
        }
    }

    async fn process_push_task(&self, task: PushTask) {
        debug!(
            "Processing push task for data_info_id={}, subscribers={}",
            task.data_info_id,
            task.subscriber_regist_ids.len()
        );

        metrics::counter!(srv_metrics::SESSION_PUSH_TASKS_TOTAL).increment(1);

        // Convert ReceivedData to ReceivedDataPb
        let pb = received_data_to_pb(&task.data);

        // For each subscriber, find their client and push via the stream.
        // The subscriber_regist_ids list contains client_ids to look up in the stream registry.
        // In a more optimized version, we'd have a regist_id -> client_id index.
        for regist_id in &task.subscriber_regist_ids {
            if let Some(tx) = self.stream_registry.get(regist_id) {
                match tx.try_send(Ok(pb.clone())) {
                    Ok(_) => {
                        debug!("Pushed data to client stream: regist_id={}", regist_id);
                    }
                    Err(mpsc::error::TrySendError::Full(_)) => {
                        warn!(
                            "Client stream full, dropping push for regist_id={}",
                            regist_id
                        );
                        metrics::counter!(srv_metrics::SESSION_PUSH_TASKS_FAILED).increment(1);
                    }
                    Err(mpsc::error::TrySendError::Closed(_)) => {
                        debug!("Client stream closed for regist_id={}, removing", regist_id);
                        self.stream_registry.unregister(regist_id);
                        metrics::counter!(srv_metrics::SESSION_PUSH_TASKS_FAILED).increment(1);
                    }
                }
            }
        }
    }
}

fn received_data_to_pb(data: &ReceivedData) -> ReceivedDataPb {
    use sofa_registry_core::pb::sofa::registry::{DataBoxListPb, DataBoxPb};

    let data_map = data
        .data
        .iter()
        .map(|(k, boxes)| {
            let box_list = DataBoxListPb {
                data_box: boxes
                    .iter()
                    .map(|b| DataBoxPb {
                        data: b.data.clone().unwrap_or_default(),
                    })
                    .collect(),
            };
            (k.clone(), box_list)
        })
        .collect();

    let data_count = data
        .data_count
        .iter()
        .map(|(k, v)| (k.clone(), *v))
        .collect();

    ReceivedDataPb {
        data_id: data.data_id.clone(),
        group: data.group.clone(),
        instance_id: data.instance_id.clone(),
        segment: data.segment.clone().unwrap_or_default(),
        scope: data.scope.clone().unwrap_or_default(),
        subscriber_regist_ids: data.subscriber_regist_ids.clone(),
        data: data_map,
        version: data.version.unwrap_or(0),
        local_zone: data.local_zone.clone().unwrap_or_default(),
        data_count,
    }
}
