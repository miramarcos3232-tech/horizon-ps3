#include "psl1ght_bridge.h"

#include <io/pad.h>
#include <rsx/rsx.h>
#include <rsx/rsx_program.h>
#include <sysutil/video.h>

#include <malloc.h>
#include <string.h>
#include <unistd.h>

#define HORIZON_CB_SIZE (0x100000)
#define HORIZON_IO_SIZE (0x100000)
#define HORIZON_BUFFER_COUNT 2
#define HORIZON_WIDTH 1280
#define HORIZON_HEIGHT 720
#define HORIZON_COLOR_BYTES 4
#define HORIZON_DEPTH_BYTES 4
#define HORIZON_MAX_MESH_UPLOAD (4 * 1024 * 1024)
#define HORIZON_VERTEX_BYTES 24
#define HORIZON_MAX_MESH_VERTICES 65535

extern const uint8_t horizon_vertex_blob_start[];
extern const uint8_t horizon_vertex_blob_end[];
extern const uint8_t horizon_fragment_blob_start[];
extern const uint8_t horizon_fragment_blob_end[];

typedef struct horizon_ps3_state {
    gcmContextData *context;
    void *io_buffer;
    uint32_t *color[HORIZON_BUFFER_COUNT];
    uint32_t color_offset[HORIZON_BUFFER_COUNT];
    uint32_t depth_offset;
    uint32_t color_pitch;
    uint32_t depth_pitch;
    uint8_t *mesh_buffer;
    uint32_t mesh_offset;
    uint32_t mesh_capacity;
    uint32_t mesh_bytes;
    uint32_t mesh_bound_offset;
    uint32_t mesh_bound_vertices;
    uint16_t *index_buffer;
    uint32_t index_offset;
    uint32_t index_capacity;
    void *fragment_buffer;
    uint32_t fragment_offset;
    uint32_t fragment_capacity;
    uint8_t programs_loaded;
    uint16_t width;
    uint16_t height;
    uint8_t current_buffer;
    uint8_t first_flip;
    uint8_t pad_initialized;
    uint8_t rsx_started;
    uint8_t booted;
    uint8_t boot_failed;
} horizon_ps3_state;

static horizon_ps3_state state;

static int load_bringup_programs(void) {
    const rsxVertexProgram *vertex_program;
    const rsxFragmentProgram *fragment_program;
    void *vertex_ucode;
    void *fragment_ucode;
    uint32_t vertex_ucode_size;
    uint32_t fragment_ucode_size;
    uint32_t vertex_blob_size = (uint32_t)(horizon_vertex_blob_end - horizon_vertex_blob_start);
    uint32_t fragment_blob_size = (uint32_t)(horizon_fragment_blob_end - horizon_fragment_blob_start);

    if (vertex_blob_size < sizeof(rsxVertexProgram) || fragment_blob_size < sizeof(rsxFragmentProgram)) return -1;
    vertex_program = (const rsxVertexProgram *)horizon_vertex_blob_start;
    fragment_program = (const rsxFragmentProgram *)horizon_fragment_blob_start;
    rsxVertexProgramGetUCode(vertex_program, &vertex_ucode, &vertex_ucode_size);
    rsxFragmentProgramGetUCode(fragment_program, &fragment_ucode, &fragment_ucode_size);
    if (!vertex_ucode || vertex_ucode_size == 0 || !fragment_ucode || fragment_ucode_size == 0) return -2;
    state.fragment_buffer = rsxMemalign(64, fragment_ucode_size);
    if (!state.fragment_buffer) return -3;
    if (rsxAddressToOffset(state.fragment_buffer, &state.fragment_offset) != 0) return -4;
    memcpy(state.fragment_buffer, fragment_ucode, fragment_ucode_size);
    state.fragment_capacity = fragment_ucode_size;
    rsxLoadVertexProgram(state.context, vertex_program, vertex_ucode);
    rsxLoadFragmentProgramLocation(state.context, fragment_program, state.fragment_offset, GCM_LOCATION_RSX);
    state.programs_loaded = 1;
    return 0;
}

