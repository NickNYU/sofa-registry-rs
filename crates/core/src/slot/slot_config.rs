use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlotConfig {
    pub slot_num: u32,
    pub slot_replicas: u32,
    pub func: SlotFuncType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SlotFuncType {
    Crc32c,
}

impl Default for SlotConfig {
    fn default() -> Self {
        Self {
            slot_num: 256,
            slot_replicas: 2,
            func: SlotFuncType::Crc32c,
        }
    }
}
