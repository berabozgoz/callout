//! macOS virtual key codes (`kVK_*`, by physical position like `KeyboardEvent.code`) to the set-1
//! scan codes the bindings and the matcher use, and back. Plain data, so it is tested everywhere.
//!
//! The pairs follow WebKit's own `kVK_*` → `KeyboardEvent.code` table, so a key captured in the
//! settings (the webview) is the key the listener reports. Notably `kVK_ISO_Section` is
//! `IntlBackslash` and `kVK_ANSI_Grave` is `Backquote`, and `kVK_Help` is `Insert`. Caps Lock is
//! left out: macOS reports it only when its lock state toggles, never on release.

/// `(kVK, scan, e0)`.
const KEYS: &[(u16, u16, bool)] = &[
    // Letters.
    (0x00, 0x1E, false), // A
    (0x01, 0x1F, false), // S
    (0x02, 0x20, false), // D
    (0x03, 0x21, false), // F
    (0x04, 0x23, false), // H
    (0x05, 0x22, false), // G
    (0x06, 0x2C, false), // Z
    (0x07, 0x2D, false), // X
    (0x08, 0x2E, false), // C
    (0x09, 0x2F, false), // V
    (0x0B, 0x30, false), // B
    (0x0C, 0x10, false), // Q
    (0x0D, 0x11, false), // W
    (0x0E, 0x12, false), // E
    (0x0F, 0x13, false), // R
    (0x10, 0x15, false), // Y
    (0x11, 0x14, false), // T
    (0x1F, 0x18, false), // O
    (0x20, 0x16, false), // U
    (0x22, 0x17, false), // I
    (0x23, 0x19, false), // P
    (0x25, 0x26, false), // L
    (0x26, 0x24, false), // J
    (0x28, 0x25, false), // K
    (0x2D, 0x31, false), // N
    (0x2E, 0x32, false), // M
    // Digits.
    (0x12, 0x02, false), // 1
    (0x13, 0x03, false), // 2
    (0x14, 0x04, false), // 3
    (0x15, 0x05, false), // 4
    (0x17, 0x06, false), // 5
    (0x16, 0x07, false), // 6
    (0x1A, 0x08, false), // 7
    (0x1C, 0x09, false), // 8
    (0x19, 0x0A, false), // 9
    (0x1D, 0x0B, false), // 0
    // Punctuation and editing keys.
    (0x18, 0x0D, false), // Equal
    (0x1B, 0x0C, false), // Minus
    (0x1E, 0x1B, false), // BracketRight
    (0x21, 0x1A, false), // BracketLeft
    (0x24, 0x1C, false), // Enter (Return)
    (0x27, 0x28, false), // Quote
    (0x29, 0x27, false), // Semicolon
    (0x2A, 0x2B, false), // Backslash
    (0x2B, 0x33, false), // Comma
    (0x2C, 0x35, false), // Slash
    (0x2F, 0x34, false), // Period
    (0x30, 0x0F, false), // Tab
    (0x31, 0x39, false), // Space
    (0x32, 0x29, false), // Backquote
    (0x33, 0x0E, false), // Backspace (Delete)
    (0x35, 0x01, false), // Escape
    (0x0A, 0x56, false), // IntlBackslash (ISO Section)
    (0x5D, 0x7D, false), // IntlYen
    (0x5E, 0x73, false), // IntlRo
    // Numpad.
    (0x41, 0x53, false), // NumpadDecimal
    (0x43, 0x37, false), // NumpadMultiply
    (0x45, 0x4E, false), // NumpadAdd
    (0x4B, 0x35, true),  // NumpadDivide
    (0x4C, 0x1C, true),  // NumpadEnter
    (0x4E, 0x4A, false), // NumpadSubtract
    (0x52, 0x52, false), // Numpad0
    (0x53, 0x4F, false), // Numpad1
    (0x54, 0x50, false), // Numpad2
    (0x55, 0x51, false), // Numpad3
    (0x56, 0x4B, false), // Numpad4
    (0x57, 0x4C, false), // Numpad5
    (0x58, 0x4D, false), // Numpad6
    (0x59, 0x47, false), // Numpad7
    (0x5B, 0x48, false), // Numpad8
    (0x5C, 0x49, false), // Numpad9
    // Function keys.
    (0x7A, 0x3B, false), // F1
    (0x78, 0x3C, false), // F2
    (0x63, 0x3D, false), // F3
    (0x76, 0x3E, false), // F4
    (0x60, 0x3F, false), // F5
    (0x61, 0x40, false), // F6
    (0x62, 0x41, false), // F7
    (0x64, 0x42, false), // F8
    (0x65, 0x43, false), // F9
    (0x6D, 0x44, false), // F10
    (0x67, 0x57, false), // F11
    (0x6F, 0x58, false), // F12
    (0x69, 0x64, false), // F13
    (0x6B, 0x65, false), // F14
    (0x71, 0x66, false), // F15
    (0x6A, 0x67, false), // F16
    (0x40, 0x68, false), // F17
    (0x4F, 0x69, false), // F18
    (0x50, 0x6A, false), // F19
    (0x5A, 0x6B, false), // F20
    // Navigation.
    (0x72, 0x52, true), // Insert (Help)
    (0x73, 0x47, true), // Home
    (0x74, 0x49, true), // PageUp
    (0x75, 0x53, true), // Delete (forward delete)
    (0x77, 0x4F, true), // End
    (0x79, 0x51, true), // PageDown
    (0x7B, 0x4B, true), // ArrowLeft
    (0x7C, 0x4D, true), // ArrowRight
    (0x7D, 0x50, true), // ArrowDown
    (0x7E, 0x48, true), // ArrowUp
    (0x6E, 0x5D, true), // ContextMenu
    // Modifiers (reported as "flags changed", see `modifier_flag`).
    (0x3B, 0x1D, false), // Control
    (0x3E, 0x1D, true),  // RightControl
    (0x38, 0x2A, false), // Shift
    (0x3C, 0x36, false), // RightShift
    (0x3A, 0x38, false), // Option
    (0x3D, 0x38, true),  // RightOption
    (0x37, 0x5B, true),  // Command
    (0x36, 0x5C, true),  // RightCommand
];

