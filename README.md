# swipecore

Backend-agnostic four-finger (and generally N-finger) swipe gesture
detection engine, part of [HeroineOS](https://github.com/HeroineOS).

Inspired by Huawei laptops' four-finger swipe between Windows and HarmonyOS.
`swipecore` is the shared recognition engine meant to be reused by:

- **tty-swipe** (HeroineOS) — swipe between active TTYs with no keybind held
- a future **os-swipe** (HeroineOS) — swipe between two running OSes without VM software (design not yet started)
- a separate Android/Termux + Termux:GUI hybrid-OS project (external to HeroineOS)

## Design

Two layers, kept deliberately decoupled:

- [`gesture`] / [`detector`] — pure gesture recognition state machine. No
  I/O, no OS assumptions. Takes raw per-finger touch points, emits
  classified `GestureEvent`s (`Start`, `Update`, `Recognized`, `Cancelled`).
- [`backend`] — turns a real input source into the raw events the detector
  consumes. Ships with a Linux `evdev` backend today (reads
  `/dev/input/eventN` directly — works with no compositor/X/Wayland running,
  which is required for the TTY-switching use case). Future backends
  (Termux:GUI on Android, an OS-switch backend) implement the same
  `InputBackend` trait without touching recognition logic.

## Platform support

- **Primary/official:** ARM64 (`aarch64`), AMD64 (`x86_64`)
- **Best-effort:** ARM32 (`armv7`), x86 (`i686`)

## Status

Early scaffold — gesture detector is implemented and tested; the evdev
backend compiles but hasn't been validated against real touchpad hardware
yet.

## License

MIT OR Apache-2.0
