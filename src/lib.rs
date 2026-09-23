//! swipecore: backend-agnostic multi-finger swipe gesture detection.
//!
//! This crate deliberately splits into two layers:
//! - [`gesture`] / [`detector`]: pure recognition logic, no I/O, no OS
//!   assumptions. Reused unchanged by every consumer.
//! - [`backend`]: turns a real input source (raw evdev today; Termux:GUI
//!   or another OS's input API later) into the events the detector consumes.
//!
//! Consumers (heroineos tty-swipe, a future os-swipe, an Android hybrid-OS
//! project) each pick or implement a [`backend::InputBackend`] and feed its
//! events into a [`detector::GestureDetector`] to get classified
//! [`gesture::GestureEvent`]s back.

pub mod backend;
pub mod detector;
pub mod gesture;

pub use detector::GestureDetector;
pub use gesture::{GestureConfig, GestureEvent, RawTouchEvent, SwipeDirection, TouchPoint};
