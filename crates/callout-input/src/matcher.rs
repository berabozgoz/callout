//! Turns raw key and mouse button events into push-to-talk press/release, platform independent
//! (the Windows listener feeds it; the tests feed it directly).
//!
//! - The trigger going down while the binding's modifiers are held starts the transmission;
//!   extra modifiers are ignored (sprinting with Shift must not block a plain `V`).
//! - Only the trigger going up ends it; key repeat while held changes nothing.
//! - Modifiers are tracked per side from the same event stream.

use crate::binding::{Binding, Mods, MouseButton, Trigger};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Input {
    Key { scan: u16, e0: bool, down: bool },
    Mouse { button: MouseButton, down: bool },
}

#[derive(Debug, Default)]
pub struct Matcher {
    binding: Option<Binding>,
    pressed: bool,
    // Held modifiers, left and right.
    ctrl: [bool; 2],
    alt: [bool; 2],
    shift: [bool; 2],
    win: [bool; 2],
}

impl Matcher {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn binding(&self) -> Option<Binding> {
        self.binding
    }

    pub fn is_pressed(&self) -> bool {
        self.pressed
    }

    /// Changes (or clears) the binding. Returns `Some(false)` if a transmission was going on:
    /// it must end, whatever the new binding.
    pub fn set_binding(&mut self, binding: Option<Binding>) -> Option<bool> {
        self.binding = binding;
        self.release()
    }

    /// Ends a transmission (the trigger is no longer physically down, e.g. its key-up was lost
    /// on the secure desktop). Returns `Some(false)` if it was on.
    pub fn release(&mut self) -> Option<bool> {
        std::mem::take(&mut self.pressed).then_some(false)
    }

    /// Feeds one event; returns the new state if it changed.
    pub fn feed(&mut self, input: Input) -> Option<bool> {
        if let Input::Key { scan, e0, down } = input {
            self.track_modifier(scan, e0, down);
        }
        let binding = self.binding?;
        let (hit, down) = match (input, binding.trigger) {
            (Input::Key { scan, e0, down }, Trigger::Key { scan: s, e0: e }) => {
                (scan == s && e0 == e, down)
            }
            (Input::Mouse { button, down }, Trigger::Mouse(b)) => (button == b, down),
            _ => (false, false),
        };
        if !hit {
            return None;
        }
        if down {
            if self.pressed || !self.holds(binding.mods) {
                return None;
            }
            self.pressed = true;
            Some(true)
        } else {
            self.release()
        }
    }

    fn holds(&self, m: Mods) -> bool {
        let any = |s: [bool; 2]| s[0] || s[1];
        (!m.ctrl || any(self.ctrl))
            && (!m.alt || any(self.alt))
            && (!m.shift || any(self.shift))
            && (!m.win || any(self.win))
    }

    fn track_modifier(&mut self, scan: u16, e0: bool, down: bool) {
        let slot = match (scan, e0) {
            (0x1D, false) => &mut self.ctrl[0],
            (0x1D, true) => &mut self.ctrl[1],
            (0x38, false) => &mut self.alt[0],
            (0x38, true) => &mut self.alt[1],
            // E0 2A / E0 36 are the "fake shifts" Windows wraps around some extended keys.
            (0x2A, false) => &mut self.shift[0],
            (0x36, false) => &mut self.shift[1],
            (0x5B, true) => &mut self.win[0],
            (0x5C, true) => &mut self.win[1],
            _ => return,
        };
        *slot = down;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(scan: u16, down: bool) -> Input {
        Input::Key {
            scan,
            e0: false,
            down,
        }
    }

    const V: u16 = 0x2F;
    const CTRL: u16 = 0x1D;
    const SHIFT: u16 = 0x2A;

    fn matcher(s: &str) -> Matcher {
        let mut m = Matcher::new();
        m.set_binding(Some(s.parse().unwrap()));
        m
    }

    #[test]
    fn plain_key_press_repeat_release() {
        let mut m = matcher("KeyV");
        assert_eq!(m.feed(key(V, true)), Some(true));
        // Key repeat while held.
        assert_eq!(m.feed(key(V, true)), None);
        assert_eq!(m.feed(key(0x2E, true)), None, "another key");
        assert_eq!(m.feed(key(V, false)), Some(false));
        assert_eq!(m.feed(key(V, false)), None);
    }

    #[test]
    fn extra_modifiers_do_not_block_a_plain_key() {
        let mut m = matcher("KeyV");
        m.feed(key(SHIFT, true));
        assert_eq!(m.feed(key(V, true)), Some(true));
    }

    #[test]
    fn combination_needs_its_modifier() {
        let mut m = matcher("Ctrl+KeyV");
        assert_eq!(m.feed(key(V, true)), None);
        m.feed(key(V, false));
        // Right Ctrl counts as well.
        m.feed(Input::Key {
            scan: CTRL,
            e0: true,
            down: true,
        });
        assert_eq!(m.feed(key(V, true)), Some(true));
        // Letting go of Ctrl first keeps talking until the key itself is released.
        m.feed(Input::Key {
            scan: CTRL,
            e0: true,
            down: false,
        });
        assert_eq!(m.feed(key(V, false)), Some(false));
    }

    #[test]
    fn extended_and_plain_keys_are_told_apart() {
        let mut m = matcher("ArrowUp");
        assert_eq!(m.feed(key(0x48, true)), None, "Numpad8");
        assert_eq!(
            m.feed(Input::Key {
                scan: 0x48,
                e0: true,
                down: true
            }),
            Some(true)
        );
    }

    #[test]
    fn mouse_side_button() {
        let mut m = matcher("Mouse4");
        let back = |down| Input::Mouse {
            button: MouseButton::Back,
            down,
        };
        assert_eq!(
            m.feed(Input::Mouse {
                button: MouseButton::Forward,
                down: true
            }),
            None
        );
        assert_eq!(m.feed(back(true)), Some(true));
        assert_eq!(m.feed(back(false)), Some(false));
    }

    #[test]
    fn clearing_the_binding_ends_a_transmission() {
        let mut m = matcher("KeyV");
        m.feed(key(V, true));
        assert_eq!(m.set_binding(None), Some(false));
        assert_eq!(m.feed(key(V, true)), None);
        assert_eq!(m.set_binding(None), None);
    }
}
