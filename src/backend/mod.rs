//! Backends turn a real input source into [`crate::gesture::RawTouchEvent`]s
//! for a [`crate::detector::GestureDetector`]. New consumers (Android via
//! Termux:GUI, a future OS-switch backend, etc.) implement this trait
//! instead of touching the detector's recognition logic.

use crate::gesture::RawTouchEvent;

#[derive(Debug, thiserror::Error)]
pub enum BackendError {
    #[error("no suitable multitouch input device found")]
    NoDevice,
    #[error("permission denied accessing input device (need root or input group membership)")]
    PermissionDenied,
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

/// Something that can be polled for raw touch events. Blocking by design —
/// each backend runs its own read loop on its own thread.
pub trait InputBackend {
    /// Block until the next raw event is available.
    fn next_event(&mut self) -> Result<RawTouchEvent, BackendError>;
}

#[cfg(all(target_os = "linux", feature = "evdev-backend"))]
pub mod evdev_backend;
