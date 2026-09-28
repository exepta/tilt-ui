# TiltUI

A component-based HTML and CSS UI framework for Bevy.

Developed by tilt-us.

Status: Early development.

## Getting started

Every project needs `src-ui/index.html`. Its `<body>` is mounted once as the
root TiltUI tree; `<link rel="stylesheet" href="...">` in `<head>` loads
global CSS from `src-ui/` in document order. For example:

```html
<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <title>My UI</title>
  <link rel="stylesheet" href="styles/site.css">
</head>
<body><app-main></app-main></body>
</html>
```

Place component triplets (`name.component.html`,
`name.component.css`, `name.component.rs`) directly in that directory or in any
subdirectory. A top-level `pages/` directory is recognized as pages for
compatibility, but neither `pages/` nor `components/` is created or required.
The build script rejects a missing entry document or broken local stylesheet
links. Complete documents use HTML5 error recovery; already well-formed
component templates retain the faster existing parser and use recovery only
when needed. Parsing occurs on asset load, not every frame.

In `build.rs`, discover the source root:

```rust
fn main() {
    tilt_ui_build::build().expect("TiltUI component discovery failed");
}
```

Install the asset source, runtime, controls, and UI camera with one plugin.
It must precede `DefaultPlugins` so the `tilt-ui://` source is registered before
Bevy creates its asset server:

```rust
use bevy::prelude::*;
use tilt_ui::prelude::*;

tilt_ui::include_components!();

fn main() {
    App::new()
        .add_plugins(TiltUiPlugin::new(tilt_ui_component_catalog())
            .with_source_root(concat!(env!("CARGO_MANIFEST_DIR"), "/src-ui")))
        .add_plugins(DefaultPlugins)
        .run();
}
```

For another source directory, use the same absolute path in the plugin and
in `build.rs` via `tilt_ui_build::build_from(&UiSourceRoot::new(path))`.
On WASM, use a relative asset URL root and copy `src-ui/` into the served
directory. [The Trunk showcase](examples/wasm-showcase/README.md) demonstrates
both browser `index.html` and TiltUI `src-ui/index.html`.
`TiltUiPlugin::with_camera(camera)` customizes the automatically spawned
`Camera2d`; `without_camera()` leaves camera creation to the application.
Mark a manually created camera with Bevy's `IsDefaultUiCamera`.

Frame pacing is optional. Without a setting, Bevy's normal window loop applies;
the showcase starts at a 60-FPS target. Choose a preset or a positive custom
value on the same plugin:

```rust
use tilt_ui::UiFrameRate;

let plugin = TiltUiPlugin::new(tilt_ui_component_catalog())
    .with_ui_fps(UiFrameRate::Fps45);
// For example: UiFrameRate::custom(144).expect("positive FPS")
```

Change the `UiFrameRate` resource to switch targets at runtime. TiltUI shares
Bevy's update/render loop, so this caps the whole app, including updates
triggered by input. Explicit FPS targets use non-VSync presentation where
supported; heavy work or a platform fallback may still lower the achieved FPS.

## Routing

Define routes with `Routes::new().route("/", home).route("/settings", load!(settings)).route("/details", lazy!(details))`. A plain route creates a fresh component on each visit. `load!` creates a retained instance as soon as an outlet exists; `lazy!` creates one on first visit and retains it. `lazy!` delays instance creation; component assets still follow TiltUI's normal startup preparation. Inactive retained instances are hidden. Redirects and fallbacks work with all three forms. `Routes::merge` combines tables, and `Router::set_routes` or `Router::merge` changes them while the app runs. Paths are normalized before matching.

See [`routing-showcase`](examples/routing/README.md) for a runnable example. Change `ResMut<UiCameraConfiguration>` to update the automatically created UI camera's render layers or HDR setting at runtime.

An optional `src-ui/routers.rs` is included automatically by `include_components!()`. It can register a combined route table with `#[ui_routes]`. Route files such as `features/bob.route.rs` can live in any subdirectory under `src-ui`; import them from `routers.rs` with a relative `#[path = "..."] mod bob;` and combine their `Routes` with `merge`.

