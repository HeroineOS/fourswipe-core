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
///
/// Recognition thresholds are screen-relative, not a fixed pixel/unit
/// distance: a swipe only counts once it covers a large fraction of the
/// screen, so a light touch-and-drag of a few fingers (e.g. picking up a
/// screenshot-tool gesture, or an accidental brush) can't be mistaken for
/// deliberate intent to switch. Callers (backends) are expected to fill in
/// `screen_width`/`screen_height` from the real touch surface's coordinate
/// range — the detector itself stays backend-agnostic, it just needs to be
/// told what "most of the screen" means in whatever units the backend's
/// `TouchPoint`s use.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GestureConfig {
    /// Exact number of simultaneous fingers required. HeroineOS's TTY and
    /// OS switching both use 4.
    pub required_fingers: u8,
    /// Touch surface extent, in the same units as [`TouchPoint`] coordinates
    /// (e.g. a touchscreen's raw `ABS_MT_POSITION_X`/`Y` range).
    pub screen_width: f64,
    pub screen_height: f64,
    /// Fraction of the relevant screen dimension (width for a
    /// left/right swipe, height for up/down) a swipe must cover before
    /// it's recognized as deliberate. 0.4-0.7 is the useful range; default
    /// is 0.4 (40% of the screen).
    pub recognize_fraction: f64,
    /// Ignore movement smaller than this per-frame (device noise floor).
    pub jitter_threshold: f64,
}

impl GestureConfig {
    pub(crate) fn recognize_distance_x(&self) -> f64 {
        self.screen_width * self.recognize_fraction
    }

    pub(crate) fn recognize_distance_y(&self) -> f64 {
        self.screen_height * self.recognize_fraction
    }
}

impl Default for GestureConfig {
    fn default() -> Self {
        Self {
            required_fingers: 4,
            // Deliberately 0x0: a caller that forgets to fill these in from
            // real touch-surface geometry gets a config that can never
            // recognize anything, rather than one that silently uses a
            // fixed pixel distance meaningless on their device.
            screen_width: 0.0,
            screen_height: 0.0,
            recognize_fraction: 0.4,
            jitter_threshold: 2.0,
        }
    }
}
