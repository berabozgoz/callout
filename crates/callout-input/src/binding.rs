//! Push-to-talk bindings as stored in the settings: the UI's key capture writes
//! `KeyboardEvent.code` names (`KeyV`, `F8`, `Numpad5`, …) joined with modifiers
//! (`Ctrl+Alt+KeyZ`), or a mouse button (`Mouse4`). Keys are matched by their physical position
//! (scan code), like `KeyboardEvent.code`, so the keyboard layout does not matter.

use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Middle,
    /// The rear side button (XButton1), "Mouse 4".
    Back,
    /// The front side button (XButton2), "Mouse 5".
    Forward,
}

/// What starts the transmission.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trigger {
    /// A key by its set-1 scan code; `e0` for the extended keys (arrows, Insert, …).
    Key {
        scan: u16,
        e0: bool,
    },
    Mouse(MouseButton),
}

/// Modifiers that must be held together with the trigger (either side counts).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Mods {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub win: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Binding {
    pub mods: Mods,
    pub trigger: Trigger,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ParseError {
    #[error("empty shortcut")]
    Empty,
    #[error("unknown modifier '{0}'")]
    UnknownModifier(String),
    #[error("unsupported key '{0}'")]
    UnknownKey(String),
}

impl FromStr for Binding {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let parts: Vec<&str> = s.split('+').map(str::trim).collect();
        let (last, mods_part) = parts.split_last().ok_or(ParseError::Empty)?;
        if last.is_empty() {
            return Err(ParseError::Empty);
        }
        let mut mods = Mods::default();
        for m in mods_part {
            match m.to_ascii_lowercase().as_str() {
                "ctrl" | "control" => mods.ctrl = true,
                "alt" => mods.alt = true,
                "shift" => mods.shift = true,
                "super" | "win" | "meta" | "cmd" => mods.win = true,
                _ => return Err(ParseError::UnknownModifier(m.to_string())),
            }
        }
        let trigger = match *last {
            "Mouse3" => Trigger::Mouse(MouseButton::Middle),
            "Mouse4" => Trigger::Mouse(MouseButton::Back),
            "Mouse5" => Trigger::Mouse(MouseButton::Forward),
            code => {
                let (scan, e0) =
                    scan_code(code).ok_or_else(|| ParseError::UnknownKey(code.to_string()))?;
                Trigger::Key { scan, e0 }
            }
        };
        Ok(Self { mods, trigger })
    }
}

impl fmt::Display for MouseButton {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Middle => "Mouse3",
            Self::Back => "Mouse4",
            Self::Forward => "Mouse5",
        })
    }
}

