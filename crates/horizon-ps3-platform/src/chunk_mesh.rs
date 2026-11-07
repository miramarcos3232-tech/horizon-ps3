//! Compact, allocation-free opaque chunk meshing for the PS3 bring-up path.
//!
//! Input is a 16³ chunk plus a one-voxel halo (18³ total). The halo lets
//! adjacent chunks cull shared boundary faces without worker-side lookups.
//! This first version treats every nonzero block ID as opaque; fluids and other
//! transparent materials need a separate mesh pass.

pub const CHUNK_EDGE: usize = 16;
pub const HALO_EDGE: usize = CHUNK_EDGE + 2;
pub const GRID_VOXELS: usize = HALO_EDGE * HALO_EDGE * HALO_EDGE;
pub const MAX_OPAQUE_QUADS: usize = CHUNK_EDGE * CHUNK_EDGE * CHUNK_EDGE * 6;
pub const BLOCK_GRID_BYTES: usize = GRID_VOXELS * core::mem::size_of::<u16>();

/// Identity and revision of a chunk whose mesh needs rebuilding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MeshRequest {
    pub chunk_x: i32,
    pub chunk_y: i32,
    pub chunk_z: i32,
    pub revision: u32,
}

/// Allocation-free FIFO for bounded chunk-mesh work.
#[derive(Debug)]
pub struct MeshQueue<const CAPACITY: usize> {
    entries: [Option<MeshRequest>; CAPACITY],
    head: usize,
    tail: usize,
    len: usize,
}

impl<const CAPACITY: usize> MeshQueue<CAPACITY> {
    pub const fn new() -> Self {
        Self {
            entries: [None; CAPACITY],
            head: 0,
            tail: 0,
            len: 0,
        }
    }

    pub const fn len(&self) -> usize {
        self.len
    }
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }
    pub const fn is_full(&self) -> bool {
        self.len == CAPACITY
    }

    pub fn push(&mut self, request: MeshRequest) -> Result<(), MeshRequest> {
        if self.is_full() {
            return Err(request);
        }
        self.entries[self.tail] = Some(request);
        self.tail = (self.tail + 1) % CAPACITY.max(1);
        self.len += 1;
        Ok(())
    }

    pub fn pop(&mut self) -> Option<MeshRequest> {
        if self.is_empty() {
            return None;
        }
        let request = self.entries[self.head].take();
        self.head = (self.head + 1) % CAPACITY.max(1);
        self.len -= 1;
        request
    }
}

impl<const CAPACITY: usize> Default for MeshQueue<CAPACITY> {
    fn default() -> Self {
        Self::new()
    }
}

/// Face normal in the chunk's positive-Y-up coordinate system.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Face {
    NegativeX = 0,
    PositiveX = 1,
    NegativeY = 2,
    PositiveY = 3,
    NegativeZ = 4,
    PositiveZ = 5,
}

/// One visible unit face. A later renderer can expand this to four vertices,
/// pack UVs/light, or greedily merge coplanar faces without changing block data.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct MeshQuad {
    pub x: u8,
    pub y: u8,
    pub z: u8,
    pub block_id: u16,
    pub face: Face,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MeshError {
    pub required_quads: usize,
    pub output_capacity: usize,
}

/// Index a haloed grid. Coordinates are each in `0..HALO_EDGE`; interior chunk
/// cells use coordinates 1..=16 and the outside halo uses 0 or 17.
pub const fn grid_index(x: usize, y: usize, z: usize) -> usize {
    (z * HALO_EDGE + y) * HALO_EDGE + x
}

/// Emit one quad for each exposed face of each non-air voxel in the chunk.
/// On insufficient output capacity, returns the exact required count and does
/// not write a partial mesh.
pub fn mesh_opaque(grid: &[u16; GRID_VOXELS], output: &mut [MeshQuad]) -> Result<usize, MeshError> {
    let required = count_faces(grid);
    if output.len() < required {
        return Err(MeshError {
            required_quads: required,
            output_capacity: output.len(),
        });
    }

    let mut written = 0;
    for z in 0..CHUNK_EDGE {
        for y in 0..CHUNK_EDGE {
            for x in 0..CHUNK_EDGE {
                let gx = x + 1;
                let gy = y + 1;
                let gz = z + 1;
                let block_id = grid[grid_index(gx, gy, gz)];
                if block_id == 0 {
                    continue;
                }
                let neighbors = [
                    (gx - 1, gy, gz, Face::NegativeX),
                    (gx + 1, gy, gz, Face::PositiveX),
                    (gx, gy - 1, gz, Face::NegativeY),
                    (gx, gy + 1, gz, Face::PositiveY),
                    (gx, gy, gz - 1, Face::NegativeZ),
                    (gx, gy, gz + 1, Face::PositiveZ),
                ];
                for (nx, ny, nz, face) in neighbors {
                    if grid[grid_index(nx, ny, nz)] == 0 {
                        output[written] = MeshQuad {
                            x: x as u8,
                            y: y as u8,
                            z: z as u8,
                            block_id,
                            face,
                        };
                        written += 1;
                    }
                }
            }
        }
    }
    debug_assert_eq!(written, required);
    Ok(written)
}

