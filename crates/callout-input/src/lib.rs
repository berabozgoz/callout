//! Push-to-talk input that is *observed*, never swallowed: the key or mouse button still reaches
//! the game and every other app. A key, a combination (`Ctrl+KeyV`) or a mouse button
//! (`Mouse4`) can be bound; see [`Binding`]. The global hotkey it replaces (`RegisterHotKey`)
//! consumed the key system-wide, so a plain `V` could not be used in the game itself.
//!
//! Privacy: the app keeps a binding set only while push-to-talk is on and the user is in a room;
//! without one no input is received at all. Each event is compared with the binding and dropped;
//! nothing is stored or logged.
//!
//! This is the one crate with `unsafe` code (CLAUDE.md rule 6), all of it in the Windows listener
//! (`win.rs`); the binding parser and the press/release logic are plain Rust and tested on every
//! platform.

#![cfg_attr(not(windows), forbid(unsafe_code))]
#![deny(unsafe_op_in_unsafe_fn)]

mod binding;
mod matcher;
#[cfg(windows)]
mod win;

pub use binding::{Binding, Mods, MouseButton, ParseError, Trigger};
pub use matcher::{Input, Matcher};
#[cfg(windows)]
pub use win::Listener;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("push-to-talk listener: {0}")]
    Os(String),
    #[error("push-to-talk listener is only available on Windows")]
    Unsupported,
}

/// Elsewhere there is no listener (the app is Windows only); the type exists so callers compile.
#[cfg(not(windows))]
pub struct Listener;

#[cfg(not(windows))]
impl Listener {
    pub fn start(_on_change: impl Fn(bool) + Send + 'static) -> Result<Self, Error> {
        Err(Error::Unsupported)
    }

    pub fn set_binding(&self, _binding: Option<Binding>) {}
}
