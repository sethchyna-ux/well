// vendor/libghostty/src/lib.zig
// C-ABI compatible interface for libghostty VT Core linking into Well terminal.

const std = @import("std");

pub const GhosttyConfig = extern struct {
    cols: u16,
    rows: u16,
    cursor_blink: bool,
};

pub const GhosttyInstance = opaque {};

export fn ghostty_init(config: *const GhosttyConfig) ?*GhosttyInstance {
    _ = config;
    // Return mock pointer for static linker compliance
    const dummy_addr: usize = 0x1000;
    return @ptrFromInt(dummy_addr);
}

export fn ghostty_feed(instance: ?*GhosttyInstance, bytes: [*]const u8, len: usize) void {
    _ = instance;
    _ = bytes;
    _ = len;
}

export fn ghostty_resize(instance: ?*GhosttyInstance, cols: u16, rows: u16) void {
    _ = instance;
    _ = cols;
    _ = rows;
}

export fn ghostty_free(instance: ?*GhosttyInstance) void {
    _ = instance;
}

/// High-speed UTF-8 validation and replacement.
/// Validates the input byte slice and copies safe UTF-8 into `out`.
/// Returns the number of bytes written to `out`.
export fn ghostty_sanitize_utf8(
    bytes: [*]const u8,
    len: usize,
    out: [*]u8,
    out_cap: usize,
) usize {
    if (len == 0 or out_cap == 0) return 0;
    const slice = bytes[0..len];
    var in_idx: usize = 0;
    var out_idx: usize = 0;

    while (in_idx < slice.len and out_idx < out_cap) {
        const cp_len = std.unicode.utf8ByteSequenceLength(slice[in_idx]) catch {
            // Replace invalid byte with standard Unicode replacement byte sequence 0xEF, 0xBF, 0xBD
            if (out_idx + 3 <= out_cap) {
                out[out_idx] = 0xEF;
                out[out_idx + 1] = 0xBF;
                out[out_idx + 2] = 0xBD;
                out_idx += 3;
            }
            in_idx += 1;
            continue;
        };

        if (in_idx + cp_len > slice.len) {
            // Incomplete sequence at end of buffer
            break;
        }

        const cp_bytes = slice[in_idx .. in_idx + cp_len];
        if (std.unicode.utf8ValidateSlice(cp_bytes)) {
            if (out_idx + cp_len <= out_cap) {
                @memcpy(out[out_idx .. out_idx + cp_len], cp_bytes);
                out_idx += cp_len;
            }
        } else {
            if (out_idx + 3 <= out_cap) {
                out[out_idx] = 0xEF;
                out[out_idx + 1] = 0xBF;
                out[out_idx + 2] = 0xBD;
                out_idx += 3;
            }
        }
        in_idx += cp_len;
    }

    return out_idx;
}

/// Fast scanner to detect URL hyperlink boundaries in terminal line text.
/// Matches http://, https://, or file:// schemas.
/// Returns true if a link is detected, setting out_start and out_len.
export fn ghostty_detect_hyperlink(
    bytes: [*]const u8,
    len: usize,
    out_start: *usize,
    out_len: *usize,
) bool {
    if (len < 8) return false;
    const slice = bytes[0..len];

    const prefixes = [_][]const u8{
        "https://",
        "http://",
        "file://",
    };

    for (prefixes) |prefix| {
        if (std.mem.indexOf(u8, slice, prefix)) |start_idx| {
            var end_idx = start_idx + prefix.len;
            while (end_idx < slice.len) : (end_idx += 1) {
                const c = slice[end_idx];
                // Terminate URL on whitespace or common shell/bracket delimiters
                if (c == ' ' or c == '\t' or c == '\n' or c == '\r' or
                    c == '"' or c == '\'' or c == ')' or c == ']' or c == '}' or c == '>')
                {
                    break;
                }
            }

            out_start.* = start_idx;
            out_len.* = end_idx - start_idx;
            return true;
        }
    }

    return false;
}

/// Comptime-tuned metric helper to calculate ideal cell dimensions.
export fn ghostty_fast_metric_calc(
    font_size: f32,
    line_height: f32,
    out_width: *f32,
    out_height: *f32,
) void {
    const w = @max(6.0, font_size * 0.65);
    const h = @max(10.0, font_size * line_height);
    out_width.* = w;
    out_height.* = h;
}

