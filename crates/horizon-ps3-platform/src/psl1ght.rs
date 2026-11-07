#![allow(unsafe_code)]

//! FFI adapter for the C bridge in `ports/ps3/psl1ght_bridge.c`.
//!
//! Enable with the `psl1ght-ffi` feature and link that bridge object plus the
//! SDK libraries. This module is excluded from normal host builds.
use crate::camera::Camera;
use crate::chunk_mesh::{MeshQueue, MeshRequest};
use crate::chunk_store::ChunkStore;
use crate::frame_budget::FrameWorkBudget;
use crate::greedy_mesh::{GreedyQuad, mesh_opaque_greedy};
use crate::rsx_mesh::{
    MAX_CLIPPED_VERTICES_PER_GREEDY_QUAD, MeshUpload, VERTEX_BYTES, Vertex,
    expand_greedy_quad_triangles_camera_clipped,
};
use crate::{
    DisplayConfig, Frame, MemoryBudget, MemoryUsage, PadState, Platform, PlatformError, Ps3Api,
    Ps3Platform,
};

const RUST_ENTRY_WIDTH: usize = 1280;
const RUST_ENTRY_HEIGHT: usize = 720;
static RUST_ENTRY_FRAME: [u8; RUST_ENTRY_WIDTH * RUST_ENTRY_HEIGHT * 4] =
    [0; RUST_ENTRY_WIDTH * RUST_ENTRY_HEIGHT * 4];

/// Stable C-visible marker used by the experimental PPU link probe.
#[unsafe(no_mangle)]
pub extern "C" fn horizon_ps3_rust_abi_version() -> u32 {
    1
}

/// The PPU build is abort-only and has no unwinding runtime.
#[unsafe(no_mangle)]
pub extern "C" fn rust_eh_personality() {}