## UI lifecycle

Query `UiState` on a component boundary, or read `UiDocumentState` for the
entry document. `load` is `Loading`, `Loaded`, `Ready`, or `Error`; `visible` is
`None` until readiness is checked and then tracks effective visibility.
`Loaded` means the required template and stylesheets are available. `Ready`
means the tree exists and initial bindings and styles have run. The document
waits for its linked stylesheets and nested components before becoming `Ready`.

Listen for transitions in a Bevy system. The target is `Document` or a
`Component(Entity)`, and errors carry a stable code plus a message:

```rust
use bevy::prelude::*;
use tilt_ui::prelude::*;

fn react_to_ui(mut events: MessageReader<UiStateEvent>) {
    for event in events.read() {
        match event {
            UiStateEvent::Ready(UiStateTarget::Component(entity)) => {
                info!("Component {entity:?} is ready");
            }
            UiStateEvent::Error { code, message, .. } => {
                warn!("UI error {code:?}: {message}");
            }
            _ => {}
        }
    }
}

// Add to your App when same-frame delivery matters:
// .add_systems(Update, react_to_ui.after(UiStateRuntimeSet::Observe))
```

Hot reload can move an instance back to `Loading`; route changes report
`Hidden` for a visible instance before it is removed. The current state
remains queryable after its message has been consumed.
The [Basic example](examples/basic/README.md) shows these values and lets you
trigger `Hidden` and `Visible` from the UI.

The [component showcase](examples/component-showcase/src-ui/pages/showcase.component.rs)
demonstrates `#[component_init]`, `#[component_update]`, `#[html_shared]`,
and `#[html_fn]`, including a Rust-driven animated progress bar. These
functions register automatically from the discovered component source; `main.rs`
does not register a component-specific plugin. Run it with
`cargo run -p tilt-ui-example-component-showcase`.
The showcase has English/German language and light/dark theme selectors in its
header, plus a 30/45/60/custom FPS control below it. Set
`TILT_UI_SHOWCASE_THEME=dark` to start with dark surfaces; set
`TILT_UI_SHOWCASE_LANG=de-DE` to start directly in German; use
`TILT_UI_SHOWCASE_SIZE=390x844` to inspect its narrow layout.

## CSS priority and calculated values

Component CSS, document stylesheets, themes, and static or bound inline styles
share one cascade. `!important` takes precedence over normal declarations;
specificity and source order break ties within a layer. Edge shorthands and
individual sides are resolved independently.

```css
:root { --gutter: 12px; --accent: #8424f5; }
.card {
  padding: max(var(--gutter), 2vw);
  width: calc(50% + 8px);
  color: var(--accent, #000000) !important;
}
```

Custom properties inherit through the UI tree and update when an inline style
or theme changes. `calc()`, `min()`, `max()`, and `sin()` can be nested; `sin()`
accepts numbers in radians or angles in `deg`/`rad`. Mixed `%`, `px`, `vw`, and
`vh` lengths resolve against the containing block and viewport, then update
after layout or viewport changes. A missing or cyclic variable can use the
second argument of `var()` as a fallback.
The [widget showcase stylesheet](examples/component-showcase/src-ui/pages/showcase.component.css)
uses all four functions. Its light and dark themes set `--showcase-accent`;
the help button's `!important` border color overrides its static inline color.

Common layout CSS now maps to Bevy's block, flex, and grid layouts. For example:

```css
.gallery {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  grid-auto-rows: auto;
  gap: 12px;
}
.gallery > .featured { grid-column: 1 / span 2; }
.card {
  box-sizing: border-box;
  border: 1px solid #d8d4ea;
  box-shadow: 0 4px 12px #00000033;
  &:hover { outline: 2px solid var(--accent); outline-offset: 2px; }
}
```

