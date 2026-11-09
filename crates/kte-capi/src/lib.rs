//! C ABI for embedding Khmer Terminal Engine in native terminals (C, C++,
//! Swift, Zig) and in WebAssembly hosts.
//!
//! The matching header is `include/kte.h`.
//!
//! Conventions:
//! - Text is UTF-8, passed as pointer + byte length; it need not be
//!   NUL-terminated. Functions taking text return `KTE_INVALID` (`SIZE_MAX`)
//!   for invalid UTF-8.
//! - Functions that return text copy at most `buf_len` bytes into `buf` (no
//!   NUL terminator) and return the full length. Pass `buf = NULL,
//!   buf_len = 0` to query the length first.
//! - A Rust panic inside a call aborts the process; it never unwinds into C.
//!
//! This is the only crate allowed to use `unsafe`; every `unsafe` block
//! carries a `SAFETY` comment.

use std::ffi::c_char;
use std::ptr;

use kte_core::{CellModel, Terminal, unicode};

/// Returned for invalid input (e.g. invalid UTF-8).
pub const KTE_INVALID: usize = usize::MAX;

/// Opaque terminal handle.
pub struct KteTerminal(Terminal);

fn model(m: u32) -> CellModel {
    if m == 1 {
        CellModel::Compact
    } else {
        CellModel::Wide
    }
}

/// Reads a UTF-8 string from C.
///
/// # Safety
/// `ptr` must be null (with `len == 0`) or valid for reads of `len` bytes.
unsafe fn read_str<'a>(ptr: *const u8, len: usize) -> Option<&'a str> {
    if len == 0 {
        return Some("");
    }
    if ptr.is_null() {
        return None;
    }
    // SAFETY: the caller guarantees `ptr` is valid for `len` bytes.
    let bytes = unsafe { std::slice::from_raw_parts(ptr, len) };
    std::str::from_utf8(bytes).ok()
}

/// Copies `text` into a C buffer and returns its full length.
///
/// # Safety
/// `buf` must be null or valid for writes of `buf_len` bytes.
unsafe fn write_out(text: &[u8], buf: *mut u8, buf_len: usize) -> usize {
    if !buf.is_null() {
        let n = text.len().min(buf_len);
        // SAFETY: `buf` is valid for `buf_len >= n` bytes, and `text` cannot
        // overlap memory owned by the caller.
        unsafe { ptr::copy_nonoverlapping(text.as_ptr(), buf, n) };
    }
    text.len()
}

/// Version of the KTE Width Spec implemented, as a static NUL-terminated string.
#[unsafe(no_mangle)]
pub extern "C" fn kte_spec_version() -> *const c_char {
    c"0.1.0".as_ptr()
}

/// Width in cells of a UTF-8 string. `model`: 0 = Wide, 1 = Compact.
///
/// # Safety
/// `text` must be valid for reads of `len` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kte_str_width(text: *const u8, len: usize, model_id: u32) -> usize {
    // SAFETY: forwarded caller guarantee.
    match unsafe { read_str(text, len) } {
        Some(s) => unicode::str_width(s, model(model_id)),
        None => KTE_INVALID,
    }
}

/// Half-units of the first cluster in `text`, or -1 if it is not a Khmer
/// cluster, is empty, or is invalid UTF-8.
///
/// # Safety
/// `text` must be valid for reads of `len` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kte_half_units(text: *const u8, len: usize) -> i32 {
    // SAFETY: forwarded caller guarantee.
    let Some(s) = (unsafe { read_str(text, len) }) else {
        return -1;
    };
    unicode::clusters(s)
        .next()
        .and_then(|(_, c)| unicode::half_units(c))
        .map_or(-1, |h| h as i32)
}

/// Byte offset of the next cluster boundary after `offset` (cursor right).
///
/// # Safety
/// `text` must be valid for reads of `len` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kte_next_boundary(text: *const u8, len: usize, offset: usize) -> usize {
    // SAFETY: forwarded caller guarantee.
    match unsafe { read_str(text, len) } {
        Some(s) => unicode::next_boundary(s, offset),
        None => KTE_INVALID,
    }
}

