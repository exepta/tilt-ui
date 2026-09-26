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
- [ ] Full expression evaluation, `innerHtml`/`innerBindings`, `@use`, scoped
  controller state, and complex reactive collection rendering. Simple paths,
  property setters, and text interpolation are wired.
- [x] Optional file-backed Fluent localization with locale selection,
  variables, fallback, and live language switching. Properties is intentionally omitted.
- [ ] General system/UI dialogs, modal controls, result payloads, and backdrop
  behavior. File-input dialogs already exist separately.
- [x] Scoped theme providers, extensible provider registration, and named theme switching.
- [ ] CSS parity: authored grid tracks/placement, custom properties, `calc`,
  images/gradients, shadows, outlines, cursor/pointer-events/z-index,
  additional typography, `!important`, visual subtree opacity, motion fill
  modes, and reduced-motion behavior.
- [ ] Full HTML5 error-recovering parsing. Table rows and sections are
  flattened, but templates still use an XML-like parser.
- [ ] Widget details: authored icons, image-file preview and alt fallback,
  calendar keyboard/localization, browser hyperlink navigation, tooltip nose
  and multiwindow viewport support, and full legacy input validation.
- [ ] Configurable automatic UI camera management and a legacy screen registry.
- [ ] Confirm ColorPicker drag latency in the user's window setup and investigate
  any remaining frame-time spikes. A repeatable hover-triggered style recascade
  dropped from about 60 ms to about 3 ms in the debug showcase after selector
  caching and partial restyling; this does not rule out other spikes.