`flex`/`flex-flow`, side-specific solid borders, `line-height`, `text-wrap`,
`text-transform`, named font families, `cursor`, `pointer-events`, `z-index`,
and `scroll-width` also work in stylesheets. `background-image` accepts linear
gradients and `url(...)`. `background-size: stretch | cover | contain` controls
fitting; `background-position` accepts edge/center keywords or percentages for
cover and contain images, and `background-attachment: fixed` pins cover crops to the viewport.
`background-filter` accepts chains of `blur(px)`, `grayscale(amount)` (also
`black-white`), `oil-paint(radius)`, `contrast(amount)` and `invert(amount)`.
Filtered images are processed asynchronously at most twice in parallel, capped
in resolution, and shared by source asset and filter chain. `backdrop-filter`
supports live `blur`, `grayscale`, `contrast`, and `invert` on dialogs and regular
UI elements. Filtered content stays sharp above the scene, and rounded corners
and scroll clipping bound the GPU treatment. Up to eight visible backdrops share
one GPU pass; no pixel readback is used.

`animated-filter` accepts `noise(strength, speed)`, `retro-tv(...)`,
`old-film(...)`, `side-glow(...)`, and `bloom(...)`, including chains of effects.
It treats the composited pixels inside the element's bounds, including child
content.
Strength ranges from `0` to `1`; speed from `0` to `4` and defaults to `1`.
`effect-quality: auto | low | medium | high` selects shader sampling quality;
`auto` uses low quality on WASM, iOS, Android, and narrow viewports. Animated
Up to eight visible animated effects use the same GPU pass as backdrops, which
is removed when no visible effect is active. The
[widget showcase](examples/component-showcase/src-ui/pages/showcase.component.html)
activates its previews on hover or touch so scrolling stays smooth. Named grid
areas and dashed borders remain open in [TODO.md](TODO.md).
`opacity` multiplies drawable alpha
through the subtree; overlapping children are not composited as one isolated
browser layer.
On WASM, media queries and `vw`/`vh` use the browser viewport even when the
Bevy canvas has a different size; native builds use the primary Bevy window.

## Providers and themes

On native targets, `UiRuntimeConfiguration` discovers `.css` files below any
directory relative to the configured `src-ui` source. Each file stem becomes
its theme name; `theme_names` can restrict the set. The bundled default theme
remains the lowest cascade layer; named themes sit above it and below component CSS.

```rust
use tilt_ui::{TiltUiPlugin, UiRuntimeConfiguration};

let files = UiRuntimeConfiguration::default()
    .with_themes_path("themes")?
    .with_theme_names(["light", "dark"]);
let plugin = TiltUiPlugin::new(tilt_ui_component_catalog())
    .with_runtime_configuration(files);
app.add_plugins(plugin).add_plugins(DefaultPlugins);
// Select a discovered theme from a Startup system or later.
```

Explicit registration is also supported:

```rust
use tilt_ui::{UiThemeAppExt, switch_ui_theme};

app.register_ui_theme_css("light", include_str!("../src-ui/themes/light.css"))?;
app.register_ui_theme_css("dark", include_str!("../src-ui/themes/dark.css"))?;
switch_ui_theme(app.world_mut(), "light")?;
```

`<theme-provider theme="dark">...</theme-provider>` pins a subtree to a named
theme. Without `theme`, it follows the active global theme, or uses its
`default` attribute when no theme is active. Provider nodes are layout-neutral
and also affect nested components. Custom tags implement `UiProvider` and are
registered with `app.register_ui_provider(provider)`; their `ProviderEffect`
may select a theme or supply parsed CSS for their subtree. An unknown theme
returns an error without changing the current selection.

## Localization (optional)

Enable the `fluent` feature on `tilt-ui` to load Fluent `.ftl` catalogs from
the configured `src-ui` directory. No `.properties` parser is included.

```toml
tilt-ui = { version = "0.1", features = ["fluent"] }
```

For example, place `locales/en-US.ftl` and `locales/de-DE.ftl` under `src-ui/`:

```ftl
welcome = Hello, { $name }!
```

Discover them from the directory, using file stems as locale tags:

