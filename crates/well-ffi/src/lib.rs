//! well-ffi: C-ABI Dynamic Library Interface for Well Terminal
//!
//! Exposes the Well engine (PTY session, Astraea prompt, Mneme editor, Hermes IPC)
//! to Android, Flutter (via dart:ffi), and native embedders via standard C symbols.

use std::ffi::{c_char, CStr};
use std::slice;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use well_editor::MnemeEditor;
use well_prompt::AstraeaCompiler;
use well_shell::pty::PtySession;

/// Opaque wrapper for a live Well terminal session
pub struct WellSessionHandle {
    pty: PtySession,
    dirty: Arc<AtomicBool>,
}

/// Opaque wrapper for a Mneme composition buffer.
pub struct WellEditorHandle {
    editor: MnemeEditor,
}

/// Creates a new interactive PTY terminal session.
/// Returns a raw pointer to `WellSessionHandle`, or NULL on error.
#[no_mangle]
pub extern "C" fn well_session_create(rows: u16, cols: u16) -> *mut WellSessionHandle {
    let dirty = Arc::new(AtomicBool::new(false));
    let dirty_clone = dirty.clone();

    match PtySession::spawn(rows, cols, move || {
        dirty_clone.store(true, Ordering::Release);
    }) {
        Ok(pty) => {
            let handle = Box::new(WellSessionHandle { pty, dirty });
            Box::into_raw(handle)
        }
        Err(_) => std::ptr::null_mut(),
    }
}

/// Writes user input bytes (keystrokes, pastes) to the active PTY session.
/// Returns number of bytes written, or -1 on error.
///
/// # Safety
///
/// `handle` must be a live pointer returned by [`well_session_create`] and access to it must be
/// synchronized by the caller. `bytes` must point to at least `len` readable bytes.
#[no_mangle]
pub unsafe extern "C" fn well_session_write(
    handle: *mut WellSessionHandle,
    bytes: *const u8,
    len: usize,
) -> i32 {
    if handle.is_null() || bytes.is_null() || len == 0 {
        return -1;
    }

    let session = &mut *handle;
    let slice = slice::from_raw_parts(bytes, len);

    match session.pty.write_all(slice) {
        Ok(()) => len as i32,
        Err(_) => -1,
    }
}

/// Resizes the PTY virtual grid to (rows, cols).
/// Returns 0 on success, -1 on error.
///
/// # Safety
///
/// `handle` must be a live pointer returned by [`well_session_create`] and access to it must be
/// synchronized by the caller.
#[no_mangle]
pub unsafe extern "C" fn well_session_resize(
    handle: *mut WellSessionHandle,
    rows: u16,
    cols: u16,
) -> i32 {
    if handle.is_null() {
        return -1;
    }

    let session = &mut *handle;
    match session.pty.resize(rows, cols) {
        Ok(()) => 0,
        Err(_) => -1,
    }
}

/// Checks if the terminal has new output pending since the last read.
/// Returns 1 if dirty, 0 if clean, -1 if invalid handle.
///
/// # Safety
///
/// `handle` must be a live pointer returned by [`well_session_create`] and must not be accessed
/// concurrently without external synchronization.
#[no_mangle]
pub unsafe extern "C" fn well_session_is_dirty(handle: *mut WellSessionHandle) -> i32 {
    if handle.is_null() {
        return -1;
    }
    let session = &*handle;
    if session.dirty.swap(false, Ordering::AcqRel) {
        1
    } else {
        0
    }
}

/// Copies the current screen text into `out_buf` (UTF-8 encoded).
/// Returns the number of bytes written, or -1 on error.
///
/// # Safety
///
/// `handle` must be a live pointer returned by [`well_session_create`]. `out_buf` must point to at
/// least `max_len` writable bytes and must not overlap memory owned by `handle`.
#[no_mangle]
pub unsafe extern "C" fn well_session_read_screen(
    handle: *mut WellSessionHandle,
    out_buf: *mut u8,
    max_len: usize,
) -> i32 {
    if handle.is_null() || out_buf.is_null() || max_len == 0 {
        return -1;
    }

    let session = &*handle;
    let Ok(parser) = session.pty.parser.lock() else {
        return -1;
    };

    let screen = parser.screen();
    let (rows, cols) = screen.size();
    let mut text = String::with_capacity((rows as usize) * (cols as usize + 1));

    for row in 0..rows {
        for col in 0..cols {
            if let Some(cell) = screen.cell(row, col) {
                for ch in cell.contents().chars() {
                    text.push(ch);
                }
            } else {
                text.push(' ');
            }
        }
        text.push('\n');
    }

    let bytes = text.as_bytes();
    let to_copy = bytes.len().min(max_len);
    std::ptr::copy_nonoverlapping(bytes.as_ptr(), out_buf, to_copy);
    to_copy as i32
}

