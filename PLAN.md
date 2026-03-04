# E2E & Chaos Test Plan for sofa-registry-rs

## Current State

**Build:** All 12 crates compile clean.
**Existing tests:** 31 test executables, but integration tests only cover health-check level (server starts, HTTP endpoint responds). No E2E pub/sub flow tests exist yet.
**gRPC services:** All fully implemented — client can publish, subscribe, receive push notifications via streaming.
**Gap:** Nobody has tested the actual end-to-end flow: client publishes → data reaches subscribers.

---

## Team Structure

### Roles (16 agents total)

| Agent | Role | Type | Responsibility |
|-------|------|------|----------------|
| `team-lead` | Team Lead | general-purpose | Orchestrate iterations, assign tasks, track progress |
| `qa-lead` | QA Lead | general-purpose | Write test plans, define test cases, run test suites, report failures |
| `qa-e2e-1` | QA Engineer | general-purpose | Write & run E2E test suite (pub/sub, multi-client, push) |
| `qa-e2e-2` | QA Engineer | general-purpose | Write & run E2E test suite (lifecycle, reconnect, lease) |
| `qa-chaos-1` | QA Chaos | general-purpose | Write & run chaos tests (server crash, network partition, slot migration) |
| `qa-chaos-2` | QA Chaos | general-purpose | Write & run chaos tests (load, concurrency, data consistency) |
| `dev-session` | Dev (Session) | general-purpose | Fix bugs in session server & push flow |
| `dev-data` | Dev (Data) | general-purpose | Fix bugs in data server & storage layer |
| `dev-meta` | Dev (Meta) | general-purpose | Fix bugs in meta server & slot management |
| `dev-client` | Dev (Client) | general-purpose | Fix bugs in client SDK & connection handling |
| `dev-core` | Dev (Core) | general-purpose | Fix bugs in core types, protobuf, remoting |
| `dev-infra` | Dev (Infra) | general-purpose | Build shared test infrastructure & harness |
| `ops-reviewer` | Ops Reviewer | general-purpose | Review test results, verify fixes, sign off on iterations |
| `ops-monitor` | Ops Monitor | general-purpose | Validate HTTP admin APIs, metrics, health checks during tests |
| `architect` | Architect | Explore | Read-only: investigate root causes, trace code paths |
| `doc-writer` | Doc Writer | general-purpose | Persist test results, write iteration reports |

---

## Phase 0: Test Infrastructure (Iteration 0)

### 0.1 Shared Test Harness

Create `crates/integration-tests/src/harness.rs`:
- `TestCluster` struct: boots Meta → Data → Session with unique ports
- `TestClient` helper: wraps `DefaultRegistryClient` with convenience methods
- Port allocator that supports 50+ parallel test clusters
- `wait_for_cluster_ready()`: polls health endpoints until all servers report UP
- `wait_for_push()`: async helper that waits for subscriber observer callback with timeout
- Tracing/logging capture per test

### 0.2 Test Result Reporter

Create `crates/integration-tests/src/reporter.rs`:
- Captures test name, duration, pass/fail, error details
- Outputs JSON report to `test-results/iteration-{N}.json`
- Aggregates results across test suites

### 0.3 Test Configuration

Create `crates/integration-tests/tests/test_config.rs`:
- Shared constants: timeouts, retry counts, port ranges
- Feature flags for enabling/disabling chaos scenarios

---

## Phase 1: E2E Test Suite

### Suite 1: Basic Pub/Sub (qa-e2e-1)

File: `crates/integration-tests/tests/e2e_pubsub_test.rs`

