#![allow(unsafe_code)]

//! Experimental PPU world client (feature `psl1ght-ffi`): flies a camera over
//! the generated terrain every frame. **Never run on hardware yet.**
//!
//! Per frame: read the pad, move the camera, stream and mesh a few chunks
//! within a fixed budget, project every cached quad on the CPU, upload one
//! vertex buffer and draw it with a cleared depth buffer. Exit with Start+Select.
//!
//! All large buffers are one `static`, handed out once, so nothing big lives on
//! the PPU thread's stack and nothing allocates at run time.

use core::sync::atomic::{AtomicBool, Ordering};

use crate::camera::Camera;
use crate::frame_budget::FrameWorkBudget;
use crate::greedy_mesh::{GreedyQuad, MAX_GREEDY_QUADS};
use crate::grounded::GroundedBody;
use crate::hotbar::Hotbar;
use crate::interaction::cast_block_ray;
use crate::mesh_cache::{BLANK_QUAD, MeshCache};
use crate::player::FlyCamera;
use crate::psl1ght::Psl1ghtApi;
use crate::rsx_mesh::{
    MAX_CLIPPED_VERTICES_PER_GREEDY_QUAD, MeshUpload, VERTEX_BYTES, Vertex,
    expand_greedy_quad_triangles_camera_clipped,
};
use crate::terrain::{self, CHUNK_LAYERS};
use crate::world::World;
use crate::{MemoryBudget, PadState, Ps3Api};

const WORLD_SEED: u32 = 0x485A_4E31;
/// Chunks within this many chunks of the camera are loaded.
const RADIUS: i32 = 2;
/// `World` evicts beyond `RADIUS + 1`, so the store must hold that whole square.
const KEEP: i32 = RADIUS + 1;
const CHUNK_SLOTS: usize = ((2 * KEEP + 1) * (2 * KEEP + 1) * CHUNK_LAYERS) as usize;
const QUEUE_SLOTS: usize = CHUNK_SLOTS + 8;
const MAX_NEW_CHUNKS_PER_FRAME: u16 = 3;
const MESHES_PER_FRAME: u16 = 3;
const SCRATCH_QUADS: usize = 8192;
/// Below the bridge's 65 535-vertex index limit, and a multiple of three.
const MAX_FRAME_VERTICES: usize = 60_000;
const SKY_ARGB: u32 = 0xFF87_CEEB;
/// No clock is exposed yet; assume one frame per 60 Hz vblank.
const FRAME_DT: f32 = 1.0 / 60.0;
const EDIT_REACH: f32 = 32.0;
const EDIT_STEP: f32 = 0.25;
const HOTBAR_SLOTS: usize = 3;
const HOTBAR_PIPS_PER_SLOT: usize = 8;
const CROSSHAIR_VERTICES: usize = 4 * 6;
const PAUSE_ICON_VERTICES: usize = 2 * 6;
const INVENTORY_VERTICES: usize = 4 * 6;
const HOTBAR_VERTICES: usize = HOTBAR_SLOTS * (1 + HOTBAR_PIPS_PER_SLOT) * 6;
const HUD_VERTICES: usize =
    HOTBAR_VERTICES + CROSSHAIR_VERTICES + PAUSE_ICON_VERTICES + INVENTORY_VERTICES;

const EMPTY_VERTEX: Vertex = Vertex {
    position: [0.0; 3],
    uv: [0.0; 2],
    rgba: [0; 4],
};

type Cache = MeshCache<CHUNK_SLOTS, MAX_GREEDY_QUADS>;

struct ClientBuffers {
    world: World<CHUNK_SLOTS, QUEUE_SLOTS>,
    body: GroundedBody,
    hotbar: Hotbar,
    cache: Cache,
    scratch: [GreedyQuad; SCRATCH_QUADS],
    vertices: [Vertex; MAX_FRAME_VERTICES],
    encoded: [u8; MAX_FRAME_VERTICES * VERTEX_BYTES],
}

