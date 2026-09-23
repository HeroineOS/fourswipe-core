//! Raw evdev backend: reads directly from `/dev/input/eventN`, bypassing
//! libinput/X11/Wayland entirely. This is what makes four-finger TTY
//! switching possible — it works with no compositor or display server
//! running at all, which is exactly the environment a TTY switch happens in.

use std::path::PathBuf;

use evdev::{AbsoluteAxisType, Device, EventType, InputEventKind};

use super::{BackendError, InputBackend};
use crate::gesture::{RawTouchEvent, TouchPoint};

/// Finds the first input device that looks like a multitouch touchpad
/// (i.e. it reports `ABS_MT_POSITION_X`/`ABS_MT_POSITION_Y`).
pub fn find_touchpad() -> Result<(PathBuf, Device), BackendError> {
    for (path, device) in evdev::enumerate() {
        let supports_mt = device
            .supported_absolute_axes()
            .map(|axes| {
                axes.contains(AbsoluteAxisType::ABS_MT_POSITION_X)
                    && axes.contains(AbsoluteAxisType::ABS_MT_POSITION_Y)
            })
            .unwrap_or(false);
        if supports_mt {
            return Ok((path, device));
        }
    }
    Err(BackendError::NoDevice)
}

/// Tracks type-B multitouch protocol state (the `ABS_MT_SLOT` /
/// `ABS_MT_TRACKING_ID` protocol nearly all modern touchpads use) and
/// translates it into [`RawTouchEvent`]s.
pub struct EvdevBackend {
    device: Device,
    current_slot: i32,
    /// slot -> in-progress (x, y); x/y may arrive on separate raw events
    /// before a SYN_REPORT flushes them as one [`RawTouchEvent`].
    pending: std::collections::HashMap<i32, (f64, f64)>,
    /// Slots that received a fresh `ABS_MT_TRACKING_ID` this frame, so the
    /// next SYN_REPORT emits `Down` instead of `Move` for them.
    new_slots: std::collections::HashSet<i32>,
    queue: std::collections::VecDeque<RawTouchEvent>,
}

impl EvdevBackend {
    pub fn new(device: Device) -> Self {
        Self {
            device,
            current_slot: 0,
            pending: std::collections::HashMap::new(),
            new_slots: std::collections::HashSet::new(),
            queue: std::collections::VecDeque::new(),
        }
    }

    pub fn open_default() -> Result<Self, BackendError> {
        let (_path, device) = find_touchpad()?;
        Ok(Self::new(device))
    }
}

impl InputBackend for EvdevBackend {
    fn next_event(&mut self) -> Result<RawTouchEvent, BackendError> {
        loop {
            if let Some(ev) = self.queue.pop_front() {
                return Ok(ev);
            }

            let events = self
                .device
                .fetch_events()
                .map_err(|e| match e.kind() {
                    std::io::ErrorKind::PermissionDenied => BackendError::PermissionDenied,
                    _ => BackendError::Io(e),
                })?;

            for ev in events {
                if ev.event_type() != EventType::ABSOLUTE
                    && ev.event_type() != EventType::SYNCHRONIZATION
                {
                    continue;
                }

                match ev.kind() {
                    InputEventKind::AbsAxis(AbsoluteAxisType::ABS_MT_SLOT) => {
                        self.current_slot = ev.value();
                    }
                    InputEventKind::AbsAxis(AbsoluteAxisType::ABS_MT_TRACKING_ID) => {
                        if ev.value() == -1 {
                            self.pending.remove(&self.current_slot);
                            self.queue.push_back(RawTouchEvent::Up {
                                slot: self.current_slot,
                            });
                        } else {
                            self.pending.entry(self.current_slot).or_insert((0.0, 0.0));
                            self.new_slots.insert(self.current_slot);
                        }
                    }
                    InputEventKind::AbsAxis(AbsoluteAxisType::ABS_MT_POSITION_X) => {
                        let entry = self.pending.entry(self.current_slot).or_insert((0.0, 0.0));
                        entry.0 = ev.value() as f64;
                    }
                    InputEventKind::AbsAxis(AbsoluteAxisType::ABS_MT_POSITION_Y) => {
                        let entry = self.pending.entry(self.current_slot).or_insert((0.0, 0.0));
                        entry.1 = ev.value() as f64;
                    }
                    InputEventKind::Synchronization(_) => {
                        for (&slot, &(x, y)) in self.pending.iter() {
                            let point = TouchPoint { slot, x, y };
                            if self.new_slots.remove(&slot) {
                                self.queue.push_back(RawTouchEvent::Down(point));
                            } else {
                                self.queue.push_back(RawTouchEvent::Move(point));
                            }
                        }
                        self.queue.push_back(RawTouchEvent::Frame);
                    }
                    _ => {}
                }
            }

            if self.queue.is_empty() {
                // fetch_events() blocks internally until data is available,
                // so an empty batch here just means non-touch events were
                // filtered out — loop back and read again.
                continue;
            }
        }
    }
}
