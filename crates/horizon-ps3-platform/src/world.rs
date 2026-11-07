//! Streaming world for the PS3 client: terrain -> chunk store -> greedy mesher.
//!
//! `World` owns a fixed-capacity [`ChunkStore`] and a bounded [`MeshQueue`].
//! Each frame the client calls [`World::stream_around`] with the player's chunk
//! (loads nearest chunks first, evicts distant ones) and then [`World::pump`]
//! with that frame's [`FrameWorkBudget`]; `pump` hands every freshly meshed
//! chunk to a sink that uploads or draws it. Nothing allocates, and the work
//! done per call is bounded by the arguments, never by world size.
//!
//! The struct is large (about 11.7 KB per chunk slot). On the console keep it
//! in a `static` or XDR block rather than on the stack.

use crate::chunk_mesh::{MeshQueue, MeshRequest};
use crate::chunk_store::ChunkStore;
use crate::frame_budget::FrameWorkBudget;
use crate::greedy_mesh::{GreedyQuad, mesh_opaque_greedy};
use crate::terrain::{self, CHUNK_LAYERS};

/// What one call to [`World::stream_around`] did.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct StreamStats {
    pub loaded: u16,
    pub evicted: u16,
    /// Chunks wanted but not loaded because the store was full.
    pub rejected: u16,
}

/// What one call to [`World::pump`] did.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PumpStats {
    pub meshed: u16,
    pub quads: u32,
    /// Chunks whose mesh did not fit the caller's scratch buffer.
    pub failed: u16,
    /// Chunks still waiting (queued or dirty) after this call.
    pub pending: u32,
}

pub struct World<const CAPACITY: usize, const QUEUE: usize> {
    store: ChunkStore<CAPACITY>,
    queue: MeshQueue<QUEUE>,
    seed: u32,
}

impl<const CAPACITY: usize, const QUEUE: usize> World<CAPACITY, QUEUE> {
    pub const fn new(seed: u32) -> Self {
        Self {
            store: ChunkStore::new(),
            queue: MeshQueue::new(),
            seed,
        }
    }

    pub const fn seed(&self) -> u32 {
        self.seed
    }

    pub fn store(&self) -> &ChunkStore<CAPACITY> {
        &self.store
    }

    /// Reads a block only when its chunk is currently resident.
    pub fn block_global(&self, wx: i32, wy: i32, wz: i32) -> Option<u16> {
        let edge = crate::chunk_mesh::CHUNK_EDGE as i32;
        if !(0..CHUNK_LAYERS.saturating_mul(edge)).contains(&wy) {
            return None;
        }
        let request = MeshRequest {
            chunk_x: wx.div_euclid(edge),
            chunk_y: wy.div_euclid(edge),
            chunk_z: wz.div_euclid(edge),
            revision: 0,
        };
        let index = crate::chunk_mesh::grid_index(
            wx.rem_euclid(edge) as usize + 1,
            wy.rem_euclid(edge) as usize + 1,
            wz.rem_euclid(edge) as usize + 1,
        );
        self.store.grid(request).map(|grid| grid[index])
    }

