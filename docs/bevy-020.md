# Bevy 0.20 Preview

The `preview-020` Git branch targets Bevy 0.20.0. Published TiltUI 0.1.0 packages
continue to target Bevy 0.19. The preview retains its workspace package version;
it is consumed through Git and does not publish a new crates.io version.

## Dependencies

Use Rust 1.97.1 or newer and select the same preview branch or commit for
`tilt-ui` and `tilt-ui-build`. This repository pins its development toolchain
in `rust-toolchain.toml`. The Bevy crates are updated together; Parley 0.11 and
wgpu-types 30 match Bevy's text and rendering dependencies.

```toml
[dependencies]
bevy = "0.20.0"
tilt-ui = { git = "https://github.com/exepta/tilt-ui", branch = "preview-020" }

[build-dependencies]
tilt-ui-build = { git = "https://github.com/exepta/tilt-ui", branch = "preview-020" }
```

## Runtime Integration

- Pointer messages use the flat Bevy 0.20 types such as `PointerPress`,
  `PointerClick`, and `PointerDrag`. Position and ID are obtained from each
  event's `pointer` field. Authored HTML event names and payloads are unchanged.
- Lifecycle observers use `On<Add<T>>` and `On<Remove<T>>`.
- Text scrolling reads and updates the native editor's `viewport.offset`.
  Wheel scrolling, scrollbar dragging, caret placement, selection, and IME
  placement use the same viewport. A normal text edit does not emit an HTML
  scroll event unless its viewport actually moves.
- TiltUI buttons use the existing `TiltButton` marker and its required node,
  focus policy, and native `Hovered` component. Query `TiltButton` rather than
  Bevy's deprecated `ui::widget::Button`. Active state uses `Pressed` and the
  existing TiltUI pointer/keyboard handling.
- The four built-in UI shaders use WESL imports and `.wesl` files. Render-world
  extraction explicitly targets `RenderApp`. Fullscreen effects intersect the
  transformed ancestor clip bounds supplied by Bevy 0.20.

The `preview-020` branch runs the workspace/example tests and browser check in
GitHub Actions. A push to this branch does not trigger a package release.

## Validation

Validated on Linux with Rust 1.97.1:

- `cargo fmt --all --check`
- `cargo check --workspace --all-features --locked`
- `cargo test --workspace --all-features --locked`: 431 tests passed.
- `cargo build --workspace --all-features --locked`
- `cargo check -p tilt-ui-example-wasm --target wasm32-unknown-unknown --locked`
- `node --test scripts/release.test.mjs`: 10 tests passed.
- `cargo clippy --workspace --all-features --all-targets --locked` completes,
  but existing repository lints prevent a clean `-D warnings` run.

Native Vulkan smoke tests ran on an AMD Radeon RX 6950 XT. The widget probe
recorded 126 scroll-position changes from 720 wheel events and 230 ColorPicker
value changes from 230 injected pointer events, with at most one frame of
response latency. The probe tracks the actual overflowing body rather than
the document's empty outer body and isolates its synthetic mouse hover from
the physical pointer. The CSS showcase was captured and visually checked for
custom borders and image filters without shader errors. These are short UI
smoke tests, not a Teramic benchmark or an exhaustive visual test of every
widget and live effect.