fn count_faces(grid: &[u16; GRID_VOXELS]) -> usize {
    let mut count = 0;
    for z in 0..CHUNK_EDGE {
        for y in 0..CHUNK_EDGE {
            for x in 0..CHUNK_EDGE {
                let gx = x + 1;
                let gy = y + 1;
                let gz = z + 1;
                if grid[grid_index(gx, gy, gz)] == 0 {
                    continue;
                }
                count += usize::from(grid[grid_index(gx - 1, gy, gz)] == 0);
                count += usize::from(grid[grid_index(gx + 1, gy, gz)] == 0);
                count += usize::from(grid[grid_index(gx, gy - 1, gz)] == 0);
                count += usize::from(grid[grid_index(gx, gy + 1, gz)] == 0);
                count += usize::from(grid[grid_index(gx, gy, gz - 1)] == 0);
                count += usize::from(grid[grid_index(gx, gy, gz + 1)] == 0);
            }
        }
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_queue_preserves_fifo_order_and_rejects_overflow() {
        let first = MeshRequest {
            chunk_x: 1,
            chunk_y: 2,
            chunk_z: 3,
            revision: 4,
        };
        let second = MeshRequest {
            chunk_x: -1,
            chunk_y: 0,
            chunk_z: 7,
            revision: 8,
        };
        let third = MeshRequest {
            chunk_x: 9,
            chunk_y: 9,
            chunk_z: 9,
            revision: 10,
        };
        let mut queue = MeshQueue::<2>::new();
        assert!(queue.is_empty());
        assert_eq!(queue.push(first), Ok(()));
        assert_eq!(queue.push(second), Ok(()));
        assert!(queue.is_full());
        assert_eq!(queue.push(third), Err(third));
        assert_eq!(queue.pop(), Some(first));
        assert_eq!(queue.push(third), Ok(()));
        assert_eq!(queue.pop(), Some(second));
        assert_eq!(queue.pop(), Some(third));
        assert_eq!(queue.pop(), None);
    }

    #[test]
    fn zero_capacity_queue_never_writes_or_panics() {
        let request = MeshRequest {
            chunk_x: 0,
            chunk_y: 0,
            chunk_z: 0,
            revision: 1,
        };
        let mut queue = MeshQueue::<0>::new();
        assert_eq!(queue.push(request), Err(request));
        assert_eq!(queue.pop(), None);
    }

    fn set(grid: &mut [u16; GRID_VOXELS], x: usize, y: usize, z: usize, id: u16) {
        grid[grid_index(x + 1, y + 1, z + 1)] = id;
    }

    #[test]
    fn one_voxel_emits_six_correctly_oriented_faces() {
        let mut grid = [0_u16; GRID_VOXELS];
        set(&mut grid, 3, 7, 11, 42);
        let mut output = [MeshQuad {
            x: 0,
            y: 0,
            z: 0,
            block_id: 0,
            face: Face::NegativeX,
        }; 6];
        assert_eq!(mesh_opaque(&grid, &mut output), Ok(6));
        let expected_faces = [
            Face::NegativeX,
            Face::PositiveX,
            Face::NegativeY,
            Face::PositiveY,
            Face::NegativeZ,
            Face::PositiveZ,
        ];
        assert!(
            output
                .iter()
                .zip(expected_faces)
                .all(|(quad, face)| quad.face == face)
        );
        assert!(
            output
                .iter()
                .all(|quad| quad.block_id == 42 && quad.x == 3 && quad.y == 7 && quad.z == 11)
        );
    }

    #[test]
    fn adjacent_voxels_cull_the_two_shared_faces() {
        let mut grid = [0_u16; GRID_VOXELS];
        set(&mut grid, 0, 0, 0, 1);
        set(&mut grid, 1, 0, 0, 2);
        let mut output = [MeshQuad {
            x: 0,
            y: 0,
            z: 0,
            block_id: 0,
            face: Face::NegativeX,
        }; 12];
        assert_eq!(mesh_opaque(&grid, &mut output), Ok(10));
        assert!(!output[..10].iter().any(|quad| {
            (quad.x == 0 && quad.face == Face::PositiveX)
                || (quad.x == 1 && quad.face == Face::NegativeX)
        }));
    }

    #[test]
    fn halo_voxels_cull_faces_at_chunk_boundaries() {
        let mut grid = [0_u16; GRID_VOXELS];
        set(&mut grid, 15, 8, 8, 1);
        grid[grid_index(17, 9, 9)] = 99;
        let mut output = [MeshQuad {
            x: 0,
            y: 0,
            z: 0,
            block_id: 0,
            face: Face::NegativeX,
        }; 6];
        assert_eq!(mesh_opaque(&grid, &mut output), Ok(5));
        assert!(!output[..5].iter().any(|quad| quad.face == Face::PositiveX));
    }

    #[test]
    fn insufficient_output_reports_exact_size_without_partial_writes() {
        let mut grid = [0_u16; GRID_VOXELS];
        set(&mut grid, 1, 1, 1, 7);
        let sentinel = MeshQuad {
            x: 9,
            y: 9,
            z: 9,
            block_id: 99,
            face: Face::PositiveZ,
        };
        let mut output = [sentinel; 5];
        assert_eq!(
            mesh_opaque(&grid, &mut output),
            Err(MeshError {
                required_quads: 6,
                output_capacity: 5,
            })
        );
        assert!(output.iter().all(|quad| *quad == sentinel));
    }

    #[test]
    fn full_chunk_emits_only_its_outer_shell() {
        let mut grid = [0_u16; GRID_VOXELS];
        for z in 0..CHUNK_EDGE {
            for y in 0..CHUNK_EDGE {
                for x in 0..CHUNK_EDGE {
                    set(&mut grid, x, y, z, 1);
                }
            }
        }
        let mut output = [MeshQuad {
            x: 0,
            y: 0,
            z: 0,
            block_id: 0,
            face: Face::NegativeX,
        }; CHUNK_EDGE * CHUNK_EDGE * 6];
        assert_eq!(
            mesh_opaque(&grid, &mut output),
            Ok(CHUNK_EDGE * CHUNK_EDGE * 6)
        );
    }
}
