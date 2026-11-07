//! Allocation-free greedy meshing for opaque 16³ PS3 chunks.
//!
//! Faces are merged into coplanar rectangles only when their block IDs match.
//! The halo is used exactly like in `chunk_mesh::mesh_opaque`, so neighboring
//! chunks can suppress shared boundary faces before this pass runs.

use crate::chunk_mesh::{CHUNK_EDGE, Face, GRID_VOXELS, HALO_EDGE, MeshError, grid_index};

pub const MAX_GREEDY_QUADS: usize = CHUNK_EDGE * CHUNK_EDGE * 6;
const MASK_CELLS: usize = CHUNK_EDGE * CHUNK_EDGE;
const FACES: [Face; 6] = [
    Face::NegativeX,
    Face::PositiveX,
    Face::NegativeY,
    Face::PositiveY,
    Face::NegativeZ,
    Face::PositiveZ,
];

/// A visible rectangle on a chunk face. Width/height lie in its face plane:
/// X faces use Z/Y, Y faces use X/Z, and Z faces use X/Y.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct GreedyQuad {
    pub x: u8,
    pub y: u8,
    pub z: u8,
    pub width: u8,
    pub height: u8,
    pub block_id: u16,
    pub face: Face,
}

/// Merge visible opaque unit faces into same-material rectangles.
///
/// The function counts first, so an undersized caller buffer is never partially
/// written. No heap allocation is performed; the temporary 16x16 mask lives on
/// the stack and is reused for every plane.
pub fn mesh_opaque_greedy(
    grid: &[u16; GRID_VOXELS],
    output: &mut [GreedyQuad],
) -> Result<usize, MeshError> {
    let mut required = 0;
    for face in FACES {
        required += greedy_face(grid, face, &mut [], false);
    }
    if output.len() < required {
        return Err(MeshError {
            required_quads: required,
            output_capacity: output.len(),
        });
    }

    let mut written = 0;
    for face in FACES {
        written += greedy_face(grid, face, &mut output[written..], true);
    }
    debug_assert_eq!(written, required);
    Ok(written)
}

fn greedy_face(
    grid: &[u16; GRID_VOXELS],
    face: Face,
    output: &mut [GreedyQuad],
    emit: bool,
) -> usize {
    let mut written = 0;
    let mut mask = [0_u16; MASK_CELLS];

    for slice in 0..CHUNK_EDGE {
        mask.fill(0);
        for v in 0..CHUNK_EDGE {
            for u in 0..CHUNK_EDGE {
                let [x, y, z] = voxel_coords(face, slice, u, v);
                let gx = x + 1;
                let gy = y + 1;
                let gz = z + 1;
                let block_id = grid[grid_index(gx, gy, gz)];
                if block_id != 0 && neighbor(grid, face, gx, gy, gz) == 0 {
                    mask[v * CHUNK_EDGE + u] = block_id;
                }
            }
        }

        for v in 0..CHUNK_EDGE {
            let mut u = 0;
            while u < CHUNK_EDGE {
                let block_id = mask[v * CHUNK_EDGE + u];
                if block_id == 0 {
                    u += 1;
                    continue;
                }

                let mut width = 1;
                while u + width < CHUNK_EDGE && mask[v * CHUNK_EDGE + u + width] == block_id {
                    width += 1;
                }

                let mut height = 1;
                'rows: while v + height < CHUNK_EDGE {
                    for x in u..(u + width) {
                        if mask[(v + height) * CHUNK_EDGE + x] != block_id {
                            break 'rows;
                        }
                    }
                    height += 1;
                }

                if emit {
                    let [x, y, z] = voxel_coords(face, slice, u, v);
                    output[written] = GreedyQuad {
                        x: x as u8,
                        y: y as u8,
                        z: z as u8,
                        width: width as u8,
                        height: height as u8,
                        block_id,
                        face,
                    };
                }
                written += 1;

                for row in v..(v + height) {
                    for column in u..(u + width) {
                        mask[row * CHUNK_EDGE + column] = 0;
                    }
                }
                u += width;
            }
        }
    }
    written
}

const fn voxel_coords(face: Face, slice: usize, u: usize, v: usize) -> [usize; 3] {
    match face {
        Face::NegativeX | Face::PositiveX => [slice, v, u],
        Face::NegativeY | Face::PositiveY => [u, slice, v],
        Face::NegativeZ | Face::PositiveZ => [u, v, slice],
    }
}