| # | Test Case | Description | Priority |
|---|-----------|-------------|----------|
| 1.1 | `test_publish_then_subscribe` | Publisher registers data, then subscriber subscribes and receives the data | P0 |
| 1.2 | `test_subscribe_then_publish` | Subscriber registers first, then publisher publishes, subscriber receives push | P0 |
| 1.3 | `test_republish_updates_subscriber` | Publisher updates data via `republish()`, subscriber receives updated value | P0 |
| 1.4 | `test_unregister_publisher` | Publisher unregisters, subscriber receives empty/updated notification | P0 |
| 1.5 | `test_unregister_subscriber` | Subscriber unregisters, no more push received | P1 |
| 1.6 | `test_multiple_publishers_same_dataid` | Two publishers register same dataId, subscriber sees both values | P0 |
| 1.7 | `test_multiple_subscribers_same_dataid` | Two subscribers on same dataId, both receive push | P0 |
| 1.8 | `test_publish_different_groups` | Publishers in different groups, subscriber only sees matching group | P1 |
| 1.9 | `test_publish_different_instance_ids` | Different instanceIds treated as different services | P1 |
| 1.10 | `test_empty_publish` | Publisher with empty data list | P2 |

### Suite 2: Multi-Client (qa-e2e-1)

File: `crates/integration-tests/tests/e2e_multiclient_test.rs`

| # | Test Case | Description | Priority |
|---|-----------|-------------|----------|
| 2.1 | `test_two_clients_cross_pubsub` | Client A publishes, Client B subscribes (different connections) | P0 |
| 2.2 | `test_many_clients_fan_out` | 1 publisher, 10 subscribers all receive data | P0 |
| 2.3 | `test_many_publishers_fan_in` | 10 publishers, 1 subscriber receives all 10 values | P1 |
| 2.4 | `test_100_services_concurrent` | 100 different dataIds, each with 1 pub + 1 sub | P1 |

### Suite 3: Push & Streaming (qa-e2e-1)

File: `crates/integration-tests/tests/e2e_push_test.rs`

| # | Test Case | Description | Priority |
|---|-----------|-------------|----------|
| 3.1 | `test_push_on_publish` | Subscriber receives push when new publisher registers | P0 |
| 3.2 | `test_push_on_republish` | Subscriber receives push when publisher updates data | P0 |
| 3.3 | `test_push_on_unpublish` | Subscriber receives push when publisher unregisters | P0 |
| 3.4 | `test_push_data_contains_all_publishers` | Push payload includes data from all publishers for the dataId | P0 |
| 3.5 | `test_push_version_monotonically_increases` | Each push has a higher version than the previous | P1 |
| 3.6 | `test_push_latency_under_threshold` | Push received within 5s of publish (configurable) | P1 |

### Suite 4: Lifecycle & Connection (qa-e2e-2)

File: `crates/integration-tests/tests/e2e_lifecycle_test.rs`

| # | Test Case | Description | Priority |
|---|-----------|-------------|----------|
| 4.1 | `test_client_heartbeat_keeps_connection` | Client heartbeat prevents connection eviction | P1 |
| 4.2 | `test_client_reconnect_after_disconnect` | Client reconnects and re-registers after connection drop | P1 |
| 4.3 | `test_graceful_shutdown_cleans_publishers` | When client disconnects, its publishers are cleaned up | P0 |
| 4.4 | `test_server_restart_client_recovers` | Session server restarts, client reconnects and subscriptions work | P1 |

### Suite 5: Admin API Verification (qa-e2e-2)

File: `crates/integration-tests/tests/e2e_admin_api_test.rs`

| # | Test Case | Description | Priority |
|---|-----------|-------------|----------|
| 5.1 | `test_session_publisher_count_reflects_registrations` | HTTP API shows correct publisher count after registrations | P1 |
| 5.2 | `test_session_subscriber_count_reflects_registrations` | HTTP API shows correct subscriber count | P1 |
| 5.3 | `test_data_datum_count_reflects_publishes` | Data server datum count matches published services | P1 |
| 5.4 | `test_meta_slot_table_has_all_slots` | Meta slot table API returns configured number of slots | P1 |
| 5.5 | `test_meta_shows_registered_servers` | Meta health shows data_server_count and session_server_count | P1 |

