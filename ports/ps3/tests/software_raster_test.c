#define main ps3_bringup_entry
#include "../bringup.c"
#undef main

#include <assert.h>
#include <stdio.h>

int horizon_ps3_boot(void) { return -1; }
int horizon_ps3_display_size(uint16_t *width, uint16_t *height) {
    (void)width; (void)height; return -1;
}
int horizon_ps3_prepare_mesh_pipeline(void) { return -1; }
int horizon_ps3_upload_mesh(const uint8_t *bytes, uint32_t length, uint32_t *offset) {
    (void)bytes; (void)length; (void)offset; return -1;
}
int horizon_ps3_bind_mesh(uint32_t offset, uint32_t vertex_count) {
    (void)offset; (void)vertex_count; return -1;
}
int horizon_ps3_draw_mesh(uint32_t offset, uint32_t vertex_count) {
    (void)offset; (void)vertex_count; return -1;
}
int horizon_ps3_tty_write(const uint8_t *bytes, uint32_t length) {
    (void)bytes; (void)length; return -1;
}
int horizon_ps3_poll_pad(horizon_ps3_pad *out) { (void)out; return -1; }
int horizon_ps3_begin_frame(void) { return -1; }
int horizon_ps3_present_rgba8(const uint8_t *rgba8, uint16_t width, uint16_t height) {
    (void)rgba8; (void)width; (void)height; return -1;
}
void horizon_ps3_shutdown(void) {}

int main(void) {
    uint8_t pixels[3 * 3 * 4] = {0};
    float depth_buffer[3 * 3] = {0};
    const int sx[8] = {0, 2, 0, 0, 0, 0, 0, 0};
    const int sy[8] = {0, 0, 2, 0, 0, 0, 0, 0};
    const float inv_depth[8] = {1.0f, 1.0f, 1.0f, 0, 0, 0, 0, 0};
    const uint8_t red[3] = {255, 0, 0};
    const uint8_t blue[3] = {0, 0, 255};
    const uint8_t green[3] = {0, 255, 0};
    const size_t sample = (size_t)1 * 3 + 0;

    raster_triangle(pixels, depth_buffer, 3, 3, sx, sy, inv_depth, 0, 1, 2, red);
    assert(depth_buffer[sample] == 1.0f);
    assert(pixels[sample * 4 + 0] == 255 && pixels[sample * 4 + 1] == 0);

    float farther[8] = {0.5f, 0.5f, 0.5f, 0, 0, 0, 0, 0};
    raster_triangle(pixels, depth_buffer, 3, 3, sx, sy, farther, 0, 1, 2, blue);
    assert(pixels[sample * 4 + 0] == 255 && pixels[sample * 4 + 2] == 0);

    float nearer[8] = {2.0f, 2.0f, 2.0f, 0, 0, 0, 0, 0};
    raster_triangle(pixels, depth_buffer, 3, 3, sx, sy, nearer, 0, 1, 2, green);
    assert(depth_buffer[sample] == 2.0f);
    assert(pixels[sample * 4 + 1] == 255);
    puts("software triangle fill and reciprocal-depth occlusion: PASS");
    return 0;
}
