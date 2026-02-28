/// Server-side data box with bytes.
#[derive(Debug, Clone)]
pub struct ServerDataBox {
    pub data: bytes::Bytes,
    pub encoding: Option<String>,
}

impl ServerDataBox {
    pub fn new(data: bytes::Bytes) -> Self {
        Self {
            data,
            encoding: None,
        }
    }

    pub fn with_encoding(data: bytes::Bytes, encoding: impl Into<String>) -> Self {
        Self {
            data,
            encoding: Some(encoding.into()),
        }
    }
}
