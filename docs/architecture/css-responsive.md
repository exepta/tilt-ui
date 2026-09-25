# Responsive CSS

TiltUI parses `@media` blocks into typed `MediaRule` and `MediaCondition`
values in `tilt-ui-css`. Conditions are never reparsed in the Bevy runtime.
The initial environment is the logical size of Bevy's `PrimaryWindow`, exposed
internally as `TiltUiMediaEnvironment`; this is also the normal canvas size on
WASM. Multiple windows and camera-specific UI targets are intentionally a
future viewport-provider extension.

Supported media types are `all` and `screen`. Other media types parse as
unsupported and do not match. Width and height support `min-*`, `max-*`, exact
syntax, comparisons, and chained ranges. `px`, `vw`, and `vh` are valid
breakpoint units. Conditions support `and`, `or`, `not`, comma-separated OR
alternatives, and `orientation: portrait|landscape`. Portrait means height is
strictly greater than width; landscape includes a square viewport.

```css
@media screen and (width >= 900px) { .panel { width: 700px; } }
@media (600px < width < 900px), (orientation: portrait) {
    .panel { width: 500px; }
}
```

Rules inside matching blocks use their original global source-order numbers,
so normal cascade precedence remains intact around media blocks. On a window
resize, only component scopes whose stylesheet has media rules are considered.
The runtime records each stylesheet's prior media-match vector and marks the
scope dirty only when that vector changes. No responsive selector matching is
performed while viewport results remain unchanged.
