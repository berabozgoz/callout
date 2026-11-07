//! Scheduling priority for the engine's audio thread.
//!
//! The thread that encodes, decodes and mixes runs at normal priority by default. Under a game's
//! CPU load Windows then runs it late: field logs (CS2, League of Legends) showed playback
//! underruns and "late" packets with a clean network. cpal already raises its own WASAPI callback
//! threads; this raises ours the way pro-audio apps do, by joining the MMCSS "Pro Audio" task.
//! MMCSS keeps a share of CPU time for everything else (SystemResponsiveness), so a busy audio
//! thread cannot freeze the desktop. If MMCSS is unavailable the thread falls back to
//! `THREAD_PRIORITY_HIGHEST` (still in the normal range, below cpal's time-critical threads).
//!
//! Like callout-input, this crate holds `unsafe` code (CLAUDE.md rule 6), all of it in the
//! Windows module below.

#![cfg_attr(not(windows), forbid(unsafe_code))]
#![deny(unsafe_op_in_unsafe_fn)]

use std::marker::PhantomData;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("thread priority: {0}")]
    Os(String),
    #[error("thread priority is only raised on Windows")]
    Unsupported,
}

/// How the priority was raised.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Boost {
    /// Joined the MMCSS "Pro Audio" task.
    Mmcss,
    /// MMCSS failed; `THREAD_PRIORITY_HIGHEST` instead.
    ThreadPriority,
}

impl Boost {
    pub fn as_str(self) -> &'static str {
        match self {
            Boost::Mmcss => "mmcss pro audio",
            Boost::ThreadPriority => "thread priority highest",
        }
    }
}

/// The raised priority of the calling thread; restored when dropped. Not `Send`: it must be
/// dropped on the thread that created it.
#[must_use = "the priority is restored when this is dropped"]
pub struct AudioThreadPriority {
    boost: Boost,
    #[cfg(windows)]
    mmcss: windows_sys::Win32::Foundation::HANDLE,
    _not_send: PhantomData<*const ()>,
}

impl AudioThreadPriority {
    /// Raises the calling thread's priority for real-time audio work.
    pub fn raise() -> Result<Self, Error> {
        imp::raise()
    }

    pub fn boost(&self) -> Boost {
        self.boost
    }
}

#[cfg(windows)]
mod imp {
    use super::{AudioThreadPriority, Boost, Error, PhantomData};
    use windows_sys::Win32::System::Threading::{
        AvRevertMmThreadCharacteristics, AvSetMmThreadCharacteristicsW, GetCurrentThread,
        SetThreadPriority, THREAD_PRIORITY_HIGHEST, THREAD_PRIORITY_NORMAL,
    };

    pub(super) fn raise() -> Result<AudioThreadPriority, Error> {
        let task: Vec<u16> = "Pro Audio\0".encode_utf16().collect();
        let mut task_index = 0u32;
        // SAFETY: `task` is a NUL-terminated UTF-16 string that outlives the call; `task_index`
        // is a valid out pointer.
        let handle = unsafe { AvSetMmThreadCharacteristicsW(task.as_ptr(), &mut task_index) };
        if !handle.is_null() {
            return Ok(AudioThreadPriority {
                boost: Boost::Mmcss,
                mmcss: handle,
                _not_send: PhantomData,
            });
        }
        let mmcss_err = std::io::Error::last_os_error();
        // SAFETY: GetCurrentThread returns a pseudo handle that is always valid for the caller.
        let ok = unsafe { SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_HIGHEST) };
        if ok == 0 {
            return Err(Error::Os(format!(
                "mmcss: {mmcss_err}; SetThreadPriority: {}",
                std::io::Error::last_os_error()
            )));
        }
        Ok(AudioThreadPriority {
            boost: Boost::ThreadPriority,
            mmcss: std::ptr::null_mut(),
            _not_send: PhantomData,
        })
    }

    impl Drop for AudioThreadPriority {
        fn drop(&mut self) {
            // SAFETY: `mmcss` came from AvSetMmThreadCharacteristicsW on this thread (the type is
            // not Send) and is reverted once; the pseudo handle is always valid.
            unsafe {
                match self.boost {
                    Boost::Mmcss => {
                        AvRevertMmThreadCharacteristics(self.mmcss);
                    }
                    Boost::ThreadPriority => {
                        SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_NORMAL);
                    }
                }
            }
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use super::{AudioThreadPriority, Error};

    pub(super) fn raise() -> Result<AudioThreadPriority, Error> {
        Err(Error::Unsupported)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn raises_and_restores_on_the_same_thread() {
        std::thread::spawn(|| {
            let p = AudioThreadPriority::raise().expect("raise");
            assert!(matches!(p.boost(), Boost::Mmcss | Boost::ThreadPriority));
            drop(p);
            // Raising again after the restore works (the MMCSS slot was released).
            let again = AudioThreadPriority::raise().expect("raise again");
            drop(again);
        })
        .join()
        .unwrap();
    }

    #[cfg(not(windows))]
    #[test]
    fn unsupported_elsewhere() {
        assert!(matches!(
            AudioThreadPriority::raise(),
            Err(Error::Unsupported)
        ));
    }
}
