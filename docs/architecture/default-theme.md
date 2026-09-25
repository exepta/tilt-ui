# Default Theme

TiltUI ships a compact built-in stylesheet for native controls. The source is
`crates/tilt-ui-runtime/src/theme/default.css`; it is parsed through
`tilt-ui-css` once during `TiltUiStyleRuntimePlugin` setup and retained as one
shared immutable stylesheet.

The stylesheet uses the `DefaultTheme` cascade origin. Component stylesheet
rules use the higher `Author` origin, so an authored declaration overrides a
default declaration without specificity tricks or `!important`.

```rust
app.add_plugins(TiltUiStyleRuntimePlugin::default());
app.add_plugins(TiltUiStyleRuntimePlugin::default().with_default_theme(false));
```

The first form enables the theme, which is the standard behavior. The second
form disables only built-in defaults; authored component CSS continues through
the normal style runtime.

The theme provides Noto Sans typography plus bright-purple, light-surface
styles for buttons, checkboxes, radios, toggle buttons, switch buttons,
editable controls, sliders, progress bars, and field sets. Structural elements
remain neutral. Appearance remains CSS-owned. Control Rust modules contain
semantic state, hierarchy, and input behavior only.

`font-family` currently accepts `sans-serif`, `monospace`, and `ui-symbols`.
The first uses the bundled Noto Sans faces, the second uses Bevy's default
monospace face, and `ui-symbols` selects the bundled Noto Sans Symbols 2 face
for control glyphs. The theme's universal `sans-serif` rule can be overridden
by component CSS. The bundled fonts are Apache-2.0 licensed; attribution is
recorded in `src/theme/fonts/NOTICE.md`.

Checkbox and radio controls expose persistent `::indicator` and `::mark`
parts. Switch controls expose persistent `::track` and `::thumb` parts. These
parts inherit component scope from their owner and may be overridden by author
CSS, for example:

```css
switch-button:checked::track {
    background-color: #00aa66;
}
```
