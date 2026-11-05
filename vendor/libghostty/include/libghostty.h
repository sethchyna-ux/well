/* vendor/libghostty/include/libghostty.h
 * C-ABI header for Well Terminal's Zig SIMD & VT Accelerator (libghostty).
 */
#ifndef WELL_LIBGHOSTTY_H
#define WELL_LIBGHOSTTY_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct GhosttyConfig {
    uint16_t cols;
    uint16_t rows;
    bool cursor_blink;
} GhosttyConfig;

typedef struct GhosttyInstance {
    uint16_t cols;
    uint16_t rows;
    bool cursor_blink;
    size_t bytes_fed;
    size_t escape_sequences_seen;
} GhosttyInstance;

uint32_t ghostty_accel_version(void);
GhosttyInstance *ghostty_init(const GhosttyConfig *config);
void ghostty_feed(GhosttyInstance *instance, const uint8_t *bytes, size_t len);
void ghostty_resize(GhosttyInstance *instance, uint16_t cols, uint16_t rows);
void ghostty_free(GhosttyInstance *instance);

size_t ghostty_simd_scan_escapes(
    const uint8_t *input,
    size_t input_len,
    size_t *matches,
    size_t matches_cap
);

size_t ghostty_parse_ansi_simd(
    const uint8_t *input,
    size_t input_len,
    uint8_t *output,
    size_t output_cap
);

size_t ghostty_sanitize_utf8(
    const uint8_t *bytes,
    size_t len,
    uint8_t *out,
    size_t out_cap
);

bool ghostty_detect_hyperlink(
    const uint8_t *bytes,
    size_t len,
    size_t *out_start,
    size_t *out_len
);

void ghostty_fast_metric_calc(
    float font_size,
    float line_height,
    float *out_width,
    float *out_height
);

#ifdef __cplusplus
}
#endif

#endif /* WELL_LIBGHOSTTY_H */

