use std::collections::HashMap;

/// Server environment information
#[derive(Debug, Clone)]
pub struct ServerEnv {
    pub hostname: String,
    pub ip: String,
    pub process_id: String,
    pub start_time: i64,
    pub properties: HashMap<String, String>,
}

impl ServerEnv {
    pub fn new(ip: &str) -> Self {
        let hostname = get_hostname();
        let pid = std::process::id();
        let start_time = chrono::Utc::now().timestamp_millis();
        let process_id = format!("{}-{}-{}", ip, start_time, pid);

        Self {
            hostname,
            ip: ip.to_string(),
            process_id,
            start_time,
            properties: HashMap::new(),
        }
    }
}

/// Get the hostname from environment variables with a fallback.
fn get_hostname() -> String {
    std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("HOST"))
        .unwrap_or_else(|_| "unknown".to_string())
}
