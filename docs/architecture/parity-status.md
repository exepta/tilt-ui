# Extended UI Parity Status

The implementation is not yet feature-equivalent to `bevy_extended_ui`.
The list below records remaining gaps after the Rust registration, form,
fieldset, table, route, binding, and hot-reload foundations were added.

- [ ] Form validation timing (`Always`/`Interact`) and the complete old
  `validation`/`pattern` rule set. Submit rejection and named text/checkable
  values work; option groups and file values need full form serialization.
- [ ] Route keepalive, path parameters, links that navigate `Router`, and
  component-local outlet behavior beyond the single global route table.
- [x] Old HTML event families (`mousedown`, wheel, key, drag, touch, focus,
  scroll, init) route to Rust handlers with event data; click/change/submit remain wired.
- [x] Runtime literal text, HTML fragment and reactive text setters, plus
  `innerText`/`innerHtml` property bindings. Per-element Rust cursor overrides
  support system icons and custom images with inherited selection.
- [x] Dynamic attribute behavior is audited in the [property binding matrix](property-bindings.md).
  Inline CSS, classes, control values, validation limits, and selected widget
  options update existing entities; anatomy-changing options remain creation-time.
- [x] JSON paths, arithmetic, logical and ternary expressions, reactive
  collection rendering, `@use` imports, and explicit pure methods registered
  from `.component.rs` are available. Legacy controller lookup is intentionally
  omitted in favor of component logic.
- [x] Optional file-backed Fluent localization with locale selection,
  variables, fallback, and live language switching. Properties is intentionally omitted.
- [ ] General system/UI dialogs, modal controls, result payloads, and backdrop
  behavior. File-input dialogs already exist separately.
- [x] Scoped theme providers, extensible provider registration, and named theme switching.
- [ ] CSS parity: authored grid tracks/placement, custom properties, `calc`,
  images/gradients, shadows, outlines, cursor/pointer-events/z-index,
  additional typography, `!important`, visual subtree opacity, motion fill
  modes, and reduced-motion behavior.
- [x] The required `src-ui/index.html` uses HTML5 recovery and mounts its
  body once; malformed component/fragment markup falls back to the same parser.
  Well-formed component templates retain the faster XML-like path.
- [ ] Widget details: authored icons, image-file preview and alt fallback,
  calendar keyboard/localization, browser hyperlink navigation, tooltip nose
  and multiwindow viewport support, and full legacy input validation.
- [ ] Legacy screen registry. Automatic camera creation accepts camera,
  render-layer, and HDR options, or can be disabled for an application camera.
- [ ] Measure frame time and perceived scroll response in a live window.
  Body wheel input now moves immediately, while nested regions retain a
  one-viewport smooth-scroll lag bound; burst regression tests do not measure
  rendering time. The showcase starts, but automated input is unavailable
  without macOS Accessibility permission in this environment.
- [ ] Confirm ColorPicker drag latency in the user's window setup and investigate
  any remaining frame-time spikes. A repeatable hover-triggered style recascade
  dropped from about 60 ms to about 3 ms in the debug showcase after selector
  caching and partial restyling; this does not rule out other spikes.