/// Byte offset of the cluster boundary before `offset` (cursor left).
///
/// # Safety
/// `text` must be valid for reads of `len` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kte_prev_boundary(text: *const u8, len: usize, offset: usize) -> usize {
    // SAFETY: forwarded caller guarantee.
    match unsafe { read_str(text, len) } {
        Some(s) => unicode::prev_boundary(s, offset),
        None => KTE_INVALID,
    }
}

/// Creates a terminal of `rows` × `cols` cells. Free it with
/// `kte_terminal_free`.
#[unsafe(no_mangle)]
pub extern "C" fn kte_terminal_new(rows: u32, cols: u32, model_id: u32) -> *mut KteTerminal {
    Box::into_raw(Box::new(KteTerminal(Terminal::new(
        rows as usize,
        cols as usize,
        model(model_id),
    ))))
}

/// Frees a terminal. `term` may be null.
///
/// # Safety
/// `term` must be null or a pointer from `kte_terminal_new` not yet freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kte_terminal_free(term: *mut KteTerminal) {
    if !term.is_null() {
        // SAFETY: the pointer came from `Box::into_raw` and is freed once.
        drop(unsafe { Box::from_raw(term) });
    }
}

/// Borrows a terminal handle.
///
/// # Safety
/// `term` must be a live pointer from `kte_terminal_new`.
unsafe fn term_ref<'a>(term: *const KteTerminal) -> &'a Terminal {
    // SAFETY: caller guarantee; the handle is never null when valid.
    unsafe { &(*term).0 }
}

/// Feeds application output (any bytes; invalid UTF-8 is replaced).
///
/// # Safety
/// `term` must be live; `bytes` valid for reads of `len` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kte_terminal_feed(term: *mut KteTerminal, bytes: *const u8, len: usize) {
    if term.is_null() || (bytes.is_null() && len > 0) {
        return;
    }
    let data = if len == 0 {
        &[][..]
    } else {
        // SAFETY: caller guarantee.
        unsafe { std::slice::from_raw_parts(bytes, len) }
    };
    // SAFETY: caller guarantee; exclusive access during the call.
    unsafe { (*term).0.feed(data) };
}

/// Resizes the terminal.
///
/// # Safety
/// `term` must be live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kte_terminal_resize(term: *mut KteTerminal, rows: u32, cols: u32) {
    if !term.is_null() {
        // SAFETY: caller guarantee.
        unsafe { (*term).0.resize(rows as usize, cols as usize) };
    }
}

/// Writes the cursor position (zero-based) to `row` and `col` (either may be null).
///
/// # Safety
/// `term` must be live; `row` and `col` null or valid for writes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kte_terminal_cursor(
    term: *const KteTerminal,
    row: *mut u32,
    col: *mut u32,
) {
    if term.is_null() {
        return;
    }
    // SAFETY: caller guarantee.
    let (r, c) = unsafe { term_ref(term) }.cursor();
    // SAFETY: each pointer is null or valid for writes (caller guarantee).
    unsafe {
        if !row.is_null() {
            *row = r as u32;
        }
        if !col.is_null() {
            *col = c as u32;
        }
    }
}

/// Copies the text of the cell at (`row`, `col`) and writes its width to
/// `width` (0 = continuation of a wider cluster; may be null). Returns the
/// text length, or `KTE_INVALID` if out of range.
///
/// # Safety
/// `term` must be live; `buf` null or valid for `buf_len` bytes; `width`
/// null or valid for writes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kte_terminal_cell(
    term: *const KteTerminal,
    row: u32,
    col: u32,
    buf: *mut u8,
    buf_len: usize,
    width: *mut u8,
) -> usize {
    if term.is_null() {
        return KTE_INVALID;
    }
    // SAFETY: caller guarantee.
    let t = unsafe { term_ref(term) };
    let (row, col) = (row as usize, col as usize);
    if row >= t.rows() || col >= t.cols() {
        return KTE_INVALID;
    }
    let cell = t.cell(row, col);
    if !width.is_null() {
        // SAFETY: caller guarantee.
        unsafe { *width = cell.width };
    }
    // SAFETY: caller guarantee for `buf`.
    unsafe { write_out(cell.text.as_bytes(), buf, buf_len) }
}