---

## Phase 2: Chaos Test Suite

### Suite 6: Server Failure & Recovery (qa-chaos-1)

File: `crates/integration-tests/tests/chaos_server_failure_test.rs`

| # | Test Case | Description | Priority |
|---|-----------|-------------|----------|
| 6.1 | `test_data_server_crash_and_restart` | Kill data server, restart, verify data recoverable | P0 |
| 6.2 | `test_session_server_crash_and_restart` | Kill session server, restart, verify clients reconnect | P0 |
| 6.3 | `test_meta_server_crash_and_restart` | Kill meta server, restart, verify leader re-election | P1 |
| 6.4 | `test_all_servers_restart_in_sequence` | Restart all servers one by one, verify system recovers | P1 |

### Suite 7: Slot Operations (qa-chaos-1)

File: `crates/integration-tests/tests/chaos_slot_test.rs`

| # | Test Case | Description | Priority |
|---|-----------|-------------|----------|
| 7.1 | `test_slot_assignment_after_data_server_join` | New data server joins, gets slots assigned | P1 |
| 7.2 | `test_slot_rebalance_after_data_server_leave` | Data server leaves, slots redistributed | P1 |
| 7.3 | `test_data_accessible_during_slot_migration` | Data remains queryable during rebalance | P2 |

### Suite 8: Concurrency & Load (qa-chaos-2)

File: `crates/integration-tests/tests/chaos_load_test.rs`

| # | Test Case | Description | Priority |
|---|-----------|-------------|----------|
| 8.1 | `test_concurrent_publish_subscribe_50_clients` | 50 clients pub/sub simultaneously | P0 |
| 8.2 | `test_rapid_publish_unpublish_cycle` | Fast register/unregister 1000 times | P1 |
| 8.3 | `test_1000_services_steady_state` | Register 1000 services, verify all subs receive data | P1 |
| 8.4 | `test_burst_100_publishes_in_1_second` | Burst publish, verify all subs eventually receive | P1 |

### Suite 9: Data Consistency (qa-chaos-2)

File: `crates/integration-tests/tests/chaos_consistency_test.rs`

| # | Test Case | Description | Priority |
|---|-----------|-------------|----------|
| 9.1 | `test_no_data_loss_after_server_restart` | Publish data, restart servers, verify data still accessible | P0 |
| 9.2 | `test_subscriber_sees_latest_version` | After multiple publishes, subscriber always sees latest | P0 |
| 9.3 | `test_eventual_consistency_after_partition` | Simulate slow responses, verify eventual delivery | P1 |
| 9.4 | `test_duplicate_publish_idempotent` | Same publisher registers twice, data not duplicated | P1 |

---

## Iteration Loop (5-10 cycles)

```
┌──────────────────────────────────────────────────────┐
│  Iteration N                                          │
│                                                        │
│  Step 1: QA RUN TESTS                                 │
│  ├── qa-e2e-1: runs E2E pub/sub + multi-client suites │
│  ├── qa-e2e-2: runs lifecycle + admin API suites       │
│  ├── qa-chaos-1: runs server failure + slot suites     │
│  ├── qa-chaos-2: runs load + consistency suites        │
│  └── qa-lead: aggregates results → failures.md         │
│                                                        │
│  Step 2: DEV FIX                                       │
│  ├── dev-session: fixes session/push bugs              │
│  ├── dev-data: fixes data storage bugs                 │
│  ├── dev-meta: fixes meta/slot bugs                    │
│  ├── dev-client: fixes client SDK bugs                 │
│  ├── dev-core: fixes core/protobuf bugs                │
│  └── dev-infra: fixes test harness issues              │
│                                                        │
│  Step 3: OPS/QA REVIEW                                 │
│  ├── ops-reviewer: verifies fixes, re-runs failed tests│
│  ├── ops-monitor: checks admin APIs, metrics           │
│  └── qa-lead: updates test status, plans next iteration│
│                                                        │
│  Output: test-results/iteration-{N}.json               │
│          test-results/iteration-{N}-report.md           │
└──────────────────────────────────────────────────────┘
```

