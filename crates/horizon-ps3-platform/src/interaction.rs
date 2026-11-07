//! Deterministic block interaction helpers shared by the PS3 client and tests.

use crate::terrain;
use crate::world::World;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BlockRayHit {
    pub solid: [i32; 3],
    pub place: [i32; 3],
}

/// Samples a bounded ray through resident chunks and returns the first solid
/// voxel plus the last empty voxel before it. A missing chunk stops the ray:
/// interaction never edits data that has not been streamed into memory.
pub fn cast_block_ray<const CAPACITY: usize, const QUEUE: usize>(
    world: &World<CAPACITY, QUEUE>,
    origin: [f32; 3],
    direction: [f32; 3],
    reach: f32,
    step: f32,
) -> Option<BlockRayHit> {
    if !reach.is_finite() || !step.is_finite() || reach < 0.0 || step <= 0.0 {
        return None;
    }
    let mut previous = [
        floor_i32(origin[0]),
        floor_i32(origin[1]),
        floor_i32(origin[2]),
    ];
    let mut distance = 0.0;
    while distance <= reach {
        let point = [
            origin[0] + direction[0] * distance,
            origin[1] + direction[1] * distance,
            origin[2] + direction[2] * distance,
        ];
        let cell = [
            floor_i32(point[0]),
            floor_i32(point[1]),
            floor_i32(point[2]),
        ];
        if cell != previous {
            let Some(block) = world.block_global(cell[0], cell[1], cell[2]) else {
                return None;
            };
            if block != terrain::AIR {
                return Some(BlockRayHit {
                    solid: cell,
                    place: previous,
                });
            }
            previous = cell;
        }
        distance += step;
    }
    None
}

pub(crate) fn floor_i32(value: f32) -> i32 {
    let truncated = value as i32;
    if (truncated as f32) > value {
        truncated - 1
    } else {
        truncated
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunk_mesh::Face;
    use crate::frame_budget::FrameWorkBudget;
    use crate::greedy_mesh::GreedyQuad;
    use crate::world::World;

    const AIR_QUAD: GreedyQuad = GreedyQuad {
        x: 0,
        y: 0,
        z: 0,
        width: 0,
        height: 0,
        block_id: 0,
        face: Face::NegativeX,
    };

    fn resident_world() -> World<1, 4> {
        let mut world = World::new(7);
        world.stream_around(0, 0, 0, u16::MAX);
        let mut scratch = [AIR_QUAD; 8192];
        let mut budget = FrameWorkBudget::new(u16::MAX, u32::MAX);
        world.pump(&mut budget, &mut scratch, |_, _| {});
        world
    }

    #[test]
    fn finds_solid_and_previous_empty_cell() {
        let world = resident_world();
        let hit = cast_block_ray(&world, [2.5, 15.5, 0.5], [0.0, 0.0, 1.0], 4.0, 0.1);
        assert_eq!(
            hit,
            Some(BlockRayHit {
                solid: [2, 15, 1],
                place: [2, 15, 0],
            })
        );
    }

    #[test]
    fn negative_coordinates_use_floor_not_truncation() {
        assert_eq!(floor_i32(-0.01), -1);
        assert_eq!(floor_i32(-1.0), -1);
        assert_eq!(floor_i32(1.99), 1);
    }

    #[test]
    fn invalid_ray_parameters_are_rejected() {
        let world = resident_world();
        assert_eq!(cast_block_ray(&world, [0.0; 3], [0.0; 3], 4.0, 0.0), None);
        assert_eq!(cast_block_ray(&world, [0.0; 3], [0.0; 3], -1.0, 0.25), None);
    }
}
