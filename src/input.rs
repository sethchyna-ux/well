use winit::keyboard::{Key, ModifiersState, NamedKey};

/// Encode named keys independently from platform text/IME delivery.
pub fn encode(
    key: &Key,
    text: Option<&str>,
    mods: ModifiersState,
    application_cursor: bool,
) -> Option<Vec<u8>> {
    let ctrl = mods.control_key();
    let alt = mods.alt_key();
    if mods.super_key() {
        return None;
    }
    let modifier = 1 + mods.shift_key() as u8 + 2 * alt as u8 + 4 * ctrl as u8;
    let csi = |suffix: char| {
        if modifier == 1 {
            format!("\x1b{}{suffix}", if application_cursor { "O" } else { "[" }).into_bytes()
        } else {
            format!("\x1b[1;{modifier}{suffix}").into_bytes()
        }
    };
    let tilde = |n: u8| {
        if modifier == 1 {
            format!("\x1b[{n}~").into_bytes()
        } else {
            format!("\x1b[{n};{modifier}~").into_bytes()
        }
    };
    let mut bytes = match key {
        Key::Named(named) => match named {
            NamedKey::Enter => vec![13],
            NamedKey::Tab if mods.shift_key() => b"\x1b[Z".to_vec(),
            NamedKey::Tab => vec![9],
            NamedKey::Backspace => vec![127],
            NamedKey::Escape => vec![27],
            NamedKey::ArrowUp => return Some(csi('A')),
            NamedKey::ArrowDown => return Some(csi('B')),
            NamedKey::ArrowRight => return Some(csi('C')),
            NamedKey::ArrowLeft => return Some(csi('D')),
            NamedKey::Home => return Some(csi('H')),
            NamedKey::End => return Some(csi('F')),
            NamedKey::Insert => return Some(tilde(2)),
            NamedKey::Delete => return Some(tilde(3)),
            NamedKey::PageUp => return Some(tilde(5)),
            NamedKey::PageDown => return Some(tilde(6)),
            NamedKey::F1 | NamedKey::F2 | NamedKey::F3 | NamedKey::F4 => {
                let suffix = match named {
                    NamedKey::F1 => 'P',
                    NamedKey::F2 => 'Q',
                    NamedKey::F3 => 'R',
                    _ => 'S',
                };
                return Some(if modifier == 1 {
                    format!("\x1bO{suffix}").into_bytes()
                } else {
                    format!("\x1b[1;{modifier}{suffix}").into_bytes()
                });
            }
            NamedKey::F5 => return Some(tilde(15)),
            NamedKey::F6 => return Some(tilde(17)),
            NamedKey::F7 => return Some(tilde(18)),
            NamedKey::F8 => return Some(tilde(19)),
            NamedKey::F9 => return Some(tilde(20)),
            NamedKey::F10 => return Some(tilde(21)),
            NamedKey::F11 => return Some(tilde(23)),
            NamedKey::F12 => return Some(tilde(24)),
            NamedKey::Space if ctrl => vec![0],
            _ => {
                return text
                    .filter(|s| !s.is_empty())
                    .map(|s| s.as_bytes().to_vec())
            }
        },
        Key::Character(value) if ctrl => {
            let ch = value.chars().next()?.to_ascii_uppercase();
            match ch {
                '@'..='_' => vec![(ch as u8) & 31],
                ' ' | '2' => vec![0],
                '?' => vec![127],
                _ => return None,
            }
        }
        Key::Character(value) => text.unwrap_or(value.as_str()).as_bytes().to_vec(),
        _ => return None,
    };
    if alt {
        bytes.insert(0, 27);
    }
    Some(bytes)
}

pub fn paste(text: &str, bracketed: bool) -> Vec<u8> {
    let text = text
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .replace('\x1b', "");
    if bracketed {
        format!("\x1b[200~{text}\x1b[201~").into_bytes()
    } else {
        text.replace('\n', "\r").into_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn controls_and_application_cursor() {
        assert_eq!(
            encode(
                &Key::Character("c".into()),
                Some("c"),
                ModifiersState::CONTROL,
                false
            ),
            Some(vec![3])
        );
        assert_eq!(
            encode(
                &Key::Named(NamedKey::ArrowUp),
                None,
                ModifiersState::empty(),
                true
            ),
            Some(b"\x1bOA".to_vec())
        );
        assert_eq!(
            encode(
                &Key::Named(NamedKey::ArrowUp),
                None,
                ModifiersState::SHIFT,
                true
            ),
            Some(b"\x1b[1;2A".to_vec())
        );
        assert_eq!(
            encode(
                &Key::Character("é".into()),
                Some("é"),
                ModifiersState::empty(),
                false
            ),
            Some("é".as_bytes().to_vec())
        );
    }
    #[test]
    fn paste_preserves_brackets_and_normalizes_newlines() {
        assert_eq!(paste("a\r\nb", true), b"\x1b[200~a\nb\x1b[201~");
        assert_eq!(paste("a\nb", false), b"a\rb");
        assert_eq!(paste("\x1b[201~", true), b"\x1b[200~[201~\x1b[201~");
    }
}
/// Encode xterm mouse reports. Coordinates are zero based at this boundary.
pub fn mouse(
    button: u8,
    pressed: bool,
    row: u16,
    col: u16,
    modifiers: ModifiersState,
    encoding: well_shell::pty::MouseProtocolEncoding,
) -> Option<Vec<u8>> {
    use well_shell::pty::MouseProtocolEncoding;
    let flags = (modifiers.shift_key() as u8 * 4)
        | (modifiers.alt_key() as u8 * 8)
        | (modifiers.control_key() as u8 * 16);
    let code = button | flags;
    match encoding {
        MouseProtocolEncoding::Sgr => Some(
            format!(
                "\x1b[<{code};{};{}{}",
                col as u32 + 1,
                row as u32 + 1,
                if pressed { 'M' } else { 'm' }
            )
            .into_bytes(),
        ),
        MouseProtocolEncoding::Default if row < 223 && col < 223 => Some(vec![
            27,
            b'[',
            b'M',
            if pressed { code } else { 3 | flags } + 32,
            col as u8 + 33,
            row as u8 + 33,
        ]),
        MouseProtocolEncoding::Utf8 if row < 2015 && col < 2015 => {
            let mut report = String::from("\x1b[M");
            for value in [
                if pressed { code } else { 3 | flags } as u32 + 32,
                col as u32 + 33,
                row as u32 + 33,
            ] {
                report.push(char::from_u32(value)?);
            }
            Some(report.into_bytes())
        }
        _ => None,
    }
}
