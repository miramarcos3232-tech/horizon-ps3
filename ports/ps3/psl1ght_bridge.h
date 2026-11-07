#ifndef HORIZON_PS3_PSL1GHT_BRIDGE_H
#define HORIZON_PS3_PSL1GHT_BRIDGE_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct horizon_ps3_pad {
    uint8_t cross;
    uint8_t circle;
    uint8_t square;
    uint8_t triangle;
    uint8_t l1;
    uint8_t r1;
    uint8_t l2;
    uint8_t r2;
    uint8_t start;
    uint8_t select;
    int8_t dpad_x;
    int8_t dpad_y;
    int8_t left_x;
    int8_t left_y;
    int8_t right_x;
    int8_t right_y;
} horizon_ps3_pad;

typedef struct horizon_ps3_memory_stats {
    uint32_t xdr_reserved;
    uint32_t rsx_reserved;
} horizon_ps3_memory_stats;

uint32_t horizon_ps3_rust_abi_version(void);
int32_t horizon_ps3_rust_smoke_entry(void);
int32_t horizon_ps3_rust_world_entry(void);

/* Return zero on success. The function configures a 1280x720-or-nearest mode. */
int horizon_ps3_boot(void);
int horizon_ps3_display_size(uint16_t *width, uint16_t *height);
int horizon_ps3_get_memory_stats(horizon_ps3_memory_stats *out);
int horizon_ps3_prepare_mesh_pipeline(void);
int horizon_ps3_upload_mesh(const uint8_t *bytes, uint32_t length, uint32_t *offset);
int horizon_ps3_bind_mesh(uint32_t offset, uint32_t vertex_count);
int horizon_ps3_draw_mesh(uint32_t offset, uint32_t vertex_count);
/* 128-byte aligned XDR block for the Rust heap; zero on success. */
int horizon_ps3_alloc_region(uint32_t bytes, void **out);
/* Opt-in per-frame path for the world client (unverified on hardware):
 * begin_scene sets viewport/scissor, clears colour (ARGB) and depth, and
 * enables the depth test; present_scene flips without the software copy. */
int horizon_ps3_begin_scene(uint32_t clear_argb);
int horizon_ps3_present_scene(void);
int horizon_ps3_tty_write(const uint8_t *bytes, uint32_t length);
int horizon_ps3_poll_pad(horizon_ps3_pad *out);
int horizon_ps3_begin_frame(void);
int horizon_ps3_present_rgba8(const uint8_t *rgba8, uint16_t width, uint16_t height);
void horizon_ps3_shutdown(void);

#ifdef __cplusplus
}
#endif

#endif