/// Copies the text of screen row `row` (trailing blanks removed). Returns its
/// length, or `KTE_INVALID` if out of range.
///
/// # Safety
/// `term` must be live; `buf` null or valid for `buf_len` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kte_terminal_row_text(
    term: *const KteTerminal,
    row: u32,
    buf: *mut u8,
    buf_len: usize,
) -> usize {
    if term.is_null() {
        return KTE_INVALID;
    }
    // SAFETY: caller guarantee.
    let t = unsafe { term_ref(term) };
    if row as usize >= t.rows() {
        return KTE_INVALID;
    }
    // SAFETY: caller guarantee for `buf`.
    unsafe { write_out(t.row_text(row as usize).as_bytes(), buf, buf_len) }
}

/// Copies the text selected between two cells (inclusive, snapped to whole
/// clusters, original codepoints preserved). Returns its length.
///
/// # Safety
/// `term` must be live; `buf` null or valid for `buf_len` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kte_terminal_selection_text(
    term: *const KteTerminal,
    from_row: u32,
    from_col: u32,
    to_row: u32,
    to_col: u32,
    buf: *mut u8,
    buf_len: usize,
) -> usize {
    if term.is_null() {
        return KTE_INVALID;
    }
    // SAFETY: caller guarantee.
    let t = unsafe { term_ref(term) };
    let clamp = |r: u32, c: u32| {
        (
            (r as usize).min(t.rows() - 1),
            (c as usize).min(t.cols() - 1),
        )
    };
    let text = t.selection_text(clamp(from_row, from_col), clamp(to_row, to_col));
    // SAFETY: caller guarantee for `buf`.
    unsafe { write_out(text.as_bytes(), buf, buf_len) }
}

/// Moves pending replies for the application (cursor reports, mode reports)
/// into `buf` and returns how many bytes were written. Bytes that do not fit
/// stay queued for the next call. Write them to the PTY.
///
/// # Safety
/// `term` must be live; `buf` valid for `buf_len` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kte_terminal_take_responses(
    term: *mut KteTerminal,
    buf: *mut u8,
    buf_len: usize,
) -> usize {
    if term.is_null() || buf.is_null() {
        return 0;
    }
    // SAFETY: caller guarantee; exclusive access during the call.
    let pending = unsafe { (*term).0.take_responses_max(buf_len) };
    // SAFETY: `buf` is valid for `buf_len >= pending.len()` bytes.
    unsafe { ptr::copy_nonoverlapping(pending.as_ptr(), buf, pending.len()) };
    pending.len()
}

/// Allocates `len` bytes inside the library (for WebAssembly hosts that must
/// write input into the module's memory). Free with `kte_free`.
#[unsafe(no_mangle)]
pub extern "C" fn kte_alloc(len: usize) -> *mut u8 {
    let mut buf = Vec::<u8>::with_capacity(len.max(1));
    let ptr = buf.as_mut_ptr();
    std::mem::forget(buf);
    ptr
}

/// Frees memory from `kte_alloc`.
///
/// # Safety
/// `ptr` must come from `kte_alloc(len)` with the same `len`, freed once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kte_free(ptr: *mut u8, len: usize) {
    if !ptr.is_null() {
        // SAFETY: rebuilds the Vec allocated in `kte_alloc` with its capacity.
        drop(unsafe { Vec::from_raw_parts(ptr, 0, len.max(1)) });
    }
}
