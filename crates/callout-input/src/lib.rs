//! Push-to-talk input that is *observed*, never swallowed: the key or mouse button still reaches
//! the game and every other app. A key, a combination (`Ctrl+KeyV`) or a mouse button
//! (`Mouse4`) can be bound; see [`Binding`]. The global hotkey it replaces (`RegisterHotKey`)
//! consumed the key system-wide, so a plain `V` could not be used in the game itself.
//!
//! Privacy: the app keeps a binding set only while push-to-talk is on and the user is in a room;
//! without one no input is received at all. Each event is compared with the binding and dropped;
//! nothing is stored or logged.
//!
//! This crate holds `unsafe` code (CLAUDE.md rule 6), all of it in the platform listeners
//! (`win.rs`: Raw Input; `mac.rs`: a Quartz event tap); the binding parser, the macOS key table
//! and the press/release logic are plain Rust and tested on every platform.

#![cfg_attr(not(any(windows, target_os = "macos")), forbid(unsafe_code))]
#![deny(unsafe_op_in_unsafe_fn)]

mod binding;
#[cfg(target_os = "macos")]
mod mac;
#[cfg(any(target_os = "macos", test))]
mod mac_keys;
mod matcher;
#[cfg(windows)]
mod win;

pub use binding::{Binding, Mods, MouseButton, ParseError, Trigger};
#[cfg(target_os = "macos")]
pub use mac::{has_input_access, request_input_access, Listener};
pub use matcher::{Input, Matcher};
#[cfg(windows)]
pub use win::Listener;

/// Whether the system lets the listener observe input in other apps. Only macOS asks the user
/// for this (Input Monitoring); elsewhere it is always allowed.
#[cfg(not(target_os = "macos"))]
pub fn has_input_access() -> bool {
    true
}

/// Asks the system for that access (macOS only; elsewhere there is nothing to ask).
#[cfg(not(target_os = "macos"))]
pub fn request_input_access() -> bool {
    true
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("push-to-talk listener: {0}")]
    Os(String),
    #[error("push-to-talk listener is only available on Windows and macOS")]
    Unsupported,
}

/// Elsewhere there is no listener (the app ships for Windows and macOS); the type exists so
/// callers compile.
#[cfg(not(any(windows, target_os = "macos")))]
pub struct Listener;

#[cfg(not(any(windows, target_os = "macos")))]
impl Listener {
    pub fn start(_on_change: impl Fn(bool) + Send + 'static) -> Result<Self, Error> {
        Err(Error::Unsupported)
    }

    pub fn set_binding(&self, _binding: Option<Binding>) {}
}
