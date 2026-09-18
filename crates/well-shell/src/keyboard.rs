use well_ipc::KittyKeyboardInput;

pub struct KeyEncoder;

impl KeyEncoder {
    /// Encodes a KittyKeyboardInput into traditional VT100/Xterm byte sequences
    /// for legacy PTY compatibility. Returns None if it shouldn't generate output (e.g. key release).
    pub fn encode_legacy(input: &KittyKeyboardInput) -> Option<Vec<u8>> {
        // Only process Press (1) and Repeat (3), ignore Release (2)
        if input.event_type == 2 {
            return None;
        }

        let key = input.key_code;
        let mods = input.modifiers;

        let shift = (mods & 0b0001) != 0;
        let alt = (mods & 0b0010) != 0;
        let ctrl = (mods & 0b0100) != 0;

        // Modifier VT100 bit format: 1 + (shift * 1) + (alt * 2) + (ctrl * 4)
        let vt_mod = 1 + (if shift { 1 } else { 0 }) + (if alt { 2 } else { 0 }) + (if ctrl { 4 } else { 0 });

        // Basic character encoding (e.g. A-Z with Ctrl)
        if (32..=126).contains(&key) {
            let mut c = key as u8;
            
            if ctrl {
                // Ctrl+A = 1, Ctrl+Z = 26
                if (65..=90).contains(&c) {
                    c -= 64;
                } else if (97..=122).contains(&c) {
                    c -= 96;
                }
            }

            let mut seq = Vec::new();
            if alt {
                seq.push(27); // ESC prefix for Alt
            }
            seq.push(c);
            return Some(seq);
        }

        // Special keys (Arrows, Enter, Esc, etc.)
        match key {
            257 => { // Enter
                if alt {
                    Some(vec![27, 13])
                } else {
                    Some(vec![13])
                }
            }
            256 => Some(vec![27]), // Esc
            259 => Some(Self::csi(vt_mod, b'A')), // Up
            260 => Some(Self::csi(vt_mod, b'B')), // Down
            261 => Some(Self::csi(vt_mod, b'C')), // Right
            262 => Some(Self::csi(vt_mod, b'D')), // Left
            _ => None,
        }
    }

    fn csi(vt_mod: u32, suffix: u8) -> Vec<u8> {
        if vt_mod == 1 {
            vec![27, b'[', suffix]
        } else {
            let mut seq = vec![27, b'[', b'1', b';'];
            seq.extend_from_slice(vt_mod.to_string().as_bytes());
            seq.push(suffix);
            seq
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_legacy_basic_char() {
        let input = KittyKeyboardInput {
            key_code: b'a' as u32,
            modifiers: 0,
            event_type: 1,
        };
        assert_eq!(KeyEncoder::encode_legacy(&input), Some(vec![b'a']));
    }

    #[test]
    fn test_encode_legacy_ctrl_c() {
        let input = KittyKeyboardInput {
            key_code: b'c' as u32,
            modifiers: 0b0100, // Ctrl
            event_type: 1,
        };
        assert_eq!(KeyEncoder::encode_legacy(&input), Some(vec![3])); // Ctrl+C is 0x03
    }

    #[test]
    fn test_encode_legacy_alt_enter() {
        let input = KittyKeyboardInput {
            key_code: 257, // Enter
            modifiers: 0b0010, // Alt
            event_type: 1,
        };
        assert_eq!(KeyEncoder::encode_legacy(&input), Some(vec![27, 13]));
    }

    #[test]
    fn test_encode_legacy_shift_up() {
        let input = KittyKeyboardInput {
            key_code: 259, // Up Arrow
            modifiers: 0b0001, // Shift
            event_type: 1,
        };
        // 1 + 1(Shift) = 2 -> \x1b[1;2A
        assert_eq!(KeyEncoder::encode_legacy(&input), Some(vec![27, b'[', b'1', b';', b'2', b'A']));
    }

    #[test]
    fn test_encode_legacy_ignore_release() {
        let input = KittyKeyboardInput {
            key_code: b'a' as u32,
            modifiers: 0,
            event_type: 2, // Release
        };
        assert_eq!(KeyEncoder::encode_legacy(&input), None);
    }
}
