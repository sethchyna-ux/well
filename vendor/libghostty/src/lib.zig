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
