//! Fixed-capacity chunk storage for the PS3 bring-up path.
//!
//! Each slot owns a 16³ block grid plus its one-voxel halo. Updates mark the
//! slot dirty; the caller drains dirty requests into the bounded mesh queue.

use crate::chunk_mesh::{GRID_VOXELS, HALO_EDGE, MeshQueue, MeshRequest, grid_index};

pub struct ChunkStore<const CAPACITY: usize> {
    requests: [Option<MeshRequest>; CAPACITY],
    grids: [[u16; GRID_VOXELS]; CAPACITY],
    dirty: [bool; CAPACITY],
}

impl<const CAPACITY: usize> ChunkStore<CAPACITY> {
    pub const fn new() -> Self {
        Self {
            requests: [None; CAPACITY],
            grids: [[0; GRID_VOXELS]; CAPACITY],
            dirty: [false; CAPACITY],
        }
    }

    pub const fn capacity(&self) -> usize {
        CAPACITY
    }

    pub fn insert(&mut self, request: MeshRequest) -> Result<(), MeshRequest> {
        if let Some(index) = self.find(request) {
            self.requests[index] = Some(request);
            self.dirty[index] = true;
            return Ok(());
        }
        for index in 0..CAPACITY {
            if self.requests[index].is_none() {
                self.requests[index] = Some(request);
                self.dirty[index] = true;
                return Ok(());
            }
        }
        Err(request)
    }

    pub fn set_block(
        &mut self,
        request: MeshRequest,
        x: usize,
        y: usize,
        z: usize,
        block_id: u16,
    ) -> bool {
        if x >= crate::chunk_mesh::CHUNK_EDGE
            || y >= crate::chunk_mesh::CHUNK_EDGE
            || z >= crate::chunk_mesh::CHUNK_EDGE
        {
            return false;
        }
        self.set_cell(request, x + 1, y + 1, z + 1, block_id)
    }

    pub fn set_halo(
        &mut self,
        request: MeshRequest,
        x: usize,
        y: usize,
        z: usize,
        block_id: u16,
    ) -> bool {
        self.set_cell(request, x, y, z, block_id)
    }

    /// Inserts (or refreshes) `request` and lets `fill` write its whole haloed
    /// grid in one pass. The chunk is marked dirty. Fails only when the store is
    /// full and the chunk is not already present.
    pub fn fill_with(
        &mut self,
        request: MeshRequest,
        fill: impl FnOnce(&mut [u16; GRID_VOXELS]),
    ) -> Result<(), MeshRequest> {
        self.insert(request)?;
        let index = self
            .find(request)
            .expect("chunk was inserted on the line above");
        fill(&mut self.grids[index]);
        Ok(())
    }

    pub fn contains(&self, request: MeshRequest) -> bool {
        self.find(request).is_some()
    }

    /// Number of chunks waiting to be queued for meshing.
    pub fn dirty_count(&self) -> usize {
        self.dirty.iter().filter(|&&flag| flag).count()
    }

    /// Unloads every chunk for which `keep` returns `false`, clearing its grid.
    /// Returns how many slots were freed.
    pub fn retain(&mut self, mut keep: impl FnMut(MeshRequest) -> bool) -> u16 {
        let mut freed = 0;
        for index in 0..CAPACITY {
            let Some(request) = self.requests[index] else {
                continue;
            };
            if !keep(request) {
                self.requests[index] = None;
                self.dirty[index] = false;
                self.grids[index].fill(0);
                freed += 1;
            }
        }
        freed
    }

    pub fn grid(&self, request: MeshRequest) -> Option<&[u16; GRID_VOXELS]> {
        self.find(request).map(|index| &self.grids[index])
    }

    pub fn bump_revision(&mut self, request: MeshRequest) -> Option<MeshRequest> {
        let index = self.find(request)?;
        let current = self.requests[index]?;
        let updated = MeshRequest {
            revision: current.revision.saturating_add(1),
            ..current
        };
        self.requests[index] = Some(updated);
        self.dirty[index] = true;
        Some(updated)
    }

    pub fn take_dirty(&mut self) -> Option<MeshRequest> {
        for index in 0..CAPACITY {
            if self.dirty[index] {
                self.dirty[index] = false;
                return self.requests[index];
            }
        }
        None
    }

    pub fn drain_dirty<const QUEUE: usize>(
        &mut self,
        queue: &mut MeshQueue<QUEUE>,
    ) -> Result<usize, MeshRequest> {
        let mut drained = 0;
        for index in 0..CAPACITY {
            if !self.dirty[index] {
                continue;
            }
            let request = self.requests[index].expect("dirty chunk must have a request");
            if queue.push(request).is_err() {
                return Err(request);
            }
            self.dirty[index] = false;
            drained += 1;
        }
        Ok(drained)
    }