/// Minimal Rust-owned PPU entrypoint used before the full client is linked.
#[unsafe(no_mangle)]
pub extern "C" fn horizon_ps3_rust_smoke_entry() -> i32 {
    let mut api = match Psl1ghtApi::boot() {
        Ok(api) => api,
        Err(_) => return 10,
    };
    let display = api.display_config();
    if display.width as usize != RUST_ENTRY_WIDTH || display.height as usize != RUST_ENTRY_HEIGHT {
        return 11;
    }
    let camera = match Camera::new(
        [16.0, 8.0, -8.0],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
        1.428_148,
        f32::from(display.width) / f32::from(display.height),
        0.1,
        128.0,
    ) {
        Ok(camera) => camera,
        Err(_) => return 12,
    };
    const DEMO_BLOCK_EDGE: usize = 4;
    const DEMO_MAX_QUADS: usize = DEMO_BLOCK_EDGE * DEMO_BLOCK_EDGE * 6;
    const MAX_UPLOAD_BYTES: usize = 40_000;
    const MAX_UPLOAD_VERTICES: usize = MAX_UPLOAD_BYTES / VERTEX_BYTES;
    let mut frame_budget = FrameWorkBudget::new(3, MAX_UPLOAD_BYTES as u32);
    let request_a = MeshRequest {
        chunk_x: 0,
        chunk_y: 0,
        chunk_z: 0,
        revision: 1,
    };
    let request_b = MeshRequest {
        chunk_x: 1,
        chunk_y: 0,
        chunk_z: 0,
        revision: 1,
    };
    let mut store = ChunkStore::<2>::new();
    if store.insert(request_a).is_err() || store.insert(request_b).is_err() {
        return 12;
    }
    for z in 6..(6 + DEMO_BLOCK_EDGE) {
        for y in 6..(6 + DEMO_BLOCK_EDGE) {
            for local_x in 0..DEMO_BLOCK_EDGE {
                let x_a = 16 - DEMO_BLOCK_EDGE + local_x;
                let x_b = local_x;
                let block_id = if (local_x + y + z) % 2 == 0 { 1 } else { 2 };
                if !store.set_block(request_a, x_a, y, z, block_id)
                    || !store.set_block(request_b, x_b, y, z, block_id)
                {
                    return 12;
                }
            }
        }
    }
    for z in 6..(6 + DEMO_BLOCK_EDGE) {
        for y in 6..(6 + DEMO_BLOCK_EDGE) {
            if !store.set_halo(request_a, 17, y + 1, z + 1, 2)
                || !store.set_halo(request_b, 0, y + 1, z + 1, 1)
            {
                return 12;
            }
        }
    }
    let mut queue = MeshQueue::<2>::new();
    if store.drain_dirty(&mut queue).is_err() {
        return 12;
    }
    let queued_a = queue.pop().ok_or(());
    let queued_b = queue.pop().ok_or(());
    if queued_a != Ok(request_a) || queued_b != Ok(request_b) {
        return 12;
    }
    let grid_a = match store.grid(request_a) {
        Some(grid) => grid,
        None => return 12,
    };
    let grid_b = match store.grid(request_b) {
        Some(grid) => grid,
        None => return 12,
    };
    let empty_quad = GreedyQuad {
        x: 0,
        y: 0,
        z: 0,
        width: 0,
        height: 0,
        block_id: 0,
        face: crate::chunk_mesh::Face::NegativeX,
    };
    let mut quads_a = [empty_quad; DEMO_MAX_QUADS];
    let mut quads_b = [empty_quad; DEMO_MAX_QUADS];
    if !frame_budget.try_start_mesh() {
        return 12;
    }
    let quad_count_a = match mesh_opaque_greedy(grid_a, &mut quads_a) {
        Ok(count) => count,
        Err(_) => return 12,
    };
    if !frame_budget.try_start_mesh() {
        return 12;
    }
    let quad_count_b = match mesh_opaque_greedy(grid_b, &mut quads_b) {
        Ok(count) => count,
        Err(_) => return 12,
    };
    if quad_count_a == 0 || quad_count_b == 0 {
        return 12;
    }
    let empty_vertex = Vertex {
        position: [0.0; 3],
        uv: [0.0; 2],
        rgba: [0; 4],
    };
    let mut vertices = [empty_vertex; MAX_UPLOAD_VERTICES];
    let mut vertex_count: usize = 0;
    for (quads, chunk_offset) in [
        (&quads_a[..quad_count_a], [0.0, 0.0, 0.0]),
        (&quads_b[..quad_count_b], [16.0, 0.0, 0.0]),
    ] {
        for quad in quads {
            let mut expanded = [empty_vertex; MAX_CLIPPED_VERTICES_PER_GREEDY_QUAD];
            let expanded_count = match expand_greedy_quad_triangles_camera_clipped(
                *quad,
                chunk_offset,
                camera,
                &mut expanded,
            ) {
                Ok(count) => count,
                Err(_) => return 12,
            };
            let end = vertex_count.saturating_add(expanded_count);
            if end > vertices.len() {
                return 12;
            }
            vertices[vertex_count..end].copy_from_slice(&expanded[..expanded_count]);
            vertex_count = end;
        }
    }
    let mut encoded = [0_u8; MAX_UPLOAD_VERTICES * VERTEX_BYTES];
    let budget = MemoryBudget::initial();
    let usage = api.memory_usage();
    let upload = match MeshUpload::begin(&mut encoded, &vertices[..vertex_count], budget, usage) {
        Ok(upload) => upload,
        Err(_) => return 12,
    };
    if !frame_budget.try_upload(upload.len()) {
        return 12;
    }
    let offset = match api.upload_mesh(upload.bytes()) {
        Ok(offset) => offset,
        Err(_) => return 13,
    };
    if api.prepare_mesh_pipeline().is_err() {
        return 14;
    }
    if api.bind_mesh(offset, vertex_count as u32).is_err() {
        return 15;
    }
    if api.draw_mesh(offset, vertex_count as u32).is_err() {
        return 16;
    }
    if !store.set_block(request_a, 15, 6, 6, 3) {
        return 17;
    }
    let revision_two = match store.bump_revision(request_a) {
        Some(request) => request,
        None => return 17,
    };
    if revision_two.revision != 2 || !store.set_block(request_a, 14, 6, 6, 1) {
        return 17;
    }
    let updated_request = match store.bump_revision(revision_two) {
        Some(request) => request,
        None => return 17,
    };
    if updated_request.revision != 3 {
        return 17;
    }
    let mut update_queue = MeshQueue::<1>::new();
    if store.drain_dirty(&mut update_queue).is_err()
        || update_queue.pop() != Some(updated_request)
        || update_queue.pop().is_some()
        || store.take_dirty().is_some()
    {
        return 17;
    }
    let updated_grid = match store.grid(updated_request) {
        Some(grid) => grid,
        None => return 17,
    };
    let mut updated_quads = [empty_quad; DEMO_MAX_QUADS];
    if !frame_budget.try_start_mesh() {
        return 17;
    }
    let updated_quad_count = match mesh_opaque_greedy(updated_grid, &mut updated_quads) {
        Ok(count) if count > 0 => count,
        _ => return 17,
    };
    let mut updated_vertices = [empty_vertex; MAX_UPLOAD_VERTICES];
    let mut updated_vertex_count: usize = 0;
    for quad in &updated_quads[..updated_quad_count] {
        let mut expanded = [empty_vertex; MAX_CLIPPED_VERTICES_PER_GREEDY_QUAD];
        let expanded_count = match expand_greedy_quad_triangles_camera_clipped(
            *quad,
            [0.0, 0.0, 0.0],
            camera,
            &mut expanded,
        ) {
            Ok(count) => count,
            Err(_) => return 17,
        };
        let end = updated_vertex_count.saturating_add(expanded_count);
        if end > updated_vertices.len() {
            return 17;
        }
        updated_vertices[updated_vertex_count..end].copy_from_slice(&expanded[..expanded_count]);
        updated_vertex_count = end;
    }
    let mut updated_encoded = [0_u8; MAX_UPLOAD_VERTICES * VERTEX_BYTES];
    let updated_upload = match MeshUpload::begin(
        &mut updated_encoded,
        &updated_vertices[..updated_vertex_count],
        budget,
        api.memory_usage(),
    ) {
        Ok(upload) => upload,
        Err(_) => return 17,
    };
    if !frame_budget.try_upload(updated_upload.len()) {
        return 17;
    }
    let updated_offset = match api.upload_mesh(updated_upload.bytes()) {
        Ok(offset) => offset,
        Err(_) => return 17,
    };
    if api
        .bind_mesh(updated_offset, updated_vertex_count as u32)
        .is_err()
        || api
            .draw_mesh(updated_offset, updated_vertex_count as u32)
            .is_err()
    {
        return 17;
    }
    let mut platform = api.into_platform(MemoryBudget::initial());
    if platform
        .log(b"Rust PS3 entrypoint and mesh online\n")
        .is_err()
    {
        return 17;
    }
    let _ = platform.poll_pad();
    if platform.begin_frame().is_err() {
        return 18;
    }
    let frame = Frame {
        rgba8: &RUST_ENTRY_FRAME,
        width: RUST_ENTRY_WIDTH as u16,
        height: RUST_ENTRY_HEIGHT as u16,
    };
    if platform.present(frame).is_err() {
        return 19;
    }
    0
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct RawPad {
    cross: u8,
    circle: u8,
    square: u8,
    triangle: u8,
    l1: u8,
    r1: u8,
    l2: u8,
    r2: u8,
    start: u8,
    select: u8,
    dpad_x: i8,
    dpad_y: i8,
    left_x: i8,
    left_y: i8,
    right_x: i8,
    right_y: i8,
}

const _: [(); 16] = [(); core::mem::size_of::<RawPad>()];

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct RawMemoryStats {
    xdr_reserved: u32,
    rsx_reserved: u32,
}

const _: [(); 8] = [(); core::mem::size_of::<RawMemoryStats>()];

unsafe extern "C" {
    fn horizon_ps3_boot() -> i32;
    fn horizon_ps3_display_size(width: *mut u16, height: *mut u16) -> i32;
    fn horizon_ps3_get_memory_stats(out: *mut RawMemoryStats) -> i32;
    fn horizon_ps3_prepare_mesh_pipeline() -> i32;
    fn horizon_ps3_upload_mesh(bytes: *const u8, length: u32, offset: *mut u32) -> i32;
    fn horizon_ps3_bind_mesh(offset: u32, vertex_count: u32) -> i32;
    fn horizon_ps3_draw_mesh(offset: u32, vertex_count: u32) -> i32;
    fn horizon_ps3_alloc_region(bytes: u32, out: *mut *mut u8) -> i32;
    fn horizon_ps3_begin_scene(clear_argb: u32) -> i32;
    fn horizon_ps3_present_scene() -> i32;
    fn horizon_ps3_tty_write(bytes: *const u8, length: u32) -> i32;
    fn horizon_ps3_poll_pad(out: *mut RawPad) -> i32;
    fn horizon_ps3_begin_frame() -> i32;
    fn horizon_ps3_present_rgba8(rgba8: *const u8, width: u16, height: u16) -> i32;
    fn horizon_ps3_shutdown();
}

/// Fixed-size buffer so the panic path never allocates.
struct PanicBuf {
    bytes: [u8; 192],
    len: usize,
}

impl core::fmt::Write for PanicBuf {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let room = self.bytes.len() - self.len;
        let n = s.len().min(room);
        self.bytes[self.len..self.len + n].copy_from_slice(&s.as_bytes()[..n]);
        self.len += n;
        Ok(())
    }
}

