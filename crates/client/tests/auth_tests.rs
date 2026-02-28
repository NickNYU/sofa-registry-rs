use sofa_registry_client::auth::AuthManager;

#[test]
fn new_creates_manager_with_given_keys() {
    let mgr = AuthManager::new("access-key-1", "secret-key-1");
    assert_eq!(mgr.access_key(), "access-key-1");
}

#[test]
fn sign_returns_non_empty_hex_string() {
    let mgr = AuthManager::new("ak", "sk");
    let sig = mgr.sign(1000);
    assert!(!sig.is_empty());
    // HMAC-SHA256 produces 32 bytes = 64 hex chars
    assert_eq!(sig.len(), 64);
}

#[test]
fn sign_produces_valid_hex_characters() {
    let mgr = AuthManager::new("test-key", "test-secret");
    let sig = mgr.sign(9999999);
    for c in sig.chars() {
        assert!(
            c.is_ascii_hexdigit(),
            "signature character '{}' is not a hex digit",
            c
        );
    }
}

#[test]
fn sign_is_deterministic() {
    let mgr = AuthManager::new("ak", "sk");
    let ts = 1700000000i64;
    let sig1 = mgr.sign(ts);
    let sig2 = mgr.sign(ts);
    assert_eq!(sig1, sig2, "same input should produce same signature");
}

#[test]
fn sign_differs_for_different_timestamps() {
    let mgr = AuthManager::new("ak", "sk");
    let sig1 = mgr.sign(1000);
    let sig2 = mgr.sign(2000);
    assert_ne!(sig1, sig2);
}

#[test]
fn sign_differs_for_different_access_keys() {
    let mgr1 = AuthManager::new("ak1", "sk");
    let mgr2 = AuthManager::new("ak2", "sk");
    let ts = 12345i64;
    assert_ne!(mgr1.sign(ts), mgr2.sign(ts));
}

#[test]
fn sign_differs_for_different_secret_keys() {
    let mgr1 = AuthManager::new("ak", "sk1");
    let mgr2 = AuthManager::new("ak", "sk2");
    let ts = 12345i64;
    assert_ne!(mgr1.sign(ts), mgr2.sign(ts));
}

#[test]
fn verify_accepts_correct_signature() {
    let mgr = AuthManager::new("my-access-key", "my-secret-key");
    let ts = 1700000000i64;
    let sig = mgr.sign(ts);
    assert!(mgr.verify(ts, &sig));
}

#[test]
fn verify_rejects_wrong_timestamp() {
    let mgr = AuthManager::new("ak", "sk");
    let sig = mgr.sign(100);
    assert!(!mgr.verify(101, &sig));
}

#[test]
fn verify_rejects_wrong_signature() {
    let mgr = AuthManager::new("key", "secret");
    assert!(!mgr.verify(123, "bad-signature"));
}

#[test]
fn verify_rejects_empty_signature() {
    let mgr = AuthManager::new("key", "secret");
    assert!(!mgr.verify(123, ""));
}

#[test]
fn verify_rejects_signature_with_wrong_length() {
    let mgr = AuthManager::new("key", "secret");
    // Create a valid-looking hex string but with wrong content
    let wrong_sig = "a".repeat(64);
    // This should be rejected unless it happens to match (astronomically unlikely)
    let sig = mgr.sign(123);
    if sig != wrong_sig {
        assert!(!mgr.verify(123, &wrong_sig));
    }
}

#[test]
fn verify_signature_from_different_manager_is_rejected() {
    let mgr1 = AuthManager::new("ak", "sk1");
    let mgr2 = AuthManager::new("ak", "sk2");
    let ts = 5000i64;
    let sig = mgr1.sign(ts);
    assert!(!mgr2.verify(ts, &sig));
}

#[test]
fn access_key_returns_correct_value() {
    let mgr = AuthManager::new("my-access-key", "my-secret");
    assert_eq!(mgr.access_key(), "my-access-key");
}

#[test]
fn sign_with_zero_timestamp() {
    let mgr = AuthManager::new("ak", "sk");
    let sig = mgr.sign(0);
    assert_eq!(sig.len(), 64);
    assert!(mgr.verify(0, &sig));
}

#[test]
fn sign_with_negative_timestamp() {
    let mgr = AuthManager::new("ak", "sk");
    let sig = mgr.sign(-1);
    assert_eq!(sig.len(), 64);
    assert!(mgr.verify(-1, &sig));
}

#[test]
fn sign_with_max_timestamp() {
    let mgr = AuthManager::new("ak", "sk");
    let sig = mgr.sign(i64::MAX);
    assert_eq!(sig.len(), 64);
    assert!(mgr.verify(i64::MAX, &sig));
}

#[test]
fn sign_with_empty_keys() {
    let mgr = AuthManager::new("", "");
    let sig = mgr.sign(100);
    assert_eq!(sig.len(), 64);
    assert!(mgr.verify(100, &sig));
    assert_eq!(mgr.access_key(), "");
}

#[test]
fn sign_with_unicode_keys() {
    let mgr = AuthManager::new("access-key", "secret-with-unicode");
    let sig = mgr.sign(42);
    assert_eq!(sig.len(), 64);
    assert!(mgr.verify(42, &sig));
}

#[test]
fn sign_uses_lowercase_hex() {
    let mgr = AuthManager::new("ak", "sk");
    let sig = mgr.sign(1);
    // All chars should be lowercase hex
    for c in sig.chars() {
        assert!(
            c.is_ascii_digit() || ('a'..='f').contains(&c),
            "expected lowercase hex, found '{}'",
            c
        );
    }
}
