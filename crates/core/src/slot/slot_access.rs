/// Result of checking slot access (whether a request is targeting the right leader)
#[derive(Debug, Clone)]
pub struct SlotAccess {
    pub slot_id: u32,
    pub status: SlotAccessStatus,
    pub epoch: i64,
    pub leader_epoch: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotAccessStatus {
    Accept,
    Moved,
    MisMatch,
    Migrating,
}

impl SlotAccess {
    pub fn accept(slot_id: u32, epoch: i64, leader_epoch: i64) -> Self {
        Self {
            slot_id,
            status: SlotAccessStatus::Accept,
            epoch,
            leader_epoch,
        }
    }

    pub fn moved(slot_id: u32, epoch: i64, leader_epoch: i64) -> Self {
        Self {
            slot_id,
            status: SlotAccessStatus::Moved,
            epoch,
            leader_epoch,
        }
    }

    pub fn is_accept(&self) -> bool {
        self.status == SlotAccessStatus::Accept
    }

    pub fn is_moved(&self) -> bool {
        self.status == SlotAccessStatus::Moved
    }
}
