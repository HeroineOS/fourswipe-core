use std::collections::HashMap;

use crate::gesture::{GestureConfig, GestureEvent, RawTouchEvent, SwipeDirection, TouchPoint};

/// Turns a stream of [`RawTouchEvent`]s from any backend into classified
/// [`GestureEvent`]s. Pure state machine, no I/O — keeps this reusable
/// across the evdev/TTY backend, a future Android/Termux:GUI backend, and
/// a future OS-switch backend without duplicating recognition logic.
pub struct GestureDetector {
    config: GestureConfig,
    active: HashMap<i32, TouchPoint>,
    origin: HashMap<i32, TouchPoint>,
    started: bool,
    recognized: bool,
    /// Set by `cancel()`: ignore everything until the fingers of the
    /// cancelled touch have lifted.
    suppressed: bool,
}

impl GestureDetector {
    pub fn new(config: GestureConfig) -> Self {
        Self {
            config,
            active: HashMap::new(),
            origin: HashMap::new(),
            started: false,
            recognized: false,
            suppressed: false,
        }
    }

    pub fn config(&self) -> &GestureConfig {
        &self.config
    }

    /// Hard reset: forget all in-progress touches and gesture state, and
    /// ignore input until the fingers currently down have lifted. Call this
    /// right after acting on a `Recognized` gesture so the rest of the same
    /// physical touch can't start (or be read as) another gesture.
    pub fn cancel(&mut self) {
        self.active.clear();
        self.origin.clear();
        self.started = false;
        self.recognized = false;
        self.suppressed = true;
    }

    /// Feed one raw event, get back zero or one classified gesture events.
    pub fn feed(&mut self, event: RawTouchEvent) -> Option<GestureEvent> {
        if self.suppressed {
            // Track the still-down fingers only to notice when they're gone.
            match event {
                RawTouchEvent::Down(tp) | RawTouchEvent::Move(tp) => {
                    self.active.insert(tp.slot, tp);
                }
                RawTouchEvent::Up { slot } => {
                    self.active.remove(&slot);
                    if self.active.is_empty() {
                        self.suppressed = false;
                    }
                }
                RawTouchEvent::Frame => {}
            }
            return None;
        }
        match event {
            RawTouchEvent::Down(tp) => {
                self.active.insert(tp.slot, tp);
                self.origin.insert(tp.slot, tp);
                // A finger count change mid-gesture invalidates it — a
                // deliberate swipe holds a fixed finger count throughout.
                if self.started && self.active.len() as u8 != self.config.required_fingers {
                    self.reset();
                    return Some(GestureEvent::Cancelled);
                }
                None
            }
            RawTouchEvent::Move(tp) => {
                self.active.insert(tp.slot, tp);
                None
            }
            RawTouchEvent::Up { slot } => {
                self.active.remove(&slot);
                self.origin.remove(&slot);
                if self.started {
                    let was_recognized = self.recognized;
                    self.reset();
                    if !was_recognized {
                        return Some(GestureEvent::Cancelled);
                    }
                }
                None
            }
            RawTouchEvent::Frame => self.evaluate(),
        }
    }

    fn reset(&mut self) {
        self.started = false;
        self.recognized = false;
    }

