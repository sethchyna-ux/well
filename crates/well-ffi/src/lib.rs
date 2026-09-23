//! well-ffi: C-ABI Dynamic Library Interface for Well Terminal
//!
//! Exposes the Well engine (PTY session, Astraea prompt, Mneme editor, Hermes IPC)
//! to Android, Flutter (via dart:ffi), and native embedders via standard C symbols.

use std::ffi::{c_char, CStr};
use std::slice;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use well_editor::MnemeEditor;
use well_prompt::{AstraeaStateVector, PromptCompiler};
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

    let state = Arc::new(AstraeaStateVector::new());
    let _ = state.refresh_from_cwd(cwd_str);
    let compiler = PromptCompiler::new(state);
    let prompt = compiler.compile_prompt(cwd_str);
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
}