### Iteration Exit Criteria

An iteration is "green" when:
- All P0 tests pass
- No P1 regressions from previous iteration
- Test report shows >= 80% pass rate overall

### Project Exit Criteria (stop iterating)

- All P0 tests pass (28 tests)
- >= 90% of P1 tests pass
- No known data loss or consistency bugs
- Chaos tests demonstrate recovery within defined timeouts

---

## File Layout

```
sofa-registry-rs/
├── crates/integration-tests/
│   ├── Cargo.toml                          # add dependencies: tokio, reqwest, etc.
│   ├── src/
│   │   ├── lib.rs                          # export harness + reporter
│   │   ├── harness.rs                      # TestCluster, TestClient, port allocator
│   │   └── reporter.rs                     # JSON test reporter
│   └── tests/
│       ├── server_lifecycle_test.rs        # (existing)
│       ├── test_config.rs                  # shared constants
│       ├── e2e_pubsub_test.rs             # Suite 1: basic pub/sub
│       ├── e2e_multiclient_test.rs        # Suite 2: multi-client
│       ├── e2e_push_test.rs               # Suite 3: push & streaming
│       ├── e2e_lifecycle_test.rs          # Suite 4: lifecycle & connection
│       ├── e2e_admin_api_test.rs          # Suite 5: admin API
│       ├── chaos_server_failure_test.rs   # Suite 6: server crash/restart
│       ├── chaos_slot_test.rs             # Suite 7: slot operations
│       ├── chaos_load_test.rs             # Suite 8: concurrency & load
│       └── chaos_consistency_test.rs      # Suite 9: data consistency
├── test-results/                           # generated per iteration
│   ├── iteration-1.json
│   ├── iteration-1-report.md
│   └── ...
└── TEST_PLAN.md                            # this document (persisted)
```

---

## Task Breakdown (Iteration 1)

### Infrastructure tasks (blocks everything)
1. **Build test harness** (`dev-infra`): TestCluster, TestClient, port allocator, wait helpers
2. **Build test reporter** (`dev-infra`): JSON output, markdown summary generation

### E2E test writing (parallel, after infra)
3. **Write Suite 1: pub/sub tests** (`qa-e2e-1`): 10 test cases
4. **Write Suite 2: multi-client tests** (`qa-e2e-1`): 4 test cases
5. **Write Suite 3: push tests** (`qa-e2e-1`): 6 test cases
6. **Write Suite 4: lifecycle tests** (`qa-e2e-2`): 4 test cases
7. **Write Suite 5: admin API tests** (`qa-e2e-2`): 5 test cases

### Chaos test writing (parallel, after infra)
8. **Write Suite 6: server failure tests** (`qa-chaos-1`): 4 test cases
9. **Write Suite 7: slot tests** (`qa-chaos-1`): 3 test cases
10. **Write Suite 8: load tests** (`qa-chaos-2`): 4 test cases
11. **Write Suite 9: consistency tests** (`qa-chaos-2`): 4 test cases

### First run
12. **Run all tests, collect results** (`qa-lead`): Execute, aggregate, report failures
13. **Triage failures** (`architect`): Read code, identify root causes
14. **Fix bugs** (dev-*): Fix identified issues
15. **Verify fixes** (`ops-reviewer`): Re-run failed tests

---

## Persistence & Reuse

- `TEST_PLAN.md` at project root: this document, version-controlled
- `test-results/` directory: JSON reports per iteration, gitignored but archivable
- Test suites are standard `#[tokio::test]` — runnable with `cargo test`
- Individual suites runnable: `cargo test --test e2e_pubsub_test`
- Full collection: `cargo test --test 'e2e_*' --test 'chaos_*'`
