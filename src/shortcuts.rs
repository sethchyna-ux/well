//! Runtime matcher for user-configured desktop shortcuts.
//!
//! The configuration crate owns persistence and action names. This module owns
//! the winit-specific translation from a key event into a configured action.

use well_config::{KeybindingRow, ShortcutAction};
use winit::keyboard::{Key, KeyCode, NamedKey, PhysicalKey};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ShortcutModifiers {
    pub command: bool,
    pub control: bool,
    pub shift: bool,
    pub alt: bool,
}

pub fn matching_action(
    bindings: &[KeybindingRow],
    logical_key: &Key,
    physical_key: PhysicalKey,
    modifiers: ShortcutModifiers,
) -> Option<ShortcutAction> {
    bindings.iter().find_map(|binding| {
        if binding.conflict {
            return None;
        }

        let action = ShortcutAction::from_label(&binding.action)?;
        chord_matches(&binding.chord, logical_key, physical_key, modifiers).then_some(action)
    })
}

fn chord_matches(
    chord: &str,
    logical_key: &Key,
    physical_key: PhysicalKey,
    modifiers: ShortcutModifiers,
) -> bool {
    chord.split('/').any(|alternative| {
        let mut command = false;
        let mut control = false;
        let mut shift = false;
        let mut alt = false;
        let mut key = None;

        for raw_part in alternative.split('+') {
            let part = raw_part.trim();
            if part.is_empty() {
                return false;
            }

            match part.to_ascii_lowercase().as_str() {
                "cmd" | "command" | "super" | "meta" => command = true,
                "ctrl" | "control" => control = true,
                "shift" => shift = true,
                "alt" | "option" => alt = true,
                _ => {
                    if key.replace(part).is_some() {
                        return false;
                    }
                }
            }
        }

        command == modifiers.command
            && control == modifiers.control
            && shift == modifiers.shift
            && alt == modifiers.alt
            && key.is_some_and(|key| key_matches(key, logical_key, physical_key))
    })
}

fn key_matches(expected: &str, logical_key: &Key, physical_key: PhysicalKey) -> bool {
    let expected = normalize_key_name(expected);

    match logical_key {
        Key::Character(value) if key_names_match(&expected, &normalize_key_name(value)) => true,
        Key::Named(named)
            if named_key_name(*named).is_some_and(|actual| key_names_match(&expected, actual)) =>
        {
            true
        }
        _ => {
            physical_key_name(physical_key).is_some_and(|actual| key_names_match(&expected, actual))
        }
    }
}

fn normalize_key_name(value: &str) -> String {
    match value.trim().to_ascii_uppercase().as_str() {
        "ESC" => "ESCAPE".to_string(),
        "RETURN" => "ENTER".to_string(),
        "PGUP" => "PAGEUP".to_string(),
        "PGDN" => "PAGEDOWN".to_string(),
        other => other.to_string(),
    }
}

fn key_names_match(expected: &str, actual: &str) -> bool {
    expected == actual
        || matches!(
            (expected, actual),
            ("+", "=") | ("=", "+") | ("PLUS", "+") | ("PLUS", "=")
        )
}

fn named_key_name(key: NamedKey) -> Option<&'static str> {
    match key {
        NamedKey::F1 => Some("F1"),
        NamedKey::F2 => Some("F2"),
        NamedKey::F3 => Some("F3"),
        NamedKey::F4 => Some("F4"),
        NamedKey::F5 => Some("F5"),
        NamedKey::F6 => Some("F6"),
        NamedKey::F7 => Some("F7"),
        NamedKey::F8 => Some("F8"),
        NamedKey::F9 => Some("F9"),
        NamedKey::F10 => Some("F10"),
        NamedKey::F11 => Some("F11"),
        NamedKey::F12 => Some("F12"),
        NamedKey::Escape => Some("ESCAPE"),
        NamedKey::Enter => Some("ENTER"),
        NamedKey::Tab => Some("TAB"),
        NamedKey::Home => Some("HOME"),
        NamedKey::End => Some("END"),
        NamedKey::PageUp => Some("PAGEUP"),
        NamedKey::PageDown => Some("PAGEDOWN"),
        _ => None,
    }
}