fn neighbor(grid: &[u16; GRID_VOXELS], face: Face, x: usize, y: usize, z: usize) -> u16 {
    let [nx, ny, nz] = match face {
        Face::NegativeX => [x - 1, y, z],
        Face::PositiveX => [x + 1, y, z],
        Face::NegativeY => [x, y - 1, z],
        Face::PositiveY => [x, y + 1, z],
        Face::NegativeZ => [x, y, z - 1],
        Face::PositiveZ => [x, y, z + 1],
    };
    debug_assert!(nx < HALO_EDGE && ny < HALO_EDGE && nz < HALO_EDGE);
    grid[grid_index(nx, ny, nz)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunk_mesh::{CHUNK_EDGE, MeshQuad, grid_index, mesh_opaque};

    fn empty_quad() -> GreedyQuad {
        GreedyQuad {
            x: 0,
            y: 0,
            z: 0,
            width: 0,
            height: 0,
            block_id: 0,
            face: Face::NegativeX,
        }
    }

    fn set(grid: &mut [u16; GRID_VOXELS], x: usize, y: usize, z: usize, id: u16) {
        grid[grid_index(x + 1, y + 1, z + 1)] = id;
    }

    #[test]
    fn full_chunk_merges_to_six_outer_rectangles() {
        let mut grid = [0_u16; GRID_VOXELS];
        for z in 0..CHUNK_EDGE {
            for y in 0..CHUNK_EDGE {
                for x in 0..CHUNK_EDGE {
                    set(&mut grid, x, y, z, 1);
                }
            }
        }
        let mut output = [empty_quad(); MAX_GREEDY_QUADS];
        assert_eq!(mesh_opaque_greedy(&grid, &mut output), Ok(6));
        for face in FACES {
            let mut matching = None;
            let mut count = 0;
            for quad in &output[..6] {
                if quad.face == face {
                    matching = Some(quad);
                    count += 1;
                }
            }
            assert_eq!(count, 1, "face: {face:?}");
            let rectangle = matching.expect("one rectangle per outer face");
            assert_eq!(rectangle.width, 16);
            assert_eq!(rectangle.height, 16);
        }
    }

    #[test]
    fn adjacent_same_material_blocks_merge_their_outer_faces() {
        let mut grid = [0_u16; GRID_VOXELS];
        set(&mut grid, 4, 5, 6, 7);
        set(&mut grid, 5, 5, 6, 7);
        let mut output = [empty_quad(); 12];
        assert_eq!(mesh_opaque_greedy(&grid, &mut output), Ok(6));
        assert!(!output[..6].iter().any(|quad| {
            (quad.x == 4 && quad.face == Face::PositiveX)
                || (quad.x == 5 && quad.face == Face::NegativeX)
        }));
    }

    #[test]
    fn different_materials_do_not_merge() {
        let mut grid = [0_u16; GRID_VOXELS];
        set(&mut grid, 4, 5, 6, 7);
        set(&mut grid, 5, 5, 6, 8);
        let mut output = [empty_quad(); 12];
        assert_eq!(mesh_opaque_greedy(&grid, &mut output), Ok(10));
        assert!(!output[..10].iter().any(|quad| {
            (quad.x == 4 && quad.face == Face::PositiveX)
                || (quad.x == 5 && quad.face == Face::NegativeX)
        }));
    }

    #[test]
    fn halo_suppresses_the_shared_boundary_face() {
        let mut grid = [0_u16; GRID_VOXELS];
        set(&mut grid, 15, 8, 8, 1);
        grid[grid_index(17, 9, 9)] = 2;
        let mut output = [empty_quad(); 6];
        assert_eq!(mesh_opaque_greedy(&grid, &mut output), Ok(5));
        assert!(
            !output[..5]
                .iter()
                .any(|quad| quad.x == 15 && quad.face == Face::PositiveX)
        );
    }

    #[test]
    fn greedy_surface_area_matches_unit_faces_per_material_and_direction() {
        let mut grid = [0_u16; GRID_VOXELS];
        for z in 4..8 {
            for y in 4..8 {
                for x in 4..8 {
                    if (x * 17 + y * 7 + z * 5) % 5 != 0 {
                        let block_id = if (x + y + z) % 2 == 0 { 1 } else { 2 };
                        set(&mut grid, x, y, z, block_id);
                    }
                }
            }
        }

        let empty_unit = MeshQuad {
            x: 0,
            y: 0,
            z: 0,
            block_id: 0,
            face: Face::NegativeX,
        };
        let mut unit_output = [empty_unit; 4 * 4 * 4 * 6];
        let unit_count = mesh_opaque(&grid, &mut unit_output).unwrap();
        let mut greedy_output = [empty_quad(); 4 * 4 * 4 * 6];
        let greedy_count = mesh_opaque_greedy(&grid, &mut greedy_output).unwrap();

        let mut unit_area = [[0_usize; 6]; 3];
        let mut greedy_area = [[0_usize; 6]; 3];
        for quad in &unit_output[..unit_count] {
            unit_area[quad.block_id as usize][quad.face as usize] += 1;
        }
        for quad in &greedy_output[..greedy_count] {
            greedy_area[quad.block_id as usize][quad.face as usize] +=
                usize::from(quad.width) * usize::from(quad.height);
        }
        assert_eq!(greedy_area, unit_area);
    }

    #[test]
    fn insufficient_capacity_reports_required_quads_without_partial_write() {
        let mut grid = [0_u16; GRID_VOXELS];
        for z in 0..CHUNK_EDGE {
            for y in 0..CHUNK_EDGE {
                for x in 0..CHUNK_EDGE {
                    set(&mut grid, x, y, z, 1);
                }
            }
        }
        let sentinel = GreedyQuad {
            block_id: 99,
            ..empty_quad()
        };
        let mut output = [sentinel; 5];
        assert_eq!(
            mesh_opaque_greedy(&grid, &mut output),
            Err(MeshError {
                required_quads: 6,
                output_capacity: 5,
            })
        );
        assert!(output.iter().all(|quad| *quad == sentinel));
    }
}