static int configure_video(void) {
    videoResolution resolution;
    videoConfiguration configuration;
    int resolution_id = 0;
    int result;

    /* 720p is the first profile; fall back to the first available mode. */
    result = videoGetResolution(VIDEO_RESOLUTION_720, &resolution);
    if (result != 0) {
        result = videoGetResolution(VIDEO_RESOLUTION_576, &resolution);
        if (result != 0) return result;
        resolution_id = VIDEO_RESOLUTION_576;
    } else {
        resolution_id = VIDEO_RESOLUTION_720;
    }

    memset(&configuration, 0, sizeof(configuration));
    configuration.resolution = (uint8_t)resolution_id;
    configuration.format = VIDEO_BUFFER_FORMAT_XRGB;
    configuration.aspect = VIDEO_ASPECT_AUTO;
    configuration.pitch = (uint32_t)resolution.width * HORIZON_COLOR_BYTES;
    result = videoConfigure(VIDEO_PRIMARY, &configuration, NULL, 0);
    if (result != 0) return result;

    state.width = resolution.width;
    state.height = resolution.height;
    state.color_pitch = (uint32_t)state.width * HORIZON_COLOR_BYTES;
    state.depth_pitch = (uint32_t)state.width * HORIZON_DEPTH_BYTES;
    return 0;
}

static void set_render_target(uint8_t index) {
    gcmSurface surface;
    memset(&surface, 0, sizeof(surface));
    surface.colorFormat = GCM_SURFACE_X8R8G8B8;
    surface.colorTarget = GCM_SURFACE_TARGET_0;
    surface.colorLocation[0] = GCM_LOCATION_RSX;
    surface.colorOffset[0] = state.color_offset[index];
    surface.colorPitch[0] = state.color_pitch;
    surface.colorLocation[1] = GCM_LOCATION_RSX;
    surface.colorLocation[2] = GCM_LOCATION_RSX;
    surface.colorLocation[3] = GCM_LOCATION_RSX;
    surface.colorPitch[1] = 64;
    surface.colorPitch[2] = 64;
    surface.colorPitch[3] = 64;
    surface.depthFormat = GCM_SURFACE_ZETA_Z24S8;
    surface.depthLocation = GCM_LOCATION_RSX;
    surface.depthOffset = state.depth_offset;
    surface.depthPitch = state.depth_pitch;
    surface.type = GCM_SURFACE_TYPE_LINEAR;
    surface.antiAlias = GCM_SURFACE_CENTER_1;
    surface.width = state.width;
    surface.height = state.height;
    rsxSetSurface(state.context, &surface);
}

static int fail_boot(int error) {
    if (state.pad_initialized) {
        ioPadEnd();
        state.pad_initialized = 0;
    }
    if (state.rsx_started) {
        /* PSL1GHT teardown is not selected yet; retain state and refuse retries. */
        state.boot_failed = 1;
    } else {
        free(state.io_buffer);
        memset(&state, 0, sizeof(state));
    }
    return error;
}

int horizon_ps3_boot(void) {
    uint32_t io_size = HORIZON_IO_SIZE;
    uint32_t i;

    if (state.booted) return -10;
    if (state.boot_failed) return -9;
    memset(&state, 0, sizeof(state));
    if (ioPadInit(1) != 0) return fail_boot(-1);
    state.pad_initialized = 1;
    state.io_buffer = memalign(1024 * 1024, io_size);
    if (!state.io_buffer) return fail_boot(-2);
    if (configure_video() != 0) return fail_boot(-3);
    /* Conservatively latch this point: a failed RSX init may be non-retryable. */
    state.rsx_started = 1;
    if (rsxInit(&state.context, HORIZON_CB_SIZE, io_size, state.io_buffer) != 0) return fail_boot(-4);
    if (load_bringup_programs() != 0) return fail_boot(-9);

    gcmSetFlipMode(GCM_FLIP_VSYNC);
    for (i = 0; i < HORIZON_BUFFER_COUNT; ++i) {
        state.color[i] = (uint32_t *)rsxMemalign(64, state.height * state.color_pitch);
        if (!state.color[i]) return fail_boot(-5);
        if (rsxAddressToOffset(state.color[i], &state.color_offset[i]) != 0) return fail_boot(-6);
        if (gcmSetDisplayBuffer(i, state.color_offset[i], state.color_pitch, state.width, state.height) != 0) return fail_boot(-7);
        memset(state.color[i], 0, state.height * state.color_pitch);
    }

    state.depth_offset = 0;
    {
        void *depth = rsxMemalign(64, state.height * state.depth_pitch);
        if (!depth || rsxAddressToOffset(depth, &state.depth_offset) != 0) return fail_boot(-8);
    }
    set_render_target(0);
    state.booted = 1;
    state.first_flip = 1;
    return 0;
}

int horizon_ps3_display_size(uint16_t *width, uint16_t *height) {
    if (!state.booted || !width || !height) return -1;
    *width = state.width;
    *height = state.height;
    return 0;
}

int horizon_ps3_get_memory_stats(horizon_ps3_memory_stats *out) {
    uint32_t color_bytes;
    uint32_t depth_bytes;
    if (!state.booted || !out) return -1;
    color_bytes = (uint32_t)state.height * state.color_pitch * HORIZON_BUFFER_COUNT;
    depth_bytes = (uint32_t)state.height * state.depth_pitch;
    out->xdr_reserved = HORIZON_IO_SIZE;
    out->rsx_reserved = color_bytes + depth_bytes + state.mesh_capacity + state.fragment_capacity + state.index_capacity;
    return 0;
}

