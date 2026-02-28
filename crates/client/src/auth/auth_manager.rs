use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

/// Manages HMAC-SHA256 authentication for client-to-session communication.
pub struct AuthManager {
    access_key: String,
    secret_key: String,
}

impl AuthManager {
    pub fn new(access_key: &str, secret_key: &str) -> Self {
        Self {
            access_key: access_key.to_string(),
            secret_key: secret_key.to_string(),
        }
    }

    /// Produce an HMAC-SHA256 hex signature for `"access_key:timestamp"`.
    pub fn sign(&self, timestamp: i64) -> String {
        let message = format!("{}:{}", self.access_key, timestamp);
        let mut mac =
            HmacSha256::new_from_slice(self.secret_key.as_bytes()).expect("HMAC key length error");
        mac.update(message.as_bytes());
        let result = mac.finalize();
        hex_encode(result.into_bytes().as_slice())
    }

    /// Verify that `signature` matches the expected HMAC for the given timestamp.
    pub fn verify(&self, timestamp: i64, signature: &str) -> bool {
        let expected = self.sign(timestamp);
        constant_time_eq(expected.as_bytes(), signature.as_bytes())
    }

    pub fn access_key(&self) -> &str {
        &self.access_key
    }
}

/// Constant-time comparison to avoid timing attacks.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Simple hex encoder (avoids pulling in another dependency).
fn hex_encode(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sign_and_verify() {
        let mgr = AuthManager::new("my-access-key", "my-secret-key");
        let ts = 1700000000i64;
        let sig = mgr.sign(ts);
        assert!(!sig.is_empty());
        assert!(mgr.verify(ts, &sig));
        assert!(!mgr.verify(ts + 1, &sig));
    }

    #[test]
    fn wrong_signature_rejected() {
        let mgr = AuthManager::new("key", "secret");
        assert!(!mgr.verify(123, "bad-signature"));
    }
}