    fn set_cell(
        &mut self,
        request: MeshRequest,
        x: usize,
        y: usize,
        z: usize,
        block_id: u16,
    ) -> bool {
        if x >= HALO_EDGE || y >= HALO_EDGE || z >= HALO_EDGE {
            return false;
        }
        let Some(index) = self.find(request) else {
            return false;
        };
        self.grids[index][grid_index(x, y, z)] = block_id;
        self.dirty[index] = true;
        true
    }

    fn find(&self, request: MeshRequest) -> Option<usize> {
        self.requests.iter().position(|stored| {
            stored.is_some_and(|candidate| {
                candidate.chunk_x == request.chunk_x
                    && candidate.chunk_y == request.chunk_y
                    && candidate.chunk_z == request.chunk_z
            })
        })
    }
}

impl<const CAPACITY: usize> Default for ChunkStore<CAPACITY> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(x: i32) -> MeshRequest {
        MeshRequest {
            chunk_x: x,
            chunk_y: 0,
            chunk_z: 0,
            revision: 1,
        }
    }

    #[test]
    fn stores_blocks_and_drains_dirty_revision() {
        let mut store = ChunkStore::<1>::new();
        let item = request(3);
        assert_eq!(store.insert(item), Ok(()));
        assert!(store.set_block(item, 15, 2, 4, 7));
        assert_eq!(store.grid(item).unwrap()[grid_index(16, 3, 5)], 7);
        assert_eq!(store.take_dirty(), Some(item));
        assert_eq!(store.take_dirty(), None);
    }

    #[test]
    fn halo_writes_are_bounded_and_mark_dirty() {
        let mut store = ChunkStore::<1>::new();
        let item = request(0);
        store.insert(item).unwrap();
        assert!(store.set_halo(item, 17, 1, 1, 9));
        assert!(!store.set_halo(item, HALO_EDGE, 1, 1, 9));
        assert_eq!(store.grid(item).unwrap()[grid_index(17, 1, 1)], 9);
    }

    #[test]
    fn rejects_new_chunk_when_capacity_is_full() {
        let mut store = ChunkStore::<1>::new();
        assert!(store.insert(request(0)).is_ok());
        assert_eq!(store.insert(request(1)), Err(request(1)));
    }

    #[test]
    fn bump_revision_only_marks_the_selected_chunk_dirty() {
        let mut store = ChunkStore::<2>::new();
        let first = request(0);
        let second = request(1);
        store.insert(first).unwrap();
        store.insert(second).unwrap();
        assert_eq!(store.take_dirty(), Some(first));
        assert_eq!(store.take_dirty(), Some(second));
        let updated = store.bump_revision(first).unwrap();
        assert_eq!(updated.revision, 2);
        assert_eq!(store.take_dirty(), Some(updated));
        assert_eq!(store.take_dirty(), None);
    }

    #[test]
    fn coalesces_multiple_revisions_into_one_queue_entry() {
        let mut store = ChunkStore::<1>::new();
        let initial = request(0);
        store.insert(initial).unwrap();
        assert_eq!(store.take_dirty(), Some(initial));
        let revision_two = store.bump_revision(initial).unwrap();
        let revision_three = store.bump_revision(revision_two).unwrap();
        let mut queue = MeshQueue::<1>::new();
        assert_eq!(store.drain_dirty(&mut queue), Ok(1));
        assert_eq!(queue.pop(), Some(revision_three));
        assert_eq!(queue.pop(), None);
        assert_eq!(store.take_dirty(), None);
    }

    #[test]
    fn preserves_dirty_when_worker_queue_is_full() {
        let mut store = ChunkStore::<1>::new();
        let item = request(0);
        store.insert(item).unwrap();
        let mut queue = MeshQueue::<0>::new();
        assert_eq!(store.drain_dirty(&mut queue), Err(item));
        assert_eq!(store.take_dirty(), Some(item));
    }

    #[test]
    fn fill_with_retain_and_dirty_count() {
        let mut store = ChunkStore::<2>::new();
        let (a, b) = (request(0), request(1));
        assert_eq!(store.fill_with(a, |grid| grid.fill(5)), Ok(()));
        assert_eq!(store.fill_with(b, |grid| grid.fill(6)), Ok(()));
        assert_eq!(store.fill_with(request(2), |_| {}), Err(request(2)));
        assert_eq!(store.dirty_count(), 2);
        assert!(store.contains(a));
        assert_eq!(store.grid(b).unwrap()[0], 6);
        assert_eq!(store.retain(|r| r.chunk_x != 0), 1);
        assert!(!store.contains(a));
        assert_eq!(store.dirty_count(), 1);
        assert_eq!(store.fill_with(request(2), |grid| grid.fill(7)), Ok(()));
        assert_eq!(store.grid(request(2)).unwrap()[10], 7);
    }
}