/// Reports `file:line` on the PS3 TTY, then parks the PPU thread.
#[panic_handler]
fn psl1ght_panic(info: &core::panic::PanicInfo<'_>) -> ! {
    use core::fmt::Write;
    let mut buf = PanicBuf {
        bytes: [0; 192],
        len: 0,
    };
    let _ = buf.write_str("horizon panic: ");
    if let Some(loc) = info.location() {
        let _ = write!(buf, "{}:{}", loc.file(), loc.line());
    }
    let _ = buf.write_str("\n");
    // SAFETY: the buffer holds `len` initialized bytes for the duration of the call.
    unsafe { horizon_ps3_tty_write(buf.bytes.as_ptr(), buf.len as u32) };
    loop {
        core::hint::spin_loop();
    }
}

/// Safe Rust wrapper over the selected PSL1GHT C bridge.
///
pub struct Psl1ghtApi {
    display: DisplayConfig,
}

impl Psl1ghtApi {
    /// Boots PSL1GHT/RSX and captures the actual video mode chosen by the bridge.
    pub fn boot() -> Result<Self, PlatformError> {
        // SAFETY: the C bridge owns its singleton state and accepts no pointers here.
        let result = unsafe { horizon_ps3_boot() };
        if result != 0 {
            return Err(PlatformError::NotReady);
        }
        let mut width = 0;
        let mut height = 0;
        // SAFETY: both output pointers refer to live, writable u16 locals.
        let result = unsafe { horizon_ps3_display_size(&mut width, &mut height) };
        if result != 0 || width == 0 || height == 0 {
            // SAFETY: boot succeeded above, so the bridge owns an initialized pad/RSX state.
            unsafe { horizon_ps3_shutdown() };
            return Err(PlatformError::NotReady);
        }
        Ok(Self {
            display: DisplayConfig {
                width,
                height,
                buffers: 2,
                vsync: true,
            },
        })
    }

