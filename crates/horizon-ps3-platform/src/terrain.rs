//! Deterministic, allocation-free terrain for the PS3 bring-up world.
//!
//! Integer-only value noise (no floats, no tables) so every platform produces
//! the same blocks from the same seed. Every cell of a chunk's haloed grid is
//! computed from world coordinates, so a chunk's halo always equals the interior
//! of its neighbour and shared boundary faces are suppressed without any
//! cross-chunk copying.

use crate::chunk_mesh::{CHUNK_EDGE, GRID_VOXELS, HALO_EDGE, grid_index};

pub const AIR: u16 = 0;
pub const STONE: u16 = 1;
pub const DIRT: u16 = 2;
pub const GRASS: u16 = 3;
pub const BEDROCK: u16 = 4;

/// Vertical chunk layers that can contain terrain (`chunk_y` in `0..CHUNK_LAYERS`).
pub const CHUNK_LAYERS: i32 = 3;
/// Highest surface block `surface_height` can return.
pub const MAX_SURFACE: i32 = 42;

const fn hash2(seed: u32, x: i32, z: i32) -> u32 {
    let mut h = seed ^ (x as u32).wrapping_mul(0x9E37_79B1) ^ (z as u32).wrapping_mul(0x85EB_CA77);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^ (h >> 15)
}

/// Smoothstep scaled to `0..=4096` for `t` in `0..=cell`.
const fn smooth(t: i64, cell: i64) -> i64 {
    t * t * (3 * cell - 2 * t) * 4096 / (cell * cell * cell)
}

/// Value noise in `0..=255` on a lattice of `1 << shift` blocks.
fn value_noise(seed: u32, wx: i32, wz: i32, shift: u32) -> i64 {
    let cell = 1_i64 << shift;
    let mask = (1_i32 << shift) - 1;
    let gx = wx >> shift;
    let gz = wz >> shift;
    let sx = smooth(i64::from(wx & mask), cell);
    let sz = smooth(i64::from(wz & mask), cell);
    let corner = |dx: i32, dz: i32| -> i64 {
        i64::from(hash2(seed, gx.wrapping_add(dx), gz.wrapping_add(dz)) & 0xFF)
    };
    let (a, b, c, d) = (corner(0, 0), corner(1, 0), corner(0, 1), corner(1, 1));
    let top = a + (((b - a) * sx) >> 12);
    let bottom = c + (((d - c) * sx) >> 12);
    top + (((bottom - top) * sz) >> 12)
}

/// Height of the topmost solid block in the column at `(wx, wz)`; `1..=MAX_SURFACE`.
pub fn surface_height(seed: u32, wx: i32, wz: i32) -> i32 {
    let broad = value_noise(seed, wx, wz, 5);
    let detail = value_noise(seed.wrapping_add(1), wx, wz, 3);
    (18 + broad * 20 / 256 + detail * 6 / 256) as i32
}

/// Block at world height `wy` in a column whose surface is `height`.
pub const fn block_for(wy: i32, height: i32) -> u16 {
    if wy <= 0 {
        BEDROCK
    } else if wy > height {
        AIR
    } else if wy == height {
        GRASS
    } else if wy >= height - 3 {
        DIRT
    } else {
        STONE
    }
}

/// Fills a whole haloed grid for chunk `(cx, cy, cz)`.
pub fn fill_chunk(seed: u32, cx: i32, cy: i32, cz: i32, grid: &mut [u16; GRID_VOXELS]) {
    let edge = CHUNK_EDGE as i32;
    let base_x = cx.wrapping_mul(edge).wrapping_sub(1);
    let base_y = cy.wrapping_mul(edge).wrapping_sub(1);
    let base_z = cz.wrapping_mul(edge).wrapping_sub(1);
    for z in 0..HALO_EDGE {
        for x in 0..HALO_EDGE {
            let height = surface_height(
                seed,
                base_x.wrapping_add(x as i32),
                base_z.wrapping_add(z as i32),
            );
            for y in 0..HALO_EDGE {
                grid[grid_index(x, y, z)] = block_for(base_y.wrapping_add(y as i32), height);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heights_are_deterministic_and_in_range() {
        for &(x, z) in &[
            (0, 0),
            (-1, -1),
            (1000, -777),
            (i32::MIN / 2, 12345),
            (31, 32),
        ] {
            let h = surface_height(7, x, z);
            assert_eq!(h, surface_height(7, x, z));
            assert!((1..=MAX_SURFACE).contains(&h), "height {h} at {x},{z}");
        }
        let mut differs = false;
        for x in 0..64 {
            differs |= surface_height(1, x, 0) != surface_height(2, x, 0);
        }
        assert!(differs, "different seeds should give different terrain");
    }

    #[test]
    fn terrain_varies_but_is_gently_sloped() {
        let (mut min, mut max) = (i32::MAX, i32::MIN);
        for x in 0..128 {
            let h = surface_height(3, x, 40);
            min = min.min(h);
            max = max.max(h);
            assert!((h - surface_height(3, x + 1, 40)).abs() <= 4);
        }
        assert!(max - min >= 3, "terrain should not be flat");
    }

    #[test]
    fn column_layers_are_ordered() {
        assert_eq!(block_for(-5, 20), BEDROCK);
        assert_eq!(block_for(0, 20), BEDROCK);
        assert_eq!(block_for(5, 20), STONE);
        assert_eq!(block_for(17, 20), DIRT);
        assert_eq!(block_for(20, 20), GRASS);
        assert_eq!(block_for(21, 20), AIR);
    }

    #[test]
    fn halo_matches_neighbour_interior() {
        let mut a = [0_u16; GRID_VOXELS];
        let mut b = [0_u16; GRID_VOXELS];
        fill_chunk(9, 0, 1, 0, &mut a);
        fill_chunk(9, 1, 1, 0, &mut b);
        for z in 0..HALO_EDGE {
            for y in 0..HALO_EDGE {
                // a's +X halo column is b's first interior column, and vice versa.
                assert_eq!(a[grid_index(HALO_EDGE - 1, y, z)], b[grid_index(1, y, z)]);
                assert_eq!(b[grid_index(0, y, z)], a[grid_index(HALO_EDGE - 2, y, z)]);
            }
        }
    }

    #[test]
    fn bottom_layer_has_bedrock_and_top_layer_is_air() {
        let mut low = [1_u16; GRID_VOXELS];
        fill_chunk(5, -3, 0, 2, &mut low);
        for z in 0..HALO_EDGE {
            for x in 0..HALO_EDGE {
                assert_eq!(low[grid_index(x, 0, z)], BEDROCK);
                assert_eq!(low[grid_index(x, 1, z)], BEDROCK);
            }
        }
        let mut high = [1_u16; GRID_VOXELS];
        fill_chunk(5, 0, CHUNK_LAYERS, 0, &mut high);
        assert!(high.iter().all(|&b| b == AIR));
    }
}