/// Destroys a session handle, cleaning up child processes and PTY file descriptors.
///
/// # Safety
///
/// `handle` must be null or a live pointer returned by [`well_session_create`]. A non-null handle
/// must be passed to this function exactly once and must not be used afterward.
#[no_mangle]
pub unsafe extern "C" fn well_session_destroy(handle: *mut WellSessionHandle) {
    if !handle.is_null() {
        let _ = Box::from_raw(handle);
    }
}

/// Renders the Astraea cyber prompt for a given CWD path into `out_buf`.
/// Returns byte length written, or -1 on error.
///
/// # Safety
///
/// `cwd` must be null or point to a valid NUL-terminated C string. `out_buf` must point to at least
/// `max_len` writable bytes and must not overlap `cwd`.
#[no_mangle]
pub unsafe extern "C" fn well_prompt_render(
    cwd: *const c_char,
    out_buf: *mut u8,
    max_len: usize,
) -> i32 {
    if out_buf.is_null() || max_len == 0 {
        return -1;
    }

    let cwd_str = if cwd.is_null() {
        "~"
    } else {
        CStr::from_ptr(cwd).to_str().unwrap_or("~")
    };

    let snap = well_prompt::AstraeaWatcher::scan_path(std::path::Path::new(cwd_str), 0, 0);
    let compiler = AstraeaCompiler::with_snapshot(snap);
    let prompt = compiler.compile_prompt();
    let bytes = prompt.as_bytes();
    let to_copy = bytes.len().min(max_len);
    std::ptr::copy_nonoverlapping(bytes.as_ptr(), out_buf, to_copy);
    to_copy as i32
}

/// Creates a standalone Mneme editor buffer.
#[no_mangle]
pub extern "C" fn well_editor_create() -> *mut WellEditorHandle {
    Box::into_raw(Box::new(WellEditorHandle {
        editor: MnemeEditor::default(),
    }))
}

/// Inserts UTF-8 text at the active Mneme cursor.
///
/// # Safety
///
/// `handle` must be a live pointer returned by [`well_editor_create`]. `text` must be null or a
/// valid NUL-terminated UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn well_editor_insert(
    handle: *mut WellEditorHandle,
    text: *const c_char,
) -> i32 {
    if handle.is_null() || text.is_null() {
        return -1;
    }

    let Ok(text) = CStr::from_ptr(text).to_str() else {
        return -1;
    };
    let editor = &mut *handle;
    editor.editor.insert_text(text);
    editor.editor.len_chars() as i32
}

/// Copies the Mneme buffer contents into `out_buf`.
///
/// Returns byte length written, or -1 on error.
///
/// # Safety
///
/// `handle` must be a live pointer returned by [`well_editor_create`]. `out_buf` must point to at
/// least `max_len` writable bytes.
#[no_mangle]
pub unsafe extern "C" fn well_editor_text(
    handle: *mut WellEditorHandle,
    out_buf: *mut u8,
    max_len: usize,
) -> i32 {
    if handle.is_null() || out_buf.is_null() || max_len == 0 {
        return -1;
    }

    let editor = &*handle;
    let text = editor.editor.get_text();
    let bytes = text.as_bytes();
    let to_copy = bytes.len().min(max_len);
    std::ptr::copy_nonoverlapping(bytes.as_ptr(), out_buf, to_copy);
    to_copy as i32
}

/// Destroys a Mneme editor handle.
///
/// # Safety
///
/// `handle` must be null or a live pointer returned by [`well_editor_create`]. A non-null handle
/// must be passed to this function exactly once and must not be used afterward.
#[no_mangle]
pub unsafe extern "C" fn well_editor_destroy(handle: *mut WellEditorHandle) {
    if !handle.is_null() {
        let _ = Box::from_raw(handle);
    }
}

// ============================================================================
// Zig libghostty C-ABI Bindings & Safe Rust Wrappers (Phase P1.3)
// ============================================================================

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct GhosttyConfig {
    pub cols: u16,
    pub rows: u16,
    pub cursor_blink: bool,
}

