//! Fixed per-frame work limits for the PS3 renderer.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FrameWorkBudget {
    max_meshes: u16,
    max_upload_bytes: u32,
    meshes_started: u16,
    uploaded_bytes: u32,
}

impl FrameWorkBudget {
    pub const fn new(max_meshes: u16, max_upload_bytes: u32) -> Self {
        Self {
            max_meshes,
            max_upload_bytes,
            meshes_started: 0,
            uploaded_bytes: 0,
        }
    }

    pub const fn try_start_mesh(&mut self) -> bool {
        if self.meshes_started >= self.max_meshes {
            return false;
        }
        self.meshes_started += 1;
        true
    }

    pub const fn try_upload(&mut self, bytes: usize) -> bool {
        let bytes = if bytes > u32::MAX as usize {
            return false;
        } else {
            bytes as u32
        };
        let Some(total) = self.uploaded_bytes.checked_add(bytes) else {
            return false;
        };
        if total > self.max_upload_bytes {
            return false;
        }
        self.uploaded_bytes = total;
        true
    }

    pub const fn meshes_started(self) -> u16 {
        self.meshes_started
    }

    pub const fn uploaded_bytes(self) -> u32 {
        self.uploaded_bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limits_mesh_count() {
        let mut budget = FrameWorkBudget::new(2, 100);
        assert!(budget.try_start_mesh());
        assert!(budget.try_start_mesh());
        assert!(!budget.try_start_mesh());
        assert_eq!(budget.meshes_started(), 2);
    }

    #[test]
    fn limits_cumulative_upload_bytes() {
        let mut budget = FrameWorkBudget::new(4, 100);
        assert!(budget.try_upload(60));
        assert!(!budget.try_upload(41));
        assert!(budget.try_upload(40));
        assert_eq!(budget.uploaded_bytes(), 100);
    }

    #[test]
    fn rejects_usize_to_u32_overflow() {
        let mut budget = FrameWorkBudget::new(1, u32::MAX);
        assert!(!budget.try_upload(usize::MAX));
        assert_eq!(budget.uploaded_bytes(), 0);
    }
}