impl ClientBuffers {
    const fn new() -> Self {
        Self {
            world: World::new(WORLD_SEED),
            body: GroundedBody::new([8.0, 50.0, 8.0]),
            hotbar: Hotbar::new(),
            cache: MeshCache::new(),
            scratch: [BLANK_QUAD; SCRATCH_QUADS],
            vertices: [EMPTY_VERTEX; MAX_FRAME_VERTICES],
            encoded: [0; MAX_FRAME_VERTICES * VERTEX_BYTES],
        }
    }
}

static BUFFERS_TAKEN: AtomicBool = AtomicBool::new(false);
static mut BUFFERS: ClientBuffers = ClientBuffers::new();

fn take_buffers() -> Option<&'static mut ClientBuffers> {
    if BUFFERS_TAKEN.swap(true, Ordering::AcqRel) {
        return None;
    }
    // SAFETY: the flag above lets this run once, so this is the only reference
    // ever created to `BUFFERS`.
    Some(unsafe { &mut *(&raw mut BUFFERS) })
}

/// C-visible entry for `ports/ps3/rust_world_probe.c`. Returns 0 on a clean
/// exit, otherwise a stage code (10 boot, 12 camera/upload, 13 upload, 14
/// pipeline, 15 bind, 16 draw, 18 frame start, 19 present, 20 re-entry).
#[unsafe(no_mangle)]
pub extern "C" fn horizon_ps3_rust_world_entry() -> i32 {
    match run() {
        Ok(()) => 0,
        Err(code) => code,
    }
}

fn run() -> Result<(), i32> {
    let buffers = take_buffers().ok_or(20)?;
    let mut api = Psl1ghtApi::boot().map_err(|_| 10)?;
    let display = api.display_config();
    let aspect = f32::from(display.width) / f32::from(display.height);
    api.prepare_mesh_pipeline().map_err(|_| 14)?;
    let _ = api.tty_write(b"Horizon world client online\n");

    let memory_budget = MemoryBudget::initial();
    // Spawn above the tallest terrain (42), looking slightly down.
    let mut fly = FlyCamera::new([8.0, 50.0, 8.0], 0.0, -0.35);
    let mut previous_pad = PadState::default();
    let mut paused = false;
    let mut inventory_open = false;

    loop {
        let pad = api.pad_state();
        if pad.start && pad.select {
            break;
        }
        if pad.start && !previous_pad.start {
            paused = !paused;
        }
        if pad.triangle && !previous_pad.triangle && !paused {
            inventory_open = !inventory_open;
        }

        if !paused && !inventory_open {
            // Stream before physics so the body can land only on resident terrain.
            let (stream_x, stream_z) = fly.chunk_coords();
            buffers
                .world
                .stream_around(stream_x, stream_z, RADIUS, MAX_NEW_CHUNKS_PER_FRAME);
            let look_pad = PadState {
                left_x: 0,
                left_y: 0,
                l2: 0,
                r2: 0,
                ..pad
            };
            buffers.hotbar.update(pad, previous_pad);
            fly.update(look_pad, FRAME_DT);
            buffers.body.step(
                &buffers.world,
                fly.horizontal_velocity(pad),
                pad.square && !previous_pad.square,
                FRAME_DT,
            );
            let _ = buffers.body.respawn_if_void([8.0, 50.0, 8.0]);
            fly.set_position(buffers.body.position);
        }

        let target = if paused || inventory_open {
            None
        } else {
            cast_block_ray(
                &buffers.world,
                fly.position(),
                fly.look_direction(),
                EDIT_REACH,
                EDIT_STEP,
            )
        };
        let target_state = target.as_ref().map_or(0, |hit| {
            if buffers
                .world
                .block_global(hit.solid[0], hit.solid[1], hit.solid[2])
                == Some(terrain::BEDROCK)
            {
                2
            } else {
                1
            }
        });
        if !paused
            && !inventory_open
            && ((pad.cross && !previous_pad.cross) || (pad.circle && !previous_pad.circle))
        {
            if let Some(target) = target {
                if pad.cross {
                    let broken = buffers.world.block_global(
                        target.solid[0],
                        target.solid[1],
                        target.solid[2],
                    );
                    if broken != Some(terrain::BEDROCK)
                        && buffers.world.set_block_global(
                            target.solid[0],
                            target.solid[1],
                            target.solid[2],
                            terrain::AIR,
                        )
                    {
                        if let Some(block) = broken {
                            let _ = buffers.hotbar.collect(block);
                        }
                    }
                } else if !buffers.body.overlaps_block(
                    target.place[0],
                    target.place[1],
                    target.place[2],
                ) && buffers.world.block_global(
                    target.place[0],
                    target.place[1],
                    target.place[2],
                ) == Some(terrain::AIR)
                {
                    let block = buffers.hotbar.selected_block();
                    if buffers.hotbar.consume_selected() {
                        if !buffers.world.set_block_global(
                            target.place[0],
                            target.place[1],
                            target.place[2],
                            block,
                        ) {
                            let _ = buffers.hotbar.collect(block);
                        }
                    }
                }
            }
        }
        previous_pad = pad;

        let (cx, cz) = fly.chunk_coords();
        buffers
            .cache
            .retain(|key| (key[0] - cx).abs() <= KEEP && (key[2] - cz).abs() <= KEEP);

        let mut meshing = FrameWorkBudget::new(MESHES_PER_FRAME, u32::MAX);
        buffers
            .world
            .pump(&mut meshing, &mut buffers.scratch, |request, quads| {
                let _ = buffers.cache.store(request, quads);
            });

        let camera = fly.camera(aspect).map_err(|_| 12)?;
        let world_vertex_count = build_vertices(&buffers.cache, camera, &mut buffers.vertices);
        let hud_vertex_count = append_hotbar_vertices(
            &mut buffers.vertices[world_vertex_count..],
            buffers.hotbar.selected_slot() as usize,
            [
                buffers.hotbar.count(0),
                buffers.hotbar.count(1),
                buffers.hotbar.count(2),
            ],
            paused,
            target_state,
            inventory_open,
        );
        let vertex_count = world_vertex_count + hud_vertex_count;

        api.begin_rsx_frame(display).map_err(|_| 18)?;
        api.begin_scene(SKY_ARGB).map_err(|_| 18)?;
        if vertex_count > 0 {
            let usage = api.memory_usage();
            let offset = {
                let upload = MeshUpload::begin(
                    &mut buffers.encoded,
                    &buffers.vertices[..vertex_count],
                    memory_budget,
                    usage,
                )
                .map_err(|_| 12)?;
                api.upload_mesh(upload.bytes()).map_err(|_| 13)?
            };
            api.bind_mesh(offset, vertex_count as u32).map_err(|_| 15)?;
            api.draw_mesh(offset, vertex_count as u32).map_err(|_| 16)?;
        }
        api.present_scene().map_err(|_| 19)?;
    }

    let _ = api.tty_write(b"Horizon world client exit\n");
    Ok(())
}

