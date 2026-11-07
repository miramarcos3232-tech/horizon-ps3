#include "psl1ght_bridge.h"

#include <math.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

/*
 * Hardware smoke test for the PS3 platform boundary.
 * SQUARE cycles colour palettes; START exits.
 * This is deliberately not the Horizon game renderer.
 */
static float edge_function(float ax, float ay, float bx, float by, float px, float py) {
    return (px - ax) * (by - ay) - (py - ay) * (bx - ax);
}

static void raster_triangle(uint8_t *pixels, float *depth_buffer, uint16_t width,
                            uint16_t height, const int sx[8], const int sy[8],
                            const float inverse_depth[8], uint8_t a, uint8_t b,
                            uint8_t c, const uint8_t color[3]) {
    const float ax = (float)sx[a], ay = (float)sy[a];
    const float bx = (float)sx[b], by = (float)sy[b];
    const float cx = (float)sx[c], cy = (float)sy[c];
    const float area = edge_function(ax, ay, bx, by, cx, cy);
    if (area == 0.0f) return;

    int min_x = sx[a], max_x = sx[a];
    int min_y = sy[a], max_y = sy[a];
    if (sx[b] < min_x) min_x = sx[b];
    if (sx[c] < min_x) min_x = sx[c];
    if (sx[b] > max_x) max_x = sx[b];
    if (sx[c] > max_x) max_x = sx[c];
    if (sy[b] < min_y) min_y = sy[b];
    if (sy[c] < min_y) min_y = sy[c];
    if (sy[b] > max_y) max_y = sy[b];
    if (sy[c] > max_y) max_y = sy[c];
    if (min_x < 0) min_x = 0;
    if (min_y < 0) min_y = 0;
    if (max_x >= width) max_x = width - 1;
    if (max_y >= height) max_y = height - 1;

    for (int y = min_y; y <= max_y; ++y) {
        for (int x = min_x; x <= max_x; ++x) {
            const float px = (float)x + 0.5f;
            const float py = (float)y + 0.5f;
            const float wa = edge_function(bx, by, cx, cy, px, py) / area;
            const float wb = edge_function(cx, cy, ax, ay, px, py) / area;
            const float wc = edge_function(ax, ay, bx, by, px, py) / area;
            if (wa < 0.0f || wb < 0.0f || wc < 0.0f) continue;

            const float depth = wa * inverse_depth[a] + wb * inverse_depth[b] + wc * inverse_depth[c];
            const size_t index = (size_t)y * width + (size_t)x;
            if (depth <= depth_buffer[index]) continue;
            depth_buffer[index] = depth;
            const size_t offset = index * 4u;
            pixels[offset + 0] = color[0];
            pixels[offset + 1] = color[1];
            pixels[offset + 2] = color[2];
            pixels[offset + 3] = 255;
        }
    }
}

static void render_scene(uint8_t *pixels, float *depth_buffer, uint16_t width,
                         uint16_t height, uint8_t palette, float angle) {
    static const uint8_t palette_colors[3][6][3] = {
        {{255, 0, 0}, {0, 255, 0}, {0, 0, 255}, {255, 255, 0}, {255, 0, 255}, {0, 255, 255}},
        {{255, 255, 255}, {192, 192, 192}, {128, 128, 128}, {96, 96, 96}, {48, 48, 48}, {0, 0, 0}},
        {{255, 128, 0}, {128, 0, 255}, {0, 192, 255}, {128, 255, 0}, {255, 0, 96}, {32, 32, 32}}
    };
    static const float vertices[8][3] = {
        {-1.0f, -1.0f, -1.0f}, { 1.0f, -1.0f, -1.0f},
        { 1.0f,  1.0f, -1.0f}, {-1.0f,  1.0f, -1.0f},
        {-1.0f, -1.0f,  1.0f}, { 1.0f, -1.0f,  1.0f},
        { 1.0f,  1.0f,  1.0f}, {-1.0f,  1.0f,  1.0f}
    };
    static const uint8_t triangles[12][3] = {
        {0, 1, 2}, {0, 2, 3}, {5, 4, 7}, {5, 7, 6},
        {4, 0, 3}, {4, 3, 7}, {1, 5, 6}, {1, 6, 2},
        {3, 2, 6}, {3, 6, 7}, {4, 5, 1}, {4, 1, 0}
    };
    static const uint8_t edges[12][2] = {
        {0, 1}, {1, 2}, {2, 3}, {3, 0}, {4, 5}, {5, 6},
        {6, 7}, {7, 4}, {0, 4}, {1, 5}, {2, 6}, {3, 7}
    };
    static const uint8_t face_for_triangle[12] = {0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5};
    int screen_x[8];
    int screen_y[8];
    float inverse_depth[8];
    const float sin_y = sinf(angle);
    const float cos_y = cosf(angle);
    const float sin_x = sinf(angle * 0.73f);
    const float cos_x = cosf(angle * 0.73f);
    const float scale = (float)(height < width ? height : width) * 0.30f;

    for (uint32_t y = 0; y < height; ++y) {
        for (uint32_t x = 0; x < width; ++x) {
            const size_t index = (size_t)y * width + x;
            const uint32_t band = (x * 6u) / width;
            const uint8_t *color = palette_colors[palette % 3u][band];
            const size_t offset = index * 4u;
            pixels[offset + 0] = color[0];
            pixels[offset + 1] = color[1];
            pixels[offset + 2] = color[2];
            pixels[offset + 3] = 255;
            depth_buffer[index] = 0.0f;
        }
    }

    for (uint32_t i = 0; i < 8; ++i) {
        const float x = vertices[i][0] * cos_y + vertices[i][2] * sin_y;
        const float z = -vertices[i][0] * sin_y + vertices[i][2] * cos_y;
        const float y = vertices[i][1] * cos_x - z * sin_x;
        const float depth = 4.0f + vertices[i][1] * sin_x + z * cos_x;
        inverse_depth[i] = 1.0f / depth;
        screen_x[i] = (int)((float)width * 0.5f + scale * x * inverse_depth[i]);
        screen_y[i] = (int)((float)height * 0.5f - scale * y * inverse_depth[i]);
    }

    /* Two triangles per face; reciprocal depth selects the nearest surface. */
    for (uint32_t triangle = 0; triangle < 12; ++triangle) {
        raster_triangle(pixels, depth_buffer, width, height, screen_x, screen_y,
                        inverse_depth, triangles[triangle][0], triangles[triangle][1],
                        triangles[triangle][2], palette_colors[palette % 3u][face_for_triangle[triangle]]);
    }

    /* White edges retain a crisp silhouette, but back edges fail the depth test. */
    for (uint32_t edge = 0; edge < 12; ++edge) {
        const uint8_t start_vertex = edges[edge][0];
        const uint8_t end_vertex = edges[edge][1];
        int x0 = screen_x[edges[edge][0]];
        int y0 = screen_y[edges[edge][0]];
        const int x1 = screen_x[edges[edge][1]];
        const int y1 = screen_y[edges[edge][1]];
        const int dx = abs(x1 - x0);
        const int steps = dx > abs(y1 - y0) ? dx : abs(y1 - y0);
        const int sx = x0 < x1 ? 1 : -1;
        const int dy = -abs(y1 - y0);
        const int sy = y0 < y1 ? 1 : -1;
        int error = dx + dy;
        int step = 0;
        for (;;) {
            if (x0 >= 0 && y0 >= 0 && x0 < width && y0 < height) {
                const float t = steps == 0 ? 0.0f : (float)step / (float)steps;
                const float edge_depth = inverse_depth[start_vertex] * (1.0f - t) +
                                         inverse_depth[end_vertex] * t;
                const size_t index = (size_t)y0 * width + (size_t)x0;
                if (edge_depth + 0.0005f >= depth_buffer[index]) {
                    const size_t offset = index * 4u;
                    pixels[offset + 0] = 255;
                    pixels[offset + 1] = 255;
                    pixels[offset + 2] = 255;
                    pixels[offset + 3] = 255;
                }
            }
            if (x0 == x1 && y0 == y1) break;
            const int twice_error = 2 * error;
            if (twice_error >= dy) { error += dy; x0 += sx; }
            if (twice_error <= dx) { error += dx; y0 += sy; }
            ++step;
        }
    }
}