    /// Changes one loaded block using world coordinates and marks every mesh
    /// affected by the edit dirty. Boundary edits also update the neighbouring
    /// chunk halo when that neighbour is resident; unloaded neighbours rebuild
    /// their halo from terrain when streamed in later.
    pub fn set_block_global(&mut self, wx: i32, wy: i32, wz: i32, block_id: u16) -> bool {
        let edge = crate::chunk_mesh::CHUNK_EDGE as i32;
        let max_y = CHUNK_LAYERS.saturating_mul(edge);
        if !(0..max_y).contains(&wy) {
            return false;
        }
        let (chunk_x, local_x) = (wx.div_euclid(edge), wx.rem_euclid(edge) as usize);
        let (chunk_y, local_y) = (wy.div_euclid(edge), wy.rem_euclid(edge) as usize);
        let (chunk_z, local_z) = (wz.div_euclid(edge), wz.rem_euclid(edge) as usize);
        let request = MeshRequest {
            chunk_x,
            chunk_y,
            chunk_z,
            revision: 0,
        };
        if !self
            .store
            .set_block(request, local_x, local_y, local_z, block_id)
        {
            return false;
        }
        self.bump_loaded(request);

        if local_x == 0 {
            self.set_halo_and_bump(
                MeshRequest {
                    chunk_x: chunk_x - 1,
                    ..request
                },
                17,
                local_y + 1,
                local_z + 1,
                block_id,
            );
        } else if local_x + 1 == edge as usize {
            self.set_halo_and_bump(
                MeshRequest {
                    chunk_x: chunk_x + 1,
                    ..request
                },
                0,
                local_y + 1,
                local_z + 1,
                block_id,
            );
        }
        if local_y == 0 {
            self.set_halo_and_bump(
                MeshRequest {
                    chunk_y: chunk_y - 1,
                    ..request
                },
                local_x + 1,
                17,
                local_z + 1,
                block_id,
            );
        } else if local_y + 1 == edge as usize {
            self.set_halo_and_bump(
                MeshRequest {
                    chunk_y: chunk_y + 1,
                    ..request
                },
                local_x + 1,
                0,
                local_z + 1,
                block_id,
            );
        }
        if local_z == 0 {
            self.set_halo_and_bump(
                MeshRequest {
                    chunk_z: chunk_z - 1,
                    ..request
                },
                local_x + 1,
                local_y + 1,
                17,
                block_id,
            );
        } else if local_z + 1 == edge as usize {
            self.set_halo_and_bump(
                MeshRequest {
                    chunk_z: chunk_z + 1,
                    ..request
                },
                local_x + 1,
                local_y + 1,
                0,
                block_id,
            );
        }
        true
    }

    fn bump_loaded(&mut self, request: MeshRequest) {
        let _ = self.store.bump_revision(request);
    }

    fn set_halo_and_bump(
        &mut self,
        request: MeshRequest,
        x: usize,
        y: usize,
        z: usize,
        block_id: u16,
    ) {
        if self.store.set_halo(request, x, y, z, block_id) {
            self.bump_loaded(request);
        }
    }

    /// Keeps the chunks within `radius` of `(center_cx, center_cz)` loaded, in
    /// every vertical layer. Chunks farther than `radius + 1` are evicted (the
    /// extra ring avoids reloading chunks when the player crosses a border).
    /// At most `max_new` chunks are generated per call, nearest first; call
    /// again next frame to continue.
    pub fn stream_around(
        &mut self,
        center_cx: i32,
        center_cz: i32,
        radius: i32,
        max_new: u16,
    ) -> StreamStats {
        let mut stats = StreamStats::default();
        let radius = radius.max(0);
        let keep = radius.unsigned_abs().saturating_add(1);
        stats.evicted = self.store.retain(|request| {
            request.chunk_x.wrapping_sub(center_cx).unsigned_abs() <= keep
                && request.chunk_z.wrapping_sub(center_cz).unsigned_abs() <= keep
        });

        let seed = self.seed;
        for ring in 0..=radius {
            for dz in -ring..=ring {
                for dx in -ring..=ring {
                    if dx.abs().max(dz.abs()) != ring {
                        continue;
                    }
                    for cy in 0..CHUNK_LAYERS {
                        let request = MeshRequest {
                            chunk_x: center_cx.wrapping_add(dx),
                            chunk_y: cy,
                            chunk_z: center_cz.wrapping_add(dz),
                            revision: 1,
                        };
                        if self.store.contains(request) {
                            continue;
                        }
                        if stats.loaded >= max_new {
                            return stats;
                        }
                        let result = self.store.fill_with(request, |grid| {
                            terrain::fill_chunk(
                                seed,
                                request.chunk_x,
                                request.chunk_y,
                                request.chunk_z,
                                grid,
                            );
                        });
                        match result {
                            Ok(()) => stats.loaded += 1,
                            Err(_) => stats.rejected = stats.rejected.saturating_add(1),
                        }
                    }
                }
            }
        }
        stats
    }