/// Projects every cached quad into `out`; returns the vertex count (a multiple
/// of three). Stops quietly when `out` is full, so a crowded frame drops the
/// last chunks instead of failing.
fn build_vertices(cache: &Cache, camera: Camera, out: &mut [Vertex]) -> usize {
    let mut count = 0;
    let mut expanded = [EMPTY_VERTEX; MAX_CLIPPED_VERTICES_PER_GREEDY_QUAD];
    for slot in 0..cache.slots() {
        let Some((key, quads)) = cache.get(slot) else {
            continue;
        };
        let offset = [
            key[0] as f32 * 16.0,
            key[1] as f32 * 16.0,
            key[2] as f32 * 16.0,
        ];
        for quad in quads {
            let Ok(produced) =
                expand_greedy_quad_triangles_camera_clipped(*quad, offset, camera, &mut expanded)
            else {
                continue;
            };
            if count + produced > out.len() {
                return count;
            }
            out[count..count + produced].copy_from_slice(&expanded[..produced]);
            count += produced;
        }
    }
    count
}

/// Appends a depth-tested, flat-colour hotbar in clip space. The HUD uses a
/// negative depth so it is drawn over the world while reusing the existing
/// bring-up shader and upload path.
fn append_hotbar_vertices(
    out: &mut [Vertex],
    selected: usize,
    counts: [u16; HOTBAR_SLOTS],
    paused: bool,
    target_state: u8,
    inventory_open: bool,
) -> usize {
    if out.len() < HUD_VERTICES || selected >= HOTBAR_SLOTS {
        return 0;
    }
    let width = 0.12_f32;
    let gap = 0.012_f32;
    let left = -((HOTBAR_SLOTS as f32 * width + (HOTBAR_SLOTS - 1) as f32 * gap) * 0.5);
    let bottom = -0.94_f32;
    let top = -0.78_f32;
    for slot in 0..HOTBAR_SLOTS {
        let x0 = left + slot as f32 * (width + gap);
        let x1 = x0 + width;
        let colour = if slot == selected {
            [245, 220, 90, 255]
        } else {
            [35, 35, 45, 235]
        };
        let quad = [
            Vertex {
                position: [x0, bottom, -0.99],
                uv: [0.0, 0.0],
                rgba: colour,
            },
            Vertex {
                position: [x1, bottom, -0.99],
                uv: [1.0, 0.0],
                rgba: colour,
            },
            Vertex {
                position: [x1, top, -0.99],
                uv: [1.0, 1.0],
                rgba: colour,
            },
            Vertex {
                position: [x0, bottom, -0.99],
                uv: [0.0, 0.0],
                rgba: colour,
            },
            Vertex {
                position: [x1, top, -0.99],
                uv: [1.0, 1.0],
                rgba: colour,
            },
            Vertex {
                position: [x0, top, -0.99],
                uv: [0.0, 1.0],
                rgba: colour,
            },
        ];
        out[slot * 6..slot * 6 + 6].copy_from_slice(&quad);

        let pip_count = usize::from(counts[slot].min(16)).div_ceil(2);
        let pip_width = (width - 0.024) / HOTBAR_PIPS_PER_SLOT as f32;
        for pip in 0..HOTBAR_PIPS_PER_SLOT {
            let px0 = x0 + 0.012 + pip as f32 * pip_width;
            let px1 = px0 + pip_width * 0.72;
            let py0 = bottom + 0.018;
            let py1 = py0 + 0.018;
            let pip_colour = if pip < pip_count {
                [235, 235, 235, 255]
            } else {
                [0, 0, 0, 0]
            };
            let pip_quad = [
                Vertex {
                    position: [px0, py0, -0.995],
                    uv: [0.0, 0.0],
                    rgba: pip_colour,
                },
                Vertex {
                    position: [px1, py0, -0.995],
                    uv: [0.0, 0.0],
                    rgba: pip_colour,
                },
                Vertex {
                    position: [px1, py1, -0.995],
                    uv: [0.0, 0.0],
                    rgba: pip_colour,
                },
                Vertex {
                    position: [px0, py0, -0.995],
                    uv: [0.0, 0.0],
                    rgba: pip_colour,
                },
                Vertex {
                    position: [px1, py1, -0.995],
                    uv: [0.0, 0.0],
                    rgba: pip_colour,
                },
                Vertex {
                    position: [px0, py1, -0.995],
                    uv: [0.0, 0.0],
                    rgba: pip_colour,
                },
            ];
            let pip_offset = HOTBAR_SLOTS * 6 + (slot * HOTBAR_PIPS_PER_SLOT + pip) * 6;
            out[pip_offset..pip_offset + 6].copy_from_slice(&pip_quad);
        }
    }

    let crosshair_colour = match target_state {
        1 => [100, 255, 130, 240],
        2 => [255, 90, 90, 240],
        _ => [255, 255, 255, 220],
    };
    for (arm, x0, y0, x1, y1) in [
        (0, -0.018_f32, -0.004_f32, 0.018_f32, 0.004_f32),
        (1, -0.004_f32, -0.018_f32, 0.004_f32, 0.018_f32),
        (2, -0.030_f32, -0.003_f32, -0.020_f32, 0.003_f32),
        (3, 0.020_f32, -0.003_f32, 0.030_f32, 0.003_f32),
    ] {
        let quad = [
            Vertex {
                position: [x0, y0, -0.998],
                uv: [0.0, 0.0],
                rgba: crosshair_colour,
            },
            Vertex {
                position: [x1, y0, -0.998],
                uv: [0.0, 0.0],
                rgba: crosshair_colour,
            },
            Vertex {
                position: [x1, y1, -0.998],
                uv: [0.0, 0.0],
                rgba: crosshair_colour,
            },
            Vertex {
                position: [x0, y0, -0.998],
                uv: [0.0, 0.0],
                rgba: crosshair_colour,
            },
            Vertex {
                position: [x1, y1, -0.998],
                uv: [0.0, 0.0],
                rgba: crosshair_colour,
            },
            Vertex {
                position: [x0, y1, -0.998],
                uv: [0.0, 0.0],
                rgba: crosshair_colour,
            },
        ];
        let offset = HOTBAR_VERTICES + arm * 6;
        out[offset..offset + 6].copy_from_slice(&quad);
    }

    let mut hud_count = HOTBAR_VERTICES + CROSSHAIR_VERTICES;
    if paused {
        let pause_colour = [245, 245, 245, 255];
        for (bar, x0, x1) in [(0, -0.045_f32, -0.012_f32), (1, 0.012_f32, 0.045_f32)] {
            let y0 = -0.12_f32;
            let y1 = 0.12_f32;
            let quad = [
                Vertex {
                    position: [x0, y0, -0.999],
                    uv: [0.0, 0.0],
                    rgba: pause_colour,
                },
                Vertex {
                    position: [x1, y0, -0.999],
                    uv: [0.0, 0.0],
                    rgba: pause_colour,
                },
                Vertex {
                    position: [x1, y1, -0.999],
                    uv: [0.0, 0.0],
                    rgba: pause_colour,
                },
                Vertex {
                    position: [x0, y0, -0.999],
                    uv: [0.0, 0.0],
                    rgba: pause_colour,
                },
                Vertex {
                    position: [x1, y1, -0.999],
                    uv: [0.0, 0.0],
                    rgba: pause_colour,
                },
                Vertex {
                    position: [x0, y1, -0.999],
                    uv: [0.0, 0.0],
                    rgba: pause_colour,
                },
            ];
            let offset = HOTBAR_VERTICES + CROSSHAIR_VERTICES + bar * 6;
            out[offset..offset + 6].copy_from_slice(&quad);
        }
        hud_count = HOTBAR_VERTICES + CROSSHAIR_VERTICES + PAUSE_ICON_VERTICES;
    } else {
        out[HOTBAR_VERTICES + CROSSHAIR_VERTICES
            ..HOTBAR_VERTICES + CROSSHAIR_VERTICES + PAUSE_ICON_VERTICES]
            .fill(EMPTY_VERTEX);
    }

    if inventory_open {
        let quad = |x0: f32, y0: f32, x1: f32, y1: f32, z: f32, rgba: [u8; 4]| {
            [
                Vertex {
                    position: [x0, y0, z],
                    uv: [0.0, 0.0],
                    rgba,
                },
                Vertex {
                    position: [x1, y0, z],
                    uv: [1.0, 0.0],
                    rgba,
                },
                Vertex {
                    position: [x1, y1, z],
                    uv: [1.0, 1.0],
                    rgba,
                },
                Vertex {
                    position: [x0, y0, z],
                    uv: [0.0, 0.0],
                    rgba,
                },
                Vertex {
                    position: [x1, y1, z],
                    uv: [1.0, 1.0],
                    rgba,
                },
                Vertex {
                    position: [x0, y1, z],
                    uv: [0.0, 1.0],
                    rgba,
                },
            ]
        };
        let panel = quad(-0.43, -0.34, 0.43, 0.34, -0.997, [18, 22, 28, 235]);
        let base = HOTBAR_VERTICES + CROSSHAIR_VERTICES + PAUSE_ICON_VERTICES;
        out[base..base + 6].copy_from_slice(&panel);
        for slot in 0..HOTBAR_SLOTS {
            let x = -0.31 + slot as f32 * 0.31;
            let colour = if slot == selected {
                [245, 205, 80, 255]
            } else {
                [90, 105, 125, 255]
            };
            let quad = quad(x - 0.105, -0.18, x + 0.105, 0.03, -0.996, colour);
            let offset = base + (slot + 1) * 6;
            out[offset..offset + 6].copy_from_slice(&quad);
        }
        hud_count = HUD_VERTICES;
    } else {
        let base = HOTBAR_VERTICES + CROSSHAIR_VERTICES + PAUSE_ICON_VERTICES;
        out[base..HUD_VERTICES].fill(EMPTY_VERTEX);
    }
    hud_count
}