#[repr(C)]
#[derive(Debug)]
pub struct GhosttyInstance {
    pub cols: u16,
    pub rows: u16,
    pub cursor_blink: bool,
    pub bytes_fed: usize,
    pub escape_sequences_seen: usize,
}

extern "C" {
    pub fn ghostty_accel_version() -> u32;
    pub fn ghostty_init(config: *const GhosttyConfig) -> *mut GhosttyInstance;
    pub fn ghostty_feed(instance: *mut GhosttyInstance, bytes: *const u8, len: usize);
    pub fn ghostty_resize(instance: *mut GhosttyInstance, cols: u16, rows: u16);
    pub fn ghostty_free(instance: *mut GhosttyInstance);
    pub fn ghostty_sanitize_utf8(
        bytes: *const u8,
        len: usize,
        out: *mut u8,
        out_cap: usize,
    ) -> usize;
    pub fn ghostty_detect_hyperlink(
        bytes: *const u8,
        len: usize,
        out_start: *mut usize,
        out_len: usize_ptr_alias,
    ) -> bool;
    pub fn ghostty_fast_metric_calc(
        font_size: f32,
        line_height: f32,
        out_width: *mut f32,
        out_height: *mut f32,
    );
}

#[allow(non_camel_case_types)]
type usize_ptr_alias = *mut usize;

/// Sanitizes arbitrary terminal byte slices into valid UTF-8 using Zig `libghostty`.
pub fn sanitize_utf8_via_ghostty(input: &[u8]) -> String {
    if input.is_empty() {
        return String::new();
    }
    let mut out = vec![0u8; input.len() * 3];
    let written =
        unsafe { ghostty_sanitize_utf8(input.as_ptr(), input.len(), out.as_mut_ptr(), out.len()) };
    out.truncate(written);
    String::from_utf8_lossy(&out).into_owned()
}

/// Detects the first hyperlink boundary (`https://`, `http://`, `file://`) in a terminal line.
pub fn detect_hyperlink_via_ghostty(line: &str) -> Option<(usize, usize)> {
    let bytes = line.as_bytes();
    let mut start = 0usize;
    let mut len = 0usize;
    let found =
        unsafe { ghostty_detect_hyperlink(bytes.as_ptr(), bytes.len(), &mut start, &mut len) };
    if found {
        Some((start, len))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;

    #[test]
    fn mneme_ffi_round_trip_inserts_and_reads_text() {
        let handle = well_editor_create();
        assert!(!handle.is_null());

        let text = CString::new("echo well").expect("CString should build");
        let len = unsafe { well_editor_insert(handle, text.as_ptr()) };
        assert_eq!(len, 9);

        let mut out = [0_u8; 32];
        let copied = unsafe { well_editor_text(handle, out.as_mut_ptr(), out.len()) };
        assert_eq!(copied, 9);
        assert_eq!(&out[..copied as usize], b"echo well");

        unsafe { well_editor_destroy(handle) };
    }

    #[test]
    fn libghostty_ffi_verifies_real_behavior_and_zero_mock_pointers() {
        unsafe {
            assert_eq!(ghostty_accel_version(), 0x0100_0002);

            let cfg = GhosttyConfig {
                cols: 100,
                rows: 30,
                cursor_blink: true,
            };
            let inst = ghostty_init(&cfg);
            assert!(!inst.is_null());
            assert_ne!(inst as usize, 0x1000, "Must not return mock pointer 0x1000");
            assert_eq!((*inst).cols, 100);
            assert_eq!((*inst).rows, 30);

            let stream = b"Hello \x1b[31mRed\x1b[0m";
            ghostty_feed(inst, stream.as_ptr(), stream.len());
            assert_eq!((*inst).bytes_fed, stream.len());
            assert_eq!((*inst).escape_sequences_seen, 2);

            ghostty_resize(inst, 132, 43);
            assert_eq!((*inst).cols, 132);
            assert_eq!((*inst).rows, 43);

            ghostty_free(inst);
        }

        let sanitized = sanitize_utf8_via_ghostty(b"valid\xfftext");
        assert_eq!(sanitized, "valid\u{FFFD}text");

        let line = "Check https://well.dev/spec for details";
        let (start, len) = detect_hyperlink_via_ghostty(line).expect("detect hyperlink");
        assert_eq!(&line[start..start + len], "https://well.dev/spec");
    }
}