    pub const fn display_config(&self) -> DisplayConfig {
        self.display
    }

    /// Returns bridge-owned allocations, not the physical memory size.
    pub fn memory_usage(&self) -> MemoryUsage {
        let mut raw = RawMemoryStats::default();
        // SAFETY: raw is a valid writable C-layout structure for the bridge.
        if unsafe { horizon_ps3_get_memory_stats(&mut raw) } != 0 {
            return MemoryUsage::default();
        }
        MemoryUsage {
            xdr_reserved: raw.xdr_reserved,
            gddr3_reserved: raw.rsx_reserved,
        }
    }

    /// Uploads original geometry into the bridge's bounded persistent RSX slot.
    pub fn upload_mesh(&mut self, bytes: &[u8]) -> Result<u32, PlatformError> {
        let length = u32::try_from(bytes.len()).map_err(|_| PlatformError::OutOfMemory)?;
        let mut offset = 0;
        // SAFETY: the bridge copies bytes and retains only its own RSX allocation.
        let result = unsafe { horizon_ps3_upload_mesh(bytes.as_ptr(), length, &mut offset) };
        match result {
            0 => Ok(offset),
            -2 | -3 => Err(PlatformError::OutOfMemory),
            _ => Err(PlatformError::NotReady),
        }
    }

    /// Reserves one 128-byte-aligned XDR block for the Rust heap or arenas.
    /// The block is never freed; hand it to `RegionAllocator::init` or carve it
    /// up with `BumpArena` offsets.
    pub fn alloc_heap_region(&mut self, bytes: u32) -> Result<*mut u8, PlatformError> {
        let mut block: *mut u8 = core::ptr::null_mut();
        // SAFETY: `block` is a valid out-pointer for the duration of the call.
        match unsafe { horizon_ps3_alloc_region(bytes, &mut block) } {
            0 if !block.is_null() => Ok(block),
            -2 => Err(PlatformError::OutOfMemory),
            _ => Err(PlatformError::NotReady),
        }
    }

    /// Opt-in per-frame scene setup (viewport, clear colour and depth, depth
    /// test on). Call after `begin_rsx_frame`; finish with `present_scene`.
    pub fn begin_scene(&mut self, clear_argb: u32) -> Result<(), PlatformError> {
        // SAFETY: plain FFI call with a by-value argument.
        if unsafe { horizon_ps3_begin_scene(clear_argb) } == 0 {
            Ok(())
        } else {
            Err(PlatformError::NotReady)
        }
    }

