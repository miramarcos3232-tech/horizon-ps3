//! Small, allocation-free grounded-body simulation for the survival controller.
//!
//! This module is deliberately independent from the free-fly camera so the
//! collision rules can be tested before they replace the bring-up movement.

use crate::terrain;
use crate::world::World;

const GRAVITY: f32 = 24.0;
const JUMP_SPEED: f32 = 8.5;
const EYE_HEIGHT: f32 = 1.62;
const BODY_RADIUS: f32 = 0.30;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GroundedBody {
    pub position: [f32; 3],
    pub vertical_velocity: f32,
    pub grounded: bool,
}

impl GroundedBody {
    pub const fn new(position: [f32; 3]) -> Self {
        Self {
            position,
            vertical_velocity: 0.0,
            grounded: false,
        }
    }

    /// Returns whether a unit block would overlap the player's capsule-like
    /// body. Used by block placement to prevent trapping the player inside a
    /// newly placed voxel.
    pub fn overlaps_block(&self, x: i32, y: i32, z: i32) -> bool {
        let foot = self.position[1] - EYE_HEIGHT;
        let body_top = foot + EYE_HEIGHT;
        let block_x = x as f32;
        let block_y = y as f32;
        let block_z = z as f32;
        self.position[0] + BODY_RADIUS > block_x
            && self.position[0] - BODY_RADIUS < block_x + 1.0
            && self.position[2] + BODY_RADIUS > block_z
            && self.position[2] - BODY_RADIUS < block_z + 1.0
            && body_top > block_y
            && foot < block_y + 1.0
    }

    /// Resets the body after falling below the playable world. Returns true
    /// only when a respawn occurred.
    pub fn respawn_if_void(&mut self, spawn: [f32; 3]) -> bool {
        if self.position[1] >= 0.0 {
            return false;
        }
        self.position = spawn;
        self.vertical_velocity = 0.0;
        self.grounded = false;
        true
    }

    /// Advances vertical physics and horizontal movement against resident blocks.
    /// Missing chunks are treated as non-solid, so streaming remains responsible
    /// for making a region interactable.
    pub fn step<const CAPACITY: usize, const QUEUE: usize>(
        &mut self,
        world: &World<CAPACITY, QUEUE>,
        horizontal: [f32; 2],
        jump_pressed: bool,
        dt: f32,
    ) {
        if !dt.is_finite() || dt <= 0.0 {
            return;
        }
        let dt = dt.min(0.1);
        let next_x = self.position[0] + horizontal[0] * dt;
        let next_z = self.position[2] + horizontal[1] * dt;
        let foot = self.position[1] - EYE_HEIGHT;
        if can_occupy(world, next_x, foot, self.position[2]) {
            self.position[0] = next_x;
        }
        if can_occupy(world, self.position[0], foot, next_z) {
            self.position[2] = next_z;
        }

        if jump_pressed && self.grounded {
            self.vertical_velocity = JUMP_SPEED;
            self.grounded = false;
        }
        self.vertical_velocity -= GRAVITY * dt;
        let next_y = self.position[1] + self.vertical_velocity * dt;
        if self.vertical_velocity <= 0.0 {
            if let Some(ground_y) = support_height(world, self.position[0], self.position[2]) {
                if next_y <= ground_y {
                    self.position[1] = ground_y;
                    self.vertical_velocity = 0.0;
                    self.grounded = true;
                    return;
                }
            }
        }
        self.position[1] = next_y.max(0.0);
        self.grounded = false;
    }
}

fn solid<const CAPACITY: usize, const QUEUE: usize>(
    world: &World<CAPACITY, QUEUE>,
    x: i32,
    y: i32,
    z: i32,
) -> bool {
    world
        .block_global(x, y, z)
        .is_some_and(|block| block != terrain::AIR)
}

