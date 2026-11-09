//! Exercises the C ABI from Rust, the way a C caller would.

use std::ptr;

use kte_capi::*;

fn bytes(s: &str) -> (*const u8, usize) {
    (s.as_ptr(), s.len())
}

#[test]
fn measurement() {
    // SAFETY: every pointer comes from a live Rust buffer or a KTE handle
    // that is still allocated, with matching lengths.
    unsafe {
        let (p, n) = bytes("ភាសាខ្មែរ");
        assert_eq!(kte_str_width(p, n, 0), 11);
        assert_eq!(kte_str_width(p, n, 1), 7); // 2 + 2 + 2 + 1
        assert_eq!(kte_half_units(p, n), 3); // ភា
        let (p, n) = bytes("ok");
        assert_eq!(kte_half_units(p, n), -1);
        assert_eq!(kte_str_width(ptr::null(), 0, 0), 0);
        let bad = [0xff_u8, 0xfe];
        assert_eq!(kte_str_width(bad.as_ptr(), 2, 0), KTE_INVALID);
    }
}

#[test]
fn boundaries() {
    // SAFETY: every pointer comes from a live Rust buffer or a KTE handle
    // that is still allocated, with matching lengths.
    unsafe {
        let s = "សួស្តី";
        let (p, n) = bytes(s);
        assert_eq!(kte_next_boundary(p, n, 0), 6);
        assert_eq!(kte_prev_boundary(p, n, n), 6);
    }
}

#[test]
fn spec_version_is_a_c_string() {
    // SAFETY: kte_spec_version returns a static NUL-terminated string.
    let v = unsafe { std::ffi::CStr::from_ptr(kte_spec_version()) };
    assert_eq!(v.to_str().unwrap(), kte_core::SPEC_VERSION);
}

#[test]
fn terminal_round_trip() {
    // SAFETY: every pointer comes from a live Rust buffer or a KTE handle
    // that is still allocated, with matching lengths.
    unsafe {
        let t = kte_terminal_new(3, 20, 0);
        let text = "ស្ត្រី ok";
        let (p, n) = bytes(text);
        kte_terminal_feed(t, p, n);

        let (mut row, mut col) = (9, 9);
        kte_terminal_cursor(t, &mut row, &mut col);
        assert_eq!((row, col), (0, 7));

        // Length query, then copy.
        let len = kte_terminal_row_text(t, 0, ptr::null_mut(), 0);
        assert_eq!(len, text.len());
        let mut buf = vec![0u8; len];
        kte_terminal_row_text(t, 0, buf.as_mut_ptr(), len);
        assert_eq!(std::str::from_utf8(&buf).unwrap(), text);

        // Truncated copy still reports the full length.
        let mut small = [0u8; 3];
        assert_eq!(kte_terminal_row_text(t, 0, small.as_mut_ptr(), 3), len);

        let mut width = 9;
        let mut cell = [0u8; 64];
        let n = kte_terminal_cell(t, 0, 0, cell.as_mut_ptr(), 64, &mut width);
        assert_eq!((&cell[..n], width), ("ស្ត្រី".as_bytes(), 4));
        kte_terminal_cell(t, 0, 1, ptr::null_mut(), 0, &mut width);
        assert_eq!(width, 0);
        assert_eq!(
            kte_terminal_cell(t, 5, 0, ptr::null_mut(), 0, ptr::null_mut()),
            KTE_INVALID
        );

        let n = kte_terminal_selection_text(t, 0, 2, 0, 5, cell.as_mut_ptr(), 64);
        assert_eq!(&cell[..n], "ស្ត្រី o".as_bytes());

        kte_terminal_resize(t, 2, 10);
        kte_terminal_free(t);
        kte_terminal_free(ptr::null_mut());
    }
}

#[test]
fn responses_are_drained_in_pieces() {
    // SAFETY: every pointer comes from a live Rust buffer or a KTE handle
    // that is still allocated, with matching lengths.
    unsafe {
        let t = kte_terminal_new(2, 10, 0);
        let (p, n) = bytes("\x1b[6n");
        kte_terminal_feed(t, p, n);
        let mut buf = [0u8; 4];
        assert_eq!(kte_terminal_take_responses(t, buf.as_mut_ptr(), 4), 4);
        assert_eq!(&buf, b"\x1b[1;");
        assert_eq!(kte_terminal_take_responses(t, buf.as_mut_ptr(), 4), 2);
        assert_eq!(&buf[..2], b"1R");
        assert_eq!(kte_terminal_take_responses(t, buf.as_mut_ptr(), 4), 0);
        kte_terminal_free(t);
    }
}

#[test]
fn alloc_and_free() {
    // SAFETY: every pointer comes from a live Rust buffer or a KTE handle
    // that is still allocated, with matching lengths.
    unsafe {
        let p = kte_alloc(16);
        assert!(!p.is_null());
        ptr::write_bytes(p, 7, 16);
        kte_free(p, 16);
    }
}