int horizon_ps3_prepare_mesh_pipeline(void) {
    if (!state.booted || !state.programs_loaded) return -1;
    return 0;
}

int horizon_ps3_upload_mesh(const uint8_t *bytes, uint32_t length, uint32_t *offset) {
    uint32_t vertex_count;
    uint32_t i;
    if (!state.booted || !bytes || !offset || length == 0) return -1;
    if (length > HORIZON_MAX_MESH_UPLOAD) return -2;
    if (length % HORIZON_VERTEX_BYTES != 0) return -2;
    vertex_count = length / HORIZON_VERTEX_BYTES;
    if (vertex_count > HORIZON_MAX_MESH_VERTICES) return -2;
    if (!state.mesh_buffer) {
        state.mesh_buffer = (uint8_t *)rsxMemalign(64, HORIZON_MAX_MESH_UPLOAD);
        if (!state.mesh_buffer) return -3;
        if (rsxAddressToOffset(state.mesh_buffer, &state.mesh_offset) != 0) return -3;
        state.mesh_capacity = HORIZON_MAX_MESH_UPLOAD;
    }
    if (!state.index_buffer) {
        state.index_capacity = HORIZON_MAX_MESH_VERTICES * sizeof(uint16_t);
        state.index_buffer = (uint16_t *)rsxMemalign(64, state.index_capacity);
        if (!state.index_buffer) return -3;
        if (rsxAddressToOffset(state.index_buffer, &state.index_offset) != 0) return -3;
        for (i = 0; i < HORIZON_MAX_MESH_VERTICES; ++i) state.index_buffer[i] = (uint16_t)i;
    }
    memcpy(state.mesh_buffer, bytes, length);
    state.mesh_bytes = length;
    *offset = state.mesh_offset;
    return 0;
}

int horizon_ps3_bind_mesh(uint32_t offset, uint32_t vertex_count) {
    uint32_t required;
    if (!state.booted || !state.mesh_buffer || vertex_count == 0) return -1;
    if (offset != state.mesh_offset) return -2;
    if (vertex_count > UINT32_MAX / HORIZON_VERTEX_BYTES) return -2;
    required = vertex_count * HORIZON_VERTEX_BYTES;
    if (required > state.mesh_bytes) return -2;
    rsxBindVertexArrayAttrib(state.context, GCM_VERTEX_ATTRIB_POS, 0, offset, HORIZON_VERTEX_BYTES, 3,
                             GCM_VERTEX_DATA_TYPE_F32, GCM_LOCATION_RSX);
    rsxBindVertexArrayAttrib(state.context, GCM_VERTEX_ATTRIB_TEX0, 0, offset + 12, HORIZON_VERTEX_BYTES, 2,
                             GCM_VERTEX_DATA_TYPE_F32, GCM_LOCATION_RSX);
    rsxBindVertexArrayAttrib(state.context, GCM_VERTEX_ATTRIB_COLOR0, 0, offset + 20, HORIZON_VERTEX_BYTES, 4,
                             GCM_VERTEX_DATA_TYPE_U8, GCM_LOCATION_RSX);
    return 0;
}

int horizon_ps3_draw_mesh(uint32_t offset, uint32_t vertex_count) {
    if (!state.booted || !state.programs_loaded || !state.index_buffer || vertex_count < 3) return -1;
    if (offset != state.mesh_offset || vertex_count % 3 != 0 || vertex_count > HORIZON_MAX_MESH_VERTICES) return -2;
    if (vertex_count * HORIZON_VERTEX_BYTES > state.mesh_bytes) return -2;
    rsxDrawIndexArray(state.context, GCM_TYPE_TRIANGLES, state.index_offset, vertex_count,
                      GCM_INDEX_TYPE_16B, GCM_LOCATION_RSX);
    state.mesh_bound_offset = offset;
    state.mesh_bound_vertices = vertex_count;
    return 0;
}

int horizon_ps3_tty_write(const uint8_t *bytes, uint32_t length) {
    uint32_t written = 0;
    if (length != 0 && !bytes) return -1;
    while (written < length) {
        ssize_t count = write(STDOUT_FILENO, bytes + written, length - written);
        if (count <= 0) return -1;
        written += (uint32_t)count;
    }
    return 0;
}