fn can_occupy<const CAPACITY: usize, const QUEUE: usize>(
    world: &World<CAPACITY, QUEUE>,
    x: f32,
    foot: f32,
    z: f32,
) -> bool {
    let min_x = (x - BODY_RADIUS) as i32;
    let max_x = (x + BODY_RADIUS) as i32;
    let min_z = (z - BODY_RADIUS) as i32;
    let max_z = (z + BODY_RADIUS) as i32;
    let foot_y = foot as i32;
    let head_y = (foot + EYE_HEIGHT - 0.1) as i32;
    !solid(world, min_x, foot_y, min_z)
        && !solid(world, max_x, foot_y, max_z)
        && !solid(world, min_x, head_y, min_z)
        && !solid(world, max_x, head_y, max_z)
}

fn support_height<const CAPACITY: usize, const QUEUE: usize>(
    world: &World<CAPACITY, QUEUE>,
    x: f32,
    z: f32,
) -> Option<f32> {
    let cells = [
        ((x - BODY_RADIUS) as i32, (z - BODY_RADIUS) as i32),
        ((x + BODY_RADIUS) as i32, (z + BODY_RADIUS) as i32),
        ((x - BODY_RADIUS) as i32, (z + BODY_RADIUS) as i32),
        ((x + BODY_RADIUS) as i32, (z - BODY_RADIUS) as i32),
    ];
    for block_y in (0..(terrain::CHUNK_LAYERS * 16)).rev() {
        if cells
            .iter()
            .all(|&(cell_x, cell_z)| solid(world, cell_x, block_y as i32, cell_z))
        {
            return Some(block_y as f32 + 1.0 + EYE_HEIGHT);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame_budget::FrameWorkBudget;
    use crate::greedy_mesh::GreedyQuad;
    use crate::world::World;

    fn resident_world() -> World<1, 4> {
        let mut world = World::new(7);
        world.stream_around(0, 0, 0, u16::MAX);
        let mut scratch = [GreedyQuad {
            x: 0,
            y: 0,
            z: 0,
            width: 0,
            height: 0,
            block_id: 0,
            face: crate::chunk_mesh::Face::NegativeX,
        }; 8192];
        let mut budget = FrameWorkBudget::new(u16::MAX, u32::MAX);
        world.pump(&mut budget, &mut scratch, |_, _| {});
        world
    }

    #[test]
    fn body_falls_and_lands_on_generated_terrain() {
        let world = resident_world();
        let mut body = GroundedBody::new([2.5, 50.0, 0.5]);
        for _ in 0..240 {
            body.step(&world, [0.0, 0.0], false, 1.0 / 60.0);
        }
        assert!(body.grounded);
        assert!(body.position[1] < 50.0);
        assert_eq!(body.vertical_velocity, 0.0);
    }

    #[test]
    fn jump_is_bounded_and_returns_to_ground() {
        let world = resident_world();
        let mut body = GroundedBody::new([2.5, 10.0, 0.5]);
        for _ in 0..240 {
            body.step(&world, [0.0, 0.0], false, 1.0 / 60.0);
        }
        assert!(body.grounded);
        let start = body.position[1];
        body.step(&world, [0.0, 0.0], true, 1.0 / 60.0);
        assert!(body.position[1] > start);
        for _ in 0..240 {
            body.step(&world, [0.0, 0.0], false, 1.0 / 60.0);
        }
        assert!(body.grounded);
        assert!((body.position[1] - start).abs() < 0.01);
    }

    #[test]
    fn body_rejects_overlapping_placement_cell() {
        let body = GroundedBody::new([2.5, 3.0, 2.5]);
        assert!(body.overlaps_block(2, 1, 2));
        assert!(!body.overlaps_block(8, 1, 8));
        assert!(!body.overlaps_block(2, 4, 2));
    }

    #[test]
    fn body_respawns_after_falling_below_world() {
        let mut body = GroundedBody::new([2.5, -1.0, 2.5]);
        assert!(body.respawn_if_void([8.0, 50.0, 8.0]));
        assert_eq!(body.position, [8.0, 50.0, 8.0]);
        assert_eq!(body.vertical_velocity, 0.0);
        assert!(!body.respawn_if_void([1.0, 1.0, 1.0]));
    }
}
