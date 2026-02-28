use sofa_registry_core::error::{RegistryError, Result};

// ===========================================================================
// RegistryError Display tests
// ===========================================================================

#[test]
fn error_slot_table_display() {
    let err = RegistryError::SlotTable("slot not found".to_string());
    assert_eq!(err.to_string(), "Slot table error: slot not found");
}

#[test]
fn error_storage_display() {
    let err = RegistryError::Storage("disk full".to_string());
    assert_eq!(err.to_string(), "Storage error: disk full");
}

#[test]
fn error_database_display() {
    let err = RegistryError::Database("connection refused".to_string());
    assert_eq!(err.to_string(), "Database error: connection refused");
}

#[test]
fn error_remoting_display() {
    let err = RegistryError::Remoting("timeout".to_string());
    assert_eq!(err.to_string(), "Remoting error: timeout");
}

#[test]
fn error_config_display() {
    let err = RegistryError::Config("missing field".to_string());
    assert_eq!(err.to_string(), "Config error: missing field");
}

#[test]
fn error_auth_display() {
    let err = RegistryError::Auth("invalid token".to_string());
    assert_eq!(err.to_string(), "Auth error: invalid token");
}

#[test]
fn error_not_leader_display() {
    let err = RegistryError::NotLeader;
    assert_eq!(err.to_string(), "Not leader");
}

#[test]
fn error_slot_moved_display() {
    let err = RegistryError::SlotMoved {
        slot_id: 42,
        new_leader: "10.0.0.2".to_string(),
    };
    assert_eq!(err.to_string(), "Slot moved: slot 42 is now at 10.0.0.2");
}

#[test]
fn error_slot_access_denied_display() {
    let err = RegistryError::SlotAccessDenied("not authorized".to_string());
    assert_eq!(err.to_string(), "Slot access denied: not authorized");
}

#[test]
fn error_refused_display() {
    let err = RegistryError::Refused("rate limited".to_string());
    assert_eq!(err.to_string(), "Registration refused: rate limited");
}

#[test]
fn error_duplicate_display() {
    let err = RegistryError::Duplicate("reg-123".to_string());
    assert_eq!(err.to_string(), "Duplicate registration: reg-123");
}

#[test]
fn error_not_found_display() {
    let err = RegistryError::NotFound("service XYZ".to_string());
    assert_eq!(err.to_string(), "Not found: service XYZ");
}

#[test]
fn error_timeout_display() {
    let err = RegistryError::Timeout("operation took too long".to_string());
    assert_eq!(err.to_string(), "Timeout: operation took too long");
}

#[test]
fn error_connection_display() {
    let err = RegistryError::Connection("peer closed".to_string());
    assert_eq!(err.to_string(), "Connection error: peer closed");
}

#[test]
fn error_internal_display() {
    let err = RegistryError::Internal("unexpected state".to_string());
    assert_eq!(err.to_string(), "Internal error: unexpected state");
}

#[test]
fn error_io_display() {
    let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file missing");
    let err = RegistryError::Io(io_err);
    assert_eq!(err.to_string(), "IO error: file missing");
}

// ===========================================================================
// RegistryError implements std::error::Error
// ===========================================================================

#[test]
fn error_is_std_error() {
    fn assert_error<T: std::error::Error>() {}
    assert_error::<RegistryError>();
}

// ===========================================================================
// From<io::Error> conversion
// ===========================================================================

#[test]
fn error_from_io_error() {
    let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "access denied");
    let registry_err: RegistryError = io_err.into();
    match registry_err {
        RegistryError::Io(ref e) => {
            assert_eq!(e.kind(), std::io::ErrorKind::PermissionDenied);
        }
        _ => panic!("expected Io variant"),
    }
}

// ===========================================================================
// Result type alias tests
// ===========================================================================

#[test]
fn result_ok() {
    let result: Result<i32> = Ok(42);
    assert_eq!(result.unwrap(), 42);
}

#[test]
fn result_err() {
    let result: Result<i32> = Err(RegistryError::NotLeader);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.to_string(), "Not leader");
}

#[test]
fn result_with_question_mark_propagation() {
    fn fallible() -> Result<String> {
        Err(RegistryError::NotFound("test".to_string()))
    }

    fn caller() -> Result<String> {
        let val = fallible()?;
        Ok(val)
    }

    let result = caller();
    assert!(result.is_err());
    match result.unwrap_err() {
        RegistryError::NotFound(msg) => assert_eq!(msg, "test"),
        other => panic!("expected NotFound, got: {:?}", other),
    }
}

// ===========================================================================
// Debug formatting
// ===========================================================================

#[test]
fn error_debug_format() {
    let err = RegistryError::SlotMoved {
        slot_id: 10,
        new_leader: "10.0.0.5".to_string(),
    };
    let debug_str = format!("{:?}", err);
    assert!(debug_str.contains("SlotMoved"));
    assert!(debug_str.contains("10"));
    assert!(debug_str.contains("10.0.0.5"));
}

#[test]
fn error_not_leader_debug() {
    let err = RegistryError::NotLeader;
    let debug_str = format!("{:?}", err);
    assert!(debug_str.contains("NotLeader"));
}

// ===========================================================================
// All variants can be constructed
// ===========================================================================

#[test]
fn all_error_variants_constructable() {
    // Verify that every variant can be created and displayed without panicking
    let errors: Vec<RegistryError> = vec![
        RegistryError::SlotTable("a".to_string()),
        RegistryError::Storage("b".to_string()),
        RegistryError::Database("c".to_string()),
        RegistryError::Remoting("d".to_string()),
        RegistryError::Config("e".to_string()),
        RegistryError::Auth("f".to_string()),
        RegistryError::NotLeader,
        RegistryError::SlotMoved {
            slot_id: 0,
            new_leader: "x".to_string(),
        },
        RegistryError::SlotAccessDenied("g".to_string()),
        RegistryError::Refused("h".to_string()),
        RegistryError::Duplicate("i".to_string()),
        RegistryError::NotFound("j".to_string()),
        RegistryError::Timeout("k".to_string()),
        RegistryError::Connection("l".to_string()),
        RegistryError::Internal("m".to_string()),
        RegistryError::Io(std::io::Error::new(std::io::ErrorKind::Other, "n")),
    ];

    for (i, err) in errors.iter().enumerate() {
        let display = err.to_string();
        assert!(!display.is_empty(), "error variant {} has empty display", i);
    }
}
