use super::SlotFuncType;

/// Trait for computing which slot a data_info_id maps to
pub trait SlotFunction: Send + Sync {
    fn slot_of(&self, data_info_id: &str, slot_num: u32) -> u32;
}

pub struct Crc32cSlotFunction;

impl SlotFunction for Crc32cSlotFunction {
    fn slot_of(&self, data_info_id: &str, slot_num: u32) -> u32 {
        let hash = crc32c::crc32c(data_info_id.as_bytes());
        hash % slot_num
    }
}

/// Factory to create slot functions
pub fn create_slot_function(func_type: SlotFuncType) -> Box<dyn SlotFunction> {
    match func_type {
        SlotFuncType::Crc32c => Box::new(Crc32cSlotFunction),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crc32c_slot_distribution() {
        let func = Crc32cSlotFunction;
        let slot_num = 256u32;

        // Verify deterministic
        let s1 = func.slot_of(
            "com.example.service#DEFAULT_INSTANCE_ID#DEFAULT_GROUP",
            slot_num,
        );
        let s2 = func.slot_of(
            "com.example.service#DEFAULT_INSTANCE_ID#DEFAULT_GROUP",
            slot_num,
        );
        assert_eq!(s1, s2);

        // Verify in range
        for i in 0..1000 {
            let id = format!("service.{}", i);
            let slot = func.slot_of(&id, slot_num);
            assert!(slot < slot_num, "slot {} must be < {}", slot, slot_num);
        }
    }

    #[test]
    fn test_slot_distribution_spread() {
        let func = Crc32cSlotFunction;
        let slot_num = 256u32;
        let mut counts = vec![0u32; slot_num as usize];
        for i in 0..10000 {
            let id = format!(
                "com.example.service.{}#DEFAULT_INSTANCE_ID#DEFAULT_GROUP",
                i
            );
            let slot = func.slot_of(&id, slot_num);
            counts[slot as usize] += 1;
        }
        // Should have some distribution across slots
        let non_empty = counts.iter().filter(|&&c| c > 0).count();
        assert!(
            non_empty > slot_num as usize / 2,
            "should have reasonable distribution"
        );
    }
}