/// Set-1 scan code of a `KeyboardEvent.code` name. Modifier keys alone, Pause and NumLock are
/// not offered (the UI waits for a real key; the last two arrive as odd raw sequences).
fn scan_code(code: &str) -> Option<(u16, bool)> {
    if let Some(c) = code.strip_prefix("Key") {
        let mut chars = c.chars();
        let (Some(ch), None) = (chars.next(), chars.next()) else {
            return None;
        };
        const ROWS: [(&str, u16); 3] =
            [("QWERTYUIOP", 0x10), ("ASDFGHJKL", 0x1E), ("ZXCVBNM", 0x2C)];
        return ROWS
            .iter()
            .find_map(|(row, first)| row.find(ch).map(|i| (first + i as u16, false)));
    }
    if let Some(d) = code.strip_prefix("Digit") {
        let n: u16 = d.parse().ok().filter(|n| *n <= 9)?;
        return Some((if n == 0 { 0x0B } else { 0x01 + n }, false));
    }
    if let Some(n) = code.strip_prefix('F').and_then(|n| n.parse::<u16>().ok()) {
        return match n {
            1..=10 => Some((0x3A + n, false)),
            11 | 12 => Some((0x57 + n - 11, false)),
            13..=23 => Some((0x64 + n - 13, false)),
            24 => Some((0x76, false)),
            _ => None,
        };
    }
    if let Some(d) = code
        .strip_prefix("Numpad")
        .and_then(|d| d.parse::<u16>().ok())
    {
        const NUMPAD: [u16; 10] = [0x52, 0x4F, 0x50, 0x51, 0x4B, 0x4C, 0x4D, 0x47, 0x48, 0x49];
        return NUMPAD.get(d as usize).map(|s| (*s, false));
    }
    let k = match code {
        "Escape" => (0x01, false),
        "Minus" => (0x0C, false),
        "Equal" => (0x0D, false),
        "Backspace" => (0x0E, false),
        "Tab" => (0x0F, false),
        "BracketLeft" => (0x1A, false),
        "BracketRight" => (0x1B, false),
        "Enter" => (0x1C, false),
        "Semicolon" => (0x27, false),
        "Quote" => (0x28, false),
        "Backquote" => (0x29, false),
        "Backslash" => (0x2B, false),
        "Comma" => (0x33, false),
        "Period" => (0x34, false),
        "Slash" => (0x35, false),
        "NumpadMultiply" => (0x37, false),
        "Space" => (0x39, false),
        "CapsLock" => (0x3A, false),
        "ScrollLock" => (0x46, false),
        "NumpadSubtract" => (0x4A, false),
        "NumpadAdd" => (0x4E, false),
        "NumpadDecimal" => (0x53, false),
        "IntlBackslash" => (0x56, false),
        "IntlRo" => (0x73, false),
        "IntlYen" => (0x7D, false),
        "NumpadEnter" => (0x1C, true),
        "NumpadDivide" => (0x35, true),
        "PrintScreen" => (0x37, true),
        "Home" => (0x47, true),
        "ArrowUp" => (0x48, true),
        "PageUp" => (0x49, true),
        "ArrowLeft" => (0x4B, true),
        "ArrowRight" => (0x4D, true),
        "End" => (0x4F, true),
        "ArrowDown" => (0x50, true),
        "PageDown" => (0x51, true),
        "Insert" => (0x52, true),
        "Delete" => (0x53, true),
        "ContextMenu" => (0x5D, true),
        _ => return None,
    };
    Some(k)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(s: &str) -> (u16, bool) {
        match s.parse::<Binding>().unwrap().trigger {
            Trigger::Key { scan, e0 } => (scan, e0),
            t => panic!("{s}: {t:?}"),
        }
    }

    #[test]
    fn scan_codes_of_common_keys() {
        assert_eq!(key("KeyQ"), (0x10, false));
        assert_eq!(key("KeyV"), (0x2F, false));
        assert_eq!(key("KeyL"), (0x26, false));
        assert_eq!(key("KeyM"), (0x32, false));
        assert_eq!(key("Digit1"), (0x02, false));
        assert_eq!(key("Digit9"), (0x0A, false));
        assert_eq!(key("Digit0"), (0x0B, false));
        assert_eq!(key("F1"), (0x3B, false));
        assert_eq!(key("F9"), (0x43, false));
        assert_eq!(key("F10"), (0x44, false));
        assert_eq!(key("F12"), (0x58, false));
        assert_eq!(key("F13"), (0x64, false));
        assert_eq!(key("Numpad0"), (0x52, false));
        assert_eq!(key("Numpad8"), (0x48, false));
        assert_eq!(
            key("ArrowUp"),
            (0x48, true),
            "differs from Numpad8 only by E0"
        );
        assert_eq!(key("CapsLock"), (0x3A, false));
        assert_eq!(key("Backquote"), (0x29, false));
    }

    #[test]
    fn modifiers_and_mouse_buttons() {
        let b: Binding = "Ctrl+Alt+KeyZ".parse().unwrap();
        assert!(b.mods.ctrl && b.mods.alt && !b.mods.shift && !b.mods.win);
        let b: Binding = "Shift+Mouse4".parse().unwrap();
        assert!(b.mods.shift);
        assert_eq!(b.trigger, Trigger::Mouse(MouseButton::Back));
        assert_eq!(
            "Mouse5".parse::<Binding>().unwrap().trigger,
            Trigger::Mouse(MouseButton::Forward)
        );
        assert!("Super+KeyV".parse::<Binding>().unwrap().mods.win);
    }

    /// Everything the old global-shortcut setting accepted from the capture field still parses.
    #[test]
    fn captured_key_codes_parse() {
        for s in [
            "F9",
            "F4",
            "Ctrl+F4",
            "KeyT",
            "Digit9",
            "Ctrl+KeyT",
            "Ctrl+Alt+KeyZ",
            "Ctrl+Shift+Digit1",
            "Alt+ArrowUp",
            "Space",
            "Ctrl+Space",
            "Numpad5",
            "Super+KeyV",
            "Backquote",
        ] {
            assert!(s.parse::<Binding>().is_ok(), "failed to parse: {s}");
        }
    }

    #[test]
    fn bad_input_is_rejected() {
        assert_eq!("".parse::<Binding>(), Err(ParseError::Empty));
        assert_eq!("Ctrl+".parse::<Binding>(), Err(ParseError::Empty));
        assert!(matches!(
            "Hyper+KeyV".parse::<Binding>(),
            Err(ParseError::UnknownModifier(_))
        ));
        assert!(matches!(
            "KeyÄ".parse::<Binding>(),
            Err(ParseError::UnknownKey(_))
        ));
        assert!(matches!(
            "Pause".parse::<Binding>(),
            Err(ParseError::UnknownKey(_))
        ));
        assert!(matches!(
            "F25".parse::<Binding>(),
            Err(ParseError::UnknownKey(_))
        ));
    }
}