static int submit_rsx_bringup_triangle(void) {
    struct bringup_vertex {
        float position[3];
        float uv[2];
        uint8_t rgba[4];
    } triangle[3] = {
        {{-0.65f, -0.55f, 0.0f}, {0.0f, 0.0f}, {255, 64, 64, 255}},
        {{0.65f, -0.55f, 0.0f}, {1.0f, 0.0f}, {64, 255, 64, 255}},
        {{0.0f, 0.65f, 0.0f}, {0.5f, 1.0f}, {64, 64, 255, 255}},
    };
    uint32_t offset = 0;
    if (horizon_ps3_upload_mesh((const uint8_t *)triangle, sizeof(triangle), &offset) != 0) return -1;
    if (horizon_ps3_prepare_mesh_pipeline() != 0) return -2;
    if (horizon_ps3_bind_mesh(offset, 3) != 0) return -3;
    if (horizon_ps3_draw_mesh(offset, 3) != 0) return -4;
    return 0;
}

int main(void) {
    uint16_t width = 0;
    uint16_t height = 0;
    uint8_t *pixels;
    float *depth_buffer;
    size_t pixel_bytes;
    size_t pixel_count;
    uint8_t palette = 0;
    uint8_t square_was_down = 0;
    float angle = 0.0f;
    int result;

    result = horizon_ps3_boot();
    if (result != 0) return 10 - result;
    if (horizon_ps3_display_size(&width, &height) != 0 || width == 0 || height == 0) {
        horizon_ps3_shutdown();
        return 20;
    }
    if (horizon_ps3_begin_frame() != 0 || submit_rsx_bringup_triangle() != 0) {
        horizon_ps3_shutdown();
        return 24;
    }

    pixel_count = (size_t)width * (size_t)height;
    pixel_bytes = pixel_count * 4u;
    pixels = (uint8_t *)malloc(pixel_bytes);
    depth_buffer = (float *)malloc(pixel_count * sizeof(float));
    if (!pixels || !depth_buffer) {
        free(pixels);
        free(depth_buffer);
        horizon_ps3_shutdown();
        return 21;
    }

    for (;;) {
        horizon_ps3_pad pad;
        if (horizon_ps3_poll_pad(&pad) != 0) {
            free(pixels);
            free(depth_buffer);
            horizon_ps3_shutdown();
            return 22;
        }
        if (pad.start) break;
        if (pad.square && !square_was_down) palette = (uint8_t)((palette + 1u) % 3u);
        square_was_down = pad.square;
        angle += 0.025f;
        if (angle >= 6.2831853f) angle -= 6.2831853f;
        render_scene(pixels, depth_buffer, width, height, palette, angle);
        if (horizon_ps3_begin_frame() != 0 ||
            horizon_ps3_present_rgba8(pixels, width, height) != 0) {
            free(pixels);
            free(depth_buffer);
            horizon_ps3_shutdown();
            return 23;
        }
        usleep(16000);
    }

    memset(pixels, 0, pixel_bytes);
    free(pixels);
    free(depth_buffer);
    horizon_ps3_shutdown();
    return 0;
}
