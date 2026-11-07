//! Per-chunk cache of greedy quads, so a chunk is meshed once and then redrawn
//! every frame without touching the world data. Fixed capacity, no allocation.

use crate::chunk_mesh::{Face, MeshRequest};
use crate::greedy_mesh::GreedyQuad;

pub const BLANK_QUAD: GreedyQuad = GreedyQuad {
    x: 0,
    y: 0,
    z: 0,
    width: 0,
    height: 0,
    block_id: 0,
    face: Face::NegativeX,
};

pub struct MeshCache<const SLOTS: usize, const QUADS: usize> {
    keys: [Option<[i32; 3]>; SLOTS],
    counts: [u16; SLOTS],
    quads: [[GreedyQuad; QUADS]; SLOTS],
}

impl<const SLOTS: usize, const QUADS: usize> MeshCache<SLOTS, QUADS> {
    pub const fn new() -> Self {
        assert!(QUADS <= u16::MAX as usize);
        Self {
            keys: [None; SLOTS],
            counts: [0; SLOTS],
            quads: [[BLANK_QUAD; QUADS]; SLOTS],
        }
    }

    pub const fn slots(&self) -> usize {
        SLOTS
    }

    pub fn len(&self) -> usize {
        self.keys.iter().filter(|key| key.is_some()).count()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Stores (or replaces) the mesh of `request`'s chunk. Returns `false` when
    /// it cannot be kept: more quads than a slot holds (any older mesh of that
    /// chunk is dropped so a stale one is never drawn) or no free slot.
    pub fn store(&mut self, request: MeshRequest, quads: &[GreedyQuad]) -> bool {
        let key = [request.chunk_x, request.chunk_y, request.chunk_z];
        if quads.len() > QUADS {
            if let Some(slot) = self.find(key) {
                self.keys[slot] = None;
                self.counts[slot] = 0;
            }
            return false;
        }
        let slot = match self.find(key) {
            Some(slot) => slot,
            None => match self.keys.iter().position(|k| k.is_none()) {
                Some(slot) => slot,
                None => return false,
            },
        };
        self.keys[slot] = Some(key);
        self.counts[slot] = quads.len() as u16;
        self.quads[slot][..quads.len()].copy_from_slice(quads);
        true
    }

    /// Chunk key and quads stored in `slot`, if occupied.
    pub fn get(&self, slot: usize) -> Option<([i32; 3], &[GreedyQuad])> {
        let key = (*self.keys.get(slot)?)?;
        Some((key, &self.quads[slot][..usize::from(self.counts[slot])]))
    }

    /// Drops every chunk for which `keep` returns `false`; returns how many.
    pub fn retain(&mut self, mut keep: impl FnMut([i32; 3]) -> bool) -> u16 {
        let mut dropped = 0;
        for slot in 0..SLOTS {
            if let Some(key) = self.keys[slot] {
                if !keep(key) {
                    self.keys[slot] = None;
                    self.counts[slot] = 0;
                    dropped += 1;
                }
            }
        }
        dropped
    }

    fn find(&self, key: [i32; 3]) -> Option<usize> {
        self.keys.iter().position(|k| *k == Some(key))
    }
}

impl<const SLOTS: usize, const QUADS: usize> Default for MeshCache<SLOTS, QUADS> {
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
            chunk_y: 1,
            chunk_z: -2,
            revision: 1,
        }
    }

    fn quad(id: u16) -> GreedyQuad {
        GreedyQuad {
            block_id: id,
            width: 1,
            height: 1,
            ..BLANK_QUAD
        }
    }

    #[test]
    fn stores_replaces_and_reads_back() {
        let mut cache = MeshCache::<2, 3>::new();
        assert!(cache.is_empty());
        assert!(cache.store(request(0), &[quad(1), quad(2)]));
        assert!(cache.store(request(0), &[quad(9)]));
        assert_eq!(cache.len(), 1);
        let (key, quads) = cache.get(0).unwrap();
        assert_eq!(key, [0, 1, -2]);
        assert_eq!(quads, &[quad(9)]);
        assert!(cache.get(1).is_none());
        assert!(cache.get(5).is_none());
    }

    #[test]
    fn full_cache_and_oversized_meshes_are_refused() {
        let mut cache = MeshCache::<1, 2>::new();
        assert!(cache.store(request(0), &[quad(1)]));
        assert!(!cache.store(request(1), &[quad(1)]));
        // Too many quads: refused, and the stale mesh of that chunk disappears.
        assert!(!cache.store(request(0), &[quad(1), quad(2), quad(3)]));
        assert!(cache.is_empty());
        assert!(cache.store(request(1), &[]));
        assert_eq!(cache.get(0).unwrap().1.len(), 0);
    }

    #[test]
    fn retain_frees_slots_for_reuse() {
        let mut cache = MeshCache::<2, 1>::new();
        cache.store(request(0), &[quad(1)]);
        cache.store(request(5), &[quad(2)]);
        assert_eq!(cache.retain(|key| key[0] < 3), 1);
        assert_eq!(cache.len(), 1);
        assert!(cache.store(request(7), &[quad(3)]));
        assert_eq!(cache.len(), 2);
    }
}