    /// Meshes dirty chunks until `budget` runs out. For each chunk the sink
    /// receives its request and the greedy quads in `scratch[..n]`; the slice is
    /// reused for the next chunk, so the sink must copy or upload it before
    /// returning. Chunks that no longer exist (evicted while queued) are skipped.
    pub fn pump(
        &mut self,
        budget: &mut FrameWorkBudget,
        scratch: &mut [GreedyQuad],
        mut sink: impl FnMut(MeshRequest, &[GreedyQuad]),
    ) -> PumpStats {
        let mut stats = PumpStats::default();
        // A full queue leaves the rest dirty; they are picked up on a later call.
        let _ = self.store.drain_dirty(&mut self.queue);
        while !self.queue.is_empty() {
            if !budget.try_start_mesh() {
                break;
            }
            let Some(request) = self.queue.pop() else {
                break;
            };
            let Some(grid) = self.store.grid(request) else {
                continue;
            };
            match mesh_opaque_greedy(grid, scratch) {
                Ok(count) => {
                    stats.meshed += 1;
                    stats.quads += count as u32;
                    sink(request, &scratch[..count]);
                }
                Err(_) => stats.failed += 1,
            }
        }
        stats.pending = (self.queue.len() + self.store.dirty_count()) as u32;
        stats
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunk_mesh::Face;

    const BLANK: GreedyQuad = GreedyQuad {
        x: 0,
        y: 0,
        z: 0,
        width: 0,
        height: 0,
        block_id: 0,
        face: Face::NegativeX,
    };

    #[test]
    fn streams_nearest_first_and_meshes_everything() {
        let mut world = World::<27, 32>::new(11);
        let stats = world.stream_around(0, 0, 1, u16::MAX);
        assert_eq!(
            stats,
            StreamStats {
                loaded: 27,
                evicted: 0,
                rejected: 0
            }
        );
        // Streaming again changes nothing.
        assert_eq!(world.stream_around(0, 0, 1, u16::MAX).loaded, 0);

        let mut scratch = [BLANK; 8192];
        let mut budget = FrameWorkBudget::new(u16::MAX, u32::MAX);
        let mut seen = 0_u32;
        let pump = world.pump(&mut budget, &mut scratch, |_, quads| {
            assert!(quads.iter().all(|q| q.width >= 1 && q.height >= 1));
            seen += 1;
        });
        assert_eq!(pump.meshed, 27);
        assert_eq!(seen, 27);
        assert_eq!(pump.failed, 0);
        assert_eq!(pump.pending, 0);
        assert!(pump.quads > 27, "surface chunks must produce geometry");

        let mut budget = FrameWorkBudget::new(u16::MAX, u32::MAX);
        let again = world.pump(&mut budget, &mut scratch, |_, _| panic!("nothing is dirty"));
        assert_eq!(again, PumpStats::default());
    }

    #[test]
    fn frame_budget_spreads_meshing_over_frames() {
        let mut world = World::<27, 32>::new(2);
        world.stream_around(0, 0, 1, u16::MAX);
        let mut scratch = [BLANK; 8192];
        let (mut total, mut frames) = (0_u32, 0);
        loop {
            let mut budget = FrameWorkBudget::new(5, u32::MAX);
            let pump = world.pump(&mut budget, &mut scratch, |_, _| {});
            assert!(pump.meshed <= 5);
            total += u32::from(pump.meshed);
            frames += 1;
            if pump.pending == 0 {
                break;
            }
            assert!(frames < 20, "meshing must make progress");
        }
        assert_eq!(total, 27);
        assert_eq!(frames, 6);
    }

    #[test]
    fn small_queue_defers_but_never_loses_chunks() {
        let mut world = World::<27, 4>::new(3);
        world.stream_around(0, 0, 1, u16::MAX);
        let mut scratch = [BLANK; 8192];
        let mut total = 0_u32;
        for _ in 0..30 {
            let mut budget = FrameWorkBudget::new(u16::MAX, u32::MAX);
            total += u32::from(world.pump(&mut budget, &mut scratch, |_, _| {}).meshed);
        }
        assert_eq!(total, 27);
    }

    #[test]
    fn moving_far_evicts_and_reloads() {
        let mut world = World::<27, 32>::new(4);
        world.stream_around(0, 0, 1, u16::MAX);
        let stats = world.stream_around(5, 0, 1, u16::MAX);
        assert_eq!(stats.evicted, 27);
        assert_eq!(stats.loaded, 27);
        assert_eq!(stats.rejected, 0);
        // One chunk step stays inside the keep ring: nothing is evicted.
        let stats = world.stream_around(6, 0, 1, u16::MAX);
        assert_eq!(stats.evicted, 0);
    }

    #[test]
    fn max_new_bounds_generation_per_call() {
        let mut world = World::<27, 32>::new(5);
        assert_eq!(world.stream_around(0, 0, 1, 4).loaded, 4);
        let mut loaded = 4;
        for _ in 0..10 {
            loaded += world.stream_around(0, 0, 1, 4).loaded;
        }
        assert_eq!(loaded, 27);
    }

    #[test]
    fn full_store_reports_rejected_chunks() {
        let mut world = World::<5, 8>::new(6);
        let stats = world.stream_around(0, 0, 1, u16::MAX);
        assert_eq!(stats.loaded, 5);
        assert_eq!(stats.rejected, 22);
    }

    #[test]
    fn undersized_scratch_is_reported_not_written_partially() {
        let mut world = World::<3, 4>::new(8);
        world.stream_around(0, 0, 0, u16::MAX);
        let mut scratch = [BLANK; 1];
        let mut budget = FrameWorkBudget::new(u16::MAX, u32::MAX);
        let pump = world.pump(&mut budget, &mut scratch, |_, _| {});
        assert!(pump.failed >= 1);
        assert_eq!(u32::from(pump.meshed) + u32::from(pump.failed), 3);
    }

    #[test]
    fn edits_loaded_block_and_marks_its_chunk_dirty() {
        let mut world = World::<1, 4>::new(9);
        let request = MeshRequest {
            chunk_x: 0,
            chunk_y: 0,
            chunk_z: 0,
            revision: 1,
        };
        world.stream_around(0, 0, 0, 1);
        let mut scratch = [BLANK; 8192];
        let mut budget = FrameWorkBudget::new(u16::MAX, u32::MAX);
        world.pump(&mut budget, &mut scratch, |_, _| {});
        assert!(world.set_block_global(0, 1, 0, 9));
        assert_eq!(
            world.store().grid(request).unwrap()[crate::chunk_mesh::grid_index(1, 2, 1)],
            9
        );
        let mut revision = 0;
        let mut budget = FrameWorkBudget::new(u16::MAX, u32::MAX);
        world.pump(&mut budget, &mut scratch, |request, _| {
            revision = request.revision
        });
        assert_eq!(revision, 2);
    }

    #[test]
    fn boundary_edit_updates_loaded_neighbour_halo() {
        let mut world = World::<27, 32>::new(10);
        world.stream_around(0, 0, 1, u16::MAX);
        let mut scratch = [BLANK; 8192];
        let mut budget = FrameWorkBudget::new(u16::MAX, u32::MAX);
        world.pump(&mut budget, &mut scratch, |_, _| {});
        assert!(world.set_block_global(15, 4, 2, 7));
        let neighbour = MeshRequest {
            chunk_x: 1,
            chunk_y: 0,
            chunk_z: 0,
            revision: 1,
        };
        assert_eq!(
            world.store().grid(neighbour).unwrap()[crate::chunk_mesh::grid_index(0, 5, 3)],
            7
        );
    }

    #[test]
    fn edit_rejects_heights_outside_loaded_vertical_layers() {
        let mut world = World::<1, 1>::new(11);
        world.stream_around(0, 0, 0, 1);
        assert!(!world.set_block_global(0, -1, 0, 1));
        assert!(!world.set_block_global(0, CHUNK_LAYERS * 16, 0, 1));
    }
}