/// The scan code of a macOS virtual key.
pub(crate) fn to_scan(kvk: u16) -> Option<(u16, bool)> {
    KEYS.iter()
        .find(|(k, ..)| *k == kvk)
        .map(|&(_, scan, e0)| (scan, e0))
}

/// The macOS virtual key of a scan code (for the "still held?" check).
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub(crate) fn to_kvk(scan: u16, e0: bool) -> Option<u16> {
    KEYS.iter()
        .find(|&&(_, s, e)| s == scan && e == e0)
        .map(|&(k, ..)| k)
}

/// For a modifier key: its device-dependent bit in the event flags (`NX_DEVICE*KEYMASK`), set
/// while that side's key is down. A "flags changed" event names the key that changed; this bit
/// says whether it went down or up.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub(crate) fn modifier_flag(kvk: u16) -> Option<u64> {
    Some(match kvk {
        0x3B => 0x0000_0001, // left control
        0x38 => 0x0000_0002, // left shift
        0x3C => 0x0000_0004, // right shift
        0x37 => 0x0000_0008, // left command
        0x36 => 0x0000_0010, // right command
        0x3A => 0x0000_0020, // left option
        0x3D => 0x0000_0040, // right option
        0x3E => 0x0000_2000, // right control
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Binding, Trigger};

    fn scan_of(code: &str) -> (u16, bool) {
        match code.parse::<Binding>().unwrap().trigger {
            Trigger::Key { scan, e0 } => (scan, e0),
            t => panic!("{code}: {t:?}"),
        }
    }

    #[test]
    fn no_duplicates_either_way() {
        for (i, a) in KEYS.iter().enumerate() {
            for b in &KEYS[i + 1..] {
                assert_ne!(a.0, b.0, "kVK {:#x} twice", a.0);
                assert!((a.1, a.2) != (b.1, b.2), "scan {:#x}/{} twice", a.1, a.2);
            }
        }
    }

    #[test]
    fn round_trip() {
        for &(kvk, scan, e0) in KEYS {
            assert_eq!(to_scan(kvk), Some((scan, e0)));
            assert_eq!(to_kvk(scan, e0), Some(kvk));
        }
    }

    /// The captured `KeyboardEvent.code` and the key the listener reports agree.
    #[test]
    fn captured_codes_match_mac_keys() {
        for (code, kvk) in [
            ("KeyV", 0x09),
            ("KeyQ", 0x0C),
            ("KeyM", 0x2E),
            ("Digit1", 0x12),
            ("Digit0", 0x1D),
            ("F1", 0x7A),
            ("F9", 0x65),
            ("F12", 0x6F),
            ("F19", 0x50),
            ("Space", 0x31),
            ("Backquote", 0x32),
            ("IntlBackslash", 0x0A),
            ("Numpad5", 0x57),
            ("NumpadEnter", 0x4C),
            ("ArrowUp", 0x7E),
            ("Insert", 0x72),
            ("Delete", 0x75),
            ("Backspace", 0x33),
        ] {
            assert_eq!(to_scan(kvk), Some(scan_of(code)), "{code}");
        }
    }

    #[test]
    fn modifiers_have_flags_and_matcher_scan_codes() {
        let mods = [0x3B, 0x3E, 0x38, 0x3C, 0x3A, 0x3D, 0x37, 0x36];
        for kvk in mods {
            assert!(modifier_flag(kvk).is_some(), "{kvk:#x}");
            assert!(to_scan(kvk).is_some(), "{kvk:#x}");
        }
        assert_eq!(modifier_flag(0x09), None, "V is not a modifier");
        // Left and right Command are the matcher's left and right "win" keys.
        assert_eq!(to_scan(0x37), Some((0x5B, true)));
        assert_eq!(to_scan(0x36), Some((0x5C, true)));
    }
}