    fn evaluate(&mut self) -> Option<GestureEvent> {
        let n = self.active.len() as u8;

        if n != self.config.required_fingers {
            return None;
        }

        if !self.started {
            self.started = true;
            self.recognized = false;
            return Some(GestureEvent::Start {
                finger_count: n,
            });
        }

        if self.recognized {
            return None;
        }

        // Average displacement across all active fingers from their touch-down origin.
        let mut sum_dx = 0.0;
        let mut sum_dy = 0.0;
        for (slot, tp) in self.active.iter() {
            if let Some(origin) = self.origin.get(slot) {
                sum_dx += tp.x - origin.x;
                sum_dy += tp.y - origin.y;
            }
        }
        let dx = sum_dx / n as f64;
        let dy = sum_dy / n as f64;
        let distance = (dx * dx + dy * dy).sqrt();

        if distance < self.config.jitter_threshold {
            return None;
        }

        let horizontal = dx.abs() >= dy.abs();
        let direction = if horizontal {
            if dx > 0.0 { SwipeDirection::Right } else { SwipeDirection::Left }
        } else if dy > 0.0 {
            SwipeDirection::Down
        } else {
            SwipeDirection::Up
        };

        // Screen-relative threshold: only the axis matching the candidate
        // direction has to clear its fraction-of-screen distance, so a
        // deliberate mostly-horizontal swipe isn't penalized by a config
        // tuned differently for vertical (and vice versa).
        let required = if horizontal {
            self.config.recognize_distance_x()
        } else {
            self.config.recognize_distance_y()
        };

        if required > 0.0 && dx.abs().max(dy.abs()) >= required {
            self.recognized = true;
            return Some(GestureEvent::Recognized {
                finger_count: n,
                direction,
            });
        }

        Some(GestureEvent::Update {
            finger_count: n,
            direction,
            distance,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tp(slot: i32, x: f64, y: f64) -> TouchPoint {
        TouchPoint { slot, x, y }
    }

    // 1000x1000 "screen" with the default 40% recognize_fraction: a swipe
    // needs to cover >= 400 units to count.
    fn test_config() -> GestureConfig {
        GestureConfig {
            screen_width: 1000.0,
            screen_height: 1000.0,
            ..GestureConfig::default()
        }
    }

    #[test]
    fn recognizes_four_finger_right_swipe_covering_most_of_the_screen() {
        let mut d = GestureDetector::new(test_config());

        for slot in 0..4 {
            d.feed(RawTouchEvent::Down(tp(slot, 0.0, 0.0)));
        }
        assert_eq!(
            d.feed(RawTouchEvent::Frame),
            Some(GestureEvent::Start { finger_count: 4 })
        );

        for slot in 0..4 {
            d.feed(RawTouchEvent::Move(tp(slot, 700.0, 0.0)));
        }
        assert_eq!(
            d.feed(RawTouchEvent::Frame),
            Some(GestureEvent::Recognized {
                finger_count: 4,
                direction: SwipeDirection::Right,
            })
        );
    }

    #[test]
    fn small_drag_does_not_recognize() {
        // Regression: a light touch-and-drag (e.g. a screenshot tool's own
        // gesture, or an accidental brush) must not trigger a switch just
        // because 4 fingers happened to be down.
        let mut d = GestureDetector::new(test_config());
        for slot in 0..4 {
            d.feed(RawTouchEvent::Down(tp(slot, 0.0, 0.0)));
        }
        d.feed(RawTouchEvent::Frame);
        for slot in 0..4 {
            d.feed(RawTouchEvent::Move(tp(slot, 100.0, 0.0)));
        }
        // In progress, but nowhere near the 400-unit threshold — must not
        // be Recognized (or Cancelled).
        assert!(matches!(
            d.feed(RawTouchEvent::Frame),
            Some(GestureEvent::Update { .. }) | None
        ));
    }

    #[test]
    fn wrong_finger_count_never_recognizes() {
        let mut d = GestureDetector::new(test_config());
        for slot in 0..3 {
            d.feed(RawTouchEvent::Down(tp(slot, 0.0, 0.0)));
        }
        assert_eq!(d.feed(RawTouchEvent::Frame), None);
        for slot in 0..3 {
            d.feed(RawTouchEvent::Move(tp(slot, 900.0, 0.0)));
        }
        assert_eq!(d.feed(RawTouchEvent::Frame), None);
    }

    #[test]
    fn rest_of_a_cancelled_touch_is_ignored() {
        let mut d = GestureDetector::new(test_config());
        for slot in 0..4 {
            d.feed(RawTouchEvent::Down(tp(slot, 0.0, 0.0)));
        }
        d.feed(RawTouchEvent::Frame);
        d.cancel();

        // Same fingers keep moving: no new Start / Recognized.
        for slot in 0..4 {
            d.feed(RawTouchEvent::Move(tp(slot, 900.0, 0.0)));
        }
        assert_eq!(d.feed(RawTouchEvent::Frame), None);

        for slot in 0..4 {
            d.feed(RawTouchEvent::Up { slot });
        }
        // Fresh touch works again.
        for slot in 0..4 {
            d.feed(RawTouchEvent::Down(tp(slot, 0.0, 0.0)));
        }
        assert_eq!(
            d.feed(RawTouchEvent::Frame),
            Some(GestureEvent::Start { finger_count: 4 })
        );
    }

    #[test]
    fn lifting_a_finger_mid_swipe_cancels() {
        let mut d = GestureDetector::new(test_config());
        for slot in 0..4 {
            d.feed(RawTouchEvent::Down(tp(slot, 0.0, 0.0)));
        }
        d.feed(RawTouchEvent::Frame);
        let cancelled = d.feed(RawTouchEvent::Up { slot: 0 });
        assert_eq!(cancelled, Some(GestureEvent::Cancelled));
    }
}
