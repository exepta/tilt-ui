# Performance validation for the 0.1.0 release

The component showcase has an opt-in, in-window probe for the two interaction
paths that previously caused frame drops. It runs only when requested:

```sh
TILT_UI_SHOWCASE_SIZE=960x720 TILT_UI_PERF_PROBE=1 cargo run -p tilt-ui-example-component-showcase --locked
```

The probe waits 90 frames for startup, then samples 120 frames each of idle,
body scrolling, ColorPicker dragging, repeated body scrolling, and repeated
ColorPicker dragging. It exits automatically. It injects three line-wheel
events per scroll frame through Bevy Picking and Pointer Press/Drag messages
through the ColorPicker's normal input handler. The pointer's hover target is
kept stable throughout the measurement so switching the synthetic hover map
does not create an unrelated CSS restyle at the start of each phase. The
ColorPicker palette is opened in the window; its canvas receives the pointer
events. The probe logs per-phase p50, p95, maximum frame interval, frames over
33.3 ms, and the number of events that changed state.

On an Apple M5 with Metal, a 960 × 720 window, and the Cargo `dev` profile,
the 2026-10-02 run produced:

| Phase | p95 frame interval | Maximum | Frames >33.3 ms |
| --- | ---: | ---: | ---: |
| Idle | 19.12 ms | 19.39 ms | 0 / 120 |
| Body scroll, first pass | 19.19 ms | 19.47 ms | 0 / 120 |
| ColorPicker drag, first pass | 19.20 ms | 20.91 ms | 0 / 120 |
| Body scroll, second pass | 19.39 ms | 19.89 ms | 0 / 120 |
| ColorPicker drag, second pass | 19.24 ms | 19.71 ms | 0 / 120 |

The scroll phases delivered 720 wheel events and changed the body's scroll
position in 180 frames. The picker received 230 pointer events, and its color
changed 230 times, each detected by the following frame. The result verifies
the normal Bevy input handlers and window frame pacing for this workload. It
does not measure subjective wheel
feel, physical mouse latency, or other hardware. macOS Accessibility and
Screen Recording were unavailable in this environment, so manual pointer and
visual inspection could not be automated. Run the showcase normally for that
final human check.

Without a stable synthetic hover target, switching the probe between phases
caused an approximately 125 ms frame from changing the hover state of the
whole document. That is not part of the steady scroll or drag measurement;
it is a separate whole-page hover transition and should be profiled if it is
observed during normal mouse use.
