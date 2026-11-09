//! Keyboard input → bytes for the shell (xterm conventions).

use winit::event::KeyEvent;
use winit::keyboard::{Key, ModifiersState, NamedKey};

/// xterm modifier parameter: 1 + shift + 2·alt + 4·ctrl.
fn modifier_param(m: ModifiersState) -> u8 {
    1 + u8::from(m.shift_key()) + 2 * u8::from(m.alt_key()) + 4 * u8::from(m.control_key())
}

/// `CSI <final>` or, with modifiers, `CSI 1 ; <mod> <final>`; in application
/// cursor mode, unmodified keys use `SS3 <final>`.
fn cursor_key(final_byte: char, m: ModifiersState, app_cursor: bool) -> Vec<u8> {
    let p = modifier_param(m);
    if p > 1 {
        format!("\x1b[1;{p}{final_byte}").into_bytes()
    } else if app_cursor {
        format!("\x1bO{final_byte}").into_bytes()
    } else {
        format!("\x1b[{final_byte}").into_bytes()
    }
}

/// `CSI <n> ~` or `CSI <n> ; <mod> ~`.
fn tilde_key(n: u8, m: ModifiersState) -> Vec<u8> {
    let p = modifier_param(m);
    if p > 1 {
        format!("\x1b[{n};{p}~").into_bytes()
    } else {
        format!("\x1b[{n}~").into_bytes()
    }
}

/// Control character for Ctrl + `c`, if there is one.
fn control_byte(c: char) -> Option<u8> {
    match c.to_ascii_lowercase() {
        c @ 'a'..='z' => Some(c as u8 - b'a' + 1),
        '@' | ' ' | '2' => Some(0),
        '[' | '3' => Some(0x1b),
        '\\' | '4' => Some(0x1c),
        ']' | '5' => Some(0x1d),
        '^' | '6' => Some(0x1e),
        '_' | '-' | '/' | '7' => Some(0x1f),
        '8' => Some(0x7f),
        _ => None,
    }
}

/// Bytes to send for a key press, or `None` if the key sends nothing (or is
/// handled by the app, like Cmd shortcuts).
pub fn encode(event: &KeyEvent, m: ModifiersState, app_cursor: bool) -> Option<Vec<u8>> {
    if !event.state.is_pressed() || m.super_key() {
        return None;
    }
    let bytes = match &event.logical_key {
        Key::Named(named) => match named {
            NamedKey::Enter => b"\r".to_vec(),
            NamedKey::Backspace if m.control_key() => vec![0x08],
            NamedKey::Backspace if m.alt_key() => b"\x1b\x7f".to_vec(),
            NamedKey::Backspace => vec![0x7f],
            NamedKey::Tab if m.shift_key() => b"\x1b[Z".to_vec(),
            NamedKey::Tab => b"\t".to_vec(),
            NamedKey::Escape => b"\x1b".to_vec(),
            NamedKey::Space if m.control_key() => vec![0],
            NamedKey::Space => b" ".to_vec(),
            NamedKey::ArrowUp => cursor_key('A', m, app_cursor),
            NamedKey::ArrowDown => cursor_key('B', m, app_cursor),
            NamedKey::ArrowRight => cursor_key('C', m, app_cursor),
            NamedKey::ArrowLeft => cursor_key('D', m, app_cursor),
            NamedKey::Home => cursor_key('H', m, app_cursor),
            NamedKey::End => cursor_key('F', m, app_cursor),
            NamedKey::Insert => tilde_key(2, m),
            NamedKey::Delete => tilde_key(3, m),
            NamedKey::PageUp => tilde_key(5, m),
            NamedKey::PageDown => tilde_key(6, m),
            NamedKey::F1 => cursor_key('P', m, true),
            NamedKey::F2 => cursor_key('Q', m, true),
            NamedKey::F3 => cursor_key('R', m, true),
            NamedKey::F4 => cursor_key('S', m, true),
            NamedKey::F5 => tilde_key(15, m),
            NamedKey::F6 => tilde_key(17, m),
            NamedKey::F7 => tilde_key(18, m),
            NamedKey::F8 => tilde_key(19, m),
            NamedKey::F9 => tilde_key(20, m),
            NamedKey::F10 => tilde_key(21, m),
            NamedKey::F11 => tilde_key(23, m),
            NamedKey::F12 => tilde_key(24, m),
            _ => return None,
        },
        Key::Character(s) if m.control_key() => {
            let c = s.chars().next()?;
            let b = control_byte(c)?;
            if m.alt_key() { vec![0x1b, b] } else { vec![b] }
        }
        _ => {
            let text = event.text.as_ref()?;
            // On macOS, Option composes characters (e.g. Khmer layouts use
            // it), so send its text unchanged. Elsewhere Alt means "ESC prefix".
            if m.alt_key() && !cfg!(target_os = "macos") {
                let mut v = vec![0x1b];
                v.extend_from_slice(text.as_bytes());
                v
            } else {
                text.as_bytes().to_vec()
            }
        }
    };
    Some(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_bytes() {
        assert_eq!(control_byte('c'), Some(3));
        assert_eq!(control_byte('C'), Some(3));
        assert_eq!(control_byte('['), Some(0x1b));
        assert_eq!(control_byte('ក'), None);
    }

    #[test]
    fn cursor_keys() {
        let none = ModifiersState::empty();
        assert_eq!(cursor_key('A', none, false), b"\x1b[A");
        assert_eq!(cursor_key('A', none, true), b"\x1bOA");
        assert_eq!(
            cursor_key('C', ModifiersState::CONTROL, false),
            b"\x1b[1;5C"
        );
        assert_eq!(tilde_key(3, ModifiersState::SHIFT), b"\x1b[3;2~");
    }
}