    /// Flips to the frame drawn since `begin_scene`, without the software
    /// framebuffer copy that `present_rsx_frame` performs.
    pub fn present_scene(&mut self) -> Result<(), PlatformError> {
        // SAFETY: plain FFI call with no arguments.
        if unsafe { horizon_ps3_present_scene() } == 0 {
            Ok(())
        } else {
            Err(PlatformError::NotReady)
        }
    }

    /// Confirms that the embedded bring-up programs were loaded into RSX.
    pub fn prepare_mesh_pipeline(&mut self) -> Result<(), PlatformError> {
        if unsafe { horizon_ps3_prepare_mesh_pipeline() } == 0 {
            Ok(())
        } else {
            Err(PlatformError::NotReady)
        }
    }

    /// Binds the position and RGBA attributes of the persistent RSX mesh.
    /// A vertex program and draw call are intentionally a separate step.
    pub fn bind_mesh(&mut self, offset: u32, vertex_count: u32) -> Result<(), PlatformError> {
        let result = unsafe { horizon_ps3_bind_mesh(offset, vertex_count) };
        match result {
            0 => Ok(()),
            -2 => Err(PlatformError::InvalidFrame),
            _ => Err(PlatformError::NotReady),
        }
    }

    /// Emits indexed triangles from the persistent mesh after binding it.
    pub fn draw_mesh(&mut self, offset: u32, vertex_count: u32) -> Result<(), PlatformError> {
        if vertex_count < 3 || vertex_count % 3 != 0 {
            return Err(PlatformError::InvalidFrame);
        }
        match unsafe { horizon_ps3_draw_mesh(offset, vertex_count) } {
            0 => Ok(()),
            -2 => Err(PlatformError::InvalidFrame),
            _ => Err(PlatformError::NotReady),
        }
    }

    /// Transfers ownership into the game-facing platform wrapper.
    pub fn into_platform(self, budget: MemoryBudget) -> Ps3Platform<Self> {
        let display = self.display;
        Ps3Platform::new(self, display, budget)
    }
}

impl Drop for Psl1ghtApi {
    fn drop(&mut self) {
        // SAFETY: this adapter owns the bridge singleton and drop ends its pad session.
        unsafe { horizon_ps3_shutdown() };
    }
}

impl Ps3Api for Psl1ghtApi {
    fn tty_write(&mut self, bytes: &[u8]) -> Result<(), PlatformError> {
        let length = u32::try_from(bytes.len()).map_err(|_| PlatformError::Unsupported)?;
        // SAFETY: bytes points to `length` initialized bytes for the duration of the call.
        let result = unsafe { horizon_ps3_tty_write(bytes.as_ptr(), length) };
        if result == 0 {
            Ok(())
        } else {
            Err(PlatformError::Unsupported)
        }
    }

    fn memory_available(&self) -> u32 {
        0
    }

    fn memory_usage(&self) -> MemoryUsage {
        self.memory_usage()
    }

    fn pad_state(&mut self) -> PadState {
        let mut raw = RawPad::default();
        // SAFETY: raw is a valid writable C-layout structure matching the bridge header.
        if unsafe { horizon_ps3_poll_pad(&mut raw) } != 0 {
            return PadState::default();
        }
        PadState {
            cross: raw.cross != 0,
            circle: raw.circle != 0,
            square: raw.square != 0,
            triangle: raw.triangle != 0,
            l1: raw.l1 != 0,
            r1: raw.r1 != 0,
            l2: raw.l2,
            r2: raw.r2,
            start: raw.start != 0,
            select: raw.select != 0,
            dpad_x: raw.dpad_x,
            dpad_y: raw.dpad_y,
            left_x: raw.left_x,
            left_y: raw.left_y,
            right_x: raw.right_x,
            right_y: raw.right_y,
        }
    }

    fn begin_rsx_frame(&mut self, _display: DisplayConfig) -> Result<(), PlatformError> {
        // SAFETY: the bridge checks its initialized state and owns all RSX state.
        if unsafe { horizon_ps3_begin_frame() } == 0 {
            Ok(())
        } else {
            Err(PlatformError::NotReady)
        }
    }

    fn present_rsx_frame(&mut self, frame: Frame<'_>) -> Result<(), PlatformError> {
        if !frame.is_valid_for(self.display) {
            return Err(PlatformError::InvalidFrame);
        }
        // SAFETY: frame validation occurs in `Ps3Platform`; the bridge reads exactly
        // width*height*4 bytes and does not retain the pointer after returning.
        if unsafe { horizon_ps3_present_rgba8(frame.rgba8.as_ptr(), frame.width, frame.height) }
            == 0
        {
            Ok(())
        } else {
            Err(PlatformError::InvalidFrame)
        }
    }
}