```rust
use tilt_ui::{TiltUiPlugin, UiRuntimeConfiguration};

let files = UiRuntimeConfiguration::default()
    .with_language_path("locales")?;
let plugin = TiltUiPlugin::new(tilt_ui_component_catalog())
    .with_runtime_configuration(files);
```

Or register individual catalogs on the same plugin:

```rust
use tilt_ui::{TiltUiPlugin, UiFluentConfig};

let fluent = UiFluentConfig::new("en-US")?
    .with_catalog("en-US", "locales/en-US.ftl")?
    .with_catalog("de-DE", "locales/de-DE.ftl")?;
let plugin = TiltUiPlugin::new(tilt_ui_component_catalog())
    .with_localization(fluent);
```

Use `{{ i18n.welcome }}` in template text or `[text]="i18n.welcome"`
on an element; Fluent attributes use `{{ i18n.welcome.tooltip }}`. Set Fluent arguments from component logic with
`ResMut<UiFluentArgs>::set("welcome", "name", "Ada")`, and switch language with
`ResMut<UiLocalization>::set_locale("de-DE")`. Text updates automatically.
Language lookup tries the selected locale, its base language, then the
configured fallback. Missing translations display their message ID; invalid
catalog reloads leave the last valid catalog in place.

`UiRuntimeConfiguration` is a Bevy resource: changing its directories during
an update discovers the new files and removes entries previously discovered
from the old directories. Call `refresh_ui_directories(world)` after editing
files in place. `components_path` relocates assets for components already
found at build time; existing instances are reloaded. `assets_path` prefixes
relative image sources and refreshes existing images. Explicit `tilt-ui://`
paths are preserved. The physical source root remains fixed because Bevy
registers it before `AssetPlugin`. Directory scanning uses the native file
system; on WebAssembly, register themes and catalogs explicitly.

## Text interaction

Input and TextArea characters enter with a short, damped motion by default.
Set `UiMotionSettings.input_text_seconds` to `0.0` to disable it globally, or
use `text-animation="none"` on one field. Paragraphs, H1-H6 headings, labels,
and text directly inside noninteractive containers support drag selection and
Ctrl/Cmd+C. The selection follows wrapped lines and Unicode text.

Right-clicking an Input or TextArea opens the default `ContextMenu` with Copy
(when nonempty), Paste, and Clear as applicable. Right-clicking selected static
text offers Copy. You can also attach your own menu to an element ID:

```html
<button id="item-actions">More actions</button>
<context-menu for="item-actions">
    <button onclick="reset_progress">Reset progress</button>
</context-menu>
```

## Dialogs

Use `<dialog>` for an in-window modal. `trigger` names an element ID in the
same component; `layout="bottom-sheet"` anchors its panel to the bottom.
The dialog closes on Escape, a backdrop click, the built-in close button, or
an authored button with `dialog-close="confirm|cancel|close"`:

```html
<button id="open-settings">Open settings</button>
<dialog trigger="open-settings" title="Settings" layout="bottom-sheet">
    <p>Apply these changes?</p>
    <button dialog-close="cancel">Cancel</button>
    <button dialog-close="confirm">Apply</button>
</dialog>
```

`renderer="system"` opens a native message dialog instead. `type` accepts
`info`, `warning`, `error`, `question`, or `blank`; native dialogs use the
platform's message buttons, while Bevy dialogs retain their authored content.
The native renderer uses the runtime's default `file-dialog` feature; without
it, `renderer="system"` falls back to the in-window modal. Rust code can call
`open_dialog(world, entity)` and `close_dialog(world, entity, result)` and read
`DialogClosed` messages for the result.

To create a dialog entirely from a component's Rust logic, send `ShowDialog`
with a materialized element as `parent`:

```rust
#[html_fn("confirm_action")]
fn confirm_action(
    In(click): In<HtmlClick>,
    mut dialogs: MessageWriter<ShowDialog>,
) {
    dialogs.write(ShowDialog {
        parent: click.target,
        config: DialogConfig::question("Continue?", "Apply the changes now?"),
    });
}
```

`DialogSpawned` reports the new entity. Rust-created dialogs are removed when
closed; template-authored dialogs remain available for the next opening.
