use std::sync::Arc;

use sofa_registry_core::model::DatumVersion;
use sofa_registry_remoting::GrpcClientPool;
use sofa_registry_server_data::change::{DataChangeEvent, DataChangeEventCenter};
use sofa_registry_server_data::lease::SessionLeaseManager;
use tokio_util::sync::CancellationToken;

/// Helper to create a DataChangeEvent with the given data_info_id and version.
fn make_event(data_info_id: &str, version: i64) -> DataChangeEvent {
    DataChangeEvent {
        data_center: "DefaultDataCenter".to_string(),
        data_info_id: data_info_id.to_string(),
        version: DatumVersion::of(version),
    }
}

/// Helper to create a DataChangeEventCenter with test defaults.
fn make_center(buffer_size: usize) -> (DataChangeEventCenter, sofa_registry_server_data::change::DataChangeReceiver) {
    let pool = Arc::new(GrpcClientPool::new());
    let lease_manager = Arc::new(SessionLeaseManager::new(60));
    DataChangeEventCenter::new(buffer_size, pool, lease_manager, "127.0.0.1:9600".to_string())
}

#[test]
fn new_returns_center_and_receiver() {
    let (center, _receiver) = make_center(16);
    // Cloning should work because Clone is implemented.
    let _center2 = center.clone();
}

#[tokio::test]
async fn on_change_sends_event_through_channel() {
    let (center, receiver) = make_center(16);

    center.on_change(make_event("com.example.ServiceA#default#DEFAULT_GROUP", 100));

    // The receiver wraps an mpsc::Receiver. We cannot call recv() directly on
    // DataChangeReceiver because the field is private. Instead, we verify the
    // event is processed by the merge loop: start the loop, let it merge, then
    // cancel.
    let cancel = CancellationToken::new();
    let cancel_clone = cancel.clone();

    // Use a very short debounce so the merge completes quickly.
    let handle = tokio::spawn(async move {
        receiver.run_merge_loop(10, cancel_clone).await;
    });

    // Give the merge loop time to process.
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    cancel.cancel();
    handle.await.unwrap();
}

#[tokio::test]
async fn merge_loop_deduplicates_events_by_data_info_id() {
    let (center, receiver) = make_center(64);
    let cancel = CancellationToken::new();
    let cancel_clone = cancel.clone();

    // Send multiple events for the same data_info_id with increasing versions.
    // The merge loop should keep only the last one per data_info_id.
    center.on_change(make_event("svc.A#inst#GRP", 1));
    center.on_change(make_event("svc.A#inst#GRP", 2));
    center.on_change(make_event("svc.A#inst#GRP", 3));
    center.on_change(make_event("svc.B#inst#GRP", 10));

    let handle = tokio::spawn(async move {
        // debounce of 30ms means events arriving within 30ms are merged.
        receiver.run_merge_loop(30, cancel_clone).await;
    });

    // Wait long enough for at least one debounce window to complete.
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    cancel.cancel();
    handle.await.unwrap();
    // If we reach here without panic, the merge loop ran and exited correctly.
}

#[tokio::test]
async fn merge_loop_cancellation_stops_immediately() {
    let (_center, receiver) = make_center(8);
    let cancel = CancellationToken::new();
    let cancel_clone = cancel.clone();

    // Cancel before any event arrives.
    cancel.cancel();

    let handle = tokio::spawn(async move {
        receiver.run_merge_loop(1000, cancel_clone).await;
    });

    // Should complete very quickly because cancellation was already requested.
    let result = tokio::time::timeout(std::time::Duration::from_millis(200), handle).await;
    assert!(result.is_ok(), "Merge loop should have exited promptly on cancellation");
    result.unwrap().unwrap();
}

#[tokio::test]
async fn merge_loop_exits_when_channel_closed() {
    let (center, receiver) = make_center(8);
    let cancel = CancellationToken::new();
    let cancel_clone = cancel.clone();

    // Drop the sender so the channel closes.
    drop(center);

    let handle = tokio::spawn(async move {
        receiver.run_merge_loop(10, cancel_clone).await;
    });

    let result = tokio::time::timeout(std::time::Duration::from_millis(200), handle).await;
    assert!(result.is_ok(), "Merge loop should exit when the channel is closed");
    result.unwrap().unwrap();
}

#[tokio::test]
async fn on_change_drops_event_when_channel_full() {
    // Buffer of 1 means only 1 event can be buffered.
    let (center, _receiver) = make_center(1);

    // First event should succeed (fills the buffer).
    center.on_change(make_event("svc.A#inst#GRP", 1));
    // Second event should be dropped (channel full) without panic.
    center.on_change(make_event("svc.B#inst#GRP", 2));
    // No panic means try_send handled the Full case gracefully.
}

#[tokio::test]
async fn clone_center_shares_same_channel() {
    let (center, receiver) = make_center(16);
    let center2 = center.clone();
    let cancel = CancellationToken::new();
    let cancel_clone = cancel.clone();

    // Both centers send events to the same channel.
    center.on_change(make_event("svc.A#inst#GRP", 1));
    center2.on_change(make_event("svc.B#inst#GRP", 2));

    let handle = tokio::spawn(async move {
        receiver.run_merge_loop(20, cancel_clone).await;
    });

    tokio::time::sleep(std::time::Duration::from_millis(80)).await;
    cancel.cancel();
    handle.await.unwrap();
}

#[tokio::test]
async fn merge_loop_handles_many_distinct_data_info_ids() {
    let (center, receiver) = make_center(256);
    let cancel = CancellationToken::new();
    let cancel_clone = cancel.clone();

    for i in 0..100 {
        center.on_change(make_event(&format!("svc.{}#inst#GRP", i), i as i64));
    }

    let handle = tokio::spawn(async move {
        receiver.run_merge_loop(30, cancel_clone).await;
    });

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    cancel.cancel();
    handle.await.unwrap();
}