int horizon_ps3_poll_pad(horizon_ps3_pad *out) {
    padInfo info;
    padData data;
    if (!state.booted || !out) return -1;
    memset(out, 0, sizeof(*out));
    if (ioPadGetInfo(&info) != 0 || info.status[0] == 0) return 0;
    if (ioPadGetData(0, &data) != 0) return 0;
    out->cross = data.BTN_CROSS;
    out->circle = data.BTN_CIRCLE;
    out->square = data.BTN_SQUARE;
    out->triangle = data.BTN_TRIANGLE;
    out->l1 = data.BTN_L1;
    out->r1 = data.BTN_R1;
    out->l2 = data.BTN_L2;
    out->r2 = data.BTN_R2;
    out->start = data.BTN_START;
    out->select = data.BTN_SELECT;
    out->dpad_x = (int8_t)data.BTN_RIGHT - (int8_t)data.BTN_LEFT;
    out->dpad_y = (int8_t)data.BTN_DOWN - (int8_t)data.BTN_UP;
    out->left_x = (int8_t)data.ANA_L_H - 128;
    out->left_y = (int8_t)data.ANA_L_V - 128;
    out->right_x = (int8_t)data.ANA_R_H - 128;
    out->right_y = (int8_t)data.ANA_R_V - 128;
    return 0;
}

int horizon_ps3_begin_frame(void) {
    if (!state.booted) return -1;
    if (!state.first_flip) {
        while (gcmGetFlipStatus() != 0) usleep(200);
        gcmResetFlipStatus();
    } else {
        gcmResetFlipStatus();
    }
    set_render_target(state.current_buffer);
    return 0;
}

int horizon_ps3_present_rgba8(const uint8_t *rgba8, uint16_t width, uint16_t height) {
    uint32_t row;
    if (!state.booted || !rgba8 || width != state.width || height != state.height) return -1;
    for (row = 0; row < height; ++row) {
        memcpy(((uint8_t *)state.color[state.current_buffer]) + row * state.color_pitch,
               rgba8 + row * width * HORIZON_COLOR_BYTES,
               width * HORIZON_COLOR_BYTES);
    }
    if (state.mesh_bound_vertices != 0) {
        rsxDrawIndexArray(state.context, GCM_TYPE_TRIANGLES, state.index_offset,
                          state.mesh_bound_vertices, GCM_INDEX_TYPE_16B, GCM_LOCATION_RSX);
    }
    gcmSetFlip(state.context, state.current_buffer);
    rsxFlushBuffer(state.context);
    gcmSetWaitFlip(state.context);
    state.current_buffer ^= 1;
    state.first_flip = 0;
    return 0;
}

int horizon_ps3_alloc_region(uint32_t bytes, void **out) {
    if (bytes == 0u || out == NULL) return -1;
    void *block = memalign(128, bytes);
    if (block == NULL) return -2;
    *out = block;
    return 0;
}

int horizon_ps3_begin_scene(uint32_t clear_argb) {
    float scale[4];
    float offset[4];
    if (!state.booted) return -1;
    scale[0] = state.width * 0.5f;
    scale[1] = state.height * -0.5f;
    scale[2] = 0.5f;
    scale[3] = 0.0f;
    offset[0] = state.width * 0.5f;
    offset[1] = state.height * 0.5f;
    offset[2] = 0.5f;
    offset[3] = 0.0f;
    rsxSetViewport(state.context, 0, 0, state.width, state.height, 0.0f, 1.0f, scale, offset);
    rsxSetScissor(state.context, 0, 0, state.width, state.height);
    rsxSetClearColor(state.context, clear_argb);
    rsxSetClearDepthStencil(state.context, 0xffffff00u);
    rsxClearSurface(state.context, GCM_CLEAR_R | GCM_CLEAR_G | GCM_CLEAR_B | GCM_CLEAR_A |
                                       GCM_CLEAR_S | GCM_CLEAR_Z);
    rsxSetDepthTestEnable(state.context, GCM_TRUE);
    rsxSetDepthFunc(state.context, GCM_LESS);
    rsxSetDepthWriteEnable(state.context, GCM_TRUE);
    return 0;
}

int horizon_ps3_present_scene(void) {
    if (!state.booted) return -1;
    gcmSetFlip(state.context, state.current_buffer);
    rsxFlushBuffer(state.context);
    gcmSetWaitFlip(state.context);
    state.current_buffer ^= 1;
    state.first_flip = 0;
    return 0;
}

void horizon_ps3_shutdown(void) {
    /* RSX allocations are reclaimed by process exit; do not permit a leaky re-boot. */
    if (state.boot_failed) return;
    if (state.pad_initialized) {
        ioPadEnd();
        state.pad_initialized = 0;
    }
    if (state.rsx_started) {
        state.booted = 0;
        state.boot_failed = 1;
        return;
    }
    memset(&state, 0, sizeof(state));
}