fn physical_key_name(key: PhysicalKey) -> Option<&'static str> {
    let PhysicalKey::Code(key) = key else {
        return None;
    };

    match key {
        KeyCode::KeyA => Some("A"),
        KeyCode::KeyB => Some("B"),
        KeyCode::KeyC => Some("C"),
        KeyCode::KeyD => Some("D"),
        KeyCode::KeyE => Some("E"),
        KeyCode::KeyF => Some("F"),
        KeyCode::KeyG => Some("G"),
        KeyCode::KeyH => Some("H"),
        KeyCode::KeyI => Some("I"),
        KeyCode::KeyJ => Some("J"),
        KeyCode::KeyK => Some("K"),
        KeyCode::KeyL => Some("L"),
        KeyCode::KeyM => Some("M"),
        KeyCode::KeyN => Some("N"),
        KeyCode::KeyO => Some("O"),
        KeyCode::KeyP => Some("P"),
        KeyCode::KeyQ => Some("Q"),
        KeyCode::KeyR => Some("R"),
        KeyCode::KeyS => Some("S"),
        KeyCode::KeyT => Some("T"),
        KeyCode::KeyU => Some("U"),
        KeyCode::KeyV => Some("V"),
        KeyCode::KeyW => Some("W"),
        KeyCode::KeyX => Some("X"),
        KeyCode::KeyY => Some("Y"),
        KeyCode::KeyZ => Some("Z"),
        KeyCode::Digit0 => Some("0"),
        KeyCode::Digit1 => Some("1"),
        KeyCode::Digit2 => Some("2"),
        KeyCode::Digit3 => Some("3"),
        KeyCode::Digit4 => Some("4"),
        KeyCode::Digit5 => Some("5"),
        KeyCode::Digit6 => Some("6"),
        KeyCode::Digit7 => Some("7"),
        KeyCode::Digit8 => Some("8"),
        KeyCode::Digit9 => Some("9"),
        KeyCode::Comma => Some(","),
        KeyCode::Equal => Some("="),
        KeyCode::Minus => Some("-"),
        KeyCode::F1 => Some("F1"),
        KeyCode::F2 => Some("F2"),
        KeyCode::F3 => Some("F3"),
        KeyCode::F4 => Some("F4"),
        KeyCode::F5 => Some("F5"),
        KeyCode::F6 => Some("F6"),
        KeyCode::F7 => Some("F7"),
        KeyCode::F8 => Some("F8"),
        KeyCode::F9 => Some("F9"),
        KeyCode::F10 => Some("F10"),
        KeyCode::F11 => Some("F11"),
        KeyCode::F12 => Some("F12"),
        KeyCode::Escape => Some("ESCAPE"),
        KeyCode::Enter => Some("ENTER"),
        KeyCode::Tab => Some("TAB"),
        KeyCode::Home => Some("HOME"),
        KeyCode::End => Some("END"),
        KeyCode::PageUp => Some("PAGEUP"),
        KeyCode::PageDown => Some("PAGEDOWN"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn binding(chord: &str, action: ShortcutAction) -> KeybindingRow {
        KeybindingRow {
            chord: chord.to_string(),
            action: action.label().to_string(),
            conflict: false,
        }
    }

    #[test]
    fn matches_configured_alternatives_and_physical_key_fallbacks() {
        let bindings = [binding(
            "Cmd+Shift+P / Ctrl+Shift+P",
            ShortcutAction::OpenCommandPalette,
        )];

        assert_eq!(
            matching_action(
                &bindings,
                &Key::Character("P".into()),
                PhysicalKey::Code(KeyCode::KeyP),
                ShortcutModifiers {
                    command: true,
                    shift: true,
                    ..ShortcutModifiers::default()
                },
            ),
            Some(ShortcutAction::OpenCommandPalette)
        );

        assert_eq!(
            matching_action(
                &bindings,
                &Key::Character("p".into()),
                PhysicalKey::Code(KeyCode::KeyP),
                ShortcutModifiers {
                    control: true,
                    shift: true,
                    ..ShortcutModifiers::default()
                },
            ),
            Some(ShortcutAction::OpenCommandPalette)
        );
    }

    #[test]
    fn requires_the_configured_modifier_set() {
        let bindings = [binding("Cmd+K", ShortcutAction::ClearBuffer)];

        assert_eq!(
            matching_action(
                &bindings,
                &Key::Character("k".into()),
                PhysicalKey::Code(KeyCode::KeyK),
                ShortcutModifiers::default(),
            ),
            None
        );
    }

    #[test]
    fn ignores_conflicted_and_unknown_bindings() {
        let bindings = [
            KeybindingRow {
                chord: "Cmd+K".to_string(),
                action: ShortcutAction::ClearBuffer.label().to_string(),
                conflict: true,
            },
            KeybindingRow {
                chord: "Cmd+K".to_string(),
                action: "Split Pane Horizontal".to_string(),
                conflict: false,
            },
        ];

        assert_eq!(
            matching_action(
                &bindings,
                &Key::Character("k".into()),
                PhysicalKey::Code(KeyCode::KeyK),
                ShortcutModifiers {
                    command: true,
                    ..ShortcutModifiers::default()
                },
            ),
            None
        );
    }
}
