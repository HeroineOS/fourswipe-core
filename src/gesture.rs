//! Backend-agnostic gesture types. Nothing in this module knows about evdev,
//! TTYs, or any particular OS — it only reasons about touch points and
//! swipes, so it can be reused by any [`crate::backend::InputBackend`].

/// A single raw touch point update, keyed by a stable per-touch slot id.
/// Backends translate whatever their native input protocol is into this.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TouchPoint {
    /// Stable identifier for one finger's contact, valid from down to up.
    pub slot: i32,
    pub x: f64,
    pub y: f64,
}

/// One backend input event: a finger touching down, moving, or lifting.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RawTouchEvent {
    Down(TouchPoint),
    Move(TouchPoint),
    Up { slot: i32 },
    /// Backend-native event batch boundary (e.g. evdev EV_SYN/SYN_REPORT).
    Frame,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwipeDirection {
    Left,
    Right,
    Up,
    Down,
}

/// A recognized gesture, emitted once enough movement has happened to
/// classify a direction. `finger_count` is fixed for the lifetime of one
/// gesture (a finger added/removed mid-swipe cancels it).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GestureEvent {
    Start { finger_count: u8 },
    Update {
        finger_count: u8,
        direction: SwipeDirection,
        /// Cumulative movement along `direction`, in the same units as
        /// [`TouchPoint`] coordinates (backend-defined, typically device px).
        distance: f64,
    },
    /// Gesture completed with enough distance/finger-count to count as a
    /// deliberate swipe — this is what consumers should act on.
    Recognized {
        finger_count: u8,
        direction: SwipeDirection,
    },
    /// Fingers lifted (or gesture invalidated) before it was recognized.
    Cancelled,
}

/// Tuning knobs for [`crate::detector::GestureDetector`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GestureConfig {
    /// Exact number of simultaneous fingers required. HeroineOS's TTY and
    /// OS switching both use 4.
    pub required_fingers: u8,
    /// Minimum straight-line distance (backend units) before a direction is
    /// recognized as deliberate rather than noise/jitter.
    pub recognize_distance: f64,
    /// Ignore movement smaller than this per-frame (device noise floor).
    pub jitter_threshold: f64,
}

impl Default for GestureConfig {
    fn default() -> Self {
        Self {
            required_fingers: 4,
            recognize_distance: 80.0,
            jitter_threshold: 2.0,
        }
    }
}
