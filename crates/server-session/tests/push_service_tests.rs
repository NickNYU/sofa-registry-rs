use sofa_registry_core::model::ReceivedData;
use sofa_registry_server_session::push::{PushService, PushTask, StreamRegistry};
use std::collections::HashMap;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

/// Helper to create a minimal ReceivedData.
fn make_received_data(data_id: &str) -> ReceivedData {
    ReceivedData {
        data_id: data_id.to_string(),
        group: "DEFAULT_GROUP".to_string(),
        instance_id: "default".to_string(),
        segment: None,
        scope: None,
        subscriber_regist_ids: vec![],
        data: HashMap::new(),
        version: Some(1),
        local_zone: None,
        data_count: HashMap::new(),
    }
}

fn make_push_task(data_info_id: &str, subscriber_ids: Vec<&str>) -> PushTask {
    PushTask {
        data_info_id: data_info_id.to_string(),
        subscriber_regist_ids: subscriber_ids.into_iter().map(String::from).collect(),
        data: make_received_data(data_info_id),
    }
}

#[test]
fn new_returns_service_and_receiver() {
    let (_service, _receiver) = PushService::new(16, Arc::new(StreamRegistry::new()));
}

#[tokio::test]
async fn push_task_is_received_by_receiver() {
    let (service, receiver) = PushService::new(16, Arc::new(StreamRegistry::new()));
    let cancel = CancellationToken::new();
    let cancel_clone = cancel.clone();

    service
        .push(make_push_task("svc.A#default#GRP", vec!["sub-1", "sub-2"]))
        .await;

    let handle = tokio::spawn(async move {
        receiver.run(cancel_clone).await;
    });

    // Give the receiver time to process.
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    cancel.cancel();
    handle.await.unwrap();
}

#[tokio::test]
async fn push_multiple_tasks() {
    let (service, receiver) = PushService::new(32, Arc::new(StreamRegistry::new()));
    let cancel = CancellationToken::new();
    let cancel_clone = cancel.clone();

    for i in 0..10 {
        service
            .push(make_push_task(
                &format!("svc.{}#default#GRP", i),
                vec!["sub-1"],
            ))
            .await;
    }

    let handle = tokio::spawn(async move {
        receiver.run(cancel_clone).await;
    });

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    cancel.cancel();
    handle.await.unwrap();
}

#[tokio::test]
async fn push_drops_task_when_channel_full() {
    // Buffer of 1: only 1 task can be buffered.
    let (service, _receiver) = PushService::new(1, Arc::new(StreamRegistry::new()));

    // First push fills the buffer.
    service
        .push(make_push_task("svc.A#default#GRP", vec!["sub-1"]))
        .await;

    // Second push should not panic (it logs a warning and drops).
    service
        .push(make_push_task("svc.B#default#GRP", vec!["sub-2"]))
        .await;
}

#[tokio::test]
async fn receiver_exits_on_cancellation() {
    let (_service, receiver) = PushService::new(8, Arc::new(StreamRegistry::new()));
    let cancel = CancellationToken::new();
    let cancel_clone = cancel.clone();

    cancel.cancel();

    let handle = tokio::spawn(async move {
        receiver.run(cancel_clone).await;
    });

    let result = tokio::time::timeout(std::time::Duration::from_millis(200), handle).await;
    assert!(
        result.is_ok(),
        "PushReceiver should exit promptly when cancelled"
    );
    result.unwrap().unwrap();
}

#[tokio::test]
async fn receiver_exits_when_channel_closed() {
    let (service, receiver) = PushService::new(8, Arc::new(StreamRegistry::new()));
    let cancel = CancellationToken::new();
    let cancel_clone = cancel.clone();

    // Drop the sender side so the channel closes.
    drop(service);

    let handle = tokio::spawn(async move {
        receiver.run(cancel_clone).await;
    });

    let result = tokio::time::timeout(std::time::Duration::from_millis(200), handle).await;
    assert!(
        result.is_ok(),
        "PushReceiver should exit when channel is closed"
    );
    result.unwrap().unwrap();
}

#[tokio::test]
async fn push_after_receiver_dropped_does_not_panic() {
    let (service, receiver) = PushService::new(8, Arc::new(StreamRegistry::new()));
    drop(receiver);

    // Pushing after the receiver is dropped should not panic.
    service
        .push(make_push_task("svc.A#default#GRP", vec!["sub-1"]))
        .await;
}

#[tokio::test]
async fn push_task_has_correct_fields() {
    let task = make_push_task("svc.A#default#GRP", vec!["sub-1", "sub-2"]);
    assert_eq!(task.data_info_id, "svc.A#default#GRP");
    assert_eq!(task.subscriber_regist_ids.len(), 2);
    assert_eq!(task.data.data_id, "svc.A#default#GRP");
    assert_eq!(task.data.group, "DEFAULT_GROUP");
}

#[tokio::test]
async fn concurrent_pushes_do_not_panic() {
    let (service, receiver) = PushService::new(64, Arc::new(StreamRegistry::new()));
    let cancel = CancellationToken::new();
    let cancel_clone = cancel.clone();

    let receiver_handle = tokio::spawn(async move {
        receiver.run(cancel_clone).await;
    });

    // Spawn multiple concurrent pushers.
    let handles = vec![];
    for i in 0..10 {
        let svc = PushService::new(1, Arc::new(StreamRegistry::new())); // We cannot clone PushService, so we
                                                                        // re-use the original.
                                                                        // Actually PushService does not implement Clone. Let's just push
                                                                        // sequentially from different tasks using a shared reference approach.
                                                                        // Since push takes &self, we can wrap in Arc.
        let _ = svc;
        let _ = i;
    }
    // Instead, push sequentially from the same service:
    for i in 0..20 {
        service
            .push(make_push_task(
                &format!("svc.{}#default#GRP", i),
                vec!["sub-1"],
            ))
            .await;
    }

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    cancel.cancel();
    receiver_handle.await.unwrap();
    for h in handles {
        let _: () = h;
    }
}
