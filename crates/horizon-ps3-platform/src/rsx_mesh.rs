//! Compact, caller-owned vertex uploads for the first original RSX geometry path.
//!
//! The encoder writes big-endian scalar bytes explicitly so host tests cannot
//! accidentally hide the PPU's byte order. It never allocates and never grows
//! the upload buffer.

use crate::camera::{Camera, CameraError};
use crate::chunk_mesh::{Face, MeshQuad};
use crate::greedy_mesh::GreedyQuad;
use crate::{MemoryBudget, MemoryUsage};

pub const VERTEX_BYTES: usize = 24;
pub const MAX_CLIPPED_VERTICES_PER_GREEDY_QUAD: usize = 18;

#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(C)]
pub struct Vertex {
    pub position: [f32; 3],
    pub uv: [f32; 2],
    pub rgba: [u8; 4],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UploadError {
    BufferTooSmall { required: usize, capacity: usize },
    Gddr3BudgetExceeded { required: u32, available: u32 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClipError {
    BufferTooSmall { required: usize, capacity: usize },
    Camera(CameraError),
    TemporaryCapacityExceeded,
}

pub struct MeshUpload<'a> {
    target: &'a mut [u8],
    written: usize,
}

/// Flat colour per block id until real textures exist. Id 1 keeps its original
/// bring-up colour; the terrain ids from `terrain.rs` get their own.
const fn block_color(block_id: u16) -> [u8; 4] {
    match block_id {
        1 => [180, 120, 70, 255],
        2 => [120, 85, 50, 255],
        3 => [90, 170, 70, 255],
        4 => [60, 60, 60, 255],
        _ => [80, 180, 220, 255],
    }
}

/// Expands one visible voxel quad into two triangles for the sequential index path.
/// Coordinates map a 16³ chunk into clip-space; this is a bring-up transform only.
pub fn expand_quad_triangles(quad: MeshQuad, output: &mut [Vertex; 6]) {
    expand_quad_triangles_scaled(quad, [0.0; 3], 0.125, output);
}

/// Expands a quad from a chunk placed at a world-space offset in a two-chunk scene.
pub fn expand_quad_triangles_scene(
    quad: MeshQuad,
    chunk_offset: [f32; 3],
    output: &mut [Vertex; 6],
) {
    expand_quad_triangles_scaled(quad, chunk_offset, 0.0625, output);
}

/// Expands a greedy rectangle to two triangles in the same clip-space bring-up
/// profile as `expand_quad_triangles_scene`. UVs span the rectangle dimensions
/// so a wrapping sampler can repeat a block tile rather than stretch it.
pub fn expand_greedy_quad_triangles_scene(
    quad: GreedyQuad,
    chunk_offset: [f32; 3],
    output: &mut [Vertex; 6],
) {
    expand_greedy_quad_triangles_world(quad, chunk_offset, output);
    for vertex in output {
        vertex.position = [
            vertex.position[0] * 0.0625 - 1.0,
            vertex.position[1] * 0.0625 - 1.0,
            vertex.position[2] * 0.0625 - 1.0,
        ];
    }
}

/// Expands a greedy rectangle into perspective-projected vertices. If any
/// corner falls outside the camera's near/far interval, `output` is unchanged.
/// Use `expand_greedy_quad_triangles_camera_clipped` for crossing geometry.
pub fn expand_greedy_quad_triangles_camera(
    quad: GreedyQuad,
    chunk_offset: [f32; 3],
    camera: Camera,
    output: &mut [Vertex; 6],
) -> Result<(), CameraError> {
    let empty = Vertex {
        position: [0.0; 3],
        uv: [0.0; 2],
        rgba: [0; 4],
    };
    let mut projected = [empty; 6];
    expand_greedy_quad_triangles_world(quad, chunk_offset, &mut projected);
    for vertex in &mut projected {
        vertex.position = camera.project(vertex.position)?;
    }
    *output = projected;
    Ok(())
}

/// Expands and clips a greedy quad to the camera's near/far depth slab.
/// X/Y viewport clipping is left to RSX. The caller supplies room for up to
/// 18 vertices; insufficient capacity is reported without partial writes.
pub fn expand_greedy_quad_triangles_camera_clipped(
    quad: GreedyQuad,
    chunk_offset: [f32; 3],
    camera: Camera,
    output: &mut [Vertex],
) -> Result<usize, ClipError> {
    let empty = Vertex {
        position: [0.0; 3],
        uv: [0.0; 2],
        rgba: [0; 4],
    };
    let mut world = [empty; 6];
    expand_greedy_quad_triangles_world(quad, chunk_offset, &mut world);
    let mut generated = [empty; MAX_CLIPPED_VERTICES_PER_GREEDY_QUAD];
    let mut generated_count = 0;
    let (near, far) = camera.depth_limits();

    for triangle_start in [0, 3] {
        let mut first = [empty; 6];
        let mut second = [empty; 6];
        first[..3].copy_from_slice(&world[triangle_start..triangle_start + 3]);
        let near_count = clip_depth_plane(camera, &first[..3], near, true, &mut second)?;
        if near_count < 3 {
            continue;
        }
        let far_count = clip_depth_plane(camera, &second[..near_count], far, false, &mut first)?;
        if far_count < 3 {
            continue;
        }
        for fan_index in 1..(far_count - 1) {
            for polygon_index in [0, fan_index, fan_index + 1] {
                if generated_count >= generated.len() {
                    return Err(ClipError::TemporaryCapacityExceeded);
                }
                let mut vertex = first[polygon_index];
                vertex.position = camera
                    .project_clipped(vertex.position)
                    .map_err(ClipError::Camera)?;
                generated[generated_count] = vertex;
                generated_count += 1;
            }
        }
    }

    if output.len() < generated_count {
        return Err(ClipError::BufferTooSmall {
            required: generated_count,
            capacity: output.len(),
        });
    }
    output[..generated_count].copy_from_slice(&generated[..generated_count]);
    Ok(generated_count)
}

fn clip_depth_plane(
    camera: Camera,
    input: &[Vertex],
    plane: f32,
    keep_greater: bool,
    output: &mut [Vertex; 6],
) -> Result<usize, ClipError> {
    if input.is_empty() {
        return Ok(0);
    }
    let mut output_count = 0;
    let mut previous = input[input.len() - 1];
    let mut previous_depth = camera
        .view_depth(previous.position)
        .map_err(ClipError::Camera)?;
    let mut previous_inside = if keep_greater {
        previous_depth >= plane
    } else {
        previous_depth <= plane
    };

    for current in input.iter().copied() {
        let current_depth = camera
            .view_depth(current.position)
            .map_err(ClipError::Camera)?;
        let current_inside = if keep_greater {
            current_depth >= plane
        } else {
            current_depth <= plane
        };
        if current_inside != previous_inside {
            let denominator = current_depth - previous_depth;
            if denominator == 0.0 || !denominator.is_finite() {
                return Err(ClipError::Camera(CameraError::InvalidConfiguration));
            }
            let amount = ((plane - previous_depth) / denominator).clamp(0.0, 1.0);
            push_clipped_vertex(
                output,
                &mut output_count,
                interpolate_vertex(previous, current, amount),
            )?;
        }
        if current_inside {
            push_clipped_vertex(output, &mut output_count, current)?;
        }
        previous = current;
        previous_depth = current_depth;
        previous_inside = current_inside;
    }
    Ok(output_count)
}

fn push_clipped_vertex(
    output: &mut [Vertex; 6],
    output_count: &mut usize,
    vertex: Vertex,
) -> Result<(), ClipError> {
    if *output_count >= output.len() {
        return Err(ClipError::TemporaryCapacityExceeded);
    }
    output[*output_count] = vertex;
    *output_count += 1;
    Ok(())
}

fn interpolate_vertex(a: Vertex, b: Vertex, amount: f32) -> Vertex {
    let mut rgba = [0_u8; 4];
    for (index, channel) in rgba.iter_mut().enumerate() {
        let start = f32::from(a.rgba[index]);
        let delta = f32::from(b.rgba[index]) - start;
        *channel = (start + delta * amount).clamp(0.0, 255.0) as u8;
    }
    Vertex {
        position: lerp3(a.position, b.position, amount),
        uv: [
            a.uv[0] + (b.uv[0] - a.uv[0]) * amount,
            a.uv[1] + (b.uv[1] - a.uv[1]) * amount,
        ],
        rgba,
    }
}

fn lerp3(a: [f32; 3], b: [f32; 3], amount: f32) -> [f32; 3] {
    [
        a[0] + (b[0] - a[0]) * amount,
        a[1] + (b[1] - a[1]) * amount,
        a[2] + (b[2] - a[2]) * amount,
    ]
}

fn expand_greedy_quad_triangles_world(
    quad: GreedyQuad,
    chunk_offset: [f32; 3],
    output: &mut [Vertex; 6],
) {
    let x = f32::from(quad.x) + chunk_offset[0];
    let y = f32::from(quad.y) + chunk_offset[1];
    let z = f32::from(quad.z) + chunk_offset[2];
    let width = f32::from(quad.width);
    let height = f32::from(quad.height);
    let corners = match quad.face {
        Face::NegativeX => [
            [x, y, z],
            [x, y, z + width],
            [x, y + height, z + width],
            [x, y + height, z],
        ],
        Face::PositiveX => [
            [x + 1.0, y, z + width],
            [x + 1.0, y, z],
            [x + 1.0, y + height, z],
            [x + 1.0, y + height, z + width],
        ],
        Face::NegativeY => [
            [x, y, z + height],
            [x + width, y, z + height],
            [x + width, y, z],
            [x, y, z],
        ],
        Face::PositiveY => [
            [x, y + 1.0, z],
            [x + width, y + 1.0, z],
            [x + width, y + 1.0, z + height],
            [x, y + 1.0, z + height],
        ],
        Face::NegativeZ => [
            [x + width, y, z],
            [x, y, z],
            [x, y + height, z],
            [x + width, y + height, z],
        ],
        Face::PositiveZ => [
            [x, y, z + 1.0],
            [x + width, y, z + 1.0],
            [x + width, y + height, z + 1.0],
            [x, y + height, z + 1.0],
        ],
    };
    let uv = [[0.0, 0.0], [width, 0.0], [width, height], [0.0, height]];
    let color = block_color(quad.block_id);
    let order = [0_usize, 1, 2, 0, 2, 3];
    for (index, corner) in order.into_iter().enumerate() {
        output[index] = Vertex {
            position: corners[corner],
            uv: uv[corner],
            rgba: color,
        };
    }
}

fn expand_quad_triangles_scaled(
    quad: MeshQuad,
    chunk_offset: [f32; 3],
    scale: f32,
    output: &mut [Vertex; 6],
) {
    let x = f32::from(quad.x) + chunk_offset[0];
    let y = f32::from(quad.y) + chunk_offset[1];
    let z = f32::from(quad.z) + chunk_offset[2];
    let corners = match quad.face {
        Face::NegativeX => [
            [x, y, z],
            [x, y, z + 1.0],
            [x, y + 1.0, z + 1.0],
            [x, y + 1.0, z],
        ],
        Face::PositiveX => [
            [x + 1.0, y, z + 1.0],
            [x + 1.0, y, z],
            [x + 1.0, y + 1.0, z],
            [x + 1.0, y + 1.0, z + 1.0],
        ],
        Face::NegativeY => [
            [x, y, z + 1.0],
            [x + 1.0, y, z + 1.0],
            [x + 1.0, y, z],
            [x, y, z],
        ],
        Face::PositiveY => [
            [x, y + 1.0, z],
            [x + 1.0, y + 1.0, z],
            [x + 1.0, y + 1.0, z + 1.0],
            [x, y + 1.0, z + 1.0],
        ],
        Face::NegativeZ => [
            [x + 1.0, y, z],
            [x, y, z],
            [x, y + 1.0, z],
            [x + 1.0, y + 1.0, z],
        ],
        Face::PositiveZ => [
            [x, y, z + 1.0],
            [x + 1.0, y, z + 1.0],
            [x + 1.0, y + 1.0, z + 1.0],
            [x, y + 1.0, z + 1.0],
        ],
    };
    let color = block_color(quad.block_id);
    let uv = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
    let order = [0_usize, 1, 2, 0, 2, 3];
    for (index, corner) in order.into_iter().enumerate() {
        let position = corners[corner];
        output[index] = Vertex {
            position: [
                position[0] * scale - 1.0,
                position[1] * scale - 1.0,
                position[2] * scale - 1.0,
            ],
            uv: uv[corner],
            rgba: color,
        };
    }
}

impl<'a> MeshUpload<'a> {
    pub fn begin(
        target: &'a mut [u8],
        vertices: &[Vertex],
        budget: MemoryBudget,
        usage: MemoryUsage,
    ) -> Result<Self, UploadError> {
        let required = vertices.len().saturating_mul(VERTEX_BYTES);
        if target.len() < required {
            return Err(UploadError::BufferTooSmall {
                required,
                capacity: target.len(),
            });
        }
        let available = budget.gddr3.capacity.saturating_sub(usage.gddr3_reserved);
        let required_u32 = u32::try_from(required).unwrap_or(u32::MAX);
        if required_u32 > available {
            return Err(UploadError::Gddr3BudgetExceeded {
                required: required_u32,
                available,
            });
        }
        let mut upload = Self {
            target,
            written: required,
        };
        for (index, vertex) in vertices.iter().enumerate() {
            upload.encode(index * VERTEX_BYTES, *vertex);
        }
        Ok(upload)
    }

    pub fn bytes(&self) -> &[u8] {
        &self.target[..self.written]
    }

    pub const fn len(&self) -> usize {
        self.written
    }

    fn encode(&mut self, offset: usize, vertex: Vertex) {
        let mut cursor = offset;
        for component in vertex.position {
            let bytes = component.to_be_bytes();
            self.target[cursor..cursor + 4].copy_from_slice(&bytes);
            cursor += 4;
        }
        for component in vertex.uv {
            let bytes = component.to_be_bytes();
            self.target[cursor..cursor + 4].copy_from_slice(&bytes);
            cursor += 4;
        }
        self.target[cursor..cursor + 4].copy_from_slice(&vertex.rgba);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn budget() -> MemoryBudget {
        MemoryBudget {
            xdr: crate::MemoryPoolBudget {
                capacity: 64,
                reserved: 0,
            },
            gddr3: crate::MemoryPoolBudget {
                capacity: 64,
                reserved: 0,
            },
        }
    }

    #[test]
    fn encodes_one_vertex_in_explicit_big_endian_layout() {
        let vertex = Vertex {
            position: [1.0, -2.0, 0.5],
            uv: [0.25, -0.5],
            rgba: [0xde, 0xad, 0xbe, 0xef],
        };
        let mut bytes = [0_u8; VERTEX_BYTES];
        let upload =
            MeshUpload::begin(&mut bytes, &[vertex], budget(), MemoryUsage::default()).unwrap();
        assert_eq!(upload.len(), VERTEX_BYTES);
        assert_eq!(
            upload.bytes(),
            &[
                0x3f, 0x80, 0x00, 0x00, 0xc0, 0x00, 0x00, 0x00, 0x3f, 0x00, 0x00, 0x00, 0x3e, 0x80,
                0x00, 0x00, 0xbf, 0x00, 0x00, 0x00, 0xde, 0xad, 0xbe, 0xef,
            ]
        );
    }

    #[test]
    fn rejects_a_short_upload_buffer_without_writing() {
        let vertex = Vertex {
            position: [0.0; 3],
            uv: [0.0; 2],
            rgba: [255; 4],
        };
        let sentinel = [0xa5_u8; VERTEX_BYTES - 1];
        let mut bytes = sentinel;
        assert!(matches!(
            MeshUpload::begin(&mut bytes, &[vertex], budget(), MemoryUsage::default()),
            Err(UploadError::BufferTooSmall {
                required: VERTEX_BYTES,
                capacity: 23,
            })
        ));
        assert_eq!(bytes, sentinel);
    }

    #[test]
    fn rejects_upload_when_gddr3_capacity_is_exhausted() {
        let vertex = Vertex {
            position: [0.0; 3],
            uv: [0.0; 2],
            rgba: [0; 4],
        };
        let mut bytes = [0_u8; VERTEX_BYTES];
        assert!(matches!(
            MeshUpload::begin(
                &mut bytes,
                &[vertex],
                budget(),
                MemoryUsage {
                    xdr_reserved: 0,
                    gddr3_reserved: 64,
                },
            ),
            Err(UploadError::Gddr3BudgetExceeded {
                required: 24,
                available: 0,
            })
        ));
    }

    #[test]
    fn expands_voxel_quad_to_six_clip_space_vertices() {
        let quad = MeshQuad {
            x: 8,
            y: 8,
            z: 8,
            block_id: 1,
            face: Face::PositiveY,
        };
        let empty = Vertex {
            position: [0.0; 3],
            uv: [0.0; 2],
            rgba: [0; 4],
        };
        let mut vertices = [empty; 6];
        expand_quad_triangles(quad, &mut vertices);
        assert_eq!(vertices[0].position, [0.0, 0.125, 0.0]);
        assert_eq!(vertices[0].rgba, [180, 120, 70, 255]);
        assert_eq!(vertices[3], vertices[0]);
        assert_eq!(vertices[4].position, [0.125, 0.125, 0.125]);
    }

    #[test]
    fn expands_greedy_rectangle_and_tiles_uv_extent() {
        let quad = GreedyQuad {
            x: 0,
            y: 0,
            z: 0,
            width: 16,
            height: 16,
            block_id: 1,
            face: Face::PositiveY,
        };
        let empty = Vertex {
            position: [0.0; 3],
            uv: [0.0; 2],
            rgba: [0; 4],
        };
        let mut vertices = [empty; 6];
        expand_greedy_quad_triangles_scene(quad, [0.0; 3], &mut vertices);

        assert_eq!(vertices[0].position, [-1.0, -0.9375, -1.0]);
        assert_eq!(vertices[1].position, [0.0, -0.9375, -1.0]);
        assert_eq!(vertices[0].uv, [0.0, 0.0]);
        assert_eq!(vertices[1].uv, [16.0, 0.0]);
        assert_eq!(vertices[2].uv, [16.0, 16.0]);
        assert!(
            vertices
                .iter()
                .all(|vertex| vertex.rgba == [180, 120, 70, 255])
        );
    }

    fn test_camera() -> Camera {
        Camera::new(
            [0.0; 3],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            1.0,
            1.0,
            1.0,
            11.0,
        )
        .unwrap()
    }

    #[test]
    fn projects_greedy_quad_vertices_to_perspective_clip_space() {
        let quad = GreedyQuad {
            x: 2,
            y: 0,
            z: 4,
            width: 1,
            height: 1,
            block_id: 1,
            face: Face::PositiveY,
        };
        let empty = Vertex {
            position: [0.0; 3],
            uv: [0.0; 2],
            rgba: [0; 4],
        };
        let mut vertices = [empty; 6];
        expand_greedy_quad_triangles_camera(quad, [0.0; 3], test_camera(), &mut vertices).unwrap();
        for (actual, expected) in vertices[0].position.into_iter().zip([0.5, 0.25, 0.65]) {
            assert!((actual - expected).abs() < 1e-6);
        }
        for (actual, expected) in vertices[1].position.into_iter().zip([0.75, 0.25, 0.65]) {
            assert!((actual - expected).abs() < 1e-6);
        }
        assert_eq!(vertices[0].uv, [0.0, 0.0]);
        assert_eq!(vertices[1].uv, [1.0, 0.0]);
    }

    #[test]
    fn camera_expansion_does_not_partially_write_out_of_depth_quad() {
        let quad = GreedyQuad {
            x: 0,
            y: 0,
            z: 0,
            width: 1,
            height: 1,
            block_id: 1,
            face: Face::PositiveY,
        };
        let sentinel = Vertex {
            position: [8.0, 9.0, 10.0],
            uv: [2.0, 3.0],
            rgba: [4, 5, 6, 7],
        };
        let mut vertices = [sentinel; 6];
        assert_eq!(
            expand_greedy_quad_triangles_camera(quad, [0.0; 3], test_camera(), &mut vertices),
            Err(CameraError::PointOutsideDepthRange)
        );
        assert_eq!(vertices, [sentinel; 6]);
    }

    #[test]
    fn clipped_expansion_handles_near_and_far_plane_crossings() {
        let quad = GreedyQuad {
            x: 0,
            y: 0,
            z: 0,
            width: 1,
            height: 1,
            block_id: 1,
            face: Face::PositiveY,
        };
        let empty = Vertex {
            position: [0.0; 3],
            uv: [0.0; 2],
            rgba: [0; 4],
        };

        let mut near_vertices = [empty; MAX_CLIPPED_VERTICES_PER_GREEDY_QUAD];
        let near_count = expand_greedy_quad_triangles_camera_clipped(
            quad,
            [0.0, 0.0, 0.5],
            test_camera(),
            &mut near_vertices,
        )
        .unwrap();
        assert_eq!(near_count, 9);
        assert!(
            near_vertices[..near_count]
                .iter()
                .all(|vertex| (-1.0..=1.0).contains(&vertex.position[2]))
        );
        assert!(
            near_vertices[..near_count]
                .iter()
                .any(|vertex| (vertex.position[2] + 1.0).abs() < 1e-6)
        );
        assert!(
            near_vertices[..near_count]
                .iter()
                .any(|vertex| (vertex.uv[1] - 0.5).abs() < 1e-6)
        );
        assert!(
            near_vertices[..near_count]
                .iter()
                .all(|vertex| vertex.rgba == [180, 120, 70, 255])
        );

        let mut far_vertices = [empty; MAX_CLIPPED_VERTICES_PER_GREEDY_QUAD];
        let far_count = expand_greedy_quad_triangles_camera_clipped(
            quad,
            [0.0, 0.0, 10.5],
            test_camera(),
            &mut far_vertices,
        )
        .unwrap();
        assert_eq!(far_count, 9);
        assert!(
            far_vertices[..far_count]
                .iter()
                .all(|vertex| (-1.0..=1.0).contains(&vertex.position[2]))
        );
        assert!(
            far_vertices[..far_count]
                .iter()
                .any(|vertex| (vertex.position[2] - 1.0).abs() < 1e-6)
        );
    }

    #[test]
    fn clipped_expansion_rejects_short_output_without_partial_write() {
        let quad = GreedyQuad {
            x: 0,
            y: 0,
            z: 0,
            width: 1,
            height: 1,
            block_id: 1,
            face: Face::PositiveY,
        };
        let sentinel = Vertex {
            position: [8.0, 9.0, 10.0],
            uv: [2.0, 3.0],
            rgba: [4, 5, 6, 7],
        };
        let mut vertices = [sentinel; 8];
        assert_eq!(
            expand_greedy_quad_triangles_camera_clipped(
                quad,
                [0.0, 0.0, 0.5],
                test_camera(),
                &mut vertices,
            ),
            Err(ClipError::BufferTooSmall {
                required: 9,
                capacity: 8,
            })
        );
        assert_eq!(vertices, [sentinel; 8]);
    }

    #[test]
    fn clipped_expansion_discards_quads_outside_the_depth_slab() {
        let quad = GreedyQuad {
            x: 0,
            y: 0,
            z: 0,
            width: 1,
            height: 1,
            block_id: 1,
            face: Face::PositiveY,
        };
        let mut vertices = [Vertex {
            position: [3.0; 3],
            uv: [4.0; 2],
            rgba: [5; 4],
        }; MAX_CLIPPED_VERTICES_PER_GREEDY_QUAD];
        let sentinel = vertices;
        assert_eq!(
            expand_greedy_quad_triangles_camera_clipped(
                quad,
                [0.0, 0.0, -3.0],
                test_camera(),
                &mut vertices,
            ),
            Ok(0)
        );
        assert_eq!(vertices, sentinel);
    }
}
